//! The wear of the tools pass by pass: a tool of the table of tools with a `life` (minutes of work) is followed through the program, and every pass it makes
//! (a run of cutting moves with no rapid move and no other tool in between) says how many minutes it cut and how much of the life of the tool is used after
//! it, counting the minutes the table says it has worked already; the pass in which the life runs out is named with the line where it does. The minutes are
//! timed as the statistics time them (the length of the curve of an arc, the canned cycles read as their holes, the acceleration of the machine when it is
//! set), and a harder material uses a tool up faster (`factor`, 1 for the wood the life of the table is given for). The numbers are an estimate by the
//! clock, not a measure of the tool. Only a reading of the program: nothing is sent.

use crate::cycles;
use crate::gcode::{arc_lengths, toolpath};
use crate::sim::{seconds_accel, Rates};
use crate::stats::tool_by_line;
use crate::tools::ToolTable;
use crate::viewer::length_of;
use crate::words::{say, Msg};

/// One pass of a tool.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pass {
    pub tool: u32,
    /// The number of the pass of this tool, from 1.
    pub number: usize,
    /// The first and the last line (from 0) of the program that cut in the pass.
    pub first_line: usize,
    pub last_line: usize,
    /// The minutes the pass cuts.
    pub minutes: f64,
    /// How much of the life of the tool is used after the pass, in percent (it may be over 100).
    pub used_percent: f64,
}

/// Where the life of a tool runs out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WornOut {
    pub tool: u32,
    /// The pass (from 1) and the line (from 0) of the move in which it does.
    pub pass: usize,
    pub line: usize,
}

/// The passes of the tools whose life the table follows, in the order the program makes them, and where the lives run out.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Wear {
    pub passes: Vec<Pass>,
    pub worn_out: Vec<WornOut>,
}

/// What a tool has done so far.
struct Run {
    tool: u32,
    life_min: f64,
    /// The life used so far, in minutes of life (the table's `used` to begin with).
    used_min: f64,
    passes: usize,
    out: bool,
}

/// The pass being made.
struct Open {
    tool: u32,
    first_line: usize,
    last_line: usize,
    seconds: f64,
}

fn rate_of(rapid: bool, feed: f64, r: Rates) -> f64 {
    if rapid {
        r.rapid
    } else if feed > 0.0 {
        feed
    } else {
        r.default_feed
    }
}

/// Follows the tools through the program. `factor` is how many times faster than the wood of the table the material uses a tool up.
pub fn passes(
    lines: &[String],
    table: &ToolTable,
    rates: Rates,
    accel_mm_s2: f64,
    factor: f64,
) -> Wear {
    let factor = if factor.is_finite() && factor > 0.0 {
        factor
    } else {
        1.0
    };
    let in_force = tool_by_line(lines);
    let expanded;
    let (path_lines, origin): (&[String], Option<&[usize]>) = if cycles::has_cycles(lines) {
        expanded = cycles::expand(lines);
        (&expanded.lines, Some(&expanded.origin))
    } else {
        (lines, None)
    };
    let arcs = arc_lengths(path_lines);
    let mut runs: Vec<Run> = Vec::new();
    let mut wear = Wear::default();
    let mut open: Option<Open> = None;
    let close = |open: &mut Option<Open>, runs: &mut Vec<Run>, wear: &mut Wear| {
        let Some(o) = open.take() else { return };
        let Some(run) = runs.iter_mut().find(|r| r.tool == o.tool) else {
            return;
        };
        let minutes = o.seconds / 60.0;
        run.used_min += minutes * factor;
        run.passes += 1;
        wear.passes.push(Pass {
            tool: o.tool,
            number: run.passes,
            first_line: o.first_line,
            last_line: o.last_line,
            minutes,
            used_percent: run.used_min / run.life_min * 100.0,
        });
    };
    for s in toolpath(path_lines) {
        let line = origin.map_or(s.line, |o| o.get(s.line).copied().unwrap_or(s.line));
        let tool = in_force
            .get(line)
            .copied()
            .flatten()
            .filter(|&t| table.find(t).is_some_and(|e| e.life_min > 0.0));
        let len = if s.arc {
            arcs.binary_search_by_key(&s.line, |a| a.0)
                .map_or_else(|_| length_of(&s), |i| arcs[i].1)
        } else {
            length_of(&s)
        };
        let secs = seconds_accel(len, rate_of(s.rapid, s.feed, rates), accel_mm_s2);
        // a rapid, a move of another tool or a move with no length ends the pass that is open
        let cutting = !s.rapid && secs > 0.0;
        let Some(tool) = tool.filter(|_| cutting) else {
            if s.rapid || open.as_ref().is_some_and(|o| Some(o.tool) != tool) {
                close(&mut open, &mut runs, &mut wear);
            }
            continue;
        };
        if open.as_ref().is_some_and(|o| o.tool != tool) {
            close(&mut open, &mut runs, &mut wear);
        }
        if !runs.iter().any(|r| r.tool == tool) {
            let e = table.find(tool).expect("the tool was looked up above");
            runs.push(Run {
                tool,
                life_min: e.life_min,
                used_min: e.used_min,
                passes: 0,
                out: false,
            });
        }
        let o = open.get_or_insert(Open {
            tool,
            first_line: line,
            last_line: line,
            seconds: 0.0,
        });
        o.last_line = line;
        o.seconds += secs;
        // the life runs out in this move, or was out before the program began
        let run = runs
            .iter_mut()
            .find(|r| r.tool == tool)
            .expect("the run was made above");
        let would_be = run.used_min + o.seconds / 60.0 * factor;
        if !run.out && would_be >= run.life_min {
            run.out = true;
            wear.worn_out.push(WornOut {
                tool,
                pass: run.passes + 1,
                line,
            });
        }
    }
    close(&mut open, &mut runs, &mut wear);
    wear
}

/// The report in a language: the title, a line for each pass and a line for each tool whose life runs out. One line that says so when no tool of the program
/// has a life in the table.
pub fn lines(wear: &Wear, lang: &str) -> Vec<String> {
    if wear.passes.is_empty() {
        return vec![say::<&str>(lang, Msg::LifeNone, &[])];
    }
    let mut out = vec![say::<&str>(lang, Msg::LifeTitle, &[])];
    for p in &wear.passes {
        out.push(format!(
            "  {}",
            say(
                lang,
                Msg::LifePass,
                &[
                    p.tool.to_string(),
                    p.number.to_string(),
                    (p.first_line + 1).to_string(),
                    (p.last_line + 1).to_string(),
                    format!("{:.2}", p.minutes),
                    format!("{:.0}", p.used_percent),
                ],
            )
        ));
    }
    for w in &wear.worn_out {
        out.push(format!(
            "  {}",
            say(
                lang,
                Msg::LifeOut,
                &[
                    w.tool.to_string(),
                    w.pass.to_string(),
                    (w.line + 1).to_string()
                ],
            )
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timesplit;

    fn prog(s: &str) -> Vec<String> {
        s.lines().map(str::to_string).collect()
    }

    fn rates() -> Rates {
        Rates {
            rapid: 3000.0,
            default_feed: 500.0,
        }
    }

    fn table(extra: &str) -> ToolTable {
        ToolTable::parse(&format!(
            "tool = 1 | diameter=3{extra}\ntool = 2 | diameter=6\n"
        ))
    }

    /// Two passes of 6 mm down and 60 mm along at 600 mm/min: 6.6 s, 0.11 min each.
    const TWO: &str =
        "T1 M6\nG21 G90\nG0 Z5\nG1 Z-1 F600\nG1 X60\nG0 Z5\nG0 X0\nG1 Z-1 F600\nG1 X60\nM2";

    #[test]
    fn every_pass_says_its_minutes_and_the_share_of_the_life_used_after_it() {
        let w = passes(&prog(TWO), &table(" | life=1"), rates(), 0.0, 1.0);
        assert_eq!(w.passes.len(), 2, "{w:?}");
        let (a, b) = (w.passes[0], w.passes[1]);
        assert_eq!((a.tool, a.number, a.first_line, a.last_line), (1, 1, 3, 4));
        assert_eq!((b.number, b.first_line, b.last_line), (2, 7, 8));
        assert!((a.minutes - 0.11).abs() < 1e-9 && (b.minutes - 0.11).abs() < 1e-9);
        assert!((a.used_percent - 11.0).abs() < 1e-6 && (b.used_percent - 22.0).abs() < 1e-6);
        assert!(w.worn_out.is_empty());
        let said = lines(&w, "en");
        assert_eq!(said[0], "Wear of the tools by pass");
        assert_eq!(
            said[1],
            "  Tool 1, pass 1 (lines 4 to 5): 0.11 min of cutting, 11 % of the life used after it"
        );
    }

    #[test]
    fn the_minutes_already_used_count_and_the_pass_where_the_life_runs_out_is_named() {
        // 0.15 min of life: the second pass goes over it in its long move (line 9)
        let w = passes(&prog(TWO), &table(" | life=0.15"), rates(), 0.0, 1.0);
        assert_eq!(
            w.worn_out,
            vec![WornOut {
                tool: 1,
                pass: 2,
                line: 8
            }]
        );
        assert!(w.passes[1].used_percent > 100.0);
        let said = lines(&w, "en");
        assert_eq!(
            said.last().unwrap(),
            "  Tool 1 is worn out in pass 2, at line 9"
        );
        // a tool used up before the program begins runs out in its first move
        let used = passes(&prog(TWO), &table(" | life=1 | used=1"), rates(), 0.0, 1.0);
        assert_eq!(
            used.worn_out,
            vec![WornOut {
                tool: 1,
                pass: 1,
                line: 3
            }]
        );
        // and some of the life used before moves every percentage up
        let some = passes(
            &prog(TWO),
            &table(" | life=1 | used=0.5"),
            rates(),
            0.0,
            1.0,
        );
        assert!((some.passes[0].used_percent - 61.0).abs() < 1e-6);
    }

    #[test]
    fn a_harder_material_uses_the_life_up_faster() {
        let w = passes(&prog(TWO), &table(" | life=1"), rates(), 0.0, 3.0);
        assert!((w.passes[0].used_percent - 33.0).abs() < 1e-6);
        // the cutting time does not change
        assert!((w.passes[0].minutes - 0.11).abs() < 1e-9);
        // a factor that is not a positive number is the wood of the table
        for bad in [0.0, -2.0, f64::NAN, f64::INFINITY] {
            let b = passes(&prog(TWO), &table(" | life=1"), rates(), 0.0, bad);
            assert!((b.passes[0].used_percent - 11.0).abs() < 1e-6, "{bad}");
        }
    }

    #[test]
    fn a_tool_with_no_life_in_the_table_and_a_program_with_no_cut_have_nothing_to_follow() {
        let none = passes(&prog(TWO), &table(""), rates(), 0.0, 1.0);
        assert!(none.passes.is_empty());
        assert_eq!(
            lines(&none, "en"),
            vec!["No tool of the program has a life in the table".to_string()]
        );
        let rapids = passes(
            &prog("T1 M6\nG21 G90\nG0 X50\nM2"),
            &table(" | life=1"),
            rates(),
            0.0,
            1.0,
        );
        assert!(rapids.passes.is_empty());
        // a tool the table does not have
        let other = passes(
            &prog("T7 M6\nG21 G90\nG1 X50 F600\nM2"),
            &table(" | life=1"),
            rates(),
            0.0,
            1.0,
        );
        assert!(other.passes.is_empty());
    }

    #[test]
    fn a_tool_change_ends_the_pass_and_each_tool_counts_its_own() {
        let p = "T1 M6\nG21 G90\nG1 X10 F600\nT2 M6\nG1 X20 F600\nT1 M6\nG1 X30 F600\nM2";
        let t =
            ToolTable::parse("tool = 1 | diameter=3 | life=10\ntool = 2 | diameter=6 | life=10\n");
        let w = passes(&prog(p), &t, rates(), 0.0, 1.0);
        let got: Vec<(u32, usize)> = w.passes.iter().map(|x| (x.tool, x.number)).collect();
        assert_eq!(got, vec![(1, 1), (2, 1), (1, 2)]);
        // the life used by tool 1 adds up over its two passes
        assert!(w.passes[2].used_percent > w.passes[0].used_percent);
    }

    #[test]
    fn the_holes_of_a_canned_cycle_are_passes_and_their_lines_are_the_ones_of_the_cycle() {
        let p = "T1 M6\nG21 G90\nG0 Z5\nG81 X10 Y10 Z-5 R2 F100\nX20\nG80\nM2";
        let w = passes(&prog(p), &table(" | life=1"), rates(), 0.0, 1.0);
        assert_eq!(w.passes.len(), 2, "{w:?}");
        assert!(w
            .passes
            .iter()
            .all(|x| x.first_line >= 3 && x.last_line <= 4));
    }

    #[test]
    fn the_acceleration_makes_the_passes_longer() {
        let plain = passes(&prog(TWO), &table(" | life=1"), rates(), 0.0, 1.0);
        let slow = passes(&prog(TWO), &table(" | life=1"), rates(), 50.0, 1.0);
        assert!(slow.passes[0].minutes > plain.passes[0].minutes);
    }

    #[test]
    fn every_line_is_told_in_the_eight_languages_with_its_holes_filled() {
        let w = passes(&prog(TWO), &table(" | life=0.15"), rates(), 0.0, 1.0);
        for lang in crate::words::LANGS {
            for l in lines(&w, lang) {
                assert!(!l.contains('{') && !l.trim().is_empty(), "{lang}: {l}");
            }
        }
    }

    #[test]
    fn programs_from_a_fixed_seed_add_up_to_the_cutting_time_of_the_split_of_the_time() {
        let mut seed = 21u64;
        let mut next = move || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((seed >> 33) % 1000) as f64 / 1000.0
        };
        let mut checked = 0;
        for _ in 0..300 {
            let mut p = vec![
                "T1 M6".to_string(),
                "G21 G90".to_string(),
                "G0 Z5".to_string(),
            ];
            for _ in 0..(3 + (next() * 20.0) as usize) {
                let (x, y) = ((next() * 80.0).round(), (next() * 80.0).round());
                match (next() * 4.0) as u8 {
                    0 => p.push(format!("G0 X{x} Y{y}")),
                    1 => p.push(format!(
                        "G1 Z-{} F{}",
                        (next() * 4.0).round(),
                        100 + (next() * 900.0) as u32
                    )),
                    2 => p.push(format!(
                        "G2 X{x} Y{y} R{} F300",
                        50 + (next() * 50.0) as u32
                    )),
                    _ => p.push(format!("G1 X{x} Y{y} F{}", 100 + (next() * 900.0) as u32)),
                }
            }
            let accel = if next() < 0.5 {
                0.0
            } else {
                100.0 + next() * 400.0
            };
            let w = passes(&p, &table(" | life=100000"), rates(), accel, 1.0);
            let split = timesplit::split(&p, rates(), 0.0, accel);
            let expected = (split.cut_s + split.plunge_s + split.arc_s) / 60.0;
            let got: f64 = w.passes.iter().map(|x| x.minutes).sum();
            assert!((got - expected).abs() < 1e-6, "{got} {expected} {p:?}");
            // the share only grows
            for pair in w.passes.windows(2) {
                assert!(pair[1].used_percent >= pair[0].used_percent);
            }
            checked += usize::from(!w.passes.is_empty());
        }
        assert!(checked > 200, "{checked}");
    }
}
