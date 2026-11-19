//! The offsets of the tools against the table of the tools: the `H` of a length offset (`G43 H3`) and the `D` of a cutter compensation (`G41 D3`)
//! name a tool of the table, and a program that names one the table does not have, one that is not the tool in force, or that takes a tool with a
//! length in the table and cuts without ever asking for it, will cut at the wrong depth or with the wrong cutter. The checks only read; the table
//! is the one the tools of the program are looked up in (`tools`).

use crate::gcode::{toolpath, words};
use crate::progcheck::{Finding, Level};
use crate::stats::{change_lines, tool_by_line};
use crate::tools::ToolTable;
use crate::words::Msg;

/// A tool number written in a word: a whole number from 0, or nothing.
fn tool_of(v: f64) -> Option<u32> {
    ((0.0..1e6).contains(&v) && v.fract() == 0.0).then_some(v as u32)
}

/// What looks wrong in the `H` and the `D` words of the program, by line.
pub fn check(lines: &[String], table: &ToolTable) -> Vec<Finding> {
    let in_force = tool_by_line(lines);
    let mut out = Vec::new();
    let mut push = |line: usize, level: Level, msg: Msg, args: Vec<String>| {
        out.push(Finding {
            line: Some(line),
            level,
            msg,
            args,
        });
    };
    for (n, raw) in lines.iter().enumerate() {
        let w = words(raw);
        let tool = in_force.get(n).copied().flatten();
        let has = |code: f64| w.iter().any(|(l, v)| *l == 'G' && *v == code);
        let word = |letter: char| {
            w.iter()
                .find(|(l, _)| *l == letter)
                .and_then(|(_, v)| tool_of(*v))
        };
        if has(43.0) {
            match word('H') {
                None => {
                    push(n, Level::Info, Msg::OffNoH, vec!["G43".to_string()]);
                }
                Some(h) => {
                    if table.find(h).is_none() {
                        push(
                            n,
                            Level::Warning,
                            Msg::OffNoEntry,
                            vec!["G43".to_string(), "H".to_string(), h.to_string()],
                        );
                    } else if tool.is_some_and(|t| t != h) {
                        push(
                            n,
                            Level::Warning,
                            Msg::OffWrongTool,
                            vec![
                                "G43".to_string(),
                                "H".to_string(),
                                h.to_string(),
                                tool.unwrap_or(0).to_string(),
                            ],
                        );
                    }
                }
            }
        }
        if has(41.0) || has(42.0) {
            let code = if has(41.0) { "G41" } else { "G42" };
            match word('D') {
                None => push(n, Level::Info, Msg::OffNoH, vec![code.to_string()]),
                Some(d) => {
                    if d != 0 && table.find(d).is_none() {
                        push(
                            n,
                            Level::Warning,
                            Msg::OffNoEntry,
                            vec![code.to_string(), "D".to_string(), d.to_string()],
                        );
                    } else if d != 0 && tool.is_some_and(|t| t != d) {
                        push(
                            n,
                            Level::Warning,
                            Msg::OffWrongTool,
                            vec![
                                code.to_string(),
                                "D".to_string(),
                                d.to_string(),
                                tool.unwrap_or(0).to_string(),
                            ],
                        );
                    }
                }
            }
        }
    }
    // a tool that has a length in the table, taken and cut with before the length is asked for
    let path = toolpath(lines);
    let changes = change_lines(lines);
    for (k, &c) in changes.iter().enumerate() {
        let Some(t) = in_force.get(c).copied().flatten() else {
            continue;
        };
        let length = table.find(t).map_or(0.0, |x| x.length_mm);
        if length.abs() < 1e-9 {
            continue;
        }
        let until = changes.get(k + 1).copied().unwrap_or(lines.len());
        let Some(cut) = path
            .iter()
            .find(|s| !s.rapid && s.line >= c && s.line < until)
        else {
            continue;
        };
        let asked = lines[c..=cut.line.min(lines.len() - 1)].iter().any(|l| {
            let w = words(l);
            w.iter()
                .any(|(a, v)| *a == 'G' && (*v == 43.0 || *v == 43.1))
                && w.iter()
                    .filter(|(a, _)| *a == 'H')
                    .all(|(_, v)| tool_of(*v) == Some(t))
        });
        if !asked {
            push(
                cut.line,
                Level::Warning,
                Msg::OffNotApplied,
                vec![t.to_string(), format!("{length}")],
            );
        }
    }
    out.sort_by_key(|f| f.line);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(str::to_string).collect()
    }

    fn table() -> ToolTable {
        ToolTable::parse(
            "tool = 1 | name=flat | diameter=3 | length=12.5\ntool = 2 | name=ball | diameter=6\ntool = 3 | diameter=1.5 | length=-4\n",
        )
    }

    fn msgs(f: &[Finding]) -> Vec<(Option<usize>, Msg)> {
        f.iter().map(|x| (x.line, x.msg)).collect()
    }

    #[test]
    fn a_program_that_asks_for_the_length_of_the_tool_it_took_has_nothing_to_say() {
        let p = lines("T1 M6\nG43 H1\nG1 Z-1 F100\nG49\nT3 M6\nG43 H3\nG1 X5 F100");
        assert!(check(&p, &table()).is_empty());
    }

    #[test]
    fn an_h_that_the_table_does_not_have_is_named() {
        let f = check(&lines("T2 M6\nG43 H7\nG1 Z-1 F100"), &table());
        assert_eq!(msgs(&f), [(Some(1), Msg::OffNoEntry)]);
        assert_eq!(f[0].args, ["G43", "H", "7"]);
        assert_eq!(f[0].level, Level::Warning);
    }

    #[test]
    fn an_h_or_a_d_that_is_not_the_tool_in_force_is_named() {
        let f = check(&lines("T2 M6\nG43 H1\nG41 D3 G1 X5 F100"), &table());
        assert_eq!(
            msgs(&f),
            [(Some(1), Msg::OffWrongTool), (Some(2), Msg::OffWrongTool)]
        );
        assert_eq!(f[0].args, ["G43", "H", "1", "2"]);
        assert_eq!(f[1].args, ["G41", "D", "3", "2"]);
    }

    #[test]
    fn a_compensation_with_no_d_or_with_d_zero_is_not_a_missing_tool() {
        let f = check(&lines("T2 M6\nG42 G1 X5 F100\nG41 D0 G1 X9"), &table());
        assert_eq!(msgs(&f), [(Some(1), Msg::OffNoH)]);
        assert_eq!(f[0].level, Level::Info);
        assert_eq!(f[0].args, ["G42"]);
        // a G43 with no H uses the tool in force
        let g = check(&lines("T1 M6\nG43\nG1 Z-1 F100"), &table());
        assert_eq!(msgs(&g), [(Some(1), Msg::OffNoH)]);
    }

    #[test]
    fn a_tool_with_a_length_that_cuts_without_asking_for_it_is_named_at_its_first_cut() {
        let f = check(&lines("T1 M6\nG0 X1\nG1 Z-1 F100\nG1 X5"), &table());
        assert_eq!(msgs(&f), [(Some(2), Msg::OffNotApplied)]);
        assert_eq!(f[0].args, ["1", "12.5"]);
        // the second tool is changed without a new G43: the first cut after the change is named
        let g = check(
            &lines("T1 M6\nG43 H1\nG1 Z-1 F100\nT3 M6\nG0 X9\nG1 X5 F100"),
            &table(),
        );
        assert_eq!(msgs(&g), [(Some(5), Msg::OffNotApplied)]);
        assert_eq!(g[0].args, ["3", "-4"]);
    }

    #[test]
    fn a_tool_with_no_length_in_the_table_needs_no_g43() {
        assert!(check(&lines("T2 M6\nG1 Z-1 F100"), &table()).is_empty());
        // and a tool that is not in the table is the business of the check of the tools, not of this one
        assert!(check(&lines("T9 M6\nG1 Z-1 F100"), &table()).is_empty());
    }

    #[test]
    fn g49_cancels_the_length_and_the_next_tool_must_ask_again() {
        let f = check(
            &lines("T1 M6\nG43 H1\nG1 Z-1 F100\nG49\nT3 M6\nG1 X5 F100"),
            &table(),
        );
        assert_eq!(msgs(&f), [(Some(5), Msg::OffNotApplied)]);
    }

    #[test]
    fn the_words_are_found_in_a_line_with_comments_and_in_lower_case() {
        let f = check(&lines("t2 m6\ng43 h8 (length of 8)\ng1 z-1 f100"), &table());
        assert_eq!(msgs(&f), [(Some(1), Msg::OffNoEntry)]);
        assert!(check(&lines("(G43 H8)\nT1 M6 ; G41 D9"), &table()).is_empty());
    }

    #[test]
    fn every_finding_is_told_in_the_eight_languages_with_its_holes_filled() {
        let f = check(
            &lines("T1 M6\nG43 H7\nG43 H3\nG42 G1 X5 F100\nG41 D9"),
            &table(),
        );
        let kinds: Vec<Msg> = f.iter().map(|x| x.msg).collect();
        for m in [
            Msg::OffNoEntry,
            Msg::OffWrongTool,
            Msg::OffNoH,
            Msg::OffNotApplied,
        ] {
            assert!(kinds.contains(&m), "{m:?} in {kinds:?}");
        }
        for lang in crate::words::LANGS {
            for x in &f {
                assert!(!x.text(lang).contains('{'), "{:?} {lang}", x.msg);
            }
        }
    }

    #[test]
    fn a_program_without_tools_or_with_an_empty_table_is_quiet() {
        assert!(check(&[], &table()).is_empty());
        assert!(check(&lines("G1 X5 F100"), &ToolTable::default()).is_empty());
        let f = check(&lines("T1 M6\nG43 H1\nG1 Z-1 F100"), &ToolTable::default());
        assert_eq!(msgs(&f), [(Some(1), Msg::OffNoEntry)]);
    }
}
