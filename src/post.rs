//! The post-processors: the last step between the program and a controller, written as a text file of rules (`phoenix-cnc-post-NAME.txt`
//! next to the settings file, `examples/post.txt` is a model) and not as code. A post-processor adds lines at the start and at the end,
//! renames or drops words, numbers the lines, fixes the number of decimals, strips the comments and changes the case. Declarative rules are
//! always on; a rule that asks for a script (`script = FILE`) runs only with `post_scripts = on` in the motion file, which is off by
//! default, and this build has no script engine yet, so even then it is not run and the program says so. Nothing here moves a machine.
//!
//! A line of the file is `key = value`; the keys `header`, `footer`, `replace` (`OLD -> NEW`, a whole word) and `drop` (a whole word) may
//! repeat. Empty lines and lines that start with `#` or `;` are skipped; a line that cannot be used is named, with its number, and left out.

use std::path::{Path, PathBuf};

/// The biggest file read, the most rules and the most lines of a program a post-processor handles.
pub const MAX_FILE_BYTES: u64 = 16 * 1024;
pub const MAX_RULES: usize = 128;
pub const MAX_PROGRAM_LINES: usize = 10_000_000;

/// A post-processor.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Post {
    pub header: Vec<String>,
    pub footer: Vec<String>,
    /// Whole words that are written another way (`G0` -> `G00`).
    pub replace: Vec<(String, String)>,
    /// Whole words that are taken out (`M6`); a line left with nothing is dropped.
    pub drop: Vec<String>,
    /// Number the lines `N<start>`, `N<start+step>`..., when set.
    pub numbers: Option<(u32, u32)>,
    /// The decimals of the numbers of the axes (`X Y Z A B C I J K R`), when set; trailing zeros are not written.
    pub decimals: Option<usize>,
    pub strip_comments: bool,
    pub uppercase: bool,
    /// A script the post-processor asks for.
    pub script: Option<String>,
}

pub use crate::problem::{Problem, Why};

fn flag(v: &str) -> Option<bool> {
    match v.trim().to_ascii_lowercase().as_str() {
        "on" | "yes" | "true" | "1" => Some(true),
        "off" | "no" | "false" | "0" => Some(false),
        _ => None,
    }
}

impl Post {
    pub fn parse(text: &str) -> (Post, Vec<Problem>) {
        let mut p = Post::default();
        let mut problems = Vec::new();
        let (mut numbers_on, mut start, mut step) = (false, 10u32, 10u32);
        let mut rules = 0;
        for (i, raw) in text.trim_start_matches('\u{feff}').lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            let n = i + 1;
            let mut bad = |why: Why, detail: &str| problems.push(Problem::new(n, why, detail));
            let Some((k, v)) = line.split_once('=') else {
                bad(Why::NotKeyValue, "");
                continue;
            };
            let (k, v) = (k.trim().to_ascii_lowercase(), v.trim());
            rules += 1;
            if rules > MAX_RULES {
                bad(Why::TooManyRules, "");
                break;
            }
            match k.as_str() {
                "header" => p.header.push(v.to_string()),
                "footer" => p.footer.push(v.to_string()),
                "replace" => match v.split_once("->") {
                    Some((a, b)) if is_word(a.trim()) && !b.trim().is_empty() => p
                        .replace
                        .push((a.trim().to_ascii_uppercase(), b.trim().to_string())),
                    _ => bad(Why::ReplaceForm, ""),
                },
                "drop" => {
                    if is_word(v) {
                        p.drop.push(v.to_ascii_uppercase());
                    } else {
                        bad(Why::DropForm, "");
                    }
                }
                "line_numbers" => match flag(v) {
                    Some(b) => numbers_on = b,
                    None => bad(Why::OnOff, ""),
                },
                "line_number_start" => match v.parse::<u32>() {
                    Ok(x) if x <= 99_999 => start = x,
                    _ => bad(Why::OutOfRange, "0 - 99999"),
                },
                "line_number_step" => match v.parse::<u32>() {
                    Ok(x) if (1..=1000).contains(&x) => step = x,
                    _ => bad(Why::OutOfRange, "1 - 1000"),
                },
                "decimals" => match v.parse::<usize>() {
                    Ok(x) if x <= 6 => p.decimals = Some(x),
                    _ => bad(Why::OutOfRange, "0 - 6"),
                },
                "comments" => match v.to_ascii_lowercase().as_str() {
                    "keep" => p.strip_comments = false,
                    "strip" => p.strip_comments = true,
                    _ => bad(Why::Choice, "keep, strip"),
                },
                "case" => match v.to_ascii_lowercase().as_str() {
                    "upper" => p.uppercase = true,
                    "keep" => p.uppercase = false,
                    _ => bad(Why::Choice, "upper, keep"),
                },
                "script" => {
                    if v.is_empty() || v.len() > 260 {
                        bad(Why::NeedsName, "");
                    } else {
                        p.script = Some(v.to_string());
                    }
                }
                _ => bad(Why::UnknownKey, ""),
            }
        }
        if numbers_on {
            p.numbers = Some((start, step));
        }
        (p, problems)
    }

    /// Whether the post-processor changes nothing at all.
    pub fn is_empty(&self) -> bool {
        *self == Post::default()
    }

    /// The program after the post-processor. A script is never run here: the caller asks [`Post::script_state`] first.
    pub fn apply(&self, lines: &[String]) -> Vec<String> {
        let mut out: Vec<String> =
            Vec::with_capacity(lines.len() + self.header.len() + self.footer.len());
        out.extend(self.header.iter().cloned());
        let mut n = self.numbers.map(|(s, _)| s);
        for raw in lines.iter().take(MAX_PROGRAM_LINES) {
            let line = self.line(raw);
            if line.trim().is_empty() {
                continue;
            }
            match (&mut n, self.numbers) {
                (Some(cur), Some((_, step))) => {
                    out.push(format!("N{cur} {line}"));
                    *cur = cur.saturating_add(step);
                }
                _ => out.push(line),
            }
        }
        out.extend(self.footer.iter().cloned());
        out
    }

    fn line(&self, raw: &str) -> String {
        let (code, comment) = split_comment(raw);
        let mut words: Vec<String> = Vec::new();
        for w in words_of(&code) {
            let head = w.to_ascii_uppercase();
            if self.drop.contains(&head) {
                continue;
            }
            let mut w = match self.replace.iter().find(|(a, _)| *a == head) {
                Some((_, b)) => b.clone(),
                None => w,
            };
            if let Some(d) = self.decimals {
                w = rounded(&w, d);
            }
            if self.uppercase {
                w = w.to_ascii_uppercase();
            }
            words.push(w);
        }
        let mut s = words.join(" ");
        if !self.strip_comments && !comment.is_empty() {
            if !s.is_empty() {
                s.push(' ');
            }
            s.push_str(comment.trim());
        }
        s
    }

    /// What a post-processor that names a script does with the settings: `Ok(())` when it names none, the label to say when it does.
    pub fn script_state(&self, scripts_on: bool) -> Result<(), crate::notes::Label> {
        match (&self.script, scripts_on) {
            (None, _) => Ok(()),
            (Some(_), false) => Err(crate::notes::Label::ScriptsOff),
            (Some(_), true) => Err(crate::notes::Label::NoEngine),
        }
    }
}

fn is_word(s: &str) -> bool {
    let mut c = s.chars();
    matches!(c.next(), Some(l) if l.is_ascii_alphabetic())
        && s.len() <= 12
        && s.chars().skip(1).all(|ch| ch.is_ascii_digit() || ch == '.')
        && s.len() > 1
}

/// The code of a line and its comments (a `;` to the end, or text in parentheses). The words after a comment in parentheses are code, and a `;` inside one is part of it.
fn split_comment(line: &str) -> (String, String) {
    let (mut code, mut comments) = (String::new(), String::new());
    let mut in_comment = false;
    for (at, c) in line.char_indices() {
        match c {
            ';' if !in_comment => {
                comments.push_str(line.get(at..).unwrap_or(""));
                break;
            }
            '(' => {
                in_comment = true;
                comments.push(c);
            }
            ')' if in_comment => {
                in_comment = false;
                comments.push(c);
            }
            _ if in_comment => comments.push(c),
            _ => code.push(c),
        }
    }
    (code, comments)
}

/// The words of the code of a line: a letter and what follows it up to the next letter (`G1X5Y-2.5` is `G1`, `X5`, `Y-2.5`).
fn words_of(code: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for c in code.chars() {
        if c.is_whitespace() {
            continue;
        }
        if c.is_ascii_alphabetic() || out.is_empty() {
            out.push(c.to_string());
        } else if let Some(last) = out.last_mut() {
            last.push(c);
        }
    }
    out
}

fn rounded(word: &str, decimals: usize) -> String {
    let mut c = word.chars();
    let Some(l) = c.next() else {
        return String::new();
    };
    if !"XYZABCIJKR".contains(l.to_ascii_uppercase()) {
        return word.to_string();
    }
    match c.as_str().parse::<f64>() {
        Ok(v) if v.is_finite() => {
            let t = format!("{v:.decimals$}");
            let t = if t.contains('.') {
                t.trim_end_matches('0').trim_end_matches('.').to_string()
            } else {
                t
            };
            let t = if t == "-0" || t.is_empty() {
                "0".to_string()
            } else {
                t
            };
            format!("{l}{t}")
        }
        _ => word.to_string(),
    }
}

/// Where the post-processor `name` is: next to the settings, as `phoenix-cnc-post-NAME.txt`.
pub fn post_path(settings: &Path, name: &str) -> Option<PathBuf> {
    // a name makes a file name: letters, digits, `-` and `_` only, so that it can never leave the folder of the settings
    let clean = name.trim();
    if clean.is_empty()
        || clean.len() > 40
        || !clean
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return None;
    }
    let stem = settings.file_stem().map_or_else(
        || "phoenix-cnc".to_string(),
        |s| s.to_string_lossy().to_string(),
    );
    Some(settings.with_file_name(format!("{stem}-post-{clean}.txt")))
}

/// Reads a post-processor from a path (the name of a file of the user or one beside the settings); `None` when it is not there, too big or
/// cannot be read.
pub fn read(path: &Path) -> Option<(Post, Vec<Problem>)> {
    let m = std::fs::metadata(path).ok()?;
    if !m.is_file() || m.len() > MAX_FILE_BYTES {
        return None;
    }
    let b = std::fs::read(path).ok()?;
    Some(Post::parse(&String::from_utf8_lossy(&b)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notes::Label;

    fn lines(t: &str) -> Vec<String> {
        t.lines().map(str::to_string).collect()
    }

    #[test]
    fn a_post_processor_with_no_rules_changes_nothing_but_the_spacing() {
        let (p, problems) = Post::parse("# nothing\n");
        assert!(p.is_empty() && problems.is_empty());
        let out = p.apply(&lines("G1 X5.5000 Y2 F100\nM3 S1000 (spindle)\n\n; note"));
        assert_eq!(
            out,
            vec!["G1 X5.5000 Y2 F100", "M3 S1000 (spindle)", "; note"]
        );
    }

    #[test]
    fn header_footer_numbers_and_word_rules_are_applied_in_order() {
        let text = "header = G21 G90\nheader = (made by the post)\nfooter = M5\nfooter = M30\n\
replace = G0 -> G00\nreplace = g1 -> G01\ndrop = M6\nline_numbers = on\nline_number_start = 100\nline_number_step = 5\n\
decimals = 2\ncomments = strip\ncase = upper\n";
        let (p, problems) = Post::parse(text);
        assert!(problems.is_empty(), "{problems:?}");
        let out = p.apply(&lines(
            "g0 x1.23456 y-0.001\ng1x5 f100 (cut)\nT1 M6\nm3 s500",
        ));
        assert_eq!(
            out,
            vec![
                "G21 G90",
                "(made by the post)",
                "N100 G00 X1.23 Y0",
                "N105 G01 X5 F100",
                "N110 T1",
                "N115 M3 S500",
                "M5",
                "M30",
            ]
        );
    }

    #[test]
    fn the_comments_are_kept_unless_stripped_and_a_line_left_empty_is_dropped() {
        let (p, _) = Post::parse("drop = M8\n");
        let out = p.apply(&lines("M8 (coolant)\nM8\nG4 P1"));
        assert_eq!(out, vec!["(coolant)", "G4 P1"]);
        let (s, _) = Post::parse("drop = M8\ncomments = strip\n");
        assert_eq!(s.apply(&lines("M8 (coolant)\nG4 P1")), vec!["G4 P1"]);
    }

    #[test]
    fn the_lines_that_cannot_be_used_are_named_and_left_out() {
        let (p, problems) = Post::parse(
            "decimals = 9\nreplace = G0\ndrop = XX\nunknown = 1\nline_numbers = maybe\nno equals\ncomments = hide\nline_number_step = 0\nscript =\n",
        );
        assert!(p.is_empty());
        let lines: Vec<usize> = problems.iter().map(|p| p.line).collect();
        assert_eq!(lines, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn the_script_runs_only_with_the_switch_and_this_build_has_no_engine() {
        let (p, problems) = Post::parse("header = G21\nscript = tidy.script\n");
        assert!(problems.is_empty());
        // the declarative rules work whatever the switch says
        assert_eq!(p.apply(&lines("G1 X1")), vec!["G21", "G1 X1"]);
        assert_eq!(p.script_state(false), Err(Label::ScriptsOff));
        assert_eq!(p.script_state(true), Err(Label::NoEngine));
        let (none, _) = Post::parse("header = G21\n");
        assert_eq!(none.script_state(false), Ok(()));
    }

    #[test]
    fn words_are_cut_at_the_letters_and_odd_text_does_not_panic() {
        assert_eq!(words_of("G1X5Y-2.5"), vec!["G1", "X5", "Y-2.5"]);
        assert_eq!(words_of("  12 G1"), vec!["12", "G1"]);
        assert_eq!(rounded("X1.5e", 2), "X1.5e");
        assert_eq!(rounded("Xabc", 2), "Xabc");
        assert_eq!(rounded("F100.123", 1), "F100.123");
        let (p, _) = Post::parse("decimals = 3\nline_numbers = on\ncase = upper\n");
        for odd in ["", "(", ";", "ü ö", "G1 X\u{0}", "X--5", "((", "N", "1e400"] {
            let _ = p.apply(&[odd.to_string()]);
        }
        // the number of a line cannot overflow
        let (big, _) =
            Post::parse("line_numbers = on\nline_number_start = 99999\nline_number_step = 1000\n");
        let out = big.apply(&lines(&"G1 X1\n".repeat(3)));
        assert_eq!(out.len(), 3);
    }

    #[test]
    fn a_post_processor_is_found_beside_the_settings_by_its_name_and_a_big_file_is_refused() {
        let settings = Path::new("some").join("dir").join("phoenix-cnc.txt");
        assert_eq!(
            post_path(&settings, "grbl-simple"),
            Some(
                Path::new("some")
                    .join("dir")
                    .join("phoenix-cnc-post-grbl-simple.txt")
            )
        );
        assert_eq!(post_path(&settings, "../../etc"), None);
        assert_eq!(post_path(&settings, ""), None);
        assert_eq!(post_path(&settings, "a/b"), None);
        assert_eq!(post_path(&settings, "a b"), None);
        let dir = std::env::temp_dir().join(format!("cnc_post_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("p.txt");
        std::fs::write(&f, "header = G21\n").unwrap();
        assert_eq!(read(&f).unwrap().0.header, vec!["G21".to_string()]);
        assert!(read(&dir.join("none.txt")).is_none());
        std::fs::write(&f, "#".repeat(MAX_FILE_BYTES as usize + 1)).unwrap();
        assert!(read(&f).is_none());
        let _ = std::fs::remove_dir_all(&dir);
        // the example of the repository is read without a problem
        let example = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("examples")
                .join("post.txt"),
        )
        .unwrap();
        let (p, problems) = Post::parse(&example);
        assert!(problems.is_empty(), "{problems:?}");
        assert!(!p.is_empty());
    }
}
