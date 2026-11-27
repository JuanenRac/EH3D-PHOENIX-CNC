//! Starting a program from one of its lines: what the program had in force when it got there (the units, the plane, the work coordinates, the
//! tool, the spindle, the coolant, the feed) so that the operator can read it before the machine is put back in that state, and the cases in
//! which starting there is not safe and is refused (relative distances, a canned cycle or a cutter compensation that is still on). The
//! lines that bring the machine back are `Session::resume_at_safe`, the same ones a program left in the middle uses; this only reads the
//! lines before the start. It knows nothing of the window and moves nothing.

use crate::gcode::{toolpath, words, Modal};

/// Why the program cannot start from the line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The program has no lines.
    Empty,
    /// Relative distances (`G91`) are in force, so where the tool was cannot be rebuilt from the lines.
    Relative,
    /// A canned cycle (`G81` to `G89`) is still on; the line is the one that started it (from 1).
    Cycle(usize),
    /// A cutter compensation (`G41`, `G42`) is on; the line is the one that started it (from 1).
    Compensation(usize),
    /// The program goes on in an arc, a probe or after `G80` without saying so again; the line is the one that set that way of moving (from 1).
    Motion(usize),
    /// The feed is inverse time or per revolution (`G93`, `G95`); the line is the one that set it (from 1).
    FeedMode(usize),
    /// A tool length offset from the table of the controller (`G43`, `G43.2`) is on; the line is the one that set it (from 1).
    TableLength(usize),
}

/// Whether the lines from `rest` on move in the way of moving that was in force before them: the first line that moves names axes and no motion code.
fn relies_on_motion(rest: &[String]) -> bool {
    for line in rest {
        let ws = words(line);
        let has = |codes: &[i32]| {
            ws.iter()
                .any(|&(l, v)| l == 'G' && codes.contains(&((v * 10.0).round() as i32)))
        };
        let states = ws.iter().any(|&(l, v)| {
            let t = (v * 10.0).round() as i32;
            l == 'G'
                && (matches!(t, 0 | 10 | 20 | 30 | 800)
                    || (381..=385).contains(&t)
                    || (810..=890).contains(&t))
        });
        let ends = ws
            .iter()
            .any(|&(l, v)| l == 'M' && matches!(v as i32, 2 | 30));
        if states || ends {
            return false;
        }
        // these take the axis words for themselves
        if has(&[100, 280, 300, 431, 920]) {
            continue;
        }
        if ws
            .iter()
            .any(|&(l, _)| matches!(l, 'X' | 'Y' | 'Z' | 'A' | 'B' | 'C' | 'I' | 'J' | 'K'))
        {
            return true;
        }
    }
    false
}

/// What is in force at `index` (from 0) that a program cannot be given back when it starts there: the feed mode (`G93`, `G95`), a tool length offset
/// of the table of the controller (`G43`), and a way of moving other than `G0` or `G1` that the lines from there on take without saying it again.
pub fn state_refusal(program: &[String], index: usize) -> Option<Refusal> {
    let index = index.min(program.len());
    let mut modal = Modal::default();
    let (mut motion, mut feed_mode, mut table) = (0usize, None, None);
    for (n, line) in program[..index].iter().enumerate() {
        modal.apply(line);
        for (letter, v) in words(line) {
            if letter != 'G' {
                continue;
            }
            match (v * 10.0).round() as i32 {
                20 | 30 | 381..=385 | 800 => motion = n + 1,
                930 | 950 => feed_mode = Some(n + 1),
                940 => feed_mode = None,
                430 | 432 => table = Some(n + 1),
                431 | 490 => table = None,
                _ => {}
            }
        }
    }
    if let Some(l) = feed_mode {
        return Some(Refusal::FeedMode(l));
    }
    if let Some(l) = table {
        return Some(Refusal::TableLength(l));
    }
    (!matches!(modal.motion, 0 | 1) && relies_on_motion(&program[index..]))
        .then_some(Refusal::Motion(motion))
}

/// Whether the feed and the tool length offset in force at `index` (from 0) were given in inches: `(feed, offset)`. A value that is written again has
/// to be written under the units it was given in, whatever units the program changed to afterwards.
pub fn units_of_values(program: &[String], index: usize) -> (bool, bool) {
    let mut modal = Modal::default();
    let (mut feed, mut offset) = (false, false);
    for line in &program[..index.min(program.len())] {
        modal.apply(line);
        for (letter, v) in words(line) {
            match letter {
                'F' if v > 0.0 => feed = modal.inches,
                'G' if (v * 10.0).round() as i32 == 431 => offset = modal.inches,
                _ => {}
            }
        }
    }
    (feed, offset)
}

/// Something to read before the start; none of them stops it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Note {
    /// The program is in inches (`G20`).
    Inches,
    /// A plane other than XY is in force (17, 18 or 19).
    Plane(u8),
    /// A work coordinate system other than `G54` (the number: 55 to 59).
    Wcs(u8),
    /// A dynamic tool length offset (`G43.1`) is in force.
    ToolLength,
    /// The tool the program had last asked for (`T`): the machine is not told to change it.
    Tool(u32),
    /// The spindle is on: `M3` or `M4`, with the speed when there was one.
    Spindle { code: u8, rpm: Option<f64> },
    /// The coolant is on: `M7` or `M8`.
    Coolant(u8),
    /// No feed rate was set before the line, and a cutting move comes before the first `F`.
    NoFeed,
    /// The zero was shifted (`G92`, `G10`) at this line (from 1): the machine has to have the same shift, it is not repeated.
    Offset(usize),
}

/// What the program has in force at a line.
#[derive(Debug, Clone, PartialEq)]
pub struct StartCheck {
    /// The line the program would start from (from 1) and its text.
    pub line: usize,
    pub text: String,
    pub refusal: Option<Refusal>,
    pub notes: Vec<Note>,
    /// Where the path of the program had the tool in X and Y (mm); `None` before the first move.
    pub at: Option<(f64, f64)>,
    pub modal: Modal,
}

/// Reads the lines before `index` (from 0; past the end is the last line) and says what is in force there.
pub fn check(program: &[String], index: usize) -> StartCheck {
    if program.is_empty() {
        return StartCheck {
            line: 0,
            text: String::new(),
            refusal: Some(Refusal::Empty),
            notes: Vec::new(),
            at: None,
            modal: Modal::default(),
        };
    }
    let index = index.min(program.len() - 1);
    let mut modal = Modal::default();
    let mut tool: Option<u32> = None;
    let mut cycle: Option<usize> = None;
    let mut comp: Option<usize> = None;
    let mut offset: Option<usize> = None;
    for (n, line) in program[..index].iter().enumerate() {
        let t = line.trim();
        if t.is_empty() || t.starts_with(';') {
            continue;
        }
        modal.apply(t);
        for (letter, v) in words(t) {
            match (letter, (v * 10.0).round() as i32) {
                ('G', 800) => cycle = None,
                ('G', 810..=890) => cycle = Some(n + 1),
                ('G', 400) => comp = None,
                ('G', 410 | 420) => comp = Some(n + 1),
                ('G', 100 | 920 | 923) => offset = Some(n + 1),
                ('G', 921 | 922) => offset = None,
                ('T', _) if v >= 0.0 => tool = Some(v as u32),
                _ => {}
            }
        }
    }
    let refusal = if !modal.absolute {
        Some(Refusal::Relative)
    } else if let Some(l) = cycle {
        Some(Refusal::Cycle(l))
    } else {
        comp.map(Refusal::Compensation)
            .or_else(|| state_refusal(program, index))
    };
    let mut notes = Vec::new();
    if modal.inches {
        notes.push(Note::Inches);
    }
    if modal.plane != 17 {
        notes.push(Note::Plane(modal.plane));
    }
    if modal.wcs != 54 {
        notes.push(Note::Wcs(modal.wcs));
    }
    if modal.tool_length.is_some() {
        notes.push(Note::ToolLength);
    }
    if let Some(t) = tool {
        notes.push(Note::Tool(t));
    }
    if modal.spindle != 5 {
        notes.push(Note::Spindle {
            code: modal.spindle,
            rpm: modal.speed,
        });
    }
    if modal.coolant != 9 {
        notes.push(Note::Coolant(modal.coolant));
    }
    if no_feed_before_a_cut(&program[index..], &modal) {
        notes.push(Note::NoFeed);
    }
    if let Some(l) = offset {
        notes.push(Note::Offset(l));
    }
    let at = toolpath(&program[..index]).last().map(|s| (s.to.0, s.to.1));
    StartCheck {
        line: index + 1,
        text: program[index].clone(),
        refusal,
        notes,
        at,
        modal,
    }
}

/// Whether a cutting move comes before the program says any feed rate, when none is in force at the start.
fn no_feed_before_a_cut(rest: &[String], modal: &Modal) -> bool {
    if modal.feed.is_some() {
        return false;
    }
    let mut m = modal.clone();
    for line in rest {
        let t = line.trim();
        if t.is_empty() || t.starts_with(';') {
            continue;
        }
        m.apply(t);
        if m.feed.is_some() {
            return false;
        }
        let ws = words(t);
        let moves = ws.iter().any(|&(l, _)| matches!(l, 'X' | 'Y' | 'Z'));
        if matches!(m.motion, 1..=3) && moves {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(s: &str) -> Vec<String> {
        s.lines().map(str::to_string).collect()
    }

    #[test]
    fn the_state_at_a_line_is_read_from_the_lines_before_it() {
        let p = lines("G21 G90\nT2 M6\nM3 S12000\nG0 X10 Y5 Z5\nG1 Z-1 F200\nG1 X20\nG1 X30\nM30");
        let c = check(&p, 5);
        assert_eq!((c.line, c.text.as_str()), (6, "G1 X20"));
        assert_eq!(c.refusal, None);
        assert_eq!(
            c.notes,
            [
                Note::Tool(2),
                Note::Spindle {
                    code: 3,
                    rpm: Some(12000.0)
                }
            ]
        );
        assert_eq!(c.at, Some((10.0, 5.0)));
        // at the first line nothing is in force and the tool has not moved
        let first = check(&p, 0);
        assert_eq!(
            (first.notes.len(), first.at, first.refusal),
            (0, None, None)
        );
    }

    #[test]
    fn the_units_the_plane_the_zero_the_offset_and_the_coolant_are_noted() {
        let p = lines("G20 G18 G55\nG43.1 Z-1.5\n; a note\n\nM8\nG92 X0 Y0\nG1 X1 F10\nG1 X2");
        let c = check(&p, 7);
        assert_eq!(
            c.notes,
            [
                Note::Inches,
                Note::Plane(18),
                Note::Wcs(55),
                Note::ToolLength,
                Note::Coolant(8),
                Note::Offset(6)
            ]
        );
        assert_eq!(c.refusal, None);
    }

    #[test]
    fn relative_distances_a_cycle_and_a_compensation_that_are_on_refuse_the_start() {
        assert_eq!(
            check(&lines("G91\nG1 X1 F10\nG1 X1"), 2).refusal,
            Some(Refusal::Relative)
        );
        // back to absolute is fine
        assert_eq!(check(&lines("G91\nG90\nG1 X1 F10"), 2).refusal, None);
        let cycle = lines("G81 X1 Y1 Z-1 R1 F10\nX2 Y2\nG80\nG0 X0");
        assert_eq!(check(&cycle, 2).refusal, Some(Refusal::Cycle(1)));
        assert_eq!(check(&cycle, 3).refusal, None);
        let comp = lines("G0 X0\nG41 D1\nG1 X5 F10\nG40\nG0 X9");
        assert_eq!(check(&comp, 2).refusal, Some(Refusal::Compensation(2)));
        assert_eq!(check(&comp, 4).refusal, None);
    }

    #[test]
    fn a_cut_before_any_feed_is_noted_and_a_feed_on_the_line_is_enough() {
        let p = lines("G90\nG0 X0 Y0\nG1 Z-1\nG1 X5 F100\nG1 X9");
        assert!(check(&p, 2).notes.contains(&Note::NoFeed));
        // the line itself sets the feed before it moves
        assert!(!check(&p, 3).notes.contains(&Note::NoFeed));
        // a feed in force before the line
        assert!(!check(&p, 4).notes.contains(&Note::NoFeed));
        // rapids never need a feed, an arc does
        assert!(!check(&lines("G90\nG0 X1\nG0 X2"), 1)
            .notes
            .contains(&Note::NoFeed));
        assert!(check(&lines("G90\nG0 X1\nG2 X2 Y2 I1 J0"), 1)
            .notes
            .contains(&Note::NoFeed));
    }

    #[test]
    fn a_line_that_goes_on_in_an_arc_without_saying_so_cannot_be_started_from() {
        let p = lines("G21 G90 F100\nG1 X0 Y0\nG2 X10 Y0 I5 J0\nX20 Y0 I5 J0\nG1 X30\nM30");
        assert_eq!(check(&p, 3).refusal, Some(Refusal::Motion(3)));
        // the line that says its own way of moving is fine, and so is the arc itself
        assert_eq!(check(&p, 2).refusal, None);
        assert_eq!(check(&p, 4).refusal, None);
        // a line that moves nothing comes first: the arc after it is still taken without saying it
        let q = lines("G3 X1 Y1 I1 J0 F50\nM8\n(note)\nG92 X0\nX2 Y2 I1 J0");
        assert_eq!(state_refusal(&q, 1), Some(Refusal::Motion(1)));
        // after G80 a bare move has no way of moving at all
        let r = lines("G0 X0\nG80\nX5\nM30");
        assert_eq!(state_refusal(&r, 2), Some(Refusal::Motion(2)));
        assert_eq!(state_refusal(&r, 99), None);
    }

    #[test]
    fn a_feed_mode_and_a_table_offset_that_cannot_be_written_back_refuse_the_start() {
        let p = lines("G93\nG1 X1 F2\nG1 X2 F2\nG94\nG1 X3 F100");
        assert_eq!(check(&p, 2).refusal, Some(Refusal::FeedMode(1)));
        assert_eq!(check(&p, 4).refusal, None);
        assert_eq!(
            check(&lines("G95\nG1 X1 F0.1\nG1 X2"), 2).refusal,
            Some(Refusal::FeedMode(1))
        );
        let t = lines("G43 H2\nG0 Z5\nG49\nG0 Z1");
        assert_eq!(check(&t, 1).refusal, Some(Refusal::TableLength(1)));
        assert_eq!(check(&t, 3).refusal, None);
        assert_eq!(
            check(&lines("G43.2 H1\nG0 X1"), 1).refusal,
            Some(Refusal::TableLength(1))
        );
        assert_eq!(check(&lines("G43.1 Z-1\nG0 X1"), 1).refusal, None);
    }

    #[test]
    fn the_units_a_value_was_given_in_are_kept_after_the_program_changes_them() {
        let p = lines("G20 F10\nG43.1 Z-1\nG21\nG1 X1");
        assert_eq!(units_of_values(&p, 4), (true, true));
        assert_eq!(units_of_values(&p, 0), (false, false));
        let q = lines("G21 F100\nG20\nG43.1 Z-1");
        assert_eq!(units_of_values(&q, 99), (false, true));
    }

    #[test]
    fn an_empty_program_is_refused_and_a_line_past_the_end_is_the_last_one() {
        let c = check(&[], 3);
        assert_eq!((c.refusal, c.line), (Some(Refusal::Empty), 0));
        let p = lines("G90\nG1 X1 F10\nM30");
        let c = check(&p, 99);
        assert_eq!((c.line, c.text.as_str()), (3, "M30"));
        // the spindle off at M30 is not a note
        assert!(c.notes.is_empty());
    }
}
