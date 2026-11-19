//! An automatic tool changer (ATC) with a lock and a blow, worked out in simulation: for a change from one tool to another, the steps the
//! machine would make (stop the spindle, go up to the safe height, put the tool in its slot of the rack and release the lock, blow the empty
//! spindle clean, take the other tool from its slot and engage the lock, go up again), the lines of G-code that make them and the time they
//! take. The program only works the sequence out: it lists the changes of a program with their time in the report and the statistics, and
//! the time of a tool change in the estimate is this one when the changer is on. Nothing here sends anything to a machine, and the changer
//! is off unless the settings say so.
//!
//! The settings line: `atc = on | safe=5 | rack=0,0,-5 | pitch=20,0 | lock=3,0.5 | blow=4,1`
//! (`on` or `off`; `safe` is the Z of the travel between slots; `rack` is X, Y and Z of the slot of tool 1 and `pitch` the X and Y step to
//! the slot of the next tool; `lock` is the output pin that clamps the tool, on = clamped, and the seconds the clamp needs; `blow` is the output pin
//! of the air and the seconds it blows). Pins are switched with `M64 P` (on) and `M65 P` (off).

use crate::gcode::words;
use crate::heightmap::num;
use crate::words::{say, Msg};

/// The changer, as the settings describe it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AtcCfg {
    pub enabled: bool,
    /// The Z the tool travels at between the slots.
    pub safe_z: f64,
    /// X, Y and Z of the slot of tool 1, where the tool is put down and picked up, and the step to the slot of the next tool.
    pub rack: [f64; 3],
    pub pitch: [f64; 2],
    /// The output that clamps the tool (on = clamped) and the seconds the clamp needs to open or close.
    pub lock_pin: u32,
    pub lock_wait_s: f64,
    /// The output of the air and the seconds it blows.
    pub blow_pin: u32,
    pub blow_s: f64,
}

impl Default for AtcCfg {
    fn default() -> Self {
        AtcCfg {
            enabled: false,
            safe_z: 5.0,
            rack: [0.0, 0.0, 0.0],
            pitch: [20.0, 0.0],
            lock_pin: 3,
            lock_wait_s: 0.5,
            blow_pin: 4,
            blow_s: 1.0,
        }
    }
}

fn pair(v: &str) -> Option<(f64, f64)> {
    let (a, b) = v.split_once(',')?;
    let (a, b) = (a.trim().parse::<f64>().ok()?, b.trim().parse::<f64>().ok()?);
    (a.is_finite() && b.is_finite()).then_some((a, b))
}

fn pin_and_seconds(v: &str) -> Option<(u32, f64)> {
    let (p, s) = v.split_once(',')?;
    let pin = p.trim().parse::<u32>().ok().filter(|p| *p <= 255)?;
    let s = s
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|s| (0.0..=60.0).contains(s))?;
    Some((pin, s))
}

impl AtcCfg {
    /// Reads what follows `atc =`. A line that is not valid gives nothing, so the changer stays off.
    pub fn parse(value: &str) -> Option<AtcCfg> {
        let mut cfg = AtcCfg::default();
        let mut parts = value.split('|');
        cfg.enabled = match parts.next()?.trim().to_ascii_lowercase().as_str() {
            "on" | "yes" => true,
            "off" | "no" => false,
            _ => return None,
        };
        for p in parts {
            let (k, v) = p.split_once('=')?;
            match k.trim().to_ascii_lowercase().as_str() {
                "safe" => {
                    cfg.safe_z = v
                        .trim()
                        .parse::<f64>()
                        .ok()
                        .filter(|z| z.is_finite() && z.abs() <= 100_000.0)?
                }
                "rack" => {
                    let mut it = v.split(',');
                    let mut r = [0.0; 3];
                    for slot in &mut r {
                        *slot = it
                            .next()?
                            .trim()
                            .parse::<f64>()
                            .ok()
                            .filter(|x| x.is_finite() && x.abs() <= 100_000.0)?;
                    }
                    if it.next().is_some() {
                        return None;
                    }
                    cfg.rack = r;
                }
                "pitch" => {
                    let (dx, dy) = pair(v)?;
                    cfg.pitch = [dx, dy];
                }
                "lock" => (cfg.lock_pin, cfg.lock_wait_s) = pin_and_seconds(v)?,
                "blow" => (cfg.blow_pin, cfg.blow_s) = pin_and_seconds(v)?,
                _ => {}
            }
        }
        Some(cfg)
    }

    /// The text that `parse` reads back.
    pub fn to_line(&self) -> String {
        format!(
            "{} | safe={} | rack={},{},{} | pitch={},{} | lock={},{} | blow={},{}",
            if self.enabled { "on" } else { "off" },
            num(self.safe_z),
            num(self.rack[0]),
            num(self.rack[1]),
            num(self.rack[2]),
            num(self.pitch[0]),
            num(self.pitch[1]),
            self.lock_pin,
            num(self.lock_wait_s),
            self.blow_pin,
            num(self.blow_s)
        )
    }

    /// X and Y of the slot of a tool (tool 1 is the first slot; tool 0 is the same slot).
    pub fn slot(&self, tool: u32) -> (f64, f64) {
        let n = f64::from(tool.saturating_sub(1));
        (
            self.rack[0] + n * self.pitch[0],
            self.rack[1] + n * self.pitch[1],
        )
    }
}

/// One thing the changer does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Step {
    SpindleOff,
    /// Up or down to this Z, at the same X and Y.
    ToZ(f64),
    /// Over this X and Y, at the same Z.
    ToXy(f64, f64),
    /// The lock is opened (the output goes off) and the clamp is given its time.
    Release {
        pin: u32,
        wait_s: f64,
    },
    /// The lock is closed (the output goes on) and the clamp is given its time.
    Engage {
        pin: u32,
        wait_s: f64,
    },
    /// The air is on for this long.
    Blow {
        pin: u32,
        seconds: f64,
    },
}

/// The steps of a change from `from` (the tool in the spindle, if any) to `to`: the old tool is put in its slot and the lock opened, the
/// empty spindle is blown clean, and the new tool is taken from its slot and the lock closed.
pub fn plan(cfg: &AtcCfg, from: Option<u32>, to: u32) -> Vec<Step> {
    let mut steps = vec![Step::SpindleOff, Step::ToZ(cfg.safe_z)];
    if let Some(old) = from {
        let (x, y) = cfg.slot(old);
        steps.extend([
            Step::ToXy(x, y),
            Step::ToZ(cfg.rack[2]),
            Step::Release {
                pin: cfg.lock_pin,
                wait_s: cfg.lock_wait_s,
            },
            Step::ToZ(cfg.safe_z),
        ]);
    }
    steps.push(Step::Blow {
        pin: cfg.blow_pin,
        seconds: cfg.blow_s,
    });
    let (x, y) = cfg.slot(to);
    steps.extend([
        Step::ToXy(x, y),
        Step::ToZ(cfg.rack[2]),
        Step::Engage {
            pin: cfg.lock_pin,
            wait_s: cfg.lock_wait_s,
        },
        Step::ToZ(cfg.safe_z),
    ]);
    steps
}

/// The lines of G-code of the steps.
pub fn g_code(steps: &[Step]) -> Vec<String> {
    let mut out = Vec::new();
    for s in steps {
        match *s {
            Step::SpindleOff => out.push("M5".to_string()),
            Step::ToZ(z) => out.push(format!("G0 Z{}", num(z))),
            Step::ToXy(x, y) => out.push(format!("G0 X{} Y{}", num(x), num(y))),
            Step::Release { pin, wait_s } => {
                out.push(format!("M65 P{pin}"));
                out.push(format!("G4 P{}", num(wait_s)));
            }
            Step::Engage { pin, wait_s } => {
                out.push(format!("M64 P{pin}"));
                out.push(format!("G4 P{}", num(wait_s)));
            }
            Step::Blow { pin, seconds } => {
                out.push(format!("M64 P{pin}"));
                out.push(format!("G4 P{}", num(seconds)));
                out.push(format!("M65 P{pin}"));
            }
        }
    }
    out
}

/// The time of the steps in seconds, with the moves at `rapid_mm_min`, starting over the origin at the safe height.
pub fn seconds(cfg: &AtcCfg, steps: &[Step], rapid_mm_min: f64) -> f64 {
    let rapid = rapid_mm_min.max(1.0) / 60.0;
    let (mut x, mut y, mut z) = (0.0_f64, 0.0_f64, cfg.safe_z);
    let mut t = 0.0;
    for s in steps {
        match *s {
            Step::SpindleOff => {}
            Step::ToZ(nz) => {
                t += (nz - z).abs() / rapid;
                z = nz;
            }
            Step::ToXy(nx, ny) => {
                t += (nx - x).hypot(ny - y) / rapid;
                (x, y) = (nx, ny);
            }
            Step::Release { wait_s, .. } | Step::Engage { wait_s, .. } => t += wait_s,
            Step::Blow { seconds, .. } => t += seconds,
        }
    }
    t
}

/// The time of a typical change (from tool 1 to tool 2), which the estimate adds for every change when the changer is on.
pub fn typical_seconds(cfg: &AtcCfg, rapid_mm_min: f64) -> f64 {
    seconds(cfg, &plan(cfg, Some(1), 2), rapid_mm_min)
}

/// The changes of a program: the line (from 0), the tool that was in the spindle and the tool taken (`T` then `M6`).
pub fn changes_of(lines: &[String]) -> Vec<(usize, Option<u32>, u32)> {
    let (mut current, mut pending): (Option<u32>, Option<u32>) = (None, None);
    let mut out = Vec::new();
    for (n, raw) in lines.iter().enumerate() {
        for (l, v) in words(raw) {
            if l == 'T' && (0.0..1e6).contains(&v) && v.fract() == 0.0 {
                pending = Some(v as u32);
            } else if l == 'M' && v == 6.0 {
                if let Some(p) = pending {
                    if current != Some(p) {
                        out.push((n, current, p));
                    }
                    current = Some(p);
                }
            }
        }
    }
    out
}

/// The most changes the report lists one by one.
const LISTED: usize = 20;

/// The lines for the report and the statistics: the changes of the program as the changer would make them, with their time. Nothing when the
/// changer is off or the program changes no tool.
pub fn report_lines(lines: &[String], cfg: &AtcCfg, rapid_mm_min: f64, lang: &str) -> Vec<String> {
    if !cfg.enabled {
        return Vec::new();
    }
    let changes = changes_of(lines);
    if changes.is_empty() {
        return Vec::new();
    }
    let times: Vec<f64> = changes
        .iter()
        .map(|&(_, from, to)| seconds(cfg, &plan(cfg, from, to), rapid_mm_min))
        .collect();
    let mut out = vec![say(
        lang,
        Msg::AtcHeader,
        &[
            changes.len().to_string(),
            crate::sim::hms(times.iter().sum::<f64>()),
        ],
    )];
    for (&(line, from, to), t) in changes.iter().zip(&times).take(LISTED) {
        let args = [
            (line + 1).to_string(),
            from.map_or_else(String::new, |f| f.to_string()),
            to.to_string(),
            num((t * 10.0).round() / 10.0),
            cfg.lock_pin.to_string(),
            cfg.blow_pin.to_string(),
            num(cfg.blow_s),
        ];
        let msg = if from.is_some() {
            Msg::AtcChange
        } else {
            Msg::AtcFirst
        };
        out.push(format!("  {}", say(lang, msg, &args)));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on() -> AtcCfg {
        AtcCfg {
            enabled: true,
            safe_z: 10.0,
            rack: [100.0, 50.0, -5.0],
            pitch: [30.0, 0.0],
            lock_pin: 3,
            lock_wait_s: 0.5,
            blow_pin: 4,
            blow_s: 1.5,
        }
    }

    fn lines(s: &str) -> Vec<String> {
        s.lines().map(str::to_string).collect()
    }

    #[test]
    fn the_changer_is_off_by_default_and_the_line_is_read_and_written_back() {
        assert!(!AtcCfg::default().enabled);
        let c = on();
        assert_eq!(AtcCfg::parse(&c.to_line()), Some(c));
        let l = AtcCfg::parse("ON | rack=1,2,3 | lock=7,2 | blow=8,0.25 | colour=red").unwrap();
        assert!(l.enabled);
        assert_eq!(
            (l.rack, l.lock_pin, l.lock_wait_s),
            ([1.0, 2.0, 3.0], 7, 2.0)
        );
        assert_eq!((l.blow_pin, l.blow_s), (8, 0.25));
        // what is not given keeps its default
        assert_eq!(l.pitch, AtcCfg::default().pitch);
        assert!(!AtcCfg::parse("off").unwrap().enabled);
        // a line with something wrong in it gives nothing, so the changer stays off
        for bad in [
            "",
            "maybe",
            "on | safe=x",
            "on | rack=1,2",
            "on | rack=1,2,3,4",
            "on | lock=300,1",
            "on | lock=3,-1",
            "on | blow=4,61",
            "on | pitch=1",
            "on | safe",
            "on | safe=1e9",
        ] {
            assert_eq!(AtcCfg::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn the_slot_of_a_tool_follows_the_pitch() {
        let c = on();
        assert_eq!(c.slot(1), (100.0, 50.0));
        assert_eq!(c.slot(3), (160.0, 50.0));
        assert_eq!(c.slot(0), (100.0, 50.0));
    }

    #[test]
    fn a_change_puts_the_old_tool_back_blows_and_takes_the_new_one() {
        let c = on();
        let steps = plan(&c, Some(1), 2);
        assert_eq!(
            steps,
            vec![
                Step::SpindleOff,
                Step::ToZ(10.0),
                Step::ToXy(100.0, 50.0),
                Step::ToZ(-5.0),
                Step::Release {
                    pin: 3,
                    wait_s: 0.5
                },
                Step::ToZ(10.0),
                Step::Blow {
                    pin: 4,
                    seconds: 1.5
                },
                Step::ToXy(130.0, 50.0),
                Step::ToZ(-5.0),
                Step::Engage {
                    pin: 3,
                    wait_s: 0.5
                },
                Step::ToZ(10.0),
            ]
        );
        // the first tool of a program has nothing to put back
        let first = plan(&c, None, 1);
        assert_eq!(first.len(), 7);
        assert!(!first.iter().any(|s| matches!(s, Step::Release { .. })));
        assert!(first.iter().any(|s| matches!(s, Step::Blow { .. })));
        // the lock is released before the blow and engaged after it
        let at = |f: fn(&Step) -> bool| steps.iter().position(f).unwrap();
        let (release, blow, engage) = (
            at(|s| matches!(s, Step::Release { .. })),
            at(|s| matches!(s, Step::Blow { .. })),
            at(|s| matches!(s, Step::Engage { .. })),
        );
        assert!(release < blow && blow < engage);
    }

    #[test]
    fn the_steps_make_gcode_with_the_pins_and_the_waits() {
        let g = g_code(&plan(&on(), Some(1), 2));
        assert_eq!(
            g,
            [
                "M5",
                "G0 Z10",
                "G0 X100 Y50",
                "G0 Z-5",
                "M65 P3",
                "G4 P0.5",
                "G0 Z10",
                "M64 P4",
                "G4 P1.5",
                "M65 P4",
                "G0 X130 Y50",
                "G0 Z-5",
                "M64 P3",
                "G4 P0.5",
                "G0 Z10"
            ]
        );
        // the spindle is stopped before anything moves
        assert_eq!(g[0], "M5");
    }

    #[test]
    fn the_time_is_the_moves_at_the_rapid_speed_plus_the_waits() {
        let c = on();
        // 6000 mm/min is 100 mm/s. From the origin at Z10: to (100,50) is ~111.8 mm, down 15, up 15, blow 1.5 s, 0.5 s twice,
        // to (130,50) 30 mm, down 15, up 15
        let steps = plan(&c, Some(1), 2);
        let expect =
            (111.803_398_874_989_48 + 15.0 + 15.0 + 30.0 + 15.0 + 15.0) / 100.0 + 1.5 + 0.5 + 0.5;
        let t = seconds(&c, &steps, 6000.0);
        assert!((t - expect).abs() < 1e-9, "{t} {expect}");
        assert_eq!(typical_seconds(&c, 6000.0), t);
        // a faster machine moves faster and waits the same
        assert!(seconds(&c, &steps, 12_000.0) < t);
        assert!(seconds(&c, &steps, 1.0e9) >= 2.5);
        // no rapid speed does not divide by zero
        assert!(seconds(&c, &steps, 0.0).is_finite());
    }

    #[test]
    fn the_changes_of_a_program_are_found_with_the_tool_that_was_in_the_spindle() {
        let p = lines("T1 M6\nG1 X1 F10\nT1 M6\nT3\nG1 X2\nM6\nT2 M6\nM30");
        assert_eq!(
            changes_of(&p),
            vec![(0, None, 1), (5, Some(1), 3), (6, Some(3), 2)]
        );
        assert!(changes_of(&lines("G1 X1 F10\nM30")).is_empty());
    }

    #[test]
    fn the_report_lists_the_changes_only_when_the_changer_is_on() {
        let p = lines("T1 M6\nG1 X1 F10\nT2 M6\nM30");
        let off = AtcCfg::default();
        assert!(report_lines(&p, &off, 6000.0, "en").is_empty());
        let r = report_lines(&p, &on(), 6000.0, "en");
        assert_eq!(r.len(), 3, "{r:?}");
        assert!(r[0].contains('2'), "{}", r[0]);
        assert!(r[1].contains("T1") && r[1].contains("line 1"), "{}", r[1]);
        assert!(
            r[2].contains("T1") && r[2].contains("T2") && r[2].contains("line 3"),
            "{}",
            r[2]
        );
        assert!(r[2].contains("P3") || r[2].contains("3"), "{}", r[2]);
        // no change, nothing
        assert!(report_lines(&lines("G1 X1 F10"), &on(), 6000.0, "en").is_empty());
        // in other languages too
        for lang in ["es", "de", "fr", "it", "ja", "zh", "pt"] {
            assert_ne!(report_lines(&p, &on(), 6000.0, lang)[0], r[0], "{lang}");
        }
        // a long program lists only the first ones
        let many: String = (0..40).map(|i| format!("T{} M6\n", i + 1)).collect();
        assert_eq!(
            report_lines(&lines(&many), &on(), 6000.0, "en").len(),
            1 + LISTED
        );
    }
}
