//! The library of macros of the operator: short pieces of G-code with a name (park the tool, go to the tool-change point, clean the table)
//! kept in a text file. A macro asks the operator before it runs (and with the default settings every macro does); the checker reads it
//! first, and a line meant for the controller itself (`$...`) is never allowed in one. Nothing here sends anything: the window shows the
//! question, runs the lines through the same path as a typed line and stops at the first one the controller refuses.
//!
//! The file: a section `[name]`, then optionally `info = text` and `confirm = yes|no`, then the lines of G-code. A line that starts with
//! `#` and the empty lines are left out of the file.

use crate::gcode::words;
use crate::progcheck::{check, Finding, Level};
use crate::tuning::Tuning;
use crate::words::{say, Msg};

/// The most macros in a library.
pub const MAX_MACROS: usize = 256;

#[derive(Debug, Clone, PartialEq)]
pub struct Macro {
    pub name: String,
    pub info: String,
    /// The macro asks before it runs (it always does when the settings say so, and when it moves or the checker has something to say).
    pub confirm: bool,
    pub lines: Vec<String>,
}

/// Something wrong in the file of macros.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// The line of the file (from 0).
    pub line: usize,
    pub msg: Msg,
    pub args: Vec<String>,
}

impl Problem {
    pub fn text(&self, lang: &str) -> String {
        let mut a = vec![(self.line + 1).to_string()];
        a.extend(self.args.iter().cloned());
        say(lang, self.msg, &a)
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Library {
    pub macros: Vec<Macro>,
    pub problems: Vec<Problem>,
}

fn valid_name(n: &str) -> bool {
    !n.is_empty()
        && n.len() <= 32
        && n.chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

fn yes(v: &str) -> bool {
    matches!(
        v.to_ascii_lowercase().as_str(),
        "yes" | "on" | "true" | "1" | "y"
    )
}

impl Library {
    /// Reads the file. A macro with a problem (a bad name, a `$` line, too many lines, a name used twice) is left out whole and the
    /// problem is listed; the rest of the file is still read.
    pub fn parse(text: &str, max_lines: usize) -> Library {
        let mut lib = Library::default();
        let mut cur: Option<(Macro, bool)> = None;
        let close = |cur: &mut Option<(Macro, bool)>, lib: &mut Library| {
            if let Some((m, bad)) = cur.take() {
                if !bad {
                    lib.macros.push(m);
                }
            }
        };
        for (n, raw) in text.trim_start_matches('\u{feff}').lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(name) = line.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
                close(&mut cur, &mut lib);
                let name = name.trim().to_string();
                let mut bad = false;
                if !valid_name(&name) {
                    lib.problems.push(Problem {
                        line: n,
                        msg: Msg::MacroName,
                        args: vec![],
                    });
                    bad = true;
                } else if lib
                    .macros
                    .iter()
                    .any(|m| m.name.eq_ignore_ascii_case(&name))
                {
                    lib.problems.push(Problem {
                        line: n,
                        msg: Msg::MacroTwice,
                        args: vec![name.clone()],
                    });
                    bad = true;
                } else if lib.macros.len() >= MAX_MACROS {
                    bad = true;
                }
                cur = Some((
                    Macro {
                        name,
                        info: String::new(),
                        confirm: true,
                        lines: Vec::new(),
                    },
                    bad,
                ));
                continue;
            }
            let Some((m, bad)) = cur.as_mut() else {
                lib.problems.push(Problem {
                    line: n,
                    msg: Msg::MacroOutside,
                    args: vec![],
                });
                continue;
            };
            if m.lines.is_empty() {
                if let Some((k, v)) = line.split_once('=') {
                    match k.trim().to_ascii_lowercase().as_str() {
                        "info" => {
                            m.info = v
                                .trim()
                                .chars()
                                .filter(|c| !c.is_control())
                                .take(120)
                                .collect();
                            continue;
                        }
                        "confirm" => {
                            m.confirm = yes(v.trim());
                            continue;
                        }
                        _ => {}
                    }
                }
            }
            if line.starts_with('$') {
                lib.problems.push(Problem {
                    line: n,
                    msg: Msg::MacroSystem,
                    args: vec![],
                });
                *bad = true;
                continue;
            }
            if m.lines.len() >= max_lines {
                if !*bad {
                    lib.problems.push(Problem {
                        line: n,
                        msg: Msg::MacroLong,
                        args: vec![m.name.clone(), max_lines.to_string()],
                    });
                }
                *bad = true;
                continue;
            }
            m.lines.push(line.to_string());
        }
        close(&mut cur, &mut lib);
        lib
    }

    /// A macro by name (the case does not matter).
    pub fn get(&self, name: &str) -> Option<&Macro> {
        self.macros
            .iter()
            .find(|m| m.name.eq_ignore_ascii_case(name.trim()))
    }

    /// The file for this library; reading it again gives the same library.
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        for m in &self.macros {
            s.push_str(&format!("[{}]\n", m.name));
            if !m.info.is_empty() {
                s.push_str(&format!("info = {}\n", m.info));
            }
            s.push_str(&format!(
                "confirm = {}\n",
                if m.confirm { "yes" } else { "no" }
            ));
            for l in &m.lines {
                s.push_str(l);
                s.push('\n');
            }
            s.push('\n');
        }
        s
    }
}

impl Macro {
    /// What the checker says about the macro. The remarks that only make sense for a whole program (no end, no moves, units never set)
    /// are left out.
    pub fn preflight(&self, t: &Tuning) -> Vec<Finding> {
        let mut forced = t.clone();
        forced.progcheck = true;
        check(&self.lines, &forced)
            .into_iter()
            .filter(|f| !matches!(f.msg, Msg::NoMoves | Msg::NoUnits))
            .collect()
    }

    /// Whether any line moves an axis.
    pub fn moves(&self) -> bool {
        self.lines.iter().any(|l| {
            words(l)
                .iter()
                .any(|&(c, _)| matches!(c, 'X' | 'Y' | 'Z' | 'A' | 'B' | 'C'))
        })
    }

    /// Whether the operator must confirm before it runs: always when the settings say so or the macro asks, and also when it moves
    /// or the checker has a warning or a danger for it.
    pub fn needs_confirmation(&self, t: &Tuning) -> bool {
        t.macros_confirm
            || self.confirm
            || self.moves()
            || self.preflight(t).iter().any(|f| f.level >= Level::Warning)
    }

    /// The question for the operator, in a language.
    pub fn question(&self, lang: &str) -> String {
        say(
            lang,
            Msg::MacroAsk,
            &[self.name.clone(), self.lines.len().to_string()],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "# my macros\n[park]\ninfo = Go to the parking point\nG90\nG53 G0 Z0\n\n[Spindle_Off]\nconfirm = no\nM5\n";

    #[test]
    fn a_file_is_read_into_macros() {
        let lib = Library::parse(FILE, 200);
        assert!(lib.problems.is_empty(), "{:?}", lib.problems);
        assert_eq!(lib.macros.len(), 2);
        let p = lib.get("PARK").unwrap();
        assert_eq!(p.info, "Go to the parking point");
        assert_eq!(p.lines, vec!["G90", "G53 G0 Z0"]);
        assert!(p.confirm);
        assert!(!lib.get("spindle_off").unwrap().confirm);
        assert!(lib.get("nothing").is_none());
    }

    #[test]
    fn the_library_is_written_back_as_it_was_read() {
        let lib = Library::parse(FILE, 200);
        assert_eq!(Library::parse(&lib.to_text(), 200), lib);
    }

    #[test]
    fn every_macro_asks_with_the_default_settings() {
        let lib = Library::parse(FILE, 200);
        let t = Tuning::default();
        assert!(lib.get("spindle_off").unwrap().needs_confirmation(&t));
        let lax = Tuning {
            macros_confirm: false,
            ..Tuning::default()
        };
        // a macro that does not move, has nothing to warn about and does not ask for it may run without asking if the settings allow it
        assert!(!lib.get("spindle_off").unwrap().needs_confirmation(&lax));
        // one that moves always asks
        assert!(lib.get("park").unwrap().needs_confirmation(&lax));
    }

    #[test]
    fn a_macro_with_a_danger_asks_even_when_it_says_it_does_not_need_to() {
        let lib = Library::parse("[bad]\nconfirm = no\nM112\n", 200);
        let lax = Tuning {
            macros_confirm: false,
            ..Tuning::default()
        };
        let m = lib.get("bad").unwrap();
        assert!(!m.moves());
        assert!(m.needs_confirmation(&lax));
        // and the checker cannot be turned off for a macro
        let off = Tuning {
            progcheck: false,
            macros_confirm: false,
            ..Tuning::default()
        };
        assert!(m.needs_confirmation(&off));
    }

    #[test]
    fn a_system_line_drops_the_whole_macro() {
        let lib = Library::parse("[hack]\nG90\n$130=9999\nG0 X1\n[ok]\nM5\n", 200);
        assert_eq!(lib.macros.len(), 1);
        assert_eq!(lib.macros[0].name, "ok");
        assert_eq!(lib.problems.len(), 1);
        assert_eq!(lib.problems[0].msg, Msg::MacroSystem);
        assert_eq!(lib.problems[0].line, 2);
        assert_eq!(
            lib.problems[0].text("en"),
            "Line 3: a line that starts with $ is not allowed in a macro"
        );
    }

    #[test]
    fn bad_names_repeats_and_stray_lines_are_reported() {
        let lib = Library::parse("G0 X1\n[bad name!]\nM5\n[a]\nM5\n[A]\nM3\n[]\nM5\n", 200);
        let kinds: Vec<_> = lib.problems.iter().map(|p| p.msg).collect();
        assert_eq!(
            kinds,
            vec![
                Msg::MacroOutside,
                Msg::MacroName,
                Msg::MacroTwice,
                Msg::MacroName
            ]
        );
        assert_eq!(lib.macros.len(), 1);
        assert_eq!(lib.macros[0].name, "a");
        assert_eq!(lib.problems[2].args, vec!["A"]);
    }

    #[test]
    fn a_macro_that_is_too_long_is_left_out_and_said_once() {
        let lib = Library::parse("[long]\nM5\nM5\nM5\nM5\n[short]\nM5\n", 3);
        assert_eq!(lib.macros.len(), 1);
        assert_eq!(lib.problems.len(), 1);
        assert_eq!(lib.problems[0].msg, Msg::MacroLong);
        assert_eq!(lib.problems[0].args, vec!["long", "3"]);
    }

    #[test]
    fn the_info_is_one_short_clean_line() {
        let long = "x".repeat(300);
        let lib = Library::parse(&format!("[m]\ninfo = a\u{7}b {long}\nM5\n"), 200);
        let info = &lib.get("m").unwrap().info;
        assert!(!info.contains('\u{7}'));
        assert_eq!(info.chars().count(), 120);
    }

    #[test]
    fn an_equals_sign_inside_the_code_is_code() {
        // after the first line of code, `info = ...` is a line like any other (the controller will decide)
        let lib = Library::parse("[m]\nM5\ninfo = x\n", 200);
        assert_eq!(lib.get("m").unwrap().lines, vec!["M5", "info = x"]);
    }

    #[test]
    fn the_checker_leaves_out_the_remarks_for_whole_programs() {
        let lib = Library::parse("[m]\nG92 X0\n", 200);
        let f = lib.get("m").unwrap().preflight(&Tuning::default());
        assert!(f
            .iter()
            .all(|f| !matches!(f.msg, Msg::NoUnits | Msg::NoMoves)));
        assert!(f.iter().any(|f| f.msg == Msg::Shift));
    }

    #[test]
    fn the_question_names_the_macro_and_its_size() {
        let lib = Library::parse(FILE, 200);
        assert_eq!(
            lib.get("park").unwrap().question("en"),
            "Run the macro park? It has 2 lines and can move the machine"
        );
    }

    #[test]
    fn a_file_with_a_byte_order_mark_and_no_macros_is_empty() {
        assert!(Library::parse("\u{feff}", 10).macros.is_empty());
        assert!(Library::parse("", 10).problems.is_empty());
        assert!(Library::parse("\u{feff}[a]\nM5\n", 10).get("a").is_some());
    }
}
