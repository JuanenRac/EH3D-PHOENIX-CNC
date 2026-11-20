//! The holder of a tool against the material: a tool of the table of tools with a `reach` (how far it stands out below its holder) cannot go into the
//! material deeper than that, because the holder would touch it. The depth is taken from Z0 as the surface of the material (the usual zero of a job); the
//! deepest point of every move counts, rapids included, and a canned cycle is read as the holes it makes. A tool the table gives no reach is not checked, and
//! one that goes into the material is said to have none. It only reads the program: nothing is sent.

use crate::cycles;
use crate::gcode::toolpath;
use crate::progcheck::{Finding, Level};
use crate::stats::tool_by_line;
use crate::tools::ToolTable;
use crate::words::Msg;

/// A depth smaller than this many mm below the surface is the surface (a rounding of the numbers of the program).
const SURFACE_MM: f64 = 1e-6;

/// What one tool does in the material.
struct Dive {
    tool: u32,
    /// The first line (from 0) that goes deeper than the reach, and the deepest depth.
    first_over: Option<usize>,
    deepest: f64,
}

/// The tools of the program that go into the material too deep for their holder, or that go in with no reach in the table, ordered by line (the remarks
/// about a tool with no reach, which have no line, last).
pub fn check(lines: &[String], table: &ToolTable) -> Vec<Finding> {
    let in_force = tool_by_line(lines);
    let expanded;
    let (path_lines, origin): (&[String], Option<&[usize]>) = if cycles::has_cycles(lines) {
        expanded = cycles::expand(lines);
        (&expanded.lines, Some(&expanded.origin))
    } else {
        (lines, None)
    };
    let mut dives: Vec<Dive> = Vec::new();
    for s in toolpath(path_lines) {
        let line = origin.map_or(s.line, |o| o.get(s.line).copied().unwrap_or(s.line));
        let Some(tool) = in_force.get(line).copied().flatten() else {
            continue;
        };
        let depth = -s.from.2.min(s.to.2);
        if !(depth.is_finite() && depth > SURFACE_MM) {
            continue;
        }
        let reach = table.find(tool).map_or(0.0, |t| t.reach_mm);
        let over = reach > 0.0 && depth > reach + SURFACE_MM;
        match dives.iter_mut().find(|d| d.tool == tool) {
            Some(d) => {
                d.deepest = d.deepest.max(depth);
                if over && d.first_over.is_none() {
                    d.first_over = Some(line);
                }
            }
            None => dives.push(Dive {
                tool,
                first_over: over.then_some(line),
                deepest: depth,
            }),
        }
    }
    let mut out = Vec::new();
    for d in &dives {
        let Some(entry) = table.find(d.tool) else {
            continue;
        };
        if entry.reach_mm <= 0.0 {
            out.push(Finding {
                line: None,
                level: Level::Info,
                msg: Msg::HolderNoReach,
                args: vec![d.tool.to_string()],
            });
        } else if let Some(line) = d.first_over {
            out.push(Finding {
                line: Some(line),
                level: Level::Warning,
                msg: Msg::HolderTooDeep,
                args: vec![
                    d.tool.to_string(),
                    format!("{:.2}", d.deepest),
                    format!("{:.2}", entry.reach_mm),
                ],
            });
        }
    }
    out.sort_by_key(|f| f.line.map_or((1, 0), |l| (0, l)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(s: &str) -> Vec<String> {
        s.lines().map(str::to_string).collect()
    }

    fn table(reach: &str) -> ToolTable {
        ToolTable::parse(&format!(
            "tool = 1 | diameter=3{reach}\ntool = 2 | diameter=6 | reach=20\n"
        ))
    }

    const DEEP: &str = "T1 M6\nG21 G90\nG0 Z5\nG1 Z-6 F100\nG1 X10\nG0 Z5\nM2";

    #[test]
    fn a_cut_deeper_than_the_reach_is_named_with_its_line_and_the_deepest_depth() {
        let f = check(&lines(DEEP), &table(" | reach=5"));
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(
            (f[0].line, f[0].level, f[0].msg),
            (Some(3), Level::Warning, Msg::HolderTooDeep)
        );
        assert_eq!(
            f[0].text("en"),
            "Line 4: tool 1 cuts 6.00 mm below the surface but only 5.00 mm of it is free of the holder: the holder would touch the material"
        );
    }

    #[test]
    fn the_depth_is_the_deepest_of_the_whole_program_and_the_line_is_the_first_that_is_too_deep() {
        let p = "T1 M6\nG21 G90\nG1 Z-4 F100\nG1 Z-6\nG1 Z-9\nG1 Z-2\nM2";
        let f = check(&lines(p), &table(" | reach=5"));
        assert_eq!(f[0].line, Some(3));
        assert_eq!(f[0].args[1], "9.00");
    }

    #[test]
    fn a_cut_at_the_reach_or_less_is_quiet() {
        assert!(check(&lines(DEEP), &table(" | reach=6")).is_empty());
        assert!(check(&lines(DEEP), &table(" | reach=6.5")).is_empty());
        // above the surface there is nothing to touch
        let up = "T1 M6\nG21 G90\nG1 Z3 F100\nG1 X10\nM2";
        assert!(check(&lines(up), &table(" | reach=1")).is_empty());
    }

    #[test]
    fn a_tool_with_no_reach_that_goes_into_the_material_is_said_to_have_none() {
        let f = check(&lines(DEEP), &table(""));
        assert_eq!(f.len(), 1);
        assert_eq!(
            (f[0].line, f[0].level, f[0].msg),
            (None, Level::Info, Msg::HolderNoReach)
        );
        assert!(f[0].text("en").contains("Tool 1 has no 'reach'"));
        // a tool that never goes in is not worth saying anything about
        assert!(check(&lines("T1 M6\nG21 G90\nG1 Z2 F100\nM2"), &table("")).is_empty());
    }

    #[test]
    fn a_rapid_into_the_material_counts_as_well() {
        let p = "T1 M6\nG21 G90\nG0 Z-8\nM2";
        let f = check(&lines(p), &table(" | reach=5"));
        assert_eq!(f[0].line, Some(2));
    }

    #[test]
    fn a_canned_cycle_is_read_as_its_holes_and_the_line_is_the_one_of_the_cycle() {
        let p = "T1 M6\nG21 G90\nG0 Z5\nG81 X10 Y10 Z-12 R2 F100\nG80\nM2";
        let f = check(&lines(p), &table(" | reach=8"));
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(f[0].line, Some(3));
        assert_eq!(f[0].args[1], "12.00");
    }

    #[test]
    fn each_tool_is_judged_by_its_own_reach_and_a_tool_the_table_lacks_is_left_alone() {
        let p =
            "T1 M6\nG21 G90\nG1 Z-6 F100\nG0 Z5\nT2 M6\nG1 Z-6 F100\nG0 Z5\nT9 M6\nG1 Z-6 F100\nM2";
        let f = check(&lines(p), &table(" | reach=5"));
        // tool 1 reaches 5, tool 2 reaches 20 (fine), tool 9 is not in the table
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].args[0], "1");
        // no tool taken: no tool to judge
        assert!(check(&lines("G21 G90\nG1 Z-30 F100\nM2"), &table(" | reach=5")).is_empty());
    }

    #[test]
    fn inches_are_read_in_millimetres() {
        let p = "T1 M6\nG20 G90\nG1 Z-0.3 F10\nM2";
        // 0.3 in = 7.62 mm
        let f = check(&lines(p), &table(" | reach=7"));
        assert_eq!(f[0].args[1], "7.62");
        assert!(check(&lines(p), &table(" | reach=8")).is_empty());
    }

    #[test]
    fn every_remark_is_told_in_the_eight_languages_with_its_holes_filled() {
        let mut found = check(&lines(DEEP), &table(" | reach=5"));
        found.extend(check(&lines(DEEP), &table("")));
        for lang in crate::words::LANGS {
            for f in &found {
                let t = f.text(lang);
                assert!(!t.contains('{') && !t.is_empty(), "{:?} {lang}: {t}", f.msg);
            }
        }
    }

    #[test]
    fn programs_from_a_fixed_seed_are_judged_exactly_by_their_deepest_cut() {
        let mut seed = 7u64;
        let mut next = move || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((seed >> 33) % 1000) as f64 / 1000.0
        };
        let (mut over, mut fine) = (0, 0);
        for _ in 0..400 {
            let reach = 2.0 + (next() * 10.0).floor();
            let mut p = vec!["T1 M6".to_string(), "G21 G90".to_string()];
            let mut first: Option<usize> = None;
            let mut deepest = 0.0f64;
            for i in 0..(2 + (next() * 12.0) as usize) {
                let z = (next() * 20.0 - 14.0).round();
                p.push(format!("G1 Z{z} X{} F100", i));
                if -z > reach && first.is_none() {
                    first = Some(p.len() - 1);
                }
                deepest = deepest.max(-z);
            }
            let f = check(&p, &table(&format!(" | reach={reach}")));
            match first {
                Some(line) => {
                    over += 1;
                    assert_eq!(f.len(), 1, "{p:?}");
                    assert_eq!(f[0].line, Some(line), "{p:?}");
                    assert_eq!(f[0].args[1], format!("{deepest:.2}"), "{p:?}");
                }
                None => {
                    fine += 1;
                    assert!(f.is_empty(), "{p:?} {f:?}");
                }
            }
        }
        assert!(over > 50 && fine > 50, "{over} {fine}");
    }
}
