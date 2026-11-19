//! The statistics of a program: how long its moves are, the time it should take, how many times the tool is changed and what each tool
//! does (the distance it cuts and the time it works). The tool in force is the one chosen by `T` and taken by `M6`. Nothing here moves a
//! machine; the numbers are the estimate of the summary, with the time of each tool change added.

use crate::gcode::{arc_lengths, toolpath, words};
use crate::sim::{hms, seconds_accel, seconds_along, Rates};
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

/// One level of depth of the cuts: the Z the cutting moves end at (to a hundredth of a mm), how many end there and how long they are.
#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    pub z_mm: f64,
    pub moves: usize,
    pub cut_mm: f64,
}

/// The most levels `Stats::layer_lines` lists one by one.
pub const LAYERS_LISTED: usize = 12;

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
    /// The lowest Z (mm) a cutting move ends at when it is below the zero of the work, and how many different levels of depth the
    /// cutting moves end at (to a hundredth of a mm); nothing when no cutting move goes below zero.
    pub deepest_mm: Option<f64>,
    pub depth_levels: usize,
    /// The levels of depth, from the one nearest the zero of the work down to the deepest.
    pub layers: Vec<Layer>,
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

/// The tool in force after each line of the program (nothing before the first tool is taken).
pub fn tool_by_line(lines: &[String]) -> Vec<Option<u32>> {
    tools_by_line(lines).0
}

/// The lines (from 0) where a different tool is taken (`T` then `M6`), in order.
pub fn change_lines(lines: &[String]) -> Vec<usize> {
    tools_by_line(lines).1
}

/// Measures the program. `change_s` is the time a tool change adds, in seconds.
pub fn measure(lines: &[String], rates: Rates, change_s: f64) -> Stats {
    measure_accel(lines, rates, change_s, 0.0)
}

/// As `measure`, with the time of every move worked out with the acceleration `accel_mm_s2` (mm/s2) of the machine: a move speeds up and
/// slows down and so takes longer than its length over its feed. 0 (or a number that is not one) gives the plain time of `measure`.
///
/// A program with canned cycles is measured with its cycles expanded into the moves they stand for (see `cycles`), so the length and
/// the time are those of the holes and not of the straight line from hole to hole.
pub fn measure_accel(lines: &[String], rates: Rates, change_s: f64, accel_mm_s2: f64) -> Stats {
    let program_lines = lines.iter().filter(|l| !words(l).is_empty()).count();
    let expanded;
    let lines: &[String] = if crate::cycles::has_cycles(lines) {
        expanded = crate::cycles::expand(lines).lines;
        &expanded
    } else {
        lines
    };
    let (by_line, change_at) = tools_by_line(lines);
    let changes = change_at.len();
    let arc_mm = arc_lengths(lines);
    let mut st = Stats {
        changes,
        program_lines,
        ..Stats::default()
    };
    // the levels of depth by hundredths of a mm
    let mut levels: Vec<(i64, usize, f64)> = Vec::new();
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
        let secs = seconds_accel_of(&s, rates, len, accel_mm_s2);
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
        // the depth the cutting moves reach
        if !s.rapid && s.to.2 < -0.005 {
            st.deepest_mm = Some(st.deepest_mm.map_or(s.to.2, |d| d.min(s.to.2)));
            let level = (s.to.2 * 100.0).round() as i64;
            match levels.iter_mut().find(|l| l.0 == level) {
                Some(l) => {
                    l.1 += 1;
                    l.2 += len;
                }
                None => levels.push((level, 1, len)),
            }
        }
    }
    st.depth_levels = levels.len();
    // from the surface down: the biggest Z first
    levels.sort_by_key(|l| std::cmp::Reverse(l.0));
    st.layers = levels
        .into_iter()
        .map(|(z, moves, cut_mm)| Layer {
            z_mm: z as f64 / 100.0,
            moves,
            cut_mm,
        })
        .collect();
    let change = if change_s.is_finite() && change_s > 0.0 {
        change_s
    } else {
        0.0
    };
    st.seconds += change * changes as f64;
    st
}

/// The seconds of a move of `len` mm, with the acceleration of the machine when it is given.
fn seconds_accel_of(s: &crate::gcode::Segment, rates: Rates, len: f64, accel: f64) -> f64 {
    if !(accel.is_finite() && accel > 0.0) {
        return seconds_along(s, rates, len);
    }
    let rate = if s.rapid {
        rates.rapid
    } else if s.feed > 0.0 {
        s.feed
    } else {
        rates.default_feed
    };
    seconds_accel(len, rate, accel)
}

impl Stats {
    /// The lines of the levels of depth that the cuts reach, for the report and the statistics window: the level (Z), how many cutting
    /// moves end there and how long they are, from the surface down. Nothing when no cut goes below zero; the lines after the first
    /// `LAYERS_LISTED` are counted in one more line.
    pub fn layer_lines(&self, lang: &str) -> Vec<String> {
        let mut out: Vec<String> = self
            .layers
            .iter()
            .take(LAYERS_LISTED)
            .map(|l| {
                say(
                    lang,
                    Msg::StatsLayer,
                    &[
                        crate::heightmap::num(l.z_mm),
                        l.moves.to_string(),
                        format!("{:.1}", l.cut_mm),
                    ],
                )
            })
            .collect();
        if self.layers.len() > LAYERS_LISTED {
            out.push(say(
                lang,
                Msg::StatsLayersMore,
                &[(self.layers.len() - LAYERS_LISTED).to_string()],
            ));
        }
        out
    }

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
        // after the tools, so the lines above keep their places
        if let Some(d) = self.deepest_mm {
            out.push(say(
                lang,
                Msg::StatsDepth,
                &[crate::heightmap::num(d), self.depth_levels.to_string()],
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

    #[test]
    fn the_deepest_cut_and_its_levels_are_counted_from_the_cutting_moves() {
        // a rapid down to -5 is not a cut; the cuts end at -1, -2, -2 again and -2.004 (the same level to a hundredth as -2)
        let s = measure(
            &prog("G21 G90\nG0 Z-5\nG0 Z5\nG1 Z-1 F100\nG1 X5 Z-2\nG1 X10\nG1 Z-2.004\nG0 Z5\nM30"),
            R,
            0.0,
        );
        assert_eq!(s.deepest_mm, Some(-2.004));
        assert_eq!(s.depth_levels, 2, "{s:?}");
        // the line comes after the tools and says both numbers
        let en = s.lines("en");
        assert!(
            en.last().unwrap().contains("-2.004") && en.last().unwrap().contains(", 2 "),
            "{en:?}"
        );
        assert_eq!(en.len(), 6);
        // a program that never cuts below zero says nothing about depth, and a rapid alone neither
        let flat = measure(&prog("G1 X5 F100\nG1 Z0.5\nG0 Z-3\nM30"), R, 0.0);
        assert_eq!((flat.deepest_mm, flat.depth_levels), (None, 0));
        assert_eq!(flat.lines("en").len(), 5);
        // a helix or an arc that ends below zero counts too
        let arc = measure(&prog("G3 X10 Y0 Z-1.5 I5 J0 F300\nM30"), R, 0.0);
        assert_eq!(arc.deepest_mm, Some(-1.5));
        // in every language
        for l in ["es", "de", "fr", "it", "ja", "zh", "pt"] {
            assert_ne!(s.lines(l).last(), en.last(), "{l}");
        }
    }

    const LAYERED: &str = "G21 G90\nG0 Z5\nG1 Z-1 F100\nG1 X10\nG1 X20\nG1 Z-2\nG1 X10\nG1 Z-2.004\nG0 Z5\nG1 Z-0.5\nM30";

    #[test]
    fn the_levels_of_depth_are_listed_from_the_surface_down_with_their_cuts() {
        let s = measure(&prog(LAYERED), R, 0.0);
        let zs: Vec<f64> = s.layers.iter().map(|l| l.z_mm).collect();
        // -0.5, -1, then -2 (with the -2.004 of the same hundredth)
        assert_eq!(zs, vec![-0.5, -1.0, -2.0], "{:?}", s.layers);
        assert_eq!(s.depth_levels, s.layers.len());
        let moves: Vec<usize> = s.layers.iter().map(|l| l.moves).collect();
        // -1 is the end of a plunge and two cuts along X; -2 the end of a plunge and a cut back and the -2.004 plunge
        assert_eq!(moves, vec![1, 3, 3]);
        // the levels hold every cutting move that ends below zero and their lengths add up to those moves
        let below: f64 = s.layers.iter().map(|l| l.cut_mm).sum();
        assert!(below > 0.0 && below <= s.cut_mm + 1e-9);
        // a program with no cut below zero has no levels
        assert!(measure(&prog("G1 X5 F100\nG0 Z-3\nM30"), R, 0.0)
            .layers
            .is_empty());
        assert!(measure(&[], R, 0.0).layers.is_empty());
    }

    #[test]
    fn the_lines_of_the_levels_follow_the_language_and_the_long_list_is_cut() {
        let s = measure(&prog(LAYERED), R, 0.0);
        let en = s.layer_lines("en");
        assert_eq!(en.len(), 3);
        assert!(
            en[0].starts_with("Level Z -0.5 mm: 1 cutting moves, "),
            "{en:?}"
        );
        assert!(en[2].contains("Z -2 mm"), "{en:?}");
        for l in ["es", "de", "fr", "it", "ja", "zh", "pt"] {
            let o = s.layer_lines(l);
            assert_eq!(o.len(), en.len(), "{l}");
            assert_ne!(o[0], en[0], "{l}");
        }
        // the window lines do not change: the levels are a section of their own
        assert_eq!(s.lines("en").len(), 6);
        // nothing for a program with no depth
        assert!(measure(&prog("G1 X5 F100"), R, 0.0)
            .layer_lines("en")
            .is_empty());
        // more than the limit: the first ones and a last line that counts the rest
        let mut text = String::from("G21 G90\n");
        for i in 1..=15 {
            text.push_str(&format!("G1 Z-{i} F100\n"));
        }
        let many = measure(&prog(&text), R, 0.0);
        assert_eq!(many.layers.len(), 15);
        let lines = many.layer_lines("en");
        assert_eq!(lines.len(), LAYERS_LISTED + 1);
        assert_eq!(lines.last().unwrap(), "and 3 more depth levels");
        assert!(lines[LAYERS_LISTED - 1].contains("Z -12 mm"), "{lines:?}");
        assert_eq!(
            many.layer_lines("pt").last().unwrap(),
            "e mais 3 níveis de profundidade"
        );
    }

    #[test]
    fn the_time_with_an_acceleration_is_longer_by_the_ramps_and_the_lengths_do_not_change() {
        let p = prog("G21\nG0 X60\nG1 Y30 F600\nM30");
        let plain = measure(&p, R, 0.0);
        let same = measure_accel(&p, R, 0.0, 0.0);
        assert_eq!(plain, same);
        let nan = measure_accel(&p, R, 0.0, f64::NAN);
        assert_eq!(plain, nan);
        // the rapid is 100 mm/s and the cut 10 mm/s; at 200 mm/s2 the ramps take 0.5 s and 0.05 s
        let slow = measure_accel(&p, R, 0.0, 200.0);
        assert!((plain.seconds - 3.6).abs() < 1e-9);
        assert!(
            (slow.seconds - (3.6 + 0.5 + 0.05)).abs() < 1e-9,
            "{}",
            slow.seconds
        );
        assert_eq!((slow.rapid_mm, slow.cut_mm), (plain.rapid_mm, plain.cut_mm));
        assert_eq!(slow.tools[0].moves, 2);
        // the tool changes still add their time, once each
        let with_change = measure_accel(&prog("T1 M6\nG1 X60 F600\nM30"), R, 30.0, 100.0);
        assert!(
            (with_change.seconds - (30.0 + 6.1)).abs() < 1e-9,
            "{}",
            with_change.seconds
        );
        // the time of the tools adds up to the time of the moves
        let total: f64 = slow.tools.iter().map(|t| t.seconds).sum();
        assert!((total - slow.seconds).abs() < 1e-9);
    }

    #[test]
    fn a_drilling_program_is_measured_by_its_holes_and_not_by_the_line_from_hole_to_hole() {
        let p = prog("G21 G90\nG0 Z5\nG81 X10 Y0 Z-3 R2 F100\nX20\nG80\nM30");
        let s = measure(&p, R, 0.0);
        // two holes of 5 mm from the R plane down to the depth
        assert!((s.cut_mm - 10.0).abs() < 1e-9, "{}", s.cut_mm);
        assert_eq!(s.deepest_mm, Some(-3.0));
        assert_eq!(s.depth_levels, 1);
        // the lines are those of the program, not of the expanded one
        assert_eq!(s.program_lines, 6);
        // the same program with the cycle written out by hand is measured the same
        let by_hand = prog(
            "G21 G90\nG0 Z5\nG0 X10 Y0\nG0 Z2\nG1 Z-3 F100\nG0 Z5\nG0 X20 Y0\nG0 Z2\nG1 Z-3 F100\nG0 Z5\nM30",
        );
        let h = measure(&by_hand, R, 0.0);
        assert!(
            (h.seconds - s.seconds).abs() < 1e-9,
            "{} {}",
            h.seconds,
            s.seconds
        );
        assert!((h.rapid_mm - s.rapid_mm).abs() < 1e-9);
        // a program with no cycle is measured as it was
        assert_eq!(measure(&prog("G1 X10 F100"), R, 0.0).cut_mm, 10.0);
    }
}
