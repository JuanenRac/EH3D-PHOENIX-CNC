//! Bookmarks on the lines of the program: a short list of places to come back to (the start of a pocket, the line a tool change is at, the
//! place a run is meant to start from). The lines are counted from 0, as the program panel counts them. The list belongs to the program
//! that is loaded: it is dropped when another one is, and cut back when the program becomes shorter. It knows nothing of the window.
//!
//! The list can be kept between sessions in a small text file next to the program (`part.nc.marks` for `part.nc`): the lines counted from 1
//! and the fingerprint (SHA-256) of the program they belong to. A file whose fingerprint is not the program's (the program was edited
//! since) is left out, so a bookmark never lands on a line it was not made for. The file is read only up to `MAX_FILE_BYTES`, and a file
//! that is there is replaced only when the caller says so.

use std::path::{Path, PathBuf};

/// The most bookmarks a program keeps.
pub const MAX: usize = 200;

/// The biggest bookmarks file that is read.
pub const MAX_FILE_BYTES: u64 = 16 * 1024;

/// What reading a bookmarks file gave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Loaded {
    /// There is no file.
    Missing,
    /// The bookmarks of the file, for this program.
    Marks(Bookmarks),
    /// The file belongs to another version of the program and was left out.
    Stale,
    /// The file is too big, cannot be read or is not a bookmarks file.
    Unusable,
}

/// Why the bookmarks could not be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveError {
    /// The file is there and nobody said it could be replaced.
    Exists,
    /// The system refused (the text of its error).
    Io(String),
}

/// What writing the bookmarks did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Saved {
    Written,
    /// The file had these very bookmarks already, so nothing was written.
    Same,
}

/// Where the bookmarks of a program are kept: next to it, with `.marks` added to its whole name.
pub fn marks_path(program: &Path) -> PathBuf {
    let mut name = program
        .file_name()
        .map_or_else(|| std::ffi::OsString::from("program"), |n| n.to_os_string());
    name.push(".marks");
    program.with_file_name(name)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bookmarks {
    /// The lines, from 0, in order and without repeats.
    lines: Vec<usize>,
}

impl Bookmarks {
    /// The bookmarked lines, in order.
    pub fn lines(&self) -> &[usize] {
        &self.lines
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub fn contains(&self, line: usize) -> bool {
        self.lines.binary_search(&line).is_ok()
    }

    /// Puts a bookmark on a line, or takes it away when there is one. Says whether the line is bookmarked now (false also when the list is
    /// full and the line could not be added).
    pub fn toggle(&mut self, line: usize) -> bool {
        match self.lines.binary_search(&line) {
            Ok(i) => {
                self.lines.remove(i);
                false
            }
            Err(i) if self.lines.len() < MAX => {
                self.lines.insert(i, line);
                true
            }
            Err(_) => false,
        }
    }

    /// Takes the bookmark of a line away. Says whether there was one.
    pub fn remove(&mut self, line: usize) -> bool {
        match self.lines.binary_search(&line) {
            Ok(i) => {
                self.lines.remove(i);
                true
            }
            Err(_) => false,
        }
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }

    /// Puts a bookmark on a line when there is none. Says whether one was added (false when the line had one or the list is full).
    pub fn add(&mut self, line: usize) -> bool {
        match self.lines.binary_search(&line) {
            Ok(_) => false,
            Err(i) if self.lines.len() < MAX => {
                self.lines.insert(i, line);
                true
            }
            Err(_) => false,
        }
    }

    /// The text of the file for these bookmarks of the program `program`.
    pub fn to_text(&self, program: &[String]) -> String {
        let mut s = String::from(
            "# Bookmarks of a program: the lines count from 1 and belong to the program with this fingerprint.\n",
        );
        s.push_str(&format!(
            "program = {}\n",
            crate::recovery::program_hash(program)
        ));
        for &l in &self.lines {
            s.push_str(&format!("mark = {}\n", l + 1));
        }
        s
    }

    /// Reads the text of a file for the program `program`: the bookmarks when it was written for this very program, `Stale` when it was
    /// written for another, `Unusable` when it names no program. Lines that are not marks, and marks past the end of the program, are left
    /// out, and there are never more than `MAX`.
    pub fn parse(text: &str, program: &[String]) -> Loaded {
        let mut fingerprint: Option<&str> = None;
        let mut found = Bookmarks::default();
        for line in text.trim_start_matches('\u{feff}').lines() {
            let line = line.trim();
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            match k.trim() {
                "program" => fingerprint = Some(v.trim()),
                "mark" => {
                    if let Some(n) = v
                        .trim()
                        .parse::<usize>()
                        .ok()
                        .filter(|n| (1..=program.len()).contains(n))
                    {
                        found.add(n - 1);
                    }
                }
                _ => {}
            }
        }
        match fingerprint {
            None => Loaded::Unusable,
            Some(f) if f != crate::recovery::program_hash(program) => Loaded::Stale,
            Some(_) => Loaded::Marks(found),
        }
    }

    /// Reads the bookmarks file of a program (see `parse`); a file bigger than `MAX_FILE_BYTES` is not read.
    pub fn load(path: &Path, program: &[String]) -> Loaded {
        match std::fs::metadata(path) {
            Err(_) => Loaded::Missing,
            Ok(m) if m.len() > MAX_FILE_BYTES => Loaded::Unusable,
            Ok(_) => match std::fs::read(path) {
                Ok(b) => Bookmarks::parse(&String::from_utf8_lossy(&b), program),
                Err(_) => Loaded::Unusable,
            },
        }
    }

    /// Writes the bookmarks next to the program. A file that is there is replaced only when `replace` is true (`Exists` otherwise), and one
    /// that has these very bookmarks is left alone (`Same`).
    pub fn save(&self, path: &Path, program: &[String], replace: bool) -> Result<Saved, SaveError> {
        use std::io::Write;
        let text = self.to_text(program);
        if std::fs::read_to_string(path).is_ok_and(|old| old == text) {
            return Ok(Saved::Same);
        }
        let mut open = std::fs::OpenOptions::new();
        open.write(true);
        if replace {
            open.create(true).truncate(true);
        } else {
            open.create_new(true);
        }
        let mut f = open.open(path).map_err(|e| match e.kind() {
            std::io::ErrorKind::AlreadyExists => SaveError::Exists,
            _ => SaveError::Io(e.to_string()),
        })?;
        f.write_all(text.as_bytes())
            .map_err(|e| SaveError::Io(e.to_string()))?;
        Ok(Saved::Written)
    }

    /// The first bookmark after a line (from the first one when there is no line), going round to the first at the end.
    pub fn next_after(&self, line: Option<usize>) -> Option<usize> {
        let first = *self.lines.first()?;
        match line {
            None => Some(first),
            Some(l) => Some(self.lines.iter().copied().find(|&b| b > l).unwrap_or(first)),
        }
    }

    /// The last bookmark before a line (from the last one when there is no line), going round to the last at the start.
    pub fn prev_before(&self, line: Option<usize>) -> Option<usize> {
        let last = *self.lines.last()?;
        match line {
            None => Some(last),
            Some(l) => Some(
                self.lines
                    .iter()
                    .rev()
                    .copied()
                    .find(|&b| b < l)
                    .unwrap_or(last),
            ),
        }
    }

    /// Drops the bookmarks of lines the program no longer has.
    pub fn cut_to(&mut self, program_len: usize) {
        self.lines.retain(|&l| l < program_len);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_is_bookmarked_and_unbookmarked_by_the_same_call_and_the_list_stays_in_order() {
        let mut b = Bookmarks::default();
        assert!(b.is_empty());
        assert!(b.toggle(7));
        assert!(b.toggle(2));
        assert!(b.toggle(40));
        assert_eq!(b.lines(), [2, 7, 40]);
        assert!(b.contains(7) && !b.contains(8));
        assert!(!b.toggle(7));
        assert_eq!(b.lines(), [2, 40]);
        assert!(b.remove(2) && !b.remove(2));
        assert_eq!((b.len(), b.is_empty()), (1, false));
        b.clear();
        assert!(b.is_empty());
    }

    #[test]
    fn the_next_and_the_previous_go_round_the_ends() {
        let mut b = Bookmarks::default();
        assert_eq!((b.next_after(None), b.prev_before(Some(3))), (None, None));
        for l in [5, 10, 20] {
            b.toggle(l);
        }
        assert_eq!(b.next_after(None), Some(5));
        assert_eq!(b.next_after(Some(5)), Some(10));
        assert_eq!(b.next_after(Some(7)), Some(10));
        assert_eq!(b.next_after(Some(20)), Some(5));
        assert_eq!(b.next_after(Some(99)), Some(5));
        assert_eq!(b.prev_before(None), Some(20));
        assert_eq!(b.prev_before(Some(20)), Some(10));
        assert_eq!(b.prev_before(Some(12)), Some(10));
        assert_eq!(b.prev_before(Some(5)), Some(20));
        assert_eq!(b.prev_before(Some(0)), Some(20));
    }

    #[test]
    fn there_is_a_limit_and_a_shorter_program_cuts_the_list() {
        let mut b = Bookmarks::default();
        for l in 0..MAX {
            assert!(b.toggle(l));
        }
        // full: another line is not added, and one that is there can still be taken away
        assert!(!b.toggle(MAX + 5));
        assert_eq!(b.len(), MAX);
        assert!(!b.toggle(3));
        assert_eq!(b.len(), MAX - 1);
        b.cut_to(10);
        assert_eq!(b.lines(), [0, 1, 2, 4, 5, 6, 7, 8, 9]);
        b.cut_to(0);
        assert!(b.is_empty());
    }

    fn program(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("G1 X{i} F100")).collect()
    }

    fn marked(lines: &[usize]) -> Bookmarks {
        let mut b = Bookmarks::default();
        for &l in lines {
            assert!(b.add(l));
        }
        b
    }

    #[test]
    fn add_puts_a_bookmark_only_when_there_is_none_and_there_is_room() {
        let mut b = Bookmarks::default();
        assert!(b.add(5) && b.add(2));
        assert!(!b.add(5));
        assert_eq!(b.lines(), [2, 5]);
        for l in 10..10 + MAX {
            b.add(l);
        }
        assert_eq!(b.len(), MAX);
        assert!(!b.add(9999));
    }

    #[test]
    fn the_bookmarks_file_is_next_to_the_program_with_its_whole_name() {
        let p = Path::new("jobs").join("part.nc");
        assert_eq!(marks_path(&p), Path::new("jobs").join("part.nc.marks"));
        assert_eq!(marks_path(Path::new("a")), Path::new("a.marks"));
        assert_eq!(marks_path(Path::new("")), Path::new("program.marks"));
    }

    #[test]
    fn the_text_of_the_file_reads_back_for_the_same_program_and_not_for_another() {
        let p = program(30);
        let b = marked(&[0, 7, 29]);
        let text = b.to_text(&p);
        assert!(
            text.contains("mark = 1\n")
                && text.contains("mark = 8\n")
                && text.contains("mark = 30\n")
        );
        assert_eq!(Bookmarks::parse(&text, &p), Loaded::Marks(b.clone()));
        // a program that was edited is another one: the bookmarks are left out
        let mut edited = p.clone();
        edited[3] = "G1 X999".to_string();
        assert_eq!(Bookmarks::parse(&text, &edited), Loaded::Stale);
        // a file with no program in it is not a bookmarks file
        assert_eq!(Bookmarks::parse("mark = 3\n", &p), Loaded::Unusable);
        assert_eq!(Bookmarks::parse("", &p), Loaded::Unusable);
    }

    #[test]
    fn marks_that_are_not_valid_or_past_the_end_are_left_out() {
        let p = program(5);
        let hash = crate::recovery::program_hash(&p);
        let text = format!(
            "\u{feff}# comment\nprogram = {hash}\nmark = 0\nmark = 2\nmark = 2\nmark = 5\nmark = 6\nmark = x\nmark = -1\nwhatever\nother = 3\n"
        );
        assert_eq!(Bookmarks::parse(&text, &p), Loaded::Marks(marked(&[1, 4])));
        // never more than the limit, whatever the file says
        let long = program(1000);
        let mut many = format!("program = {}\n", crate::recovery::program_hash(&long));
        for i in 1..=500 {
            many.push_str(&format!("mark = {i}\n"));
        }
        let Loaded::Marks(b) = Bookmarks::parse(&many, &long) else {
            panic!("not read");
        };
        assert_eq!(b.len(), MAX);
    }

    #[test]
    fn the_file_is_written_once_and_a_file_that_is_there_is_replaced_only_when_asked() {
        let dir = std::env::temp_dir().join(format!("eh3d-marks-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = marks_path(&dir.join("part.nc"));
        let p = program(20);
        // nothing there yet
        assert_eq!(Bookmarks::load(&file, &p), Loaded::Missing);
        let first = marked(&[3, 9]);
        assert_eq!(first.save(&file, &p, false), Ok(Saved::Written));
        assert_eq!(Bookmarks::load(&file, &p), Loaded::Marks(first.clone()));
        // the same bookmarks again: nothing is written, nothing is asked
        assert_eq!(first.save(&file, &p, false), Ok(Saved::Same));
        // other bookmarks: the file is there, so it is not written over unless the caller says so
        let second = marked(&[1]);
        assert_eq!(second.save(&file, &p, false), Err(SaveError::Exists));
        assert_eq!(Bookmarks::load(&file, &p), Loaded::Marks(first));
        assert_eq!(second.save(&file, &p, true), Ok(Saved::Written));
        assert_eq!(Bookmarks::load(&file, &p), Loaded::Marks(second.clone()));
        // the file is not left with the tail of a longer one
        assert_eq!(std::fs::read_to_string(&file).unwrap(), second.to_text(&p));
        // a program that was edited since: the file is stale
        let mut edited = p.clone();
        edited.push("M30".to_string());
        assert_eq!(Bookmarks::load(&file, &edited), Loaded::Stale);
        // a file that is too big is not read
        std::fs::write(&file, vec![b'#'; MAX_FILE_BYTES as usize + 1]).unwrap();
        assert_eq!(Bookmarks::load(&file, &p), Loaded::Unusable);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
