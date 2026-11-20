//! The guard between the program and a controller: it fails closed. A controller that falls silent while it may be moving is stopped and
//! the machine is refused until the operator resets it; an order that moves is never sent a second time (a lost answer does not say
//! the order did not run); a read (the status) may be tried again a few times; a position that is not a number, or that jumps, is not
//! believed; and the jog follows two rules: one move order at a time (a second one is a stop) and a target that is not renewed in time
//! leaves the machine still. Whatever is doubtful degrades towards the safer state, which is stopped.
//!
//! The guard is itself a [`Controller`], so the session uses it in place of the controller it guards. It keeps no clock of its own: the
//! caller tells it the time in milliseconds with [`Guarded::at`], so that every rule is tested without waiting. Every option is a switch of
//! [`GuardCfg`] (`phoenix-cnc-motion.txt`) and the safe way is the default.

use eh3d_controller::urgent::UrgentChannel;
use eh3d_controller::{Capabilities, Command, Controller, Error, Family, State, Status, AXES};

use crate::ctltext::Guard;
use crate::motioncfg::GuardCfg;

/// Why the guard stopped the machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trip {
    /// The controller did not answer an order that may move the machine.
    Silent,
    /// The controller did not answer a read, after the tries.
    Deaf,
    /// The position was not a number or jumped too far.
    Implausible,
    /// A second move order was given while one was active.
    TwoMoves,
}

/// The guard.
pub struct Guarded<C: Controller> {
    inner: C,
    cfg: GuardCfg,
    now_ms: u64,
    tripped: Option<Trip>,
    last_position: Option<[f64; AXES]>,
    /// The jog that is active: its axis, its direction (+1 or -1) and the moment its target was last renewed.
    jog: Option<(usize, i8, u64)>,
    /// What the guard did, in order, for the log and the tests.
    pub events: Vec<String>,
    /// The reads that were tried again.
    pub retried_reads: u32,
}

/// Whether a command can move the machine (or start something that does), and so is refused after a stop.
fn moves(c: &Command) -> bool {
    match c {
        Command::Jog { .. } | Command::Home(_) | Command::Resume | Command::ZeroWork(_) => true,
        Command::Spindle { rpm, .. } => *rpm != 0.0,
        Command::Coolant(on) => *on,
        Command::JogStop
        | Command::Pause
        | Command::Stop
        | Command::EmergencyStop
        | Command::Unlock => false,
    }
}

/// What a line of G-code does: (it moves an axis, it starts something that can hurt: the spindle, the coolant, a tool change).
fn scan(line: &str) -> (bool, bool) {
    let l = line.trim().to_ascii_uppercase();
    if let Some(rest) = l.strip_prefix('$') {
        return (rest.starts_with("J=") || rest.starts_with('H'), false);
    }
    let code = l.split(['(', ';']).next().unwrap_or("");
    let (mut motion, mut outputs) = (false, false);
    let b = code.as_bytes();
    for i in 0..b.len() {
        if b[i] != b'G' && b[i] != b'M' {
            continue;
        }
        let tail: String = code[i + 1..]
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        let n: f64 = tail.parse().unwrap_or(-1.0);
        if b[i] == b'G' {
            motion |= (0.0..=3.0).contains(&n)
                || (28.0..=31.0).contains(&n)
                || (38.0..39.0).contains(&n)
                || (81.0..90.0).contains(&n);
        } else {
            outputs |= matches!(n as i32, 3 | 4 | 6 | 7 | 8);
        }
    }
    (motion, outputs)
}

/// Whether a line of G-code can move the machine or start something that hurts: a motion code, a cycle, the home, a jog, a tool change,
/// the spindle or the coolant.
pub fn line_moves(line: &str) -> bool {
    let (m, o) = scan(line);
    m || o
}

/// Whether a line of G-code moves an axis (the spindle and the coolant do not).
pub fn line_motion(line: &str) -> bool {
    scan(line).0
}

impl<C: Controller> Guarded<C> {
    pub fn new(inner: C, cfg: GuardCfg) -> Self {
        Guarded {
            inner,
            cfg,
            now_ms: 0,
            tripped: None,
            last_position: None,
            jog: None,
            events: Vec::new(),
            retried_reads: 0,
        }
    }

    pub fn inner(&self) -> &C {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut C {
        &mut self.inner
    }

    /// Tells the guard what time it is (milliseconds on any clock that only goes forward).
    pub fn at(&mut self, now_ms: u64) -> &mut Self {
        self.now_ms = now_ms;
        self
    }

    /// Why the machine is stopped, when it is.
    pub fn tripped(&self) -> Option<Trip> {
        self.tripped
    }

    /// The operator's all clear: the orders that move are taken again, and the guard forgets the last position and the jog.
    pub fn reset(&mut self) {
        if self.tripped.take().is_some() {
            self.events.push("reset".into());
        }
        self.last_position = None;
        self.jog = None;
    }

    /// The message to show for the state of the guard, in the language.
    pub fn message(&self, lang: &str) -> Option<&'static str> {
        Some(
            match self.tripped? {
                Trip::Silent | Trip::Deaf => Guard::Silent,
                Trip::Implausible => Guard::Implausible,
                Trip::TwoMoves => Guard::TwoMoves,
            }
            .text(lang),
        )
    }

    /// The safe stop: the jog is cancelled and the job stopped; if the controller will not take that, the emergency stop is given.
    fn safe_stop(&mut self) {
        let _ = self.inner.command(Command::JogStop);
        if self.inner.command(Command::Stop).is_err() {
            let _ = self.inner.command(Command::EmergencyStop);
        }
        self.jog = None;
    }

    fn trip(&mut self, why: Trip) {
        self.events.push(format!("trip {why:?}"));
        self.tripped = Some(why);
        self.safe_stop();
    }

    fn refuse(&self) -> Error {
        Error::Alarm(Guard::Tripped.text("en").to_string())
    }

    /// The renewal of time for the jog: call it often (every poll). A jog whose target was not renewed within `jog_stale_ms` is stopped.
    /// Returns whether it stopped one.
    pub fn tick(&mut self) -> bool {
        if let Some((_, _, renewed)) = self.jog {
            if self.now_ms.saturating_sub(renewed) > self.cfg.jog_stale_ms {
                self.events.push("stale jog".into());
                let _ = self.inner.command(Command::JogStop);
                self.jog = None;
                return true;
            }
        }
        false
    }

    fn plausible(&mut self, st: &Status) -> bool {
        if !self.cfg.check_position {
            return true;
        }
        let pos = st.machine_mm;
        if pos.iter().any(|v| !v.is_finite()) {
            return false;
        }
        let ok = match self.last_position {
            Some(prev) => pos
                .iter()
                .zip(prev.iter())
                .all(|(a, b)| (a - b).abs() <= self.cfg.max_jump_mm),
            None => true,
        };
        self.last_position = Some(pos);
        ok
    }
}

impl<C: Controller> Controller for Guarded<C> {
    fn family(&self) -> Family {
        self.inner.family()
    }

    fn capabilities(&self) -> Capabilities {
        self.inner.capabilities()
    }

    fn status(&mut self) -> Result<Status, Error> {
        let mut tries = 0;
        let st = loop {
            match self.inner.status() {
                Ok(st) => break st,
                Err(e @ (Error::Timeout | Error::Link(_))) => {
                    if tries >= self.cfg.status_retries {
                        if self.cfg.stop_on_silence {
                            self.trip(Trip::Deaf);
                        }
                        return Err(e);
                    }
                    tries += 1;
                    self.retried_reads += 1;
                }
                Err(e) => return Err(e),
            }
        };
        if !self.plausible(&st) {
            self.trip(Trip::Implausible);
            return Err(Error::Alarm(Guard::Implausible.text("en").to_string()));
        }
        // a jog is over when the controller is not jogging nor running any more
        if self.jog.is_some() && !matches!(st.state, State::Jog | State::Run) {
            self.jog = None;
        }
        Ok(st)
    }

    fn command(&mut self, command: Command) -> Result<(), Error> {
        if self.tripped.is_some() && moves(&command) {
            return Err(self.refuse());
        }
        if let Command::Jog {
            axis, distance_mm, ..
        } = &command
        {
            let dir: i8 = if *distance_mm < 0.0 { -1 } else { 1 };
            if self.cfg.jog_one_order {
                if let Some((a, d, _)) = self.jog {
                    if a != *axis || d != dir {
                        // a second, different order while one is active: stop, and the new one is not sent
                        self.trip(Trip::TwoMoves);
                        return Err(Error::Alarm(Guard::TwoMoves.text("en").to_string()));
                    }
                }
            }
            self.jog = Some((*axis, dir, self.now_ms));
        }
        if matches!(command, Command::Home(_) | Command::Resume)
            && self.jog.is_some()
            && self.cfg.jog_one_order
        {
            self.trip(Trip::TwoMoves);
            return Err(Error::Alarm(Guard::TwoMoves.text("en").to_string()));
        }
        if matches!(
            command,
            Command::JogStop | Command::Stop | Command::EmergencyStop
        ) {
            self.jog = None;
        }
        // an order is given once: a lost answer is not a reason to give it again
        match self.inner.command(command.clone()) {
            Err(Error::Timeout | Error::Link(_)) if moves(&command) => {
                if self.cfg.stop_on_silence {
                    self.trip(Trip::Silent);
                }
                Err(Error::Timeout)
            }
            other => other,
        }
    }

    fn send_line(&mut self, line: &str) -> Result<(), Error> {
        let moving = line_moves(line);
        if self.tripped.is_some() && moving {
            return Err(self.refuse());
        }
        if line_motion(line) && self.jog.is_some() && self.cfg.jog_one_order {
            self.trip(Trip::TwoMoves);
            return Err(Error::Alarm(Guard::TwoMoves.text("en").to_string()));
        }
        match self.inner.send_line(line) {
            Err(Error::Timeout | Error::Link(_)) => {
                // the line may or may not have run: with a line that moves, that is doubt, and doubt is a stop
                if moving && self.cfg.stop_on_silence {
                    self.trip(Trip::Silent);
                }
                Err(Error::Timeout)
            }
            other => other,
        }
    }

    fn set_travel(&mut self, travel: [(f64, f64); AXES]) -> bool {
        self.inner.set_travel(travel)
    }

    fn max_travel(&mut self) -> Result<Option<[f64; 3]>, Error> {
        self.inner.max_travel()
    }

    fn urgent_channel(&self) -> Option<UrgentChannel> {
        self.inner.urgent_channel()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::faulty::{Fault, FaultyController};
    use eh3d_controller::sim::SimController;

    fn guarded(faults: Vec<Fault>) -> Guarded<FaultyController<SimController>> {
        Guarded::new(
            FaultyController::new(SimController::new(), faults),
            GuardCfg::default(),
        )
    }

    fn jog(axis: usize, d: f64) -> Command {
        Command::Jog {
            axis,
            distance_mm: d,
            feed_mm_min: 300.0,
        }
    }

    #[test]
    fn a_line_that_moves_is_known_from_one_that_does_not() {
        for l in [
            "G0 X1",
            "g1 x2 f100",
            "G2 X1 Y1 I1",
            "G28",
            "G38.2 Z-5 F20",
            "G81 Z-2 R2",
            "M3 S1000",
            "M6 T2",
            "$H",
            "$J=G91 X1 F100",
        ] {
            assert!(line_moves(l), "{l}");
        }
        for l in [
            "G21",
            "G90",
            "G54",
            "G17",
            "M5",
            "M9",
            "(G1 X5 comment)",
            "; G0 X1",
            "$G",
            "$$",
            "$X",
            "F100",
            "",
            "S12000",
        ] {
            assert!(!line_moves(l), "{l}");
        }
    }

    #[test]
    fn a_read_is_tried_again_but_a_silence_that_goes_on_stops_the_machine() {
        // two silent reads then an answer: the tries (2) are enough and nothing is tripped
        let mut g = guarded(vec![Fault::Silence {
            from: 0,
            calls: Some(2),
        }]);
        assert!(g.status().is_ok());
        assert_eq!(g.retried_reads, 2);
        assert!(g.tripped().is_none());
        // a silence that does not end trips after the tries
        let mut g = guarded(vec![Fault::Silence {
            from: 0,
            calls: None,
        }]);
        assert_eq!(g.status(), Err(Error::Timeout));
        assert_eq!(g.retried_reads, 2);
        assert_eq!(g.tripped(), Some(Trip::Deaf));
        assert_eq!(g.message("es"), Some(Guard::Silent.text("es")));
        // it stops the machine: the stop reached the inner controller
        assert!(g.inner().delivered.iter().any(|d| d.contains("Stop")));
    }

    #[test]
    fn an_order_that_moves_is_never_sent_twice_and_the_silence_stops_the_machine() {
        let mut g = guarded(vec![Fault::Silence {
            from: 0,
            calls: Some(1),
        }]);
        assert_eq!(g.send_line("G1 X5 F100"), Err(Error::Timeout));
        // it went once, and the guard did not send it again
        let sent = g
            .inner()
            .delivered
            .iter()
            .filter(|d| d.as_str() == "G1 X5 F100")
            .count();
        assert_eq!(sent, 1);
        assert_eq!(g.tripped(), Some(Trip::Silent));
        // while stopped the orders that move are refused, the ones that make safe are not
        assert!(matches!(g.send_line("G1 X6"), Err(Error::Alarm(_))));
        assert!(matches!(g.command(jog(0, 1.0)), Err(Error::Alarm(_))));
        assert!(matches!(
            g.command(Command::Home(vec![])),
            Err(Error::Alarm(_))
        ));
        assert!(matches!(g.command(Command::Resume), Err(Error::Alarm(_))));
        assert!(g.send_line("G21").is_ok());
        assert!(g.command(Command::Pause).is_ok());
        assert!(g.command(Command::Stop).is_ok());
        assert!(g
            .command(Command::Spindle {
                rpm: 0.0,
                clockwise: true
            })
            .is_ok());
        assert!(matches!(
            g.command(Command::Spindle {
                rpm: 5000.0,
                clockwise: true
            }),
            Err(Error::Alarm(_))
        ));
        // the operator's all clear takes the orders again
        g.reset();
        assert!(g.tripped().is_none());
        assert!(g.send_line("G1 X6 F100").is_ok());
    }

    #[test]
    fn a_silence_on_a_command_that_moves_is_also_a_stop_and_not_a_resend() {
        let mut g = guarded(vec![Fault::Silence {
            from: 0,
            calls: Some(1),
        }]);
        assert_eq!(g.command(Command::Home(vec![])), Err(Error::Timeout));
        let homes = g
            .inner()
            .delivered
            .iter()
            .filter(|d| d.starts_with("Home"))
            .count();
        assert_eq!(homes, 1);
        assert_eq!(g.tripped(), Some(Trip::Silent));
    }

    #[test]
    fn the_switches_make_the_guard_less_strict_and_the_defaults_are_the_strict_ones() {
        let d = GuardCfg::default();
        assert!(d.stop_on_silence && d.check_position && d.jog_one_order);
        let cfg = GuardCfg {
            stop_on_silence: false,
            ..GuardCfg::default()
        };
        let mut g = Guarded::new(
            FaultyController::new(
                SimController::new(),
                vec![Fault::Silence {
                    from: 0,
                    calls: Some(1),
                }],
            ),
            cfg,
        );
        assert_eq!(g.send_line("G1 X5 F100"), Err(Error::Timeout));
        assert!(g.tripped().is_none());
    }

    #[test]
    fn a_position_that_jumps_or_is_not_a_number_is_not_believed() {
        let mut off = [0.0; AXES];
        off[1] = 400.0;
        let mut g = guarded(vec![Fault::FalsePosition {
            from: 1,
            offset_mm: off,
        }]);
        assert!(g.status().is_ok());
        assert!(matches!(g.status(), Err(Error::Alarm(_))));
        assert_eq!(g.tripped(), Some(Trip::Implausible));
        // the same fault with the check switched off passes
        let mut g = Guarded::new(
            FaultyController::new(
                SimController::new(),
                vec![Fault::FalsePosition {
                    from: 1,
                    offset_mm: off,
                }],
            ),
            GuardCfg {
                check_position: false,
                ..GuardCfg::default()
            },
        );
        assert!(g.status().is_ok() && g.status().is_ok());
        // not a number
        let mut nan = [0.0; AXES];
        nan[0] = f64::NAN;
        let mut g = guarded(vec![Fault::FalsePosition {
            from: 0,
            offset_mm: nan,
        }]);
        assert!(g.status().is_err());
        assert_eq!(g.tripped(), Some(Trip::Implausible));
    }

    #[test]
    fn a_second_move_order_while_one_is_active_is_a_stop_and_a_renewal_is_not() {
        let mut g = guarded(vec![]);
        g.at(0);
        assert!(g.command(jog(0, 1.0)).is_ok());
        // the same axis and direction renews the target
        g.at(100);
        assert!(g.command(jog(0, 1.0)).is_ok());
        assert!(g.tripped().is_none());
        // another axis while the first is active is two orders: stop, and the new one is not sent
        let before = g.inner().delivered.len();
        assert!(matches!(g.command(jog(1, 1.0)), Err(Error::Alarm(_))));
        assert_eq!(g.tripped(), Some(Trip::TwoMoves));
        assert!(!g.inner().delivered[before..]
            .iter()
            .any(|d| d.contains("axis: 1")));
        // the opposite direction is also a second order
        let mut g = guarded(vec![]);
        assert!(g.command(jog(0, 1.0)).is_ok());
        assert!(g.command(jog(0, -1.0)).is_err());
        assert_eq!(g.tripped(), Some(Trip::TwoMoves));
        // a line that moves while a jog is active is a second order too
        let mut g = guarded(vec![]);
        assert!(g.command(jog(2, 1.0)).is_ok());
        assert!(g.send_line("G1 X3 F100").is_err());
        assert_eq!(g.tripped(), Some(Trip::TwoMoves));
    }

    #[test]
    fn a_jog_that_ends_lets_the_next_one_in_and_a_stale_target_leaves_the_machine_still() {
        let mut g = guarded(vec![]);
        g.at(1000);
        assert!(g.command(jog(0, 1.0)).is_ok());
        // inside the time nothing happens
        g.at(1200);
        assert!(!g.tick());
        // past it the jog is stopped
        g.at(1301);
        assert!(g.tick());
        assert!(g.inner().delivered.iter().any(|d| d == "JogStop"));
        assert!(!g.tick());
        // a stopped jog frees the way for another axis
        assert!(g.command(jog(1, 1.0)).is_ok());
        // an explicit stop also frees it
        assert!(g.command(Command::JogStop).is_ok());
        assert!(g.command(jog(2, -1.0)).is_ok());
        assert!(g.tripped().is_none());
        // with the rule switched off a second order goes through
        let mut g = Guarded::new(
            FaultyController::new(SimController::new(), vec![]),
            GuardCfg {
                jog_one_order: false,
                ..GuardCfg::default()
            },
        );
        assert!(g.command(jog(0, 1.0)).is_ok() && g.command(jog(1, 1.0)).is_ok());
    }

    #[test]
    fn the_refusals_of_the_controller_pass_through_and_do_not_stop_anything() {
        let mut g = guarded(vec![Fault::ErrorAtLine { line: 2, code: 22 }]);
        assert!(g.send_line("G21").is_ok());
        let e = g.send_line("G1 X5").unwrap_err();
        assert_eq!(e.code(), Some(22));
        assert!(g.tripped().is_none());
        // and the error of a line is explained in the language of the window
        assert_eq!(
            crate::ctltext::explain("error", e.code(), "error:22", "pt"),
            "Ainda não foi definido nenhum avanço."
        );
    }

    #[test]
    fn made_up_schedules_of_faults_never_send_a_move_twice_nor_leave_a_move_after_a_trip() {
        // a fixed seed walks many schedules: whatever the faults, no line that moves reaches the machine twice, and once tripped none does
        let mut seed: u64 = 0x5eed_cafe;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for _ in 0..200 {
            let from = (next() % 8) as usize;
            let calls = if next() % 3 == 0 {
                None
            } else {
                Some((next() % 4) as usize + 1)
            };
            let mut g = guarded(vec![Fault::Silence { from, calls }]);
            let mut lines = Vec::new();
            for i in 0..12 {
                let l = format!("G1 X{} F100", i + 1);
                let tripped_before = g.tripped().is_some();
                let _ = g.send_line(&l);
                if tripped_before {
                    assert!(!g.inner().delivered.contains(&l), "sent after a trip");
                }
                lines.push(l);
                let _ = g.status();
            }
            for l in &lines {
                let n = g.inner().delivered.iter().filter(|d| *d == l).count();
                assert!(n <= 1, "{l} went {n} times");
            }
        }
    }
}
