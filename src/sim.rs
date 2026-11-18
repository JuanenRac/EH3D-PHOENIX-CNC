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
}

fn length(s: &Segment) -> f64 {
    ((s.to.0 - s.from.0).powi(2) + (s.to.1 - s.from.1).powi(2) + (s.to.2 - s.from.2).powi(2)).sqrt()
}

/// The seconds a move takes (0 when its speed is not a positive number).
pub fn seconds_of(s: &Segment, r: Rates) -> f64 {
    let rate = if s.rapid {
        r.rapid
    } else if s.feed > 0.0 {
        s.feed
    } else {
        r.default_feed
    };
    if rate > 0.0 && rate.is_finite() {
        length(s) / rate * 60.0
    } else {
        0.0
    }
}

impl Timeline {
    pub fn build(path: &[Segment], r: Rates) -> Timeline {
        let mut steps = Vec::with_capacity(path.len());
        let mut t = 0.0;
        for s in path {
            let d = seconds_of(s, r);
            steps.push(Step {
                seg: *s,
                t0: t,
                t1: t + d,
            });
            t += d;
        }
        Timeline {
            steps,
            total_s: t,
            arcs: path.iter().filter(|s| s.arc).count(),
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
}
