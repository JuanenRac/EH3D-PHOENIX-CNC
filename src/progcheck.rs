//! The second checker of a G-code program: it names, line by line, the commands that change what the controller believes (the units never
//! set or changed on the way, a shifted zero, stored positions, offsets written, machine coordinates, probing, a plane or a work system
//! that changes, a program that stops or ends in relative mode). What the summary checks already (a rapid move below the surface, a cut
//! with the spindle off, the limits of feed and speed, the end of the program) is in `lint` and is not repeated here. It only reads: it
//! never changes the program and never sends anything.

use crate::gcode::{toolpath, words};
use crate::tuning::Tuning;
use crate::words::{say, Msg};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// Worth knowing.
    Info,
    /// Easy to get wrong.
    Warning,
    /// Can hurt the work or the machine.
    Danger,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// The line of the program (from 0), or nothing for a remark about the whole program.
    pub line: Option<usize>,
    pub level: Level,
    pub msg: Msg,
    /// What fills the holes of the message after the line number.
    pub args: Vec<String>,
}

impl Finding {
    /// The sentence in a language; the line number is shown from 1.
    pub fn text(&self, lang: &str) -> String {
        let mut a: Vec<String> = Vec::new();
        if let Some(l) = self.line {
            a.push((l + 1).to_string());
        }
        a.extend(self.args.iter().cloned());
        say(lang, self.msg, &a)
    }
}

/// The name of a G code given in tenths: 530 is `G53`, 921 is `G92.1`.
fn gname(tenths: i32) -> String {
    if tenths % 10 == 0 {
        format!("G{}", tenths / 10)
    } else {
        format!("G{}.{}", tenths / 10, tenths % 10)
    }
}

/// Reads the program and returns what looks wrong, ordered by line (the remarks about the whole program last).
pub fn check(lines: &[String], t: &Tuning) -> Vec<Finding> {
    let mut out: Vec<Finding> = Vec::new();
    if !t.progcheck {
        return out;
    }
    let mut push = |line: Option<usize>, level: Level, msg: Msg, args: Vec<String>| {
        out.push(Finding {
            line,
            level,
            msg,
            args,
        })
    };
    // None until the program says G20 or G21
    let mut inches: Option<bool> = None;
    let (mut plane, mut wcs, mut absolute) = (17, 54, true);
    let (mut spindle, mut warned_units) = (false, false);
    // a tool number (T) has been given, here or on an earlier line
    let mut chosen = false;
    for (n, raw) in lines.iter().enumerate() {
        let trimmed = raw.trim_start();
        // a line for the controller itself ($...) or a program marker (%) is not G-code
        if trimmed.starts_with('$') || trimmed.starts_with('%') {
            continue;
        }
        let ws = words(raw);
        let mut axis = false;
        // the number of the tool may come after M6 on the same line
        if ws.iter().any(|&(l, _)| l == 'T') {
            chosen = true;
        }
        for &(l, v) in &ws {
            let tenths = (v * 10.0).round() as i32;
            match l {
                'G' => match tenths {
                    170 | 180 | 190 => {
                        if plane != tenths / 10 {
                            push(
                                Some(n),
                                Level::Info,
                                Msg::Plane,
                                vec![(tenths / 10).to_string()],
                            );
                            plane = tenths / 10;
                        }
                    }
                    200 | 210 => {
                        let now = tenths == 200;
                        if inches.is_some_and(|before| before != now) {
                            push(
                                Some(n),
                                Level::Warning,
                                Msg::UnitSwitch,
                                vec![gname(tenths)],
                            );
                        }
                        inches = Some(now);
                    }
                    280 | 281 | 300 | 301 => {
                        push(Some(n), Level::Warning, Msg::Stored, vec![gname(tenths)])
                    }
                    382..=385 => push(Some(n), Level::Warning, Msg::Probe, vec![gname(tenths)]),
                    530 => push(Some(n), Level::Warning, Msg::MachineCoords, vec![]),
                    540 | 550 | 560 | 570 | 580 | 590 => {
                        if wcs != tenths / 10 {
                            push(
                                Some(n),
                                Level::Info,
                                Msg::Wcs,
                                vec![(tenths / 10).to_string()],
                            );
                            wcs = tenths / 10;
                        }
                    }
                    900 => absolute = true,
                    910 => absolute = false,
                    920 => push(Some(n), Level::Warning, Msg::Shift, vec![]),
                    100 => push(Some(n), Level::Danger, Msg::Offsets, vec![]),
                    _ => {}
                },
                'M' => match v as i32 {
                    0 | 1 => push(
                        Some(n),
                        Level::Info,
                        Msg::Stop,
                        vec![format!("M{}", v as i32)],
                    ),
                    6 => {
                        if spindle {
                            push(Some(n), Level::Warning, Msg::ToolSpindle, vec![]);
                        }
                        if !chosen {
                            push(Some(n), Level::Warning, Msg::ToolNoNumber, vec![]);
                        }
                    }
                    112 => push(Some(n), Level::Danger, Msg::Emergency, vec![]),
                    3 | 4 => spindle = true,
                    5 => spindle = false,
                    _ => {}
                },
                'X' | 'Y' | 'Z' => axis = true,
                _ => {}
            }
        }
        if axis && inches.is_none() && !warned_units {
            warned_units = true;
            push(None, Level::Warning, Msg::NoUnits, vec![]);
        }
    }
    let path = toolpath(lines);
    if path.is_empty() {
        push(None, Level::Info, Msg::NoMoves, vec![]);
    } else if !absolute {
        push(None, Level::Warning, Msg::Relative, vec![]);
    }
    out.sort_by_key(|f| f.line.unwrap_or(usize::MAX));
    out
}

/// The most serious level among the findings.
pub fn worst(findings: &[Finding]) -> Option<Level> {
    findings.iter().map(|f| f.level).max()
}

/// How many findings of each level: (info, warning, danger).
pub fn counts(findings: &[Finding]) -> (usize, usize, usize) {
    let c = |l| findings.iter().filter(|f| f.level == l).count();
    (c(Level::Info), c(Level::Warning), c(Level::Danger))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prog(text: &str) -> Vec<String> {
        text.lines().map(str::to_string).collect()
    }

    fn msgs(text: &str) -> Vec<Msg> {
        check(&prog(text), &Tuning::default())
            .into_iter()
            .map(|f| f.msg)
            .collect()
    }

    const GOOD: &str = "G21 G90
M3 S12000
G0 X10 Y10
G0 Z5
G1 Z-1 F200
G1 X20
G0 Z5
M5
M30";

    #[test]
    fn a_clean_program_has_nothing_to_say() {
        assert_eq!(msgs(GOOD), vec![]);
    }

    #[test]
    fn what_the_summary_checks_already_is_not_repeated_here() {
        // a rapid below the surface, a cut with the spindle off, a speed and a feed too high, no end: all of them are `lint`
        let m = msgs(
            "G21
G0 Z-3
G1 X1 F99999
M3 S999999
G1 X2",
        );
        assert_eq!(m, vec![]);
    }

    #[test]
    fn moves_with_no_units_are_flagged_once() {
        let m = msgs(
            "M3
G1 X1 F100
G1 X2
M30",
        );
        assert_eq!(m.iter().filter(|m| **m == Msg::NoUnits).count(), 1);
    }

    #[test]
    fn a_change_of_units_is_flagged_but_setting_them_twice_is_not() {
        assert!(msgs(
            "G21
M3
G1 X1 F100
G20
G1 X2
M30"
        )
        .contains(&Msg::UnitSwitch));
        assert!(!msgs(
            "G21
M3
G1 X1 F100
G21
G1 X2
M30"
        )
        .contains(&Msg::UnitSwitch));
    }

    #[test]
    fn the_dangerous_commands_are_named_with_their_line() {
        let f = check(
            &prog(
                "G21
G28
G92 X0
G10 L2 P1 X0
G53 G0 Z0
G38.2 Z-10 F50
M0
M112
M30",
            ),
            &Tuning::default(),
        );
        let by = |m: Msg| f.iter().find(|f| f.msg == m).map(|f| (f.line, f.level));
        assert_eq!(by(Msg::Stored), Some((Some(1), Level::Warning)));
        assert_eq!(by(Msg::Shift), Some((Some(2), Level::Warning)));
        assert_eq!(by(Msg::Offsets), Some((Some(3), Level::Danger)));
        assert_eq!(by(Msg::MachineCoords), Some((Some(4), Level::Warning)));
        assert_eq!(by(Msg::Probe), Some((Some(5), Level::Warning)));
        assert_eq!(by(Msg::Stop), Some((Some(6), Level::Info)));
        assert_eq!(by(Msg::Emergency), Some((Some(7), Level::Danger)));
        assert_eq!(worst(&f), Some(Level::Danger));
    }

    #[test]
    fn the_name_of_a_code_keeps_its_decimal() {
        let f = check(
            &prog(
                "G21
G38.3 Z-5 F20
G28.1
M30",
            ),
            &Tuning::default(),
        );
        assert!(f.iter().any(|f| f.msg == Msg::Probe && f.args == ["G38.3"]));
        assert!(f
            .iter()
            .any(|f| f.msg == Msg::Stored && f.args == ["G28.1"]));
    }

    #[test]
    fn work_systems_and_planes_are_noted_only_when_they_change() {
        let m = msgs(
            "G21
G54
G17
G55
G18
G18
M30",
        );
        assert_eq!(m.iter().filter(|m| **m == Msg::Wcs).count(), 1);
        assert_eq!(m.iter().filter(|m| **m == Msg::Plane).count(), 1);
    }

    #[test]
    fn a_tool_change_with_the_spindle_on_is_flagged() {
        assert!(msgs(
            "G21
M3
T2 M6
M30"
        )
        .contains(&Msg::ToolSpindle));
        assert!(!msgs(
            "G21
M3
M5
T2 M6
M30"
        )
        .contains(&Msg::ToolSpindle));
    }

    #[test]
    fn a_tool_change_with_no_tool_chosen_is_flagged_once_per_change() {
        let f = check(&prog("G21\nM6\nT3\nM6\nM30"), &Tuning::default());
        let at: Vec<(Option<usize>, Msg)> = f
            .iter()
            .filter(|f| f.msg == Msg::ToolNoNumber)
            .map(|f| (f.line, f.msg))
            .collect();
        // only the first M6 comes before any T
        assert_eq!(at, vec![(Some(1), Msg::ToolNoNumber)]);
        assert_eq!(f[0].level, Level::Warning);
        // the number may come first on the line or after the change on the same line
        assert!(!msgs("G21\nT1 M6\nM30").contains(&Msg::ToolNoNumber));
        assert!(!msgs("G21\nM6 T1\nM30").contains(&Msg::ToolNoNumber));
        // a program with no change says nothing, and a spindle that turns adds its own remark
        assert!(!msgs("G21\nT2\nM30").contains(&Msg::ToolNoNumber));
        let both = msgs("G21\nM3\nM6\nM30");
        assert!(both.contains(&Msg::ToolNoNumber) && both.contains(&Msg::ToolSpindle));
        // said in the eight languages with its line
        let en = f[0].text("en");
        assert!(en.starts_with("Line 2: M6"), "{en}");
        for l in ["es", "de", "fr", "it", "ja", "zh", "pt"] {
            assert_ne!(f[0].text(l), en, "{l}");
        }
        // with the checker off nothing is said
        let off = Tuning {
            progcheck: false,
            ..Tuning::default()
        };
        assert!(check(&prog("M6"), &off).is_empty());
    }

    #[test]
    fn the_end_of_the_program_is_checked() {
        assert!(msgs(
            "G21
G91
M3
G1 X1 F100
M30"
        )
        .contains(&Msg::Relative));
        assert!(!msgs(
            "G21
G91
M3
G1 X1 F100
G90
M30"
        )
        .contains(&Msg::Relative));
        assert_eq!(
            msgs(
                "(nothing)
M30"
            ),
            vec![Msg::NoMoves]
        );
    }

    #[test]
    fn comments_and_controller_lines_are_not_read() {
        assert_eq!(
            msgs(
                "G21
(G92 X0 M112)
; G10
$130=9999
%
M3
G1 X1 F100
M30"
            ),
            vec![]
        );
    }

    #[test]
    fn the_checker_can_be_turned_off() {
        let t = Tuning {
            progcheck: false,
            ..Tuning::default()
        };
        assert!(check(
            &prog(
                "G10 L2 X0
M112"
            ),
            &t
        )
        .is_empty());
    }

    #[test]
    fn findings_come_ordered_by_line_with_the_program_remarks_last() {
        let f = check(
            &prog(
                "G1 X1 F100
G92 X0
G10 L2",
            ),
            &Tuning::default(),
        );
        let lines: Vec<_> = f.iter().map(|f| f.line).collect();
        let mut sorted = lines.clone();
        sorted.sort_by_key(|l| l.unwrap_or(usize::MAX));
        assert_eq!(lines, sorted);
        assert_eq!(lines.last(), Some(&None));
    }

    #[test]
    fn the_text_has_the_line_from_one_and_follows_the_language() {
        let f = check(
            &prog(
                "G21
G92 X0
M30",
            ),
            &Tuning::default(),
        );
        let s = f.iter().find(|f| f.msg == Msg::Shift).unwrap();
        assert_eq!(
            s.text("en"),
            "Line 2: G92 shifts the work zero until G92.1 or a reset"
        );
        assert!(s.text("de").starts_with("Zeile 2:"));
    }

    #[test]
    fn counts_add_up() {
        let f = check(
            &prog(
                "G1 X1 F100
G92 X0
M112",
            ),
            &Tuning::default(),
        );
        let (i, w, d) = counts(&f);
        assert_eq!(i + w + d, f.len());
        assert_eq!(d, 1);
        assert_eq!(worst(&[]), None);
    }
}
