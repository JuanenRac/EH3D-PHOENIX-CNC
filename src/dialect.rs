//! The reports of GRBL 1.1 and of grblHAL read as data: the build (`$I`), the settings (`$$`) and the state of the parser (`$G`). The
//! program learns from them how fast the machine may go, how far it may travel and which words are in force, and tells the two
//! controllers apart (grblHAL says so in its build and its banner). Nothing here talks to a machine: it reads text that something else
//! got, so it is tested on the lines the two controllers print. The mode of the laser (`$32`) is read and never acted on: this program is
//! not a laser program.
//!
//! Every reader is bounded: it looks at no more than [`MAX_LINES`] lines of at most [`MAX_LINE_BYTES`] bytes and a line it cannot use is
//! counted and left out, never a panic.

use std::collections::BTreeMap;

use eh3d_controller::{Family, AXES};

/// The most lines a report is read from and the longest line that is looked at.
pub const MAX_LINES: usize = 512;
pub const MAX_LINE_BYTES: usize = 512;

/// Which controller printed a report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Kind {
    #[default]
    Unknown,
    Grbl,
    GrblHal,
}

impl Kind {
    /// The family of the controller interface it is driven as.
    pub fn family(self) -> Family {
        match self {
            Kind::GrblHal => Family::GrblHal,
            _ => Family::Grbl,
        }
    }
}

/// What `$I` and the banner say.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Build {
    pub kind: Kind,
    /// The version text (`1.1h.20190825`), empty when it is not said.
    pub version: String,
    /// The options of the build (`V,15,128`) and the extra ones of grblHAL (`ENC,RT+`), as they were said.
    pub options: String,
    pub extra_options: String,
}

fn bounded(lines: &[String]) -> impl Iterator<Item = &str> {
    lines.iter().take(MAX_LINES).filter_map(|l| {
        let l = l.trim();
        (l.len() <= MAX_LINE_BYTES && !l.is_empty()).then_some(l)
    })
}

fn bracket<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let body = line.strip_prefix('[')?.strip_suffix(']')?;
    let rest = body.strip_prefix(key)?.strip_prefix(':')?;
    Some(rest.trim_end_matches(':'))
}

/// Reads the lines of `$I` (and the banner, if it is among them) into a build.
pub fn parse_build(lines: &[String]) -> Build {
    let mut b = Build::default();
    for l in bounded(lines) {
        let lower = l.to_ascii_lowercase();
        if lower.starts_with("grblhal") || lower.contains("[firmware:grblhal") {
            b.kind = Kind::GrblHal;
        } else if lower.starts_with("grbl ") && b.kind == Kind::Unknown {
            b.kind = Kind::Grbl;
        }
        if let Some(v) = bracket(l, "VER") {
            b.version = v.to_string();
            if b.kind == Kind::Unknown {
                b.kind = Kind::Grbl;
            }
        } else if let Some(o) = bracket(l, "OPT") {
            b.options = o.to_string();
        } else if let Some(o) = bracket(l, "NEWOPT") {
            b.extra_options = o.to_string();
            b.kind = Kind::GrblHal;
        }
    }
    b
}

/// The numbered settings of `$$`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Settings {
    pub map: BTreeMap<u32, f64>,
    /// Lines that were not a setting (the `ok`, a banner) or could not be read.
    pub skipped: usize,
}

/// Reads `$N=value` lines (a comment after the value, as grblHAL writes it, is dropped).
pub fn parse_settings(lines: &[String]) -> Settings {
    let mut s = Settings::default();
    for l in bounded(lines) {
        let one = l
            .strip_prefix('$')
            .and_then(|r| r.split_once('='))
            .and_then(|(k, v)| {
                let n: u32 = k.trim().parse().ok()?;
                let v: f64 = v.split_whitespace().next()?.parse().ok()?;
                v.is_finite().then_some((n, v))
            });
        match one {
            Some((n, v)) => {
                s.map.insert(n, v);
            }
            None => s.skipped += 1,
        }
    }
    s
}

impl Settings {
    fn axis(&self, base: u32, axis: usize) -> Option<f64> {
        if axis >= 3 {
            return None;
        }
        self.map.get(&(base + axis as u32)).copied()
    }

    /// The most speed of an axis in mm/min (`$110` to `$112`).
    pub fn max_rate(&self, axis: usize) -> Option<f64> {
        self.axis(110, axis).filter(|v| *v > 0.0)
    }

    /// The acceleration of an axis in mm/s2 (`$120` to `$122`).
    pub fn accel(&self, axis: usize) -> Option<f64> {
        self.axis(120, axis).filter(|v| *v > 0.0)
    }

    /// The travel of an axis in mm (`$130` to `$132`).
    pub fn travel(&self, axis: usize) -> Option<f64> {
        self.axis(130, axis).filter(|v| *v > 0.0)
    }

    /// The deviation of the junctions in mm (`$11`) and the tolerance of the arcs in mm (`$12`).
    pub fn junction_deviation(&self) -> Option<f64> {
        self.map.get(&11).copied().filter(|v| *v > 0.0)
    }
    pub fn arc_tolerance(&self) -> Option<f64> {
        self.map.get(&12).copied().filter(|v| *v > 0.0)
    }

    /// Whether the status report says the position of the machine (`$10` bit 0) or of the work: with neither bit set GRBL reports the
    /// work position. The report of the controller says which it sent (`MPos:` or `WPos:`), so this is only for what to ask.
    pub fn reports_machine_position(&self) -> bool {
        self.map.get(&10).is_some_and(|v| (*v as u32) & 1 == 1)
    }

    /// Whether the controller is in the mode of the laser (`$32`). It is only said: this program never acts on it.
    pub fn laser_flag(&self) -> bool {
        self.map.get(&32).is_some_and(|v| *v != 0.0)
    }

    /// The most feed of the machine in mm/min along the three axes, the lowest of the three, when all three are known.
    pub fn lowest_rate(&self) -> Option<f64> {
        let mut low: Option<f64> = None;
        for a in 0..3 {
            let r = self.max_rate(a)?;
            low = Some(low.map_or(r, |l: f64| l.min(r)));
        }
        low
    }

    /// The accelerations of the three axes, when they are all known.
    pub fn accel3(&self) -> Option<[f64; 3]> {
        Some([self.accel(0)?, self.accel(1)?, self.accel(2)?])
    }
}

/// The state of the parser (`[GC:...]`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GState {
    /// Every word of the report, as it came (`G54`, `M5`, `T0`...).
    pub words: Vec<String>,
    pub motion: Option<String>,
    pub wcs: Option<String>,
    pub plane: Option<String>,
    pub tool: Option<u32>,
    pub feed: Option<f64>,
    pub spindle: Option<f64>,
    /// `Some(true)` for `G21`, `Some(false)` for `G20`.
    pub millimetres: Option<bool>,
    /// `Some(true)` for `G90`, `Some(false)` for `G91`.
    pub absolute: Option<bool>,
}

/// Reads `[GC:G0 G54 G17 G21 G90 G94 M5 M9 T0 F0 S0]` (or the same without the brackets); `None` when it is not a report of the parser.
pub fn parse_gstate(line: &str) -> Option<GState> {
    let line = line.trim();
    if line.len() > MAX_LINE_BYTES {
        return None;
    }
    let body = line.strip_prefix("[GC:")?.strip_suffix(']')?;
    let mut g = GState::default();
    for w in body.split_whitespace().take(64) {
        g.words.push(w.to_string());
        let (head, rest) = w.split_at(1);
        match head {
            "G" => match rest {
                "0" | "1" | "2" | "3" | "38.2" | "38.3" | "38.4" | "38.5" | "80" => {
                    g.motion = Some(w.to_string())
                }
                "54" | "55" | "56" | "57" | "58" | "59" | "59.1" | "59.2" | "59.3" => {
                    g.wcs = Some(w.to_string())
                }
                "17" | "18" | "19" => g.plane = Some(w.to_string()),
                "21" => g.millimetres = Some(true),
                "20" => g.millimetres = Some(false),
                "90" => g.absolute = Some(true),
                "91" => g.absolute = Some(false),
                _ => {}
            },
            "T" => g.tool = rest.parse().ok(),
            "F" => g.feed = rest.parse().ok().filter(|v: &f64| v.is_finite()),
            "S" => g.spindle = rest.parse().ok().filter(|v: &f64| v.is_finite()),
            _ => {}
        }
    }
    Some(g)
}

/// The number of axes a build says it has (grblHAL `[AXS:3:XYZ]`), 3 when it does not say.
pub fn axes_of(lines: &[String]) -> usize {
    for l in bounded(lines) {
        if let Some(a) = bracket(l, "AXS") {
            if let Some(n) = a.split(':').next().and_then(|n| n.parse::<usize>().ok()) {
                return n.clamp(1, AXES);
            }
        }
    }
    3
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(lines: &[&str]) -> Vec<String> {
        lines.iter().map(|l| l.to_string()).collect()
    }

    #[test]
    fn grbl_and_grblhal_are_told_apart_by_the_build_and_the_banner() {
        let g = parse_build(&v(&["[VER:1.1h.20190825:]", "[OPT:V,15,128]", "ok"]));
        assert_eq!(g.kind, Kind::Grbl);
        assert_eq!(g.version, "1.1h.20190825");
        assert_eq!(g.options, "V,15,128");
        assert_eq!(g.kind.family(), Family::Grbl);
        let h = parse_build(&v(&[
            "GrblHAL 1.1f ['$' or '$HELP' for help]",
            "[VER:1.1f.20230416:]",
            "[OPT:VNMSL,35,1024,3]",
            "[NEWOPT:ENC,RT+]",
        ]));
        assert_eq!(h.kind, Kind::GrblHal);
        assert_eq!(h.extra_options, "ENC,RT+");
        assert_eq!(h.kind.family(), Family::GrblHal);
        // the extra options alone say it is grblHAL, a banner alone says it is GRBL, nothing says nothing
        assert_eq!(parse_build(&v(&["[NEWOPT:X]"])).kind, Kind::GrblHal);
        assert_eq!(
            parse_build(&v(&["Grbl 1.1h ['$' for help]"])).kind,
            Kind::Grbl
        );
        assert_eq!(parse_build(&[]).kind, Kind::Unknown);
        // the build a simulated grblHAL prints is told from GRBL's by its extra options
        let sim = parse_build(&v(&["[VER:1.1f.20240401:]", "[NEWOPT:ENC,RT+,TMC]"]));
        assert_eq!(sim.kind.family(), Family::GrblHal);
    }

    #[test]
    fn the_settings_are_read_with_their_comments_and_the_rest_is_counted() {
        let s = parse_settings(&v(&[
            "$0=10",
            "$10=1",
            "$11=0.010",
            "$12=0.002",
            "$32=0",
            "$110=500.000",
            "$111=500.000",
            "$112=250.000 (z max rate)",
            "$120=10.000",
            "$121=10.000",
            "$122=5.000",
            "$130=200.000",
            "$131=200.000",
            "$132=80.000",
            "ok",
            "$x=1",
            "$5=nan",
        ]));
        assert_eq!(s.map.len(), 14);
        assert_eq!(s.skipped, 3);
        assert_eq!(s.max_rate(2), Some(250.0));
        assert_eq!(s.accel3(), Some([10.0, 10.0, 5.0]));
        assert_eq!(s.travel(2), Some(80.0));
        assert_eq!(s.junction_deviation(), Some(0.01));
        assert_eq!(s.arc_tolerance(), Some(0.002));
        assert_eq!(s.lowest_rate(), Some(250.0));
        assert!(s.reports_machine_position());
        assert!(!s.laser_flag());
        // an axis that is not X, Y or Z, and a value that is not positive, are not given
        assert_eq!(s.max_rate(3), None);
        assert_eq!(parse_settings(&v(&["$110=0", "$32=1"])).max_rate(0), None);
        assert!(parse_settings(&v(&["$32=1"])).laser_flag());
    }

    #[test]
    fn the_state_of_the_parser_is_read_word_by_word() {
        let g = parse_gstate("[GC:G0 G54 G17 G21 G90 G94 M5 M9 T3 F120.5 S12000]").unwrap();
        assert_eq!(g.motion.as_deref(), Some("G0"));
        assert_eq!(g.wcs.as_deref(), Some("G54"));
        assert_eq!(g.plane.as_deref(), Some("G17"));
        assert_eq!(g.millimetres, Some(true));
        assert_eq!(g.absolute, Some(true));
        assert_eq!(
            (g.tool, g.feed, g.spindle),
            (Some(3), Some(120.5), Some(12000.0))
        );
        assert_eq!(g.words.len(), 11);
        let r = parse_gstate("[GC:G1 G55 G18 G20 G91 T0 F0 S0]").unwrap();
        assert_eq!((r.millimetres, r.absolute), (Some(false), Some(false)));
        assert!(parse_gstate("<Idle|MPos:0,0,0>").is_none());
        assert!(parse_gstate("[GC:G1").is_none());
    }

    #[test]
    fn the_readers_are_bounded_and_never_panic_on_garbage() {
        // more lines than the limit: only the first ones are read
        let many: Vec<String> = (0..MAX_LINES + 100).map(|i| format!("${}=1", i)).collect();
        assert_eq!(parse_settings(&many).map.len(), MAX_LINES);
        // a line that is too long is left out
        let long = format!("$1={}", "9".repeat(MAX_LINE_BYTES));
        assert!(parse_settings(std::slice::from_ref(&long)).map.is_empty());
        assert!(parse_gstate(&format!("[GC:{}]", "G0 ".repeat(MAX_LINE_BYTES))).is_none());
        // odd bytes of text
        for junk in [
            "", "$", "$=", "$=1", "[", "[]", "[GC:]", "$-1=3", "ü$1=2", "[VER", "[AXS:x]",
        ] {
            let l = vec![junk.to_string()];
            let _ = parse_settings(&l);
            let _ = parse_build(&l);
            let _ = parse_gstate(junk);
            assert_eq!(axes_of(&l), 3);
        }
        assert_eq!(axes_of(&v(&["[AXS:5:XYZAB]"])), 5);
        assert_eq!(axes_of(&v(&["[AXS:99:X]"])), AXES);
    }
}
