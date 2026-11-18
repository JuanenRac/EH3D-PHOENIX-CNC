//! The rules of a queue of programs that the plain queue does not have: how many times each program is repeated, and what happens between
//! two runs (the operator confirms, which is the default because the work has to be changed, or a wait of some seconds). It knows nothing
//! of the window or of the controller: the window loads each program, shows its summary like any other start and tells the runner how it
//! ended; the runner says what comes next. Nothing starts by itself unless the settings say there is no confirmation and a wait.

/// What comes between a run and the next one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum After {
    /// The next run starts at once.
    Nothing,
    /// The operator must confirm.
    Confirm,
    /// A wait in seconds.
    Wait(f64),
}

/// One program of the queue with the number of times it runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    /// The position of the program in the queue.
    pub item: usize,
    pub repeat: u32,
}

/// One run: the program of the queue and the round of its repeats (from 1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Step {
    pub item: usize,
    pub round: u32,
    /// What comes after this run (`Nothing` for the last one).
    pub after: After,
}

/// How runs follow each other.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rules {
    pub confirm_between: bool,
    pub pause_s: f64,
    /// The most repeats of one program.
    pub repeat_max: u32,
}

impl Rules {
    pub fn between(&self) -> After {
        if self.confirm_between {
            After::Confirm
        } else if self.pause_s.is_finite() && self.pause_s > 0.0 {
            After::Wait(self.pause_s)
        } else {
            After::Nothing
        }
    }
}

/// The runs in order: each entry repeated, with the pause or the confirmation between two runs. A repeat of 0 counts as 1 and one above
/// the limit is brought down to it.
pub fn expand(entries: &[Entry], rules: Rules) -> Vec<Step> {
    let cap = rules.repeat_max.max(1);
    let mut runs: Vec<(usize, u32)> = Vec::new();
    for e in entries {
        for round in 1..=e.repeat.clamp(1, cap) {
            runs.push((e.item, round));
        }
    }
    let last = runs.len().saturating_sub(1);
    runs.iter()
        .enumerate()
        .map(|(i, &(item, round))| Step {
            item,
            round,
            after: if i == last {
                After::Nothing
            } else {
                rules.between()
            },
        })
        .collect()
}

/// The time of the whole queue in seconds, given the time of each program of the queue (`times[item]`; a program with no time counts
/// 0) and the waits between runs. A confirmation adds nothing: nobody knows how long the operator takes.
pub fn total_seconds(steps: &[Step], times: &[f64]) -> f64 {
    steps
        .iter()
        .map(|s| {
            let run = times
                .get(s.item)
                .copied()
                .filter(|t| t.is_finite() && *t > 0.0)
                .unwrap_or(0.0);
            let wait = match s.after {
                After::Wait(w) => w,
                _ => 0.0,
            };
            run + wait
        })
        .sum()
}

/// What the window must do now.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Gate {
    /// Load and run this program (it shows its summary like any other start).
    Run { item: usize, round: u32 },
    /// Ask the operator to confirm before the next run.
    Confirm { next_item: usize },
    /// Wait this many seconds and then call `proceed`.
    Wait { seconds: f64, next_item: usize },
    /// Everything ran.
    Done,
    /// A program did not finish: nothing else runs.
    Halted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    Running,
    Between,
    Halted,
    Finished,
}

/// Walks the runs. It moves only when the window tells it that a run ended or that the operator confirmed.
#[derive(Debug, Clone, PartialEq)]
pub struct Runner {
    steps: Vec<Step>,
    pos: usize,
    phase: Phase,
}

impl Runner {
    pub fn new(steps: Vec<Step>) -> Runner {
        Runner {
            steps,
            pos: 0,
            phase: Phase::Idle,
        }
    }

    /// The run in progress.
    pub fn current(&self) -> Option<&Step> {
        (self.phase == Phase::Running).then(|| &self.steps[self.pos])
    }

    /// How many runs are left, the one in progress included.
    pub fn left(&self) -> usize {
        match self.phase {
            Phase::Idle => self.steps.len(),
            Phase::Running => self.steps.len() - self.pos,
            Phase::Between => self.steps.len() - self.pos - 1,
            Phase::Halted | Phase::Finished => 0,
        }
    }

    fn run_now(&mut self) -> Gate {
        self.phase = Phase::Running;
        let s = self.steps[self.pos];
        Gate::Run {
            item: s.item,
            round: s.round,
        }
    }

    /// Starts the first run (only from the beginning).
    pub fn start(&mut self) -> Gate {
        match self.phase {
            Phase::Idle if self.steps.is_empty() => {
                self.phase = Phase::Finished;
                Gate::Done
            }
            Phase::Idle => self.run_now(),
            _ => self.gate(),
        }
    }

    /// The gate for the present state, without moving.
    pub fn gate(&self) -> Gate {
        match self.phase {
            Phase::Idle | Phase::Finished => Gate::Done,
            Phase::Halted => Gate::Halted,
            Phase::Running => Gate::Run {
                item: self.steps[self.pos].item,
                round: self.steps[self.pos].round,
            },
            Phase::Between => {
                let next_item = self.steps[self.pos + 1].item;
                match self.steps[self.pos].after {
                    After::Wait(seconds) => Gate::Wait { seconds, next_item },
                    _ => Gate::Confirm { next_item },
                }
            }
        }
    }

    /// The run in progress ended; `ok` is false when the program did not finish (the queue stops).
    pub fn finish(&mut self, ok: bool) -> Gate {
        if self.phase != Phase::Running {
            return self.gate();
        }
        if !ok {
            self.phase = Phase::Halted;
            return Gate::Halted;
        }
        if self.pos + 1 >= self.steps.len() {
            self.phase = Phase::Finished;
            return Gate::Done;
        }
        match self.steps[self.pos].after {
            After::Nothing => {
                self.pos += 1;
                self.run_now()
            }
            _ => {
                self.phase = Phase::Between;
                self.gate()
            }
        }
    }

    /// The operator confirmed, or the wait ended: the next run is given (only from a pause between two runs).
    pub fn proceed(&mut self) -> Gate {
        if self.phase != Phase::Between {
            return self.gate();
        }
        self.pos += 1;
        self.run_now()
    }

    /// The operator cancels what is left.
    pub fn stop(&mut self) {
        if self.phase != Phase::Finished {
            self.phase = Phase::Halted;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAUTIOUS: Rules = Rules {
        confirm_between: true,
        pause_s: 0.0,
        repeat_max: 100,
    };

    fn e(item: usize, repeat: u32) -> Entry {
        Entry { item, repeat }
    }

    #[test]
    fn repeats_are_expanded_in_order_with_the_rounds() {
        let s = expand(&[e(0, 2), e(1, 1)], CAUTIOUS);
        let order: Vec<_> = s.iter().map(|s| (s.item, s.round)).collect();
        assert_eq!(order, vec![(0, 1), (0, 2), (1, 1)]);
        assert_eq!(s[0].after, After::Confirm);
        assert_eq!(s[2].after, After::Nothing);
    }

    #[test]
    fn zero_repeats_count_one_and_the_limit_is_kept() {
        assert_eq!(expand(&[e(0, 0)], CAUTIOUS).len(), 1);
        assert_eq!(expand(&[e(0, 5_000)], CAUTIOUS).len(), 100);
        let r = Rules {
            repeat_max: 0,
            ..CAUTIOUS
        };
        assert_eq!(expand(&[e(0, 7)], r).len(), 1);
        assert!(expand(&[], CAUTIOUS).is_empty());
    }

    #[test]
    fn between_two_runs_it_is_a_confirmation_a_wait_or_nothing() {
        let wait = Rules {
            confirm_between: false,
            pause_s: 12.5,
            ..CAUTIOUS
        };
        assert_eq!(expand(&[e(0, 2)], wait)[0].after, After::Wait(12.5));
        let none = Rules {
            confirm_between: false,
            pause_s: 0.0,
            ..CAUTIOUS
        };
        assert_eq!(expand(&[e(0, 2)], none)[0].after, After::Nothing);
        let nan = Rules {
            confirm_between: false,
            pause_s: f64::NAN,
            ..CAUTIOUS
        };
        assert_eq!(expand(&[e(0, 2)], nan)[0].after, After::Nothing);
        // the confirmation wins over a wait
        let both = Rules {
            confirm_between: true,
            pause_s: 30.0,
            ..CAUTIOUS
        };
        assert_eq!(expand(&[e(0, 2)], both)[0].after, After::Confirm);
    }

    #[test]
    fn the_total_time_adds_runs_and_waits_but_not_confirmations() {
        let wait = Rules {
            confirm_between: false,
            pause_s: 10.0,
            ..CAUTIOUS
        };
        let s = expand(&[e(0, 2), e(1, 1)], wait);
        // 2 x 60 + 30 and two waits of 10
        assert!((total_seconds(&s, &[60.0, 30.0]) - 170.0).abs() < 1e-9);
        let c = expand(&[e(0, 2), e(1, 1)], CAUTIOUS);
        assert!((total_seconds(&c, &[60.0, 30.0]) - 150.0).abs() < 1e-9);
        // a program with no known time, or a wrong one, counts 0
        assert!((total_seconds(&c, &[f64::NAN]) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn nothing_runs_until_it_is_started_and_each_run_waits_for_the_operator() {
        let mut r = Runner::new(expand(&[e(0, 2), e(1, 1)], CAUTIOUS));
        assert_eq!(r.left(), 3);
        assert_eq!(r.gate(), Gate::Done);
        assert_eq!(r.start(), Gate::Run { item: 0, round: 1 });
        assert_eq!(r.left(), 3);
        assert_eq!(r.finish(true), Gate::Confirm { next_item: 0 });
        assert_eq!(r.left(), 2);
        // a second finish while waiting changes nothing
        assert_eq!(r.finish(true), Gate::Confirm { next_item: 0 });
        assert_eq!(r.proceed(), Gate::Run { item: 0, round: 2 });
        assert_eq!(r.finish(true), Gate::Confirm { next_item: 1 });
        assert_eq!(r.proceed(), Gate::Run { item: 1, round: 1 });
        assert_eq!(r.finish(true), Gate::Done);
        assert_eq!(r.left(), 0);
        assert_eq!(r.proceed(), Gate::Done);
    }

    #[test]
    fn a_wait_is_given_with_its_seconds() {
        let wait = Rules {
            confirm_between: false,
            pause_s: 5.0,
            ..CAUTIOUS
        };
        let mut r = Runner::new(expand(&[e(3, 2)], wait));
        r.start();
        assert_eq!(
            r.finish(true),
            Gate::Wait {
                seconds: 5.0,
                next_item: 3
            }
        );
        assert_eq!(r.proceed(), Gate::Run { item: 3, round: 2 });
    }

    #[test]
    fn with_no_confirmation_and_no_wait_the_next_run_follows_at_once() {
        let none = Rules {
            confirm_between: false,
            pause_s: 0.0,
            ..CAUTIOUS
        };
        let mut r = Runner::new(expand(&[e(0, 2)], none));
        r.start();
        assert_eq!(r.finish(true), Gate::Run { item: 0, round: 2 });
        assert_eq!(r.finish(true), Gate::Done);
    }

    #[test]
    fn a_program_that_does_not_finish_stops_everything() {
        let mut r = Runner::new(expand(&[e(0, 3)], CAUTIOUS));
        r.start();
        assert_eq!(r.finish(false), Gate::Halted);
        assert_eq!(r.proceed(), Gate::Halted);
        assert_eq!(r.start(), Gate::Halted);
        assert_eq!(r.left(), 0);
        assert!(r.current().is_none());
    }

    #[test]
    fn the_operator_can_cancel_what_is_left_but_not_undo_what_ended() {
        let mut r = Runner::new(expand(&[e(0, 2)], CAUTIOUS));
        r.start();
        r.finish(true);
        r.stop();
        assert_eq!(r.proceed(), Gate::Halted);
        let mut done = Runner::new(expand(&[e(0, 1)], CAUTIOUS));
        done.start();
        done.finish(true);
        done.stop();
        assert_eq!(done.gate(), Gate::Done);
    }

    #[test]
    fn an_empty_queue_is_done_at_once_and_the_current_run_is_known() {
        let mut r = Runner::new(vec![]);
        assert_eq!(r.start(), Gate::Done);
        let mut q = Runner::new(expand(&[e(2, 1)], CAUTIOUS));
        assert!(q.current().is_none());
        q.start();
        assert_eq!(q.current().map(|s| s.item), Some(2));
    }
}
