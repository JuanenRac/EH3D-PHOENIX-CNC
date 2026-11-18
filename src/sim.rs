//! The program played on paper: every straight move of the path gets a start and an end time at the speeds of the settings, so a viewer can
//! put the tool at any moment of the run, draw the path done so far, jump to a line and show the time left. Nothing here moves a machine.

use crate::gcode::{Point, Segment};

/// The speeds of the estimate, in mm per minute.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rates {
    pub rapid: f64,
    /// The feed of a cut that comes before any `F`.
    pub default_feed: f64,
}

/// One move with its time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Step {
    pub seg: Segment,
    /// Seconds from the start of the program to the start and to the end of the move.
    pub t0: f64,
    pub t1: f64,
}

/// The whole program in time.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Timeline {
    pub steps: Vec<Step>,
    pub total_s: f64,
    /// Arc moves, whose time is the one of their chord and so is short.
    pub arcs: usize,
    /// The lines where the tool is changed and the seconds a change takes, which are in the times above.
    pub changes: Vec<usize>,
    pub change_s: f64,
}

fn length(s: &Segment) -> f64 {
    ((s.to.0 - s.from.0).powi(2) + (s.to.1 - s.from.1).powi(2) + (s.to.2 - s.from.2).powi(2)).sqrt()
}

/// What the estimate adds to the straight moves: the length of the curve of the arcs and the lines where the tool is changed.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Extras {
    /// The lines of the arcs with the length along the arc in mm, by line.
    pub arc_mm: Vec<(usize, f64)>,
    /// The lines where a different tool is taken (`T` then `M6`).
    pub changes: Vec<usize>,
}

impl Extras {
    /// Reads them from the lines of the program.
    pub fn of(lines: &[String]) -> Extras {
        Extras {
            arc_mm: crate::gcode::arc_lengths(lines),
            changes: crate::stats::change_lines(lines),
        }
    }

    /// The length of the arc at a line, when it is known.
    pub fn arc_len(&self, line: usize) -> Option<f64> {
        self.arc_mm
            .binary_search_by_key(&line, |a| a.0)
            .ok()
            .map(|i| self.arc_mm[i].1)
    }
}

fn rate_of(s: &Segment, r: Rates) -> f64 {
    if s.rapid {
        r.rapid
    } else if s.feed > 0.0 {
        s.feed
    } else {
        r.default_feed
    }
}

/// The seconds a move takes (0 when its speed is not a positive number).
pub fn seconds_of(s: &Segment, r: Rates) -> f64 {
    seconds_along(s, r, length(s))
}

/// The seconds a move takes when its way is `len` mm long (an arc is longer than its chord).
pub fn seconds_along(s: &Segment, r: Rates, len: f64) -> f64 {
    let rate = rate_of(s, r);
    if rate > 0.0 && rate.is_finite() {
        len / rate * 60.0
    } else {
        0.0
    }
}

impl Timeline {
    pub fn build(path: &[Segment], r: Rates) -> Timeline {
        Timeline::build_with(path, r, &Extras::default(), 0.0)
    }

    /// The program in time with the length of its arcs and `change_s` seconds for every tool change (a change comes before the move of
    /// its line; the ones after the last move are added to the total).
    pub fn build_with(path: &[Segment], r: Rates, extras: &Extras, change_s: f64) -> Timeline {
        let change = if change_s.is_finite() && change_s > 0.0 {
            change_s
        } else {
            0.0
        };
        let mut steps = Vec::with_capacity(path.len());
        let mut t = 0.0;
        let mut next_change = 0;
        for s in path {
            while next_change < extras.changes.len() && extras.changes[next_change] <= s.line {
                t += change;
                next_change += 1;
            }
            let len = if s.arc {
                extras.arc_len(s.line).unwrap_or_else(|| length(s))
            } else {
                length(s)
            };
            let d = seconds_along(s, r, len);
            steps.push(Step {
                seg: *s,
                t0: t,
                t1: t + d,
            });
            t += d;
        }
        t += change * (extras.changes.len() - next_change) as f64;
        Timeline {
            steps,
            total_s: t,
            arcs: path.iter().filter(|s| s.arc).count(),
            changes: extras.changes.clone(),
            change_s: change,
        }
    }

    /// The step that holds the moment `t` (the last one for a moment after the end).
    fn index_at(&self, t: f64) -> Option<usize> {
        if self.steps.is_empty() {
            return None;
        }
        let i = self.steps.partition_point(|s| s.t1 <= t);
        Some(i.min(self.steps.len() - 1))
    }

    /// Where the tool is at the moment `t` seconds (before the start it is at the first point, after the end at the last).
    pub fn position_at(&self, t: f64) -> Option<Point> {
        let i = self.index_at(t)?;
        let s = &self.steps[i];
        let span = s.t1 - s.t0;
        let f = if span > 0.0 {
            ((t - s.t0) / span).clamp(0.0, 1.0)
        } else if t < s.t0 {
            0.0
        } else {
            1.0
        };
        let (a, b) = (s.seg.from, s.seg.to);
        Some((
            a.0 + (b.0 - a.0) * f,
            a.1 + (b.1 - a.1) * f,
            a.2 + (b.2 - a.2) * f,
        ))
    }

    /// The line of the program being run at the moment `t`.
    pub fn line_at(&self, t: f64) -> Option<usize> {
        self.index_at(t).map(|i| self.steps[i].seg.line)
    }

    /// The moment a line starts, or the start of the first move of a later line when the line has no move of its own.
    pub fn time_of_line(&self, line: usize) -> Option<f64> {
        self.steps.iter().find(|s| s.seg.line >= line).map(|s| s.t0)
    }

    /// The moment the work of a line begins: as `time_of_line`, but a tool change on that line (or between it and its first move) is still to do.
    pub fn begin_of_line(&self, line: usize) -> f64 {
        let pending = |upto: usize| {
            self.changes
                .iter()
                .filter(|c| **c >= line && **c <= upto)
                .count() as f64
                * self.change_s
        };
        match self.steps.iter().find(|s| s.seg.line >= line) {
            Some(s) => (s.t0 - pending(s.seg.line)).max(0.0),
            None => (self.total_s - pending(usize::MAX)).max(0.0),
        }
    }

    /// The part of the path done by the moment `t`: the whole moves before it and the part of the one in progress.
    pub fn done_until(&self, t: f64) -> Vec<Segment> {
        let mut out = Vec::new();
        for s in &self.steps {
            if s.t1 <= t {
                out.push(s.seg);
            } else if s.t0 < t {
                if let Some(p) = self.position_at(t) {
                    let mut part = s.seg;
                    part.to = p;
                    out.push(part);
                }
                break;
            } else {
                break;
            }
        }
        out
    }

    /// How much is left at the moment `t`.
    pub fn remaining(&self, t: f64) -> f64 {
        (self.total_s - t.max(0.0)).max(0.0)
    }
}

/// `1 h 02 min 05 s`, `3 min 07 s` or `42 s` for a number of seconds (rounded; a negative or odd number is 0).
pub fn hms(seconds: f64) -> String {
    let total = if seconds.is_finite() && seconds > 0.0 {
        seconds.round() as u64
    } else {
        0
    };
    let (h, m, s) = (total / 3600, total % 3600 / 60, total % 60);
    if h > 0 {
        format!("{h} h {m:02} min {s:02} s")
    } else if m > 0 {
        format!("{m} min {s:02} s")
    } else {
        format!("{s} s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(from: Point, to: Point, rapid: bool, line: usize, feed: f64) -> Segment {
        Segment {
            from,
            to,
            rapid,
            line,
            feed,
            arc: false,
        }
    }

    const R: Rates = Rates {
        rapid: 6000.0,
        default_feed: 300.0,
    };

    fn demo() -> Timeline {
        // 60 mm of rapid (0.6 s), 30 mm cut at 600 mm/min (3 s), 30 mm cut with no feed at the default 300 (6 s)
        Timeline::build(
            &[
                seg((0.0, 0.0, 0.0), (60.0, 0.0, 0.0), true, 0, 0.0),
                seg((60.0, 0.0, 0.0), (60.0, 30.0, 0.0), false, 2, 600.0),
                seg((60.0, 30.0, 0.0), (30.0, 30.0, 0.0), false, 3, 0.0),
            ],
            R,
        )
    }

    #[test]
    fn each_move_gets_its_time() {
        let t = demo();
        assert!((t.steps[0].t1 - 0.6).abs() < 1e-9);
        assert!((t.steps[1].t1 - 3.6).abs() < 1e-9);
        assert!((t.total_s - 9.6).abs() < 1e-9);
        assert_eq!(t.steps[2].t0, t.steps[1].t1);
    }

    #[test]
    fn the_tool_is_found_at_any_moment() {
        let t = demo();
        assert_eq!(t.position_at(0.0), Some((0.0, 0.0, 0.0)));
        let p = t.position_at(0.3).unwrap();
        assert!((p.0 - 30.0).abs() < 1e-9 && p.1 == 0.0);
        let q = t.position_at(2.1).unwrap();
        assert!((q.0 - 60.0).abs() < 1e-9 && (q.1 - 15.0).abs() < 1e-9);
        // before the start and after the end
        assert_eq!(t.position_at(-5.0), Some((0.0, 0.0, 0.0)));
        assert_eq!(t.position_at(1e9), Some((30.0, 30.0, 0.0)));
    }

    #[test]
    fn the_line_and_the_time_of_a_line_agree() {
        let t = demo();
        assert_eq!(t.line_at(0.1), Some(0));
        assert_eq!(t.line_at(2.0), Some(2));
        assert_eq!(t.line_at(9.0), Some(3));
        assert_eq!(t.time_of_line(2), Some(t.steps[1].t0));
        // a line with no move of its own goes to the next one that has
        assert_eq!(t.time_of_line(1), Some(t.steps[1].t0));
        assert_eq!(t.time_of_line(99), None);
    }

    #[test]
    fn the_path_done_ends_in_the_middle_of_the_move_in_progress() {
        let t = demo();
        assert!(t.done_until(0.0).is_empty());
        let d = t.done_until(2.1);
        assert_eq!(d.len(), 2);
        assert!((d[1].to.1 - 15.0).abs() < 1e-9 && d[1].to.0 == 60.0);
        assert_eq!(t.done_until(1e9).len(), 3);
        assert_eq!(t.done_until(1e9)[2].to, (30.0, 30.0, 0.0));
    }

    #[test]
    fn what_is_left_never_goes_below_zero() {
        let t = demo();
        assert!((t.remaining(3.6) - 6.0).abs() < 1e-9);
        assert_eq!(t.remaining(100.0), 0.0);
        assert!((t.remaining(-4.0) - t.total_s).abs() < 1e-9);
    }

    #[test]
    fn an_empty_path_has_no_position() {
        let t = Timeline::build(&[], R);
        assert_eq!(t.position_at(1.0), None);
        assert_eq!(t.line_at(1.0), None);
        assert_eq!(t.total_s, 0.0);
    }

    #[test]
    fn a_speed_that_is_not_positive_takes_no_time() {
        let s = seg((0.0, 0.0, 0.0), (10.0, 0.0, 0.0), true, 0, 0.0);
        assert_eq!(
            seconds_of(
                &s,
                Rates {
                    rapid: 0.0,
                    default_feed: 1.0
                }
            ),
            0.0
        );
        assert_eq!(
            seconds_of(
                &s,
                Rates {
                    rapid: f64::NAN,
                    default_feed: 1.0
                }
            ),
            0.0
        );
        let c = seg((0.0, 0.0, 0.0), (10.0, 0.0, 0.0), false, 0, 0.0);
        assert!(
            (seconds_of(
                &c,
                Rates {
                    rapid: 1.0,
                    default_feed: 600.0
                }
            ) - 1.0)
                .abs()
                < 1e-9
        );
    }

    #[test]
    fn arcs_are_counted() {
        let mut a = seg((0.0, 0.0, 0.0), (10.0, 0.0, 0.0), false, 0, 100.0);
        a.arc = true;
        assert_eq!(Timeline::build(&[a], R).arcs, 1);
    }

    #[test]
    fn the_time_is_written_for_people() {
        assert_eq!(hms(0.0), "0 s");
        assert_eq!(hms(42.4), "42 s");
        assert_eq!(hms(187.0), "3 min 07 s");
        assert_eq!(hms(3725.0), "1 h 02 min 05 s");
        assert_eq!(hms(-3.0), "0 s");
        assert_eq!(hms(f64::NAN), "0 s");
        assert_eq!(hms(f64::INFINITY), "0 s");
    }

    fn program(text: &str) -> Vec<String> {
        text.lines().map(str::to_string).collect()
    }

    fn timeline_of(text: &str, change_s: f64) -> Timeline {
        let lines = program(text);
        Timeline::build_with(
            &crate::gcode::toolpath(&lines),
            R,
            &Extras::of(&lines),
            change_s,
        )
    }

    #[test]
    fn an_arc_takes_the_time_of_its_curve_and_not_of_its_chord() {
        // a half circle of radius 5 at 600 mm/min: 15.708 mm take 1.5708 s, the chord of 10 mm would take 1 s
        let t = timeline_of(
            "G21 G90
G3 X10 Y0 I5 J0 F600",
            0.0,
        );
        assert!(
            (t.total_s - 5.0 * std::f64::consts::PI / 10.0).abs() < 1e-9,
            "{}",
            t.total_s
        );
        // without the extras the chord is what counts, as before
        let lines = program("G3 X10 Y0 I5 J0 F600");
        let chord = Timeline::build(&crate::gcode::toolpath(&lines), R);
        assert!((chord.total_s - 1.0).abs() < 1e-9);
    }

    #[test]
    fn the_work_of_a_line_begins_before_the_change_of_tool_that_comes_with_it() {
        let t = timeline_of(
            "G21 G90
T1 M6
G1 X60 F600
T2 M6
G1 X0",
            30.0,
        );
        // the first change is still to do from the start and from its own line, not from the move that follows it
        assert_eq!(t.begin_of_line(0), 0.0);
        assert_eq!(t.begin_of_line(1), 0.0);
        assert!((t.begin_of_line(2) - 30.0).abs() < 1e-9);
        assert!((t.begin_of_line(3) - 36.0).abs() < 1e-9);
        assert!((t.begin_of_line(4) - 66.0).abs() < 1e-9);
        // after the last move
        assert!((t.begin_of_line(99) - t.total_s).abs() < 1e-9);
        let tail = timeline_of(
            "T1 M6
G1 X60 F600
T2 M6",
            10.0,
        );
        assert!((tail.begin_of_line(2) - 16.0).abs() < 1e-9);
        assert!((tail.remaining(tail.begin_of_line(2)) - 10.0).abs() < 1e-9);
    }

    #[test]
    fn every_tool_change_adds_its_time_before_the_move_of_its_line() {
        let text = "G21 G90
T1 M6
G1 X60 F600
T2 M6
G1 X0";
        let t = timeline_of(text, 30.0);
        // two changes of 30 s and two moves of 6 s
        assert!((t.total_s - 72.0).abs() < 1e-9, "{}", t.total_s);
        assert!((t.steps[0].t0 - 30.0).abs() < 1e-9);
        assert!((t.steps[1].t0 - 66.0).abs() < 1e-9);
        // the time left from the second move counts only that move
        let from = t.time_of_line(4).unwrap();
        assert!((t.remaining(from) - 6.0).abs() < 1e-9);
        // a change that is not there, a tool taken again and a negative time add nothing
        assert!(
            (timeline_of(
                "T1 M6
G1 X60 F600
T1 M6",
                30.0
            )
            .total_s
                - 36.0)
                .abs()
                < 1e-9
        );
        assert!((timeline_of(text, -5.0).total_s - 12.0).abs() < 1e-9);
        assert!((timeline_of(text, f64::NAN).total_s - 12.0).abs() < 1e-9);
    }

    #[test]
    fn a_change_after_the_last_move_is_added_to_the_total() {
        let t = timeline_of(
            "T1 M6
G1 X60 F600
T2 M6",
            10.0,
        );
        assert!((t.total_s - 26.0).abs() < 1e-9, "{}", t.total_s);
        // the tool stays where it ended
        assert_eq!(t.position_at(1e9), Some((60.0, 0.0, 0.0)));
    }
}
