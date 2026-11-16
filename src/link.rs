//! The controller the program drives: the simulated one, or GRBL on a serial port (`--grbl PORT[@BAUD]`).

use std::time::Duration;

use eh3d_controller::sim::SimController;

use crate::cli::Options;
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
}
