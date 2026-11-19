//! The check of the format of a program for the controller it is made for, as the output of a post-processor is held to: the length of a line, the
//! number of decimals, the `%` lines, the codes the controller lists, two codes of the same group on one line, a word given twice in a line and the
//! end of the program. A profile is a short list of what a kind of controller takes (`grbl`, `full`, `generic`); the lists are in this file
//! and say what the documentation of GRBL lists and what a controller with canned cycles, cutter compensation and tool changes takes, so a code a profile
//! does not list is a reason to read the line, not a proof that the
//! controller refuses it. Only a reading: nothing is sent and nothing is changed.

use crate::gcode::words;
use crate::progcheck::{Finding, Level};
use crate::words::Msg;

/// A kind of controller and what its format asks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Profile {
    pub name: &'static str,
    /// The most characters of a line once its spaces and its comments are taken out (0: no limit).
    pub max_line: usize,
    /// The most decimals of a number (0: no limit).
    pub max_decimals: usize,
    /// A line with only `%` is taken.
    pub percent: bool,
    /// The program must end with `M2` or `M30`.
    pub needs_end: bool,
    dialect: Dialect,
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
                name: "grbl",
                max_line: 79,
                max_decimals: 4,
                percent: false,
                needs_end: true,
                dialect: Dialect::Grbl,
            }),
            "full" => Some(Profile {
                name: "full",
                max_line: 255,
                max_decimals: 6,
                percent: true,
                needs_end: true,
                dialect: Dialect::Full,
            }),
            "generic" => Some(Profile {
                name: "generic",
                max_line: 0,
                max_decimals: 0,
                percent: true,
                needs_end: false,
                dialect: Dialect::Generic,
            }),
            _ => None,
        }
    }

    fn lists_g(&self, tenths: i32) -> bool {
        match self.dialect {
            Dialect::Generic => true,
            Dialect::Grbl => GRBL_G.contains(&tenths),
            Dialect::Full => GRBL_G.contains(&tenths) || FULL_EXTRA_G.contains(&tenths),
        }
    }

    fn lists_m(&self, code: i32) -> bool {
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
}
