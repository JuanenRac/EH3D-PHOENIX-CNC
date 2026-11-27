//! What a machine-control program does with a controller: show where the machine is, move it by hand, zero it, send a line, and run a
//! program that can be paused, stopped and cut by the emergency stop. It knows nothing of the window, so it is tested on the simulated
//! controller.
//!
//! The lines of a program are sent by a thread of their own (the emitter), because a controller can take a long time to answer a line
//! (GRBL answers `ok` only when its buffer has room, which with a long move in front can be minutes). The window never waits for it: it
//! asks for the next lines with [`Session::pump_async`] and looks at what was done, and the pause, the stop and the emergency stop go
//! to the controller out of turn through its urgent channel, so they are heard while a line is still waiting for its `ok`. What the
//! window shows meanwhile is the report the controller reads while it waits (a status request sent out of turn), or the last status
//! until one comes.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use eh3d_controller::urgent::{Urgent, UrgentChannel};
use eh3d_controller::{Command, Controller, Error, Family, State, Status, AXES};

use crate::gcode::{toolpath, words, Modal};
use crate::heightmap::HeightMap;
use crate::jog::{limit_step, Step as JogStep};
use crate::logwords::{self, Lg};
use crate::motioncfg::SetterCfg;
use crate::probe::{grid_script, z_touch_off_lines, Act, ProbeCfg};

/// The jog steps offered, in mm.
pub const STEPS: [f64; 5] = [0.01, 0.1, 1.0, 10.0, 100.0];

/// The mark of a pause (in seconds) inside the lines that put the controller back in its state.
const WAIT_MARK: &str = "@wait ";

/// How often the report of a controller that is busy with a line is asked for out of turn.
const LIVE_EVERY: Duration = Duration::from_millis(200);

/// How long after a pause that asks for the spindle to stop the controller has to say that it is in its door state.
const DOOR_WAIT: Duration = Duration::from_secs(3);

/// A controller that can be handed to the thread of the emitter.
pub type BoxedController = Box<dyn Controller + Send>;

/// One thing the emitter does with the controller.
#[derive(Debug, Clone, PartialEq)]
enum Step {
    /// Sends a line and waits for the controller to take it.
    Line(String),
    /// An empty line or a comment: it counts as sent and nothing goes out.
    Skip,
    /// Waits this many seconds (cut short by a stop).
    Wait(f64),
    /// Tells the controller to go on after a hold.
    Resume,
    /// Reads the machine Z where the probe stands and keeps it (the height of a contact of a grid).
    Sample,
}

/// What a batch of steps came to: how many were done and why it stopped early, if it did.
struct Outcome {
    done: usize,
    error: Option<Error>,
}

/// What follows when a batch ends well.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Then {
    /// Lines of the program: `ends` is that the program has no more lines after them.
    Program { ends: bool },
    /// The controller is back in the state of the program and goes on.
    Resumed,
    /// The controller is back in the state of the program and waits for the operator to lower the tool.
    Positioned,
    /// The cycle of the Z probe.
    Probed,
    /// The grid of the height map: when it ends well the map is built from the heights read.
    Grid {
        area: [f64; 4],
        nx: usize,
        ny: usize,
    },
    /// The touch on the tool setter: when it ends well the length of the tool is worked out from the height read.
    Setter { cfg: SetterCfg },
}

/// A batch the emitter is working on.
struct InFlight {
    rx: Receiver<Outcome>,
    steps: Vec<Step>,
    /// How many steps have been taken into account (the codes in force, the next line).
    applied: usize,
    /// Each step done is a line of the program.
    advances: bool,
    then: Then,
    /// The controller was in inches before a probe cycle, which runs in millimetres.
    inches_before: bool,
}

/// The last move of a resumed program: lowering the tool to where the program had it, at a cutting feed, once the operator says so.
#[derive(Debug, Clone, PartialEq)]
struct Descent {
    lines: Vec<String>,
    text: String,
}

pub struct Session {
    controller: Arc<Mutex<BoxedController>>,
    /// The family of the controller, and the language its errors are told in (the numbers of Klipper have words of their own).
    family: Family,
    lang: String,
    /// The way in for the bytes that cannot wait (see `eh3d_controller::urgent`); `None` when the controller has none.
    urgent: Option<UrgentChannel>,
    /// The emitter stops before its next step while this is set (a pause, a stop).
    halt: Arc<AtomicBool>,
    /// How many steps of the batch in flight are done.
    progress: Arc<AtomicUsize>,
    inflight: Option<InFlight>,
    /// When the status was last asked while a line waits for its `ok`, or while a door state is awaited.
    live_asked: Option<Instant>,
    /// The controller was opened with the pause that also stops the spindle and the coolant (the safety door of GRBL).
    pub spindle_stop_on_pause: bool,
    /// A pause sent out of turn asked for the spindle to stop: the controller has until then to say that it is in its door state.
    door_due: Option<Instant>,
    /// A stop or an emergency stop was asked while a batch was in flight: the line it cuts short is not a failure.
    aborted: bool,
    /// How many times a stop or an emergency stop was asked.
    halts: u64,
    pub status: Status,
    pub program: Vec<String>,
    /// The next line of the program to send.
    pub next: usize,
    pub running: bool,
    pub paused: bool,
    /// What was said between the program and the controller, the oldest first.
    pub log: Vec<String>,
    /// How many lines were dropped from the front of `log` to keep it short, so a reader can tell where it was.
    pub log_dropped: usize,
    pub step_mm: f64,
    pub feed_mm_min: f64,
    /// The codes in force, from the lines the controller took.
    pub modal: Modal,
    /// The zero of the operator, apart from the work zero: a position of the machine the operator chose to measure from.
    pub operator_zero: [f64; AXES],
    /// The lines that put the controller back in the state the program had reached; they go out when a program that was left in the middle is resumed.
    pub resume_preamble: Vec<String>,
    /// The tool is lowered to where the program had it only when the operator presses Start a second time.
    resume_descent: Option<Descent>,
    /// The feed (mm/min) of that descent when the program had set none.
    pub resume_plunge_feed_mm_min: f64,
    /// The longest travel of X, Y and Z (mm) the controller said it has (GRBL `$130` to `$132`); `None` when it was not asked or did not say.
    pub controller_travel: Option<[f64; 3]>,
    /// The controller was asked for its travel already.
    pub travel_asked: bool,
    /// Lines that start with `$` (settings and commands of the controller) are sent when the program has them; off by default, and then the program stops at such a line.
    pub allow_system_lines: bool,
    /// The travel of X, Y and Z in the coordinates of the machine: a step of the jog past it is shortened and one from the limit outwards is refused (`None`: no limit).
    pub soft_limits: Option<[(f64, f64); 3]>,
    /// The soft limits hold only after the machine was homed (before that its coordinates mean nothing).
    pub limits_need_homing: bool,
    /// The machine was homed since it was last reset or in alarm.
    homed: bool,
    /// The height map measured with the probe, and the program as it was before the map corrected it (to put it back).
    pub height_map: Option<HeightMap>,
    uncorrected: Option<Vec<String>>,
    /// The heights the emitter read during a grid.
    samples: Arc<Mutex<Vec<f64>>>,
    /// The axis and the direction (+1 or -1) of the jog that may still be moving: a jog in another direction cancels it first.
    last_jog: Option<(usize, i8)>,
    /// A jog that crosses the last one (another axis, or the other way) cancels the move in progress first; off, the order goes as it is.
    pub cancel_crossing_jog: bool,
    /// The length of the tool the last measure on the tool setter gave (mm), until the operator writes it into the table or drops it.
    pub setter_length: Option<f64>,
    /// The work zero was shifted by hand (`G92`, typed or by the zero button): a reset of the controller would take the shift away.
    pub hand_shift: bool,
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

/// Waits `seconds`, in short slices, and gives up as soon as `halt` is set. Says whether the wait was completed.
fn wait_unless_halted(seconds: f64, halt: &AtomicBool) -> bool {
    let seconds = if seconds.is_finite() {
        seconds.clamp(0.0, 120.0)
    } else {
        0.0
    };
    let end = std::time::Instant::now() + Duration::from_secs_f64(seconds);
    while std::time::Instant::now() < end {
        if halt.load(Ordering::SeqCst) {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    !halt.load(Ordering::SeqCst)
}

/// The work of the emitter: the steps one after the other, until they end, one fails or `halt` is set.
fn run_steps(
    controller: &Mutex<BoxedController>,
    steps: &[Step],
    halt: &AtomicBool,
    progress: &AtomicUsize,
    samples: &Mutex<Vec<f64>>,
) -> Outcome {
    let mut c = controller.lock().unwrap_or_else(PoisonError::into_inner);
    for (i, step) in steps.iter().enumerate() {
        if halt.load(Ordering::SeqCst) {
            return Outcome {
                done: i,
                error: None,
            };
        }
        let result = match step {
            Step::Skip => Ok(()),
            Step::Line(line) => c.send_line(line),
            Step::Wait(seconds) => {
                if !wait_unless_halted(*seconds, halt) {
                    return Outcome {
                        done: i,
                        error: None,
                    };
                }
                Ok(())
            }
            Step::Resume => c.command(Command::Resume),
            Step::Sample => c.status().map(|s| {
                samples
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .push(s.machine_mm[2]);
            }),
        };
        match result {
            Ok(()) => progress.store(i + 1, Ordering::SeqCst),
            Err(e) => {
                return Outcome {
                    done: i,
                    error: Some(e),
                }
            }
        }
    }
    Outcome {
        done: steps.len(),
        error: None,
    }
}

impl Session {
    pub fn new(controller: BoxedController) -> Self {
        let urgent = controller.urgent_channel();
        let family = controller.family();
        let mut s = Session {
            controller: Arc::new(Mutex::new(controller)),
            family,
            lang: "en".to_string(),
            urgent,
            halt: Arc::new(AtomicBool::new(false)),
            progress: Arc::new(AtomicUsize::new(0)),
            inflight: None,
            live_asked: None,
            spindle_stop_on_pause: false,
            door_due: None,
            aborted: false,
            halts: 0,
            status: Status::default(),
            program: Vec::new(),
            next: 0,
            running: false,
            paused: false,
            log: Vec::new(),
            log_dropped: 0,
            step_mm: 1.0,
            feed_mm_min: 600.0,
            modal: Modal::default(),
            operator_zero: [0.0; AXES],
            resume_preamble: Vec::new(),
            resume_descent: None,
            resume_plunge_feed_mm_min: 100.0,
            controller_travel: None,
            travel_asked: false,
            allow_system_lines: false,
            soft_limits: None,
            limits_need_homing: false,
            homed: false,
            height_map: None,
            uncorrected: None,
            samples: Arc::new(Mutex::new(Vec::new())),
            last_jog: None,
            cancel_crossing_jog: true,
            setter_length: None,
            hand_shift: false,
        };
        s.refresh();
        s
    }

    fn ctl(&self) -> MutexGuard<'_, BoxedController> {
        self.controller
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// A message of the log in the language of the window.
    fn tell<S: AsRef<str>>(&mut self, message: Lg, args: &[S]) {
        let line = logwords::say(&self.lang, message, args);
        self.say(line);
    }

    /// A message of the log about something that was asked (`jog`, `probe`...), with both in the language of the window.
    fn tell_about(&mut self, message: Lg, what: &str) {
        let what = logwords::what(&self.lang, what);
        self.tell(message, &[what]);
    }

    /// A message of the log with no holes.
    fn note(&mut self, message: Lg) {
        self.tell(message, &[] as &[&str]);
    }

    /// The reason of a probe, a tool setter or a height map that failed, after the name of what was asked.
    fn trouble(&mut self, what: &str, why: &str) {
        let what = logwords::what(&self.lang, what);
        self.say(format!("{what}: {why}"));
    }

    fn say(&mut self, line: impl Into<String>) {
        self.log.push(line.into());
        if self.log.len() > 500 {
            self.log.drain(..100);
            self.log_dropped += 100;
        }
    }

    /// What went wrong stops a program that is running, and is written in the log.
    fn failed(&mut self, what: &str, e: Error) {
        let what = logwords::what(&self.lang, what);
        let text = if self.family == Family::Klipper {
            crate::ctltext::explain_error(self.family, &e, &self.lang)
        } else {
            e.to_string()
        };
        self.say(format!("{what}: {text}"));
        self.running = false;
        self.paused = false;
    }

    /// The language of the log (the code of the window; English for one the tables do not have).
    pub fn set_lang(&mut self, code: &str) {
        if self.lang != code {
            self.lang = code.to_string();
        }
    }

    /// The emitter is sending lines: the controller is not free for anything else but the urgent bytes.
    pub fn busy(&self) -> bool {
        self.inflight.is_some()
    }

    /// Says (in the log) that the controller is busy, and tells so.
    fn refuse_busy(&mut self, what: &str) -> bool {
        if self.inflight.is_some() {
            self.tell_about(Lg::Busy, what);
            true
        } else {
            false
        }
    }

    /// Asks the controller how it is. While the emitter works the controller is not asked in the usual way: a status request goes out of
    /// turn, the report that comes back while the line waits for its `ok` replaces the status, and until one comes the window keeps the
    /// last status.
    pub fn refresh(&mut self) {
        if self.inflight.is_some() {
            self.live_status();
            return;
        }
        let r = self.ctl().status();
        match r {
            Ok(s) => self.take_status(s),
            Err(e) => self.failed("status", e),
        }
    }

    /// Keeps a status the controller gave.
    fn take_status(&mut self, s: Status) {
        // an alarm may have lost the position
        if s.state == State::Alarm {
            self.homed = false;
        }
        if !matches!(s.state, State::Jog | State::Run) {
            self.last_jog = None;
        }
        self.status = s;
    }

    /// The status of a controller that is busy with a line: takes the report that has come and asks for the next one out of turn.
    fn live_status(&mut self) {
        let Some(channel) = self.urgent.clone() else {
            return;
        };
        if let Some(report) = channel.take_report() {
            self.take_status(report);
        }
        if self.live_asked.is_none_or(|at| at.elapsed() >= LIVE_EVERY) {
            channel.send(Urgent::Status);
            self.live_asked = Some(Instant::now());
        }
    }

    /// Looks whether the controller said that it is in its door state after a pause sent out of turn, and says so when it did not.
    fn check_door(&mut self) {
        let Some(due) = self.door_due else {
            return;
        };
        if self.status.state == State::Door {
            self.door_due = None;
        } else if Instant::now() >= due {
            self.door_due = None;
            self.note(Lg::SpindleNotStopped);
        }
    }

    /// The program runs but the controller stands in its door state (a resume given before the door state was complete, or a safety door
    /// of the machine): a start gives the resume again.
    pub fn held_at_door(&self) -> bool {
        self.running && !self.paused && self.status.state == State::Door
    }

    /// The controller has an input for a probe (the probe, the height map and the tool setter need one). A controller that is busy sending
    /// answers no, to be asked again.
    pub fn has_probe(&self) -> bool {
        self.inflight.is_none() && self.ctl().capabilities().probe
    }

    pub fn alarm(&self) -> bool {
        self.status.state == State::Alarm
    }

    /// Moves an axis by one step, in the direction `dir` (1 or -1).
    pub fn jog(&mut self, axis: usize, dir: f64) {
        if self.running && !self.paused {
            self.note(Lg::JogRunning);
            return;
        }
        if self.refuse_busy("jog") {
            return;
        }
        let mut distance_mm = dir * self.step_mm;
        let holds = !self.limits_need_homing || self.homed;
        if let (Some(limits), true, true) = (self.soft_limits, axis < 3, holds) {
            match limit_step(self.status.machine_mm[axis], distance_mm, limits[axis]) {
                JogStep::Go(d) => distance_mm = d,
                JogStep::Shorten(d) => {
                    distance_mm = d;
                    self.tell(Lg::JogShortened, &[format!("{d:.3}")]);
                }
                JogStep::Refuse => {
                    self.note(Lg::JogEnd);
                    return;
                }
            }
        }
        // two jogs that cross (X+ and then Y+ while the first still moves) are normal for a person; the guard takes a second, different move
        // order while one is active for a fault, so the move in progress is cancelled before the new one is given
        let way = (axis, if distance_mm < 0.0 { -1i8 } else { 1 });
        if self.cancel_crossing_jog
            && matches!(self.status.state, State::Jog | State::Run)
            && self.last_jog.is_some_and(|prev| prev != way)
        {
            let stop = self.ctl().command(Command::JogStop);
            if let Err(e) = stop {
                self.failed("jog", e);
                self.refresh();
                return;
            }
        }
        self.last_jog = Some(way);
        let command = Command::Jog {
            axis,
            distance_mm,
            feed_mm_min: self.feed_mm_min,
        };
        let r = self.ctl().command(command);
        if let Err(e) = r {
            self.failed("jog", e);
        }
        self.refresh();
    }

    /// Gives an order to the controller; says whether it was taken.
    fn order(&mut self, what: &str, command: Command) -> bool {
        if self.refuse_busy(what) {
            return false;
        }
        let r = self.ctl().command(command);
        let taken = r.is_ok();
        if let Err(e) = r {
            self.failed(what, e);
        }
        self.refresh();
        taken
    }

    /// Sends a line of the program's own making (a move to a stored position, to the zero) and keeps what it sets.
    fn own_line(&mut self, what: &str, line: &str) {
        if self.refuse_busy(what) {
            return;
        }
        let r = self.ctl().send_line(line);
        match r {
            Ok(()) => {
                self.modal.apply(line);
                self.note_shift(line);
            }
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
        self.zero_work(axes);
    }

    /// Makes the position here the zero of the work system in use. The order of a GRBL controller writes `G54` only, so with another system
    /// (`G55` to `G59`) the line that names the one in use (`P0`) is sent instead. On Marlin and Klipper the zero is a shift (`G92`) that a
    /// reset takes away, which is kept in mind.
    fn zero_work(&mut self, axes: &[usize]) {
        let grbl = matches!(
            self.family,
            Family::Grbl | Family::GrblHal | Family::FluidNc
        );
        if grbl && self.modal.wcs != 54 {
            let words: String = axes
                .iter()
                .filter_map(|&a| ['X', 'Y', 'Z', 'A', 'B', 'C'].get(a))
                .map(|letter| format!(" {letter}0"))
                .collect();
            self.own_line("zero", &format!("G10 L20 P0{words}"));
            return;
        }
        if self.order("zero", Command::ZeroWork(axes.to_vec()))
            && matches!(self.family, Family::Marlin | Family::Klipper)
        {
            self.hand_shift = true;
        }
    }

    /// Keeps in mind a shift of the zero (`G92`) given by hand: a reset of the controller takes it away, and no line of the program
    /// gives it back.
    fn note_shift(&mut self, line: &str) {
        for (letter, v) in words(line) {
            if letter != 'G' {
                continue;
            }
            match (v * 10.0).round() as i32 {
                920 | 923 => self.hand_shift = true,
                921 | 922 => self.hand_shift = false,
                _ => {}
            }
        }
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
        if self.refuse_busy("travel") {
            return false;
        }
        let mut all = [(0.0, 0.0); AXES];
        all[..3].copy_from_slice(&xyz);
        let ok = self.ctl().set_travel(all);
        if !ok {
            self.note(Lg::TravelKeeps);
        }
        ok
    }

    /// Asks the controller for its longest travel and keeps the answer; it says in the log what it took: the smaller of this and the settings,
    /// or the settings alone when the controller does not give it or does not answer.
    pub fn read_controller_travel(&mut self) -> Option<[f64; 3]> {
        if self.refuse_busy("travel") {
            return self.controller_travel;
        }
        self.travel_asked = true;
        self.controller_travel = None;
        let r = self.ctl().max_travel();
        match r {
            Ok(Some(t)) => {
                self.tell(Lg::TravelAllows, &t.map(|v| format!("{v:.1}")));
                self.controller_travel = Some(t);
            }
            Ok(None) => self.note(Lg::TravelNotGiven),
            Err(e) => self.tell(Lg::TravelSilent, &[e.to_string()]),
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
        if self.refuse_busy("home") {
            return;
        }
        let r = self.ctl().command(Command::Home(Vec::new()));
        match r {
            Ok(()) => self.homed = true,
            Err(e) => self.failed("home", e),
        }
        self.refresh();
    }

    /// Whether the machine was homed since it was last reset or in alarm.
    pub fn homed(&self) -> bool {
        self.homed
    }

    /// Makes the work position here zero on X, Y and Z.
    pub fn zero(&mut self) {
        self.zero_work(&[0, 1, 2]);
    }

    pub fn unlock(&mut self) {
        self.order("unlock", Command::Unlock);
    }

    /// Puts an urgent byte in the way in of the controller, which sends it without waiting for the line in progress. Says whether it could.
    fn send_urgent(&mut self, what: &str, u: Urgent) -> bool {
        let channel = self.urgent.clone();
        match channel {
            Some(c) => {
                c.send(u);
                true
            }
            None => {
                self.tell_about(Lg::NoUrgent, what);
                false
            }
        }
    }

    /// How many times a stop or an emergency stop was asked since the session began.
    pub fn halts(&self) -> u64 {
        self.halts
    }

    /// Stops everything at once; the controller stays in alarm until it is unlocked. It does not wait for the line that is waiting for its `ok`.
    pub fn estop(&mut self) {
        self.halts += 1;
        self.homed = false;
        self.running = false;
        self.paused = false;
        self.door_due = None;
        self.resume_preamble.clear();
        self.resume_descent = None;
        self.halt.store(true, Ordering::SeqCst);
        if self.inflight.is_some() {
            self.aborted = true;
            self.send_urgent("emergency stop", Urgent::Reset);
        } else {
            self.order("emergency stop", Command::EmergencyStop);
        }
        self.note(Lg::Estop);
    }

    /// One line typed by the operator.
    pub fn mdi(&mut self, line: &str) {
        self.mdi_checked(line);
    }

    /// One line sent like a typed one, saying whether the controller took it (an empty line, a controller that is busy sending a program and a
    /// line the controller refuses give `false`). A macro stops at the first line that gives `false`.
    pub fn mdi_checked(&mut self, line: &str) -> bool {
        let line = line.trim();
        if line.is_empty() {
            return false;
        }
        self.say(format!("> {line}"));
        if self.refuse_busy("mdi") {
            return false;
        }
        let r = self.ctl().send_line(line);
        let taken = r.is_ok();
        match r {
            Ok(()) => {
                self.modal.apply(line);
                self.note_shift(line);
                self.say("ok")
            }
            Err(e) => self.failed("mdi", e),
        }
        self.refresh();
        taken
    }

    /// Takes a program (the lines of a G-code file); a program that was running is stopped.
    pub fn load(&mut self, text: &str) {
        self.running = false;
        self.paused = false;
        self.program = text.lines().map(|l| l.trim_end().to_string()).collect();
        self.uncorrected = None;
        self.next = 0;
    }

    /// The lines that bring a controller back to the state of the program at line `next`: units and absolute distances, the plane, the work
    /// coordinate system and the tool length offset in force, up to a safe Z, the spindle with the time to reach its speed, the coolant, over the
    /// point the program had reached, and then the modal state of the program (inches, the feed and the way of moving). The program is never in
    /// relative distances here: [`Session::resume_at_safe`] does not resume one.
    pub fn preamble_for(&self, next: usize, safe_z_mm: f64, spindle_wait_s: f64) -> Vec<String> {
        let m = &self.modal;
        // the tool length offset and the feed are written in the units of the lines that set them, not in the ones in force now
        let (feed_in_inches, offset_in_inches) =
            crate::startline::units_of_values(&self.program, next);
        let mut out = vec!["G21".to_string(), "G90".to_string()];
        if m.plane != 17 {
            out.push(format!("G{}", m.plane));
        }
        if m.wcs != 54 {
            out.push(format!("G{}", m.wcs));
        }
        if let Some(z) = m.tool_length {
            if offset_in_inches {
                out.push("G20".to_string());
            }
            out.push(format!("G43.1 Z{z}"));
            if offset_in_inches {
                out.push("G21".to_string());
            }
        }
        out.push(format!("G0 Z{safe_z_mm}"));
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
        let path = toolpath(&self.program[..next.min(self.program.len())]);
        if let Some(last) = path.last() {
            out.push(format!("G0 X{:.4} Y{:.4}", last.to.0, last.to.1));
        }
        if m.inches {
            out.push("G20".to_string());
        }
        if let Some(f) = m.feed {
            let units = |inches: bool| if inches { "G20" } else { "G21" }.to_string();
            if feed_in_inches == m.inches {
                out.push(format!("F{f}"));
            } else {
                out.extend([units(feed_in_inches), format!("F{f}"), units(m.inches)]);
            }
        }
        if m.motion == 1 {
            out.push("G1".to_string());
        }
        out
    }

    /// The move that takes the tool from the safe height down to where the program had it, at a cutting feed. `None` when the line the
    /// program goes on with has a Z of its own (it lowers the tool itself), or the program was not below the safe height.
    fn descent_for(&self, next: usize, safe_z_mm: f64) -> Option<Descent> {
        let next = next.min(self.program.len());
        let z = toolpath(&self.program[..next]).last()?.to.2;
        let line_has_z = self
            .program
            .get(next)
            .is_some_and(|l| words(l).iter().any(|&(c, _)| c == 'Z'));
        if line_has_z || z >= safe_z_mm - 1e-9 {
            return None;
        }
        let m = &self.modal;
        // the lines are in the units the preamble puts back
        let unit = if m.inches { 25.4 } else { 1.0 };
        let (feed_in_inches, _) = crate::startline::units_of_values(&self.program, next);
        let given = if feed_in_inches { 25.4 } else { 1.0 };
        let feed = m
            .feed
            .map_or(self.resume_plunge_feed_mm_min / unit, |f| f * given / unit);
        let z_cmd = z / unit;
        let mut lines = vec![format!("G1 Z{z_cmd:.4} F{feed}")];
        if m.motion == 0 {
            lines.push("G0".to_string());
        }
        Some(Descent {
            lines,
            text: logwords::say(
                &self.lang,
                Lg::DescentWait,
                &[format!("{z:.3}"), feed.to_string()],
            ),
        })
    }

    /// Why the program cannot go on from line `next` after the controller was stopped or reset; `None` when its state can be rebuilt from the lines
    /// before it. Refused: relative distances (`G91`), a canned cycle or a cutter compensation still on, a shift of the zero (`G92`), which the reset
    /// takes away and the lines cannot give back, and an axis last moved by `G28`, `G30` or `G53`, whose place in the work coordinates is not known.
    pub fn resume_refusal(&self, next: usize) -> Option<String> {
        let next = next.min(self.program.len());
        let line = next + 1;
        let mut m = Modal::default();
        let mut shifted: Option<usize> = None;
        let (mut cycle, mut comp): (Option<usize>, Option<usize>) = (None, None);
        // for X, Y and Z: the line that left the axis where the work coordinates do not say
        let mut lost: [Option<usize>; 3] = [None; 3];
        for (n, text) in self.program[..next].iter().enumerate() {
            let t = text.trim();
            if t.is_empty() || t.starts_with(';') {
                continue;
            }
            m.apply(t);
            let all = words(t);
            let (mut home, mut machine, mut offset) = (false, false, false);
            for &(l, v) in &all {
                if l != 'G' {
                    continue;
                }
                match (v * 10.0).round() as i32 {
                    800 => cycle = None,
                    810..=890 => cycle = Some(n + 1),
                    400 => comp = None,
                    410 | 420 => comp = Some(n + 1),
                    920 | 923 => {
                        shifted = Some(n + 1);
                        offset = true;
                    }
                    921 | 922 => shifted = None,
                    100 | 431 => offset = true,
                    280 | 300 => home = true,
                    530 => machine = true,
                    _ => {}
                }
            }
            let named = ['X', 'Y', 'Z'].map(|axis| all.iter().any(|&(l, _)| l == axis));
            // `G28` and `G30` with no axis take every axis away
            let every = home && !named.contains(&true);
            for (axis, here) in lost.iter_mut().zip(named) {
                if (home || machine) && (here || every) {
                    *axis = Some(n + 1);
                } else if !(home || machine || offset) && here && m.absolute {
                    *axis = None;
                }
            }
        }
        let why = |message: Lg, first: usize| {
            Some(logwords::say(
                &self.lang,
                message,
                &[first.to_string(), line.to_string()],
            ))
        };
        if !m.absolute {
            return why(Lg::ResRelative, line);
        }
        if let Some(l) = cycle {
            return why(Lg::ResCycle, l);
        }
        if let Some(l) = comp {
            return why(Lg::ResComp, l);
        }
        if let Some(l) = shifted {
            return why(Lg::ResShift, l);
        }
        if let Some(l) = lost.iter().flatten().next() {
            return why(Lg::ResLost, *l);
        }
        use crate::startline::Refusal;
        match crate::startline::state_refusal(&self.program, next) {
            Some(Refusal::Motion(l)) => Some(logwords::say(
                &self.lang,
                Lg::ResMotion,
                &[line.to_string(), l.to_string()],
            )),
            Some(Refusal::FeedMode(l)) => why(Lg::ResFeedMode, l),
            Some(Refusal::TableLength(l)) => why(Lg::ResTable, l),
            _ => None,
        }
    }

    /// As [`Session::resume_refusal`], for a program that was cut short by a failure and saved: lines past `next` may have run already, so an offset
    /// set there (`G10`), which the controller keeps, refuses it too. `margin` is the number of lines that move the saved line was taken back.
    pub fn recovery_refusal(&self, next: usize, margin: usize) -> Option<String> {
        self.resume_refusal(next).or_else(|| {
            // the recovery file is written every twenty lines, so the program may have gone that much further
            crate::recovery::offset_set_again(&self.program, next, margin, 20).map(|n| {
                logwords::say(
                    &self.lang,
                    Lg::ResOffsetAgain,
                    &[(n + 1).to_string(), (next + 1).to_string()],
                )
            })
        })
    }

    /// Like `resume_at`, and the controller is brought back to the state of the program when the operator resumes. Says whether the program
    /// waits to be resumed: it does not where [`Session::resume_refusal`] says the state at that line cannot be rebuilt (the controller may have
    /// run fewer lines than were sent).
    pub fn resume_at_safe(&mut self, next: usize, safe_z_mm: f64, spindle_wait_s: f64) -> bool {
        self.resume_preamble.clear();
        self.resume_descent = None;
        if let Some(why) = self.resume_refusal(next) {
            self.running = false;
            self.paused = false;
            self.say(why);
            return false;
        }
        self.resume_at(next);
        self.resume_preamble = self.preamble_for(self.next, safe_z_mm, spindle_wait_s);
        self.resume_descent = self.descent_for(self.next, safe_z_mm);
        true
    }

    /// The steps of the lines that put the controller back (a mark of a wait is a wait).
    fn steps_of(lines: &[String]) -> Vec<Step> {
        lines
            .iter()
            .map(|line| match line.strip_prefix(WAIT_MARK) {
                // a wait is made here, not by the controller: a simulator or a controller that does not know G4 works the same
                Some(seconds) => Step::Wait(seconds.parse::<f64>().unwrap_or(0.0)),
                None => Step::Line(line.clone()),
            })
            .collect()
    }

    /// Hands a batch of steps to the emitter.
    fn post(&mut self, steps: Vec<Step>, advances: bool, then: Then) {
        self.halt.store(false, Ordering::SeqCst);
        self.progress.store(0, Ordering::SeqCst);
        self.aborted = false;
        let (tx, rx) = mpsc::channel();
        let controller = Arc::clone(&self.controller);
        let halt = Arc::clone(&self.halt);
        let progress = Arc::clone(&self.progress);
        let work = steps.clone();
        let samples = Arc::clone(&self.samples);
        samples
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
        std::thread::spawn(move || {
            let outcome = run_steps(&controller, &work, &halt, &progress, &samples);
            // the controller is free before the answer is given
            let _ = tx.send(outcome);
        });
        self.inflight = Some(InFlight {
            rx,
            steps,
            applied: 0,
            advances,
            then,
            inches_before: false,
        });
    }

    /// Hands a probe cycle to the emitter: its distances and feeds are millimetres whatever units the program left the controller in, and
    /// a controller that was in inches goes back to them after it.
    fn post_cycle(&mut self, mut steps: Vec<Step>, then: Then) {
        let inches = self.modal.inches;
        steps.insert(0, Step::Line("G21".to_string()));
        if inches {
            steps.push(Step::Line("G20".to_string()));
        }
        self.post(steps, false, then);
        if let Some(f) = self.inflight.as_mut() {
            f.inches_before = inches;
        }
    }

    /// Takes into account the steps of the batch done up to `done`: the codes they set and the line the program is at.
    fn apply_done(&mut self, f: &mut InFlight, done: usize) {
        let done = done.min(f.steps.len());
        while f.applied < done {
            if let Step::Line(line) = &f.steps[f.applied] {
                self.modal.apply(line);
            }
            if f.advances {
                self.next += 1;
            }
            f.applied += 1;
        }
    }

    /// Looks at what the emitter has done since the last look; when its batch ended, the outcome is dealt with and the controller is asked how it is.
    pub fn collect(&mut self) {
        let Some(mut f) = self.inflight.take() else {
            return;
        };
        let received = f.rx.try_recv();
        let (outcome, finished) = match received {
            Ok(o) => (Some(o), true),
            Err(TryRecvError::Empty) => (None, false),
            Err(TryRecvError::Disconnected) => (None, true),
        };
        let done = match &outcome {
            Some(o) => o.done,
            None => self.progress.load(Ordering::SeqCst),
        };
        self.apply_done(&mut f, done);
        if !finished {
            self.inflight = Some(f);
            return;
        }
        match outcome {
            None => {
                self.note(Lg::ThreadStopped);
                self.running = false;
                self.paused = false;
            }
            Some(Outcome { error: Some(e), .. }) => {
                // what a stop cut short is not a failure
                if !self.aborted {
                    let what = match f.then {
                        Then::Program { .. } => {
                            let n = self.next + 1;
                            let line = match f.steps.get(f.applied) {
                                Some(Step::Line(l)) => l.clone(),
                                _ => String::new(),
                            };
                            logwords::say(&self.lang, Lg::LineWhat, &[n.to_string(), line])
                        }
                        Then::Resumed | Then::Positioned => {
                            let line = match f.steps.get(f.applied) {
                                Some(Step::Line(l)) => l.clone(),
                                _ => String::new(),
                            };
                            format!("{} ({line})", logwords::what(&self.lang, "resume"))
                        }
                        Then::Probed | Then::Grid { .. } | Then::Setter { .. } => {
                            let line = match f.steps.get(f.applied) {
                                Some(Step::Line(l)) => l.clone(),
                                _ => String::new(),
                            };
                            // a touch that fails leaves the controller in relative distances, and a cycle is run in millimetres
                            let _ = self.ctl().send_line("G90");
                            if f.inches_before && self.ctl().send_line("G20").is_ok() {
                                self.modal.apply("G20");
                            }
                            format!("{} ({line})", logwords::what(&self.lang, "probe"))
                        }
                    };
                    self.failed(&what, e);
                }
            }
            Some(o) => {
                if o.done == f.steps.len() {
                    match f.then {
                        Then::Program { ends } => {
                            if ends && self.running {
                                self.running = false;
                                self.paused = false;
                                self.note(Lg::ProgramDone);
                            }
                        }
                        Then::Resumed => self.paused = false,
                        Then::Positioned => {
                            if let Some(d) = &self.resume_descent {
                                let text = d.text.clone();
                                self.say(text);
                            }
                        }
                        Then::Probed => self.note(Lg::ProbeTouched),
                        Then::Grid { area, nx, ny } => self.finish_grid(area, nx, ny),
                        Then::Setter { cfg } => self.finish_setter(&cfg),
                    }
                }
            }
        }
        self.refresh();
    }

    /// Whether a probe cannot start now (and why, in the log). A probe moves the machine, so it is only made when the operator confirmed it.
    fn probe_refused(&mut self, what: &str, confirmed: bool) -> bool {
        if !confirmed {
            self.tell_about(Lg::NotConfirmed, what);
            return true;
        }
        if self.running {
            self.tell_about(Lg::ProgramRunning, what);
            return true;
        }
        if self.alarm() {
            self.tell_about(Lg::InAlarm, what);
            return true;
        }
        if self.refuse_busy(what) {
            return true;
        }
        let has_probe = self.ctl().capabilities().probe;
        if !has_probe {
            self.tell_about(Lg::NoProbe, what);
            return true;
        }
        false
    }

    /// The cycle of the Z probe: down until the probe closes, the Z of the work set to the thickness of the plate, and up. The lines go out by the
    /// emitter, so the stop is heard meanwhile. It moves the machine: nothing happens unless `confirmed`.
    pub fn probe_z(&mut self, cfg: &ProbeCfg, confirmed: bool) -> bool {
        if self.probe_refused("probe", confirmed) {
            return false;
        }
        match z_touch_off_lines(cfg) {
            Ok(lines) => {
                self.tell(
                    Lg::ProbeStart,
                    &[cfg.depth_mm, cfg.feed_mm_min, cfg.plate_mm].map(|v| v.to_string()),
                );
                self.post_cycle(Self::steps_of(&lines), Then::Probed);
                true
            }
            Err(e) => {
                self.trouble("probe", &e);
                false
            }
        }
    }

    /// The grid of the height map: the head visits the points of `area` (`[x0, y0, x1, y1]`, work coordinates) and touches at each; when it ends
    /// the map is in `height_map`. It moves the machine: nothing happens unless `confirmed`.
    pub fn probe_grid(
        &mut self,
        area: [f64; 4],
        nx: usize,
        ny: usize,
        cfg: &ProbeCfg,
        confirmed: bool,
    ) -> bool {
        if self.probe_refused("probe", confirmed) {
            return false;
        }
        match grid_script(area, nx, ny, cfg) {
            Ok(script) => {
                let steps: Vec<Step> = script
                    .into_iter()
                    .map(|a| match a {
                        Act::Mark(_) => Step::Skip,
                        Act::Line(l) => Step::Line(l),
                        Act::Sample => Step::Sample,
                    })
                    .collect();
                self.tell(Lg::GridStart, &[nx.to_string(), ny.to_string()]);
                self.post_cycle(steps, Then::Grid { area, nx, ny });
                true
            }
            Err(e) => {
                self.trouble("probe", &e);
                false
            }
        }
    }

    /// Measures the tool in the spindle by touching the tool setter (see `toolsetter`): the head goes to the setter, touches it and lifts, and
    /// when it ends well the length is in `setter_length`. It moves the machine: nothing happens unless `confirmed`, and with `setter_confirm`
    /// on in the motion file the caller must have asked the operator whether the setter is there and clear.
    pub fn measure_tool(&mut self, cfg: &SetterCfg, confirmed: bool) -> bool {
        if self.probe_refused("tool setter", confirmed) {
            return false;
        }
        self.setter_length = None;
        match crate::toolsetter::script(cfg) {
            Ok(script) => {
                let steps: Vec<Step> = script
                    .into_iter()
                    .map(|a| match a {
                        Act::Mark(_) => Step::Skip,
                        Act::Line(l) => Step::Line(l),
                        Act::Sample => Step::Sample,
                    })
                    .collect();
                self.note(Lg::SetterStart);
                self.post_cycle(steps, Then::Setter { cfg: *cfg });
                true
            }
            Err(e) => {
                self.trouble("tool setter", &e);
                false
            }
        }
    }

    /// The touch ended well: the length of the tool is worked out from the height read.
    fn finish_setter(&mut self, cfg: &SetterCfg) {
        let z = self
            .samples
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .first()
            .copied();
        match z
            .ok_or_else(|| "no reading".to_string())
            .and_then(|z| crate::toolsetter::length_from_touch(z, cfg))
        {
            Ok(length) => {
                self.tell(Lg::SetterResult, &[format!("{length:.3}")]);
                self.setter_length = Some(length);
            }
            Err(e) => self.trouble("tool setter", &e),
        }
    }

    /// The grid ended well: the map is built from the heights read.
    fn finish_grid(&mut self, area: [f64; 4], nx: usize, ny: usize) {
        let readings = self
            .samples
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        match HeightMap::from_readings(area, nx, ny, &readings) {
            Ok(map) => {
                let (lo, hi) = map.span();
                self.tell(
                    Lg::MapPoints,
                    &[
                        readings.len().to_string(),
                        format!("{lo:.3}"),
                        format!("{hi:.3}"),
                    ],
                );
                self.height_map = Some(map);
            }
            Err(e) => self.trouble("heightmap", &e),
        }
    }

    /// Corrects the loaded program to follow the height map (the program as it was is kept, see [`Session::drop_height_correction`]).
    pub fn apply_height_map(&mut self, segment_mm: f64) -> bool {
        if self.running {
            self.tell_about(Lg::ProgramRunning, "heightmap");
            return false;
        }
        if self.uncorrected.is_some() {
            self.note(Lg::MapAlready);
            return false;
        }
        let Some(result) = self
            .height_map
            .as_ref()
            .map(|m| m.correct(&self.program, segment_mm))
        else {
            self.note(Lg::MapNone);
            return false;
        };
        match result {
            Ok(out) => {
                self.tell(
                    Lg::MapDone,
                    &[out.len().to_string(), self.program.len().to_string()],
                );
                self.uncorrected = Some(std::mem::replace(&mut self.program, out));
                self.next = 0;
                true
            }
            Err(e) => {
                let why = e.text(&self.lang);
                self.trouble("heightmap", &why);
                false
            }
        }
    }

    /// Puts back the program as it was before the height map corrected it.
    pub fn drop_height_correction(&mut self) -> bool {
        if self.running {
            return false;
        }
        match self.uncorrected.take() {
            Some(original) => {
                self.program = original;
                self.next = 0;
                self.note(Lg::MapBack);
                true
            }
            None => false,
        }
    }

    /// Whether the loaded program has been corrected by the height map.
    pub fn corrected(&self) -> bool {
        self.uncorrected.is_some()
    }

    /// Waits until the emitter has nothing in hand (for what has to see the outcome now: a script, the tests).
    pub fn wait_idle(&mut self) {
        while self.inflight.is_some() {
            self.collect();
            if self.inflight.is_some() {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }

    /// Starts the program from its first line, or goes on if it was paused.
    pub fn start(&mut self) {
        self.collect();
        if self.alarm() {
            self.tell_about(Lg::InAlarm, "start");
            return;
        }
        self.door_due = None;
        if self.held_at_door() {
            if self.inflight.is_some() {
                self.send_urgent("resume", Urgent::Resume);
            } else {
                self.order("resume", Command::Resume);
            }
            return;
        }
        if self.paused {
            if self.inflight.is_some() {
                // the pause came while a line was waiting: the controller is only told to go on (out of turn, it is busy with that line)
                self.halt.store(false, Ordering::SeqCst);
                if self.send_urgent("resume", Urgent::Resume) {
                    self.paused = false;
                }
                return;
            }
            // after a failure the controller may have been reset: its state is put back before anything moves
            if !self.resume_preamble.is_empty() {
                let preamble = std::mem::take(&mut self.resume_preamble);
                let mut steps = Self::steps_of(&preamble);
                let then = if self.resume_descent.is_some() {
                    // the tool is not lowered without the operator saying so: it stays paused over the point
                    Then::Positioned
                } else {
                    steps.push(Step::Resume);
                    Then::Resumed
                };
                self.post(steps, false, then);
                return;
            }
            if let Some(d) = self.resume_descent.take() {
                let mut steps = Self::steps_of(&d.lines);
                steps.push(Step::Resume);
                self.post(steps, false, Then::Resumed);
                return;
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
        self.tell(Lg::ResumedAt, &[(next + 1).to_string()]);
    }

    /// Holds the program. While a line waits for its `ok` the hold goes to the controller out of turn.
    pub fn pause(&mut self) {
        if self.running && !self.paused {
            self.paused = true;
            self.halt.store(true, Ordering::SeqCst);
            if self.inflight.is_some() {
                // out of turn the controller cannot wait for the door state: the reports are looked at (see `check_door`)
                if self.send_urgent("pause", Urgent::Hold) && self.spindle_stop_on_pause {
                    self.door_due = Some(Instant::now() + DOOR_WAIT);
                }
            } else if self.spindle_stop_on_pause {
                self.pause_with_door();
            } else {
                self.order("pause", Command::Pause);
            }
        }
    }

    /// The pause that also stops the spindle, with no line in progress. When the door state is not confirmed but the feed is held, the
    /// program stays paused (so it can be gone on with or stopped) and the log says that the spindle may still turn.
    fn pause_with_door(&mut self) {
        let r = self.ctl().command(Command::Pause);
        let Err(e) = r else {
            self.refresh();
            return;
        };
        self.refresh();
        if self.paused && self.status.state == State::Hold {
            self.note(Lg::SpindleNotStopped);
        } else {
            self.failed("pause", e);
        }
    }

    /// Stops the program. While a line waits for its `ok` the stop goes to the controller out of turn (a hold, and a reset once the machine stands still).
    pub fn stop(&mut self) {
        self.halts += 1;
        self.running = false;
        self.paused = false;
        self.door_due = None;
        self.resume_preamble.clear();
        self.resume_descent = None;
        self.halt.store(true, Ordering::SeqCst);
        if self.inflight.is_some() {
            self.aborted = true;
            self.send_urgent("stop", Urgent::StopJob);
        } else {
            self.order("stop", Command::Stop);
        }
    }

    /// Hands the emitter the next lines of the running program (at most `max_lines`), the empty ones and the comments counting as sent, and
    /// takes into account what it has done meanwhile. It never waits for the controller. A line the controller refuses stops the program
    /// at that line; a line that starts with `$` stops it before it is sent unless the settings allow it.
    pub fn pump_async(&mut self, max_lines: usize) {
        self.collect();
        if self.inflight.is_some() {
            // the line waits for its answer: the position follows the reports asked for out of turn
            self.live_status();
            self.check_door();
            return;
        }
        if self.door_due.is_some() && self.live_asked.is_none_or(|at| at.elapsed() >= LIVE_EVERY) {
            self.live_asked = Some(Instant::now());
            self.refresh();
        }
        self.check_door();
        if !self.running || self.paused {
            return;
        }
        let mut steps = Vec::new();
        let mut ends = false;
        let mut at = self.next;
        let mut stopped_here = false;
        while steps.len() < max_lines {
            let Some(line) = self.program.get(at).cloned() else {
                ends = true;
                break;
            };
            let trimmed = line.trim();
            // a comment in front of it does not hide it: the comments are taken out before the line is sent
            let code = crate::gcode::code_of(trimmed);
            if code.trim_start().starts_with('$') && !self.allow_system_lines {
                let n = at + 1;
                self.tell(Lg::SystemLine, &[n.to_string(), trimmed.to_string()]);
                self.running = false;
                self.paused = false;
                stopped_here = true;
                break;
            }
            steps.push(if trimmed.is_empty() || trimmed.starts_with(';') {
                Step::Skip
            } else {
                Step::Line(trimmed.to_string())
            });
            at += 1;
        }
        if steps.is_empty() {
            if ends && !stopped_here {
                self.running = false;
                self.note(Lg::ProgramDone);
            }
            self.refresh();
            return;
        }
        self.post(steps, true, Then::Program { ends });
    }

    /// Sends the next lines of the running program (at most `max_lines`) and waits until they are sent: like [`Session::pump_async`] for
    /// whoever has nothing else to do meanwhile (a script, the tests).
    pub fn pump(&mut self, max_lines: usize) {
        self.pump_async(max_lines);
        self.wait_idle();
        self.refresh();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eh3d_controller::sim::SimController;

    #[test]
    fn a_refusal_of_klipper_is_logged_in_the_language_of_the_window() {
        use crate::link::{simulated_with, Dialect};
        use eh3d_controller::wire::Impair;
        let mut texts = Vec::new();
        for lang in crate::ctltext::LANGS {
            let mut s = Session::new(simulated_with(
                Dialect::Klipper,
                Impair::clean(),
                false,
                Some(1),
            ));
            s.set_lang(lang);
            s.mdi("G1 X1 F100");
            let last = s.log.last().cloned().unwrap_or_default();
            let asked = format!("{}: ", crate::logwords::what(lang, "mdi"));
            assert!(last.starts_with(&asked), "{last}");
            assert!(!last.contains("Must home"), "{lang}: {last}");
            texts.push(last);
        }
        texts.sort_unstable();
        texts.dedup();
        assert_eq!(texts.len(), 8, "eight languages, eight texts");
    }

    #[test]
    fn a_resume_is_refused_where_an_arc_a_feed_mode_or_a_table_offset_cannot_be_written_back() {
        for (program, at, want) in [
            (
                "G21 G90 F100\nG2 X2 Y0 I1 J0\nX4 I1 J0",
                2,
                "goes on in the way of moving of line 2",
            ),
            ("G93\nG1 X1 F2\nG1 X2 F2", 2, "G93, G95"),
            ("G43 H1\nG0 Z5\nG0 X1", 2, "(G43)"),
        ] {
            let mut s = Session::new(Box::new(SimController::new()));
            s.load(program);
            let why = s.resume_refusal(at).unwrap();
            assert!(why.contains(want), "{why}");
            assert!(!s.resume_at_safe(at, 5.0, 0.0) && !s.running, "{program}");
        }
    }

    #[test]
    fn a_resume_writes_the_feed_in_the_units_it_was_given_in_and_stops_are_counted() {
        let mut s = Session::new(Box::new(SimController::new()));
        s.load("G20 G90 F10\nG21\nG1 X1\nG1 X2");
        assert!(s.resume_at_safe(3, 5.0, 0.0));
        let at = s.resume_preamble.iter().position(|l| l == "F10").unwrap();
        assert_eq!(
            (
                s.resume_preamble[at - 1].as_str(),
                s.resume_preamble[at + 1].as_str()
            ),
            ("G20", "G21")
        );
        assert_eq!(s.halts(), 0);
        s.stop();
        s.estop();
        assert_eq!(s.halts(), 2);
    }

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
    fn a_program_line_that_starts_with_a_dollar_stops_the_program_unless_it_is_allowed() {
        let mut s = session();
        s.load("G21\n$130=9999\nG0 X1\n");
        s.start();
        s.pump(10);
        assert!(!s.running, "the program stops at the line");
        assert_eq!(s.next, 1, "the line is not sent and not passed");
        assert!(
            s.log.iter().any(|l| l.contains("starts with $")),
            "{:?}",
            s.log
        );
        let mut allowed = session();
        allowed.allow_system_lines = true;
        allowed.load("G21\n$130=9999\nG0 X1\n");
        allowed.start();
        allowed.pump(10);
        assert!(
            !allowed.log.iter().any(|l| l.contains("starts with $")),
            "{:?}",
            allowed.log
        );
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
        // nothing goes out until the operator resumes; then the preamble goes first and only once, and the tool is not lowered yet
        s.pump(10);
        assert_eq!(s.next, 6);
        s.start();
        s.wait_idle();
        assert!(s.resume_preamble.is_empty() && s.paused);
        assert!(s.log.iter().all(|l| !l.contains("resume (")), "{:?}", s.log);
        // the second Start lowers the tool to where the program had it and goes on
        s.start();
        s.wait_idle();
        assert!(!s.paused);
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

    /// A controller that never answers a line by itself, as GRBL with a long move in front: the line waits until the machine is told to go on
    /// (resume) or is stopped (reset or stop of the job), and everything it was told out of turn is kept.
    struct Stalled {
        urgent: UrgentChannel,
        seen: Arc<Mutex<Vec<Urgent>>>,
        /// Set when a line has been handed to the controller and waits for its answer.
        entered: Arc<AtomicBool>,
        alarm: bool,
    }

    impl Stalled {
        fn new() -> (Stalled, Arc<Mutex<Vec<Urgent>>>, Arc<AtomicBool>) {
            let seen = Arc::new(Mutex::new(Vec::new()));
            let entered = Arc::new(AtomicBool::new(false));
            (
                Stalled {
                    urgent: UrgentChannel::new(),
                    seen: Arc::clone(&seen),
                    entered: Arc::clone(&entered),
                    alarm: false,
                },
                seen,
                entered,
            )
        }
    }

    impl Controller for Stalled {
        fn family(&self) -> eh3d_controller::Family {
            eh3d_controller::Family::Grbl
        }
        fn capabilities(&self) -> eh3d_controller::Capabilities {
            SimController::new().capabilities()
        }
        fn status(&mut self) -> Result<Status, Error> {
            Ok(Status {
                state: if self.alarm { State::Alarm } else { State::Run },
                ..Status::default()
            })
        }
        fn command(&mut self, _command: Command) -> Result<(), Error> {
            Ok(())
        }
        fn send_line(&mut self, _line: &str) -> Result<(), Error> {
            self.entered.store(true, Ordering::SeqCst);
            loop {
                for u in self.urgent.take() {
                    self.seen.lock().unwrap().push(u);
                    if u.ends_job() {
                        self.alarm = u == Urgent::Reset;
                        return Err(Error::Refused("interrupted by an urgent stop".into()));
                    }
                    if u == Urgent::Resume {
                        return Ok(());
                    }
                }
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        fn urgent_channel(&self) -> Option<UrgentChannel> {
            Some(self.urgent.clone())
        }
    }

    fn stalled_session(program: &str) -> (Session, Arc<Mutex<Vec<Urgent>>>) {
        let (c, seen, entered) = Stalled::new();
        let mut s = Session::new(Box::new(c));
        s.load(program);
        s.start();
        s.pump_async(1);
        assert!(s.busy(), "the line is waiting for its answer");
        // the stop must find the line waiting in the controller, not still on its way there
        let t0 = std::time::Instant::now();
        while !entered.load(Ordering::SeqCst) {
            assert!(
                t0.elapsed() < Duration::from_secs(5),
                "the line never arrived"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        (s, seen)
    }

    fn quickly(t0: std::time::Instant) {
        assert!(
            t0.elapsed() < Duration::from_secs(5),
            "the window waited for the line"
        );
    }

    #[test]
    fn the_position_follows_the_machine_while_a_line_waits_for_its_answer() {
        let (mut s, seen) = stalled_session("G1 X1\nG1 X2");
        let channel = s
            .urgent
            .clone()
            .expect("the controller has an urgent channel");
        let before = s.status.machine_mm[0];
        s.refresh();
        assert_eq!(
            s.status.machine_mm[0], before,
            "no report yet: the last status stays"
        );
        // the window calls `pump_async` every frame: that is what has to follow the machine
        for x in [12.5, 13.0] {
            let mut live = Status::default();
            live.machine_mm[0] = x;
            channel.publish_report(live);
            s.pump_async(1);
            assert!(s.busy(), "the line is still waiting");
            assert_eq!(s.status.machine_mm[0], x, "the report that came is shown");
        }
        s.stop();
        s.wait_idle();
        assert!(
            seen.lock().unwrap().contains(&Urgent::Status),
            "a status request went out of turn"
        );
    }

    fn door_session(no_door: bool) -> Session {
        use eh3d_controller::grbl::GrblController;
        use eh3d_controller::grblsim::{Flavor, GrblDevice};
        use eh3d_controller::wire::{Impair, WireLink};

        let mut device = GrblDevice::new(Flavor::Grbl);
        device.no_door = no_door;
        let controller = GrblController::new(WireLink::new(device, Impair::clean()))
            .with_spindle_stop_on_pause(true);
        let mut s = Session::new(Box::new(controller));
        s.spindle_stop_on_pause = true;
        s.mdi("M3 S8000");
        s.load("G0 X1\nG0 X2");
        s.start();
        s
    }

    #[test]
    fn a_pause_that_stops_the_spindle_goes_to_the_door_state_and_a_start_goes_on() {
        let mut s = door_session(false);
        s.pause();
        assert!(s.paused && s.running, "{:?}", s.log);
        assert_eq!((s.status.state, s.status.spindle_rpm), (State::Door, 0.0));
        s.start();
        assert!(!s.paused && s.running);
        assert_eq!(
            (s.status.state, s.status.spindle_rpm),
            (State::Idle, 8000.0)
        );
    }

    #[test]
    fn a_pause_whose_door_state_is_not_confirmed_stays_paused_and_says_so() {
        let mut s = door_session(true);
        s.pause();
        assert!(s.paused && s.running, "{:?}", s.log);
        assert_eq!(s.status.state, State::Hold);
        let said = logwords::say("en", Lg::SpindleNotStopped, &[] as &[&str]);
        assert!(s.log.contains(&said), "{:?}", s.log);
    }

    #[test]
    fn a_start_resumes_a_running_program_that_stands_in_the_door_state_without_rewinding_it() {
        let mut s = door_session(false);
        s.pump(1);
        let next = s.next;
        s.status.state = State::Door;
        assert!(s.held_at_door());
        s.start();
        assert_eq!(s.next, next, "the program was not started again");
        assert!(s.running && !s.paused && !s.held_at_door());
    }

    #[test]
    fn a_pause_out_of_turn_that_does_not_reach_the_door_state_is_said_once() {
        let (mut s, seen) = stalled_session("G1 X1");
        s.spindle_stop_on_pause = true;
        s.pause();
        assert!(s.door_due.is_some());
        let said = logwords::say("en", Lg::SpindleNotStopped, &[] as &[&str]);
        s.pump_async(1);
        assert!(!s.log.contains(&said), "the controller still has time");
        s.door_due = Some(Instant::now());
        s.pump_async(1);
        s.pump_async(1);
        assert_eq!(
            s.log.iter().filter(|l| **l == said).count(),
            1,
            "{:?}",
            s.log
        );
        s.stop();
        s.wait_idle();
        assert!(seen.lock().unwrap().contains(&Urgent::Hold));
    }

    #[test]
    fn the_emergency_stop_is_heard_while_a_line_waits_for_its_answer() {
        let (mut s, seen) = stalled_session("G1 X1\nG1 X2");
        let t0 = std::time::Instant::now();
        s.estop();
        quickly(t0);
        assert!(!s.running && !s.paused);
        s.wait_idle();
        assert_eq!(*seen.lock().unwrap(), vec![Urgent::Reset]);
        assert!(s.alarm(), "the status after the reset says alarm");
        assert!(s.log.iter().any(|l| l == "EMERGENCY STOP"));
        assert!(
            s.log.iter().all(|l| !l.contains("interrupted")),
            "a line cut short by the stop is not a failure: {:?}",
            s.log
        );
    }

    #[test]
    fn the_stop_is_heard_while_a_line_waits_for_its_answer() {
        let (mut s, seen) = stalled_session("G1 X1\nG1 X2");
        let t0 = std::time::Instant::now();
        s.stop();
        quickly(t0);
        s.wait_idle();
        assert_eq!(*seen.lock().unwrap(), vec![Urgent::StopJob]);
        assert!(!s.running && s.log.iter().all(|l| !l.contains("interrupted")));
    }

    #[test]
    fn the_pause_is_heard_and_the_program_goes_on_when_the_operator_starts_again() {
        let (mut s, seen) = stalled_session("G1 X1");
        let t0 = std::time::Instant::now();
        s.pause();
        quickly(t0);
        assert!(s.paused && s.busy(), "the line is still waiting");
        // nothing but the urgent bytes is asked of a controller that is busy
        s.mdi("G0 X9");
        assert!(s.log.iter().any(|l| l.contains("busy")), "{:?}", s.log);
        s.start();
        assert!(!s.paused);
        s.wait_idle();
        assert_eq!(*seen.lock().unwrap(), vec![Urgent::Hold, Urgent::Resume]);
        assert_eq!(s.next, 1, "the line was taken once the machine went on");
    }

    #[test]
    fn a_controller_with_no_urgent_channel_says_that_the_stop_cannot_be_given_out_of_turn() {
        struct Plain(SimController);
        impl Controller for Plain {
            fn family(&self) -> eh3d_controller::Family {
                self.0.family()
            }
            fn capabilities(&self) -> eh3d_controller::Capabilities {
                self.0.capabilities()
            }
            fn status(&mut self) -> Result<Status, Error> {
                self.0.status()
            }
            fn command(&mut self, c: Command) -> Result<(), Error> {
                self.0.command(c)
            }
            fn send_line(&mut self, line: &str) -> Result<(), Error> {
                std::thread::sleep(Duration::from_millis(150));
                self.0.send_line(line)
            }
        }
        let mut s = Session::new(Box::new(Plain(SimController::new())));
        s.load("G0 X1");
        s.start();
        s.pump_async(1);
        assert!(s.busy());
        s.stop();
        assert!(
            s.log.iter().any(|l| l.contains("cannot take it")),
            "{:?}",
            s.log
        );
        s.wait_idle();
        assert!(!s.running);
    }

    #[test]
    fn the_program_goes_to_the_emitter_in_batches_and_the_window_sees_its_progress() {
        let mut s = session();
        s.load("G0 X1\nG0 X2\nG0 X3\nG0 X4");
        s.start();
        s.pump_async(3);
        s.wait_idle();
        assert_eq!((s.next, s.status.machine_mm[0]), (3, 3.0));
        assert!(s.running);
        s.pump_async(3);
        s.wait_idle();
        assert!(!s.running);
        assert_eq!(s.status.machine_mm[0], 4.0);
        assert!(s.log.last().unwrap().contains("program done"));
    }

    #[test]
    fn a_program_in_relative_distances_is_not_resumed_and_the_codes_of_the_work_are_put_back() {
        let mut s = session();
        s.load("G21 G91\nG1 X5 F100\nG1 X5\nG1 X5\n");
        assert!(!s.resume_at_safe(2, 4.0, 0.0));
        assert!(!s.running && !s.paused && s.resume_preamble.is_empty());
        assert!(s.log.iter().any(|l| l.contains("G91")), "{:?}", s.log);
        // in absolute distances the work coordinate system, the plane and the tool length come back before the tool moves
        let mut s = session();
        s.load("G21 G90 G55 G18\nG43.1 Z-2.5\nG0 X5 Y6 Z6\nG1 X9 F100\nG1 X10");
        assert!(s.resume_at_safe(4, 4.0, 0.0));
        let p = s.resume_preamble.clone();
        let at = |t: &str| {
            p.iter()
                .position(|l| l == t)
                .unwrap_or_else(|| panic!("{t} in {p:?}"))
        };
        assert!(at("G90") < at("G55") && at("G55") < at("G0 Z4"));
        assert!(at("G18") < at("G0 Z4") && at("G43.1 Z-2.5") < at("G0 Z4"));
        let g = s.modal.g_codes();
        assert!(g.contains(&"G55".to_string()) && g.contains(&"G18".to_string()));
        assert!(g.contains(&"G43.1".to_string()));
    }

    #[test]
    fn a_resumed_program_lowers_the_tool_only_when_the_operator_says_so() {
        let mut s = session();
        s.load("G21 G90\nG0 X5 Y6 Z2\nG1 Z-1.5 F150\nG1 X20 Y6\nG1 X30");
        assert!(s.resume_at_safe(4, 4.0, 0.0));
        // first Start: over the point, at the safe height, still paused
        s.start();
        s.wait_idle();
        assert!(s.paused, "the tool waits over the point");
        assert_eq!(s.status.machine_mm[2], 4.0);
        assert!(
            s.log.iter().any(|l| l.contains("press Start again")),
            "{:?}",
            s.log
        );
        // second Start: down to the Z the program had, at the feed of the program, and on
        s.start();
        s.wait_idle();
        assert!(!s.paused && s.running);
        assert_eq!(s.status.machine_mm[2], -1.5);
        assert_eq!(s.modal.feed, Some(150.0));
        // a line with a Z of its own lowers the tool by itself: no descent
        let mut s = session();
        s.load("G21 G90\nG0 X5 Y6 Z2\nG1 Z-1.5 F150\nG1 X20 Y6\nG1 Z-3 X30");
        assert!(s.resume_at_safe(4, 4.0, 0.0));
        s.start();
        s.wait_idle();
        assert!(!s.paused, "nothing to confirm");
        assert_eq!(s.status.machine_mm[2], 4.0);
        // a program that was above the safe height has nothing to lower
        let mut s = session();
        s.load("G21 G90\nG0 X5 Y6 Z6\nG1 X20 Y6\nG1 X30");
        assert!(s.resume_at_safe(3, 4.0, 0.0));
        s.start();
        s.wait_idle();
        assert!(!s.paused);
    }

    #[test]
    fn the_descent_uses_the_feed_of_the_settings_when_the_program_had_none() {
        let mut s = session();
        s.resume_plunge_feed_mm_min = 80.0;
        s.load("G21 G90\nG0 X5 Y6 Z2\nG1 Z-1\nG1 X20 Y6\nG1 X30");
        assert!(s.resume_at_safe(4, 4.0, 0.0));
        let d = s.resume_descent.clone().expect("a descent");
        assert_eq!(d.lines[0], "G1 Z-1.0000 F80");
    }

    #[test]
    fn a_shifted_zero_a_lost_place_or_a_mode_still_on_is_never_resumed_and_nothing_is_sent() {
        for (program, next, want) in [
            (
                "G21 G90\nG0 X5 Y5 Z5\nG92 X0 Y0 Z0\nG1 X1 F100\nG1 X2",
                4,
                "G92",
            ),
            ("G21 G90\nG92.3\nG0 X1 Y1 Z5\nG0 X2", 3, "G92"),
            ("G21 G90\nG81 X1 Y1 Z-1 R1 F50\nX2\nG80", 3, "canned cycle"),
            ("G21 G90\nG41 D1 G1 X1 F50\nG1 X5", 2, "compensation"),
            ("G21 G90\nG0 X5 Y6 Z2\nG53 G0 Z0\nG0 X9", 3, "G53"),
            ("G21 G90\nG0 X5 Y6 Z2\nG28\nG0 X9 Y9\nG1 X10", 4, "G28"),
            ("G21 G90\nG0 X5 Y6 Z2\nG28 G91 Z0\nG90\nG0 X9", 5, "G28"),
        ] {
            let (mut s, sent) = touchy_session(|_, _| 0.0);
            s.load(program);
            let why = s.recovery_refusal(next, 16).unwrap_or_default();
            assert!(why.contains(want), "{program}: {why}");
            assert!(!s.resume_at_safe(next, 4.0, 0.0), "{program}");
            assert!(!s.running && !s.paused && s.resume_preamble.is_empty());
            s.pump(10);
            assert!(sent.lock().unwrap().is_empty(), "{program}");
        }
        // an offset the controller keeps, set where lines may have run, refuses the recovery of a program that failed
        let mut s = session();
        s.load("G21 G90\nG0 X5 Y6 Z2\nG1 X6 F50\nG1 X7\nG10 L20 P1 X0\nG1 X8");
        let why = s.recovery_refusal(2, 16).unwrap_or_default();
        assert!(why.contains("line 5") && why.contains("G10"), "{why}");
        // once the shift is taken away, or every axis was named again, the program goes on
        for (program, next) in [
            ("G21 G90\nG92 X0\nG92.1\nG0 X1 Y1 Z5\nG0 X2", 4),
            ("G21 G90\nG28 G91 Z0\nG90\nG0 X1 Y1\nG0 Z5\nG0 X2", 5),
            ("G21 G90\nG53 G0 Z0\nG0 X1 Y1 Z5\nG0 X2", 3),
        ] {
            let mut s = session();
            s.load(program);
            assert_eq!(s.recovery_refusal(next, 16), None, "{program}");
        }
    }

    #[test]
    fn the_lines_of_a_resume_reach_the_controller_exactly_and_the_offset_keeps_its_units() {
        let (mut s, sent) = touchy_session(|_, _| 0.0);
        s.load("G20 G90\nG43.1 Z-1.5\nM3 S9000\nG0 X1 Y2 Z1\nG1 X2 F10\nG1 X3\n");
        assert!(s.resume_at_safe(5, 4.0, 0.0));
        assert!(sent.lock().unwrap().is_empty());
        s.start();
        s.wait_idle();
        s.pump(10);
        assert_eq!(
            *sent.lock().unwrap(),
            [
                "G21",
                "G90",
                "G20",
                "G43.1 Z-1.5",
                "G21",
                "G0 Z4",
                "M3 S9000",
                "G0 X50.8000 Y50.8000",
                "G20",
                "F10",
                "G1",
                "G1 X3",
            ]
        );
    }

    #[test]
    fn a_program_taken_up_after_a_failure_sends_again_the_moves_the_buffer_may_have_lost() {
        let (mut s, sent) = touchy_session(|_, _| 0.0);
        s.load("G21 G90\nG0 X0 Y0 Z6\nG1 X1 F100\n; a\nG1 X2\n; b\nG1 X3\n; c\n");
        // all eight lines were taken, and the buffer held the last two moves
        let next = crate::recovery::backed_off(&s.program, 8, 2);
        assert_eq!(next, 4);
        assert!(s.resume_at_safe(next, 4.0, 0.0));
        s.start();
        s.wait_idle();
        s.pump(10);
        assert_eq!(
            *sent.lock().unwrap(),
            [
                "G21",
                "G90",
                "G0 Z4",
                "G0 X1.0000 Y0.0000",
                "F100",
                "G1",
                "G1 X2",
                "G1 X3",
            ]
        );
    }

    /// A controller with a probe: it keeps where the head is, takes the lines of a touch and answers a contact at the height of a surface.
    struct Touchy {
        sent: Arc<Mutex<Vec<String>>>,
        at: (f64, f64, f64),
        surface: fn(f64, f64) -> f64,
        miss_at: Option<usize>,
        touches: usize,
    }

    impl Touchy {
        fn new(surface: fn(f64, f64) -> f64) -> (Touchy, Arc<Mutex<Vec<String>>>) {
            let sent = Arc::new(Mutex::new(Vec::new()));
            (
                Touchy {
                    sent: Arc::clone(&sent),
                    at: (0.0, 0.0, 5.0),
                    surface,
                    miss_at: None,
                    touches: 0,
                },
                sent,
            )
        }
    }

    impl Controller for Touchy {
        fn family(&self) -> eh3d_controller::Family {
            eh3d_controller::Family::Grbl
        }
        fn capabilities(&self) -> eh3d_controller::Capabilities {
            eh3d_controller::Capabilities {
                probe: true,
                ..SimController::new().capabilities()
            }
        }
        fn status(&mut self) -> Result<Status, Error> {
            let mut machine_mm = [0.0; AXES];
            machine_mm[..3].copy_from_slice(&[self.at.0, self.at.1, self.at.2]);
            Ok(Status {
                state: State::Idle,
                machine_mm,
                ..Status::default()
            })
        }
        fn command(&mut self, _command: Command) -> Result<(), Error> {
            Ok(())
        }
        fn send_line(&mut self, line: &str) -> Result<(), Error> {
            self.sent.lock().unwrap().push(line.to_string());
            if line.starts_with("G38.2") {
                self.touches += 1;
                if Some(self.touches) == self.miss_at {
                    return Err(Error::Refused("error:9".into()));
                }
                self.at.2 = (self.surface)(self.at.0, self.at.1);
            } else if let Some(rest) = line.strip_prefix("G0 X") {
                let (x, y) = rest.split_once(" Y").unwrap();
                self.at.0 = x.parse().unwrap();
                self.at.1 = y.parse().unwrap();
            } else if let Some(z) = line.strip_prefix("G0 Z") {
                self.at.2 = z.parse().unwrap();
            }
            Ok(())
        }
    }

    fn touchy_session(surface: fn(f64, f64) -> f64) -> (Session, Arc<Mutex<Vec<String>>>) {
        let (c, sent) = Touchy::new(surface);
        (Session::new(Box::new(c)), sent)
    }

    /// A controller whose jog goes on after the order: it says `Jog` until it is told to stop.
    struct Sticky {
        sent: Arc<Mutex<Vec<String>>>,
        jogging: bool,
    }

    impl Controller for Sticky {
        fn family(&self) -> eh3d_controller::Family {
            eh3d_controller::Family::Grbl
        }
        fn capabilities(&self) -> eh3d_controller::Capabilities {
            SimController::new().capabilities()
        }
        fn status(&mut self) -> Result<Status, Error> {
            Ok(Status {
                state: if self.jogging {
                    State::Jog
                } else {
                    State::Idle
                },
                ..Status::default()
            })
        }
        fn command(&mut self, command: Command) -> Result<(), Error> {
            match command {
                Command::Jog { .. } => {
                    self.jogging = true;
                    self.sent.lock().unwrap().push("jog".into());
                }
                Command::JogStop => {
                    self.jogging = false;
                    self.sent.lock().unwrap().push("stop".into());
                }
                _ => {}
            }
            Ok(())
        }
        fn send_line(&mut self, _line: &str) -> Result<(), Error> {
            Ok(())
        }
    }

    #[test]
    fn jogs_that_cross_are_normal_and_do_not_trip_the_guard_because_the_move_in_progress_is_cancelled_first(
    ) {
        let sent = Arc::new(Mutex::new(Vec::new()));
        let inner = Sticky {
            sent: Arc::clone(&sent),
            jogging: false,
        };
        let guard =
            crate::guard::GuardHandle::new(Box::new(inner), crate::motioncfg::GuardCfg::default());
        let mut s = Session::new(Box::new(guard.clone()));
        s.step_mm = 1.0;
        s.jog(0, 1.0);
        // the first jog still moves: another axis, then the other way
        s.jog(1, 1.0);
        s.jog(1, -1.0);
        assert_eq!(guard.tripped(), None, "{:?}", s.log);
        assert!(s.log.iter().all(|l| !l.contains("jog:")), "{:?}", s.log);
        assert_eq!(
            sent.lock().unwrap().join(","),
            "jog,stop,jog,stop,jog",
            "a stop goes before each jog that changes the axis or the way"
        );
        // the same axis and way again renews the target: no stop between
        s.jog(1, -1.0);
        assert_eq!(sent.lock().unwrap().join(","), "jog,stop,jog,stop,jog,jog");
        assert_eq!(guard.tripped(), None);
    }

    #[test]
    fn with_the_cancel_turned_off_a_crossing_jog_is_a_second_move_order_and_the_guard_stops_the_machine(
    ) {
        let sent = Arc::new(Mutex::new(Vec::new()));
        let inner = Sticky {
            sent: Arc::clone(&sent),
            jogging: false,
        };
        let guard =
            crate::guard::GuardHandle::new(Box::new(inner), crate::motioncfg::GuardCfg::default());
        let mut s = Session::new(Box::new(guard.clone()));
        s.cancel_crossing_jog = false;
        s.step_mm = 1.0;
        s.jog(0, 1.0);
        s.jog(1, 1.0);
        assert_eq!(guard.tripped(), Some(crate::guard::Trip::TwoMoves));
        // the second order was not sent, and the machine was told to stop
        assert_eq!(sent.lock().unwrap().join(","), "jog,stop");
        assert!(s.log.iter().any(|l| l.contains("jog:")), "{:?}", s.log);
    }

    #[test]
    fn jogs_in_any_order_from_a_fixed_seed_never_trip_the_guard_and_never_send_two_moves_in_a_row()
    {
        let mut seed: u64 = 0x1065_EED0_0000_0005;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let sent = Arc::new(Mutex::new(Vec::new()));
        let inner = Sticky {
            sent: Arc::clone(&sent),
            jogging: false,
        };
        let guard =
            crate::guard::GuardHandle::new(Box::new(inner), crate::motioncfg::GuardCfg::default());
        let mut s = Session::new(Box::new(guard.clone()));
        s.step_mm = 1.0;
        for _ in 0..300 {
            let axis = (next() % 3) as usize;
            let dir = if next() % 2 == 0 { 1.0 } else { -1.0 };
            s.jog(axis, dir);
            assert_eq!(guard.tripped(), None, "{:?}", s.log);
        }
        // two different jog orders never follow each other without a stop between them
        let sent = sent.lock().unwrap();
        let mut last_jog_since_stop = false;
        let mut jogs = 0;
        for c in sent.iter() {
            match c.as_str() {
                "jog" => {
                    jogs += 1;
                    last_jog_since_stop = true;
                }
                "stop" => assert!(last_jog_since_stop, "a stop with nothing to stop"),
                other => panic!("{other}"),
            }
            if c == "stop" {
                last_jog_since_stop = false;
            }
        }
        assert_eq!(jogs, 300);
    }

    #[test]
    fn a_jog_after_the_last_one_ended_needs_no_stop_and_without_the_cancel_the_guard_would_have_tripped(
    ) {
        let sent = Arc::new(Mutex::new(Vec::new()));
        let inner = Sticky {
            sent: Arc::clone(&sent),
            jogging: false,
        };
        // the guard alone, as the session used to meet it: a crossing jog is a second move order
        let mut bare = crate::guard::Guarded::new(inner, crate::motioncfg::GuardCfg::default());
        let jog = |axis| Command::Jog {
            axis,
            distance_mm: 1.0,
            feed_mm_min: 300.0,
        };
        assert!(bare.command(jog(0)).is_ok());
        assert!(bare.command(jog(1)).is_err());
        assert_eq!(bare.tripped(), Some(crate::guard::Trip::TwoMoves));
        // the session over a controller that is idle when the next jog comes: nothing is cancelled and nothing is said
        let (mut s, sent) = touchy_session(|_, _| 0.0);
        s.step_mm = 1.0;
        s.jog(0, 1.0);
        s.jog(1, 1.0);
        assert!(sent.lock().unwrap().is_empty());
        assert!(s.log.is_empty(), "{:?}", s.log);
    }

    #[test]
    fn the_tool_setter_moves_nothing_unconfirmed_and_gives_the_length_of_a_believable_touch() {
        let cfg = SetterCfg::default();
        let (mut s, sent) = touchy_session(|_, _| -23.5);
        assert!(!s.measure_tool(&cfg, false));
        assert!(sent.lock().unwrap().is_empty());
        assert_eq!(s.setter_length, None);
        assert!(s.measure_tool(&cfg, true));
        s.wait_idle();
        assert_eq!(s.setter_length, Some(-23.5), "{:?}", s.log);
        assert!(sent.lock().unwrap().iter().any(|l| l == "G38.2 Z-60 F60"));
        // a touch outside the travel of the cycle is not believed and gives no length
        let (mut s, _) = touchy_session(|_, _| -90.0);
        assert!(s.measure_tool(&cfg, true));
        s.wait_idle();
        assert_eq!(s.setter_length, None);
        assert!(
            s.log.iter().any(|l| l.contains("outside the travel")),
            "{:?}",
            s.log
        );
        // the simulator has no probe: refused, with the reason
        let mut sim = session();
        assert!(!sim.measure_tool(&cfg, true));
        assert!(
            sim.log.iter().any(|l| l.contains("no probe")),
            "{:?}",
            sim.log
        );
    }

    #[test]
    fn a_jog_stops_at_the_end_of_the_travel_when_the_limits_are_on() {
        let mut s = session();
        s.step_mm = 10.0;
        s.soft_limits = Some([(0.0, 15.0), (0.0, 100.0), (-50.0, 50.0)]);
        s.jog(0, 1.0);
        assert_eq!(s.status.machine_mm[0], 10.0);
        // the second step would reach 20: it is shortened to the 15 where the travel ends
        s.jog(0, 1.0);
        assert_eq!(s.status.machine_mm[0], 15.0);
        assert!(s.log.iter().any(|l| l.contains("shortened")), "{:?}", s.log);
        // from the limit outwards nothing moves
        s.jog(0, 1.0);
        assert_eq!(s.status.machine_mm[0], 15.0);
        assert!(
            s.log.iter().any(|l| l.contains("end of its travel")),
            "{:?}",
            s.log
        );
        // and the way back is free
        s.jog(0, -1.0);
        assert_eq!(s.status.machine_mm[0], 5.0);
    }

    #[test]
    fn the_limits_that_need_homing_hold_only_after_the_machine_was_homed() {
        let mut s = session();
        s.step_mm = 10.0;
        s.soft_limits = Some([(0.0, 15.0), (0.0, 100.0), (-50.0, 50.0)]);
        s.limits_need_homing = true;
        assert!(!s.homed());
        // before homing nothing stops the step past the limit
        s.jog(0, 1.0);
        s.jog(0, 1.0);
        assert_eq!(s.status.machine_mm[0], 20.0);
        s.jog(0, -1.0);
        s.jog(0, -1.0);
        s.home();
        assert!(s.homed());
        s.jog(0, 1.0);
        s.jog(0, 1.0);
        assert_eq!(s.status.machine_mm[0], 15.0);
        // a reset takes the homing away
        s.estop();
        assert!(!s.homed());
    }

    #[test]
    fn a_probe_moves_nothing_without_the_confirmation_and_not_on_a_controller_without_a_probe() {
        let (mut s, sent) = touchy_session(|_, _| 0.0);
        assert!(!s.probe_z(&ProbeCfg::default(), false));
        assert!(!s.probe_grid([0.0, 0.0, 10.0, 10.0], 2, 2, &ProbeCfg::default(), false));
        assert!(sent.lock().unwrap().is_empty());
        assert!(
            s.log.iter().any(|l| l.contains("not confirmed")),
            "{:?}",
            s.log
        );
        // the simulator has no probe
        let mut sim = session();
        assert!(!sim.probe_z(&ProbeCfg::default(), true));
        assert!(
            sim.log.iter().any(|l| l.contains("no probe")),
            "{:?}",
            sim.log
        );
        // values that make no sense are refused before anything is sent
        let bad = ProbeCfg {
            depth_mm: 0.0,
            ..ProbeCfg::default()
        };
        assert!(!s.probe_z(&bad, true));
        assert!(sent.lock().unwrap().is_empty());
        // a program that runs does not share the controller with a probe
        s.load("G0 X1");
        s.start();
        assert!(!s.probe_z(&ProbeCfg::default(), true));
    }

    #[test]
    fn the_z_probe_sends_its_cycle_through_the_emitter() {
        let (mut s, sent) = touchy_session(|_, _| 0.0);
        let cfg = ProbeCfg {
            plate_mm: 1.2,
            ..ProbeCfg::default()
        };
        assert!(s.probe_z(&cfg, true));
        s.wait_idle();
        assert_eq!(
            *sent.lock().unwrap(),
            vec![
                "G21",
                "G91",
                "G38.2 Z-10 F20",
                "G90",
                "G10 L20 P0 Z1.2",
                "G91",
                "G0 Z2",
                "G90"
            ]
        );
        assert!(
            s.log.iter().any(|l| l.contains("zero of Z is set")),
            "{:?}",
            s.log
        );
        assert!(!s.busy());
    }

    #[test]
    fn a_probe_cycle_runs_in_millimetres_and_a_controller_in_inches_goes_back_to_them() {
        let (mut s, sent) = touchy_session(|_, _| 0.0);
        s.mdi("G20");
        sent.lock().unwrap().clear();
        assert!(s.probe_z(&ProbeCfg::default(), true));
        s.wait_idle();
        {
            let lines = sent.lock().unwrap();
            assert_eq!(lines.first().map(String::as_str), Some("G21"), "{lines:?}");
            assert_eq!(lines.last().map(String::as_str), Some("G20"), "{lines:?}");
        }
        assert!(s.modal.inches);
        sent.lock().unwrap().clear();
        assert!(s.probe_grid([0.0, 0.0, 10.0, 10.0], 2, 2, &ProbeCfg::default(), true));
        s.wait_idle();
        assert_eq!(
            sent.lock().unwrap().first().map(String::as_str),
            Some("G21")
        );
        assert_eq!(sent.lock().unwrap().last().map(String::as_str), Some("G20"));
        // a touch that fails puts the distances and the units back too
        let (mut c, sent) = Touchy::new(|_, _| 0.0);
        c.miss_at = Some(1);
        let mut s = Session::new(Box::new(c));
        s.mdi("G20");
        assert!(s.probe_z(&ProbeCfg::default(), true));
        s.wait_idle();
        let lines = sent.lock().unwrap().clone();
        assert_eq!(&lines[lines.len() - 2..], ["G90", "G20"], "{lines:?}");
        assert!(s.modal.inches);
        // in millimetres nothing else is added at the end
        let (mut s, sent) = touchy_session(|_, _| 0.0);
        assert!(s.probe_z(&ProbeCfg::default(), true));
        s.wait_idle();
        assert_eq!(sent.lock().unwrap().last().map(String::as_str), Some("G90"));
    }

    #[test]
    fn zero_writes_the_work_system_in_use_and_not_always_the_first() {
        let (mut s, sent) = touchy_session(|_, _| 0.0);
        // in G54 the order of the controller is used
        s.zero();
        assert!(sent.lock().unwrap().is_empty());
        s.mdi("G55");
        s.zero_axes(&[0, 2]);
        assert_eq!(
            sent.lock().unwrap().last().map(String::as_str),
            Some("G10 L20 P0 X0 Z0")
        );
        s.zero();
        assert_eq!(
            sent.lock().unwrap().last().map(String::as_str),
            Some("G10 L20 P0 X0 Y0 Z0")
        );
        assert!(!s.hand_shift);
    }

    #[test]
    fn a_shift_of_the_zero_given_by_hand_is_kept_in_mind_until_it_is_taken_back() {
        let (mut s, _) = touchy_session(|_, _| 0.0);
        assert!(!s.hand_shift);
        s.mdi("G92 X0");
        assert!(s.hand_shift);
        s.mdi("G4 P1");
        assert!(s.hand_shift);
        s.mdi("G92.1");
        assert!(!s.hand_shift);
        s.mdi("G92 Y0");
        s.mdi("(a note) G92.2");
        assert!(!s.hand_shift);
        s.mdi("G92.3");
        assert!(s.hand_shift);
    }

    #[test]
    fn the_log_and_the_reasons_not_to_resume_are_told_in_the_language_of_the_window() {
        let mut s = session();
        s.set_lang("es");
        s.load("G91\nG1 X1 F10\nG1 X2");
        assert!(!s.resume_at_safe(2, 4.0, 0.0));
        assert!(
            s.log
                .last()
                .is_some_and(|l| l.starts_with("reanudación:") && l.contains("(G91)")),
            "{:?}",
            s.log
        );
        s.load("G0 X1\nG92 X0\nG0 X2");
        let why = s.resume_refusal(2).unwrap();
        assert!(why.contains("línea 2") && why.contains("línea 3"), "{why}");
        s.load("G0 X1");
        s.start();
        s.jog(0, 1.0);
        assert!(s
            .log
            .iter()
            .any(|l| l == "desplazamiento manual: hay un programa en marcha"));
        s.pump(10);
        assert!(
            s.log.iter().any(|l| l == "programa terminado"),
            "{:?}",
            s.log
        );
        s.set_lang("de");
        s.estop();
        assert_eq!(s.log.last().map(String::as_str), Some("NOT-HALT"));
    }

    #[test]
    fn a_grid_builds_the_height_map_that_then_corrects_the_program_and_can_be_put_back() {
        let (mut s, _) = touchy_session(|x, _| -1.0 + x * 0.1);
        assert!(s.probe_grid([0.0, 0.0, 10.0, 10.0], 3, 3, &ProbeCfg::default(), true));
        s.wait_idle();
        let map = s.height_map.clone().expect("a map");
        assert_eq!((map.nx, map.ny), (3, 3));
        assert!(
            (map.offset_at(10.0, 10.0) - 1.0).abs() < 1e-9,
            "{:?}",
            map.z
        );
        // no map, no correction
        let (mut empty, _) = touchy_session(|_, _| 0.0);
        empty.load("G1 X1 F10");
        assert!(!empty.apply_height_map(1.0));
        // the program is corrected, and put back as it was
        s.load("M3 S100\nG0 X0 Y0\nG1 Z-1 F100\nG1 X10\nM30");
        let original = s.program.clone();
        assert!(s.apply_height_map(5.0));
        assert!(s.corrected() && s.program.len() > original.len());
        assert!(!s.apply_height_map(5.0), "it is not corrected twice");
        assert!(
            s.program.iter().any(|l| l == "G1 X10 Y0 Z0"),
            "{:?}",
            s.program
        );
        assert!(s.drop_height_correction());
        assert!(!s.corrected() && s.program == original);
        // a program that leaves the probed area is not corrected
        s.load("G0 X0 Y0 Z1\nG1 X50 F10");
        assert!(!s.apply_height_map(5.0));
        assert!(
            s.log.iter().any(|l| l.contains("probed area")),
            "{:?}",
            s.log
        );
        // loading another program forgets the way back
        s.load("G0 X0 Y0 Z1\nG1 X1 F10");
        assert!(s.apply_height_map(5.0));
        s.load("G1 X2 F10");
        assert!(!s.corrected());
    }

    #[test]
    fn a_touch_that_misses_ends_the_grid_and_puts_the_distances_back() {
        let (mut c, sent) = Touchy::new(|_, _| 0.0);
        c.miss_at = Some(2);
        let mut s = Session::new(Box::new(c));
        assert!(s.probe_grid([0.0, 0.0, 10.0, 10.0], 2, 2, &ProbeCfg::default(), true));
        s.wait_idle();
        assert!(s.height_map.is_none());
        assert!(
            s.log.iter().any(|l| l.contains("probe (G38.2")),
            "{:?}",
            s.log
        );
        assert_eq!(sent.lock().unwrap().last().map(String::as_str), Some("G90"));
    }
}
