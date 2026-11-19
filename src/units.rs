//! The conversion of a program between millimetres and inches. Every length of the program (`X Y Z U V W`, the centre of an arc `I J K`, `R`, the
//! depth of a peck `Q`) and the feed `F` are multiplied by 25.4 or divided by it, line by line, from the unit that is in force on that line
//! (`G20` inches, `G21` millimetres) to the unit asked for, and the `G20`/`G21` words are written as the unit asked for. What is not a length is
//! left as it is: the angles of `A B C`, the speed `S`, the tool `T`, the dwell `P`, the numbers of `N`, `H`, `D`, `L` and the codes `G` and `M`,
//! the comments and the text. A value given by a parameter or an expression cannot be converted and is said, and so is a feed in inverse time.
//! Nothing here sends anything: it reads the text of the program and gives the text of another.

use crate::gcode::words;
use crate::progcheck::{Finding, Level};
use crate::words::Msg;

/// Millimetres in an inch.
pub const MM_PER_INCH: f64 = 25.4;

/// A program converted: its lines, what the conversion has to say, and how much it did.
#[derive(Debug, Clone, PartialEq)]
pub struct Converted {
    pub lines: Vec<String>,
    pub findings: Vec<Finding>,
    /// How many numbers were converted and in how many lines.
    pub numbers: usize,
    pub changed_lines: usize,
}

/// The letters whose number is a length.
fn is_length(letter: char) -> bool {
    matches!(
        letter.to_ascii_uppercase(),
        'X' | 'Y' | 'Z' | 'U' | 'V' | 'W' | 'I' | 'J' | 'K' | 'R' | 'Q'
    )
}

/// A number written for the unit asked for: five decimals in inches and four in millimetres, without the zeros at the end.
fn write_number(v: f64, to_inches: bool) -> String {
    let decimals = if to_inches { 5 } else { 4 };
    let text = format!("{v:.decimals$}");
    let text = if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        text
    };
    if text == "-0" {
        "0".to_string()
    } else {
        text
    }
}

fn is_number_char(c: char) -> bool {
    c.is_ascii_digit() || matches!(c, '.' | '+' | '-')
}

/// What one line asks of the conversion: the unit it is written in, and whether its feed is in inverse time.
struct LineState {
    inches: bool,
    inverse_feed: bool,
}

/// Rewrites one line: the lengths and the feed of `state` into the unit asked for, and the `G20`/`G21` words as that unit. Gives the new text, the numbers
/// converted and the words whose value is a parameter or an expression (left as they were).
fn rewrite(line: &str, state: &LineState, to_inches: bool) -> (String, usize, Vec<String>) {
    let factor = if state.inches == to_inches {
        None
    } else if to_inches {
        Some(1.0 / MM_PER_INCH)
    } else {
        Some(MM_PER_INCH)
    };
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::with_capacity(line.len() + 4);
    let (mut converted, mut skipped) = (0usize, Vec::new());
    let mut i = 0;
    let mut paren = false;
    while i < chars.len() {
        let c = chars[i];
        if paren {
            out.push(c);
            paren = c != ')';
            i += 1;
            continue;
        }
        if c == '(' {
            paren = true;
            out.push(c);
            i += 1;
            continue;
        }
        if c == ';' {
            out.extend(&chars[i..]);
            break;
        }
        if !c.is_ascii_alphabetic() {
            out.push(c);
            i += 1;
            continue;
        }
        // a word: the letter, spaces, and a number
        let mut j = i + 1;
        while j < chars.len() && (chars[j] == ' ' || chars[j] == '\t') {
            j += 1;
        }
        let start = j;
        while j < chars.len() && is_number_char(chars[j]) {
            j += 1;
        }
        let number: String = chars[start..j].iter().collect();
        let value = number.parse::<f64>().ok();
        let letter = c.to_ascii_uppercase();
        let wants = is_length(c) || (letter == 'F' && !state.inverse_feed);
        match value {
            Some(v) if letter == 'G' && (v == 20.0 || v == 21.0) => {
                out.push(c);
                out.push_str(if to_inches { "20" } else { "21" });
                i = j;
            }
            Some(v) if wants && factor.is_some() => {
                out.push(c);
                out.extend(&chars[i + 1..start]);
                out.push_str(&write_number(v * factor.unwrap_or(1.0), to_inches));
                converted += 1;
                i = j;
            }
            None if wants && start < chars.len() && matches!(chars[start], '#' | '[') => {
                skipped.push(letter.to_string());
                out.push(c);
                i += 1;
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    (out, converted, skipped)
}

/// Converts the program to inches (`to_inches`) or to millimetres. A program that never says its unit is read as millimetres (the usual default of a
/// controller) and says so; one that is converted to inches and has numbers before it says its unit gets a `G20` as its first line, because the
/// controller would read those new numbers as millimetres.
pub fn convert(lines: &[String], to_inches: bool) -> Converted {
    let mut findings = Vec::new();
    let mut out = Vec::with_capacity(lines.len() + 1);
    let (mut inches, mut declared, mut assumed_said) = (false, false, false);
    // the numbers converted before the program says its unit: the controller reads them in its own default, so the new unit has to be said before them
    let mut before_unit = 0usize;
    let mut inverse = false;
    let (mut numbers, mut changed_lines) = (0usize, 0usize);
    for (n, raw) in lines.iter().enumerate() {
        let mut line_inches = inches;
        let mut line_inverse = inverse;
        for (l, v) in words(raw) {
            if l != 'G' {
                continue;
            }
            if v == 20.0 || v == 21.0 {
                let now = v == 20.0;
                if declared && now != inches {
                    findings.push(Finding {
                        line: Some(n),
                        level: Level::Warning,
                        msg: Msg::UnitsSwitched,
                        args: vec![format!("G{}", v as i32)],
                    });
                }
                declared = true;
                line_inches = now;
                inches = now;
            } else if v == 93.0 {
                line_inverse = true;
                inverse = true;
            } else if v == 94.0 || v == 95.0 {
                line_inverse = false;
                inverse = false;
            }
        }
        let state = LineState {
            inches: line_inches,
            inverse_feed: line_inverse,
        };
        let (text, count, skipped) = rewrite(raw, &state, to_inches);
        let has_length = words(raw).iter().any(|(l, _)| is_length(*l) || *l == 'F');
        if has_length && !declared && !assumed_said {
            assumed_said = true;
            findings.push(Finding {
                line: None,
                level: Level::Info,
                msg: Msg::UnitsAssumed,
                args: vec![],
            });
        }
        if line_inverse && words(raw).iter().any(|(l, _)| *l == 'F') {
            findings.push(Finding {
                line: Some(n),
                level: Level::Warning,
                msg: Msg::UnitsInverseFeed,
                args: vec![],
            });
        }
        for word in skipped {
            findings.push(Finding {
                line: Some(n),
                level: Level::Warning,
                msg: Msg::UnitsExpression,
                args: vec![word],
            });
        }
        if text != *raw {
            changed_lines += 1;
        }
        numbers += count;
        if !declared {
            before_unit += count;
        }
        out.push(text);
    }
    if to_inches && before_unit > 0 {
        out.insert(0, "G20".to_string());
    }
    Converted {
        lines: out,
        findings,
        numbers,
        changed_lines,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(str::to_string).collect()
    }

    #[test]
    fn millimetres_become_inches_and_the_unit_word_follows() {
        let c = convert(&lines("G21\nG1 X25.4 Y50.8 F254"), true);
        assert_eq!(c.lines, ["G20", "G1 X1 Y2 F10"]);
        assert_eq!((c.numbers, c.changed_lines), (3, 2));
        assert!(c.findings.is_empty());
    }

    #[test]
    fn inches_become_millimetres() {
        let c = convert(&lines("G20\nG0 X1 Z-0.5\nG1 Y0.25 F10"), false);
        assert_eq!(c.lines, ["G21", "G0 X25.4 Z-12.7", "G1 Y6.35 F254"]);
    }

    #[test]
    fn a_program_already_in_the_unit_asked_for_keeps_its_numbers() {
        let text = lines("G21\nG1 X12.345 F600");
        let c = convert(&text, false);
        assert_eq!(c.lines, text);
        assert_eq!((c.numbers, c.changed_lines), (0, 0));
    }

    #[test]
    fn only_the_lengths_and_the_feed_change_and_the_comments_stay() {
        let c = convert(
            &lines("G21\nN10 G2 X25.4 Y0 I12.7 J0 F508 S12000 (cut X99 here)\nG83 Z-25.4 R2.54 Q5.08 P1\nA90 B180 T3 M6 H3 D3 ; X25.4"),
            true,
        );
        assert_eq!(c.lines[1], "N10 G2 X1 Y0 I0.5 J0 F20 S12000 (cut X99 here)");
        assert_eq!(c.lines[2], "G83 Z-1 R0.1 Q0.2 P1");
        assert_eq!(c.lines[3], "A90 B180 T3 M6 H3 D3 ; X25.4");
    }

    #[test]
    fn spaces_inside_a_word_are_kept() {
        let c = convert(&lines("G21\nG1 X 25.4 Y  -50.8"), true);
        assert_eq!(c.lines[1], "G1 X 1 Y  -2");
    }

    #[test]
    fn a_program_that_never_says_its_unit_is_read_as_millimetres_and_told_so() {
        let c = convert(&lines("G1 X25.4 F254"), true);
        assert_eq!(c.lines, ["G20", "G1 X1 F10"]);
        assert_eq!(c.findings.len(), 1);
        assert_eq!(c.findings[0].msg, Msg::UnitsAssumed);
        assert_eq!(c.findings[0].line, None);
        // to millimetres it needs no G21 and no change
        let back = convert(&lines("G1 X25.4 F254"), false);
        assert_eq!(back.lines, ["G1 X25.4 F254"]);
        // nothing to convert: no G20 added
        assert_eq!(convert(&lines("M3 S100"), true).lines, ["M3 S100"]);
    }

    #[test]
    fn moves_before_the_unit_is_said_are_in_the_default_of_the_controller_and_the_new_unit_is_said_first(
    ) {
        let c = convert(&lines("G1 X25.4\nG21\nG1 X25.4"), true);
        assert_eq!(c.lines, ["G20", "G1 X1", "G20", "G1 X1"]);
        assert_eq!(c.findings[0].msg, Msg::UnitsAssumed);
        // nothing to say first when only the lines after the unit have lengths
        let late = convert(&lines("M3 S100\nG21\nG1 X25.4"), true);
        assert_eq!(late.lines, ["M3 S100", "G20", "G1 X1"]);
        // converted again to the same unit, nothing changes
        assert_eq!(convert(&c.lines, true).lines, c.lines);
    }

    #[test]
    fn a_program_that_changes_unit_halfway_is_converted_from_the_unit_of_each_line() {
        let c = convert(&lines("G21\nG1 X25.4\nG20\nG1 X1\nG21\nG1 X25.4"), true);
        assert_eq!(c.lines, ["G20", "G1 X1", "G20", "G1 X1", "G20", "G1 X1"]);
        let at: Vec<(Option<usize>, Vec<String>)> = c
            .findings
            .iter()
            .map(|f| (f.line, f.args.clone()))
            .collect();
        assert_eq!(
            at,
            [
                (Some(2), vec!["G20".to_string()]),
                (Some(4), vec!["G21".to_string()])
            ]
        );
        assert!(c.findings.iter().all(|f| f.msg == Msg::UnitsSwitched));
    }

    #[test]
    fn the_unit_on_the_line_applies_to_the_whole_line() {
        // G20 and the move on one line: the move is in inches
        let c = convert(&lines("G21\nG1 G20 X1"), false);
        assert_eq!(c.lines, ["G21", "G1 G21 X25.4"]);
    }

    #[test]
    fn a_feed_in_inverse_time_is_not_a_length_and_is_said() {
        let c = convert(&lines("G21 G93\nG1 X25.4 F2\nG94\nG1 X25.4 F254"), true);
        assert_eq!(c.lines, ["G20 G93", "G1 X1 F2", "G94", "G1 X1 F10"]);
        assert_eq!(c.findings.len(), 1);
        assert_eq!(c.findings[0].msg, Msg::UnitsInverseFeed);
        assert_eq!(c.findings[0].line, Some(1));
        // a feed per revolution (G95) is a length
        let per_rev = convert(&lines("G21 G95\nG1 X25.4 F2.54"), true);
        assert_eq!(per_rev.lines[1], "G1 X1 F0.1");
    }

    #[test]
    fn a_value_given_by_a_parameter_or_an_expression_is_left_and_said() {
        let c = convert(&lines("G21\nG1 X#100 Y[#1+2] Z25.4"), true);
        assert_eq!(c.lines[1], "G1 X#100 Y[#1+2] Z1");
        let said: Vec<String> = c
            .findings
            .iter()
            .filter(|f| f.msg == Msg::UnitsExpression)
            .map(|f| f.args[0].clone())
            .collect();
        assert_eq!(said, ["X", "Y"]);
    }

    #[test]
    fn there_and_back_again_loses_nothing_a_machine_can_show() {
        // a fixed seed: 300 lengths between -500 and 500 mm with three decimals, through inches and back
        let mut seed = 0x2545_F491_4F6C_DD1Du64;
        for _ in 0..300 {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let mm = ((seed >> 33) % 1_000_000) as f64 / 1000.0 - 500.0;
            let program = vec![
                "G21".to_string(),
                format!("G1 X{mm:.3} F{:.1}", mm.abs() + 1.0),
            ];
            let inches = convert(&program, true);
            let back = convert(&inches.lines, false);
            assert_eq!(back.lines[0], "G21");
            let x: f64 = words(&back.lines[1])[1].1;
            assert!((x - mm).abs() < 0.0005, "{mm} came back as {x}");
        }
    }

    #[test]
    fn negative_zero_is_zero_and_the_zeros_at_the_end_are_dropped() {
        assert_eq!(write_number(-0.000_001, true), "0");
        assert_eq!(write_number(1.5, true), "1.5");
        assert_eq!(write_number(25.0, false), "25");
        assert_eq!(write_number(0.393_700_787, true), "0.3937");
        assert_eq!(write_number(-12.7, false), "-12.7");
    }

    #[test]
    fn lower_case_words_and_odd_lines_are_read_as_well() {
        let c = convert(
            &lines("g21\ng1 x25.4 f254\n\n%\n(only a comment x25.4)"),
            true,
        );
        assert_eq!(
            c.lines,
            ["g20", "g1 x1 f10", "", "%", "(only a comment x25.4)"]
        );
    }

    #[test]
    fn the_findings_are_told_in_the_eight_languages() {
        let c = convert(&lines("G1 X#1\nG21\nG20\nG93 F1"), true);
        let msgs: Vec<Msg> = c.findings.iter().map(|f| f.msg).collect();
        for m in [
            Msg::UnitsExpression,
            Msg::UnitsSwitched,
            Msg::UnitsInverseFeed,
        ] {
            assert!(msgs.contains(&m), "{m:?}");
        }
        for lang in crate::words::LANGS {
            for f in &c.findings {
                assert!(!f.text(lang).contains('{'), "{:?} {lang}", f.msg);
            }
        }
    }
}
