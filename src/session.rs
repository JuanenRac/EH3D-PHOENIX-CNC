//! What a machine-control program does with a controller: show where the machine is, move it by hand, zero it, send a line, and run a
//! program one line at a time that can be paused, stopped and cut by the emergency stop. It knows nothing of the window, so it is tested
//! on the simulated controller.

use eh3d_controller::{Command, Controller, Error, State, Status, AXES};

use crate::gcode::Modal;

/// The jog steps offered, in mm.
pub const STEPS: [f64; 5] = [0.01, 0.1, 1.0, 10.0, 100.0];

/// The mark of a pause (in seconds) inside the lines that put the controller back in its state.
const WAIT_MARK: &str = "@wait ";

pub struct Session {
    pub controller: Box<dyn Controller>,
    pub status: Status,
    pub program: Vec<String>,
    /// The next line of the program to send.
    pub next: usize,
    pub running: bool,
    pub paused: bool,
    /// What was said between the program and the controller, the oldest first.
    pub log: Vec<String>,
    pub step_mm: f64,
    pub feed_mm_min: f64,
    /// The codes in force, from the lines the controller took.
    pub modal: Modal,
    /// The zero of the operator, apart from the work zero: a position of the machine the operator chose to measure from.
    pub operator_zero: [f64; AXES],
    /// The lines that put the controller back in the state the program had reached; they go out when a program that was left in the middle is resumed.
    pub resume_preamble: Vec<String>,
    /// The longest travel of X, Y and Z (mm) the controller said it has (GRBL `$130` to `$132`); `None` when it was not asked or did not say.
    pub controller_travel: Option<[f64; 3]>,
    /// The controller was asked for its travel already.
    pub travel_asked: bool,
}

/// The travel the summary and the refusal use: where the settings put it, shortened to the longest travel the controller reports when that is
/// smaller (the start of each axis stays as the settings have it).
pub fn effective_travel(
    configured: [(f64, f64); 3],
    controller: Option<[f64; 3]>,
) -> [(f64, f64); 3] {
    let mut out = configured;
    if let Some(c) = controller {
        for (i, (lo, hi)) in out.iter_mut().enumerate() {
            if c[i].is_finite() && c[i] > 0.0 {
                *hi = (*hi).min(*lo + c[i]);
            }
        }
    }
    out
}

impl Session {
    pub fn new(controller: Box<dyn Controller>) -> Self {
        let mut s = Session {
            controller,
            status: Status::default(),
            program: Vec::new(),
            next: 0,
            running: false,
            paused: false,
            log: Vec::new(),
            step_mm: 1.0,
            feed_mm_min: 600.0,
            modal: Modal::default(),
            operator_zero: [0.0; AXES],
            resume_preamble: Vec::new(),
            controller_travel: None,
            travel_asked: false,
        };
        s.refresh();
        s
    }

    fn say(&mut self, line: impl Into<String>) {
        self.log.push(line.into());
        if self.log.len() > 500 {
            self.log.drain(..100);
        }
    }

    /// What went wrong stops a program that is running, and is written in the log.
    fn failed(&mut self, what: &str, e: Error) {
        self.say(format!("{what}: {e}"));
        self.running = false;
        self.paused = false;
    }

    /// Asks the controller how it is.
    pub fn refresh(&mut self) {
        match self.controller.status() {
            Ok(s) => self.status = s,
            Err(e) => self.failed("status", e),
        }
    }

    pub fn alarm(&self) -> bool {
        self.status.state == State::Alarm
    }

    /// Moves an axis by one step, in the direction `dir` (1 or -1).
    pub fn jog(&mut self, axis: usize, dir: f64) {
        if self.running && !self.paused {
            self.say("jog: a program is running");
            return;
        }
        let command = Command::Jog {
            axis,
            distance_mm: dir * self.step_mm,
            feed_mm_min: self.feed_mm_min,
        };
        if let Err(e) = self.controller.command(command) {
            self.failed("jog", e);
        }
        self.refresh();
    }

    fn order(&mut self, what: &str, command: Command) {
        if let Err(e) = self.controller.command(command) {
            self.failed(what, e);
        }
        self.refresh();
    }

    /// Sends a line of the program's own making (a move to a stored position, to the zero) and keeps what it sets.
    fn own_line(&mut self, what: &str, line: &str) {
        match self.controller.send_line(line) {
            Ok(()) => self.modal.apply(line),
            Err(e) => self.failed(what, e),
        }
        self.refresh();
    }

    /// Goes to the position stored with `G28` (the first one).
    pub fn go_g28(&mut self) {
        self.own_line("G28", "G28");
    }

    /// Goes to the position stored with `G30` (the second one).
    pub fn go_g30(&mut self) {
        self.own_line("G30", "G30");
    }

    /// Goes to the work zero in X and Y.
    pub fn go_zero_xy(&mut self) {
        self.own_line("go to zero", "G90 G0 X0 Y0");
    }

    /// Goes to the work zero in Z.
    pub fn go_zero_z(&mut self) {
        self.own_line("go to zero", "G90 G0 Z0");
    }

    /// Makes the work position here zero on these axes only.
    pub fn zero_axes(&mut self, axes: &[usize]) {
        self.order("zero", Command::ZeroWork(axes.to_vec()));
    }

    /// Makes the operator's position here zero on these axes.
    pub fn zero_operator(&mut self, axes: &[usize]) {
        for &a in axes {
            if a < AXES {
                self.operator_zero[a] = self.status.machine_mm[a];
            }
        }
    }

    /// Where the machine is measured from the operator's zero.
    pub fn operator_mm(&self) -> [f64; AXES] {
        let mut o = [0.0; AXES];
        for (i, v) in o.iter_mut().enumerate() {
            *v = self.status.machine_mm[i] - self.operator_zero[i];
        }
        o
    }

    /// Tells the controller the limits of the travel of X, Y and Z (mm); the other axes are left as they are. Says whether the controller
    /// took them: one that keeps its own limits does not.
    pub fn set_travel(&mut self, xyz: [(f64, f64); 3]) -> bool {
        let mut all = [(0.0, 0.0); AXES];
        all[..3].copy_from_slice(&xyz);
        let ok = self.controller.set_travel(all);
        if !ok {
            self.say("travel: the controller keeps its own limits");
        }
        ok
    }

    /// Asks the controller for its longest travel and keeps the answer; it says in the log what it took: the smaller of this and the settings,
    /// or the settings alone when the controller does not give it or does not answer.
    pub fn read_controller_travel(&mut self) -> Option<[f64; 3]> {
        self.travel_asked = true;
        self.controller_travel = None;
        match self.controller.max_travel() {
            Ok(Some(t)) => {
                self.say(format!(
                    "travel: the controller allows X {:.1}, Y {:.1}, Z {:.1} mm; the smaller of that and the settings is used",
                    t[0], t[1], t[2]
                ));
                self.controller_travel = Some(t);
            }
            Ok(None) => self.say(
                "travel: the controller does not give its travel ($130 to $132); the settings are used",
            ),
            Err(e) => self.say(format!(
                "travel: the controller did not answer ({e}); the settings are used"
            )),
        }
        self.controller_travel
    }

    /// Stores the place where the machine is as `G28` or `G30`.
    pub fn store_position(&mut self, which: u8) {
        let line = if which == 28 { "G28.1" } else { "G30.1" };
        self.own_line(line, line);
    }

    pub fn spindle(&mut self, rpm: Option<f64>) {
        match rpm {
            Some(r) => self.own_line("spindle", &format!("M3 S{r:.0}")),
            None => self.own_line("spindle", "M5"),
        }
    }

    pub fn coolant(&mut self, on: bool) {
        self.own_line("coolant", if on { "M8" } else { "M9" });
    }

    /// The next line to send is the first again.
    pub fn rewind(&mut self) {
        self.running = false;
        self.paused = false;
        self.next = 0;
    }

    /// Takes the program away.
    pub fn clear_program(&mut self) {
        self.load("");
    }

    pub fn home(&mut self) {
        self.order("home", Command::Home(Vec::new()));
    }

    /// Makes the work position here zero on X, Y and Z.
    pub fn zero(&mut self) {
        self.order("zero", Command::ZeroWork(vec![0, 1, 2]));
    }

    pub fn unlock(&mut self) {
        self.order("unlock", Command::Unlock);
    }

    /// Stops everything at once; the controller stays in alarm until it is unlocked.
    pub fn estop(&mut self) {
        self.running = false;
        self.paused = false;
        self.order("emergency stop", Command::EmergencyStop);
        self.say("EMERGENCY STOP");
    }

    /// One line typed by the operator.
    pub fn mdi(&mut self, line: &str) {
        let line = line.trim();
        if line.is_empty() {
            return;
        }
        self.say(format!("> {line}"));
        match self.controller.send_line(line) {
            Ok(()) => {
                self.modal.apply(line);
                self.say("ok")
            }
            Err(e) => self.failed("mdi", e),
        }
        self.refresh();
    }

    /// Takes a program (the lines of a G-code file); a program that was running is stopped.
    pub fn load(&mut self, text: &str) {
        self.running = false;
        self.paused = false;
        self.program = text.lines().map(|l| l.trim_end().to_string()).collect();
        self.next = 0;
    }

    /// Starts the program from its first line, or goes on if it was paused.
    /// The lines that bring a controller back to the state of the program at line `next`: units and absolute distances, up to a safe Z, the spindle with
    /// the time to reach its speed, the coolant, over the point the program had reached, and then the modal state of the program (inches, relative
    /// distances, the feed and the way of moving).
    pub fn preamble_for(&self, next: usize, safe_z_mm: f64, spindle_wait_s: f64) -> Vec<String> {
        let m = &self.modal;
        let mut out = vec![
            "G21".to_string(),
            "G90".to_string(),
            format!("G0 Z{safe_z_mm}"),
        ];
        if m.spindle != 5 {
            let speed = m.speed.map(|s| format!(" S{s}")).unwrap_or_default();
            out.push(format!("M{}{}", m.spindle, speed));
            if spindle_wait_s > 0.0 {
                out.push(format!("{WAIT_MARK}{spindle_wait_s}"));
            }
        }
        if m.coolant != 9 {
            out.push(format!("M{}", m.coolant));
        }
        let path = crate::gcode::toolpath(&self.program[..next.min(self.program.len())]);
        if let Some(last) = path.last() {
            out.push(format!("G0 X{:.4} Y{:.4}", last.to.0, last.to.1));
        }
        if m.inches {
            out.push("G20".to_string());
        }
        if !m.absolute {
            out.push("G91".to_string());
        }
        if let Some(f) = m.feed {
            out.push(format!("F{f}"));
        }
        if m.motion == 1 {
            out.push("G1".to_string());
        }
        out
    }

    /// Like `resume_at`, and the controller is brought back to the state of the program when the operator resumes.
    pub fn resume_at_safe(&mut self, next: usize, safe_z_mm: f64, spindle_wait_s: f64) {
        self.resume_at(next);
        self.resume_preamble = self.preamble_for(self.next, safe_z_mm, spindle_wait_s);
    }

    pub fn start(&mut self) {
        if self.alarm() {
            self.say("start: the controller is in alarm");
            return;
        }
        if self.paused {
            // after a failure the controller may have been reset: its state is put back before anything moves
            let preamble = std::mem::take(&mut self.resume_preamble);
            for line in &preamble {
                // a wait is made here, not by the controller: a simulator or a controller that does not know G4 works the same
                if let Some(seconds) = line.strip_prefix(WAIT_MARK) {
                    if let Ok(s) = seconds.parse::<f64>() {
                        std::thread::sleep(std::time::Duration::from_secs_f64(s.clamp(0.0, 120.0)));
                    }
                    continue;
                }
                if let Err(e) = self.controller.send_line(line) {
                    self.failed(&format!("resume ({line})"), e);
                    return;
                }
                self.modal.apply(line);
            }
            self.order("resume", Command::Resume);
            self.paused = false;
            return;
        }
        if self.program.is_empty() {
            return;
        }
        self.next = 0;
        self.running = true;
    }

    /// Goes on with a program that was left in the middle: the codes in force are rebuilt from the lines that were sent, and the program waits paused at `next`,
    /// so the operator looks at the machine (where the tool is, the spindle) before it moves again.
    pub fn resume_at(&mut self, next: usize) {
        let next = next.min(self.program.len());
        self.modal = Modal::default();
        for line in &self.program[..next] {
            let t = line.trim();
            if !t.is_empty() && !t.starts_with(';') {
                self.modal.apply(t);
            }
        }
        self.next = next;
        self.running = true;
        self.paused = true;
        self.say(format!("program resumed at line {}", next + 1));
    }

    pub fn pause(&mut self) {
        if self.running && !self.paused {
            self.paused = true;
            self.order("pause", Command::Pause);
        }
    }

    pub fn stop(&mut self) {
        self.running = false;
        self.paused = false;
        self.order("stop", Command::Stop);
    }

    /// Sends the next lines of the running program (at most `max_lines`), the empty ones and the comments counting as sent. A line the
    /// controller refuses stops the program at that line.
    pub fn pump(&mut self, max_lines: usize) {
        let mut sent = 0;
        while self.running && !self.paused && sent < max_lines {
            let Some(line) = self.program.get(self.next).cloned() else {
                self.running = false;
                self.say("program done");
                break;
            };
            let trimmed = line.trim();
            if !trimmed.is_empty() && !trimmed.starts_with(';') {
                if let Err(e) = self.controller.send_line(trimmed) {
                    let n = self.next + 1;
                    self.failed(&format!("line {n} ({trimmed})"), e);
                    break;
                }
                self.modal.apply(trimmed);
            }
            self.next += 1;
            sent += 1;
        }
        self.refresh();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eh3d_controller::sim::SimController;

    #[test]
    fn a_program_left_in_the_middle_goes_on_paused_with_its_codes_rebuilt() {
        let mut s = Session::new(Box::new(SimController::new()));
        s.load("G21\nG91\nG0 X1\nG0 X1\nG0 X1\n");
        s.resume_at(3);
        assert!(s.running && s.paused && s.next == 3);
        assert!(
            s.modal == {
                let mut m = Modal::default();
                m.apply("G21");
                m.apply("G91");
                m.apply("G0 X1");
                m
            }
        );
        // nothing is sent until the operator resumes
        s.pump(10);
        assert_eq!(s.next, 3);
        // a line past the end is the end
        s.resume_at(99);
        assert_eq!(s.next, s.program.len());
    }

    fn session() -> Session {
        Session::new(Box::new(SimController::new()))
    }

    #[test]
    fn a_resumed_program_first_puts_the_controller_back_in_the_state_it_had() {
        let mut s = session();
        s.load("G21 G90\nM3 S12000\nM8\nG0 X5 Y6 Z2\nG1 Z-1 F150\nG1 X20 Y6\nG1 X30\n");
        s.resume_at_safe(6, 4.0, 0.01);
        let want = [
            "G21",
            "G90",
            "G0 Z4",
            "M3 S12000",
            "@wait 0.01",
            "M8",
            "G0 X20.0000 Y6.0000",
            "F150",
            "G1",
        ];
        assert_eq!(s.resume_preamble, want);
        // nothing goes out until the operator resumes; then the preamble goes first and only once
        s.pump(10);
        assert_eq!(s.next, 6);
        s.start();
        assert!(s.resume_preamble.is_empty() && !s.paused);
        assert!(s.log.iter().all(|l| !l.contains("resume (")), "{:?}", s.log);
    }

    #[test]
    fn a_jog_moves_by_the_step_and_zero_makes_the_work_position_nothing() {
        let mut s = session();
        s.step_mm = 10.0;
        s.jog(0, 1.0);
        s.jog(1, 1.0);
        assert_eq!(
            (s.status.machine_mm[0], s.status.machine_mm[1]),
            (10.0, 10.0)
        );
        s.zero();
        s.jog(0, -1.0);
        s.step_mm = 1.0;
        s.jog(0, 1.0);
        assert_eq!(s.status.work_mm[0], -9.0);
    }

    #[test]
    fn a_program_runs_line_by_line_and_ends_by_itself() {
        let mut s = session();
        s.load("G21 G90\nG0 X10 Y10\n\n; a comment\nG1 X20 F300\nM30");
        s.start();
        assert!(s.running);
        s.pump(2);
        assert_eq!((s.next, s.status.machine_mm[0]), (2, 10.0));
        s.pump(100);
        assert!(!s.running);
        assert_eq!(s.status.machine_mm[0], 20.0);
        assert!(s.log.last().unwrap().contains("program done"));
    }

    #[test]
    fn a_pause_holds_the_program_and_start_goes_on() {
        let mut s = session();
        s.load("G0 X1\nG0 X2\nG0 X3");
        s.start();
        s.pump(1);
        s.pause();
        s.pump(10);
        assert_eq!(s.next, 1, "lines went out while it was paused");
        s.start();
        s.pump(10);
        assert_eq!(s.status.machine_mm[0], 3.0);
    }

    #[test]
    fn a_refused_line_stops_the_program_where_it_is_and_says_which() {
        let mut s = session();
        s.load("G0 X1\nG2 X5 I1\nG0 X9");
        s.start();
        s.pump(10);
        assert!(!s.running);
        assert_eq!(s.next, 1);
        assert!(s.log.iter().any(|l| l.contains("line 2")), "{:?}", s.log);
        assert_eq!(s.status.machine_mm[0], 1.0);
    }

    #[test]
    fn the_emergency_stop_ends_the_run_and_blocks_it_until_unlocked() {
        let mut s = session();
        s.load("G0 X1\nG0 X2");
        s.start();
        s.pump(1);
        s.estop();
        assert!(!s.running && s.alarm());
        s.start();
        assert!(!s.running, "a program started in alarm");
        s.jog(0, 1.0);
        assert_eq!(s.status.machine_mm[0], 1.0, "it moved in alarm");
        s.unlock();
        assert!(!s.alarm());
        s.start();
        assert!(s.running);
    }

    #[test]
    fn a_jog_is_refused_while_a_program_runs_and_the_mdi_line_is_logged() {
        let mut s = session();
        s.load("G0 X1");
        s.start();
        s.jog(0, 1.0);
        assert!(s.log.last().unwrap().contains("running"));
        s.stop();
        s.mdi("G0 X7");
        assert_eq!(s.status.machine_mm[0], 7.0);
        assert!(s.log.iter().any(|l| l == "> G0 X7"));
    }

    #[test]
    fn the_stored_positions_the_axis_zero_and_the_operator_zero_work_on_their_own() {
        let mut s = session();
        s.mdi("G0 X30 Y40 Z-5");
        s.zero_axes(&[0]);
        s.mdi("G0 X50");
        assert_eq!((s.status.work_mm[0], s.status.work_mm[1]), (50.0, 40.0));
        s.zero_operator(&[1]);
        s.mdi("G0 Y60");
        assert_eq!(s.operator_mm()[1], 20.0);
        s.go_zero_xy();
        assert_eq!((s.status.work_mm[0], s.status.work_mm[1]), (0.0, 0.0));
        s.go_g28();
        assert_eq!(s.status.machine_mm[2], 50.0);
        s.go_zero_z();
        assert_eq!(s.status.work_mm[2], 0.0);
    }

    #[test]
    fn the_codes_in_force_come_from_what_the_controller_took() {
        let mut s = session();
        s.mdi("G91 M3 S500");
        assert_eq!(s.modal.m_codes()[0], "M3");
        assert!(s.modal.g_codes().contains(&"G91".to_string()));
        s.load(
            "G90
M5",
        );
        s.start();
        s.pump(5);
        assert!(s.modal.g_codes().contains(&"G90".to_string()));
        assert_eq!(s.modal.m_codes()[0], "M5");
        // a refused line changes nothing
        s.mdi("G2 X1");
        assert!(s.modal.g_codes().contains(&"G90".to_string()));
    }

    #[test]
    fn the_menu_orders_set_the_spindle_the_coolant_the_stored_positions_and_the_travel() {
        let mut s = session();
        s.spindle(Some(12_000.0));
        assert_eq!(s.status.spindle_rpm, 12_000.0);
        assert_eq!(s.modal.m_codes()[0], "M3");
        s.spindle(None);
        assert_eq!(s.status.spindle_rpm, 0.0);
        s.coolant(true);
        assert_eq!(s.modal.m_codes()[1], "M8");
        s.coolant(false);
        s.mdi("G0 X10 Y20 Z-3");
        s.store_position(30);
        s.mdi("G0 X0 Y0 Z0");
        s.go_g30();
        assert_eq!(s.status.machine_mm[0], 10.0);
        assert!(s.set_travel([(0.0, 50.0), (0.0, 50.0), (-10.0, 10.0)]));
        s.mdi("G0 X90");
        assert!(s.alarm());
        s.unlock();
        s.load(
            "G0 X1
G0 X2",
        );
        s.start();
        s.pump(1);
        s.rewind();
        assert_eq!((s.next, s.running), (0, false));
        s.clear_program();
        assert!(s.program.iter().all(|l| l.is_empty()));
    }

    #[test]
    fn homing_and_an_alarm_from_the_travel_are_shown() {
        let mut s = session();
        s.mdi("G0 X400");
        s.home();
        assert_eq!(s.status.machine_mm[0], 0.0);
        s.mdi("G0 X9000");
        assert!(s.alarm());
    }

    fn grbl_session(replies: &[&str]) -> Session {
        use eh3d_controller::grbl::GrblController;
        use eh3d_controller::mock::MockTransport;
        let mut all = vec!["<Idle|MPos:0.000,0.000,0.000|FS:0,0>"];
        all.extend_from_slice(replies);
        Session::new(Box::new(GrblController::new(MockTransport::with_replies(
            &all,
        ))))
    }

    #[test]
    fn the_travel_of_the_controller_shortens_the_travel_of_the_settings_and_never_lengthens_it() {
        let cfg = [(0.0, 500.0), (0.0, 300.0), (-100.0, 50.0)];
        assert_eq!(effective_travel(cfg, None), cfg);
        // X and Z are shorter in the controller, Y is longer in it: the smaller span wins on each axis
        let t = effective_travel(cfg, Some([300.0, 400.0, 45.0]));
        assert_eq!(t, [(0.0, 300.0), (0.0, 300.0), (-100.0, -55.0)]);
        // a value that makes no sense is ignored
        assert_eq!(effective_travel(cfg, Some([0.0, f64::NAN, -5.0])), cfg);
    }

    #[test]
    fn the_travel_is_read_from_grbl_and_said_in_the_log() {
        let mut s = grbl_session(&["$130=300.000", "$131=200.000", "$132=45.000", "ok"]);
        assert_eq!(s.read_controller_travel(), Some([300.0, 200.0, 45.0]));
        assert_eq!(s.controller_travel, Some([300.0, 200.0, 45.0]));
        assert!(s.travel_asked);
        assert!(s.log.last().unwrap().contains("smaller"), "{:?}", s.log);
    }

    #[test]
    fn without_an_answer_or_without_the_values_the_settings_stay_and_the_log_says_so() {
        // it answers with no travel settings
        let mut s = grbl_session(&["$100=250.000", "ok"]);
        assert_eq!(s.read_controller_travel(), None);
        assert!(s.log.last().unwrap().contains("settings are used"));
        // it does not answer at all
        let mut s = grbl_session(&[]);
        assert_eq!(s.read_controller_travel(), None);
        assert!(s.log.last().unwrap().contains("did not answer"));
        // the simulator keeps no such settings
        let mut s = session();
        assert_eq!(s.read_controller_travel(), None);
        assert!(s.controller_travel.is_none() && s.travel_asked);
    }
}
