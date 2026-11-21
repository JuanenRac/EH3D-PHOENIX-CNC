//! The controller the program drives: the simulated one, GRBL, grblHAL or Marlin on a serial port (`--grbl`, `--grblhal` and `--marlin`,
//! each `PORT[@BAUD]`), Klipper through Moonraker over HTTP (`--klipper HOST[:PORT]`, with the `klipper.*` keys of the motion file), or the
//! dialect of one of them on a simulated cable (`--sim grbl|grblhal|marlin|klipper`, see `eh3d_controller::wire`).

use std::time::Duration;

use eh3d_controller::grbl::GrblController;
use eh3d_controller::grblsim::{Flavor, GrblDevice};
use eh3d_controller::httprpc;
use eh3d_controller::klipper::{KlipperController, E_HOME_FIRST};
use eh3d_controller::klippersim::{KlipperDevice, RpcLink};
use eh3d_controller::marlin::{MarlinController, MarlinDevice, E_HOMING};
use eh3d_controller::sim::SimController;
use eh3d_controller::wire::{Impair, WireLink};
use eh3d_controller::Family;

use crate::cli::Options;
use crate::guard::GuardHandle;
use crate::motioncfg::{GuardCfg, KlipperCfg, MotionCfg, WireCfg};
use crate::session::BoxedController;

/// The baud rate of GRBL 1.1 when the option names only the port.
pub const GRBL_BAUD: u32 = 115_200;
/// The baud rate Marlin is usually flashed with.
pub const MARLIN_BAUD: u32 = 250_000;

/// The controllers the program speaks to: three over a line of text and Klipper through Moonraker over HTTP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    Grbl,
    GrblHal,
    Marlin,
    Klipper,
}

impl Dialect {
    pub const ALL: [Dialect; 4] = [
        Dialect::Grbl,
        Dialect::GrblHal,
        Dialect::Marlin,
        Dialect::Klipper,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Dialect::Grbl => "grbl",
            Dialect::GrblHal => "grblhal",
            Dialect::Marlin => "marlin",
            Dialect::Klipper => "klipper",
        }
    }

    pub fn from_key(key: &str) -> Option<Dialect> {
        let k = key.trim().to_ascii_lowercase();
        Dialect::ALL.into_iter().find(|d| d.key() == k)
    }

    pub fn family(self) -> Family {
        match self {
            Dialect::Grbl => Family::Grbl,
            Dialect::GrblHal => Family::GrblHal,
            Dialect::Marlin => Family::Marlin,
            Dialect::Klipper => Family::Klipper,
        }
    }

    /// The baud rate used when the option names only the port (0 for Klipper, which is not on a serial port).
    pub fn default_baud(self) -> u32 {
        match self {
            Dialect::Marlin => MARLIN_BAUD,
            Dialect::Klipper => 0,
            _ => GRBL_BAUD,
        }
    }

    /// The number the simulator of the dialect answers with when a line is made to fail (see [`simulated_with`]).
    pub fn failing_code(self) -> u32 {
        match self {
            Dialect::Grbl => 22,
            Dialect::GrblHal => 46,
            Dialect::Marlin => E_HOMING,
            Dialect::Klipper => E_HOME_FIRST,
        }
    }
}

/// The cable of the settings as the simulator takes it.
pub fn impair_of(wire: &WireCfg) -> Impair {
    Impair {
        seed: wire.seed,
        chunk_max: wire.chunk_max,
        noise_per_mille: wire.noise_per_mille,
        flip_per_mille: wire.flip_per_mille,
        mute_from: None,
        mute_lines: 0,
    }
}

/// A simulated controller of the dialect behind a cable, with a line (from 1) that fails with the failure of the dialect
/// ([`Dialect::failing_code`]) when one is asked for.
pub fn simulated_with(
    d: Dialect,
    impair: Impair,
    numbering: bool,
    fail_at: Option<usize>,
) -> BoxedController {
    match d {
        Dialect::Grbl | Dialect::GrblHal => {
            let flavor = if d == Dialect::Grbl {
                Flavor::Grbl
            } else {
                Flavor::GrblHal
            };
            let mut dev = GrblDevice::new(flavor);
            if let Some(n) = fail_at {
                dev.fail.insert(n, d.failing_code());
            }
            Box::new(GrblController::new(WireLink::new(dev, impair)).as_family(d.family()))
        }
        Dialect::Marlin => {
            let mut dev = MarlinDevice::new();
            if let Some(n) = fail_at {
                dev.fail.insert(n, "Error:Homing Failed".to_string());
            }
            Box::new(MarlinController::new(WireLink::new(dev, impair)).with_numbering(numbering))
        }
        Dialect::Klipper => {
            let mut dev = KlipperDevice::new();
            if let Some(n) = fail_at {
                dev.fail.insert(n, "Must home axis first: X".to_string());
            }
            Box::new(KlipperController::new(RpcLink::new(dev, impair)))
        }
    }
}

/// A simulated controller of the dialect with the cable of the settings.
pub fn simulated(d: Dialect, wire: &WireCfg) -> BoxedController {
    simulated_with(d, impair_of(wire), wire.marlin_numbering, None)
}

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
    controller_for_cfg(o, busy, &WireCfg::default())
}

/// As [`controller_for_with`] with everything the motion file says: the cable, the numbering of Marlin and the `klipper.*` keys.
pub fn controller_for_motion(
    o: &Options,
    busy: Duration,
    motion: &MotionCfg,
) -> Result<BoxedController, String> {
    controller_for_parts(o, busy, &motion.wire, &motion.klipper)
}

/// As [`controller_for_with`] with the cable and the numbering of Marlin of the motion file.
pub fn controller_for_cfg(
    o: &Options,
    busy: Duration,
    wire: &WireCfg,
) -> Result<BoxedController, String> {
    controller_for_parts(o, busy, wire, &KlipperCfg::default())
}

/// Splits `HOST[:PORT]` (an IPv6 address in brackets when it has a port) into the host and the port, when there is one.
pub fn parse_endpoint(text: &str) -> Result<(String, Option<u16>), String> {
    let port_of = |p: &str| -> Result<u16, String> {
        match p.parse::<u16>() {
            Ok(n) if n >= 1 => Ok(n),
            _ => Err(format!("'{p}' is not a port (1 to 65535)")),
        }
    };
    let (host, port) = if let Some(rest) = text.strip_prefix('[') {
        let (host, tail) = rest
            .split_once(']')
            .ok_or("an address in brackets has to close them")?;
        match tail {
            "" => (host, None),
            t => match t.strip_prefix(':') {
                Some(p) => (host, Some(port_of(p)?)),
                None => return Err("after ] only :PORT can follow".to_string()),
            },
        }
    } else if text.matches(':').count() == 1 {
        let (host, p) = text.split_once(':').unwrap_or((text, ""));
        (host, Some(port_of(p)?))
    } else {
        (text, None)
    };
    if host.is_empty() {
        return Err("the address is empty".to_string());
    }
    Ok((host.to_string(), port))
}

/// The words for a failure to start the HTTP link of Klipper, in the language: the number it carries, or the text it came with.
pub fn klipper_error_text(e: &std::io::Error, lang: &str) -> String {
    eh3d_controller::dialtext::explain(
        Family::Klipper,
        "error",
        httprpc::http_error_code(e),
        &e.to_string(),
        lang,
    )
}

/// Klipper through Moonraker over HTTP at `HOST[:PORT]`, with the settings of the `klipper.*` keys. Nothing is connected until the first
/// question is asked; an address the program may not reach is refused here.
fn open_klipper(text: &str, cfg: &KlipperCfg, lang: &str) -> Result<BoxedController, String> {
    let (host, port) = parse_endpoint(text)?;
    let http = cfg
        .http(Some((&host, port)))
        .map_err(|p| format!("klipper: {}", p.why.text(lang)))?;
    let controller = httprpc::klipper_controller(http).map_err(|e| klipper_error_text(&e, lang))?;
    Ok(Box::new(controller))
}

fn controller_for_parts(
    o: &Options,
    busy: Duration,
    wire: &WireCfg,
    klipper: &KlipperCfg,
) -> Result<BoxedController, String> {
    if let Some(text) = o.klipper.as_deref() {
        let lang = o.lang.map_or("en", |l| l.code());
        return open_klipper(text, klipper, lang);
    }
    if let Some((dialect, text)) = o.real_link() {
        let (port, baud) = parse_port(text)?;
        let baud = if text.contains('@') {
            baud
        } else {
            dialect.default_baud()
        };
        return open_real(dialect, &port, baud, busy, wire.marlin_numbering);
    }
    if let Some(name) = o.sim.as_deref() {
        let d = Dialect::from_key(name).ok_or_else(|| format!("unknown dialect '{name}'"))?;
        return Ok(simulated(d, wire));
    }
    Ok(Box::new(SimController::new()))
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
    Ok(wrap(controller, !o.is_real(), cfg))
}

/// As [`guarded_for_with`] with everything the motion file says: the guard, the simulated cable and the numbering of Marlin.
pub fn guarded_for_motion(
    o: &Options,
    busy: Duration,
    motion: &MotionCfg,
) -> Result<Linked, String> {
    let controller = controller_for_parts(o, busy, &motion.wire, &motion.klipper)?;
    Ok(wrap(controller, !o.is_real(), &motion.guard))
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
fn open_real(
    dialect: Dialect,
    port: &str,
    baud: u32,
    busy: Duration,
    numbering: bool,
) -> Result<BoxedController, String> {
    use eh3d_controller::serial::SerialTransport;

    let transport = SerialTransport::open(port, baud).map_err(|e| format!("{port}: {e}"))?;
    Ok(match dialect {
        Dialect::Grbl => Box::new(GrblController::new(transport).with_busy_limit(busy)),
        Dialect::GrblHal => Box::new(
            GrblController::new(transport)
                .with_busy_limit(busy)
                .as_family(Family::GrblHal),
        ),
        Dialect::Marlin => Box::new(
            MarlinController::new(transport)
                .with_busy_limit(busy)
                .with_numbering(numbering),
        ),
        Dialect::Klipper => {
            return Err("Klipper is not on a serial port: use --klipper".to_string())
        }
    })
}

#[cfg(not(feature = "serial"))]
fn open_real(
    _dialect: Dialect,
    _port: &str,
    _baud: u32,
    _busy: Duration,
    _numbering: bool,
) -> Result<BoxedController, String> {
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
    fn the_dialects_have_keys_families_and_bauds_of_their_own() {
        for d in Dialect::ALL {
            assert_eq!(Dialect::from_key(d.key()), Some(d));
            assert_eq!(Dialect::from_key(&d.key().to_uppercase()), Some(d));
        }
        assert_eq!(Dialect::from_key("smoothie"), None);
        assert_eq!(Dialect::GrblHal.family(), Family::GrblHal);
        assert_eq!(Dialect::Marlin.family(), Family::Marlin);
        assert_eq!(Dialect::Marlin.default_baud(), 250_000);
        assert_eq!(Dialect::Grbl.default_baud(), GRBL_BAUD);
    }

    #[test]
    fn a_simulated_dialect_is_chosen_with_sim_and_a_real_port_with_its_own_option() {
        for (args, family) in [
            ("--sim grbl", Family::Grbl),
            ("--sim grblhal", Family::GrblHal),
            ("--sim MARLIN", Family::Marlin),
        ] {
            let o = Options::parse(args.split_whitespace().map(String::from)).unwrap();
            let c = controller_for(&o).unwrap();
            assert_eq!(c.family(), family, "{args}");
        }
        // in the guard, and the guard does not take the jump of a simulator for a fault
        let o = Options::parse(["--sim".to_string(), "marlin".to_string()]).unwrap();
        let mut l =
            guarded_for_motion(&o, Duration::from_millis(10), &MotionCfg::default()).unwrap();
        assert!(l.guard.is_some());
        assert!(l.controller.status().is_ok());
        #[cfg(not(feature = "serial"))]
        for opt in ["--grblhal", "--marlin"] {
            let o = Options::parse([opt.to_string(), "COM3".to_string()]).unwrap();
            match controller_for(&o) {
                Err(e) => assert!(e.contains("serial"), "{opt}: {e}"),
                Ok(_) => panic!("a build without serial opened a port"),
            }
        }
    }

    #[test]
    fn the_cable_of_the_motion_file_reaches_the_simulator() {
        let (cfg, problems) = MotionCfg::parse(
            "wire_chunk_max = 2\nwire_noise = 300\nwire_seed = 9\nmarlin_numbering = on\n",
        );
        assert!(problems.is_empty());
        let i = impair_of(&cfg.wire);
        assert_eq!(
            (i.seed, i.chunk_max, i.noise_per_mille, i.flip_per_mille),
            (9, 2, 300, 0)
        );
        let mut c = simulated(Dialect::Marlin, &cfg.wire);
        c.send_line("G1 X3 F100").unwrap();
        assert_eq!(c.status().unwrap().machine_mm[0], 3.0);
    }

    #[test]
    fn a_simulated_line_that_fails_gives_the_failure_of_its_dialect() {
        for d in Dialect::ALL {
            let mut c = simulated_with(d, Impair::clean(), false, Some(1));
            let e = c.send_line("G1 X1 F100").unwrap_err();
            assert_eq!(e.code(), Some(d.failing_code()), "{d:?}: {e:?}");
        }
    }

    #[test]
    fn a_simulated_grbl_alarms_on_a_move_past_its_travel_and_unlock_clears_the_lock() {
        use eh3d_controller::{Command, Error, State};
        let mut c = simulated(Dialect::Grbl, &WireCfg::default());
        let e = c.send_line("G0 X900").unwrap_err();
        assert!(matches!(e, Error::Alarm(_)), "{e:?}");
        assert_eq!(e.code(), Some(2));
        assert_eq!(c.status().unwrap().state, State::Alarm);
        assert_eq!(c.send_line("G0 X1").unwrap_err().code(), Some(9));
        c.command(Command::Unlock).unwrap();
        c.send_line("G0 X1").unwrap();
    }

    #[test]
    fn an_urgent_reset_stops_a_simulated_marlin_and_it_refuses_the_moves_after_it() {
        use eh3d_controller::urgent::Urgent;
        use eh3d_controller::Error;
        let mut c = simulated(Dialect::Marlin, &WireCfg::default());
        let channel = c.urgent_channel().expect("Marlin takes urgent orders");
        channel.send(Urgent::Reset);
        assert!(c.send_line("G1 X5 F100").is_err());
        let e = c.send_line("G1 X6 F100").unwrap_err();
        assert!(matches!(e, Error::Alarm(_)), "{e:?}");
        assert_eq!(e.code(), Some(eh3d_controller::marlin::E_HALTED));
    }

    #[test]
    fn the_address_of_klipper_is_host_or_host_and_port() {
        let ok = |t: &str| parse_endpoint(t).unwrap();
        assert_eq!(ok("192.168.0.50"), ("192.168.0.50".to_string(), None));
        assert_eq!(
            ok("printer.local:80"),
            ("printer.local".to_string(), Some(80))
        );
        assert_eq!(ok("[::1]"), ("::1".to_string(), None));
        assert_eq!(ok("[::1]:7125"), ("::1".to_string(), Some(7125)));
        assert_eq!(ok("::1"), ("::1".to_string(), None));
        for bad in [
            "", ":80", "a:0", "a:65536", "a:x", "[::1", "[::1]x", "[]:80",
        ] {
            assert!(parse_endpoint(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn klipper_is_simulated_with_sim_and_sits_in_the_guard_like_the_others() {
        let o = Options::parse(["--sim".to_string(), "KLIPPER".to_string()]).unwrap();
        let mut l =
            guarded_for_motion(&o, Duration::from_millis(10), &MotionCfg::default()).unwrap();
        assert_eq!(l.controller.family(), Family::Klipper);
        assert!(l.guard.is_some());
        assert!(l.controller.status().is_ok());
        assert!(!o.is_real());
        assert_eq!(Dialect::from_key("klipper"), Some(Dialect::Klipper));
        assert_eq!(Dialect::Klipper.family(), Family::Klipper);
        assert_eq!(Dialect::Klipper.default_baud(), 0);
    }

    #[test]
    fn a_simulated_klipper_comes_through_noisy_cables_from_a_row_of_seeds() {
        for seed in 1..=8u64 {
            let wire = WireCfg {
                seed,
                chunk_max: 0,
                noise_per_mille: 400,
                flip_per_mille: 0,
                marlin_numbering: false,
            };
            let mut c = simulated(Dialect::Klipper, &wire);
            c.command(eh3d_controller::Command::Home(vec![])).unwrap();
            c.send_line("G1 X12 Y3 F600").unwrap();
            let st = c.status().unwrap();
            assert_eq!(
                (st.machine_mm[0], st.machine_mm[1]),
                (12.0, 3.0),
                "seed {seed}"
            );
        }
    }

    #[test]
    fn an_address_klipper_may_not_be_reached_at_is_refused_before_any_connection() {
        let motion = MotionCfg::default();
        for addr in ["8.8.8.8", "example.org", "0.0.0.0:7125", "224.0.0.1"] {
            let o = Options::parse(["--klipper".to_string(), addr.to_string()]).unwrap();
            match guarded_for_motion(&o, Duration::from_millis(10), &motion) {
                Err(e) => assert!(e.starts_with("klipper: "), "{addr}: {e}"),
                Ok(_) => panic!("{addr} was taken"),
            }
        }
        // in the language of the window
        let o = Options::parse(["--klipper", "8.8.8.8", "--lang", "es"].map(String::from)).unwrap();
        let e = controller_for_cfg(&o, Duration::from_millis(10), &WireCfg::default());
        assert!(e.is_err());
        // the local network is taken (nothing is connected until the first question)
        for addr in ["127.0.0.1", "192.168.0.50:7126", "localhost", "[::1]:7125"] {
            let o = Options::parse(["--klipper".to_string(), addr.to_string()]).unwrap();
            assert!(
                controller_for_motion(&o, Duration::from_millis(10), &motion).is_ok(),
                "{addr}"
            );
        }
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
        use eh3d_controller::faulty::{Fault, FaultyController};
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
