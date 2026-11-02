//! The queue of programs in the window: the programs picked in the Open window (or given on the command line) run one after the other. Each one
//! is loaded and goes through the same start as any program, so its summary comes first; the queue stops when a program does not finish.

use super::*;
use crate::queue::{Queue, State};

impl App {
    /// Takes the files as a queue and loads the first. One file is just opened.
    pub fn start_queue(&mut self, paths: Vec<PathBuf>) {
        match paths.len() {
            0 => {}
            1 => self.open(&paths[0]),
            _ => {
                if self.session.running || self.server_busy() {
                    self.session
                        .log
                        .push("queue: a program is running; stop it first".into());
                    return;
                }
                self.queue = Queue::new(paths);
                self.queue_seen = false;
                self.session
                    .log
                    .push(format!("queue: {} programs", self.queue.items.len()));
                self.queue_next();
            }
        }
    }

    /// Loads the next program of the queue and asks to start it (the summary comes first, unless the settings turn it off).
    pub(super) fn queue_next(&mut self) {
        self.queue_seen = false;
        let Some(i) = self.queue.begin_next() else {
            if self.queue.finished() {
                self.queue.active = false;
                self.session.log.push("queue: all programs are done".into());
            }
            return;
        };
        let path = self.queue.items[i].path.clone();
        if !self.load_file(&path) || self.session.program.iter().all(|l| l.trim().is_empty()) {
            self.queue.finish(false);
            self.session.log.push(format!(
                "queue: {} cannot be run; the queue stops",
                self.queue.name_of(i)
            ));
            return;
        }
        self.request_start();
    }

    /// Watches the program of the queue: when it ends the next one is loaded; when it does not finish, or its start is left, the queue stops.
    pub(super) fn queue_tick(&mut self) {
        let Some(i) = self.queue.current() else {
            return;
        };
        if self.session.running {
            self.queue_seen = true;
            return;
        }
        if self.queue_seen {
            let ok = self.session.next >= self.session.program.len();
            self.queue.finish(ok);
            if ok {
                self.queue_next();
            } else {
                self.session.log.push(format!(
                    "queue: {} did not finish; the queue stops",
                    self.queue.name_of(i)
                ));
            }
        } else if !matches!(self.win, Some(Win::Summary(_))) {
            // the start did not happen: a controller in alarm is a failure, a summary that was closed is the operator's choice
            if self.session.alarm() {
                self.queue.finish(false);
                self.session.log.push(format!(
                    "queue: {} could not start; the queue stops",
                    self.queue.name_of(i)
                ));
            } else {
                self.queue.unstart();
                self.session
                    .log
                    .push("queue: stopped before the start".into());
            }
        }
    }

    /// Skips program `i`: one that waits is never run; the loaded one is stopped and the next is loaded.
    pub(super) fn queue_skip(&mut self, i: usize) {
        let loaded = self.queue.current() == Some(i);
        if !self.queue.skip(i) {
            return;
        }
        if loaded {
            if self.session.running || self.session.paused {
                self.program_stop();
            }
            if matches!(self.win, Some(Win::Summary(_))) {
                self.win = None;
            }
            self.queue_seen = false;
            if self.queue.active {
                self.queue_next();
            }
        }
    }

    /// Goes on with the queue after it stopped.
    pub(super) fn queue_continue(&mut self) {
        self.queue.resume();
        self.queue_next();
    }

    /// The list of the queue in the program panel: the state of each program and the buttons to take it out, skip it and move it.
    pub(super) fn queue_panel(&mut self, ui: &mut egui::Ui) {
        if self.queue.is_empty() {
            return;
        }
        ui.add_space(4.0);
        ui.label(RichText::new(self.t(Text::McQueue).to_uppercase()).color(col(self.theme.accent)));
        let (mut remove, mut skip, mut up, mut down) = (None, None, None, None);
        let count = self.queue.items.len();
        egui::ScrollArea::vertical()
            .id_salt("queue")
            .max_height(110.0)
            .show(ui, |ui| {
                for i in 0..count {
                    let state = self.queue.items[i].state;
                    let (word, color) = match state {
                        State::Waiting => (self.t(Text::McQueueWaiting), self.theme.dim),
                        State::Running => (self.t(Text::McStRun), self.theme.accent),
                        State::Done => (self.t(Text::McQueueDone), self.theme.ok),
                        State::Failed => (self.t(Text::McQueueFailed), self.theme.danger),
                        State::Skipped => (self.t(Text::McQueueSkipped), self.theme.dim),
                    };
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{}.", i + 1)).monospace());
                        ui.label(RichText::new(word).color(col(color)));
                        ui.label(self.queue.name_of(i))
                            .on_hover_text(self.queue.items[i].path.display().to_string());
                        let waiting = state == State::Waiting;
                        if ui
                            .add_enabled(waiting && i > 0, egui::Button::new("^"))
                            .clicked()
                        {
                            up = Some(i);
                        }
                        if ui
                            .add_enabled(waiting && i + 1 < count, egui::Button::new("v"))
                            .clicked()
                        {
                            down = Some(i);
                        }
                        if ui
                            .add_enabled(
                                matches!(state, State::Waiting | State::Running),
                                egui::Button::new(">>"),
                            )
                            .on_hover_text(self.t(Text::McQueueSkip))
                            .clicked()
                        {
                            skip = Some(i);
                        }
                        if ui
                            .add_enabled(state != State::Running, egui::Button::new("x"))
                            .on_hover_text(self.t(Text::McQueueRemove))
                            .clicked()
                        {
                            remove = Some(i);
                        }
                    });
                }
            });
        let mut cont = false;
        let mut clear = false;
        if self.queue.halted() {
            ui.colored_label(col(self.theme.warn), self.t(Text::McQueueHalted));
            cont = ui.button(self.t(Text::McQueueContinue)).clicked();
        }
        if ui.button(self.t(Text::McQueueClear)).clicked() {
            clear = true;
        }
        if let Some(i) = up {
            self.queue.move_up(i);
        }
        if let Some(i) = down {
            self.queue.move_down(i);
        }
        if let Some(i) = skip {
            self.queue_skip(i);
        }
        if let Some(i) = remove {
            self.queue.remove(i);
        }
        if clear {
            self.queue.clear();
        }
        if cont {
            self.queue_continue();
        }
        ui.separator();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        App::isolated()
    }

    fn jobs(tag: &str, programs: &[&str]) -> Vec<PathBuf> {
        let dir = std::env::temp_dir().join(format!("eh3d_mc_queue_{}_{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        programs
            .iter()
            .enumerate()
            .map(|(i, text)| {
                let p = dir.join(format!("job{i}.nc"));
                std::fs::write(&p, text).unwrap();
                p
            })
            .collect()
    }

    /// Lets the program of the queue run to its end, the way the frames of the window would.
    fn run_to_the_end(a: &mut App) {
        for _ in 0..200 {
            if !a.session.running {
                break;
            }
            a.session.pump(1);
        }
        a.queue_tick();
    }

    fn states(a: &App) -> Vec<State> {
        a.queue.items.iter().map(|i| i.state).collect()
    }

    #[test]
    fn two_files_run_one_after_the_other_and_each_one_shows_its_summary_first() {
        let mut a = app();
        let files = jobs("two", &["G21 G90\nG1 X10 F600\n", "G21 G90\nG1 Y10 F600\n"]);
        a.start_queue(files.clone());
        // the first program is loaded and waits for its summary
        assert_eq!(states(&a), vec![State::Running, State::Waiting]);
        assert!(matches!(a.win, Some(Win::Summary(_))));
        assert!(!a.session.running);
        a.queue_tick();
        assert_eq!(
            a.queue.current(),
            Some(0),
            "the summary is open: nothing changes"
        );
        // the operator says start
        a.win = None;
        a.program_start();
        a.queue_tick();
        assert!(a.session.running);
        run_to_the_end(&mut a);
        // the second one is loaded with its own summary
        assert_eq!(states(&a), vec![State::Done, State::Running]);
        assert!(matches!(a.win, Some(Win::Summary(_))));
        assert_eq!(a.path, files[1].display().to_string());
        a.win = None;
        a.program_start();
        a.queue_tick();
        run_to_the_end(&mut a);
        assert_eq!(states(&a), vec![State::Done, State::Done]);
        assert!(a.queue.finished() && !a.queue.active);
    }

    #[test]
    fn with_the_summary_turned_off_the_programs_start_by_themselves() {
        let mut a = app();
        a.settings.confirm_start = false;
        a.start_queue(jobs("auto", &["G1 X1 F600\n", "G1 X2 F600\n"]));
        assert!(a.session.running && a.win.is_none());
        a.queue_tick();
        run_to_the_end(&mut a);
        assert!(a.session.running, "the second program started by itself");
        a.queue_tick();
        run_to_the_end(&mut a);
        assert!(a.queue.finished());
    }

    #[test]
    fn a_program_that_stops_before_its_end_stops_the_queue() {
        let mut a = app();
        a.settings.confirm_start = false;
        a.start_queue(jobs(
            "fail",
            &["G1 X1 F600\nG1 X2\nG1 X3\n", "G1 X5 F600\n"],
        ));
        a.queue_tick();
        a.session.pump(1);
        // the operator (or the controller) ends it in the middle
        a.program_stop();
        a.queue_tick();
        assert_eq!(states(&a), vec![State::Failed, State::Waiting]);
        assert!(a.queue.halted() && !a.session.running);
        // the operator goes on with the one that waits
        a.queue_continue();
        assert_eq!(a.queue.current(), Some(1));
        assert!(a.session.running);
    }

    #[test]
    fn a_file_that_cannot_be_read_or_is_empty_stops_the_queue_with_a_line_in_the_log() {
        let mut a = app();
        let mut files = jobs("bad", &["G1 X1 F600\n", "   \n\n"]);
        files.insert(0, files[0].with_file_name("missing.nc"));
        a.start_queue(files);
        assert_eq!(states(&a)[0], State::Failed);
        assert!(a.queue.halted());
        assert!(a.session.log.iter().any(|l| l.contains("cannot be run")));
        a.queue_continue();
        assert_eq!(states(&a)[1], State::Running, "{:?}", states(&a));
        // the empty one is refused when it comes
        a.queue_skip(1);
        assert_eq!(states(&a)[2], State::Failed, "{:?}", states(&a));
    }

    #[test]
    fn a_summary_that_is_closed_leaves_the_queue_waiting_and_a_controller_in_alarm_fails_it() {
        let mut a = app();
        a.start_queue(jobs("closed", &["G1 X1 F600\n", "G1 X2 F600\n"]));
        a.win = None;
        a.queue_tick();
        assert_eq!(states(&a), vec![State::Waiting, State::Waiting]);
        assert!(!a.queue.active && !a.queue.halted());
        // going on shows the summary again, now the machine is in alarm
        a.queue_continue();
        a.win = None;
        a.session.mdi("G0 X9999");
        assert!(a.session.alarm());
        a.queue_tick();
        assert_eq!(states(&a)[0], State::Failed);
    }

    #[test]
    fn skipping_the_loaded_program_loads_the_next_and_a_single_file_is_not_a_queue() {
        let mut a = app();
        let files = jobs("skip", &["G1 X1 F600\n", "G1 X2 F600\n"]);
        a.start_queue(files.clone());
        a.queue_skip(0);
        assert_eq!(states(&a), vec![State::Skipped, State::Running]);
        assert_eq!(a.path, files[1].display().to_string());
        // opening a file by itself drops the queue
        a.open(&files[0]);
        assert!(a.queue.is_empty());
        a.start_queue(vec![files[0].clone()]);
        assert!(a.queue.is_empty());
        assert_eq!(a.path, files[0].display().to_string());
    }
}
