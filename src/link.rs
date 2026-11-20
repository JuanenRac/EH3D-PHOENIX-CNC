//! The controller the program drives: the simulated one, or GRBL on a serial port (`--grbl PORT[@BAUD]`).

use std::time::Duration;

use eh3d_controller::sim::SimController;

use crate::cli::Options;
use crate::guard::GuardHandle;
use crate::motioncfg::GuardCfg;
use crate::session::BoxedController;

/// The baud rate of GRBL 1.1 when the option names only the port.
pub const GRBL_BAUD: u32 = 115_200;

/// Splits `PORT[@BAUD]` into the port and the baud rate.
pub fn parse_port(text: &str) -> Result<(String, u32), String> {
    match text.rsplit_once('@') {
        Some((port, baud)) if !port.is_empty() => {
            let n: u32 = baud
                .parse()
                .map_err(|_| format!("'{baud}' is not a baud rate"))?;
            if !(1200..=2_000_000).contains(&n) {
                return Err(format!("{n} baud is out of range (1200 to 2000000)"));
            }
            Ok((port.to_string(), n))
        }
        Some(_) => Err("the port name is empty".to_string()),
        None if text.is_empty() => Err("the port name is empty".to_string()),
        None => Ok((text.to_string(), GRBL_BAUD)),
    }
}

/// The controller the options ask for, waiting the usual time for a busy machine.
pub fn controller_for(o: &Options) -> Result<BoxedController, String> {
    controller_for_with(o, eh3d_controller::grbl::MAX_BUSY_WAIT)
}

/// The controller the options ask for. `busy` is the most a line waits for its answer while the machine says it is working (the setting
/// `busy_wait` of the settings file).
pub fn controller_for_with(o: &Options, busy: Duration) -> Result<BoxedController, String> {
    let Some(text) = &o.grbl else {
        return Ok(Box::new(SimController::new()));
    };
    let (port, baud) = parse_port(text)?;
    open_grbl(&port, baud, busy)
}

/// The controller of the window and, when the guard is on, the handle the window keeps to feed it the time and to read why it stopped.
pub struct Linked {
    /// What the session holds: the guard when it is on, the controller as it is when not.
    pub controller: BoxedController,
    /// The guard, when the motion file has it on.
    pub guard: Option<GuardHandle>,
}

/// The controller the options ask for, wrapped in the guard that fails closed unless the motion file turns it off (`guard = off`). The
/// simulator is wrapped too, but it jumps to its target in one step, so the check of the position that jumps is off for it.
pub fn guarded_for_with(o: &Options, busy: Duration, cfg: &GuardCfg) -> Result<Linked, String> {
    let controller = controller_for_with(o, busy)?;
    Ok(wrap(controller, o.grbl.is_none(), cfg))
}

/// Puts a controller in the guard (or leaves it as it is when the guard is off).
pub fn wrap(controller: BoxedController, simulated: bool, cfg: &GuardCfg) -> Linked {
    if !cfg.enabled {
        return Linked {
            controller,
            guard: None,
        };
    }
    let mut cfg = cfg.clone();
    if simulated {
        cfg.check_position = false;
    }
    let handle = GuardHandle::new(controller, cfg);
    Linked {
        controller: Box::new(handle.clone()),
        guard: Some(handle),
    }
}

#[cfg(feature = "serial")]
fn open_grbl(port: &str, baud: u32, busy: Duration) -> Result<BoxedController, String> {
    use eh3d_controller::grbl::GrblController;
    use eh3d_controller::serial::SerialTransport;

    let transport = SerialTransport::open(port, baud).map_err(|e| format!("{port}: {e}"))?;
    Ok(Box::new(
        GrblController::new(transport).with_busy_limit(busy),
    ))
}

#[cfg(not(feature = "serial"))]
fn open_grbl(_port: &str, _baud: u32, _busy: Duration) -> Result<BoxedController, String> {
    Err("this build has no serial port support (build with --features serial)".to_string())
}

/// The serial ports of this computer, one per line, for `--ports`.
pub fn ports_text() -> String {
    #[cfg(feature = "serial")]
    {
        let ports = eh3d_controller::serial::SerialTransport::available();
        if ports.is_empty() {
            return "no serial ports\n".to_string();
        }
        ports.join("\n") + "\n"
    }
    #[cfg(not(feature = "serial"))]
    {
        "this build has no serial port support (build with --features serial)\n".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_port_and_the_baud_rate_are_read() {
        assert_eq!(parse_port("COM3").unwrap(), ("COM3".into(), 115_200));
        assert_eq!(
            parse_port("/dev/ttyUSB0@250000").unwrap(),
            ("/dev/ttyUSB0".into(), 250_000)
        );
        assert!(parse_port("COM3@fast").is_err());
        assert!(parse_port("COM3@10").is_err());
        assert!(parse_port("@115200").is_err());
        assert!(parse_port("").is_err());
    }

    #[test]
    fn without_the_option_the_simulator_is_driven() {
        assert!(controller_for(&Options::default()).is_ok());
    }

    #[cfg(not(feature = "serial"))]
    #[test]
    fn a_build_without_serial_says_so() {
        let o = Options {
            grbl: Some("COM3".into()),
            ..Default::default()
        };
        match controller_for(&o) {
            Err(e) => assert!(e.contains("serial")),
            Ok(_) => panic!("a build without serial opened a port"),
        }
    }

    fn after_a_jump() -> BoxedController {
        use crate::faulty::{Fault, FaultyController};
        let mut offset_mm = [0.0; eh3d_controller::AXES];
        offset_mm[0] = 900.0;
        Box::new(FaultyController::new(
            SimController::new(),
            vec![Fault::FalsePosition { from: 1, offset_mm }],
        ))
    }

    #[test]
    fn the_guard_is_on_by_default_and_the_switch_takes_it_away() {
        let cfg = GuardCfg::default();
        let l = wrap(Box::new(SimController::new()), true, &cfg);
        assert!(l.guard.is_some());
        let off = GuardCfg {
            enabled: false,
            ..GuardCfg::default()
        };
        let l = wrap(Box::new(SimController::new()), true, &off);
        assert!(l.guard.is_none());
    }

    #[test]
    fn a_real_controller_that_jumps_is_stopped_but_the_simulator_may_jump() {
        let cfg = GuardCfg::default();
        // a real one: the second status is 900 mm from the first
        let mut real = wrap(after_a_jump(), false, &cfg);
        assert!(real.controller.status().is_ok());
        assert!(real.controller.status().is_err());
        assert!(real.guard.as_ref().unwrap().tripped().is_some());
        // the simulator reaches its target in one step: the check of the jump is left off for it
        let mut sim = wrap(after_a_jump(), true, &cfg);
        assert!(sim.controller.status().is_ok());
        assert!(sim.controller.status().is_ok());
        assert_eq!(sim.guard.as_ref().unwrap().tripped(), None);
    }

    #[test]
    fn the_options_without_a_port_give_the_simulator_in_the_guard() {
        let l = guarded_for_with(
            &Options::default(),
            Duration::from_millis(10),
            &GuardCfg::default(),
        )
        .unwrap();
        assert!(l.guard.is_some());
    }
}
