//! The probe: finding the surface of the stock by touching it. One cycle sets the Z zero of the work on a touch plate (`G38.2` down until the
//! probe closes, then the zero is set to the thickness of the plate and the head goes up), and a grid of such touches measures the height
//! map of the stock (see `heightmap`).
//!
//! This module only makes the lines and drives the order of a grid through [`ProbeIo`], so it knows nothing of the controller and is tested
//! with a stand-in surface. The orders that move a real machine are given to the session, which asks the operator to confirm them.

use crate::heightmap::{num, probe_points, HeightMap};

/// The values of the probe, all of them in the settings file.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProbeCfg {
    /// The speed of the probing move (mm/min): slow, so the stop on contact is sharp.
    pub feed_mm_min: f64,
    /// The farthest the probe goes down looking for the surface (mm); the controller raises an alarm when it does not touch.
    pub depth_mm: f64,
    /// How far the head goes up after a touch (mm).
    pub retract_mm: f64,
    /// The thickness of the touch plate (mm): the Z of the work after a touch of the plate (0 for a probe that touches the stock itself).
    pub plate_mm: f64,
    /// The height (work Z, mm) the head travels at between the points of a grid.
    pub clearance_mm: f64,
}

impl Default for ProbeCfg {
    fn default() -> Self {
        ProbeCfg {
            feed_mm_min: 20.0,
            depth_mm: 10.0,
            retract_mm: 2.0,
            plate_mm: 0.0,
            clearance_mm: 5.0,
        }
    }
}

impl ProbeCfg {
    /// Whether the values can be used; the reason when they cannot.
    pub fn check(&self) -> Result<(), String> {
        let ok = |v: f64, lo: f64, hi: f64| v.is_finite() && (lo..=hi).contains(&v);
        if !ok(self.feed_mm_min, 1.0, 1000.0) {
            return Err("the probe feed must be between 1 and 1000 mm/min".to_string());
        }
        if !ok(self.depth_mm, 0.1, 200.0) {
            return Err("the probe depth must be between 0.1 and 200 mm".to_string());
        }
        if !ok(self.retract_mm, 0.0, 100.0) {
            return Err("the retract must be between 0 and 100 mm".to_string());
        }
        if !ok(self.plate_mm, 0.0, 100.0) {
            return Err("the thickness of the plate must be between 0 and 100 mm".to_string());
        }
        if !ok(self.clearance_mm, 0.5, 200.0) {
            return Err("the clearance must be between 0.5 and 200 mm".to_string());
        }
        Ok(())
    }
}

/// The lines of the Z cycle: down until the probe closes (relative move), the work zero set to the thickness of the plate (`G10 L20 P0`,
/// the work system in use, `G54` to `G59`), back to absolute distances and up by the retract.
pub fn z_touch_off_lines(cfg: &ProbeCfg) -> Result<Vec<String>, String> {
    cfg.check()?;
    Ok(vec![
        "G91".to_string(),
        format!("G38.2 Z-{} F{}", num(cfg.depth_mm), num(cfg.feed_mm_min)),
        "G90".to_string(),
        format!("G10 L20 P0 Z{}", num(cfg.plate_mm)),
        "G91".to_string(),
        format!("G0 Z{}", num(cfg.retract_mm)),
        "G90".to_string(),
    ])
}

/// What a grid needs from the machine.
pub trait ProbeIo {
    /// Sends a line and waits for the controller to take it.
    fn send(&mut self, line: &str) -> Result<(), String>;
    /// The machine Z (mm) where the probe stands now.
    fn machine_z(&mut self) -> Result<f64, String>;
    /// The operator asked to stop (the grid ends before the next point).
    fn halted(&self) -> bool;
}

/// One thing a grid does, in order.
#[derive(Debug, Clone, PartialEq)]
pub enum Act {
    /// The start of point `k` (from 0): where a stop is heard between two points.
    Mark(usize),
    /// A line for the controller.
    Line(String),
    /// Reads the machine Z where the probe stands (the height of the contact).
    Sample,
}

/// What a grid does, point by point: it rises to the clearance, goes over the point, touches (relative move) and reads the height; at the
/// end it rises again. The distances are absolute outside the touch.
pub fn grid_script(
    area: [f64; 4],
    nx: usize,
    ny: usize,
    cfg: &ProbeCfg,
) -> Result<Vec<Act>, String> {
    cfg.check()?;
    let points = probe_points(area, nx, ny)?;
    let up = format!("G0 Z{}", num(cfg.clearance_mm));
    let mut out = vec![Act::Line("G90".to_string())];
    for (k, (x, y)) in points.iter().enumerate() {
        out.push(Act::Mark(k));
        out.push(Act::Line(up.clone()));
        out.push(Act::Line(format!("G0 X{} Y{}", num(*x), num(*y))));
        out.push(Act::Line("G91".to_string()));
        out.push(Act::Line(format!(
            "G38.2 Z-{} F{}",
            num(cfg.depth_mm),
            num(cfg.feed_mm_min)
        )));
        out.push(Act::Line("G90".to_string()));
        out.push(Act::Sample);
    }
    out.push(Act::Line(up));
    Ok(out)
}

/// Goes through the script of a grid with the machine; the head ends at the clearance.
fn touch_all(io: &mut dyn ProbeIo, script: &[Act], readings: &mut Vec<f64>) -> Result<(), String> {
    let mut point = 0;
    for act in script {
        match act {
            Act::Mark(k) => {
                point = *k;
                if io.halted() {
                    return Err(format!("stopped before point {}", k + 1));
                }
            }
            Act::Line(line) => {
                let r = io.send(line);
                if line.starts_with("G38.2") {
                    r.map_err(|e| format!("point {} did not touch: {e}", point + 1))?;
                } else {
                    r?;
                }
            }
            Act::Sample => readings.push(io.machine_z()?),
        }
    }
    Ok(())
}

/// Probes the grid and builds the map. Between the points the head rises to the clearance; after any failure the distances are put back to
/// absolute (`G90`) and the head is left where it is.
pub fn run_grid(
    io: &mut dyn ProbeIo,
    area: [f64; 4],
    nx: usize,
    ny: usize,
    cfg: &ProbeCfg,
) -> Result<HeightMap, String> {
    let script = grid_script(area, nx, ny, cfg)?;
    let mut readings = Vec::with_capacity(nx * ny);
    if let Err(e) = touch_all(io, &script, &mut readings) {
        // best effort: the failure to report is the first one
        let _ = io.send("G90");
        return Err(e);
    }
    HeightMap::from_readings(area, nx, ny, &readings)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A surface whose height under the probe is given by a function of X and Y; it records what was sent.
    struct Table {
        surface: fn(f64, f64) -> f64,
        at: (f64, f64),
        z: f64,
        sent: Vec<String>,
        fail_at: Option<usize>,
        stop_after: Option<usize>,
        touched: usize,
    }

    impl Table {
        fn new(surface: fn(f64, f64) -> f64) -> Table {
            Table {
                surface,
                at: (0.0, 0.0),
                z: 5.0,
                sent: Vec::new(),
                fail_at: None,
                stop_after: None,
                touched: 0,
            }
        }
    }

    impl ProbeIo for Table {
        fn send(&mut self, line: &str) -> Result<(), String> {
            self.sent.push(line.to_string());
            if line.starts_with("G38.2") {
                self.touched += 1;
                if Some(self.touched) == self.fail_at {
                    return Err("ALARM:4".to_string());
                }
                self.z = (self.surface)(self.at.0, self.at.1);
            } else if let Some(rest) = line.strip_prefix("G0 X") {
                let (x, y) = rest.split_once(" Y").unwrap();
                self.at = (x.parse().unwrap(), y.parse().unwrap());
            } else if let Some(z) = line.strip_prefix("G0 Z") {
                self.z = z.parse().unwrap();
            }
            Ok(())
        }
        fn machine_z(&mut self) -> Result<f64, String> {
            Ok(self.z)
        }
        fn halted(&self) -> bool {
            self.stop_after.is_some_and(|n| self.touched >= n)
        }
    }

    const AREA: [f64; 4] = [0.0, 0.0, 10.0, 10.0];

    #[test]
    fn the_z_cycle_is_down_zero_and_up() {
        let l = z_touch_off_lines(&ProbeCfg {
            plate_mm: 1.5,
            ..ProbeCfg::default()
        })
        .unwrap();
        assert_eq!(
            l,
            vec![
                "G91",
                "G38.2 Z-10 F20",
                "G90",
                "G10 L20 P0 Z1.5",
                "G91",
                "G0 Z2",
                "G90"
            ]
        );
    }

    #[test]
    fn values_that_make_no_sense_are_refused() {
        let d = ProbeCfg::default();
        for bad in [
            ProbeCfg {
                feed_mm_min: 0.0,
                ..d
            },
            ProbeCfg {
                feed_mm_min: f64::NAN,
                ..d
            },
            ProbeCfg { depth_mm: 0.0, ..d },
            ProbeCfg {
                retract_mm: -1.0,
                ..d
            },
            ProbeCfg {
                plate_mm: 500.0,
                ..d
            },
            ProbeCfg {
                clearance_mm: 0.1,
                ..d
            },
        ] {
            assert!(bad.check().is_err(), "{bad:?}");
            assert!(z_touch_off_lines(&bad).is_err());
        }
        assert!(d.check().is_ok());
    }

    #[test]
    fn a_grid_measures_a_tilted_board() {
        let mut t = Table::new(|x, _| -2.0 + x * 0.05);
        let m = run_grid(&mut t, AREA, 3, 2, &ProbeCfg::default()).unwrap();
        // 0.05 mm per mm: 0, 0.25, 0.5 across the 10 mm
        assert_eq!(m.nx * m.ny, 6);
        assert!((m.offset_at(10.0, 5.0) - 0.5).abs() < 1e-9, "{:?}", m.z);
        // the head rose to the clearance between points and ended there with the distances absolute
        assert_eq!(t.sent.first().map(String::as_str), Some("G90"));
        assert_eq!(t.sent.last().map(String::as_str), Some("G0 Z5"));
        assert_eq!(t.sent.iter().filter(|l| *l == "G38.2 Z-10 F20").count(), 6);
    }

    #[test]
    fn a_miss_ends_the_grid_and_puts_the_distances_back() {
        let mut t = Table::new(|_, _| 0.0);
        t.fail_at = Some(2);
        let e = run_grid(&mut t, AREA, 2, 2, &ProbeCfg::default()).unwrap_err();
        assert!(e.contains("point 2") && e.contains("ALARM:4"), "{e}");
        assert_eq!(t.touched, 2);
        assert_eq!(t.sent.last().map(String::as_str), Some("G90"));
    }

    #[test]
    fn a_stop_is_heard_before_the_next_point() {
        let mut t = Table::new(|_, _| 0.0);
        t.stop_after = Some(1);
        let e = run_grid(&mut t, AREA, 2, 2, &ProbeCfg::default()).unwrap_err();
        assert!(e.contains("before point 2"), "{e}");
        assert_eq!(t.touched, 1);
    }

    #[test]
    fn the_script_marks_each_point_and_reads_after_each_touch() {
        let s = grid_script(AREA, 2, 2, &ProbeCfg::default()).unwrap();
        assert_eq!(s.iter().filter(|a| matches!(a, Act::Mark(_))).count(), 4);
        assert_eq!(s.iter().filter(|a| **a == Act::Sample).count(), 4);
        assert_eq!(s.first(), Some(&Act::Line("G90".to_string())));
        assert_eq!(s.last(), Some(&Act::Line("G0 Z5".to_string())));
        // every touch is inside a relative move
        for (k, a) in s.iter().enumerate() {
            if matches!(a, Act::Line(l) if l.starts_with("G38.2")) {
                assert_eq!(s[k - 1], Act::Line("G91".to_string()));
                assert_eq!(s[k + 1], Act::Line("G90".to_string()));
            }
        }
        assert!(grid_script(AREA, 1, 2, &ProbeCfg::default()).is_err());
    }

    #[test]
    fn a_grid_with_a_bad_area_sends_nothing() {
        let mut t = Table::new(|_, _| 0.0);
        assert!(run_grid(&mut t, [0.0, 0.0, 0.0, 0.0], 3, 3, &ProbeCfg::default()).is_err());
        assert!(t.sent.is_empty());
    }
}
