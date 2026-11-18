//! The statistics of a program: how long its moves are, the time it should take, how many times the tool is changed and what each tool
//! does (the distance it cuts and the time it works). The tool in force is the one chosen by `T` and taken by `M6`. Nothing here moves a
//! machine; the numbers are the estimate of the summary, with the time of each tool change added.

use crate::gcode::{arc_lengths, toolpath, words};
use crate::sim::{hms, seconds_along, Rates};
use crate::words::{say, Msg};

/// What one tool does in the program.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolStats {
    /// The tool number; nothing for the moves that come before the first tool is taken.
    pub tool: Option<u32>,
    pub moves: usize,
    pub rapid_mm: f64,
    pub cut_mm: f64,
    /// The time of its moves, without the tool changes.
    pub seconds: f64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Stats {
    /// The tools in the order they first move.
    pub tools: Vec<ToolStats>,
    /// How many times a different tool is taken (`T` then `M6`).
    pub changes: usize,
    pub rapid_mm: f64,
    pub cut_mm: f64,
    /// The time of the moves plus `changes` times the time of a change.
    pub seconds: f64,
    /// The lines that have at least one G-code word.
    pub program_lines: usize,
    /// Arc moves, whose length is that of the curve (their chord when the centre or the radius is missing).
    pub arcs: usize,
}

/// The tool in force after each line, and the lines where a different tool is taken.
fn tools_by_line(lines: &[String]) -> (Vec<Option<u32>>, Vec<usize>) {
    let (mut current, mut pending): (Option<u32>, Option<u32>) = (None, None);
    let mut changes = Vec::new();
    let mut out = Vec::with_capacity(lines.len());
    for (n, raw) in lines.iter().enumerate() {
        for (l, v) in words(raw) {
            if l == 'T' && (0.0..1e6).contains(&v) && v.fract() == 0.0 {
                pending = Some(v as u32);
            } else if l == 'M' && v == 6.0 {
                if let Some(p) = pending {
                    if current != Some(p) {
                        changes.push(n);
                    }
                    current = Some(p);
                }
            }
        }
        out.push(current);
    }
    (out, changes)
}

/// The lines (from 0) where a different tool is taken (`T` then `M6`), in order.
pub fn change_lines(lines: &[String]) -> Vec<usize> {
    tools_by_line(lines).1
}

/// Measures the program. `change_s` is the time a tool change adds, in seconds.
pub fn measure(lines: &[String], rates: Rates, change_s: f64) -> Stats {
    let (by_line, change_at) = tools_by_line(lines);
    let changes = change_at.len();
    let arc_mm = arc_lengths(lines);
    let mut st = Stats {
        changes,
        program_lines: lines.iter().filter(|l| !words(l).is_empty()).count(),
        ..Stats::default()
    };
    for s in toolpath(lines) {
        let tool = by_line.get(s.line).copied().flatten();
        let chord = ((s.to.0 - s.from.0).powi(2)
            + (s.to.1 - s.from.1).powi(2)
            + (s.to.2 - s.from.2).powi(2))
        .sqrt();
        // an arc is as long as its curve, when that can be worked out
        let len = if s.arc {
            arc_mm
                .binary_search_by_key(&s.line, |a| a.0)
                .map_or(chord, |i| arc_mm[i].1)
        } else {
            chord
        };
        let secs = seconds_along(&s, rates, len);
        let i = match st.tools.iter().position(|t| t.tool == tool) {
            Some(i) => i,
            None => {
                st.tools.push(ToolStats {
                    tool,
                    moves: 0,
                    rapid_mm: 0.0,
                    cut_mm: 0.0,
                    seconds: 0.0,
                });
                st.tools.len() - 1
            }
        };
        let t = &mut st.tools[i];
        t.moves += 1;
        t.seconds += secs;
        if s.rapid {
            t.rapid_mm += len;
            st.rapid_mm += len;
        } else {
            t.cut_mm += len;
            st.cut_mm += len;
        }
        st.seconds += secs;
        if s.arc {
            st.arcs += 1;
        }
    }
    let change = if change_s.is_finite() && change_s > 0.0 {
        change_s
    } else {
        0.0
    };
    st.seconds += change * changes as f64;
    st
}

impl Stats {
    /// The lines of the statistics window, in a language.
    pub fn lines(&self, lang: &str) -> Vec<String> {
        let mm = |v: f64| format!("{v:.1}");
        let mut out = vec![
            say(lang, Msg::StatsTitle, &[] as &[&str]),
            say(
                lang,
                Msg::StatsLength,
                &[mm(self.cut_mm), mm(self.rapid_mm)],
            ),
            say(lang, Msg::StatsChanges, &[self.changes.to_string()]),
            say(lang, Msg::StatsTime, &[hms(self.seconds)]),
        ];
        for t in &self.tools {
            let name = match t.tool {
                Some(n) => say(lang, Msg::StatsTool, &[n.to_string()]),
                None => say(lang, Msg::StatsNoTool, &[] as &[&str]),
            };
            out.push(format!(
                "{name}: {} - {}",
                say(lang, Msg::StatsLength, &[mm(t.cut_mm), mm(t.rapid_mm)]),
                hms(t.seconds)
            ));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: Rates = Rates {
        rapid: 6000.0,
        default_feed: 300.0,
    };

    fn prog(text: &str) -> Vec<String> {
        text.lines().map(str::to_string).collect()
    }

    const TWO_TOOLS: &str = "G21 G90\nT1 M6\nM3 S10000\nG0 X10\nG1 X30 F600\nM5\nT2 M6\nM3\nG0 X0\nG1 Y20 F300\nM5\nT1 M6\nG1 X5 F300\nM30";

    #[test]
    fn the_lengths_and_the_time_add_up() {
        let s = measure(&prog("G21\nG0 X60\nG1 Y30 F600\nM30"), R, 30.0);
        assert!((s.rapid_mm - 60.0).abs() < 1e-9);
        assert!((s.cut_mm - 30.0).abs() < 1e-9);
        assert!((s.seconds - 3.6).abs() < 1e-9);
        assert_eq!(s.changes, 0);
        assert_eq!(s.tools.len(), 1);
        assert_eq!(s.tools[0].tool, None);
        assert_eq!(s.program_lines, 4);
    }

    #[test]
    fn each_tool_gets_its_own_share() {
        let s = measure(&prog(TWO_TOOLS), R, 0.0);
        let tools: Vec<_> = s.tools.iter().map(|t| t.tool).collect();
        assert_eq!(tools, vec![Some(1), Some(2)]);
        // tool 1: a rapid of 10 and cuts of 20 and (after taking it again) 5; tool 2: a rapid of 30 and a cut of 20
        let t1 = &s.tools[0];
        let t2 = &s.tools[1];
        assert!((t1.rapid_mm - 10.0).abs() < 1e-9);
        assert!((t1.cut_mm - (20.0 + 5.0)).abs() < 1e-9);
        assert!((t2.rapid_mm - 30.0).abs() < 1e-9);
        assert!((t2.cut_mm - 20.0).abs() < 1e-9);
        assert_eq!(t1.moves + t2.moves, 5);
        let total: f64 = s.tools.iter().map(|t| t.seconds).sum();
        assert!((total - s.seconds).abs() < 1e-9);
    }

    #[test]
    fn changes_are_counted_and_cost_time() {
        let a = measure(&prog(TWO_TOOLS), R, 0.0);
        let b = measure(&prog(TWO_TOOLS), R, 30.0);
        assert_eq!(a.changes, 3);
        assert!((b.seconds - a.seconds - 90.0).abs() < 1e-9);
    }

    #[test]
    fn taking_the_same_tool_again_is_not_a_change() {
        let s = measure(&prog("T3 M6\nG1 X1 F100\nT3 M6\nG1 X2\nM30"), R, 10.0);
        assert_eq!(s.changes, 1);
    }

    #[test]
    fn a_tool_chosen_but_not_taken_changes_nothing() {
        let s = measure(&prog("T5\nG1 X1 F100\nM30"), R, 10.0);
        assert_eq!(s.changes, 0);
        assert_eq!(s.tools[0].tool, None);
    }

    #[test]
    fn a_bad_tool_number_is_not_taken() {
        let s = measure(&prog("T-1 M6\nT1.5 M6\nG1 X1 F100\nM30"), R, 10.0);
        assert_eq!(s.changes, 0);
    }

    #[test]
    fn a_change_time_that_is_not_a_number_counts_as_zero() {
        let a = measure(&prog("T1 M6\nG1 X1 F60\nM30"), R, f64::NAN);
        let b = measure(&prog("T1 M6\nG1 X1 F60\nM30"), R, -5.0);
        assert!((a.seconds - 1.0).abs() < 1e-9 && (b.seconds - 1.0).abs() < 1e-9);
    }

    #[test]
    fn an_arc_counts_by_its_curve_and_the_change_lines_are_known() {
        let s = measure(&prog("G3 X10 Y0 I5 J0 F600\nT1 M6\nG1 X0"), R, 0.0);
        assert_eq!(s.arcs, 1);
        let curve = 5.0 * std::f64::consts::PI;
        assert!((s.cut_mm - (curve + 10.0)).abs() < 1e-9, "{}", s.cut_mm);
        assert!((s.seconds - (curve + 10.0) / 10.0).abs() < 1e-9);
        assert_eq!(change_lines(&prog("G1 X1\nT1 M6\nT1 M6\nT2 M6")), [1, 3]);
        assert!(change_lines(&[]).is_empty());
    }

    #[test]
    fn an_empty_program_has_empty_statistics() {
        let s = measure(&[], R, 30.0);
        assert_eq!(s, Stats::default());
        assert_eq!(
            measure(&prog("(only a comment)\n\n; and another"), R, 1.0).program_lines,
            0
        );
    }

    #[test]
    fn the_window_lines_follow_the_language() {
        let s = measure(&prog(TWO_TOOLS), R, 0.0);
        let en = s.lines("en");
        assert_eq!(en[0], "Program statistics");
        assert_eq!(en[2], "3 tool changes");
        assert!(en[4].starts_with("Tool 1: Cutting 25.0 mm, rapid 10.0 mm - "));
        assert_eq!(s.lines("es")[0], "Estadísticas del programa");
        assert_eq!(s.lines("ja").len(), en.len());
    }

    #[test]
    fn moves_before_the_first_tool_are_listed_first_with_no_tool() {
        let s = measure(&prog("G1 X1 F100\nT1 M6\nG1 X2\nM30"), R, 0.0);
        assert_eq!(s.tools[0].tool, None);
        assert_eq!(s.tools[1].tool, Some(1));
        assert!(s.lines("en")[4].starts_with("No tool chosen: "));
    }
}
