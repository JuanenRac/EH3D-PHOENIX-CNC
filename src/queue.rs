//! A queue of programs that run one after the other. It knows nothing of the window or of the controller: it only keeps the order, the state
//! of each program and the rule that the queue stops when a program does not finish. The window loads each program, shows its summary before
//! it starts (like any other start) and tells the queue how it ended.

use std::path::{Path, PathBuf};

/// Where a program of the queue is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Waiting,
    /// Loaded: its summary is shown or it runs.
    Running,
    Done,
    /// It did not finish: a line was refused, the operator stopped it, or it could not start (the queue stops here).
    Failed,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub path: PathBuf,
    pub state: State,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Queue {
    pub items: Vec<Item>,
    /// The queue goes on by itself to the next program when one is done (false: not started yet, or stopped by a program that did not finish).
    pub active: bool,
}

impl Queue {
    pub fn new<I: IntoIterator<Item = PathBuf>>(paths: I) -> Queue {
        Queue {
            items: paths
                .into_iter()
                .map(|path| Item {
                    path,
                    state: State::Waiting,
                })
                .collect(),
            active: false,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The program that is loaded now.
    pub fn current(&self) -> Option<usize> {
        self.items.iter().position(|i| i.state == State::Running)
    }

    pub fn waiting(&self) -> usize {
        self.items
            .iter()
            .filter(|i| i.state == State::Waiting)
            .count()
    }

    /// The queue stopped before its end: a program did not finish (or was left before it started) and others wait.
    pub fn halted(&self) -> bool {
        !self.active && self.current().is_none() && self.waiting() > 0 && self.started()
    }

    fn started(&self) -> bool {
        self.items.iter().any(|i| i.state != State::Waiting)
    }

    /// The queue has gone through every program.
    pub fn finished(&self) -> bool {
        !self.items.is_empty()
            && self
                .items
                .iter()
                .all(|i| matches!(i.state, State::Done | State::Skipped | State::Failed))
            && self.current().is_none()
    }

    /// Takes the next program that waits as the one that is loaded and turns the queue on. `None` when there is none, or when a program is
    /// loaded already.
    pub fn begin_next(&mut self) -> Option<usize> {
        if self.current().is_some() {
            return None;
        }
        let i = self.items.iter().position(|i| i.state == State::Waiting)?;
        self.items[i].state = State::Running;
        self.active = true;
        Some(i)
    }

    /// The loaded program ended: done, or not finished (which stops the queue). Returns its place.
    pub fn finish(&mut self, ok: bool) -> Option<usize> {
        let i = self.current()?;
        self.items[i].state = if ok { State::Done } else { State::Failed };
        if !ok {
            self.active = false;
        }
        Some(i)
    }

    /// The loaded program was left before it started (its summary was closed): it waits again and the queue stops.
    pub fn unstart(&mut self) -> Option<usize> {
        let i = self.current()?;
        self.items[i].state = State::Waiting;
        self.active = false;
        Some(i)
    }

    /// Takes a program out of the queue. The one that is loaded cannot be taken out (skip it instead).
    pub fn remove(&mut self, i: usize) -> bool {
        match self.items.get(i) {
            Some(item) if item.state != State::Running => {
                self.items.remove(i);
                true
            }
            _ => false,
        }
    }

    /// Leaves a program out: one that waits is never run; the loaded one is given up (the caller stops it) and the queue goes on.
    pub fn skip(&mut self, i: usize) -> bool {
        match self.items.get_mut(i) {
            Some(item) if matches!(item.state, State::Waiting | State::Running) => {
                item.state = State::Skipped;
                true
            }
            _ => false,
        }
    }

    /// Moves a program that waits one place earlier, past another that waits (what is done or running keeps its place).
    pub fn move_up(&mut self, i: usize) -> bool {
        i > 0 && self.swap_waiting(i - 1, i)
    }

    /// Moves a program that waits one place later.
    pub fn move_down(&mut self, i: usize) -> bool {
        self.swap_waiting(i, i + 1)
    }

    fn swap_waiting(&mut self, a: usize, b: usize) -> bool {
        let both = |q: &Queue, i: usize| q.items.get(i).is_some_and(|x| x.state == State::Waiting);
        if both(self, a) && both(self, b) {
            self.items.swap(a, b);
            true
        } else {
            false
        }
    }

    /// Empties the queue except for the program that is loaded.
    pub fn clear(&mut self) {
        self.items.retain(|i| i.state == State::Running);
        if self.items.is_empty() {
            self.active = false;
        }
    }

    /// Turns the queue on again after it stopped.
    pub fn resume(&mut self) {
        if self.waiting() > 0 {
            self.active = true;
        }
    }

    /// The name of a program of the queue, for the list.
    pub fn name_of(&self, i: usize) -> String {
        self.items.get(i).map_or_else(String::new, |it| {
            Path::new(&it.path).file_name().map_or_else(
                || it.path.display().to_string(),
                |n| n.to_string_lossy().to_string(),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(n: usize) -> Queue {
        Queue::new((0..n).map(|i| PathBuf::from(format!("dir/job{i}.nc"))))
    }

    fn states(q: &Queue) -> Vec<State> {
        q.items.iter().map(|i| i.state).collect()
    }

    #[test]
    fn the_programs_run_in_order_and_the_queue_ends_when_all_are_done() {
        let mut q = q(3);
        assert!(!q.active && !q.halted() && !q.finished());
        for want in 0..3 {
            assert_eq!(q.begin_next(), Some(want));
            // one program at a time
            assert_eq!(q.begin_next(), None);
            assert_eq!(q.current(), Some(want));
            assert_eq!(q.finish(true), Some(want));
        }
        assert_eq!(q.begin_next(), None);
        assert!(q.finished() && !q.halted());
        assert_eq!(states(&q), vec![State::Done; 3]);
    }

    #[test]
    fn a_program_that_does_not_finish_stops_the_queue_until_the_operator_goes_on() {
        let mut q = q(3);
        q.begin_next();
        q.finish(true);
        q.begin_next();
        q.finish(false);
        assert!(!q.active && q.halted() && !q.finished());
        assert_eq!(states(&q), vec![State::Done, State::Failed, State::Waiting]);
        q.resume();
        assert!(q.active);
        assert_eq!(q.begin_next(), Some(2));
        q.finish(true);
        assert!(q.finished());
    }

    #[test]
    fn a_summary_that_is_closed_puts_the_program_back_and_stops_the_queue() {
        let mut q = q(2);
        q.begin_next();
        assert_eq!(q.unstart(), Some(0));
        assert_eq!(states(&q), vec![State::Waiting; 2]);
        assert!(!q.active && !q.halted(), "nothing ran yet");
        q.resume();
        assert_eq!(q.begin_next(), Some(0));
    }

    #[test]
    fn programs_that_wait_are_removed_skipped_and_reordered_but_not_the_one_that_runs() {
        let mut q = q(4);
        q.begin_next();
        assert!(!q.remove(0), "the loaded program stays");
        assert!(q.skip(2));
        // job1 and job3 wait with job2 skipped between them: they do not swap across a skipped one
        let before = q.clone();
        assert!(!q.move_down(1));
        assert_eq!(q, before);
        // a program that waits does not move above the loaded one
        assert!(!q.move_up(1));
        assert_eq!(q.items[0].path.file_name().unwrap(), "job0.nc");
        assert!(q.remove(2));
        assert_eq!(q.len_names(), vec!["job0.nc", "job1.nc", "job3.nc"]);
        assert!(q.move_down(1));
        assert_eq!(q.len_names(), vec!["job0.nc", "job3.nc", "job1.nc"]);
        assert!(q.move_up(2));
        assert_eq!(q.len_names(), vec!["job0.nc", "job1.nc", "job3.nc"]);
        assert!(!q.remove(9) && !q.skip(9) && !q.move_down(9));
    }

    #[test]
    fn skipping_the_loaded_program_lets_the_queue_go_on_with_the_next() {
        let mut q = q(2);
        q.begin_next();
        assert!(q.skip(0));
        assert_eq!(q.current(), None);
        assert!(q.active, "a skip does not stop the queue");
        assert_eq!(q.begin_next(), Some(1));
        q.finish(true);
        assert!(q.finished());
        assert_eq!(states(&q), vec![State::Skipped, State::Done]);
    }

    #[test]
    fn clearing_keeps_only_the_program_that_runs() {
        let mut q = q(3);
        q.begin_next();
        q.clear();
        assert_eq!(q.items.len(), 1);
        q.finish(true);
        q.clear();
        assert!(q.is_empty() && !q.active);
        assert_eq!(q.name_of(0), "");
    }

    impl Queue {
        fn len_names(&self) -> Vec<String> {
            (0..self.items.len()).map(|i| self.name_of(i)).collect()
        }
    }
}
