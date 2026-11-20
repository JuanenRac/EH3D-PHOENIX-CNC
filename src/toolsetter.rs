//! The tool setter: measuring the length of the tool in the spindle by touching a fixed sensor, and writing the length into the table of
//! tools the program already has (`phoenix-cnc-tools.txt`, see `tools`), not into a table of its own. The cycle moves to the setter in
//! machine coordinates, goes down by a relative move until the sensor closes (`G38.2`), reads the machine Z where it touched and lifts.
//! The length of the tool is that Z less the Z at which a tool of length zero would touch (`setter_height`), so a longer tool, which
//! touches sooner, has the larger length.
//!
//! The lines are made here and sent through [`ProbeIo`], the same way in and out of the machine the probe uses, so the cycle is tested with
//! a stand-in setter. The cycle asks the operator to confirm that the setter is there and clear when `setter_confirm` is on (it is by
//! default): a machine is never sent to the setter on its own. Nothing was tried on a machine.

use crate::heightmap::num;
use crate::motioncfg::SetterCfg;
use crate::probe::{Act, ProbeIo};

/// What is wrong with the numbers of a setter.
pub fn check(cfg: &SetterCfg) -> Result<(), String> {
    let all = [
        cfg.x_mm,
        cfg.y_mm,
        cfg.start_z_mm,
        cfg.max_travel_mm,
        cfg.feed_mm_min,
        cfg.retract_mm,
        cfg.height_mm,
    ];
    if all.iter().any(|v| !v.is_finite()) {
        return Err("a number of the setter is not a number".into());
    }
    if cfg.max_travel_mm <= 0.0 || cfg.feed_mm_min <= 0.0 || cfg.retract_mm <= 0.0 {
        return Err("the travel, the feed and the lift of the setter must be above zero".into());
    }
    Ok(())
}

/// What the cycle does, in order: lines for the controller and the reading of the height.
pub fn script(cfg: &SetterCfg) -> Result<Vec<Act>, String> {
    check(cfg)?;
    Ok(vec![
        Act::Line("G90".into()),
        Act::Line(format!("G53 G0 Z{}", num(cfg.start_z_mm))),
        Act::Line(format!("G53 G0 X{} Y{}", num(cfg.x_mm), num(cfg.y_mm))),
        Act::Line("G91".into()),
        Act::Line(format!(
            "G38.2 Z-{} F{}",
            num(cfg.max_travel_mm),
            num(cfg.feed_mm_min)
        )),
        Act::Line("G90".into()),
        Act::Sample,
        Act::Line("G91".into()),
        Act::Line(format!("G0 Z{}", num(cfg.retract_mm))),
        Act::Line("G90".into()),
    ])
}

/// Measures the tool in the spindle and returns its length in mm. `confirmed` says that the operator confirmed that the setter is there and
/// clear; with `setter_confirm` on, the cycle does not start without it. A touch that comes from outside the travel asked for is not
/// believed. After a failure the distances are put back to absolute and the head is left where it is.
pub fn measure(io: &mut dyn ProbeIo, cfg: &SetterCfg, confirmed: bool) -> Result<f64, String> {
    let steps = script(cfg)?;
    if cfg.confirm && !confirmed {
        return Err("the operator has not confirmed that the setter is there and clear".into());
    }
    if io.halted() {
        return Err("stopped before the cycle".into());
    }
    let mut touched: Option<f64> = None;
    for act in &steps {
        let r = match act {
            Act::Mark(_) => Ok(()),
            Act::Line(l) => io.send(l).map_err(|e| {
                if l.starts_with("G38.2") {
                    format!("the tool did not touch the setter: {e}")
                } else {
                    e
                }
            }),
            Act::Sample => io.machine_z().map(|z| touched = Some(z)),
        };
        if let Err(e) = r {
            // best effort: the failure to report is the first one
            let _ = io.send("G90");
            return Err(e);
        }
    }
    let z = touched.ok_or_else(|| "no reading".to_string())?;
    let lowest = cfg.start_z_mm - cfg.max_travel_mm;
    if !z.is_finite() || z > cfg.start_z_mm + 0.05 || z < lowest - 0.05 {
        return Err(format!(
            "the touch at Z{} is outside the travel of the cycle",
            num(z)
        ));
    }
    Ok(z - cfg.height_mm)
}

/// The text of the table of tools with the length of tool `number` set to `length_mm`: only the `length=` part of its line changes (it is
/// added when the line has none) and every other line, comment and part stays as it was. `Err` when the table has no such tool or the
/// length is not a number the table accepts.
pub fn with_length(table_text: &str, number: u32, length_mm: f64) -> Result<String, String> {
    if !length_mm.is_finite() || length_mm.abs() > crate::tools::MAX_LENGTH_MM {
        return Err("the length is not a number the table accepts".into());
    }
    let nl = if table_text.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut out: Vec<String> = Vec::new();
    let mut done = false;
    for raw in table_text.split('\n') {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        let trimmed = line.trim();
        let is_it = !done
            && !trimmed.starts_with('#')
            && !trimmed.starts_with(';')
            && trimmed
                .split_once('=')
                .filter(|(k, _)| k.trim().eq_ignore_ascii_case("tool"))
                .and_then(|(_, v)| v.split('|').next()?.trim().parse::<u32>().ok())
                == Some(number);
        if !is_it {
            out.push(line.to_string());
            continue;
        }
        done = true;
        let mut parts: Vec<String> = line.split('|').map(str::to_string).collect();
        let new = format!("length={}", num(length_mm));
        match parts
            .iter_mut()
            .skip(1)
            .find(|p| p.trim().to_ascii_lowercase().starts_with("length"))
        {
            Some(p) => *p = format!(" {new} "),
            None => {
                if let Some(last) = parts.last_mut() {
                    if !last.ends_with(' ') {
                        last.push(' ');
                    }
                }
                parts.push(format!(" {new}"));
            }
        }
        out.push(parts.join("|").trim_end().to_string());
    }
    if !done {
        return Err(format!("the table has no tool {number}"));
    }
    Ok(out.join(nl))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::ToolTable;

    /// A setter whose surface is at the machine Z given; a tool of length `tool` touches when the spindle is `tool` above it.
    struct Setter {
        surface_z: f64,
        tool_length: f64,
        z: f64,
        sent: Vec<String>,
        deaf: bool,
        stop: bool,
    }

    impl Setter {
        fn new(surface_z: f64, tool_length: f64) -> Setter {
            Setter {
                surface_z,
                tool_length,
                z: 0.0,
                sent: Vec::new(),
                deaf: false,
                stop: false,
            }
        }
    }

    impl ProbeIo for Setter {
        fn send(&mut self, line: &str) -> Result<(), String> {
            self.sent.push(line.to_string());
            if line.starts_with("G38.2") {
                if self.deaf {
                    return Err("ALARM:5".into());
                }
                self.z = self.surface_z + self.tool_length;
            }
            Ok(())
        }
        fn machine_z(&mut self) -> Result<f64, String> {
            Ok(self.z)
        }
        fn halted(&self) -> bool {
            self.stop
        }
    }

    fn cfg() -> SetterCfg {
        SetterCfg {
            x_mm: 10.0,
            y_mm: 20.0,
            start_z_mm: -10.0,
            max_travel_mm: 60.0,
            height_mm: -80.0,
            ..SetterCfg::default()
        }
    }

    #[test]
    fn the_cycle_goes_to_the_setter_touches_reads_and_lifts() {
        let mut io = Setter::new(-80.0, 35.0);
        let len = measure(&mut io, &cfg(), true).unwrap();
        assert!((len - 35.0).abs() < 1e-9);
        let sent = io.sent.join(" | ");
        assert_eq!(
            sent,
            "G90 | G53 G0 Z-10 | G53 G0 X10 Y20 | G91 | G38.2 Z-60 F60 | G90 | G91 | G0 Z5 | G90"
        );
        // a longer tool has the larger length
        let mut longer = Setter::new(-80.0, 50.0);
        assert!(measure(&mut longer, &cfg(), true).unwrap() > len);
    }

    #[test]
    fn the_machine_does_not_go_to_the_setter_without_the_confirmation_unless_it_is_switched_off() {
        let mut io = Setter::new(-80.0, 35.0);
        assert!(measure(&mut io, &cfg(), false).is_err());
        assert!(io.sent.is_empty());
        let off = SetterCfg {
            confirm: false,
            ..cfg()
        };
        assert!(measure(&mut io, &off, false).is_ok());
        // a stop heard before the cycle ends it before the first line
        let mut stopped = Setter::new(-80.0, 35.0);
        stopped.stop = true;
        assert!(measure(&mut stopped, &cfg(), true).is_err());
        assert!(stopped.sent.is_empty());
    }

    #[test]
    fn a_touch_that_never_comes_or_comes_from_nowhere_is_a_failure_that_ends_absolute() {
        let mut io = Setter::new(-80.0, 35.0);
        io.deaf = true;
        let e = measure(&mut io, &cfg(), true).unwrap_err();
        assert!(e.contains("did not touch"));
        assert_eq!(io.sent.last().map(String::as_str), Some("G90"));
        // a reading outside the travel asked for (the setter is not where the settings say) is not believed
        let mut far = Setter::new(-200.0, 0.0);
        assert!(measure(&mut far, &cfg(), true)
            .unwrap_err()
            .contains("outside"));
        let mut high = Setter::new(20.0, 0.0);
        assert!(measure(&mut high, &cfg(), true).is_err());
    }

    #[test]
    fn numbers_that_make_no_sense_are_refused_before_anything_is_sent() {
        for bad in [
            SetterCfg {
                max_travel_mm: 0.0,
                ..cfg()
            },
            SetterCfg {
                feed_mm_min: -1.0,
                ..cfg()
            },
            SetterCfg {
                x_mm: f64::NAN,
                ..cfg()
            },
            SetterCfg {
                retract_mm: 0.0,
                ..cfg()
            },
        ] {
            let mut io = Setter::new(-80.0, 35.0);
            assert!(measure(&mut io, &bad, true).is_err());
            assert!(io.sent.is_empty());
        }
    }

    #[test]
    fn the_length_goes_into_the_table_of_tools_in_its_own_format_and_nothing_else_changes() {
        let table = "# my tools\ntool = 1 | name=flat | diameter=3 | length=10 | flutes=2\ntool = 2 | diameter=6\n; end\n";
        let one = with_length(table, 1, 35.25).unwrap();
        let parsed = ToolTable::parse(&one);
        assert_eq!(parsed.find(1).unwrap().length_mm, 35.25);
        assert_eq!(parsed.find(1).unwrap().flutes, 2);
        assert_eq!(parsed.find(2).unwrap().length_mm, 0.0);
        assert!(parsed.issues.is_empty(), "{:?}", parsed.issues);
        assert!(one.starts_with("# my tools\n") && one.ends_with("; end\n"));
        // a tool with no length gets one
        let two = with_length(table, 2, -4.5).unwrap();
        assert_eq!(ToolTable::parse(&two).find(2).unwrap().length_mm, -4.5);
        assert_eq!(ToolTable::parse(&two).find(1).unwrap().length_mm, 10.0);
        // a tool that is not there, or a length the table would refuse
        assert!(with_length(table, 9, 1.0).is_err());
        assert!(with_length(table, 1, f64::NAN).is_err());
        assert!(with_length(table, 1, 1e9).is_err());
        // a table with the other kind of line ends keeps it
        let crlf = with_length("tool = 3 | diameter=2\r\n", 3, 7.0).unwrap();
        assert_eq!(crlf, "tool = 3 | diameter=2 | length=7\r\n");
    }
}
