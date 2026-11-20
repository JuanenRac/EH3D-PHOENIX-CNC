//! The check of the format of a program for the controller it is made for, as the output of a post-processor is held to: the length of a line, the
//! number of decimals, the `%` lines, the codes the controller lists, two codes of the same group on one line, a word given twice in a line and the
//! end of the program. A profile is a short list of what a kind of controller takes (`grbl`, `full`, `generic`); the lists are in this file
//! and say what the documentation of GRBL lists and what a controller with canned cycles, cutter compensation and tool changes takes, so a code a profile
//! does not list is a reason to read the line, not a proof that the
//! controller refuses it. Only a reading: nothing is sent and nothing is changed.
//!
//! Besides the three profiles of the program the user can write one in a file (`key = value`, `#` and `;` start a comment):
//!
//! ```text
//! name = my-controller
//! base = grbl            # grbl, full or generic: the codes it lists start from this one (generic when left out)
//! max_line = 60          # 0: no limit
//! max_decimals = 3       # 0: no limit
//! percent = no           # a line with only % is taken
//! needs_end = yes        # the program must end with M2 or M30
//! allow_g = 5.1, 64      # G codes it lists besides the base's
//! deny_g = 28.1          # G codes it does not list, whatever the base says (this wins over allow_g)
//! allow_m = 6
//! deny_m = 7, 8
//! ```
//!
//! A file is found by its path, or by its name next to the settings file (`phoenix-cnc-format-NAME.txt`); see [`resolve`].

use std::path::{Path, PathBuf};

use crate::gcode::words;
use crate::profiles::clean_name;
use crate::progcheck::{Finding, Level};
use crate::words::Msg;

/// A kind of controller and what its format asks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub name: String,
    /// The most characters of a line once its spaces and its comments are taken out (0: no limit).
    pub max_line: usize,
    /// The most decimals of a number (0: no limit).
    pub max_decimals: usize,
    /// A line with only `%` is taken.
    pub percent: bool,
    /// The program must end with `M2` or `M30`.
    pub needs_end: bool,
    dialect: Dialect,
    /// Codes a profile of the user lists besides (`allow_g`, `allow_m`) or does not list (`deny_g`, `deny_m`) whatever its base says; the G codes in tenths.
    extra_g: Vec<i32>,
    deny_g: Vec<i32>,
    extra_m: Vec<i32>,
    deny_m: Vec<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dialect {
    Grbl,
    Full,
    Generic,
}

/// The names of the profiles, in the order they are listed.
pub const NAMES: [&str; 3] = ["grbl", "full", "generic"];

/// The G codes the GRBL documentation lists, in tenths (`G38.2` is 382).
const GRBL_G: &[i32] = &[
    0, 10, 20, 30, 40, 100, 170, 180, 190, 200, 210, 280, 281, 300, 301, 382, 383, 384, 385, 400,
    431, 490, 530, 540, 550, 560, 570, 580, 590, 610, 800, 900, 910, 911, 920, 921, 930, 940,
];
/// The M codes the GRBL documentation lists.
const GRBL_M: &[i32] = &[0, 1, 2, 3, 4, 5, 7, 8, 9, 30, 56];
/// The G codes a controller with canned cycles, cutter compensation and tool changes takes besides those of GRBL.
const FULL_EXTRA_G: &[i32] = &[
    50, 51, 330, 331, 410, 420, 430, 500, 541, 620, 640, 730, 760, 810, 820, 830, 840, 850, 860,
    870, 880, 890, 950, 980, 990,
];

impl Profile {
    /// The profile with this name (the case does not matter), if there is one.
    pub fn named(name: &str) -> Option<Profile> {
        match name.trim().to_ascii_lowercase().as_str() {
            "grbl" => Some(Profile {
                name: "grbl".to_string(),
                max_line: 79,
                max_decimals: 4,
                percent: false,
                needs_end: true,
                dialect: Dialect::Grbl,
                extra_g: Vec::new(),
                deny_g: Vec::new(),
                extra_m: Vec::new(),
                deny_m: Vec::new(),
            }),
            "full" => Some(Profile {
                name: "full".to_string(),
                max_line: 255,
                max_decimals: 6,
                percent: true,
                needs_end: true,
                dialect: Dialect::Full,
                extra_g: Vec::new(),
                deny_g: Vec::new(),
                extra_m: Vec::new(),
                deny_m: Vec::new(),
            }),
            "generic" => Some(Profile {
                name: "generic".to_string(),
                max_line: 0,
                max_decimals: 0,
                percent: true,
                needs_end: false,
                dialect: Dialect::Generic,
                extra_g: Vec::new(),
                deny_g: Vec::new(),
                extra_m: Vec::new(),
                deny_m: Vec::new(),
            }),
            _ => None,
        }
    }

    fn lists_g(&self, tenths: i32) -> bool {
        if self.deny_g.contains(&tenths) {
            return false;
        }
        if self.extra_g.contains(&tenths) {
            return true;
        }
        match self.dialect {
            Dialect::Generic => true,
            Dialect::Grbl => GRBL_G.contains(&tenths),
            Dialect::Full => GRBL_G.contains(&tenths) || FULL_EXTRA_G.contains(&tenths),
        }
    }

    fn lists_m(&self, code: i32) -> bool {
        if self.deny_m.contains(&code) {
            return false;
        }
        if self.extra_m.contains(&code) {
            return true;
        }
        match self.dialect {
            Dialect::Generic => true,
            Dialect::Grbl => GRBL_M.contains(&code),
            Dialect::Full => {
                (0..=9).contains(&code)
                    || code == 30
                    || (48..=53).contains(&code)
                    || (60..=68).contains(&code)
                    || (70..=73).contains(&code)
                    || code == 98
                    || code == 99
                    || (100..=199).contains(&code)
            }
        }
    }
}

/// The biggest file of a profile that is read, in bytes.
pub const MAX_PROFILE_BYTES: u64 = 16 * 1024;

fn bad(line: usize, msg: Msg, args: Vec<String>) -> Finding {
    Finding {
        line: Some(line),
        level: Level::Danger,
        msg,
        args,
    }
}

fn yes_no(v: &str) -> Option<bool> {
    match v.trim().to_ascii_lowercase().as_str() {
        "yes" | "on" | "true" | "1" => Some(true),
        "no" | "off" | "false" | "0" => Some(false),
        _ => None,
    }
}

/// A list of codes written `5.1, 64` (a leading `G` or `M` is allowed): the G codes in tenths, the M codes as they are. Gives the item that is not a code.
fn code_list(v: &str, g: bool) -> Result<Vec<i32>, String> {
    let mut out = Vec::new();
    if v.trim().is_empty() {
        return Ok(out);
    }
    for item in v.split(',') {
        let t = item.trim();
        let digits = t.trim_start_matches(['G', 'g', 'M', 'm']);
        let n = digits
            .parse::<f64>()
            .ok()
            .filter(|n| n.is_finite())
            .ok_or_else(|| t.to_string())?;
        let code = if g {
            let tenths = (n * 10.0).round();
            if (n * 10.0 - tenths).abs() > 1e-6 || !(0.0..=9999.0).contains(&tenths) {
                return Err(t.to_string());
            }
            tenths as i32
        } else {
            if n.fract() != 0.0 || !(0.0..=999.0).contains(&n) {
                return Err(t.to_string());
            }
            n as i32
        };
        if !out.contains(&code) {
            out.push(code);
        }
    }
    Ok(out)
}

impl Profile {
    /// A profile written in the text of a file (see the top of this module). `default_name` names it when the text does not. Every line that cannot be
    /// used is named, with its number, in the error.
    pub fn from_text(text: &str, default_name: &str) -> Result<Profile, Vec<Finding>> {
        let mut errors: Vec<Finding> = Vec::new();
        let mut entries: Vec<(usize, String, String)> = Vec::new();
        for (n, raw) in text.trim_start_matches('\u{feff}').lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            match line.split_once('=') {
                Some((k, v)) => {
                    entries.push((n, k.trim().to_ascii_lowercase(), v.trim().to_string()))
                }
                None => errors.push(bad(n, Msg::FmtProfileSyntax, Vec::new())),
            }
        }
        let generic = || Profile::named("generic").expect("the generic profile is there");
        let base = entries.iter().rev().find(|(_, k, _)| k == "base");
        let mut p = match base {
            None => generic(),
            Some((n, _, v)) => Profile::named(v).unwrap_or_else(|| {
                errors.push(bad(*n, Msg::FmtProfileValue, vec!["base".to_string()]));
                generic()
            }),
        };
        p.name = clean_name(default_name).unwrap_or_else(|| "custom".to_string());
        for (n, k, v) in &entries {
            let wrong = |errors: &mut Vec<Finding>| {
                errors.push(bad(*n, Msg::FmtProfileValue, vec![k.clone()]));
            };
            match k.as_str() {
                "base" => {}
                "name" => match clean_name(v) {
                    Some(name) => p.name = name,
                    None => wrong(&mut errors),
                },
                "max_line" => match v.parse::<usize>() {
                    Ok(x) if x <= 100_000 => p.max_line = x,
                    _ => wrong(&mut errors),
                },
                "max_decimals" => match v.parse::<usize>() {
                    Ok(x) if x <= 15 => p.max_decimals = x,
                    _ => wrong(&mut errors),
                },
                "percent" => match yes_no(v) {
                    Some(b) => p.percent = b,
                    None => wrong(&mut errors),
                },
                "needs_end" => match yes_no(v) {
                    Some(b) => p.needs_end = b,
                    None => wrong(&mut errors),
                },
                "allow_g" | "deny_g" | "allow_m" | "deny_m" => {
                    let g = k.ends_with('g');
                    match code_list(v, g) {
                        Ok(list) => match k.as_str() {
                            "allow_g" => p.extra_g = list,
                            "deny_g" => p.deny_g = list,
                            "allow_m" => p.extra_m = list,
                            _ => p.deny_m = list,
                        },
                        Err(item) => errors.push(bad(*n, Msg::FmtProfileCode, vec![item])),
                    }
                }
                other => errors.push(bad(*n, Msg::FmtProfileKey, vec![other.to_string()])),
            }
        }
        if errors.is_empty() {
            Ok(p)
        } else {
            errors.sort_by_key(|f| f.line);
            Err(errors)
        }
    }

    /// The profile in a file. A file that is too big, is not a file or cannot be read is one error with no line.
    pub fn load(path: &Path) -> Result<Profile, Vec<Finding>> {
        let stem = path
            .file_stem()
            .map_or_else(String::new, |s| s.to_string_lossy().to_string());
        Profile::load_named(path, &stem)
    }

    /// As `load`, with the name the profile gets when the text does not give one.
    pub fn load_named(path: &Path, default_name: &str) -> Result<Profile, Vec<Finding>> {
        let unreadable = || {
            vec![Finding {
                line: None,
                level: Level::Danger,
                msg: Msg::FmtProfileUnreadable,
                args: vec![path.display().to_string()],
            }]
        };
        let meta = std::fs::metadata(path).map_err(|_| unreadable())?;
        if !meta.is_file() || meta.len() > MAX_PROFILE_BYTES {
            return Err(unreadable());
        }
        let bytes = std::fs::read(path).map_err(|_| unreadable())?;
        Profile::from_text(&String::from_utf8_lossy(&bytes), default_name)
    }
}

/// Where the profile of the user named `name` is: next to the settings, with the name of the settings file in front (`phoenix-cnc-format-NAME.txt`). Only
/// a name of letters, digits, `-` and `_` has a place there.
pub fn profile_path(settings: &Path, name: &str) -> Option<PathBuf> {
    let ok = !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !ok {
        return None;
    }
    let stem = settings.file_stem().map_or_else(
        || "phoenix-cnc".to_string(),
        |s| s.to_string_lossy().to_string(),
    );
    Some(settings.with_file_name(format!("{stem}-format-{name}.txt")))
}

/// What the name given to `--format` came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found {
    Profile(Profile),
    /// It is neither one of the three profiles nor a file.
    NotFound,
    /// It is a file that cannot be used: what is wrong with it.
    Invalid(Vec<Finding>),
}

/// The profile a name stands for: one of the three of the program (the case does not matter), else the file of that name next to the settings file
/// `settings` (when it is given), else the path as it is written.
pub fn resolve(name: &str, settings: Option<&Path>) -> Found {
    if let Some(p) = Profile::named(name) {
        return Found::Profile(p);
    }
    let t = name.trim();
    let beside = settings
        .and_then(|s| profile_path(s, t))
        .filter(|p| p.is_file());
    let path = beside.clone().unwrap_or_else(|| PathBuf::from(t));
    if t.is_empty() || !path.is_file() {
        return Found::NotFound;
    }
    let loaded = if beside.is_some() {
        Profile::load_named(&path, t)
    } else {
        Profile::load(&path)
    };
    match loaded {
        Ok(p) => Found::Profile(p),
        Err(findings) => Found::Invalid(findings),
    }
}

/// The group of a G code that cannot be twice on one line, by name.
fn g_group(tenths: i32) -> Option<&'static str> {
    match tenths {
        0 | 10 | 20 | 30 | 330 | 382..=385 | 730 | 760 | 800 | 810..=890 => Some("motion"),
        900 | 910 => Some("distance"),
        200 | 210 => Some("units"),
        170 | 180 | 190 => Some("plane"),
        400 | 410 | 420 => Some("compensation"),
        540..=590 => Some("system"),
        930 | 940 | 950 => Some("feed"),
        _ => None,
    }
}

fn g_name(tenths: i32) -> String {
    if tenths % 10 == 0 {
        format!("G{}", tenths / 10)
    } else {
        format!("G{}.{}", tenths / 10, tenths % 10)
    }
}

/// The characters of a line that count against the buffer: no spaces and no comments.
fn stripped_len(line: &str) -> usize {
    let mut count = 0;
    let mut paren = false;
    for c in line.chars() {
        match c {
            ';' if !paren => break,
            '(' => paren = true,
            ')' if paren => paren = false,
            c if paren || c.is_whitespace() => {}
            _ => count += 1,
        }
    }
    count
}

/// The longest run of decimals in the numbers of a line (outside the comments), and the word it belongs to.
fn most_decimals(line: &str) -> (usize, String) {
    let mut best = (0usize, String::new());
    let mut paren = false;
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            ';' if !paren => break,
            '(' => paren = true,
            ')' if paren => paren = false,
            c if !paren && c.is_ascii_alphabetic() => {
                let mut j = i + 1;
                while j < chars.len()
                    && (chars[j].is_ascii_digit() || matches!(chars[j], '.' | '-' | '+'))
                {
                    j += 1;
                }
                let number: String = chars[i + 1..j].iter().collect();
                if let Some((_, decimals)) = number.split_once('.') {
                    if decimals.len() > best.0 {
                        best = (
                            decimals.len(),
                            format!("{}{number}", c.to_ascii_uppercase()),
                        );
                    }
                }
                i = j.max(i + 1);
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    best
}

/// What looks wrong in the format of the program for the profile, by line (the remarks about the whole program last).
pub fn check(lines: &[String], profile: &Profile) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut push = |line: Option<usize>, level: Level, msg: Msg, args: Vec<String>| {
        out.push(Finding {
            line,
            level,
            msg,
            args,
        });
    };
    let mut last_code: Option<usize> = None;
    for (n, raw) in lines.iter().enumerate() {
        if raw.trim() == "%" {
            if !profile.percent {
                push(
                    Some(n),
                    Level::Info,
                    Msg::FmtPercent,
                    vec![profile.name.to_string()],
                );
            }
            continue;
        }
        let w = words(raw);
        if profile.max_line > 0 {
            let len = stripped_len(raw);
            if len > profile.max_line {
                push(
                    Some(n),
                    Level::Warning,
                    Msg::FmtLong,
                    vec![len.to_string(), profile.max_line.to_string()],
                );
            }
        }
        if profile.max_decimals > 0 {
            let (decimals, word) = most_decimals(raw);
            if decimals > profile.max_decimals {
                push(
                    Some(n),
                    Level::Info,
                    Msg::FmtDecimals,
                    vec![word, profile.max_decimals.to_string()],
                );
            }
        }
        if w.is_empty() {
            continue;
        }
        last_code = Some(n);
        // the codes the profile lists, and two of a group on the line
        let mut groups: Vec<(&'static str, i32)> = Vec::new();
        let mut spindle: Option<i32> = None;
        for (letter, v) in &w {
            let code = (*v * 10.0).round() as i32;
            match letter {
                'G' => {
                    if !profile.lists_g(code) {
                        push(
                            Some(n),
                            Level::Warning,
                            Msg::FmtCode,
                            vec![profile.name.to_string(), g_name(code)],
                        );
                    }
                    if let Some(g) = g_group(code) {
                        if let Some((_, other)) =
                            groups.iter().find(|(name, o)| *name == g && *o != code)
                        {
                            push(
                                Some(n),
                                Level::Warning,
                                Msg::FmtGroup,
                                vec![g_name(*other), g_name(code)],
                            );
                        }
                        groups.push((g, code));
                    }
                }
                'M' => {
                    let m = *v as i32;
                    if !profile.lists_m(m) {
                        push(
                            Some(n),
                            Level::Warning,
                            Msg::FmtCode,
                            vec![profile.name.to_string(), format!("M{m}")],
                        );
                    }
                    if (3..=5).contains(&m) {
                        if let Some(other) = spindle.filter(|o| *o != m) {
                            push(
                                Some(n),
                                Level::Warning,
                                Msg::FmtGroup,
                                vec![format!("M{other}"), format!("M{m}")],
                            );
                        }
                        spindle = Some(m);
                    }
                }
                _ => {}
            }
        }
        // a word that is twice in the line (G and M may be: they are different codes)
        let mut seen: Vec<char> = Vec::new();
        let mut said: Vec<char> = Vec::new();
        for (letter, _) in &w {
            if matches!(letter, 'G' | 'M') {
                continue;
            }
            if seen.contains(letter) && !said.contains(letter) {
                said.push(*letter);
                push(
                    Some(n),
                    Level::Warning,
                    Msg::FmtDupWord,
                    vec![letter.to_string()],
                );
            }
            seen.push(*letter);
        }
    }
    if profile.needs_end {
        let ends = last_code.is_some_and(|n| {
            words(&lines[n])
                .iter()
                .any(|(l, v)| *l == 'M' && (*v == 2.0 || *v == 30.0))
        });
        if last_code.is_some() && !ends {
            push(
                None,
                Level::Info,
                Msg::FmtNoEnd,
                vec![profile.name.to_string()],
            );
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(str::to_string).collect()
    }

    fn grbl() -> Profile {
        Profile::named("grbl").unwrap()
    }

    fn msgs(f: &[Finding]) -> Vec<(Option<usize>, Msg)> {
        f.iter().map(|x| (x.line, x.msg)).collect()
    }

    #[test]
    fn the_profiles_are_found_by_name_whatever_the_case() {
        for name in NAMES {
            assert_eq!(Profile::named(name).unwrap().name, name);
            assert!(Profile::named(&name.to_uppercase()).is_some());
        }
        assert!(Profile::named(" Grbl ").is_some());
        assert!(Profile::named("nosuchprofile").is_none());
        assert!(Profile::named("").is_none());
    }

    #[test]
    fn a_program_in_the_format_of_the_controller_has_nothing_to_say() {
        let p = lines(
            "G21 G90 G17\nM3 S12000\nG0 X10.5 Y10\nG1 Z-1.25 F200\nG1 X50 F600\nG0 Z5\nM5\nM30",
        );
        for name in NAMES {
            assert!(
                check(&p, &Profile::named(name).unwrap()).is_empty(),
                "{name}"
            );
        }
    }

    #[test]
    fn a_line_longer_than_the_buffer_counts_without_its_spaces_and_comments() {
        let spaced = format!(
            "G1 X1{}Y2 (a comment that is not counted at all, {})",
            " ".repeat(100),
            "x".repeat(100)
        );
        let f = check(&lines(&format!("G90\n{spaced}\nM30")), &grbl());
        assert!(f.is_empty(), "{f:?}");
        let longer = format!("G1 X{}", "1".repeat(90));
        let g = check(&lines(&format!("G90\n{longer}\nM30")), &grbl());
        assert_eq!(msgs(&g), [(Some(1), Msg::FmtLong)], "{g:?}");
        assert_eq!(g[0].args, ["93", "79"]);
        // the full profile takes it
        let full = Profile::named("full").unwrap();
        assert!(check(&lines(&format!("G90\n{longer}\nM30")), &full).is_empty());
    }

    #[test]
    fn more_decimals_than_the_profile_asks_are_named_with_the_word() {
        let f = check(&lines("G90\nG1 X10.123456 Y2.5 F600\nM30"), &grbl());
        assert_eq!(msgs(&f), [(Some(1), Msg::FmtDecimals)]);
        assert_eq!(f[0].args, ["X10.123456", "4"]);
        assert_eq!(f[0].level, Level::Info);
        // five decimals are fine for the full profile and nothing is counted in a comment
        let ok = lines("G90\nG1 X10.12345 (Y1.123456789) ; Z0.1234567\nM30");
        assert!(check(&ok, &Profile::named("full").unwrap()).is_empty());
        assert!(check(&ok, &Profile::named("generic").unwrap()).is_empty());
    }

    #[test]
    fn a_code_that_the_profile_does_not_list_is_named() {
        let p = lines("G90\nG41 D1 G1 X5 F100\nG83 Z-5 R2 Q1\nM6 T1\nG1 X9\nM30");
        let f = check(&p, &grbl());
        let named: Vec<String> = f
            .iter()
            .filter(|x| x.msg == Msg::FmtCode)
            .map(|x| x.args[1].clone())
            .collect();
        assert_eq!(named, ["G41", "G83", "M6"]);
        assert_eq!(f[0].args[0], "grbl");
        // the full profile lists the cycles, the compensation and the tool change
        assert!(check(&p, &Profile::named("full").unwrap())
            .iter()
            .all(|x| x.msg != Msg::FmtCode));
        // a code nobody lists is named by the full profile as well, and never by the generic profile
        let odd = lines("G90\nG200 X1\nM250\nM30");
        assert_eq!(
            check(&odd, &Profile::named("full").unwrap())
                .iter()
                .filter(|x| x.msg == Msg::FmtCode)
                .count(),
            2
        );
        assert!(check(&odd, &Profile::named("generic").unwrap()).is_empty());
        // the dotted codes keep their decimal
        let g = check(&lines("G90\nG38.6 Z-5 F50\nG43.1 Z1\nM30"), &grbl());
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].args[1], "G38.6");
    }

    #[test]
    fn two_codes_of_a_group_on_one_line_are_named_and_the_same_code_twice_is_not() {
        let f = check(
            &lines("G90\nG0 G1 X5\nG20 G21\nG90 G91\nG17 G18 G19\nG1 G1 X1\nM3 M5\nM30"),
            &Profile::named("generic").unwrap(),
        );
        let at: Vec<(Option<usize>, Vec<String>)> =
            f.iter().map(|x| (x.line, x.args.clone())).collect();
        assert_eq!(
            at,
            [
                (Some(1), vec!["G0".to_string(), "G1".to_string()]),
                (Some(2), vec!["G20".to_string(), "G21".to_string()]),
                (Some(3), vec!["G90".to_string(), "G91".to_string()]),
                (Some(4), vec!["G17".to_string(), "G18".to_string()]),
                (Some(4), vec!["G17".to_string(), "G19".to_string()]),
                (Some(6), vec!["M3".to_string(), "M5".to_string()]),
            ]
        );
        // a motion and a work system, a dwell and a motion are not the same group
        assert!(check(
            &lines("G54 G1 X5 F100\nG4 P1\nG53 G0 Z0"),
            &Profile::named("generic").unwrap()
        )
        .is_empty());
    }

    #[test]
    fn a_word_given_twice_is_named_once_and_g_and_m_may_repeat() {
        let f = check(
            &lines("G1 X1 X2 X3 Y5 F10 F20\nG90 G54 M3 M8"),
            &Profile::named("generic").unwrap(),
        );
        let at: Vec<(Option<usize>, Msg, String)> = f
            .iter()
            .map(|x| (x.line, x.msg, x.args[0].clone()))
            .collect();
        assert_eq!(
            at,
            [
                (Some(0), Msg::FmtDupWord, "X".to_string()),
                (Some(0), Msg::FmtDupWord, "F".to_string())
            ]
        );
    }

    #[test]
    fn percent_lines_and_the_end_of_the_program_follow_the_profile() {
        let p = lines("%\nG21 G90\nG1 X5 F100\n%");
        let f = check(&p, &grbl());
        assert_eq!(
            msgs(&f),
            [
                (Some(0), Msg::FmtPercent),
                (Some(3), Msg::FmtPercent),
                (None, Msg::FmtNoEnd)
            ]
        );
        assert_eq!(f[2].level, Level::Info);
        // the full profile takes the % and still asks for the end; the generic profile asks for nothing
        let l = check(&p, &Profile::named("full").unwrap());
        assert_eq!(msgs(&l), [(None, Msg::FmtNoEnd)]);
        assert!(check(&p, &Profile::named("generic").unwrap()).is_empty());
        // an end with M2, comments and blank lines after it
        assert!(check(&lines("G90\nG1 X5 F100\nM2\n\n(done)\n"), &grbl()).is_empty());
        // a program with no code has no end to miss
        assert!(check(&lines("(only a comment)\n\n"), &grbl()).is_empty());
        assert!(check(&[], &grbl()).is_empty());
    }

    #[test]
    fn every_finding_is_told_in_the_eight_languages_with_its_holes_filled() {
        let long = format!("G1 G0 X10.123456 X2 {}", "Y1.1 ".repeat(20));
        let f = check(&lines(&format!("%\nM6\n{long}\nG1 X1")), &grbl());
        let kinds: Vec<Msg> = f.iter().map(|x| x.msg).collect();
        for m in [
            Msg::FmtPercent,
            Msg::FmtLong,
            Msg::FmtDecimals,
            Msg::FmtCode,
            Msg::FmtGroup,
            Msg::FmtDupWord,
            Msg::FmtNoEnd,
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
    fn the_helpers_count_what_the_controller_counts() {
        assert_eq!(stripped_len("G1 X1 (not this) Y2 ; nor this"), 6);
        assert_eq!(stripped_len(""), 0);
        assert_eq!(
            most_decimals("G1 X1.25 Y-0.123 (Z9.99999)"),
            (3, "Y-0.123".to_string())
        );
        assert_eq!(most_decimals("G1 X10").0, 0);
        assert_eq!(g_name(431), "G43.1");
        assert_eq!(g_name(540), "G54");
    }

    const MINE: &str = "# a controller of my own\nname = my-controller\nbase = grbl\nmax_line = 60\nmax_decimals = 3\npercent = yes\nneeds_end = no\nallow_g = 41, G64, 5.1\ndeny_g = 28.1\nallow_m = 6\ndeny_m = 7, M8\n";

    /// A folder of its own in the temporary one, removed when the test is over.
    struct Folder(PathBuf);

    impl Folder {
        fn new(tag: &str) -> Folder {
            let d = std::env::temp_dir().join(format!(
                "phoenix-cnc-postcheck-{tag}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&d);
            std::fs::create_dir_all(&d).unwrap();
            Folder(d)
        }
    }

    impl Drop for Folder {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_profile_of_the_user_is_read_from_its_text() {
        let p = Profile::from_text(MINE, "file-name").unwrap();
        assert_eq!(p.name, "my-controller");
        assert_eq!((p.max_line, p.max_decimals), (60, 3));
        assert!(p.percent && !p.needs_end);
        // the base is the one named, wherever the line is
        let late = Profile::from_text("max_line = 10\nbase = full\n", "x").unwrap();
        assert_eq!((late.max_line, late.max_decimals), (10, 6));
        assert!(late.lists_g(810));
        // with no base the profile lists everything and has no limits; the name is the one of the file
        let bare = Profile::from_text("# nothing\n", "plain").unwrap();
        assert_eq!(bare.name, "plain");
        assert_eq!(
            (bare.max_line, bare.max_decimals, bare.needs_end),
            (0, 0, false)
        );
        assert!(bare.lists_g(1234) && bare.lists_m(999));
        // the case of the keys and the byte order mark do not matter
        let loud = Profile::from_text("\u{feff}MAX_LINE = 33\nPercent = No\n", "x").unwrap();
        assert_eq!((loud.max_line, loud.percent), (33, false));
    }

    #[test]
    fn the_codes_a_profile_adds_or_takes_away_change_what_it_lists() {
        let p = Profile::from_text(MINE, "x").unwrap();
        // GRBL does not list these; the profile does
        assert!(p.lists_g(410) && p.lists_g(640) && p.lists_g(51));
        assert!(p.lists_m(6));
        // GRBL lists these; the profile does not
        assert!(!p.lists_g(281) && !p.lists_m(7) && !p.lists_m(8));
        // the rest is the base's
        assert!(p.lists_g(10) && !p.lists_g(1230));
        // taking a code away wins over adding it
        let both = Profile::from_text("base = grbl\nallow_g = 41\ndeny_g = 41\n", "x").unwrap();
        assert!(!both.lists_g(410));
        // and the check uses it, with the name of the profile in the sentence
        let found = check(&lines("G41 G1 X1 F100\nM7\nM6\nM2"), &p);
        let said: Vec<String> = found.iter().map(|f| f.text("en")).collect();
        assert!(
            said.iter()
                .any(|t| t.contains("M7 is not in the list of my-controller")),
            "{said:?}"
        );
        assert!(
            !said.iter().any(|t| t.contains("G41") || t.contains("M6")),
            "{said:?}"
        );
    }

    #[test]
    fn every_line_of_a_profile_that_cannot_be_used_is_named_with_its_number() {
        let text = "max_line = many\nnot a pair\nflavour = sweet\nbase = nosuch\nallow_g = 41, abc\nallow_m = 6.5\nmax_decimals = 16\npercent = maybe\nname = |\n";
        let errors = Profile::from_text(text, "x").unwrap_err();
        let at: Vec<Option<usize>> = errors.iter().map(|e| e.line).collect();
        assert_eq!(at, (0..=8).map(Some).collect::<Vec<_>>(), "{errors:?}");
        let kinds: Vec<Msg> = errors.iter().map(|e| e.msg).collect();
        assert_eq!(kinds[1], Msg::FmtProfileSyntax);
        assert_eq!(kinds[2], Msg::FmtProfileKey);
        assert_eq!(kinds[4], Msg::FmtProfileCode);
        assert_eq!(
            errors[2].text("en"),
            "Line 3 of the profile: 'flavour' is not a key of a format profile"
        );
        assert_eq!(errors[4].args, vec!["abc".to_string()]);
        // each one is told in the eight languages with its holes filled
        for lang in crate::words::LANGS {
            for e in &errors {
                assert!(!e.text(lang).contains('{'), "{:?} {lang}", e.msg);
            }
        }
        // a G code is a number of tenths, an M code a whole number
        assert!(Profile::from_text("allow_g = 5.15\n", "x").is_err());
        assert!(Profile::from_text("allow_g = -1\n", "x").is_err());
        assert!(Profile::from_text("allow_m = 1000\n", "x").is_err());
        assert!(Profile::from_text("allow_g =\nallow_m =\n", "x").is_ok());
    }

    #[test]
    fn a_profile_is_loaded_from_a_file_and_is_named_after_it_when_the_text_does_not() {
        let d = Folder::new("load");
        let named = d.0.join("lathe.txt");
        std::fs::write(&named, "base = full\nmax_line = 40\n").unwrap();
        let p = Profile::load(&named).unwrap();
        assert_eq!((p.name.as_str(), p.max_line), ("lathe", 40));
        // not there, a folder, and too big are the same error: the file could not be read
        let big = d.0.join("big.txt");
        std::fs::write(&big, "#".repeat(MAX_PROFILE_BYTES as usize + 1)).unwrap();
        for path in [d.0.join("missing.txt"), d.0.clone(), big] {
            let e = Profile::load(&path).unwrap_err();
            assert_eq!(
                (e.len(), e[0].line, e[0].msg),
                (1, None, Msg::FmtProfileUnreadable)
            );
        }
    }

    #[test]
    fn a_name_is_one_of_the_three_a_file_beside_the_settings_or_a_path() {
        let d = Folder::new("resolve");
        let settings = d.0.join("phoenix-cnc.cfg");
        std::fs::write(
            d.0.join("phoenix-cnc-format-shop.txt"),
            "base = grbl\nmax_line = 50\n",
        )
        .unwrap();
        let other = d.0.join("elsewhere.txt");
        std::fs::write(&other, "max_decimals = 2\n").unwrap();
        let broken = d.0.join("broken.txt");
        std::fs::write(&broken, "oops\n").unwrap();
        let get = |n: &str| resolve(n, Some(&settings));
        assert!(matches!(get("GRBL"), Found::Profile(p) if p.name == "grbl"));
        assert!(matches!(get("shop"), Found::Profile(p) if p.name == "shop" && p.max_line == 50));
        assert!(matches!(get(&other.to_string_lossy()), Found::Profile(p) if p.max_decimals == 2));
        assert!(matches!(get(&broken.to_string_lossy()), Found::Invalid(f) if f.len() == 1));
        for nothing in ["nosuch", "", "  ", "..", "no-such-folder/x.txt"] {
            assert_eq!(get(nothing), Found::NotFound, "{nothing:?}");
        }
        // with no settings file only the three and the paths are known
        assert_eq!(resolve("shop", None), Found::NotFound);
        // a name that is not letters, digits, - and _ has no place beside the settings
        for odd in ["", "a b", "a/b", "..", "a.b", &"x".repeat(65)] {
            assert_eq!(profile_path(&settings, odd), None, "{odd:?}");
        }
        assert_eq!(
            profile_path(&settings, "shop")
                .unwrap()
                .file_name()
                .unwrap(),
            "phoenix-cnc-format-shop.txt"
        );
    }

    #[test]
    fn texts_from_a_fixed_seed_never_panic_and_give_a_profile_or_errors_with_lines() {
        let tokens = [
            "name",
            "base",
            "max_line",
            "max_decimals",
            "percent",
            "needs_end",
            "allow_g",
            "deny_g",
            "allow_m",
            "deny_m",
            "=",
            "grbl",
            "full",
            "generic",
            "yes",
            "no",
            "41",
            "5.1",
            "G64",
            "M6",
            "-3",
            "1e9",
            ",",
            "#",
            ";",
            "\n",
            "\n",
            " ",
            "\u{e9}",
            "x",
            "12",
        ];
        let mut seed = 99u64;
        let mut next = move || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 33) as usize
        };
        let (mut good, mut wrong) = (0, 0);
        for _ in 0..600 {
            let n = next() % 30;
            let text: String = (0..n)
                .map(|_| tokens[next() % tokens.len()])
                .collect::<Vec<_>>()
                .join(" ");
            let rows = text.lines().count();
            match Profile::from_text(&text, "fuzz") {
                Ok(p) => {
                    good += 1;
                    assert!(p.max_line <= 100_000 && p.max_decimals <= 15, "{text:?}");
                }
                Err(e) => {
                    wrong += 1;
                    assert!(!e.is_empty());
                    assert!(
                        e.iter().all(|f| f.line.is_none_or(|l| l < rows.max(1))),
                        "{text:?} {e:?}"
                    );
                    for f in &e {
                        assert!(!f.text("en").contains('{'));
                    }
                }
            }
        }
        assert!(good > 20 && wrong > 20, "{good} {wrong}");
    }
}
