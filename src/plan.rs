//! The check before a program starts on a machine that moves: the moves of the program are followed on paper (how far each axis goes, how
//! long the moves take at the speeds of the settings, whether any of them leaves the travel of the machine) and what looks wrong is said
//! (a program that is tiny, far from the corner, bigger than the table, with arcs the estimate leaves out or with cuts before any feed).
//!
//! The measure, the warnings, the clock and the refusal are those of `eh3d-summary` (the same words as in the drilling and routing programs);
//! this module only feeds them with the path of a G-code program. It knows nothing of the window, so it is tested on its own.

use eh3d_summary::dryrun::{Axis, Course, DryRun, Violation};
use eh3d_summary::summary::{bounds_of, check_work, clock, Limits, Summary, Warning};
use eh3d_view::texts::{tr, Lang, Text};

use crate::gcode::{arc_moves, toolpath};

/// What the check needs to know about the machine and the operator (all of it comes from the settings).
#[derive(Debug, Clone, PartialEq)]
pub struct Checks {
    /// The travel of X, Y and Z in mm of the machine (minimum, maximum).
    pub travel: [(f64, f64); 3],
    /// Where the zero of the work is in the machine (machine position minus work position), so the program, which speaks in work
    /// coordinates, can be put in the travel.
    pub offset: [f64; 3],
    /// The feed used for a cut that comes before any `F`, and the speed of the rapid moves, in mm per minute.
    pub default_feed: f64,
    pub rapid_mm_min: f64,
    /// The thresholds of the sanity checks and whether a program that leaves the travel is refused.
    pub limits: Limits,
}

/// What the program does, measured, and what looks wrong in it.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub rapid_moves: usize,
    pub cut_moves: usize,
    pub cut_mm: f64,
    /// Arc moves (`G2`/`G3`) that the estimate leaves out, and cuts that come before any feed rate.
    pub arcs: usize,
    pub cuts_without_feed: usize,
    pub summary: Summary,
    pub limits: Limits,
}

fn axis_of(i: usize) -> Axis {
    [Axis::X, Axis::Y, Axis::Z][i]
}

/// Follows the program and measures it.
pub fn analyse(lines: &[String], c: &Checks) -> Report {
    // an arc is only its chord in the path: it is counted apart and left out of the measure
    let path: Vec<_> = toolpath(lines).into_iter().filter(|s| !s.arc).collect();
    let (mut rapid_moves, mut cut_moves, mut cut_mm) = (0, 0, 0.0);
    let (mut seconds, mut cuts_without_feed) = (0.0, 0);
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    let mut violations: Vec<Violation> = Vec::new();
    let mut end = (0.0, 0.0, 0.0);
    for s in &path {
        let len = ((s.to.0 - s.from.0).powi(2)
            + (s.to.1 - s.from.1).powi(2)
            + (s.to.2 - s.from.2).powi(2))
        .sqrt();
        let rate = if s.rapid {
            rapid_moves += 1;
            c.rapid_mm_min
        } else {
            cut_moves += 1;
            cut_mm += len;
            if s.feed <= 0.0 {
                cuts_without_feed += 1;
                c.default_feed
            } else {
                s.feed
            }
        };
        if rate > 0.0 {
            seconds += len / rate * 60.0;
        }
        let to = [s.to.0, s.to.1, s.to.2];
        for i in 0..3 {
            min[i] = min[i].min(to[i]).min([s.from.0, s.from.1, s.from.2][i]);
            max[i] = max[i].max(to[i]).max([s.from.0, s.from.1, s.from.2][i]);
            // in the machine, with the margin of the play
            let at = to[i] + c.offset[i];
            let (lo, hi) = c.travel[i];
            let (limit, past) = if at < lo - c.limits.margin_mm {
                (lo, true)
            } else if at > hi + c.limits.margin_mm {
                (hi, true)
            } else {
                (0.0, false)
            };
            if past {
                match violations.iter_mut().find(|v| v.axis == axis_of(i)) {
                    Some(v) => {
                        if (at - limit).abs() > v.excess_mm() {
                            v.at_mm = at;
                            v.limit_mm = limit;
                        }
                    }
                    None => violations.push(Violation {
                        item: s.line,
                        axis: axis_of(i),
                        at_mm: at,
                        limit_mm: limit,
                    }),
                }
            }
        }
        end = s.to;
    }
    if path.is_empty() {
        min = [0.0; 3];
        max = [0.0; 3];
    }
    let dry = DryRun {
        min_mm: (min[0], min[1], min[2]),
        max_mm: (max[0], max[1], max[2]),
        end_mm: end,
        steps: 0,
        seconds,
        items: path.len(),
        swallowed_steps: 0,
        violations,
    };
    let span = |i: usize| c.travel[i].1 - c.travel[i].0;
    let course = Course {
        x_mm: span(0),
        y_mm: span(1),
        z_mm: span(2),
    };
    // the program from the corner of the travel, to tell a wrong offset or unit
    let corner = (c.travel[0].0, c.travel[1].0);
    let points = path
        .iter()
        .flat_map(|s| [(s.from.0, s.from.1), (s.to.0, s.to.1)])
        .map(|(x, y)| (x + c.offset[0] - corner.0, y + c.offset[1] - corner.1));
    let bounds = bounds_of(points);
    let mut warnings = check_work(bounds, path.len(), None, &c.limits);
    if let Some([x0, y0, x1, y1]) = bounds {
        let (w, h) = (x1 - x0, y1 - y0);
        if w > course.x_mm || h > course.y_mm {
            warnings.push(Warning::TooBig {
                width_mm: w,
                height_mm: h,
                table_x_mm: course.x_mm,
                table_y_mm: course.y_mm,
            });
        }
    }
    let program_bounds = bounds_of(
        path.iter()
            .flat_map(|s| [(s.from.0, s.from.1), (s.to.0, s.to.1)]),
    );
    Report {
        rapid_moves,
        cut_moves,
        cut_mm,
        arcs: arc_moves(lines),
        cuts_without_feed,
        summary: Summary::new(path.len(), 0, program_bounds, dry, course, warnings),
        limits: c.limits,
    }
}

/// Replaces the first `{}` of a text.
fn fill(template: &str, value: &str) -> String {
    template.replacen("{}", value, 1)
}

impl Report {
    /// Whether the program has nothing to move.
    pub fn is_empty(&self) -> bool {
        self.rapid_moves + self.cut_moves == 0
    }

    /// Whether every move stays inside the travel of the machine.
    pub fn within_travel(&self) -> bool {
        self.summary.within_course()
    }

    /// The reason to refuse the program, when it leaves the travel and the settings say to refuse.
    pub fn refusal(&self, lang: impl Into<Lang>) -> Option<String> {
        self.summary.refusal(&self.limits, lang)
    }

    /// What looks wrong, one line each: the warnings of the summary and the two that only a G-code program has.
    pub fn warnings(&self, lang: impl Into<Lang>) -> Vec<String> {
        let lang = lang.into();
        let mut out: Vec<String> = self
            .summary
            .warnings
            .iter()
            .map(|w| w.message(lang))
            .collect();
        if self.arcs > 0 {
            out.push(fill(tr(lang, Text::McSumArcs), &self.arcs.to_string()));
        }
        if self.cuts_without_feed > 0 {
            out.push(fill(
                tr(lang, Text::McSumNoFeed),
                &self.cuts_without_feed.to_string(),
            ));
        }
        out
    }

    /// Whether anything is worth the operator's second look before the start.
    pub fn has_warnings(&self) -> bool {
        !self.summary.warnings.is_empty() || self.arcs > 0 || self.cuts_without_feed > 0
    }

    /// The lines of the summary, ready to show: the facts, the reach of the axes, the verdict of the dry run and the warnings.
    pub fn lines(&self, lang: impl Into<Lang>) -> Vec<String> {
        let lang = lang.into();
        let mm = |v: f64| format!("{v:.1}");
        let d = &self.summary.dry;
        let mut out = vec![
            format!(
                "{}: {} / {}",
                tr(lang, Text::McSumMoves),
                self.rapid_moves,
                self.cut_moves
            ),
            format!("{}: {}", tr(lang, Text::McSumCut), mm(self.cut_mm)),
        ];
        if let Some([x0, y0, x1, y1]) = self.summary.bounds_mm {
            out.push(format!(
                "{}: {} x {}",
                tr(lang, Text::SummarySize),
                mm(x1 - x0),
                mm(y1 - y0)
            ));
        }
        out.push(format!(
            "{}: {}",
            tr(lang, Text::SummaryTime),
            clock(d.seconds)
        ));
        out.push(format!(
            "{}: X {} .. {}   Y {} .. {}   Z {} .. {}",
            tr(lang, Text::SummaryReach),
            mm(d.min_mm.0),
            mm(d.max_mm.0),
            mm(d.min_mm.1),
            mm(d.max_mm.1),
            mm(d.min_mm.2),
            mm(d.max_mm.2)
        ));
        if self.within_travel() {
            out.push(tr(lang, Text::DryOk).to_string());
        }
        out.extend(self.warnings(lang));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eh3d_view::texts::Lang;

    fn lines(s: &str) -> Vec<String> {
        s.lines().map(str::to_string).collect()
    }

    fn checks() -> Checks {
        Checks {
            travel: [(0.0, 400.0), (0.0, 300.0), (-100.0, 50.0)],
            offset: [0.0; 3],
            default_feed: 600.0,
            rapid_mm_min: 3000.0,
            limits: Limits {
                tiny_mm: 5.0,
                far_mm: 1000.0,
                margin_mm: 0.5,
                reject: true,
            },
        }
    }

    #[test]
    fn the_moves_the_distance_the_reach_and_the_time_are_measured() {
        // a rapid of 100 mm at 3000 mm/min (2 s) and a cut of 50 mm at 600 mm/min (5 s)
        let r = analyse(&lines("G0 X100\nG1 X150 F600"), &checks());
        assert_eq!((r.rapid_moves, r.cut_moves), (1, 1));
        assert!((r.cut_mm - 50.0).abs() < 1e-9);
        assert!((r.summary.dry.seconds - 7.0).abs() < 1e-9);
        assert_eq!(r.summary.dry.max_mm, (150.0, 0.0, 0.0));
        assert!(r.within_travel() && !r.has_warnings());
        assert!(r.refusal(Lang::En).is_none());
        let text = r.lines(Lang::En);
        assert!(text.iter().any(|l| l.contains("7 s")), "{text:?}");
        assert!(text.contains(&tr(Lang::En, Text::DryOk).to_string()));
    }

    #[test]
    fn a_move_past_the_travel_is_found_and_refused_only_when_the_settings_say_so() {
        let p = lines("G0 X100\nG1 X450 F600");
        let r = analyse(&p, &checks());
        assert!(!r.within_travel());
        let v = r.summary.dry.violations[0];
        assert_eq!((v.axis, v.limit_mm), (Axis::X, 400.0));
        assert!((v.excess_mm() - 50.0).abs() < 1e-9);
        let why = r.refusal(Lang::En).expect("refused");
        assert!(why.contains('X') && why.contains("50.0"), "{why}");
        let mut c = checks();
        c.limits.reject = false;
        let r = analyse(&p, &c);
        assert!(!r.within_travel() && r.refusal(Lang::En).is_none());
        // the play of the machine is not a fault
        assert!(analyse(&lines("G1 X400.4 F100"), &checks()).within_travel());
    }

    #[test]
    fn the_program_is_put_in_the_travel_through_the_zero_of_the_work() {
        // the work zero is 300 mm along X: the program goes to 150 in the work, 450 in the machine
        let mut c = checks();
        c.offset = [300.0, 0.0, 0.0];
        assert!(!analyse(&lines("G1 X150 F100"), &c).within_travel());
        assert!(analyse(&lines("G1 X50 F100"), &c).within_travel());
        // and below the minimum
        c.offset = [-10.0, 0.0, 0.0];
        assert!(!analyse(&lines("G1 X5 F100"), &c).within_travel());
    }

    #[test]
    fn what_looks_wrong_is_said() {
        let c = checks();
        // tiny: both sides under 5 mm with moves to see
        let r = analyse(&lines("G1 X1 F100\nG1 Y2"), &c);
        assert!(r
            .summary
            .warnings
            .iter()
            .any(|w| matches!(w, Warning::Tiny { .. })));
        // bigger than the table
        let r = analyse(&lines("G1 X390 F100\nG1 Y290\nG1 X-100"), &c);
        assert!(r
            .summary
            .warnings
            .iter()
            .any(|w| matches!(w, Warning::TooBig { .. })));
        // arcs and cuts before a feed
        let r = analyse(&lines("G1 X10\nG2 X20 Y10 I5 J0"), &c);
        assert_eq!((r.arcs, r.cuts_without_feed), (1, 1));
        assert!(r.has_warnings());
        let text = r.warnings(Lang::En);
        assert!(
            text.iter().any(|l| l.starts_with("1 arc moves")),
            "{text:?}"
        );
        assert!(
            text.iter().any(|l| l.starts_with("1 cutting moves")),
            "{text:?}"
        );
        // a cut with no feed is timed at the default one: 10 mm at 600 mm/min
        assert!((r.summary.dry.seconds - 1.0).abs() < 1e-9);
    }

    #[test]
    fn a_program_with_no_moves_is_empty() {
        let r = analyse(&lines("M3 S1000\n; nothing"), &checks());
        assert!(r.is_empty() && r.within_travel());
        assert_eq!(r.summary.dry.max_mm, (0.0, 0.0, 0.0));
    }

    #[test]
    fn the_summary_is_said_in_the_37_languages() {
        let r = analyse(&lines("G1 X10\nG2 X20 Y10 I5 J0\nG1 X999 F10"), &checks());
        let en = r.lines(Lang::En);
        for l in Lang::ALL {
            let text = r.lines(l);
            assert!(
                text.iter()
                    .all(|x| !x.trim().is_empty() && !x.contains("{}")),
                "{l:?}: {text:?}"
            );
        }
        for l in [Lang::Es, Lang::De, Lang::Ja, Lang::Ru, Lang::Uk] {
            assert_ne!(en, r.lines(l), "{l:?} says it in English");
        }
    }
}
