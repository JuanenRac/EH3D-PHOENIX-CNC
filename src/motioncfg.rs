//! The values of the motion layer that the operator tunes: how the planner accelerates and takes the corners, whether arcs are rebuilt, how
//! the guard of the controller behaves, what the importers read and cut, and where the tool setter is. None of them is in the code: they
//! are `key = value` lines of a text file, `phoenix-cnc-motion.txt` next to the settings file (`examples/motion.txt` is a model, and it
//! is the defaults). With no file every value is its default; a line that cannot be used is named, with its number and the reason, and
//! the default stays. Every option that makes the machine safer has a switch and starts on.

use std::collections::BTreeMap;
use std::path::Path;

/// The biggest file read and the most lines looked at.
pub const MAX_FILE_BYTES: u64 = 16 * 1024;
pub const MAX_LINES: usize = 256;

/// Where the planner and the estimate get their numbers.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannerCfg {
    /// The acceleration of X, Y and Z in mm/s2.
    pub accel_mm_s2: [f64; 3],
    /// The most speed of X, Y and Z in mm/min.
    pub max_rate_mm_min: [f64; 3],
    /// How far from the corner the path may cut it, in mm (the larger, the faster the corners are taken).
    pub junction_deviation_mm: f64,
    /// The speed a move may start and end at when it has to stop, in mm/min.
    pub min_speed_mm_min: f64,
    /// Arcs (`G2`/`G3`) are rebuilt as short straight moves for the plan (off: they are timed along their curve as they are).
    pub arcs: bool,
    /// How far the straight moves of a rebuilt arc may leave the curve, in mm.
    pub arc_tolerance_mm: f64,
    /// The most moves the planner looks ahead.
    pub lookahead: usize,
}

impl Default for PlannerCfg {
    fn default() -> Self {
        PlannerCfg {
            accel_mm_s2: [50.0, 50.0, 25.0],
            max_rate_mm_min: [1500.0, 1500.0, 600.0],
            junction_deviation_mm: 0.01,
            min_speed_mm_min: 60.0,
            arcs: false,
            arc_tolerance_mm: 0.01,
            lookahead: 32,
        }
    }
}

/// How the guard of the controller behaves.
#[derive(Debug, Clone, PartialEq)]
pub struct GuardCfg {
    /// More tries of a read (the status) that got no answer; an order that moves is never tried again whatever this says.
    pub status_retries: u32,
    /// A controller that falls silent while it may be moving is stopped and the machine is refused until the operator resets.
    pub stop_on_silence: bool,
    /// A position that is not a number, or that jumps further than `max_jump_mm` between two reads, is not believed: stop.
    pub check_position: bool,
    pub max_jump_mm: f64,
    /// The jog: one move order at a time (a second one is a stop), and a target not renewed in this many ms leaves the machine still.
    pub jog_one_order: bool,
    pub jog_stale_ms: u64,
}

impl Default for GuardCfg {
    fn default() -> Self {
        GuardCfg {
            status_retries: 2,
            stop_on_silence: true,
            check_position: true,
            max_jump_mm: 250.0,
            jog_one_order: true,
            jog_stale_ms: 300,
        }
    }
}

/// What the importers read and what they cut.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportCfg {
    pub max_bytes: u64,
    pub max_points: usize,
    /// The depth of the cut and the depth of one pass, in mm (positive numbers: down).
    pub depth_mm: f64,
    pub pass_mm: f64,
    pub feed_mm_min: f64,
    pub plunge_mm_min: f64,
    pub safe_z_mm: f64,
    /// How far the straight pieces of a curve may leave it, in mm.
    pub curve_tolerance_mm: f64,
    /// The post-processors may run a script (off: a post-processor that names one is not run; the declarative rules always work).
    pub post_scripts: bool,
}

impl Default for ImportCfg {
    fn default() -> Self {
        ImportCfg {
            max_bytes: 8 * 1024 * 1024,
            max_points: 500_000,
            depth_mm: 1.0,
            pass_mm: 0.5,
            feed_mm_min: 300.0,
            plunge_mm_min: 100.0,
            safe_z_mm: 5.0,
            curve_tolerance_mm: 0.05,
            post_scripts: false,
        }
    }
}

/// Where the tool setter is and how it is touched.
#[derive(Debug, Clone, PartialEq)]
pub struct SetterCfg {
    /// The machine position of the setter (X, Y) and the height it is touched from (Z), in mm.
    pub x_mm: f64,
    pub y_mm: f64,
    pub start_z_mm: f64,
    /// The most the tool goes down looking for it, the feed of the touch and the lift after it, in mm and mm/min.
    pub max_travel_mm: f64,
    pub feed_mm_min: f64,
    pub retract_mm: f64,
    /// The height of the setter above the zero of the machine in Z, in mm.
    pub height_mm: f64,
    /// The measure waits for the operator to confirm that the setter is there and clear.
    pub confirm: bool,
}

impl Default for SetterCfg {
    fn default() -> Self {
        SetterCfg {
            x_mm: 0.0,
            y_mm: 0.0,
            start_z_mm: 0.0,
            max_travel_mm: 60.0,
            feed_mm_min: 60.0,
            retract_mm: 5.0,
            height_mm: 0.0,
            confirm: true,
        }
    }
}

/// Where the motion file is: next to the settings, with the name of the settings file in front (`phoenix-cnc-motion.txt`).
pub fn motion_path(settings: &Path) -> std::path::PathBuf {
    let stem = settings.file_stem().map_or_else(
        || "phoenix-cnc".to_string(),
        |s| s.to_string_lossy().to_string(),
    );
    settings.with_file_name(format!("{stem}-motion.txt"))
}

/// Everything of the file.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MotionCfg {
    pub planner: PlannerCfg,
    pub guard: GuardCfg,
    pub import: ImportCfg,
    pub setter: SetterCfg,
}

pub use crate::problem::{Problem, Why};

fn num(v: &str) -> Option<f64> {
    v.trim().parse::<f64>().ok().filter(|n| n.is_finite())
}

fn switch(v: &str) -> Option<bool> {
    match v.trim().to_ascii_lowercase().as_str() {
        "on" | "yes" | "true" | "1" => Some(true),
        "off" | "no" | "false" | "0" => Some(false),
        _ => None,
    }
}

impl MotionCfg {
    /// Reads the text of the file; the lines that cannot be used are returned with the reason and leave their value as it was.
    pub fn parse(text: &str) -> (MotionCfg, Vec<Problem>) {
        let mut c = MotionCfg::default();
        let mut problems = Vec::new();
        let mut seen: BTreeMap<String, usize> = BTreeMap::new();
        for (i, raw) in text.lines().take(MAX_LINES).enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            let n = i + 1;
            let Some((k, v)) = line.split_once('=') else {
                problems.push(Problem::new(n, Why::NotKeyValue, ""));
                continue;
            };
            let (k, v) = (k.trim().to_ascii_lowercase(), v.trim());
            if let Some(first) = seen.insert(k.clone(), n) {
                problems.push(Problem::new(n, Why::Repeated, first.to_string()));
                continue;
            }
            if let Err((why, detail)) = c.set(&k, v) {
                problems.push(Problem::new(n, why, detail));
            }
        }
        (c, problems)
    }

    /// Reads the file at a path (nothing is wrong with one that is not there: the defaults stay), refusing one bigger than the limit.
    pub fn read(path: &Path) -> (MotionCfg, Vec<Problem>) {
        match std::fs::metadata(path) {
            Err(_) => (MotionCfg::default(), Vec::new()),
            Ok(m) if m.len() > MAX_FILE_BYTES => {
                (MotionCfg::default(), vec![Problem::new(0, Why::TooBig, "")])
            }
            Ok(_) => match std::fs::read(path) {
                Ok(b) => MotionCfg::parse(&String::from_utf8_lossy(&b)),
                Err(_) => (
                    MotionCfg::default(),
                    vec![Problem::new(0, Why::Unreadable, "")],
                ),
            },
        }
    }

    fn set(&mut self, key: &str, v: &str) -> Result<(), (Why, String)> {
        fn ranged(v: &str, lo: f64, hi: f64) -> Result<f64, (Why, String)> {
            match num(v) {
                Some(n) if (lo..=hi).contains(&n) => Ok(n),
                Some(_) => Err((Why::OutOfRange, format!("{lo} - {hi}"))),
                None => Err((Why::NotANumber, String::new())),
            }
        }
        fn flag(v: &str) -> Result<bool, (Why, String)> {
            switch(v).ok_or((Why::OnOff, String::new()))
        }
        let (p, g, i, s) = (
            &mut self.planner,
            &mut self.guard,
            &mut self.import,
            &mut self.setter,
        );
        match key {
            "accel_x" => p.accel_mm_s2[0] = ranged(v, 1.0, 100_000.0)?,
            "accel_y" => p.accel_mm_s2[1] = ranged(v, 1.0, 100_000.0)?,
            "accel_z" => p.accel_mm_s2[2] = ranged(v, 1.0, 100_000.0)?,
            "max_rate_x" => p.max_rate_mm_min[0] = ranged(v, 1.0, 1_000_000.0)?,
            "max_rate_y" => p.max_rate_mm_min[1] = ranged(v, 1.0, 1_000_000.0)?,
            "max_rate_z" => p.max_rate_mm_min[2] = ranged(v, 1.0, 1_000_000.0)?,
            "junction_deviation" => p.junction_deviation_mm = ranged(v, 0.0001, 10.0)?,
            "min_speed" => p.min_speed_mm_min = ranged(v, 1.0, 10_000.0)?,
            "arcs" => p.arcs = flag(v)?,
            "arc_tolerance" => p.arc_tolerance_mm = ranged(v, 0.0001, 5.0)?,
            "lookahead" => p.lookahead = ranged(v, 1.0, 4096.0)? as usize,
            "guard_status_retries" => g.status_retries = ranged(v, 0.0, 10.0)? as u32,
            "guard_stop_on_silence" => g.stop_on_silence = flag(v)?,
            "guard_check_position" => g.check_position = flag(v)?,
            "guard_max_jump" => g.max_jump_mm = ranged(v, 0.1, 100_000.0)?,
            "jog_one_order" => g.jog_one_order = flag(v)?,
            "jog_stale_ms" => g.jog_stale_ms = ranged(v, 20.0, 60_000.0)? as u64,
            "import_max_bytes" => i.max_bytes = ranged(v, 1024.0, 268_435_456.0)? as u64,
            "import_max_points" => i.max_points = ranged(v, 16.0, 10_000_000.0)? as usize,
            "import_depth" => i.depth_mm = ranged(v, 0.001, 1000.0)?,
            "import_pass" => i.pass_mm = ranged(v, 0.001, 1000.0)?,
            "import_feed" => i.feed_mm_min = ranged(v, 1.0, 100_000.0)?,
            "import_plunge" => i.plunge_mm_min = ranged(v, 1.0, 100_000.0)?,
            "import_safe_z" => i.safe_z_mm = ranged(v, 0.1, 1000.0)?,
            "post_scripts" => i.post_scripts = flag(v)?,
            "import_curve_tolerance" => i.curve_tolerance_mm = ranged(v, 0.0005, 5.0)?,
            "setter_x" => s.x_mm = ranged(v, -100_000.0, 100_000.0)?,
            "setter_y" => s.y_mm = ranged(v, -100_000.0, 100_000.0)?,
            "setter_start_z" => s.start_z_mm = ranged(v, -100_000.0, 100_000.0)?,
            "setter_max_travel" => s.max_travel_mm = ranged(v, 0.1, 1000.0)?,
            "setter_feed" => s.feed_mm_min = ranged(v, 1.0, 5000.0)?,
            "setter_retract" => s.retract_mm = ranged(v, 0.1, 1000.0)?,
            "setter_height" => s.height_mm = ranged(v, -1000.0, 1000.0)?,
            "setter_confirm" => s.confirm = flag(v)?,
            _ => return Err((Why::UnknownKey, String::new())),
        }
        Ok(())
    }

    /// The text of a file with every value (the defaults when nothing was read): the model the example file is made from.
    pub fn to_text(&self) -> String {
        let (p, g, i, s) = (&self.planner, &self.guard, &self.import, &self.setter);
        let f = |b: bool| if b { "on" } else { "off" };
        let mut t = String::from(
            "# The motion layer of phoenix-cnc: the planner, the guard of the controller, the importers and the tool setter.\n\
             # One key = value per line; what is missing or wrong keeps its default.\n",
        );
        for (k, v) in [
            ("accel_x", p.accel_mm_s2[0]),
            ("accel_y", p.accel_mm_s2[1]),
            ("accel_z", p.accel_mm_s2[2]),
            ("max_rate_x", p.max_rate_mm_min[0]),
            ("max_rate_y", p.max_rate_mm_min[1]),
            ("max_rate_z", p.max_rate_mm_min[2]),
            ("junction_deviation", p.junction_deviation_mm),
            ("min_speed", p.min_speed_mm_min),
        ] {
            t.push_str(&format!("{k} = {v}\n"));
        }
        t.push_str(&format!("arcs = {}\n", f(p.arcs)));
        t.push_str(&format!("arc_tolerance = {}\n", p.arc_tolerance_mm));
        t.push_str(&format!("lookahead = {}\n", p.lookahead));
        t.push_str(&format!("guard_status_retries = {}\n", g.status_retries));
        t.push_str(&format!(
            "guard_stop_on_silence = {}\n",
            f(g.stop_on_silence)
        ));
        t.push_str(&format!("guard_check_position = {}\n", f(g.check_position)));
        t.push_str(&format!("guard_max_jump = {}\n", g.max_jump_mm));
        t.push_str(&format!("jog_one_order = {}\n", f(g.jog_one_order)));
        t.push_str(&format!("jog_stale_ms = {}\n", g.jog_stale_ms));
        t.push_str(&format!("import_max_bytes = {}\n", i.max_bytes));
        t.push_str(&format!("import_max_points = {}\n", i.max_points));
        t.push_str(&format!("post_scripts = {}\n", f(i.post_scripts)));
        for (k, v) in [
            ("import_depth", i.depth_mm),
            ("import_pass", i.pass_mm),
            ("import_feed", i.feed_mm_min),
            ("import_plunge", i.plunge_mm_min),
            ("import_safe_z", i.safe_z_mm),
            ("import_curve_tolerance", i.curve_tolerance_mm),
            ("setter_x", s.x_mm),
            ("setter_y", s.y_mm),
            ("setter_start_z", s.start_z_mm),
            ("setter_max_travel", s.max_travel_mm),
            ("setter_feed", s.feed_mm_min),
            ("setter_retract", s.retract_mm),
            ("setter_height", s.height_mm),
        ] {
            t.push_str(&format!("{k} = {v}\n"));
        }
        t.push_str(&format!("setter_confirm = {}\n", f(s.confirm)));
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_no_text_every_value_is_its_default_and_the_model_file_is_the_defaults() {
        let (c, p) = MotionCfg::parse("");
        assert!(p.is_empty());
        assert_eq!(c, MotionCfg::default());
        // the safe options start on and the unsafe one (rebuilding arcs) starts off
        assert!(c.guard.stop_on_silence && c.guard.check_position && c.guard.jog_one_order);
        assert!(!c.planner.arcs && c.setter.confirm);
        let (back, problems) = MotionCfg::parse(&c.to_text());
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(back, c);
        // the example file of the repository is the model
        let example = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("examples")
                .join("motion.txt"),
        )
        .unwrap()
        .replace("\r\n", "\n");
        assert_eq!(example, c.to_text());
    }

    #[test]
    fn values_are_read_and_the_wrong_ones_are_named_and_keep_their_default() {
        let (c, p) = MotionCfg::parse(
            "# a comment\naccel_x = 120\narcs = on\njunction_deviation = 0.05\nmax_rate_z = -3\nguard_stop_on_silence = maybe\nnothing = 1\nno equals\naccel_x = 5\njog_stale_ms = 500\n",
        );
        assert_eq!(c.planner.accel_mm_s2[0], 120.0);
        assert!(c.planner.arcs);
        assert_eq!(c.planner.junction_deviation_mm, 0.05);
        assert_eq!(c.planner.max_rate_mm_min[2], 600.0);
        assert!(c.guard.stop_on_silence);
        assert_eq!(c.guard.jog_stale_ms, 500);
        let lines: Vec<usize> = p.iter().map(|p| p.line).collect();
        assert_eq!(lines, vec![5, 6, 7, 8, 9]);
        assert_eq!(p[4].why, Why::Repeated);
    }

    #[test]
    fn a_missing_file_is_the_defaults_and_a_big_one_is_refused_without_a_panic() {
        let dir = std::env::temp_dir().join(format!("cnc_motioncfg_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (c, p) = MotionCfg::read(&dir.join("none.txt"));
        assert!(p.is_empty() && c == MotionCfg::default());
        let big = dir.join("big.txt");
        std::fs::write(&big, "#".repeat(MAX_FILE_BYTES as usize + 1)).unwrap();
        let (c, p) = MotionCfg::read(&big);
        assert!(c == MotionCfg::default() && p.len() == 1 && p[0].line == 0);
        let odd = dir.join("odd.txt");
        std::fs::write(&odd, [0xff, 0xfe, b'=', 0x00, b'\n']).unwrap();
        let (_c, p) = MotionCfg::read(&odd);
        assert_eq!(p.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
