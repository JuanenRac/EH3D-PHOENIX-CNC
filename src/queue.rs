//! A queue of programs that run one after the other. It knows nothing of the window or of the controller: it only keeps the order, the state
//! of each program and the rule that the queue stops when a program does not finish. The window loads each program, shows its summary before
//! it starts (like any other start) and tells the queue how it ended.

use std::path::{Path, PathBuf};

/// The first line of the file the queue is kept in, with the version of its form.
pub const STATE_HEADER: &str = "phoenix-cnc-queue 1";
/// The most programs the file keeps.
pub const MAX_STATE_ITEMS: usize = 256;

/// A number that changes when any byte of the text changes (FNV-1a), for the check line of the state file.
fn fnv(text: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

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
    /// How many times the program runs, and the run it is at (from 1) while it is the loaded one.
    pub repeat: u32,
    pub round: u32,
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
                    repeat: 1,
                    round: 1,
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

    /// Sets how many times a program that waits runs, from 1 to `max`. Nothing for a program that has begun or is over.
    pub fn set_repeat(&mut self, i: usize, n: u32, max: u32) -> bool {
        match self.items.get_mut(i) {
            Some(item) if item.state == State::Waiting => {
                item.repeat = n.clamp(1, max.max(1));
                true
            }
            _ => false,
        }
    }

    /// The loaded program has more runs to do: the next one begins (the program stays the loaded one) and this says `true`. `false` when
    /// the run that ended was the last or nothing is loaded.
    pub fn next_round(&mut self) -> bool {
        let Some(i) = self.current() else {
            return false;
        };
        let item = &mut self.items[i];
        if item.round < item.repeat {
            item.round += 1;
            true
        } else {
            false
        }
    }

    /// The run of the loaded program and how many it has: `(2, 5)` is the second of five.
    pub fn round_of_current(&self) -> Option<(u32, u32)> {
        self.current()
            .map(|i| (self.items[i].round, self.items[i].repeat))
    }

    /// The runs still to do of each program that has some: all the runs of one that waits, the rest of the ones of the loaded program
    /// (counting the one in progress).
    pub fn remaining_runs(&self) -> Vec<(usize, u32)> {
        self.items
            .iter()
            .enumerate()
            .filter_map(|(i, it)| match it.state {
                State::Waiting => Some((i, it.repeat)),
                State::Running => Some((i, it.repeat.saturating_sub(it.round - 1).max(1))),
                _ => None,
            })
            .collect()
    }

    /// The text kept between runs: the programs that are still to run with the runs each has left. A queue with nothing to run gives an
    /// empty text. The last line is a check of the lines before it, so a file that was cut or changed by hand is not taken back.
    pub fn to_state(&self) -> String {
        let pending = self.remaining_runs();
        if pending.is_empty() {
            return String::new();
        }
        let mut s = format!("{STATE_HEADER}\n");
        for (i, runs) in pending.into_iter().take(MAX_STATE_ITEMS) {
            let path = self.items[i].path.display().to_string();
            s.push_str(&format!(
                "item={runs}|{}\n",
                path.replace(['\r', '\n'], " ")
            ));
        }
        s.push_str(&format!("check={:016x}\n", fnv(&s)));
        s
    }

    /// Reads what [`Queue::to_state`] wrote: every program waits (nothing starts by itself) with the runs it had left, at most `repeat_max`.
    /// `None` for a text that is not one, that was cut or changed, or that has no program.
    pub fn from_state(text: &str, repeat_max: u32) -> Option<Queue> {
        let body_end = text.rfind("check=")?;
        let (body, tail) = text.split_at(body_end);
        let sum = tail.trim().strip_prefix("check=")?;
        if u64::from_str_radix(sum, 16).ok()? != fnv(body) || !tail.ends_with('\n') {
            return None;
        }
        let mut lines = body.lines();
        if lines.next()? != STATE_HEADER {
            return None;
        }
        let mut q = Queue::default();
        for line in lines {
            let (runs, path) = line.strip_prefix("item=")?.split_once('|')?;
            if path.trim().is_empty() || q.items.len() >= MAX_STATE_ITEMS {
                return None;
            }
            q.items.push(Item {
                path: PathBuf::from(path),
                state: State::Waiting,
                repeat: runs.trim().parse::<u32>().ok()?.clamp(1, repeat_max.max(1)),
                round: 1,
            });
        }
        (!q.items.is_empty()).then_some(q)
    }

    /// Writes the state to a side file and renames it, so a crash never leaves half of it; a queue with nothing to run takes the file away.
    pub fn save_state(&self, file: &Path) -> std::io::Result<()> {
        let text = self.to_state();
        if text.is_empty() {
            return match std::fs::remove_file(file) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            };
        }
        let tmp = file.with_extension("tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, file)
    }

    /// Reads the state file. Programs whose file is not there any more are left out; `None` when the file is missing, too big, not a state,
    /// or nothing is left.
    pub fn load_state(file: &Path, repeat_max: u32) -> Option<Queue> {
        if std::fs::metadata(file).ok()?.len() > 1024 * 1024 {
            return None;
        }
        let mut q = Queue::from_state(&std::fs::read_to_string(file).ok()?, repeat_max)?;
        q.items.retain(|i| i.path.is_file());
        (!q.items.is_empty()).then_some(q)
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

    #[test]
    fn a_program_runs_as_many_times_as_it_is_told_and_only_while_it_waits() {
        let mut q = q(2);
        assert_eq!(q.items[0].repeat, 1);
        assert!(q.set_repeat(0, 3, 100));
        // 0 counts as 1 and more than the most is brought down to it
        assert!(q.set_repeat(1, 0, 100));
        assert_eq!(q.items[1].repeat, 1);
        assert!(q.set_repeat(1, 500, 10));
        assert_eq!(q.items[1].repeat, 10);
        assert!(!q.set_repeat(9, 2, 10));
        assert_eq!(q.begin_next(), Some(0));
        assert!(
            !q.set_repeat(0, 9, 100),
            "the one that runs keeps its count"
        );
        assert_eq!(q.round_of_current(), Some((1, 3)));
        assert!(q.next_round());
        assert_eq!(q.round_of_current(), Some((2, 3)));
        assert!(q.next_round());
        assert!(!q.next_round(), "the third was the last");
        assert_eq!(q.finish(true), Some(0));
        assert!(!q.next_round(), "nothing is loaded");
        assert_eq!(q.round_of_current(), None);
    }

    #[test]
    fn the_runs_left_count_the_one_in_progress() {
        let mut q = q(3);
        q.set_repeat(0, 3, 100);
        q.set_repeat(1, 2, 100);
        assert_eq!(q.remaining_runs(), vec![(0, 3), (1, 2), (2, 1)]);
        q.begin_next();
        q.next_round();
        // the second of three is in progress: it and one more are left
        assert_eq!(q.remaining_runs(), vec![(0, 2), (1, 2), (2, 1)]);
        q.skip(1);
        q.finish(true);
        assert_eq!(q.remaining_runs(), vec![(2, 1)]);
    }

    #[test]
    fn an_unstarted_program_goes_on_from_the_same_run() {
        let mut q = q(1);
        q.set_repeat(0, 4, 100);
        q.begin_next();
        q.next_round();
        q.next_round();
        q.unstart();
        assert_eq!(q.begin_next(), Some(0));
        assert_eq!(q.round_of_current(), Some((3, 4)));
    }

    fn real_files(tag: &str, n: usize) -> Vec<PathBuf> {
        let d = std::env::temp_dir().join(format!("eh3d_queue_state_{}_{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        (0..n)
            .map(|i| {
                let p = d.join(format!("job{i}.nc"));
                std::fs::write(&p, "G1 X1 F100\n").unwrap();
                p
            })
            .collect()
    }

    #[test]
    fn the_state_is_written_and_read_back_with_the_runs_left() {
        let mut a = Queue::new(real_files("rt", 3));
        a.set_repeat(0, 4, 100);
        a.set_repeat(2, 2, 100);
        a.begin_next();
        a.next_round();
        a.finish(true);
        let text = a.to_state();
        assert!(text.starts_with("phoenix-cnc-queue 1\n"));
        let b = Queue::from_state(&text, 100).unwrap();
        // the program that was done is not kept; the others wait, nothing is active
        assert_eq!(b.items.len(), 2);
        assert!(!b.active && b.items.iter().all(|i| i.state == State::Waiting));
        assert_eq!(b.items[0].path, a.items[1].path);
        assert_eq!((b.items[0].repeat, b.items[1].repeat), (1, 2));
    }

    #[test]
    fn a_program_in_progress_comes_back_with_the_runs_it_still_needs() {
        let mut a = Queue::new(real_files("prog", 1));
        a.set_repeat(0, 5, 100);
        a.begin_next();
        a.next_round();
        a.next_round();
        let b = Queue::from_state(&a.to_state(), 100).unwrap();
        assert_eq!(b.items[0].repeat, 3);
        assert_eq!(b.items[0].round, 1);
    }

    #[test]
    fn a_cut_or_changed_state_is_not_taken_back() {
        let a = Queue::new(real_files("bad", 2));
        let text = a.to_state();
        assert!(Queue::from_state(&text, 100).is_some());
        // a program added by hand
        let edited = text.replacen("item=1|", "item=9|", 1);
        assert!(Queue::from_state(&edited, 100).is_none());
        // cut in the middle
        assert!(Queue::from_state(&text[..text.len() - 10], 100).is_none());
        assert!(Queue::from_state("", 100).is_none());
        assert!(Queue::from_state("check=0000000000000000\n", 100).is_none());
        // another header with a right check
        let other = "other 1\nitem=1|a.nc\n";
        let signed = format!("{other}check={:016x}\n", fnv(other));
        assert!(Queue::from_state(&signed, 100).is_none());
        // a line that is not an item
        let odd = format!("{STATE_HEADER}\nitem=1|a.nc\nsomething\n");
        let signed = format!("{odd}check={:016x}\n", fnv(&odd));
        assert!(Queue::from_state(&signed, 100).is_none());
        // no program
        let none = format!("{STATE_HEADER}\n");
        let signed = format!("{none}check={:016x}\n", fnv(&none));
        assert!(Queue::from_state(&signed, 100).is_none());
    }

    #[test]
    fn the_runs_in_the_state_are_held_to_the_most_the_settings_allow() {
        let body = format!("{STATE_HEADER}\nitem=99999|a.nc\nitem=0|b.nc\n");
        let signed = format!("{body}check={:016x}\n", fnv(&body));
        let q = Queue::from_state(&signed, 20).unwrap();
        assert_eq!((q.items[0].repeat, q.items[1].repeat), (20, 1));
    }

    #[test]
    fn a_path_with_a_bar_or_spaces_survives_and_a_line_break_cannot_add_an_item() {
        let q = Queue::new([
            PathBuf::from("C:/my jobs/a|b.nc"),
            PathBuf::from("x\nitem=5|evil.nc"),
        ]);
        let back = Queue::from_state(&q.to_state(), 100).unwrap();
        assert_eq!(back.items.len(), 2);
        assert_eq!(back.items[0].path, PathBuf::from("C:/my jobs/a|b.nc"));
        assert_eq!(back.items[1].repeat, 1);
    }

    #[test]
    fn the_state_file_is_written_read_and_taken_away() {
        let files = real_files("file", 2);
        let state = files[0].with_file_name("queue.state");
        let mut a = Queue::new(files.clone());
        a.save_state(&state).unwrap();
        let b = Queue::load_state(&state, 100).unwrap();
        assert_eq!(b.items.len(), 2);
        // a program whose file is gone is left out
        std::fs::remove_file(&files[0]).unwrap();
        assert_eq!(Queue::load_state(&state, 100).unwrap().items.len(), 1);
        std::fs::remove_file(&files[1]).unwrap();
        assert!(Queue::load_state(&state, 100).is_none());
        // a queue with nothing to run takes the file away, and taking it away twice is not an error
        a.skip(0);
        a.skip(1);
        a.save_state(&state).unwrap();
        assert!(!state.exists());
        a.save_state(&state).unwrap();
        // a file that is not there, and one that is too big
        assert!(Queue::load_state(&state, 100).is_none());
        std::fs::write(&state, vec![b'x'; 2 * 1024 * 1024]).unwrap();
        assert!(Queue::load_state(&state, 100).is_none());
    }

    #[test]
    fn no_more_programs_than_the_limit_are_kept() {
        let q = Queue::new((0..300).map(|i| PathBuf::from(format!("j{i}.nc"))));
        let back = Queue::from_state(&q.to_state(), 100).unwrap();
        assert_eq!(back.items.len(), MAX_STATE_ITEMS);
    }
}
