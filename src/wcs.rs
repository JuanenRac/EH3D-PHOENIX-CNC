//! The work coordinate systems of a G-code program (`G54` to `G59`) and the offsets written into them (`G10 L2` and `G10 L20`): which
//! systems the program moves in, which it changes the offsets of and what that does to the moves around it. The program-wide checks name
//! every `G10` and every change of system already; this reads the numbers: the offsets a line writes, a write made after moves already
//! ran in that system (so the part is not in the same place before and after), a system that is used and never set by the program, and a
//! change of system followed at once by a cutting move. It only reads and never sends anything.

use crate::gcode::words;
use crate::progcheck::{Finding, Level};
use crate::words::{say, Msg};

/// How many moves a program makes in one work coordinate system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Use {
    /// 54 to 59.
    pub system: u8,
    /// The line (from 0) of the first move in it.
    pub first_line: usize,
    pub moves: usize,
}

/// The result of reading a program.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Scan {
    /// The systems the program moves in, in the order of their number.
    pub used: Vec<Use>,
    pub findings: Vec<Finding>,
}

/// The system a `P` word names (`P1` is `G54` ... `P6` is `G59`); `P0` is the one in force.
fn system_of(p: f64, current: u8) -> Option<u8> {
    if p.fract() != 0.0 || !(0.0..=6.0).contains(&p) {
        return None;
    }
    Some(if p == 0.0 { current } else { 53 + p as u8 })
}

fn index(system: u8) -> usize {
    usize::from(system - 54)
}

fn shown(v: Option<f64>) -> String {
    match v {
        Some(v) => {
            let s = format!("{v:.4}");
            let s = s.trim_end_matches('0').trim_end_matches('.');
            if s.is_empty() || s == "-0" {
                "0".to_string()
            } else {
                s.to_string()
            }
        }
        None => "-".to_string(),
    }
}

/// Reads the program.
pub fn scan(lines: &[String]) -> Scan {
    let mut out = Scan::default();
    let mut current = 54u8;
    let mut motion = 0;
    let mut moves = [0usize; 6];
    let mut first: [Option<usize>; 6] = [None; 6];
    let mut written = [false; 6];
    // a system was selected at this line and no move has followed yet
    let mut just_switched: Option<(usize, u8)> = None;
    for (n, raw) in lines.iter().enumerate() {
        let trimmed = raw.trim_start();
        if trimmed.starts_with('$') || trimmed.starts_with('%') {
            continue;
        }
        let ws = words(raw);
        let get = |l: char| ws.iter().find(|&&(c, _)| c == l).map(|&(_, v)| v);
        let (mut g10, mut other_frame, mut selects) = (false, false, None);
        for &(c, v) in &ws {
            if c != 'G' {
                continue;
            }
            let t = (v * 10.0).round() as i32;
            match t {
                0 | 10 | 20 | 30 => motion = t / 10,
                100 => g10 = true,
                // machine coordinates and the shifted zero have no work system of their own
                530 | 920 => other_frame = true,
                540 | 550 | 560 | 570 | 580 | 590 => selects = Some((t / 10) as u8),
                _ => {}
            }
        }
        // the system a G10 writes is read with the one that is in force before this line changes it
        if g10 {
            if let Some(l) = get('L').filter(|l| *l == 2.0 || *l == 20.0) {
                let p = get('P').unwrap_or(0.0);
                match system_of(p, current) {
                    None => out.findings.push(Finding {
                        line: Some(n),
                        level: Level::Warning,
                        msg: Msg::WcsBadP,
                        args: vec![shown(Some(p))],
                    }),
                    Some(s) => {
                        let i = index(s);
                        let a = vec![
                            format!("G{s}"),
                            shown(get('X')),
                            shown(get('Y')),
                            shown(get('Z')),
                        ];
                        out.findings.push(Finding {
                            line: Some(n),
                            level: Level::Warning,
                            msg: if l == 2.0 {
                                Msg::WcsWrite
                            } else {
                                Msg::WcsWriteHere
                            },
                            args: a,
                        });
                        if let Some(at) = first[i].filter(|_| moves[i] > 0) {
                            out.findings.push(Finding {
                                line: Some(n),
                                level: Level::Warning,
                                msg: Msg::WcsWriteAfterUse,
                                args: vec![format!("G{s}"), (at + 1).to_string()],
                            });
                        }
                        written[i] = true;
                    }
                }
            }
        }
        if let Some(s) = selects {
            if s != current {
                just_switched = Some((n, s));
            }
            current = s;
        }
        let axes = ws.iter().any(|&(c, _)| matches!(c, 'X' | 'Y' | 'Z'));
        if axes && !g10 && !other_frame {
            let i = index(current);
            moves[i] += 1;
            first[i].get_or_insert(n);
            if let Some((at, s)) = just_switched.take() {
                if matches!(motion, 1..=3) {
                    out.findings.push(Finding {
                        line: Some(n),
                        level: Level::Warning,
                        msg: Msg::WcsSwitchCut,
                        args: vec![format!("G{s}"), (at + 1).to_string()],
                    });
                }
            }
        }
    }
    for s in 54u8..=59 {
        let i = index(s);
        if moves[i] > 0 {
            out.used.push(Use {
                system: s,
                first_line: first[i].unwrap_or(0),
                moves: moves[i],
            });
            if s != 54 && !written[i] {
                out.findings.push(Finding {
                    line: first[i],
                    level: Level::Info,
                    msg: Msg::WcsUnset,
                    args: vec![format!("G{s}")],
                });
            }
        }
    }
    if out.used.len() > 1 {
        let list: Vec<String> = out
            .used
            .iter()
            .map(|u| format!("G{} ({})", u.system, u.moves))
            .collect();
        out.findings.push(Finding {
            line: None,
            level: Level::Info,
            msg: Msg::WcsUsed,
            args: vec![list.join(", ")],
        });
    }
    out.findings.sort_by_key(|f| f.line.unwrap_or(usize::MAX));
    out
}

/// The findings of `scan`.
pub fn findings(lines: &[String]) -> Vec<Finding> {
    scan(lines).findings
}

/// The sentence of the systems in use for the report, in a language; nothing while the program moves in one system or none.
pub fn report_lines(lines: &[String], lang: &str) -> Vec<String> {
    let s = scan(lines);
    s.findings
        .iter()
        .filter(|f| f.msg == Msg::WcsUsed)
        .map(|f| say(lang, f.msg, &f.args))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prog(text: &str) -> Vec<String> {
        text.lines().map(str::to_string).collect()
    }

    fn msgs(text: &str) -> Vec<Msg> {
        findings(&prog(text)).iter().map(|f| f.msg).collect()
    }

    #[test]
    fn the_offsets_a_g10_line_writes_are_listed_with_their_numbers() {
        let f = findings(&prog("G10 L2 P2 X10.5 Y-3 Z0\nG55\nG0 X1"));
        assert_eq!(f[0].msg, Msg::WcsWrite);
        assert_eq!(f[0].line, Some(0));
        assert_eq!(f[0].args, ["G55", "10.5", "-3", "0"]);
        // an axis the line leaves out is a dash
        let g = findings(&prog("G10 L2 P1 Z-2"));
        assert_eq!(g[0].args, ["G54", "-", "-", "-2"]);
        // G10 L20 sets the offset so the tool is where the line says
        let h = findings(&prog("G55\nG10 L20 P0 X0 Y0"));
        assert_eq!(h[0].msg, Msg::WcsWriteHere);
        assert_eq!(h[0].args[0], "G55");
    }

    #[test]
    fn a_p_that_names_no_system_is_said() {
        assert_eq!(msgs("G10 L2 P7 X1"), vec![Msg::WcsBadP]);
        assert_eq!(msgs("G10 L2 P2.5 X1"), vec![Msg::WcsBadP]);
        assert_eq!(msgs("G10 L2 P-1 X1"), vec![Msg::WcsBadP]);
        // P0 is the system in force and a G10 of another kind is not a write of offsets
        assert_eq!(msgs("G10 L2 P0 X1"), vec![Msg::WcsWrite]);
        assert!(msgs("G10 L1 P3 Z5").is_empty());
        assert!(msgs("G10").is_empty());
    }

    #[test]
    fn an_offset_written_after_moves_in_that_system_is_said_with_the_first_move() {
        let f = findings(&prog("G0 X1\nG1 X2 F5\nG10 L2 P1 X5\nG0 X0"));
        let kinds: Vec<Msg> = f.iter().map(|f| f.msg).collect();
        assert_eq!(kinds, vec![Msg::WcsWrite, Msg::WcsWriteAfterUse]);
        assert_eq!(f[1].args, ["G54", "1"]);
        // the same write before any move says nothing more
        assert_eq!(msgs("G10 L2 P1 X5\nG0 X0"), vec![Msg::WcsWrite]);
        // a move in another system does not count
        assert!(!msgs("G55\nG0 X1\nG10 L2 P1 X5").contains(&Msg::WcsWriteAfterUse));
    }

    #[test]
    fn a_system_the_program_never_sets_is_an_info_but_g54_is_left_alone() {
        let f = findings(&prog("G55\nG0 X1\nG1 X2 F5"));
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!((f[0].msg, f[0].line), (Msg::WcsUnset, Some(1)));
        assert_eq!(f[0].level, Level::Info);
        assert!(msgs("G0 X1\nG1 X2 F5").is_empty());
        // set in the program, before or after: no warning of this kind
        assert!(!msgs("G10 L2 P2 X0\nG55\nG0 X1").contains(&Msg::WcsUnset));
    }

    #[test]
    fn a_change_of_system_followed_by_a_cut_is_a_warning_and_by_a_rapid_is_not() {
        let cut = findings(&prog("G10 L2 P2 X0\nG1 F5\nG0 X5\nG55\nG1 X9"));
        assert!(cut
            .iter()
            .any(|f| f.msg == Msg::WcsSwitchCut && f.line == Some(4)));
        let s = cut.iter().find(|f| f.msg == Msg::WcsSwitchCut).unwrap();
        assert_eq!(s.args, ["G55", "4"]);
        let rapid = msgs("G10 L2 P2 X0\nG0 X5\nG55\nG0 X9\nG1 X1 F5");
        assert!(!rapid.contains(&Msg::WcsSwitchCut), "{rapid:?}");
        // selecting the system already in force changes nothing
        assert!(!msgs("G10 L2 P1 X0\nG54\nG1 X9 F5").contains(&Msg::WcsSwitchCut));
        // said once: the move after it is not the first
        let twice = msgs("G10 L2 P2 X0\nG55\nG1 X1 F5\nG1 X2");
        assert_eq!(twice.iter().filter(|m| **m == Msg::WcsSwitchCut).count(), 1);
    }

    #[test]
    fn the_systems_in_use_are_counted_in_the_order_of_their_number() {
        let s = scan(&prog(
            "G10 L2 P2 X0\nG10 L2 P3 X0\nG56\nG0 X1\nG55\nG0 X2\nG0 X3\nG54\nG0 X4",
        ));
        let used: Vec<(u8, usize)> = s.used.iter().map(|u| (u.system, u.moves)).collect();
        assert_eq!(used, [(54, 1), (55, 2), (56, 1)]);
        assert_eq!(s.used[1].first_line, 5);
        let sentence = s.findings.iter().find(|f| f.msg == Msg::WcsUsed).unwrap();
        assert_eq!(sentence.line, None);
        assert_eq!(sentence.args, ["G54 (1), G55 (2), G56 (1)"]);
        let r = report_lines(&prog("G10 L2 P2 X0\nG0 X1\nG55\nG0 X2"), "en");
        assert_eq!(r.len(), 1);
        assert!(r[0].contains("G54 (1), G55 (1)"), "{}", r[0]);
        // one system, or none, is not worth a line
        assert!(report_lines(&prog("G0 X1"), "en").is_empty());
        assert!(report_lines(&prog("M30"), "en").is_empty());
    }

    #[test]
    fn machine_coordinates_and_the_shifted_zero_are_not_moves_in_a_system() {
        let s = scan(&prog("G55\nG53 G0 Z0\nG92 X0\nG10 L2 P2 X1"));
        assert!(s.used.is_empty(), "{:?}", s.used);
        assert!(!s.findings.iter().any(|f| f.msg == Msg::WcsUnset));
        // lines for the controller are skipped
        assert!(scan(&prog("$G55\n%\n")).findings.is_empty());
    }

    #[test]
    fn the_numbers_are_written_short() {
        assert_eq!(shown(Some(2.0)), "2");
        assert_eq!(shown(Some(-0.00001)), "0");
        assert_eq!(shown(Some(0.125)), "0.125");
        assert_eq!(shown(None), "-");
        assert_eq!(system_of(0.0, 57), Some(57));
        assert_eq!(system_of(6.0, 54), Some(59));
        assert_eq!(system_of(7.0, 54), None);
    }
}
