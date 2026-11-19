//! The canned cycles of a G-code program (`G73`, `G81` to `G86` and `G89`) worked out into plain moves, and what is wrong with them. A
//! cycle line is replaced by the rapid moves and the cutting moves it stands for (to the hole, down to the R plane, in to the depth, in
//! pecks, a dwell, out again), so the path, the time and the keep-out boxes of a program with drilling cycles are the real ones and not
//! the straight line from hole to hole that a program without cycle codes would draw. The cycles of the retract plane `G98` (back to the
//! level the cycle started from) and `G99` (back to the R plane) are both followed.
//!
//! Only absolute distances (`G90`) in the plane `G17` are expanded; a cycle in relative mode, in another plane, a boring-out cycle
//! (`G87`, `G88`) or one with a missing depth or R plane is left as it is and said. Nothing here moves a machine.

use crate::gcode::{toolpath, words, Segment};
use crate::progcheck::{Finding, Level};
use crate::words::Msg;

/// How far above the last depth of a peck the tool goes back down to by a rapid move before it cuts again (`G83`), and how far it
/// backs off to break the chip (`G73`), in mm.
pub const PECK_CLEARANCE_MM: f64 = 0.5;

/// The most pecks one hole is cut in; a smaller step is made bigger so a mistaken step cannot write a program that never ends.
pub const MAX_PECKS: usize = 1000;

/// A program with its cycles expanded.
#[derive(Debug, Clone, PartialEq)]
pub struct Expanded {
    /// The lines of the new program.
    pub lines: Vec<String>,
    /// The line of the original program (from 0) that each new line comes from.
    pub origin: Vec<usize>,
    /// What is wrong with the cycles, ordered by line.
    pub findings: Vec<Finding>,
    /// How many holes (executions of a cycle) were expanded.
    pub holes: usize,
}

/// The values a cycle keeps from one line to the next.
#[derive(Debug, Clone, Copy, Default)]
struct Params {
    z: Option<f64>,
    r: Option<f64>,
    q: Option<f64>,
    p: Option<f64>,
    f: Option<f64>,
}

#[derive(Debug, Clone, Copy)]
struct Active {
    code: i32,
    started: usize,
    initial_z: f64,
    /// The cycle cannot be expanded (and was said once): its lines go on as written.
    left_alone: bool,
}

/// `G81` for 810, `G73` for 730.
fn gname(tenths: i32) -> String {
    format!("G{}", tenths / 10)
}

/// A number as short as it can be written back (up to four decimals).
fn num(v: f64) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" {
        "0".to_string()
    } else {
        s.to_string()
    }
}

fn is_cycle(tenths: i32) -> bool {
    tenths == 730 || ((810..=890).contains(&tenths) && tenths % 10 == 0)
}

/// Whether the program has any canned cycle code.
pub fn has_cycles(lines: &[String]) -> bool {
    lines.iter().any(|l| {
        words(l)
            .iter()
            .any(|&(c, v)| c == 'G' && is_cycle((v * 10.0).round() as i32))
    })
}

/// The words of a line that the expansion keeps (the other codes, the spindle, the tool...) written as a line of their own, or nothing.
fn kept_words(ws: &[(char, f64)]) -> Option<String> {
    let parts: Vec<String> = ws
        .iter()
        .filter(|&&(c, v)| match c {
            'G' => {
                let t = (v * 10.0).round() as i32;
                !is_cycle(t) && t != 800
            }
            'X' | 'Y' | 'Z' | 'R' | 'Q' | 'P' | 'L' | 'F' | 'I' | 'J' | 'K' => false,
            _ => true,
        })
        .map(|&(c, v)| format!("{c}{}", num(v)))
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" "))
    }
}

/// Reads the program and writes it again with every cycle that can be expanded replaced by its moves.
pub fn expand(lines: &[String]) -> Expanded {
    expand_with(lines, PECK_CLEARANCE_MM)
}

/// `expand` with the clearance of the pecks given (mm, not below 0).
pub fn expand_with(lines: &[String], clearance_mm: f64) -> Expanded {
    let clearance = if clearance_mm.is_finite() {
        clearance_mm.max(0.0)
    } else {
        PECK_CLEARANCE_MM
    };
    let mut out = Expanded {
        lines: Vec::with_capacity(lines.len()),
        origin: Vec::with_capacity(lines.len()),
        findings: Vec::new(),
        holes: 0,
    };
    let mut pos = (0.0f64, 0.0f64, 0.0f64);
    let (mut absolute, mut plane, mut back_to_initial) = (true, 17, true);
    let mut spindle = 5;
    let mut params = Params::default();
    let mut active: Option<Active> = None;
    for (n, raw) in lines.iter().enumerate() {
        let trimmed = raw.trim_start();
        if trimmed.starts_with('$') || trimmed.starts_with('%') {
            out.lines.push(raw.clone());
            out.origin.push(n);
            continue;
        }
        let ws = words(raw);
        let mut cycle_code = None;
        let mut motion = false;
        let mut cancel = false;
        let mut target = [None; 3];
        for &(c, v) in &ws {
            let t = (v * 10.0).round() as i32;
            match c {
                'G' if is_cycle(t) => cycle_code = Some(t),
                'G' if t == 800 => cancel = true,
                'G' if matches!(t, 0 | 10 | 20 | 30) => motion = true,
                'G' if t == 900 => absolute = true,
                'G' if t == 910 => absolute = false,
                'G' if t == 980 => back_to_initial = true,
                'G' if t == 990 => back_to_initial = false,
                'G' if matches!(t, 170 | 180 | 190) => plane = t / 10,
                'M' if matches!(v as i32, 3..=5) => spindle = v as i32,
                'X' => target[0] = Some(v),
                'Y' => target[1] = Some(v),
                'Z' => target[2] = Some(v),
                _ => {}
            }
            match c {
                'R' => params.r = Some(v),
                'Q' => params.q = Some(v),
                'P' => params.p = Some(v),
                'F' if v > 0.0 => params.f = Some(v),
                _ => {}
            }
        }
        if cancel || motion {
            active = None;
        }
        if cancel && cycle_code.is_none() && !motion && target.iter().all(Option::is_none) {
            // the end of the cycle, kept as it is written
            out.lines.push(raw.clone());
            out.origin.push(n);
            continue;
        }
        if let Some(code) = cycle_code {
            if !cancel {
                let initial_z = active.map_or(pos.2, |a| a.initial_z);
                let started = active.map_or(n, |a| a.started);
                active = Some(Active {
                    code,
                    started,
                    initial_z,
                    left_alone: false,
                });
                if let Some(z) = target[2] {
                    params.z = Some(z);
                }
            }
        } else if active.is_some() {
            if let Some(z) = target[2] {
                params.z = Some(z);
            }
        }
        // the cycle runs on the line that names it and on each later line that has an X or a Y
        let runs = active.is_some()
            && !motion
            && !cancel
            && (cycle_code.is_some() || target[0].is_some() || target[1].is_some());
        if !runs {
            out.lines.push(raw.clone());
            out.origin.push(n);
            if let Some(next) = move_to(pos, target, absolute) {
                pos = next;
            }
            continue;
        }
        let Some(mut a) = active else { continue };
        let code = a.code;
        let mut say_it = |msg: Msg, level: Level| {
            out.findings.push(Finding {
                line: Some(n),
                level,
                msg,
                args: vec![gname(code)],
            });
        };
        let refused = if a.left_alone {
            Some(None)
        } else if matches!(code, 870 | 880) || plane != 17 {
            Some(Some(Msg::CycleUnsupported))
        } else if !absolute {
            Some(Some(Msg::CycleRelative))
        } else if params.z.is_none() {
            Some(Some(Msg::CycleNoZ))
        } else if params.r.is_none() {
            Some(Some(Msg::CycleNoR))
        } else if params.r.zip(params.z).is_some_and(|(r, z)| r <= z) {
            Some(Some(Msg::CycleRBelowZ))
        } else {
            None
        };
        if let Some(why) = refused {
            if let Some(msg) = why {
                say_it(msg, Level::Warning);
                a.left_alone = true;
                active = Some(a);
            }
            out.lines.push(raw.clone());
            out.origin.push(n);
            if let Some(next) = move_to(pos, target, absolute) {
                pos = next;
            }
            continue;
        }
        let (Some(z), Some(r)) = (params.z, params.r) else {
            continue;
        };
        let mut q = params.q.unwrap_or(0.0);
        if matches!(code, 730 | 830) && q <= 0.0 {
            say_it(Msg::CycleNoQ, Level::Warning);
            // with no step the hole is cut in one go
            q = r - z;
        }
        if let Some(line) = kept_words(&ws) {
            out.lines.push(line);
            out.origin.push(n);
        }
        let mut emit = |text: String| {
            out.lines.push(text);
            out.origin.push(n);
        };
        let feed = params
            .f
            .map(|f| format!(" F{}", num(f)))
            .unwrap_or_default();
        let (x, y) = (target[0].unwrap_or(pos.0), target[1].unwrap_or(pos.1));
        let retract = if back_to_initial && a.initial_z > r {
            a.initial_z
        } else {
            r
        };
        if pos.2 < r {
            emit(format!("G0 Z{}", num(r)));
        }
        emit(format!("G0 X{} Y{}", num(x), num(y)));
        if pos.2 > r {
            emit(format!("G0 Z{}", num(r)));
        }
        match code {
            810 | 840 | 860 => {
                emit(format!("G1 Z{}{feed}", num(z)));
                if code == 860 && matches!(spindle, 3 | 4) {
                    emit("M5".to_string());
                    emit(format!("G0 Z{}", num(retract)));
                    emit(format!("M{spindle}"));
                } else if code == 840 {
                    emit(format!("G1 Z{}{feed}", num(r)));
                    if retract > r {
                        emit(format!("G0 Z{}", num(retract)));
                    }
                } else {
                    emit(format!("G0 Z{}", num(retract)));
                }
            }
            820 => {
                emit(format!("G1 Z{}{feed}", num(z)));
                emit(format!("G4 P{}", num(params.p.unwrap_or(0.0))));
                emit(format!("G0 Z{}", num(retract)));
            }
            850 | 890 => {
                emit(format!("G1 Z{}{feed}", num(z)));
                if code == 890 {
                    emit(format!("G4 P{}", num(params.p.unwrap_or(0.0))));
                }
                emit(format!("G1 Z{}{feed}", num(r)));
                if retract > r {
                    emit(format!("G0 Z{}", num(retract)));
                }
            }
            _ => {
                // G73 and G83: the hole is cut in pecks of q
                let depth = r - z;
                let step = q.max(depth / MAX_PECKS as f64);
                let mut at = r;
                let mut first = true;
                while at > z + 1e-9 {
                    let next = (at - step).max(z);
                    emit(format!(
                        "G1 Z{}{}",
                        num(next),
                        if first { feed.clone() } else { String::new() }
                    ));
                    first = false;
                    if next <= z + 1e-9 {
                        break;
                    }
                    if code == 830 {
                        emit(format!("G0 Z{}", num(r)));
                        emit(format!("G0 Z{}", num(next + clearance)));
                    } else {
                        emit(format!("G0 Z{}", num((next + clearance).min(r))));
                    }
                    at = next;
                }
                emit(format!("G0 Z{}", num(retract)));
            }
        }
        out.holes += 1;
        pos = (x, y, retract);
    }
    if let Some(a) = active {
        out.findings.push(Finding {
            line: Some(a.started),
            level: Level::Warning,
            msg: Msg::CycleOpen,
            args: vec![gname(a.code)],
        });
    }
    out.findings.sort_by_key(|f| f.line.unwrap_or(usize::MAX));
    out
}

/// Where a line with these axis words leaves the tool (nothing when it has none).
fn move_to(
    pos: (f64, f64, f64),
    target: [Option<f64>; 3],
    absolute: bool,
) -> Option<(f64, f64, f64)> {
    if target.iter().all(Option::is_none) {
        return None;
    }
    let pick = |t: Option<f64>, now: f64| match (t, absolute) {
        (Some(v), true) => v,
        (Some(v), false) => now + v,
        (None, _) => now,
    };
    Some((
        pick(target[0], pos.0),
        pick(target[1], pos.1),
        pick(target[2], pos.2),
    ))
}

/// The straight moves of the program with its cycles expanded, each one carrying the line of the original program it comes from. A
/// program with no cycle gives the same moves as `toolpath`.
pub fn toolpath_expanded(lines: &[String]) -> Vec<Segment> {
    if !has_cycles(lines) {
        return toolpath(lines);
    }
    let e = expand(lines);
    let mut path = toolpath(&e.lines);
    for s in &mut path {
        s.line = e.origin.get(s.line).copied().unwrap_or(s.line);
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prog(text: &str) -> Vec<String> {
        text.lines().map(str::to_string).collect()
    }

    fn exp(text: &str) -> Expanded {
        expand(&prog(text))
    }

    fn msgs(e: &Expanded) -> Vec<Msg> {
        e.findings.iter().map(|f| f.msg).collect()
    }

    #[test]
    fn a_simple_drilling_cycle_goes_down_to_the_depth_and_back_to_the_r_plane() {
        let e = exp("G90 G99\nG0 Z5\nG81 X10 Y20 Z-3 R2 F100\nG80\nM30");
        assert_eq!(e.holes, 1);
        assert_eq!(
            e.lines,
            [
                "G90 G99",
                "G0 Z5",
                "G0 X10 Y20",
                "G0 Z2",
                "G1 Z-3 F100",
                "G0 Z2",
                "G80",
                "M30"
            ]
        );
        assert!(e.findings.is_empty());
    }

    #[test]
    fn g98_returns_to_the_level_the_cycle_started_from_and_g99_to_the_r_plane() {
        let a = exp("G98\nG0 Z10\nG81 X1 Y1 Z-2 R3 F50\nX2 Y2\nG80");
        assert_eq!(a.holes, 2);
        // back to 10 after each hole (the level of the start), not to 3
        let ups: Vec<&String> = a.lines.iter().filter(|l| l.as_str() == "G0 Z10").collect();
        assert_eq!(ups.len(), 3, "{:?}", a.lines);
        let b = exp("G99\nG0 Z10\nG81 X1 Y1 Z-2 R3 F50\nX2 Y2\nG80");
        assert!(
            !b.lines.iter().skip(2).any(|l| l == "G0 Z10"),
            "{:?}",
            b.lines
        );
        assert_eq!(b.lines.iter().filter(|l| l.as_str() == "G0 Z3").count(), 3);
    }

    #[test]
    fn the_second_hole_takes_the_values_of_the_first() {
        let e = exp("G0 Z5\nG81 X0 Y0 Z-4 R1 F80\nX5\nX10 Y5\nG80");
        assert_eq!(e.holes, 3);
        let at: Vec<&String> = e.lines.iter().filter(|l| l.starts_with("G0 X")).collect();
        assert_eq!(at, ["G0 X0 Y0", "G0 X5 Y0", "G0 X10 Y5"]);
        assert_eq!(
            e.lines.iter().filter(|l| l.starts_with("G1 Z-4")).count(),
            3
        );
    }

    #[test]
    fn a_dwell_goes_in_after_the_depth_of_g82_and_g89() {
        let e = exp("G0 Z5\nG82 X1 Y1 Z-1 R2 P0.5 F60\nG89 X2 Y1 Z-1 R2 P1 F60\nG80");
        assert_eq!(e.holes, 2);
        assert!(e.lines.contains(&"G4 P0.5".to_string()));
        assert!(e.lines.contains(&"G4 P1".to_string()));
        // G89 feeds out: a cutting move back to R, and no rapid retract before it
        let g89: Vec<&String> = e.lines.iter().skip_while(|l| *l != "G4 P0.5").collect();
        assert!(g89.iter().any(|l| l.starts_with("G1 Z2")), "{g89:?}");
    }

    #[test]
    fn a_peck_cycle_cuts_in_steps_and_g73_backs_off_only_a_little() {
        let e = exp("G0 Z5\nG83 X0 Y0 Z-5 R0 Q2 F90\nG80");
        let cuts: Vec<&String> = e.lines.iter().filter(|l| l.starts_with("G1")).collect();
        assert_eq!(cuts, ["G1 Z-2 F90", "G1 Z-4", "G1 Z-5"]);
        // full retract to R and back down to just above the last depth
        assert!(e.lines.contains(&"G0 Z-1.5".to_string()), "{:?}", e.lines);
        assert!(e.lines.contains(&"G0 Z-3.5".to_string()));
        let c = exp("G0 Z5\nG73 X0 Y0 Z-5 R0 Q2 F90\nG80");
        assert!(c.lines.contains(&"G0 Z-1.5".to_string()), "{:?}", c.lines);
        // the chip break goes up by the clearance only, never above R
        assert_eq!(c.lines.iter().filter(|l| l.as_str() == "G0 Z0").count(), 1);
        let tiny = expand_with(&prog("G0 Z5\nG73 X0 Y0 Z-1 R0 Q0.4 F9\nG80"), 9.0);
        assert!(
            tiny.lines.iter().all(|l| l != "G0 Z8.6"),
            "{:?}",
            tiny.lines
        );
    }

    #[test]
    fn a_step_of_zero_or_a_tiny_one_cannot_make_a_program_that_never_ends() {
        let none = exp("G0 Z5\nG83 X0 Y0 Z-5 R0 F90\nG80");
        assert_eq!(msgs(&none), vec![Msg::CycleNoQ]);
        assert_eq!(none.lines.iter().filter(|l| l.starts_with("G1")).count(), 1);
        let tiny = exp("G0 Z5\nG83 X0 Y0 Z-5000 R0 Q0.000001 F90\nG80");
        assert!(
            tiny.lines.len() < 4 * MAX_PECKS + 20,
            "{}",
            tiny.lines.len()
        );
    }

    #[test]
    fn the_cycles_of_boring_and_tapping_go_in_and_out_at_the_feed() {
        let e = exp("M3 S900\nG0 Z5\nG85 X0 Y0 Z-2 R1 F40\nG80");
        assert!(e.lines.iter().filter(|l| l.starts_with("G1")).count() == 2);
        let s = exp("M3 S900\nG0 Z5\nG86 X0 Y0 Z-2 R1 F40\nG80");
        let at = s.lines.iter().position(|l| l == "M5").unwrap();
        assert_eq!(s.lines[at + 2], "M3");
        // with the spindle off G86 has no spindle to stop
        assert!(!exp("G0 Z5\nG86 X0 Y0 Z-2 R1 F40\nG80")
            .lines
            .contains(&"M5".to_string()));
        let t = exp("G0 Z5\nG84 X0 Y0 Z-2 R1 F40\nG80");
        assert_eq!(t.lines.iter().filter(|l| l.starts_with("G1 Z")).count(), 2);
    }

    #[test]
    fn what_the_cycle_line_also_says_is_kept() {
        let e = exp("G0 Z5\nG81 X1 Y1 Z-1 R1 F10 M8 S500\nG80");
        assert!(e.lines.contains(&"M8 S500".to_string()), "{:?}", e.lines);
        assert_eq!(e.origin[0], 0);
        // every new line knows its line of the program
        assert!(e.origin.iter().all(|&o| o < 3));
        assert_eq!(e.origin.len(), e.lines.len());
    }

    #[test]
    fn a_cycle_with_a_missing_value_or_a_wrong_one_is_left_as_written_and_said() {
        let no_z = exp("G81 X1 Y1 R2 F10\nG80");
        assert_eq!(msgs(&no_z), vec![Msg::CycleNoZ]);
        assert_eq!(no_z.lines, ["G81 X1 Y1 R2 F10", "G80"]);
        assert_eq!(msgs(&exp("G81 X1 Y1 Z-1 F10\nG80")), vec![Msg::CycleNoR]);
        // R at or below the depth
        assert_eq!(
            msgs(&exp("G81 X1 Y1 Z-1 R-2 F10\nG80")),
            vec![Msg::CycleRBelowZ]
        );
        assert_eq!(
            msgs(&exp("G81 X1 Y1 Z-1 R-1 F10\nG80")),
            vec![Msg::CycleRBelowZ]
        );
        // said once for a cycle, not for every hole
        let many = exp("G81 X1 Y1 R2 F10\nX2\nX3\nG80");
        assert_eq!(many.findings.len(), 1);
        assert_eq!(many.holes, 0);
    }

    #[test]
    fn relative_mode_other_planes_and_the_cycles_that_are_not_expanded_are_said() {
        let rel = exp("G91\nG81 X1 Y1 Z-1 R1 F10\nG80");
        assert_eq!(msgs(&rel), vec![Msg::CycleRelative]);
        let g18 = exp("G18\nG81 X1 Y1 Z-1 R2 F10\nG80");
        assert_eq!(msgs(&g18), vec![Msg::CycleUnsupported]);
        let g87 = exp("G87 X1 Y1 Z-1 R2 F10\nG80");
        assert_eq!(msgs(&g87), vec![Msg::CycleUnsupported]);
        assert_eq!(g87.holes, 0);
    }

    #[test]
    fn a_cycle_never_cancelled_is_said_with_the_line_that_started_it() {
        let e = exp("G0 Z5\nG81 X1 Y1 Z-1 R2 F10\nX2\nM30");
        assert_eq!(msgs(&e), vec![Msg::CycleOpen]);
        assert_eq!(e.findings[0].line, Some(1));
        assert_eq!(e.findings[0].args, ["G81"]);
        assert!(exp("G0 Z5\nG81 X1 Y1 Z-1 R2 F10\nG80\nM30")
            .findings
            .is_empty());
        // a move code ends the cycle as G80 does
        assert!(exp("G0 Z5\nG81 X1 Y1 Z-1 R2 F10\nG0 X9\nM30")
            .findings
            .is_empty());
    }

    #[test]
    fn the_path_of_a_program_with_cycles_is_the_real_one() {
        let p = prog("G21 G90\nG0 Z5\nG81 X10 Y0 Z-3 R2 F100\nX20\nG80\nM30");
        assert!(has_cycles(&p));
        let path = toolpath_expanded(&p);
        // every move comes from a line of the original program
        assert!(path.iter().all(|s| s.line < p.len()));
        let deepest = path.iter().map(|s| s.to.2).fold(f64::MAX, f64::min);
        assert_eq!(deepest, -3.0);
        let cuts = path.iter().filter(|s| !s.rapid).count();
        assert_eq!(cuts, 2);
        // a program with no cycle gives the same path as before
        let plain = prog("G0 X5\nG1 Z-1 F10");
        assert!(!has_cycles(&plain));
        assert_eq!(toolpath_expanded(&plain), toolpath(&plain));
    }

    #[test]
    fn the_numbers_are_written_short_and_without_a_negative_zero() {
        assert_eq!(num(2.0), "2");
        assert_eq!(num(-1.5), "-1.5");
        assert_eq!(num(0.10001), "0.1");
        assert_eq!(num(-0.00001), "0");
        assert_eq!(num(0.0), "0");
    }

    #[test]
    fn lines_for_the_controller_and_comments_go_through_untouched() {
        let e = exp("$H\n; G81 X1 Y1 Z-1 R1\n(G81 X9)\nG0 X1");
        assert_eq!(e.lines.len(), 4);
        assert_eq!(e.holes, 0);
        assert!(!has_cycles(&prog("; G81 X1\n(G83)")));
    }
}
