//! The values of the motion layer that the operator tunes: how the planner accelerates and takes the corners, whether arcs are rebuilt, how
//! the guard of the controller behaves, what the importers read and cut (a drawing, and a board from its Gerber and Excellon files), how
//! the simulated cable of a controller misbehaves, and where the tool setter is. None of them is in the code: they
//! are `key = value` lines of a text file, `phoenix-cnc-motion.txt` next to the settings file (`examples/motion.txt` is a model, and it
//! is the defaults). With no file every value is its default; a line that cannot be used is named, with its number and the reason, and
//! the default stays. Every option that makes the machine safer has a switch and starts on.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use eh3d_controller::httprpc::{self, HttpConfig};

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
    /// The controller of the window is wrapped in the guard (off: the window talks to the controller directly, as it did before).
    pub enabled: bool,
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
    /// A jog in another axis or the other way while the last one still moves cancels the move in progress first (off: the order goes as it is,
    /// and with `jog_one_order` on the guard takes it for a second move order and stops the machine).
    pub jog_cancel_crossing: bool,
}

impl Default for GuardCfg {
    fn default() -> Self {
        GuardCfg {
            enabled: true,
            status_retries: 2,
            stop_on_silence: true,
            check_position: true,
            max_jump_mm: 250.0,
            jog_one_order: true,
            jog_stale_ms: 300,
            jog_cancel_crossing: true,
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

/// How a board is cut from its Gerber and Excellon files.
#[derive(Debug, Clone, PartialEq)]
pub struct PcbCfg {
    /// The isolation: the width of the tool at the depth of the cut, how many passes, how far apart they are and the cell of the grid
    /// the copper is drawn on (mm); the depth and the feeds.
    pub tool_mm: f64,
    pub passes: usize,
    pub stepover_mm: f64,
    pub res_mm: f64,
    pub depth_mm: f64,
    pub feed_mm_min: f64,
    pub plunge_mm_min: f64,
    /// The drilling: the depth, the depth of one peck, the feed and the height the drill is lifted to between pecks (mm).
    pub drill_depth_mm: f64,
    pub peck_mm: f64,
    pub drill_feed_mm_min: f64,
    pub retract_mm: f64,
    /// The profile: the tool, the depth, the depth of one pass and the feed.
    pub profile_tool_mm: f64,
    pub profile_depth_mm: f64,
    pub profile_pass_mm: f64,
    pub profile_feed_mm_min: f64,
    /// Two ends of the outline closer than this are one point (mm).
    pub join_mm: f64,
    /// The speed of the spindle the program starts with (0: the program does not start it).
    pub rpm: f64,
    /// The layers of the bottom are mirrored about the Y axis.
    pub mirror_bottom: bool,
    /// The lower left corner of each file is the zero of the work (off: the zero of the file is kept, so that the layers stay aligned).
    pub zero_origin: bool,
    /// A change of drill is `T1 M6` (off: a pause with the diameter in its comment, `M0`).
    pub tool_change_m6: bool,
    /// The most cells of the grid of the isolation.
    pub max_cells: u64,
}

impl Default for PcbCfg {
    fn default() -> Self {
        PcbCfg {
            tool_mm: 0.2,
            passes: 2,
            stepover_mm: 0.1,
            res_mm: 0.05,
            depth_mm: 0.05,
            feed_mm_min: 200.0,
            plunge_mm_min: 60.0,
            drill_depth_mm: 1.8,
            peck_mm: 0.6,
            drill_feed_mm_min: 60.0,
            retract_mm: 1.0,
            profile_tool_mm: 1.0,
            profile_depth_mm: 1.7,
            profile_pass_mm: 0.6,
            profile_feed_mm_min: 150.0,
            join_mm: 0.01,
            rpm: 0.0,
            mirror_bottom: true,
            zero_origin: false,
            tool_change_m6: false,
            max_cells: 16_000_000,
        }
    }
}

/// How the cable to a simulated controller misbehaves (`--sim`), and how Marlin is driven.
#[derive(Debug, Clone, PartialEq)]
pub struct WireCfg {
    /// The seed of the schedule: the same seed is the same cable.
    pub seed: u64,
    /// The most bytes of an answer that arrive in one piece (0: whole).
    pub chunk_max: usize,
    /// Per thousand pieces, the chance of a line of noise in front of one; per thousand lines, the chance that a bit of one goes wrong.
    pub noise_per_mille: u32,
    pub flip_per_mille: u32,
    /// Marlin is sent numbered lines with a checksum, so that a line spoiled on the way is asked for again (off: plain lines).
    pub marlin_numbering: bool,
}

impl Default for WireCfg {
    fn default() -> Self {
        WireCfg {
            seed: 1,
            chunk_max: 0,
            noise_per_mille: 0,
            flip_per_mille: 0,
            marlin_numbering: false,
        }
    }
}

/// How the program reaches Moonraker, the HTTP side of Klipper (`--klipper HOST[:PORT]`): the `klipper.*` keys of the file. Only the keys that
/// were written are kept apart; the others are the defaults of `eh3d_controller::httprpc`. The credential, when there is one, is never
/// shown: not by `Debug`, not by a fault of a line, not by the text of the file the program writes.
#[derive(Clone, PartialEq)]
pub struct KlipperCfg {
    /// An IP address or `localhost`; a name only with `allow_non_local` on.
    pub host: String,
    /// Accepts an address outside the local network (and a name). Off by default.
    pub allow_non_local: bool,
    api_key: Option<String>,
    numbers: BTreeMap<String, u64>,
}

impl Default for KlipperCfg {
    fn default() -> Self {
        let d = HttpConfig::default();
        KlipperCfg {
            host: d.host,
            allow_non_local: d.allow_non_local,
            api_key: None,
            numbers: BTreeMap::new(),
        }
    }
}

impl fmt::Debug for KlipperCfg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KlipperCfg")
            .field("host", &self.host)
            .field("allow_non_local", &self.allow_non_local)
            .field("api_key", &self.api_key.as_ref().map(|_| "<set>"))
            .field("numbers", &self.numbers)
            .finish()
    }
}

impl KlipperCfg {
    /// Whether the file gives a credential.
    pub fn has_api_key(&self) -> bool {
        self.api_key.is_some()
    }

    /// The value of a numeric key: the one the file gave, or the default of the shared crate.
    pub fn number(&self, key: &str) -> u64 {
        self.numbers
            .get(key)
            .copied()
            .or_else(|| HttpConfig::default().number(key))
            .unwrap_or(0)
    }

    fn set(&mut self, key: &str, v: &str) -> Result<(), (Why, String)> {
        match key {
            "klipper.host" => {
                if v.is_empty() || v.len() > 253 {
                    return Err((Why::HostRefused, String::new()));
                }
                self.host = v.to_string();
            }
            "klipper.allow_non_local" => {
                self.allow_non_local = switch(v).ok_or((Why::OnOff, String::new()))?;
            }
            "klipper.api_key" => {
                // the value is tried and dropped: nothing of it goes in the fault
                HttpConfig::default()
                    .with_api_key(v)
                    .map_err(|_| (Why::SecretRefused, String::new()))?;
                self.api_key = Some(v.to_string());
            }
            _ => {
                let Some(s) = httprpc::SETTINGS.iter().find(|s| s.key == key) else {
                    return Err((Why::UnknownKey, String::new()));
                };
                let Ok(n) = v.parse::<u64>() else {
                    return Err((Why::NotANumber, String::new()));
                };
                if n < s.min || n > s.max {
                    return Err((Why::OutOfRange, format!("{} - {}", s.min, s.max)));
                }
                // only what differs from the default is kept, so the text of the defaults reads back as the defaults
                if n == HttpConfig::default().number(key).unwrap_or(0) {
                    self.numbers.remove(key);
                } else {
                    self.numbers.insert(key.to_string(), n);
                }
            }
        }
        Ok(())
    }

    /// The settings of the transport and of the controller. `over` is what `--klipper HOST[:PORT]` says: it takes the place of the host (and of
    /// the port when it names one). What cannot hold together is refused here: an address the program may not reach, a script that would be
    /// cut off before the controller gives up on it.
    pub fn http(&self, over: Option<(&str, Option<u16>)>) -> Result<HttpConfig, Problem> {
        let mut c = HttpConfig::new(&self.host, HttpConfig::default().port);
        c.allow_non_local = self.allow_non_local;
        for (k, n) in &self.numbers {
            // each number was checked against its range when it was read
            c.set_number(k, *n)
                .map_err(|_| Problem::new(0, Why::OutOfRange, ""))?;
        }
        if let Some((host, port)) = over {
            c.host = host.to_string();
            if let Some(p) = port {
                c.port = p;
            }
        }
        if let Some(key) = &self.api_key {
            c = c
                .with_api_key(key)
                .map_err(|_| Problem::new(0, Why::SecretRefused, ""))?;
        }
        if c.response_wait < c.busy_limit {
            return Err(Problem::new(0, Why::WaitShorter, ""));
        }
        c.check()
            .map_err(|_| Problem::new(0, Why::HostRefused, ""))?;
        Ok(c)
    }

    /// The lines of the text of the file. The credential is never one of them.
    fn lines(&self) -> String {
        let mut t = format!("klipper.host = {}\n", self.host);
        t.push_str(&format!(
            "klipper.allow_non_local = {}\n",
            if self.allow_non_local { "on" } else { "off" }
        ));
        for s in httprpc::SETTINGS {
            t.push_str(&format!("{} = {}\n", s.key, self.number(s.key)));
        }
        t.push_str(
            "# klipper.api_key = the credential of Moonraker, when it asks for one (written by hand; this program never prints it)\n",
        );
        t
    }
}

/// Where the tool setter is and how it is touched.
#[derive(Debug, Clone, Copy, PartialEq)]
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
    pub pcb: PcbCfg,
    pub wire: WireCfg,
    pub setter: SetterCfg,
    pub klipper: KlipperCfg,
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
        // what no single key can say: the address, and that a script is not cut off before the controller gives up on it
        if let Err(p) = c.klipper.http(None) {
            let line = |key: &str| seen.get(key).copied().unwrap_or(0);
            match p.why {
                Why::WaitShorter => {
                    let at = line("klipper.response_wait_ms").max(line("klipper.busy_limit_s"));
                    problems.push(Problem::new(at, p.why, ""));
                    c.klipper.numbers.remove("klipper.response_wait_ms");
                    c.klipper.numbers.remove("klipper.busy_limit_s");
                }
                _ => {
                    problems.push(Problem::new(
                        line("klipper.host").max(line("klipper.allow_non_local")),
                        Why::HostRefused,
                        "",
                    ));
                    c.klipper.host = KlipperCfg::default().host;
                }
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
        let (p, g, i, s, b, w) = (
            &mut self.planner,
            &mut self.guard,
            &mut self.import,
            &mut self.setter,
            &mut self.pcb,
            &mut self.wire,
        );
        if key.starts_with("klipper.") {
            return self.klipper.set(key, v);
        }
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
            "guard" => g.enabled = flag(v)?,
            "guard_status_retries" => g.status_retries = ranged(v, 0.0, 10.0)? as u32,
            "guard_stop_on_silence" => g.stop_on_silence = flag(v)?,
            "guard_check_position" => g.check_position = flag(v)?,
            "guard_max_jump" => g.max_jump_mm = ranged(v, 0.1, 100_000.0)?,
            "jog_one_order" => g.jog_one_order = flag(v)?,
            "jog_stale_ms" => g.jog_stale_ms = ranged(v, 20.0, 60_000.0)? as u64,
            "jog_cancel_crossing" => g.jog_cancel_crossing = flag(v)?,
            "import_max_bytes" => i.max_bytes = ranged(v, 1024.0, 268_435_456.0)? as u64,
            "import_max_points" => i.max_points = ranged(v, 16.0, 10_000_000.0)? as usize,
            "import_depth" => i.depth_mm = ranged(v, 0.001, 1000.0)?,
            "import_pass" => i.pass_mm = ranged(v, 0.001, 1000.0)?,
            "import_feed" => i.feed_mm_min = ranged(v, 1.0, 100_000.0)?,
            "import_plunge" => i.plunge_mm_min = ranged(v, 1.0, 100_000.0)?,
            "import_safe_z" => i.safe_z_mm = ranged(v, 0.1, 1000.0)?,
            "post_scripts" => i.post_scripts = flag(v)?,
            "import_curve_tolerance" => i.curve_tolerance_mm = ranged(v, 0.0005, 5.0)?,
            "pcb_tool" => b.tool_mm = ranged(v, 0.02, 10.0)?,
            "pcb_passes" => b.passes = ranged(v, 1.0, 10.0)? as usize,
            "pcb_stepover" => b.stepover_mm = ranged(v, 0.01, 10.0)?,
            "pcb_res" => b.res_mm = ranged(v, 0.002, 1.0)?,
            "pcb_depth" => b.depth_mm = ranged(v, 0.001, 10.0)?,
            "pcb_feed" => b.feed_mm_min = ranged(v, 1.0, 100_000.0)?,
            "pcb_plunge" => b.plunge_mm_min = ranged(v, 1.0, 100_000.0)?,
            "pcb_drill_depth" => b.drill_depth_mm = ranged(v, 0.01, 100.0)?,
            "pcb_peck" => b.peck_mm = ranged(v, 0.01, 100.0)?,
            "pcb_drill_feed" => b.drill_feed_mm_min = ranged(v, 1.0, 100_000.0)?,
            "pcb_retract" => b.retract_mm = ranged(v, 0.1, 100.0)?,
            "pcb_profile_tool" => b.profile_tool_mm = ranged(v, 0.1, 20.0)?,
            "pcb_profile_depth" => b.profile_depth_mm = ranged(v, 0.01, 100.0)?,
            "pcb_profile_pass" => b.profile_pass_mm = ranged(v, 0.01, 100.0)?,
            "pcb_profile_feed" => b.profile_feed_mm_min = ranged(v, 1.0, 100_000.0)?,
            "pcb_join" => b.join_mm = ranged(v, 0.0001, 1.0)?,
            "pcb_rpm" => b.rpm = ranged(v, 0.0, 1_000_000.0)?,
            "pcb_mirror_bottom" => b.mirror_bottom = flag(v)?,
            "pcb_zero_origin" => b.zero_origin = flag(v)?,
            "pcb_tool_change_m6" => b.tool_change_m6 = flag(v)?,
            "pcb_max_cells" => b.max_cells = ranged(v, 1000.0, 1_000_000_000.0)? as u64,
            "wire_seed" => w.seed = ranged(v, 0.0, 9_007_199_254_740_991.0)? as u64,
            "wire_chunk_max" => w.chunk_max = ranged(v, 0.0, 512.0)? as usize,
            "wire_noise" => w.noise_per_mille = ranged(v, 0.0, 1000.0)? as u32,
            "wire_flip" => w.flip_per_mille = ranged(v, 0.0, 1000.0)? as u32,
            "marlin_numbering" => w.marlin_numbering = flag(v)?,
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
        let (b, w) = (&self.pcb, &self.wire);
        let f = |b: bool| if b { "on" } else { "off" };
        let mut t = String::from(
            "# The motion layer of phoenix-cnc: the planner, the guard of the controller, the importers (drawings and boards), the cable of the simulators and the tool setter.\n\
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
        t.push_str(&format!("guard = {}\n", f(g.enabled)));
        t.push_str(&format!("guard_status_retries = {}\n", g.status_retries));
        t.push_str(&format!(
            "guard_stop_on_silence = {}\n",
            f(g.stop_on_silence)
        ));
        t.push_str(&format!("guard_check_position = {}\n", f(g.check_position)));
        t.push_str(&format!("guard_max_jump = {}\n", g.max_jump_mm));
        t.push_str(&format!("jog_one_order = {}\n", f(g.jog_one_order)));
        t.push_str(&format!("jog_stale_ms = {}\n", g.jog_stale_ms));
        t.push_str(&format!(
            "jog_cancel_crossing = {}\n",
            f(g.jog_cancel_crossing)
        ));
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
        // the boards: the isolation, the drilling and the profile, then what is switched
        for (k, v) in [
            ("pcb_tool", b.tool_mm),
            ("pcb_passes", b.passes as f64),
            ("pcb_stepover", b.stepover_mm),
            ("pcb_res", b.res_mm),
            ("pcb_depth", b.depth_mm),
            ("pcb_feed", b.feed_mm_min),
            ("pcb_plunge", b.plunge_mm_min),
            ("pcb_drill_depth", b.drill_depth_mm),
            ("pcb_peck", b.peck_mm),
            ("pcb_drill_feed", b.drill_feed_mm_min),
            ("pcb_retract", b.retract_mm),
            ("pcb_profile_tool", b.profile_tool_mm),
            ("pcb_profile_depth", b.profile_depth_mm),
            ("pcb_profile_pass", b.profile_pass_mm),
            ("pcb_profile_feed", b.profile_feed_mm_min),
            ("pcb_join", b.join_mm),
            ("pcb_rpm", b.rpm),
        ] {
            t.push_str(&format!("{k} = {v}\n"));
        }
        t.push_str(&format!("pcb_mirror_bottom = {}\n", f(b.mirror_bottom)));
        t.push_str(&format!("pcb_zero_origin = {}\n", f(b.zero_origin)));
        t.push_str(&format!("pcb_tool_change_m6 = {}\n", f(b.tool_change_m6)));
        t.push_str(&format!("pcb_max_cells = {}\n", b.max_cells));
        t.push_str(&format!("wire_seed = {}\n", w.seed));
        t.push_str(&format!("wire_chunk_max = {}\n", w.chunk_max));
        t.push_str(&format!("wire_noise = {}\n", w.noise_per_mille));
        t.push_str(&format!("wire_flip = {}\n", w.flip_per_mille));
        t.push_str(&format!("marlin_numbering = {}\n", f(w.marlin_numbering)));
        t.push_str(&self.klipper.lines());
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
        assert!(c.guard.enabled, "the guard starts on");
        assert!(
            c.guard.jog_cancel_crossing,
            "crossing jogs are cancelled by default"
        );
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

    const CRED_SAMPLE: &str = "k7Q-sample-0123456789";

    #[test]
    fn the_klipper_keys_are_read_checked_and_the_credential_is_never_shown() {
        let text = format!(
            "klipper.host = 192.168.0.50\nklipper.port = 7126\nklipper.allow_non_local = off\nklipper.line_timeout_ms = 2500\nklipper.api_key = {CRED_SAMPLE}\n"
        );
        let (c, p) = MotionCfg::parse(&text);
        assert!(p.is_empty(), "{p:?}");
        assert!(c.klipper.has_api_key());
        assert_eq!(c.klipper.number("klipper.port"), 7126);
        assert_eq!(c.klipper.number("klipper.busy_limit_s"), 14_400);
        let http = c.klipper.http(None).unwrap();
        assert_eq!(
            (http.host.as_str(), http.port, http.has_api_key()),
            ("192.168.0.50", 7126, true)
        );
        assert_eq!(http.timeouts.line.as_millis(), 2500);
        // --klipper takes the place of the host, and of the port when it names one
        let over = c.klipper.http(Some(("10.0.0.7", None))).unwrap();
        assert_eq!((over.host.as_str(), over.port), ("10.0.0.7", 7126));
        let over = c.klipper.http(Some(("10.0.0.7", Some(80)))).unwrap();
        assert_eq!(over.port, 80);
        // nothing of the credential comes out: not in Debug, not in the text the program writes, not in a fault
        assert!(!format!("{c:?}").contains(CRED_SAMPLE));
        assert!(!format!("{http:?}").contains(CRED_SAMPLE));
        assert!(!c.to_text().contains(CRED_SAMPLE));
        let (back, _) = MotionCfg::parse(&c.to_text());
        assert!(!back.klipper.has_api_key(), "the text keeps no credential");
        let long = "x".repeat(257);
        for bad in ["a b", "", long.as_str()] {
            let (c, p) = MotionCfg::parse(&format!("klipper.api_key = {bad}\n"));
            assert!(!c.klipper.has_api_key());
            assert_eq!(p.len(), 1);
            assert_eq!(p[0].why, Why::SecretRefused);
            for l in crate::ctltext::LANGS {
                assert!(!p[0].text(l).contains(bad) || bad.is_empty());
            }
        }
    }

    #[test]
    fn the_wrong_klipper_keys_are_named_and_keep_their_default() {
        let (c, p) = MotionCfg::parse(
            "klipper.port = 70000\nklipper.max_in_flight = many\nklipper.allow_non_local = maybe\nklipper.speed = 3\nklipper.port = 7126\nklipper.jog_timeout_ms = 90000\n",
        );
        let whys: Vec<(usize, Why)> = p.iter().map(|p| (p.line, p.why)).collect();
        assert_eq!(
            whys,
            vec![
                (1, Why::OutOfRange),
                (2, Why::NotANumber),
                (3, Why::OnOff),
                (4, Why::UnknownKey),
                (5, Why::Repeated),
            ]
        );
        assert_eq!(p[0].detail, "1 - 65535");
        assert_eq!(c.klipper.number("klipper.port"), 7125);
        assert_eq!(c.klipper.number("klipper.max_in_flight"), 4);
        assert_eq!(c.klipper.number("klipper.jog_timeout_ms"), 90_000);
        assert!(!c.klipper.allow_non_local);
    }

    #[test]
    fn an_address_outside_the_local_network_is_refused_unless_the_switch_is_on() {
        for host in ["8.8.8.8", "example.org", "0.0.0.0", "239.1.2.3"] {
            let (c, p) = MotionCfg::parse(&format!("klipper.host = {host}\n"));
            assert_eq!(p.len(), 1, "{host}");
            assert_eq!((p[0].line, p[0].why), (1, Why::HostRefused), "{host}");
            assert_eq!(c.klipper.host, "127.0.0.1", "the default stays");
        }
        // with the switch (whichever line it is on) a public address is taken, an unspecified one never is
        for host in ["8.8.8.8", "93.184.216.34"] {
            let (c, p) = MotionCfg::parse(&format!(
                "klipper.host = {host}\nklipper.allow_non_local = on\n"
            ));
            assert!(p.is_empty(), "{host}: {p:?}");
            assert_eq!(c.klipper.host, host);
        }
        let (_, p) = MotionCfg::parse("klipper.host = 0.0.0.0\nklipper.allow_non_local = on\n");
        assert_eq!(p.len(), 1);
        for l in crate::ctltext::LANGS {
            assert!(!p[0].text(l).is_empty());
        }
    }

    #[test]
    fn a_script_that_would_be_cut_off_before_the_controller_gives_up_is_refused() {
        let (c, p) =
            MotionCfg::parse("klipper.busy_limit_s = 3600\nklipper.response_wait_ms = 60000\n");
        assert_eq!(p.len(), 1);
        assert_eq!((p[0].line, p[0].why), (2, Why::WaitShorter));
        assert_eq!(c.klipper.number("klipper.busy_limit_s"), 14_400);
        assert_eq!(c.klipper.number("klipper.response_wait_ms"), 14_460_000);
        // both moved together are fine
        let (_, p) =
            MotionCfg::parse("klipper.busy_limit_s = 60\nklipper.response_wait_ms = 61000\n");
        assert!(p.is_empty(), "{p:?}");
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
    fn the_guard_has_a_switch_that_starts_on_and_a_wrong_value_is_named() {
        let (c, p) = MotionCfg::parse("guard = off\n");
        assert!(p.is_empty() && !c.guard.enabled);
        // the other options of the guard stay as they were
        assert!(c.guard.stop_on_silence && c.guard.jog_one_order);
        for on in ["on", "yes", "true", "1"] {
            assert!(MotionCfg::parse(&format!("guard = {on}\n")).0.guard.enabled);
        }
        let (c, p) = MotionCfg::parse("jog_cancel_crossing = off\n");
        assert!(p.is_empty() && !c.guard.jog_cancel_crossing && c.guard.jog_one_order);
        let (c, p) = MotionCfg::parse("guard = maybe\n");
        assert!(
            c.guard.enabled,
            "a value that cannot be used keeps the default, which is on"
        );
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].why, Why::OnOff);
        // written and read back
        let mut off = MotionCfg::default();
        off.guard.enabled = false;
        assert!(off.to_text().contains("guard = off\n"));
        assert_eq!(MotionCfg::parse(&off.to_text()).0, off);
    }

    #[test]
    fn the_board_and_the_cable_settings_are_read_ranged_and_written_back() {
        let (c, p) = MotionCfg::parse(
            "pcb_tool = 0.1\npcb_passes = 4\npcb_mirror_bottom = off\npcb_peck = 0.3\nwire_seed = 77\nwire_chunk_max = 5\nwire_noise = 250\nwire_flip = 100\nmarlin_numbering = on\npcb_res = 5\npcb_passes = 99\nwire_noise = 2000\nwire_seed = -1\n",
        );
        assert_eq!((c.pcb.tool_mm, c.pcb.passes, c.pcb.peck_mm), (0.1, 4, 0.3));
        assert!(!c.pcb.mirror_bottom);
        assert_eq!(
            (
                c.wire.seed,
                c.wire.chunk_max,
                c.wire.noise_per_mille,
                c.wire.flip_per_mille
            ),
            (77, 5, 250, 100)
        );
        assert!(c.wire.marlin_numbering);
        // a value out of its range keeps the one before it, and so does a key given twice
        let lines: Vec<usize> = p.iter().map(|p| p.line).collect();
        assert_eq!(lines, vec![10, 11, 12, 13], "{p:?}");
        assert_eq!(c.pcb.res_mm, 0.05);
        // the defaults are the ones of a clean cable and of a board of 0.2 mm tools
        let d = MotionCfg::default();
        assert_eq!(
            (
                d.wire.flip_per_mille,
                d.wire.noise_per_mille,
                d.wire.chunk_max
            ),
            (0, 0, 0)
        );
        assert!(!d.wire.marlin_numbering && !d.pcb.zero_origin && d.pcb.mirror_bottom);
        // written and read back
        let (back, problems) = MotionCfg::parse(&c.to_text());
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(back, c);
    }

    #[test]
    fn the_model_file_fits_in_the_limits_it_has_to_respect() {
        let text = MotionCfg::default().to_text();
        assert!(
            (text.len() as u64) < MAX_FILE_BYTES / 2,
            "{} bytes",
            text.len()
        );
        assert!(text.lines().count() < MAX_LINES / 2);
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
