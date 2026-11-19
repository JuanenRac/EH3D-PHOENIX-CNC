//! Bookmarks on the lines of the program: a short list of places to come back to (the start of a pocket, the line a tool change is at, the
//! place a run is meant to start from). The lines are counted from 0, as the program panel counts them. The list belongs to the program
//! that is loaded: it is dropped when another one is, and cut back when the program becomes shorter. It knows nothing of the window.

/// The most bookmarks a program keeps.
pub const MAX: usize = 200;

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
}
