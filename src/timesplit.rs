//! The time of a program split by the kind of move: the rapids, the cuts along the work, the plunges, the arcs, the dwells (`G4 P`, in seconds)
//! and the tool changes, with the length of each kind of move. The moves are timed the way the statistics time them (the length of the curve of an
//! arc, the cycles expanded into the holes they stand for, the acceleration of the machine when it is set), so the parts add up to the time the
//! report gives, plus the dwells, which the statistics leave out. Only a reading of the program: nothing is sent.

use crate::gcode::{arc_lengths, toolpath, words, Segment};
use crate::sim::{hms, seconds_accel, Rates};
use crate::stats::change_lines;
use crate::viewer::{kind_of, length_of, Kind};
use crate::words::{say, Msg};

/// Seconds and millimetres of each kind of move.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Split {
    pub rapid_s: f64,
    pub cut_s: f64,
    pub plunge_s: f64,
    pub arc_s: f64,
    pub dwell_s: f64,
    pub change_s: f64,
    pub rapid_mm: f64,
    pub cut_mm: f64,
    pub plunge_mm: f64,
    pub arc_mm: f64,
}

impl Split {
    /// Everything: the moves, the dwells and the tool changes.
    pub fn total_s(&self) -> f64 {
        self.rapid_s + self.cut_s + self.plunge_s + self.arc_s + self.dwell_s + self.change_s
    }

    /// The part of the time each kind takes, in percent rounded, for the lines of the report (0 when the program takes no time).
    fn percent(&self, part: f64) -> u32 {
        let total = self.total_s();
        if total > 0.0 {
            (part / total * 100.0).round() as u32
        } else {
            0
        }
    }

    /// The title, one line for each kind of move that takes time, and the total, in a language.
    pub fn lines(&self, lang: &str) -> Vec<String> {
        let mut out = vec![say(lang, Msg::SplitTitle, &[] as &[&str])];
        let moves = [
            (Msg::SplitRapid, self.rapid_s, self.rapid_mm),
            (Msg::SplitCut, self.cut_s, self.cut_mm),
            (Msg::SplitPlunge, self.plunge_s, self.plunge_mm),
            (Msg::SplitArc, self.arc_s, self.arc_mm),
        ];
        for (msg, s, mm) in moves {
            if s > 0.0 {
                out.push(say(
                    lang,
                    msg,
                    &[hms(s), self.percent(s).to_string(), format!("{mm:.1}")],
                ));
            }
        }
        for (msg, s) in [
            (Msg::SplitDwell, self.dwell_s),
            (Msg::SplitChange, self.change_s),
        ] {
            if s > 0.0 {
                out.push(say(lang, msg, &[hms(s), self.percent(s).to_string()]));
            }
        }
        out.push(say(lang, Msg::SplitTotal, &[hms(self.total_s())]));
        out
    }
}

/// The speed a move is made at, in mm per minute.
fn rate_of(s: &Segment, r: Rates) -> f64 {
    if s.rapid {
        r.rapid
    } else if s.feed > 0.0 {
        s.feed
    } else {
        r.default_feed
    }
}

/// The seconds of the dwells of the program: a `G4` with a `P` in seconds, as GRBL and most controllers read it.
fn dwell_seconds(lines: &[String]) -> f64 {
    lines
        .iter()
        .map(|l| {
            let w = words(l);
            if w.iter().any(|(a, v)| *a == 'G' && *v == 4.0) {
                w.iter()
                    .find(|(a, _)| *a == 'P')
                    .map_or(0.0, |(_, p)| p.clamp(0.0, 86_400.0))
            } else {
                0.0
            }
        })
        .sum()
}

/// Splits the time of the program. `change_s` is the time of a tool change in seconds and `accel_mm_s2` the acceleration of the machine (0 for none), as
/// in `stats::measure_accel`.
pub fn split(lines: &[String], rates: Rates, change_s: f64, accel_mm_s2: f64) -> Split {
    let expanded;
    let lines: &[String] = if crate::cycles::has_cycles(lines) {
        expanded = crate::cycles::expand(lines).lines;
        &expanded
    } else {
        lines
    };
    let arcs = arc_lengths(lines);
    let mut out = Split::default();
    for s in toolpath(lines) {
        let chord = length_of(&s);
        let len = if s.arc {
            arcs.binary_search_by_key(&s.line, |a| a.0)
                .map_or(chord, |i| arcs[i].1)
        } else {
            chord
        };
        let secs = seconds_accel(len, rate_of(&s, rates), accel_mm_s2);
        match kind_of(&s) {
            Kind::Rapid => {
                out.rapid_s += secs;
                out.rapid_mm += len;
            }
            Kind::Cut => {
                out.cut_s += secs;
                out.cut_mm += len;
            }
            Kind::Plunge => {
                out.plunge_s += secs;
                out.plunge_mm += len;
            }
            Kind::Arc => {
                out.arc_s += secs;
                out.arc_mm += len;
            }
        }
    }
    out.dwell_s = dwell_seconds(lines);
    if change_s.is_finite() && change_s > 0.0 {
        out.change_s = change_s * change_lines(lines).len() as f64;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::measure_accel;

    const RATES: Rates = Rates {
        rapid: 3000.0,
        default_feed: 600.0,
    };

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(str::to_string).collect()
    }

    #[test]
    fn each_kind_of_move_has_its_own_time_and_length() {
        // rapid 30 mm at 3000 mm/min = 0.6 s; plunge 5 mm at 300 = 1 s; cut 60 mm at 600 = 6 s
        let p = lines("G21 G90\nG0 X30\nG1 Z-5 F300\nG1 X90 F600");
        let s = split(&p, RATES, 0.0, 0.0);
        assert!((s.rapid_s - 0.6).abs() < 1e-9 && (s.rapid_mm - 30.0).abs() < 1e-9);
        assert!((s.plunge_s - 1.0).abs() < 1e-9 && (s.plunge_mm - 5.0).abs() < 1e-9);
        assert!((s.cut_s - 6.0).abs() < 1e-9 && (s.cut_mm - 60.0).abs() < 1e-9);
        assert_eq!((s.arc_s, s.dwell_s, s.change_s), (0.0, 0.0, 0.0));
        assert!((s.total_s() - 7.6).abs() < 1e-9);
    }

    #[test]
    fn an_arc_is_timed_along_its_curve_and_not_along_its_chord() {
        // a half circle of radius 10 from (0,0) to (20,0): the curve is 31.4159 mm, the chord 20 mm
        let p = lines("G21 G90\nG2 X20 Y0 I10 J0 F600");
        let s = split(&p, RATES, 0.0, 0.0);
        assert!(
            (s.arc_mm - std::f64::consts::PI * 10.0).abs() < 0.01,
            "{}",
            s.arc_mm
        );
        assert!((s.arc_s - s.arc_mm / 10.0).abs() < 0.001);
        assert_eq!((s.cut_s, s.plunge_s), (0.0, 0.0));
    }

    #[test]
    fn the_parts_add_up_to_the_time_of_the_statistics_plus_the_dwells() {
        let p = lines(
            "G21 G90\nT1 M6\nG0 X10 Y10\nG1 Z-2 F200\nG1 X50 F800\nG3 X10 Y10 I-20 J0\nG4 P2.5\nT2 M6\nG0 Z5\nG1 Z-1 F150\nG4 P0.5",
        );
        for accel in [0.0, 250.0] {
            let s = split(&p, RATES, 8.0, accel);
            let stats = measure_accel(&p, RATES, 8.0, accel);
            assert!(
                (s.total_s() - s.dwell_s - stats.seconds).abs() < 1e-6,
                "accel {accel}: {} {} {}",
                s.total_s(),
                s.dwell_s,
                stats.seconds
            );
            assert!((s.dwell_s - 3.0).abs() < 1e-9);
            assert!((s.change_s - 16.0).abs() < 1e-9);
            assert!(
                (s.rapid_mm + s.cut_mm + s.plunge_mm + s.arc_mm - (stats.rapid_mm + stats.cut_mm))
                    .abs()
                    < 1e-6
            );
        }
    }

    #[test]
    fn a_canned_cycle_is_timed_as_the_holes_it_stands_for_with_its_dwell() {
        let p = lines("G21 G90 G17\nG0 X10 Y10 Z5\nG82 X10 Y10 Z-3 R2 P1.5 F100\nG80");
        let s = split(&p, RATES, 0.0, 0.0);
        assert!((s.dwell_s - 1.5).abs() < 1e-9, "{}", s.dwell_s);
        assert!(s.plunge_s > 0.0);
        let stats = measure_accel(&p, RATES, 0.0, 0.0);
        assert!((s.total_s() - s.dwell_s - stats.seconds).abs() < 1e-6);
    }

    #[test]
    fn a_dwell_is_read_from_p_only_on_a_g4_and_a_silly_value_is_kept_in_bounds() {
        assert_eq!(dwell_seconds(&lines("G4 P2\nG1 X1 P9\nG4\nG4 P-3")), 2.0);
        assert_eq!(dwell_seconds(&lines("G4 P-1")), 0.0);
        assert_eq!(dwell_seconds(&lines("G4 P999999")), 86_400.0);
        assert_eq!(dwell_seconds(&lines("(G4 P5)\n; G4 P5")), 0.0);
    }

    #[test]
    fn the_report_lists_only_what_takes_time_and_the_percentages_follow() {
        let s = Split {
            rapid_s: 10.0,
            cut_s: 30.0,
            cut_mm: 5.0,
            dwell_s: 60.0,
            ..Split::default()
        };
        let text = s.lines("en");
        assert_eq!(text.len(), 5, "{text:?}");
        assert_eq!(text[0], "Time by kind of move");
        assert!(text[1].starts_with("Rapid moves: 10 s (10 %)"));
        assert!(text[2].starts_with("Cutting: 30 s (30 %), 5.0 mm"));
        assert!(text[3].starts_with("Dwells: 1 min 00 s (60 %)"));
        assert_eq!(text[4], "Total: 1 min 40 s");
        // a program that takes no time says so and divides by nothing
        let empty = Split::default().lines("es");
        assert_eq!(empty.len(), 2);
        assert!(empty[1].ends_with("0 s"));
    }

    #[test]
    fn every_line_is_told_in_the_eight_languages() {
        let s = Split {
            rapid_s: 1.0,
            cut_s: 1.0,
            plunge_s: 1.0,
            arc_s: 1.0,
            dwell_s: 1.0,
            change_s: 1.0,
            ..Split::default()
        };
        for lang in crate::words::LANGS {
            let text = s.lines(lang);
            assert_eq!(text.len(), 8, "{lang}");
            assert!(text.iter().all(|l| !l.contains('{')), "{lang}: {text:?}");
        }
    }

    #[test]
    fn a_tool_change_time_that_is_not_a_number_counts_as_none() {
        let p = lines("T1 M6\nG1 X10 F600");
        assert_eq!(split(&p, RATES, f64::NAN, 0.0).change_s, 0.0);
        assert_eq!(split(&p, RATES, -5.0, 0.0).change_s, 0.0);
        assert_eq!(split(&[], RATES, 5.0, 0.0), Split::default());
    }
}
