//! The validator of a library of macros: what a macro leaves behind when it ends. A macro runs between two lines of a program or before
//! one, so a mode it turns on and never turns off (relative distances, a work system other than G54, a spindle or the coolant left on,
//! a canned cycle or a cutter compensation still on, inches) is inherited by what runs next. It also names a macro that has no line, one
//! that ends the program, and one that moves without choosing G90 or G91 in it. The checks of the program (`progcheck`) are run on each
//! macro as well. Nothing here sends anything: it reads the text of the library.

use crate::gcode::words;
use crate::macros::{Library, Macro};
use crate::progcheck::{Finding, Level};
use crate::tuning::Tuning;
use crate::words::{say, Msg};

/// The modes a macro changes and leaves changed, each with the line (from 0) that last changed it.
#[derive(Debug, Default)]
struct Left {
    relative: Option<usize>,
    wcs: Option<(usize, u8)>,
    spindle: Option<(usize, i32)>,
    coolant: Option<(usize, i32)>,
    cycle: Option<(usize, i32)>,
    comp: Option<(usize, i32)>,
    plane: Option<(usize, i32)>,
    inches: Option<usize>,
}

/// What the macro leaves on, as findings in the order of their lines: `Msg::MacroLeak` with the line, the macro and the code; a line
/// with a `M2` or `M30` is `Msg::MacroEnds`; a macro with no line, and one that moves with no `G90` or `G91` in it, have no line.
pub fn leaks(m: &Macro) -> Vec<Finding> {
    let mut out: Vec<Finding> = Vec::new();
    if m.lines.iter().all(|l| words(l).is_empty()) {
        out.push(Finding {
            line: None,
            level: Level::Warning,
            msg: Msg::MacroEmpty,
            args: vec![m.name.clone()],
        });
        return out;
    }
    let mut left = Left::default();
    let mut chooses_distance = false;
    for (n, raw) in m.lines.iter().enumerate() {
        for (c, v) in words(raw) {
            let t = (v * 10.0).round() as i32;
            match c {
                'G' => match t {
                    900 | 910 => {
                        chooses_distance = true;
                        left.relative = (t == 910).then_some(n);
                    }
                    200 => left.inches = Some(n),
                    210 => left.inches = None,
                    540 => left.wcs = None,
                    550 | 560 | 570 | 580 | 590 => left.wcs = Some((n, (t / 10) as u8)),
                    170 => left.plane = None,
                    180 | 190 => left.plane = Some((n, t / 10)),
                    400 => left.comp = None,
                    410 | 420 => left.comp = Some((n, t / 10)),
                    800 => left.cycle = None,
                    730 | 810..=890 if t % 10 == 0 => left.cycle = Some((n, t / 10)),
                    _ => {}
                },
                'M' => match v as i32 {
                    3 | 4 => left.spindle = Some((n, v as i32)),
                    5 => left.spindle = None,
                    7 | 8 => left.coolant = Some((n, v as i32)),
                    9 => left.coolant = None,
                    2 | 30 => {
                        out.push(Finding {
                            line: Some(n),
                            level: Level::Warning,
                            msg: Msg::MacroEnds,
                            args: vec![m.name.clone(), format!("M{}", v as i32)],
                        });
                        left.spindle = None;
                        left.coolant = None;
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }
    let mut leak = |line: usize, code: String| {
        out.push(Finding {
            line: Some(line),
            level: Level::Warning,
            msg: Msg::MacroLeak,
            args: vec![m.name.clone(), code],
        });
    };
    if let Some(n) = left.relative {
        leak(n, "G91".to_string());
    }
    if let Some(n) = left.inches {
        leak(n, "G20".to_string());
    }
    if let Some((n, s)) = left.wcs {
        leak(n, format!("G{s}"));
    }
    if let Some((n, p)) = left.plane {
        leak(n, format!("G{p}"));
    }
    if let Some((n, c)) = left.comp {
        leak(n, format!("G{c}"));
    }
    if let Some((n, c)) = left.cycle {
        leak(n, format!("G{c}"));
    }
    if let Some((n, s)) = left.spindle {
        leak(n, format!("M{s}"));
    }
    if let Some((n, c)) = left.coolant {
        leak(n, format!("M{c}"));
    }
    if m.moves() && !chooses_distance {
        out.push(Finding {
            line: None,
            level: Level::Warning,
            msg: Msg::MacroModeless,
            args: vec![m.name.clone()],
        });
    }
    out.sort_by_key(|f| f.line.unwrap_or(usize::MAX));
    out
}

/// Every macro of the library with what is wrong in it: what it leaves on, and what the checks of the program say about its lines
/// (the remarks about a whole program are left out, as `Macro::preflight` does). The name of the macro goes with each finding.
pub fn validate(lib: &Library, t: &Tuning) -> Vec<(String, Finding)> {
    let mut out = Vec::new();
    for m in &lib.macros {
        for f in leaks(m) {
            out.push((m.name.clone(), f));
        }
        for f in m.preflight(t) {
            if f.level >= Level::Warning {
                out.push((m.name.clone(), f));
            }
        }
    }
    out
}

/// The validation of the text of a file of macros: the lines of the text that could not be read, then each finding, and how many things
/// need reading. This is what `--macros FILE` prints; the exit code is 0 when the count is 0.
pub fn validate_text(text: &str, t: &Tuning, lang: &str) -> (String, usize) {
    let lib = Library::parse(text, t.macro_max_lines);
    let mut lines: Vec<String> = Vec::new();
    let mut count = 0;
    for p in &lib.problems {
        lines.push(format!("- {}", p.text(lang)));
        count += 1;
    }
    for (name, f) in validate(&lib, t) {
        let said = f.text(lang);
        // the findings of the program checks do not carry the name of the macro; those of `leaks` do
        if matches!(
            f.msg,
            Msg::MacroEmpty | Msg::MacroLeak | Msg::MacroEnds | Msg::MacroModeless
        ) {
            lines.push(format!("- {said}"));
        } else {
            lines.push(format!("- [{name}] {said}"));
        }
        count += 1;
    }
    if lines.is_empty() {
        lines.push(format!("- {}", say(lang, Msg::ReportClean, &[] as &[&str])));
    }
    let mut s = lines.join("\n");
    s.push('\n');
    (s, count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lib(text: &str) -> Library {
        Library::parse(text, 200)
    }

    fn leaks_of(text: &str) -> Vec<(Msg, Option<usize>, Vec<String>)> {
        let l = lib(text);
        leaks(&l.macros[0])
            .into_iter()
            .map(|f| (f.msg, f.line, f.args))
            .collect()
    }

    #[test]
    fn a_macro_that_puts_everything_back_leaks_nothing() {
        let r = leaks_of(
            "[park]\nG91\nG0 Z5\nG90\nG55\nG54\nM3 S1000\nM5\nM8\nM9\nG81 Z-1 R1\nG80\nG41\nG40\nG18\nG17\nG20\nG21\nG0 X0 Y0",
        );
        assert!(r.is_empty(), "{r:?}");
    }

    #[test]
    fn what_is_left_on_is_named_with_its_line() {
        let r = leaks_of("[a]\nG90\nG91\nG0 X5\nG55\nM3 S100\nM8\nG81 Z-1 R1\nG42\nG19\nG20");
        let named: Vec<(usize, &str)> = r
            .iter()
            .map(|(m, l, a)| {
                assert_eq!(*m, Msg::MacroLeak);
                (l.unwrap(), a[1].as_str())
            })
            .collect();
        assert_eq!(
            named,
            [
                (1, "G91"),
                (3, "G55"),
                (4, "M3"),
                (5, "M8"),
                (6, "G81"),
                (7, "G42"),
                (8, "G19"),
                (9, "G20")
            ]
        );
        assert!(r.iter().all(|(_, _, a)| a[0] == "a"));
    }

    #[test]
    fn a_mode_turned_off_later_or_set_to_the_usual_one_is_not_left() {
        assert!(leaks_of("[a]\nG20\nG0 X1\nG21\nG90").is_empty());
        assert!(leaks_of("[a]\nG55\nG0 X1\nG54\nG90").is_empty());
        // the last word wins
        let r = leaks_of("[a]\nM3 S1\nM5\nM4 S1\nG90");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].2[1], "M4");
        // the usual modes are not a leak
        assert!(leaks_of("[a]\nG21\nG17\nG54\nG40\nG80\nG90\nG0 X1").is_empty());
    }

    #[test]
    fn a_macro_that_ends_the_program_is_said_and_stops_the_spindle_and_the_coolant_for_the_rest() {
        let r = leaks_of("[a]\nM3 S100\nM8\nM30");
        assert_eq!(
            r,
            vec![(
                Msg::MacroEnds,
                Some(2),
                vec!["a".to_string(), "M30".to_string()]
            )]
        );
        let two = leaks_of("[a]\nG90\nM2");
        assert_eq!(two[0].0, Msg::MacroEnds);
    }

    #[test]
    fn a_macro_with_no_line_and_one_that_moves_without_a_distance_mode_are_said() {
        let l = lib("[empty]\ninfo = nothing\nconfirm = yes\n\n[moves]\nG0 X5\n[stay]\nM5");
        let empty = leaks(&l.macros[0]);
        assert_eq!(empty.len(), 1);
        assert_eq!((empty[0].msg, empty[0].line), (Msg::MacroEmpty, None));
        let moves = leaks(&l.macros[1]);
        assert_eq!(moves.len(), 1);
        assert_eq!(moves[0].msg, Msg::MacroModeless);
        assert_eq!(moves[0].args, ["moves"]);
        // no move, so no need for a mode
        assert!(leaks(&l.macros[2]).is_empty());
        // a comment-only macro is empty
        let c = lib("[c]\n(only a comment)\n; nothing");
        assert_eq!(leaks(&c.macros[0])[0].msg, Msg::MacroEmpty);
    }

    #[test]
    fn the_whole_library_is_checked_and_the_checks_of_the_program_come_with_each_macro() {
        let l = lib("[a]\nG91\nG0 X1\n[b]\nG90\nG0 X1\nG10 L2 P1 X0\n");
        let v = validate(&l, &Tuning::default());
        let names: Vec<&str> = v.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"a") && names.contains(&"b"));
        // b only has the G10 of the checker, a only has the mode it left
        assert!(v.iter().any(|(n, f)| n == "b" && f.msg == Msg::Offsets));
        assert!(v.iter().any(|(n, f)| n == "a" && f.msg == Msg::MacroLeak));
        assert!(!v.iter().any(|(n, f)| n == "a" && f.msg == Msg::Offsets));
        // the remark about a whole program (no units) is not made about a macro
        assert!(!v.iter().any(|(_, f)| f.msg == Msg::NoUnits));
    }

    #[test]
    fn the_text_of_the_validation_lists_the_problems_of_the_file_and_counts_them() {
        let text = "[ok]\nG90\nG0 X1\n\n[bad name!]\nG0 X1\n\n[leaky]\nG91\nG1 X1 F5\n";
        let (s, n) = validate_text(text, &Tuning::default(), "en");
        // the name that cannot be used and the leak (and the remark of the checker about the end in relative mode)
        assert!(n >= 2, "{s}");
        assert!(s.contains("G91 is still in force"), "{s}");
        assert!(s.lines().all(|l| l.starts_with("- ")), "{s}");
        assert_eq!(s.lines().count(), n, "{s}");
        let clean = validate_text("[a]\nG90\nG0 X1\n", &Tuning::default(), "en");
        assert_eq!(clean.1, 0);
        assert_eq!(clean.0, "- Nothing looks wrong\n");
        let nothing = validate_text("", &Tuning::default(), "en");
        assert_eq!(nothing, ("- Nothing looks wrong\n".to_string(), 0));
    }

    #[test]
    fn every_finding_reads_in_the_eight_languages_without_a_hole_left() {
        let text = "[a]\nG91\nG0 X1\nM30\n\n[e]\n";
        for lang in crate::words::LANGS {
            let (s, n) = validate_text(text, &Tuning::default(), lang);
            assert!(n >= 3, "{lang}: {s}");
            assert!(!s.contains('{'), "{lang}: {s}");
        }
    }
}
