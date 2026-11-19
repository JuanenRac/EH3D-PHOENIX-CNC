//! The comparison of two programs: what each one is (the lines with code, the moves, the length of the rapids and of the cuts, the time, the tool
//! changes and the tools, the reach of the path, the deepest cut and what the second checker finds in it) and where the two differ, in a few lines a
//! person reads before sending the second one in place of the first. The numbers are the ones of the report, so a difference here is a difference there.
//! Only a reading: nothing is sent and nothing is changed.

use crate::cycles::toolpath_expanded;
use crate::progcheck::{check, counts};
use crate::sim::{hms, Rates};
use crate::stats::measure_accel;
use crate::tuning::Tuning;
use crate::words::{say, Msg};

/// Two lengths closer than this are the same one, in mm.
const SAME_MM: f64 = 0.05;
/// Two times closer than this are the same one, in seconds.
const SAME_S: f64 = 0.5;

/// Two numbers are the same when they are equal (two infinities of the same sign, or two numbers that are not numbers, are), or closer than `tol`.
fn close(a: f64, b: f64, tol: f64) -> bool {
    a == b || (a - b).abs() <= tol || (a.is_nan() && b.is_nan())
}

/// What a program is, in the numbers that are compared.
#[derive(Debug, Clone, PartialEq)]
pub struct Facts {
    pub code_lines: usize,
    pub moves: usize,
    pub rapid_mm: f64,
    pub cut_mm: f64,
    pub seconds: f64,
    pub changes: usize,
    /// The tools that make a move, in the order they first do, without repeats.
    pub tools: Vec<u32>,
    pub deepest_mm: Option<f64>,
    /// The box the path reaches: `[min x, max x, min y, max y, min z, max z]`.
    pub reach: Option<[f64; 6]>,
    /// What the second checker finds: info, warning and danger.
    pub findings: (usize, usize, usize),
}

/// Reads a program into its facts. The speeds and the time of a tool change are the ones the report takes.
pub fn facts(
    lines: &[String],
    rates: Rates,
    change_s: f64,
    accel_mm_s2: f64,
    tuning: &Tuning,
) -> Facts {
    let stats = measure_accel(lines, rates, change_s, accel_mm_s2);
    let path = toolpath_expanded(lines);
    let mut reach: Option<[f64; 6]> = None;
    for s in &path {
        for p in [s.from, s.to] {
            let r = reach.get_or_insert([p.0, p.0, p.1, p.1, p.2, p.2]);
            r[0] = r[0].min(p.0);
            r[1] = r[1].max(p.0);
            r[2] = r[2].min(p.1);
            r[3] = r[3].max(p.1);
            r[4] = r[4].min(p.2);
            r[5] = r[5].max(p.2);
        }
    }
    let mut tools = Vec::new();
    for t in stats.tools.iter().filter_map(|t| t.tool) {
        if !tools.contains(&t) {
            tools.push(t);
        }
    }
    Facts {
        code_lines: stats.program_lines,
        moves: path.len(),
        rapid_mm: stats.rapid_mm,
        cut_mm: stats.cut_mm,
        seconds: stats.seconds,
        changes: stats.changes,
        tools,
        deepest_mm: stats.deepest_mm,
        reach,
        findings: counts(&check(lines, tuning)),
    }
}

/// `+12.5 mm`, `-3.0 mm` or `0.0 mm`, with the percent of the first when it is not nothing.
fn length_change(a: f64, b: f64) -> String {
    let d = b - a;
    let sign = if d > 0.0 { "+" } else { "" };
    if a.abs() > 1e-9 {
        format!("{sign}{d:.1} mm, {sign}{:.0} %", d / a * 100.0)
    } else {
        format!("{sign}{d:.1} mm")
    }
}

fn time_change(a: f64, b: f64) -> String {
    let d = b - a;
    let sign = if d > 0.0 { "+" } else { "-" };
    let percent = if a > 1e-9 {
        format!(", {}{:.0} %", if d > 0.0 { "+" } else { "" }, d / a * 100.0)
    } else {
        String::new()
    };
    format!("{sign}{}{percent}", hms(d.abs()))
}

fn tool_list(tools: &[u32]) -> String {
    if tools.is_empty() {
        "-".to_string()
    } else {
        tools
            .iter()
            .map(|t| format!("T{t}"))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn reach_text(r: Option<[f64; 6]>) -> String {
    match r {
        Some(r) => format!(
            "X {:.1}..{:.1}, Y {:.1}..{:.1}, Z {:.1}..{:.1}",
            r[0], r[1], r[2], r[3], r[4], r[5]
        ),
        None => "-".to_string(),
    }
}

fn depth_text(d: Option<f64>) -> String {
    d.map_or("-".to_string(), |d| format!("{d:.2} mm"))
}

fn finding_text(f: (usize, usize, usize)) -> String {
    format!("{}/{}/{}", f.2, f.1, f.0)
}

/// The differences between the two, as lines in a language: a title with the two names, then one line for each thing that differs; if nothing does, one
/// line that says so. The number returned is how many things differ.
pub fn compare(a: &Facts, b: &Facts, names: (&str, &str), lang: &str) -> (Vec<String>, usize) {
    let mut out = vec![say(lang, Msg::CmpTitle, &[names.0, names.1])];
    let mut differ = 0usize;
    let mut line = |msg: Msg, args: Vec<String>| {
        out.push(say(lang, msg, &args));
        differ += 1;
    };
    if a.code_lines != b.code_lines {
        line(
            Msg::CmpLines,
            vec![a.code_lines.to_string(), b.code_lines.to_string()],
        );
    }
    if a.moves != b.moves {
        line(
            Msg::CmpMoves,
            vec![a.moves.to_string(), b.moves.to_string()],
        );
    }
    if !close(a.rapid_mm, b.rapid_mm, SAME_MM) {
        line(
            Msg::CmpRapid,
            vec![
                format!("{:.1}", a.rapid_mm),
                format!("{:.1}", b.rapid_mm),
                length_change(a.rapid_mm, b.rapid_mm),
            ],
        );
    }
    if !close(a.cut_mm, b.cut_mm, SAME_MM) {
        line(
            Msg::CmpCut,
            vec![
                format!("{:.1}", a.cut_mm),
                format!("{:.1}", b.cut_mm),
                length_change(a.cut_mm, b.cut_mm),
            ],
        );
    }
    if !close(a.seconds, b.seconds, SAME_S) {
        line(
            Msg::CmpTime,
            vec![
                hms(a.seconds),
                hms(b.seconds),
                time_change(a.seconds, b.seconds),
            ],
        );
    }
    if a.changes != b.changes {
        line(
            Msg::CmpChanges,
            vec![a.changes.to_string(), b.changes.to_string()],
        );
    }
    if a.tools != b.tools {
        line(
            Msg::CmpTools,
            vec![tool_list(&a.tools), tool_list(&b.tools)],
        );
    }
    let same_depth = match (a.deepest_mm, b.deepest_mm) {
        (None, None) => true,
        (Some(x), Some(y)) => close(x, y, SAME_MM),
        _ => false,
    };
    if !same_depth {
        line(
            Msg::CmpDeepest,
            vec![depth_text(a.deepest_mm), depth_text(b.deepest_mm)],
        );
    }
    let same_reach = match (a.reach, b.reach) {
        (None, None) => true,
        (Some(x), Some(y)) => x.iter().zip(y.iter()).all(|(p, q)| close(*p, *q, SAME_MM)),
        _ => false,
    };
    if !same_reach {
        line(
            Msg::CmpReach,
            vec![reach_text(a.reach), reach_text(b.reach)],
        );
    }
    if a.findings != b.findings {
        line(
            Msg::CmpFindings,
            vec![finding_text(a.findings), finding_text(b.findings)],
        );
    }
    if differ == 0 {
        out.push(say(lang, Msg::CmpSame, &[] as &[&str]));
    }
    (out, differ)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATES: Rates = Rates {
        rapid: 3000.0,
        default_feed: 600.0,
    };

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(str::to_string).collect()
    }

    fn facts_of(text: &str) -> Facts {
        facts(&lines(text), RATES, 5.0, 0.0, &Tuning::default())
    }

    const A: &str =
        "G21 G90\nM3 S12000\nT1 M6\nG0 X10 Y10\nG1 Z-2 F200\nG1 X50 F600\nG0 Z5\nM5\nM30";

    #[test]
    fn the_facts_are_the_numbers_of_the_report() {
        let f = facts_of(A);
        assert_eq!(f.code_lines, 9);
        assert_eq!(f.moves, 4);
        assert_eq!(f.tools, [1]);
        assert_eq!(f.changes, 1);
        assert!((f.cut_mm - 42.0).abs() < 1e-9, "{}", f.cut_mm);
        assert_eq!(f.deepest_mm, Some(-2.0));
        assert_eq!(f.reach, Some([0.0, 50.0, 0.0, 10.0, -2.0, 5.0]));
        // the same text has the same facts
        assert_eq!(facts_of(A), f);
    }

    #[test]
    fn two_equal_programs_do_not_differ() {
        let (text, n) = compare(&facts_of(A), &facts_of(A), ("a.nc", "b.nc"), "en");
        assert_eq!(n, 0);
        assert_eq!(text.len(), 2);
        assert!(text[0].contains("a.nc") && text[0].contains("b.nc"));
        assert_eq!(
            text[1],
            "The two programs do not differ in what is compared."
        );
    }

    #[test]
    fn a_longer_cut_a_new_tool_and_a_deeper_pass_are_each_a_line() {
        let b = "G21 G90\nM3 S12000\nT1 M6\nG0 X10 Y10\nG1 Z-3 F200\nG1 X90 F600\nG0 Z5\nT2 M6\nG1 Z-1 F100\nM5\nM30";
        let (text, n) = compare(&facts_of(A), &facts_of(b), ("a", "b"), "en");
        let all = text.join("\n");
        assert!(n >= 6, "{n}: {all}");
        assert!(all.contains("Lines with code: 9 -> 11"), "{all}");
        assert!(all.contains("Tools: T1 -> T1 T2"), "{all}");
        assert!(all.contains("Tool changes: 1 -> 2"), "{all}");
        assert!(all.contains("Deepest cut: -2.00 mm -> -3.00 mm"), "{all}");
        assert!(all.contains("Cutting length: 42.0 mm -> "), "{all}");
        assert!(all.contains("Reach: X 0.0..50.0"), "{all}");
        assert_eq!(text.len(), n + 1);
    }

    #[test]
    fn the_change_is_signed_and_has_its_percent() {
        assert_eq!(length_change(100.0, 125.0), "+25.0 mm, +25 %");
        assert_eq!(length_change(100.0, 50.0), "-50.0 mm, -50 %");
        assert_eq!(length_change(0.0, 7.0), "+7.0 mm");
        assert_eq!(time_change(60.0, 90.0), "+30 s, +50 %");
        assert_eq!(time_change(120.0, 60.0), "-1 min 00 s, -50 %");
        assert_eq!(time_change(0.0, 10.0), "+10 s");
    }

    #[test]
    fn a_difference_smaller_than_what_a_machine_can_show_is_not_one() {
        let mut a = facts_of(A);
        let mut b = a.clone();
        b.cut_mm += 0.01;
        b.rapid_mm -= 0.04;
        b.seconds += 0.4;
        b.deepest_mm = Some(-2.04);
        b.reach = Some([0.0, 50.04, 0.0, 10.0, -2.0, 5.0]);
        assert_eq!(compare(&a, &b, ("a", "b"), "en").1, 0);
        // the checker counts and a missing path are differences
        b.findings = (a.findings.0, a.findings.1 + 1, a.findings.2);
        assert_eq!(compare(&a, &b, ("a", "b"), "en").1, 1);
        a.reach = None;
        a.deepest_mm = None;
        let (text, n) = compare(&a, &b, ("a", "b"), "en");
        assert_eq!(n, 3, "{text:?}");
        assert!(text.iter().any(|l| l.contains("Reach: - ->")), "{text:?}");
    }

    #[test]
    fn a_number_that_is_not_a_number_or_is_infinite_is_the_same_as_itself() {
        assert!(close(f64::INFINITY, f64::INFINITY, SAME_MM));
        assert!(close(f64::NAN, f64::NAN, SAME_MM));
        assert!(close(1.0, 1.04, SAME_MM) && !close(1.0, 1.06, SAME_MM));
        assert!(!close(f64::INFINITY, f64::NEG_INFINITY, SAME_MM));
        assert!(!close(f64::INFINITY, 5.0, SAME_MM));
        assert!(!close(f64::NAN, 5.0, SAME_MM));
        // a program whose numbers overflow still does not differ from itself
        let huge = format!("G21 G90\nG1 X{} F100", "9".repeat(400));
        let f = facts_of(&huge);
        assert_eq!(compare(&f, &f, ("a", "a"), "en").1, 0);
    }

    #[test]
    fn the_findings_are_shown_as_danger_warning_info() {
        assert_eq!(finding_text((3, 2, 1)), "1/2/3");
        let risky = facts_of("G0 X5\nG1 X6");
        assert!(risky.findings != (0, 0, 0));
    }

    #[test]
    fn an_empty_program_has_no_reach_and_no_tools() {
        let f = facts_of("");
        assert_eq!(
            (f.code_lines, f.moves, f.reach, f.tools.len()),
            (0, 0, None, 0)
        );
        let (text, n) = compare(&f, &f, ("x", "y"), "de");
        assert_eq!(n, 0);
        assert_eq!(text.len(), 2);
    }

    #[test]
    fn every_line_is_told_in_the_eight_languages_with_its_holes_filled() {
        let b = "G21 G90\nT1 M6\nT2 M6\nG0 X1\nG1 Z-9 F10\nG1 X400 F10";
        for lang in crate::words::LANGS {
            let (text, n) = compare(&facts_of(A), &facts_of(b), ("one.nc", "two.nc"), lang);
            assert!(n >= 8, "{lang}: {n}");
            assert!(text.iter().all(|l| !l.contains('{')), "{lang}: {text:?}");
            let (same, _) = compare(&facts_of(A), &facts_of(A), ("one.nc", "two.nc"), lang);
            assert!(same[1] != text[1] && !same[1].contains('{'), "{lang}");
        }
    }
}
