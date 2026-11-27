//! A program that starts again from one of its lines, as a program of its own: the lines before the start are read to rebuild what the program had in force
//! there (`startline`: the units, the distance mode, the plane, the work coordinate system, the tool length offset, the feed and the speed of the spindle) and
//! the new program begins with the lines that put the controller back in that state, then goes on from the chosen line. It starts nothing by itself: the
//! spindle and the coolant, the tool and the place the tool was at are written as comments the operator reads, and a pause (`M0`) comes before the first
//! line of the program, so the machine moves only when the operator has looked at it and goes on. Starting is refused where the state cannot be rebuilt
//! from the lines (relative distances, a canned cycle or a cutter compensation still on). Only a reading of the program: nothing is sent.

use crate::startline::{self, Note, Refusal};
use crate::words::{say, Msg};

/// The new program and what was read to make it.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    /// The line of the original program the new program goes on from (from 1).
    pub line: usize,
    /// The lines that put the controller back, and then all the lines of the original from `line` on.
    pub program: Vec<String>,
    /// How many lines at the start of `program` are the new ones.
    pub preamble: usize,
    /// How many lines before `line` were read.
    pub before: usize,
    pub notes: Vec<Note>,
}

/// Why there is no program to start from the line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Why {
    /// The program has this many lines and the line asked for is not one of them (or is 0).
    PastEnd { lines: usize, asked: usize },
    /// The state cannot be rebuilt from the lines before it.
    Refused(Refusal),
}

impl Why {
    pub fn text(self, lang: &str) -> String {
        match self {
            Why::PastEnd { lines, asked } => say(
                lang,
                Msg::RestartPastEnd,
                &[lines.to_string(), asked.to_string()],
            ),
            Why::Refused(r) => refusal_text(r, lang),
        }
    }
}

/// The sentence for why a program cannot start from a line.
pub fn refusal_text(r: Refusal, lang: &str) -> String {
    match r {
        Refusal::Empty => say::<&str>(lang, Msg::SlRefEmpty, &[]),
        Refusal::Relative => say::<&str>(lang, Msg::SlRefRelative, &[]),
        Refusal::Cycle(l) => say(lang, Msg::SlRefCycle, &[l.to_string()]),
        Refusal::Compensation(l) => say(lang, Msg::SlRefComp, &[l.to_string()]),
        Refusal::Motion(l) => say(lang, Msg::SlRefMotion, &[l.to_string()]),
        Refusal::FeedMode(l) => say(lang, Msg::SlRefFeedMode, &[l.to_string()]),
        Refusal::TableLength(l) => say(lang, Msg::SlRefTable, &[l.to_string()]),
    }
}

/// The sentence for something the program has in force at the line it starts from.
pub fn note_text(n: &Note, lang: &str) -> String {
    match *n {
        Note::Inches => say::<&str>(lang, Msg::SlInches, &[]),
        Note::Plane(p) => say(lang, Msg::SlPlane, &[p.to_string()]),
        Note::Wcs(w) => say(lang, Msg::SlWcs, &[w.to_string()]),
        Note::ToolLength => say::<&str>(lang, Msg::SlToolLength, &[]),
        Note::Tool(t) => say(lang, Msg::SlTool, &[t.to_string()]),
        Note::Spindle {
            code,
            rpm: Some(rpm),
        } => say(lang, Msg::SlSpindle, &[code.to_string(), rpm.to_string()]),
        Note::Spindle { code, rpm: None } => say(lang, Msg::SlSpindleNoSpeed, &[code.to_string()]),
        Note::Coolant(c) => say(lang, Msg::SlCoolant, &[c.to_string()]),
        Note::NoFeed => say::<&str>(lang, Msg::SlNoFeed, &[]),
        Note::Offset(l) => say(lang, Msg::SlOffset, &[l.to_string()]),
    }
}

/// The new program that starts from `line` (from 1) of this one.
pub fn plan(program: &[String], line: usize) -> Result<Plan, Why> {
    if line == 0 || line > program.len() {
        return Err(Why::PastEnd {
            lines: program.len(),
            asked: line,
        });
    }
    let index = line - 1;
    let check = startline::check(program, index);
    if let Some(r) = check.refusal {
        return Err(Why::Refused(r));
    }
    let m = &check.modal;
    let mut head = vec![format!("(restart at line {line} of the program)")];
    head.push(if m.inches { "G20" } else { "G21" }.to_string());
    head.push("G90".to_string());
    head.push(format!("G{}", m.plane));
    head.push(format!("G{}", m.wcs));
    // a value given in other units than the ones in force now is written under its own units
    let (feed_in_inches, offset_in_inches) = startline::units_of_values(program, index);
    let units = |inches: bool| if inches { "G20" } else { "G21" }.to_string();
    let restate = |head: &mut Vec<String>, given_in_inches: bool, line: String| {
        if given_in_inches == m.inches {
            head.push(line);
        } else {
            head.extend([units(given_in_inches), line, units(m.inches)]);
        }
    };
    if let Some(z) = m.tool_length {
        restate(&mut head, offset_in_inches, format!("G43.1 Z{z}"));
    }
    if let Some(f) = m.feed {
        restate(&mut head, feed_in_inches, format!("F{f}"));
    }
    if let Some(s) = m.speed {
        head.push(format!("S{s}"));
    }
    for n in &check.notes {
        match *n {
            Note::Tool(t) => head.push(format!("(restart: the tool in use was T{t})")),
            Note::Spindle { code, rpm } => head.push(match rpm {
                Some(r) => format!("(restart: the spindle was on, M{code} S{r}, start it first)"),
                None => format!("(restart: the spindle was on, M{code}, start it first)"),
            }),
            Note::Coolant(c) => head.push(format!(
                "(restart: the coolant was on, M{c}, start it first)"
            )),
            Note::Offset(l) => head.push(format!(
                "(restart: the zero was shifted at line {l}, the machine needs the same shift)"
            )),
            _ => {}
        }
    }
    if let Some((x, y)) = check.at {
        head.push(format!(
            "(restart: the tool was at X{x:.3} Y{y:.3} mm before this line, move there at a safe height)"
        ));
    }
    head.push("M0 (restart: check the machine, then resume)".to_string());
    // after the pause, so that nothing typed while it lasts changes it: a line that names only axes cuts when the program was cutting
    if matches!(m.motion, 0 | 1) {
        head.push(format!("G{}", m.motion));
    }
    let preamble = head.len();
    let mut out = head;
    out.extend(program[index..].iter().cloned());
    Ok(Plan {
        line,
        program: out,
        preamble,
        before: index,
        notes: check.notes,
    })
}

/// What is said of the plan on the side, in a language: the summary and one sentence for each thing the program had in force.
pub fn remarks(p: &Plan, lang: &str) -> Vec<String> {
    let mut out = vec![say(
        lang,
        Msg::RestartSummary,
        &[p.line.to_string(), p.before.to_string()],
    )];
    out.extend(p.notes.iter().map(|n| note_text(n, lang)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gcode::Modal;

    fn prog(s: &str) -> Vec<String> {
        s.lines().map(str::to_string).collect()
    }

    const JOB: &str =
        "G20 G90 G55\nG18\nT3 M6\nM3 S1200\nM8\nF15\nG0 X1 Y2\nG1 Z-0.1\nG1 X2\nG1 X3\nM30";

    #[test]
    fn the_state_the_program_had_is_written_back_before_the_chosen_line() {
        let p = plan(&prog(JOB), 9).unwrap();
        assert_eq!((p.line, p.before), (9, 8));
        assert_eq!(&p.program[1..6], ["G20", "G90", "G18", "G55", "F15"]);
        assert!(p.program.contains(&"S1200".to_string()));
        // what would start the machine is a comment, never a command
        assert!(p
            .program
            .iter()
            .any(|l| l == "(restart: the spindle was on, M3 S1200, start it first)"));
        assert!(p
            .program
            .iter()
            .any(|l| l == "(restart: the coolant was on, M8, start it first)"));
        assert!(p
            .program
            .iter()
            .any(|l| l == "(restart: the tool in use was T3)"));
        assert!(p.program[..p.preamble - 2]
            .iter()
            .filter(|l| !l.starts_with('('))
            .all(|l| !l.starts_with('M')));
        // the place the tool was at, then the pause, then the program as it was
        assert!(p
            .program
            .iter()
            .any(|l| l.starts_with("(restart: the tool was at X")));
        assert_eq!(
            p.program[p.preamble - 2],
            "M0 (restart: check the machine, then resume)"
        );
        // the way of moving the program was in comes last: the next line names only X
        assert_eq!(p.program[p.preamble - 1], "G1");
        assert_eq!(&p.program[p.preamble..], &prog(JOB)[8..]);
    }

    #[test]
    fn a_start_at_the_first_line_has_the_defaults_and_nothing_to_read() {
        let p = plan(&prog(JOB), 1).unwrap();
        assert_eq!(p.before, 0);
        assert_eq!(&p.program[1..5], ["G21", "G90", "G17", "G54"]);
        assert!(p.notes.is_empty());
        assert_eq!(&p.program[p.preamble..], &prog(JOB)[..]);
    }

    #[test]
    fn a_line_that_does_not_exist_is_an_error_that_says_how_many_lines_there_are() {
        let e = plan(&prog(JOB), 12).unwrap_err();
        assert_eq!(
            e,
            Why::PastEnd {
                lines: 11,
                asked: 12
            }
        );
        assert_eq!(
            e.text("en"),
            "The program has only 11 lines: there is no line 12 to restart at"
        );
        assert!(plan(&prog(JOB), 0).is_err());
        assert!(plan(&[], 1).is_err());
        assert!(plan(&prog(JOB), 11).is_ok());
    }

    #[test]
    fn where_the_state_cannot_be_rebuilt_the_start_is_refused_with_the_line_that_caused_it() {
        let rel = plan(&prog("G21 G91\nG1 X1 F100\nG1 X1\nM30"), 3).unwrap_err();
        assert_eq!(rel, Why::Refused(Refusal::Relative));
        let cycle = plan(&prog("G21 G90\nG81 X1 Y1 Z-2 R1 F100\nX2\nG80\nM30"), 3).unwrap_err();
        assert_eq!(cycle, Why::Refused(Refusal::Cycle(2)));
        let comp = plan(&prog("G21 G90\nG41 D1 G1 X1 F100\nG1 X5\nG40\nM30"), 3).unwrap_err();
        assert_eq!(comp, Why::Refused(Refusal::Compensation(2)));
        // once they are off again the start is fine
        assert!(plan(&prog("G21 G90\nG81 X1 Y1 Z-2 R1 F100\nG80\nG1 X5\nM30"), 4).is_ok());
        assert!(comp.text("es").contains('2'));
    }

    #[test]
    fn a_shifted_zero_and_a_length_offset_are_written_or_said() {
        let p = plan(
            &prog("G21 G90\nG92 X5\nG43.1 Z-3\nG1 X1 F100\nG1 X2\nM30"),
            5,
        )
        .unwrap();
        assert!(p.program.contains(&"G43.1 Z-3".to_string()));
        assert!(p
            .program
            .iter()
            .any(|l| l.starts_with("(restart: the zero was shifted at line 2")));
        assert!(p.notes.contains(&Note::Offset(2)) && p.notes.contains(&Note::ToolLength));
    }

    #[test]
    fn the_remarks_are_told_in_the_eight_languages_with_their_holes_filled() {
        let p = plan(&prog(JOB), 9).unwrap();
        let en = remarks(&p, "en");
        assert_eq!(
            en[0],
            "Restart at line 9: the 8 lines before it were read to rebuild the state"
        );
        assert_eq!(en.len(), 1 + p.notes.len());
        for lang in crate::words::LANGS {
            for r in remarks(&p, lang) {
                assert!(!r.contains('{') && !r.is_empty(), "{lang}: {r}");
            }
        }
    }

    #[test]
    fn a_line_that_names_only_axes_still_cuts_after_a_restart() {
        let p = plan(&prog("G21 G90 F100\nG1 Z-1\nX10\nX20\nM30"), 4).unwrap();
        assert_eq!(p.program[p.preamble - 1], "G1");
        assert_eq!(startline::check(&p.program, p.preamble).modal.motion, 1);
        let rapid = plan(&prog("G21 G90\nG0 Z5\nX10\nX20"), 4).unwrap();
        assert_eq!(rapid.program[rapid.preamble - 1], "G0");
        let arc = plan(&prog("G21 G90 F100\nG2 X2 Y0 I1 J0\nX4 I1 J0\nM30"), 3).unwrap_err();
        assert_eq!(arc, Why::Refused(Refusal::Motion(2)));
        assert!(arc.text("de").contains('2'));
    }

    #[test]
    fn the_feed_and_the_offset_are_written_in_the_units_they_were_given_in() {
        let p = plan(&prog("G20 G90 F10\nG43.1 Z-1\nG21\nG1 X1\nG1 X2"), 5).unwrap();
        let head = &p.program[..p.preamble];
        let at = head.iter().position(|l| l == "F10").unwrap();
        assert_eq!(
            (head[at - 1].as_str(), head[at + 1].as_str()),
            ("G20", "G21")
        );
        let at = head.iter().position(|l| l == "G43.1 Z-1").unwrap();
        assert_eq!(
            (head[at - 1].as_str(), head[at + 1].as_str()),
            ("G20", "G21")
        );
    }

    /// The part of the state the new program writes back.
    #[allow(clippy::type_complexity)]
    fn state(
        m: &Modal,
    ) -> (
        u8,
        bool,
        bool,
        u8,
        u8,
        Option<f64>,
        Option<f64>,
        Option<f64>,
    ) {
        (
            m.motion,
            m.inches,
            m.absolute,
            m.plane,
            m.wcs,
            m.feed,
            m.speed,
            m.tool_length,
        )
    }

    #[test]
    fn the_new_program_has_the_state_the_old_one_had_at_that_line() {
        let mut seed = 5u64;
        let mut next = move || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 33) as usize
        };
        let pieces = [
            "G20",
            "G21",
            "G17",
            "G18",
            "G19",
            "G54",
            "G55",
            "G59",
            "F100",
            "F250.5",
            "S800",
            "S15000",
            "M3",
            "M5",
            "G43.1 Z-2",
            "G49",
            "G0 X5 Y5",
            "G1 X10 Y10",
            "G1 Z-1",
            "T2 M6",
            "M8",
            "M9",
        ];
        let (mut made, mut refused) = (0, 0);
        for _ in 0..500 {
            let n = 4 + next() % 14;
            let mut p = vec!["G90".to_string()];
            for _ in 0..n {
                p.push(pieces[next() % pieces.len()].to_string());
            }
            p.push("G1 X1 F50".to_string());
            let line = 1 + next() % p.len();
            match plan(&p, line) {
                Ok(plan) => {
                    made += 1;
                    let old = startline::check(&p, line - 1);
                    let new = startline::check(&plan.program, plan.preamble);
                    // the units, the distance mode, the plane, the work system, the feed, the speed and the length offset are the same
                    assert_eq!(
                        state(&old.modal),
                        state(&new.modal),
                        "{p:?} line {line}\n{:?}",
                        plan.program
                    );
                    assert_eq!(&plan.program[plan.preamble..], &p[line - 1..]);
                }
                Err(_) => refused += 1,
            }
        }
        assert!(made > 300, "{made} {refused}");
    }
}
