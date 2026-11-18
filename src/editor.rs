//! The numbers behind the editor of the program: finding and replacing text, the places of the lines, and the notes that belong to each
//! line. Only text and numbers and no window, so every rule is tested on its own; the window (`gui/editwin.rs`) shows what is worked out
//! here. Nothing here moves a machine or writes a file.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::report::Note;

/// The most matches the editor keeps and marks (a word that is everywhere does not slow the window down).
pub const MAX_MATCHES: usize = 10_000;

/// Where `needle` is in `text`, as pairs of byte offsets (start, end), in order and not overlapping. Capitals are told apart only when
/// `match_case` is true. An empty needle has no matches.
pub fn find_all(text: &str, needle: &str, match_case: bool) -> Vec<(usize, usize)> {
    if needle.is_empty() {
        return Vec::new();
    }
    // lower case in ASCII keeps every byte offset where it is
    let (hay, pin) = if match_case {
        (text.to_string(), needle.to_string())
    } else {
        (text.to_ascii_lowercase(), needle.to_ascii_lowercase())
    };
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = hay[from..].find(&pin) {
        let start = from + i;
        let end = start + pin.len();
        out.push((start, end));
        if out.len() >= MAX_MATCHES {
            break;
        }
        from = end;
    }
    out
}

/// The first match that starts at `from` or later, or the first of all when there is none after it (the search goes round).
pub fn next_match(matches: &[(usize, usize)], from: usize) -> Option<(usize, usize)> {
    matches
        .iter()
        .find(|m| m.0 >= from)
        .or_else(|| matches.first())
        .copied()
}

/// The text with every match of `needle` replaced, and how many were. The text goes back as it was when nothing is found.
pub fn replace_all(text: &str, needle: &str, with: &str, match_case: bool) -> (String, usize) {
    let found = find_all(text, needle, match_case);
    if found.is_empty() {
        return (text.to_string(), 0);
    }
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for &(s, e) in &found {
        out.push_str(&text[at..s]);
        out.push_str(with);
        at = e;
    }
    out.push_str(&text[at..]);
    (out, found.len())
}

/// The text with the bytes `range` replaced by `with`; a range that is not inside the text (or does not start and end on a character)
/// changes nothing.
pub fn replace_range(text: &str, range: (usize, usize), with: &str) -> String {
    let (s, e) = range;
    if s > e || e > text.len() || !text.is_char_boundary(s) || !text.is_char_boundary(e) {
        return text.to_string();
    }
    format!("{}{}{}", &text[..s], with, &text[e..])
}

/// How many lines the text has (an empty text has none; a last line with nothing after its end counts only when it has text).
pub fn line_count(text: &str) -> usize {
    text.lines().count()
}

/// The byte offset where a line starts and the one where it ends (before its line break), counting the lines from 1. A line that is not
/// in the text gives nothing.
pub fn line_span(text: &str, line: usize) -> Option<(usize, usize)> {
    let want = line.checked_sub(1)?;
    let mut start = 0;
    for (i, l) in text.split_inclusive('\n').enumerate() {
        if i == want {
            let body = l.trim_end_matches(['\n', '\r']);
            return Some((start, start + body.len()));
        }
        start += l.len();
    }
    None
}

/// The line (from 1) a byte offset is on.
pub fn line_at(text: &str, offset: usize) -> usize {
    let end = offset.min(text.len());
    let end = (0..=end)
        .rev()
        .find(|i| text.is_char_boundary(*i))
        .unwrap_or(0);
    text[..end].bytes().filter(|b| *b == b'\n').count() + 1
}

/// The index of the character (not the byte) at a byte offset, which is what the text box counts in.
pub fn char_index(text: &str, byte: usize) -> usize {
    let end = byte.min(text.len());
    let end = (0..=end)
        .rev()
        .find(|i| text.is_char_boundary(*i))
        .unwrap_or(0);
    text[..end].chars().count()
}

/// The numbers of the lines for the margin: as many as the text has (at least one), each as wide as the biggest, one per row.
pub fn margin_numbers(text: &str) -> Vec<String> {
    let n = line_count(text).max(1) + usize::from(text.ends_with('\n'));
    let width = n.to_string().len();
    (1..=n).map(|i| format!("{i:>width$}")).collect()
}

/// The notes that belong to a line (counted from 1), those the check made for that line.
pub fn notes_of_line(notes: &[Note], line: usize) -> impl Iterator<Item = &Note> {
    notes
        .iter()
        .filter(move |n| n.line.is_some_and(|l| l + 1 == line))
}

/// A number typed to go to a line: spaces around it do not matter, and it has to be a whole number from 1.
pub fn parse_line(text: &str) -> Option<usize> {
    text.trim().parse::<usize>().ok().filter(|n| *n >= 1)
}

/// What the last search, replace or jump said, for the window to say in its language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Said {
    /// How many matches there are and the line (from 1) of the one that is selected.
    Found {
        count: usize,
        line: usize,
    },
    NotFound,
    Replaced(usize),
    /// The line asked for is not in the text (0 when what was typed is not a line number).
    NoLine(usize),
}

/// The editor of the program as it is being used: the text on its way, the boxes of the search, the notes of the check and the place to
/// select in the next frame. The text is the program only once the operator applies it.
#[derive(Debug, Clone, Default)]
pub struct EditState {
    pub text: String,
    /// The text as it was when the window opened or when it was last applied, to tell whether there is something to apply.
    pub applied: String,
    pub find: String,
    pub with: String,
    pub goto: String,
    pub match_case: bool,
    pub said: Option<Said>,
    /// What the check finds in the text, with the lines from 0 (see `report::notes`).
    pub notes: Vec<Note>,
    notes_of: Option<u64>,
    /// The byte offset where the next search starts.
    pub from: usize,
    /// The bytes (start, end) the window selects and shows in the next frame.
    pub select: Option<(usize, usize)>,
}

impl EditState {
    /// The editor for a program.
    pub fn new(program: &[String]) -> EditState {
        let text = program.join("\n");
        EditState {
            applied: text.clone(),
            text,
            ..EditState::default()
        }
    }

    /// Whether the text is not the program yet.
    pub fn dirty(&self) -> bool {
        self.text != self.applied
    }

    /// The text as the lines of a program.
    pub fn lines(&self) -> Vec<String> {
        self.text.lines().map(str::to_string).collect()
    }

    /// The text has been put in the program.
    pub fn mark_applied(&mut self) {
        self.applied = self.text.clone();
    }

    fn text_hash(&self) -> u64 {
        let mut h = DefaultHasher::new();
        self.text.hash(&mut h);
        h.finish()
    }

    /// Whether the notes were made for another text (the check is made again only then).
    pub fn notes_stale(&self) -> bool {
        self.notes_of != Some(self.text_hash())
    }

    /// Keeps the notes the check made for the text as it is now.
    pub fn set_notes(&mut self, notes: Vec<Note>) {
        self.notes = notes;
        self.notes_of = Some(self.text_hash());
    }

    /// Finds the next match from the place the last search ended (going round at the end) and selects it.
    pub fn find_next(&mut self) {
        let all = find_all(&self.text, &self.find, self.match_case);
        match next_match(&all, self.from) {
            Some(m) => {
                self.select = Some(m);
                self.from = m.1;
                self.said = Some(Said::Found {
                    count: all.len(),
                    line: line_at(&self.text, m.0),
                });
            }
            None => {
                self.select = None;
                self.said = Some(Said::NotFound);
            }
        }
    }

    /// Replaces the next match from the place the last search ended and goes on after what was put in.
    pub fn replace_next(&mut self) {
        let all = find_all(&self.text, &self.find, self.match_case);
        match next_match(&all, self.from) {
            Some(m) => {
                self.text = replace_range(&self.text, m, &self.with);
                self.from = m.0 + self.with.len();
                self.select = None;
                self.said = Some(Said::Replaced(1));
            }
            None => {
                self.select = None;
                self.said = Some(Said::NotFound);
            }
        }
    }

    /// Replaces every match.
    pub fn replace_everything(&mut self) {
        let (text, n) = replace_all(&self.text, &self.find, &self.with, self.match_case);
        self.select = None;
        if n == 0 {
            self.said = Some(Said::NotFound);
        } else {
            self.text = text;
            self.from = 0;
            self.said = Some(Said::Replaced(n));
        }
    }

    /// Selects a line (from 1), or says that the text has no such line.
    pub fn go_to(&mut self, line: usize) {
        match line_span(&self.text, line) {
            Some(span) => {
                self.select = Some(span);
                self.from = span.0;
                self.said = None;
            }
            None => {
                self.select = None;
                self.said = Some(Said::NoLine(line));
            }
        }
    }

    /// Goes to the line typed in the box.
    pub fn go_to_typed(&mut self) {
        match parse_line(&self.goto) {
            Some(n) => self.go_to(n),
            None => {
                self.select = None;
                self.said = Some(Said::NoLine(0));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::progcheck::Level;

    #[test]
    fn the_matches_are_found_in_order_and_capitals_count_only_when_asked() {
        let t = "G1 X10\ng1 x10\nG1 X10";
        assert_eq!(find_all(t, "G1", true), [(0, 2), (14, 16)]);
        assert_eq!(find_all(t, "G1", false), [(0, 2), (7, 9), (14, 16)]);
        assert_eq!(find_all(t, "x10", false).len(), 3);
        assert!(find_all(t, "", false).is_empty());
        assert!(find_all(t, "Z", true).is_empty());
        // matches do not overlap
        assert_eq!(find_all("aaaa", "aa", true), [(0, 2), (2, 4)]);
        // an accented letter keeps the offsets right
        let a = "Ü X1\nÜ X1";
        assert_eq!(find_all(a, "x1", false), [(3, 5), (9, 11)]);
        assert_eq!(&a[3..5], "X1");
    }

    #[test]
    fn a_search_goes_on_from_a_place_and_comes_round() {
        let m = [(0, 2), (7, 9), (14, 16)];
        assert_eq!(next_match(&m, 0), Some((0, 2)));
        assert_eq!(next_match(&m, 1), Some((7, 9)));
        assert_eq!(next_match(&m, 9), Some((14, 16)));
        // past the last one it starts again from the first
        assert_eq!(next_match(&m, 15), Some((0, 2)));
        assert_eq!(next_match(&[], 0), None);
    }

    #[test]
    fn everything_is_replaced_and_counted() {
        let (t, n) = replace_all("G1 X1\nG1 X2\ng1 X3", "G1", "G0", true);
        assert_eq!((t.as_str(), n), ("G0 X1\nG0 X2\ng1 X3", 2));
        let (t, n) = replace_all("G1 X1\nG1 X2\ng1 X3", "G1", "G0", false);
        assert_eq!((t.as_str(), n), ("G0 X1\nG0 X2\nG0 X3", 3));
        // with something longer, shorter or nothing
        assert_eq!(replace_all("X1 X1", "X1", "X100", true).0, "X100 X100");
        assert_eq!(replace_all("X100 Y1", "X100", "", true).0, " Y1");
        // nothing found: the same text and no count
        assert_eq!(
            replace_all("G1 X1", "Z", "Q", true),
            ("G1 X1".to_string(), 0)
        );
        assert_eq!(
            replace_all("G1 X1", "", "Q", true),
            ("G1 X1".to_string(), 0)
        );
        // what is put in is not searched again
        assert_eq!(replace_all("a", "a", "aa", true), ("aa".to_string(), 1));
    }

    #[test]
    fn one_range_is_replaced_and_a_bad_range_changes_nothing() {
        assert_eq!(replace_range("G1 X1", (3, 5), "Y22"), "G1 Y22");
        assert_eq!(replace_range("G1 X1", (0, 0), ">"), ">G1 X1");
        assert_eq!(replace_range("G1 X1", (3, 99), "Y"), "G1 X1");
        assert_eq!(replace_range("G1 X1", (4, 3), "Y"), "G1 X1");
        // inside a character
        assert_eq!(replace_range("Üx", (1, 2), "Y"), "Üx");
    }

    #[test]
    fn the_places_of_the_lines_are_counted_from_one() {
        let t = "G21\nG1 X1\r\nM30";
        assert_eq!(line_count(t), 3);
        assert_eq!(line_span(t, 1), Some((0, 3)));
        // the line break (even a Windows one) is not part of the line
        assert_eq!(line_span(t, 2), Some((4, 9)));
        assert_eq!(&t[4..9], "G1 X1");
        assert_eq!(line_span(t, 3), Some((11, 14)));
        assert_eq!(line_span(t, 0), None);
        assert_eq!(line_span(t, 4), None);
        assert_eq!(line_span("", 1), None);
        assert_eq!(line_at(t, 0), 1);
        assert_eq!(line_at(t, 4), 2);
        assert_eq!(line_at(t, 12), 3);
        assert_eq!(line_at(t, 999), 3);
        // a place inside a character reads as the one before it
        assert_eq!(line_at("Ü\nx", 1), 1);
        assert_eq!(char_index("Üx\nG1", 3), 2);
        assert_eq!(char_index("abc", 99), 3);
    }

    #[test]
    fn the_margin_has_a_number_for_each_row_and_as_wide_as_the_biggest() {
        assert_eq!(margin_numbers(""), ["1"]);
        assert_eq!(margin_numbers("a\nb"), ["1", "2"]);
        // a line break at the end gives one more row to type in
        assert_eq!(margin_numbers("a\nb\n"), ["1", "2", "3"]);
        let long = "x\n".repeat(11);
        let m = margin_numbers(&long);
        assert_eq!(m.len(), 12);
        assert_eq!((m[0].as_str(), m[11].as_str()), (" 1", "12"));
    }

    #[test]
    fn a_line_to_go_to_is_a_whole_number_from_one() {
        assert_eq!(parse_line(" 12 "), Some(12));
        assert_eq!(parse_line("1"), Some(1));
        assert_eq!(parse_line("0"), None);
        assert_eq!(parse_line("-3"), None);
        assert_eq!(parse_line("2.5"), None);
        assert_eq!(parse_line(""), None);
        assert_eq!(parse_line("x"), None);
    }

    fn editor(text: &str) -> EditState {
        EditState::new(&text.lines().map(str::to_string).collect::<Vec<_>>())
    }

    #[test]
    fn the_editor_starts_with_the_program_and_knows_when_it_has_changed() {
        let mut e = editor("G21\nG1 X1 F100\nM30");
        assert_eq!(e.text, "G21\nG1 X1 F100\nM30");
        assert!(!e.dirty());
        e.text.push_str("\n; end");
        assert!(e.dirty());
        assert_eq!(e.lines(), ["G21", "G1 X1 F100", "M30", "; end"]);
        e.mark_applied();
        assert!(!e.dirty());
        assert!(!editor("").dirty());
    }

    #[test]
    fn the_search_steps_through_the_matches_and_says_how_many_there_are() {
        let mut e = editor("G1 X1\nG1 X2\nG0 Z5\nG1 X3");
        e.find = "G1".to_string();
        e.find_next();
        assert_eq!(e.select, Some((0, 2)));
        assert_eq!(e.said, Some(Said::Found { count: 3, line: 1 }));
        e.find_next();
        assert_eq!(e.select, Some((6, 8)));
        assert_eq!(e.said, Some(Said::Found { count: 3, line: 2 }));
        e.find_next();
        assert_eq!(e.said, Some(Said::Found { count: 3, line: 4 }));
        // after the last one it comes round to the first
        e.find_next();
        assert_eq!(e.select, Some((0, 2)));
        // nothing found selects nothing
        e.find = "Y9".to_string();
        e.find_next();
        assert_eq!((e.select, e.said.clone()), (None, Some(Said::NotFound)));
        // capitals
        e.find = "g1".to_string();
        e.match_case = true;
        e.find_next();
        assert_eq!(e.said, Some(Said::NotFound));
        e.match_case = false;
        e.find_next();
        assert!(matches!(e.said, Some(Said::Found { count: 3, .. })));
    }

    #[test]
    fn one_match_is_replaced_and_the_search_goes_on_after_it() {
        let mut e = editor("X1 X1 X1");
        e.find = "X1".to_string();
        e.with = "X10".to_string();
        e.replace_next();
        assert_eq!(e.text, "X10 X1 X1");
        assert_eq!(e.said, Some(Said::Replaced(1)));
        e.replace_next();
        assert_eq!(e.text, "X10 X10 X1");
        e.replace_next();
        assert_eq!(e.text, "X10 X10 X10");
        // what was put in is not searched again: the next one comes round to the first
        e.find = "X10".to_string();
        e.with = String::new();
        e.replace_next();
        assert_eq!(e.text, " X10 X10");
        e.find = "Q".to_string();
        e.replace_next();
        assert_eq!(e.said, Some(Said::NotFound));
        assert_eq!(e.text, " X10 X10");
        assert!(e.dirty());
    }

    #[test]
    fn everything_is_replaced_at_once_and_counted() {
        let mut e = editor("G1 X1\nG1 X2");
        e.find = "G1".to_string();
        e.with = "G0".to_string();
        e.replace_everything();
        assert_eq!(e.text, "G0 X1\nG0 X2");
        assert_eq!(e.said, Some(Said::Replaced(2)));
        e.replace_everything();
        assert_eq!(e.said, Some(Said::NotFound));
        assert_eq!(e.text, "G0 X1\nG0 X2");
    }

    #[test]
    fn a_line_is_selected_by_its_number() {
        let mut e = editor("G21\nG1 X1\n\nM30");
        e.goto = " 2 ".to_string();
        e.go_to_typed();
        assert_eq!(e.select, Some((4, 9)));
        assert_eq!(e.said, None);
        // the search goes on from there
        e.find = "M30".to_string();
        e.find_next();
        assert_eq!(e.said, Some(Said::Found { count: 1, line: 4 }));
        // an empty line is a place to put the cursor
        e.go_to(3);
        assert_eq!(e.select, Some((10, 10)));
        // a line that is not there, and what is not a line
        e.go_to(9);
        assert_eq!((e.select, e.said.clone()), (None, Some(Said::NoLine(9))));
        e.goto = "two".to_string();
        e.go_to_typed();
        assert_eq!(e.said, Some(Said::NoLine(0)));
    }

    #[test]
    fn the_notes_are_made_again_only_when_the_text_changes() {
        let mut e = editor("G1 X1");
        assert!(e.notes_stale());
        e.set_notes(Vec::new());
        assert!(!e.notes_stale());
        e.text.push('2');
        assert!(e.notes_stale());
        e.set_notes(Vec::new());
        assert!(!e.notes_stale());
    }

    #[test]
    fn the_notes_are_given_to_their_lines() {
        let note = |line: Option<usize>, text: &str| Note {
            line,
            level: Level::Warning,
            problem: true,
            text: text.to_string(),
        };
        let notes = [
            note(Some(1), "a"),
            note(None, "whole"),
            note(Some(1), "b"),
            note(Some(4), "c"),
        ];
        let on_two: Vec<_> = notes_of_line(&notes, 2).map(|n| n.text.as_str()).collect();
        assert_eq!(on_two, ["a", "b"]);
        assert_eq!(notes_of_line(&notes, 5).count(), 1);
        assert_eq!(notes_of_line(&notes, 1).count(), 0);
        assert_eq!(notes_of_line(&notes, 0).count(), 0);
    }
}
