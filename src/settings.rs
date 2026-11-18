//! What the program remembers between runs: the language and the theme, the units, the travel limits of the machine, the steps of the
//! jog, which parts of the window are shown and the files opened last. It is a small text of `key = value` lines in the settings folder of
//! the user, with the other EH3D programs.

use std::path::{Path, PathBuf};

use eh3d_view::texts::Lang;
use eh3d_view::theme::ThemeId;

use crate::jog::Mode as JogMode;
use crate::lint::LintCfg;
use crate::probe::ProbeCfg;
use crate::tuning::Tuning;

/// How many recent files are kept.
pub const RECENT_MAX: usize = 8;

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub lang: Lang,
    pub theme: ThemeId,
    /// Lengths are shown in inches instead of millimetres (the machine and the program go on in mm).
    pub inches: bool,
    /// The limits of the travel of X, Y and Z in mm (minimum, maximum).
    pub travel: [(f64, f64); 3],
    /// The step and the speed of the jog at the start.
    pub step_mm: f64,
    pub feed_mm_min: f64,
    /// The speed of the spindle that the menu turns on with `M3`.
    pub spindle_rpm: f64,
    pub show_buttons: bool,
    pub show_program: bool,
    pub show_log: bool,
    pub show_rapids: bool,
    /// The files opened last, the newest first.
    pub recent: Vec<String>,
    /// The folder of the last file opened or saved.
    pub last_dir: String,
    /// The EH3D server a program can be sent to: its address (`host:port`), the token of this program and the id of the machine on the server.
    pub server_address: String,
    pub server_token: String,
    pub server_machine: String,
    /// When a program goes on after a stop, also start the spindle and the coolant again as they were (off: the operator starts them).
    pub server_restore_outputs: bool,
    /// When a program goes on after a failure: the Z (work coordinates) the head goes up to before it moves sideways, and the seconds the spindle gets to reach its speed.
    pub resume_safe_z_mm: f64,
    pub resume_spindle_wait_s: f64,
    /// Show the summary (time, reach, what looks wrong) before a program starts, and let the operator start it or not.
    pub confirm_start: bool,
    /// A program whose moves leave the travel of the machine cannot be started from the summary (off: it is only said, and can be started anyway).
    pub reject_out_of_travel: bool,
    /// A line of a program that starts with `$` (a setting or a command of the controller itself) is sent to it. Off: the program stops at that line without sending it.
    pub program_system_commands: bool,
    /// The speed of the rapid moves in mm per minute, which the estimate of the time uses.
    pub rapid_mm_min: f64,
    /// The checks of the summary: a program smaller than `tiny_mm` on both sides, or that starts farther than `far_mm` from the corner of the travel, looks like a wrong unit or offset; `margin_mm` is how far past the travel a move may go before it counts.
    pub summary_tiny_mm: f64,
    pub summary_far_mm: f64,
    pub summary_margin_mm: f64,
    /// The keys of the keyboard that open, save, start, pause and stop (off: only the mouse).
    pub shortcuts: bool,
    pub show_toolbar: bool,
    /// The file chosen in the Open window is drawn beside the list.
    pub open_preview: bool,
    /// The centre of the window shows the path in 3D instead of from above.
    pub view_3d: bool,
    /// On connecting to a GRBL controller, read its longest travel of X, Y and Z (`$130` to `$132`) and take the smaller of that and the
    /// travel above for the summary and the refusal (off: only the travel above counts).
    pub read_controller_travel: bool,
    /// The most a line of the program waits for its answer while the machine says it is working, in seconds (the controller answers a line only
    /// when its buffer has room, which behind a long move can take minutes). The stop is heard meanwhile.
    pub busy_wait_s: f64,
    /// When a program goes on after a failure and the program had set no feed: the feed (mm/min) the tool is lowered to its depth with.
    pub resume_plunge_feed_mm_min: f64,
    /// When a program goes on after a failure it is taken up this many lines before the last one that was sent, because the controller keeps
    /// some lines in its buffer and may not have run them.
    pub resume_backoff_lines: u32,
    /// A step of the jog that would take an axis past the travel is shortened to the limit, and one from the limit outwards is refused:
    /// always, only after the machine was homed (the default) or never.
    pub jog_soft_limits: JogMode,
    /// What looks wrong in a program besides the travel is said in the summary (`lint`), and the floor under which a rapid move is said.
    pub lint: LintCfg,
    /// The probe: speed, depth, retract, thickness of the plate and the height it travels at.
    pub probe: ProbeCfg,
    /// The distance between the points of a grid of the height map (mm), and the longest piece a cut is divided into when the program is corrected (mm).
    pub probe_grid_mm: f64,
    pub heightmap_segment_mm: f64,
    /// The values of the program tools (second checker, keep-out boxes, repeats and pauses of the queue, macros, session log, statistics).
    /// They live in the same file, so saving the settings writes them back and never drops them.
    pub tuning: Tuning,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            lang: Lang::En,
            theme: ThemeId::Forge,
            inches: false,
            travel: [(0.0, 500.0), (0.0, 300.0), (-100.0, 50.0)],
            step_mm: 1.0,
            feed_mm_min: 600.0,
            spindle_rpm: 10_000.0,
            show_buttons: true,
            show_program: true,
            show_log: true,
            show_rapids: true,
            recent: Vec::new(),
            last_dir: String::new(),
            server_address: String::new(),
            server_token: String::new(),
            server_machine: String::new(),
            server_restore_outputs: false,
            resume_safe_z_mm: 5.0,
            resume_spindle_wait_s: 3.0,
            confirm_start: true,
            reject_out_of_travel: true,
            program_system_commands: false,
            rapid_mm_min: 3000.0,
            summary_tiny_mm: 5.0,
            summary_far_mm: 1000.0,
            summary_margin_mm: 0.5,
            shortcuts: true,
            show_toolbar: true,
            open_preview: true,
            view_3d: false,
            read_controller_travel: true,
            busy_wait_s: 4.0 * 3600.0,
            resume_plunge_feed_mm_min: 100.0,
            resume_backoff_lines: 16,
            jog_soft_limits: JogMode::AfterHoming,
            lint: LintCfg::default(),
            probe: ProbeCfg::default(),
            probe_grid_mm: 10.0,
            heightmap_segment_mm: 1.0,
            tuning: Tuning::default(),
        }
    }
}

fn on(v: &str) -> bool {
    let v = v.trim();
    !["no", "off", "false", "0"]
        .iter()
        .any(|off| v.eq_ignore_ascii_case(off))
}

/// A value on one line: a line break in it could add keys to the file.
fn one_line(v: &str) -> String {
    v.replace(['\r', '\n'], " ").trim().to_string()
}

fn yes(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "no"
    }
}

impl Settings {
    /// The text of the file: `key = value` lines; what is not known or not valid keeps its default.
    pub fn parse(text: &str) -> Settings {
        let mut s = Settings {
            tuning: Tuning::parse(text),
            ..Settings::default()
        };
        // a file saved by an editor that writes a byte order mark still has its first line read
        for line in text.trim_start_matches('\u{feff}').lines() {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let (k, v) = (k.trim().to_ascii_lowercase(), v.trim());
            let num = |v: &str| v.parse::<f64>().ok().filter(|x| x.is_finite());
            match k.as_str() {
                "lang" => s.lang = Lang::from_code(v).unwrap_or(s.lang),
                "theme" => s.theme = ThemeId::from_key(v).unwrap_or(s.theme),
                "units" => {
                    s.inches = v.eq_ignore_ascii_case("in") || v.eq_ignore_ascii_case("inch")
                }
                "step" => {
                    s.step_mm = num(v)
                        .filter(|x| (0.001..=1000.0).contains(x))
                        .unwrap_or(s.step_mm)
                }
                "feed" => {
                    s.feed_mm_min = num(v)
                        .filter(|x| (1.0..=100_000.0).contains(x))
                        .unwrap_or(s.feed_mm_min)
                }
                "spindle" => {
                    s.spindle_rpm = num(v)
                        .filter(|x| (0.0..=1_000_000.0).contains(x))
                        .unwrap_or(s.spindle_rpm)
                }
                "show_buttons" => s.show_buttons = on(v),
                "show_program" => s.show_program = on(v),
                "show_log" => s.show_log = on(v),
                "show_rapids" => s.show_rapids = on(v),
                "last_dir" => s.last_dir = v.to_string(),
                "server_address" => s.server_address = v.to_string(),
                "server_token" => s.server_token = v.to_string(),
                "server_machine" => s.server_machine = v.to_string(),
                "server_restore_outputs" => s.server_restore_outputs = on(v),
                "resume_safe_z" => {
                    s.resume_safe_z_mm = num(v)
                        .filter(|x| (-500.0..=500.0).contains(x))
                        .unwrap_or(s.resume_safe_z_mm)
                }
                "resume_spindle_wait" => {
                    s.resume_spindle_wait_s = num(v)
                        .filter(|x| (0.0..=120.0).contains(x))
                        .unwrap_or(s.resume_spindle_wait_s)
                }
                "confirm_start" => s.confirm_start = on(v),
                "reject_out_of_travel" => s.reject_out_of_travel = on(v),
                "program_system_commands" => s.program_system_commands = on(v),
                "rapid" => {
                    s.rapid_mm_min = num(v)
                        .filter(|x| (1.0..=1_000_000.0).contains(x))
                        .unwrap_or(s.rapid_mm_min)
                }
                "summary_tiny" => {
                    s.summary_tiny_mm = num(v)
                        .filter(|x| (0.0..=10_000.0).contains(x))
                        .unwrap_or(s.summary_tiny_mm)
                }
                "summary_far" => {
                    s.summary_far_mm = num(v)
                        .filter(|x| (0.0..=1_000_000.0).contains(x))
                        .unwrap_or(s.summary_far_mm)
                }
                "summary_margin" => {
                    s.summary_margin_mm = num(v)
                        .filter(|x| (0.0..=1000.0).contains(x))
                        .unwrap_or(s.summary_margin_mm)
                }
                "busy_wait" => {
                    s.busy_wait_s = num(v)
                        .filter(|x| (0.0..=604_800.0).contains(x))
                        .unwrap_or(s.busy_wait_s)
                }
                "resume_plunge_feed" => {
                    s.resume_plunge_feed_mm_min = num(v)
                        .filter(|x| (1.0..=100_000.0).contains(x))
                        .unwrap_or(s.resume_plunge_feed_mm_min)
                }
                "resume_backoff" => {
                    s.resume_backoff_lines = v
                        .parse::<u32>()
                        .ok()
                        .filter(|x| *x <= 1000)
                        .unwrap_or(s.resume_backoff_lines)
                }
                "jog_soft_limits" => {
                    s.jog_soft_limits = JogMode::from_key(v).unwrap_or(s.jog_soft_limits)
                }
                "lint" => s.lint.enabled = on(v),
                "lint_rapid_floor" => {
                    s.lint.rapid_floor_mm = num(v)
                        .filter(|x| (-500.0..=500.0).contains(x))
                        .unwrap_or(s.lint.rapid_floor_mm)
                }
                "lint_max_feed" => {
                    s.lint.max_feed_mm_min = num(v)
                        .filter(|x| (0.0..=1_000_000.0).contains(x))
                        .unwrap_or(s.lint.max_feed_mm_min)
                }
                "lint_max_rpm" => {
                    s.lint.max_rpm = num(v)
                        .filter(|x| (0.0..=1_000_000.0).contains(x))
                        .unwrap_or(s.lint.max_rpm)
                }
                "probe_feed" => {
                    s.probe.feed_mm_min = num(v)
                        .filter(|x| (1.0..=1000.0).contains(x))
                        .unwrap_or(s.probe.feed_mm_min)
                }
                "probe_depth" => {
                    s.probe.depth_mm = num(v)
                        .filter(|x| (0.1..=200.0).contains(x))
                        .unwrap_or(s.probe.depth_mm)
                }
                "probe_retract" => {
                    s.probe.retract_mm = num(v)
                        .filter(|x| (0.0..=100.0).contains(x))
                        .unwrap_or(s.probe.retract_mm)
                }
                "probe_plate" => {
                    s.probe.plate_mm = num(v)
                        .filter(|x| (0.0..=100.0).contains(x))
                        .unwrap_or(s.probe.plate_mm)
                }
                "probe_clearance" => {
                    s.probe.clearance_mm = num(v)
                        .filter(|x| (0.5..=200.0).contains(x))
                        .unwrap_or(s.probe.clearance_mm)
                }
                "probe_grid" => {
                    s.probe_grid_mm = num(v)
                        .filter(|x| (1.0..=1000.0).contains(x))
                        .unwrap_or(s.probe_grid_mm)
                }
                "heightmap_segment" => {
                    s.heightmap_segment_mm = num(v)
                        .filter(|x| (0.05..=100.0).contains(x))
                        .unwrap_or(s.heightmap_segment_mm)
                }
                "shortcuts" => s.shortcuts = on(v),
                "show_toolbar" => s.show_toolbar = on(v),
                "open_preview" => s.open_preview = on(v),
                "view_3d" => s.view_3d = on(v),
                "read_controller_travel" => s.read_controller_travel = on(v),
                "recent" => {
                    s.recent = v
                        .split('|')
                        .map(str::trim)
                        .filter(|p| !p.is_empty())
                        .take(RECENT_MAX)
                        .map(str::to_string)
                        .collect()
                }
                "travel_x" | "travel_y" | "travel_z" => {
                    let i = (k.as_bytes()[7] - b'x') as usize;
                    if let Some((lo, hi)) = v.split_once(',') {
                        if let (Some(lo), Some(hi)) = (num(lo.trim()), num(hi.trim())) {
                            if hi > lo {
                                s.travel[i] = (lo, hi);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        s
    }

    pub fn to_text(&self) -> String {
        let t = &self.travel;
        let mut out = format!(
            "lang = {}\ntheme = {}\nunits = {}\nstep = {}\nfeed = {}\nspindle = {}\nshow_buttons = {}\nshow_program = {}\nshow_log = {}\nshow_rapids = {}\ntravel_x = {},{}\ntravel_y = {},{}\ntravel_z = {},{}\nlast_dir = {}\nrecent = {}\nserver_address = {}\nserver_token = {}\nserver_machine = {}\nserver_restore_outputs = {}\nresume_safe_z = {}\nresume_spindle_wait = {}\nconfirm_start = {}\nreject_out_of_travel = {}\nprogram_system_commands = {}\nrapid = {}\nsummary_tiny = {}\nsummary_far = {}\nsummary_margin = {}\nshortcuts = {}\nshow_toolbar = {}\nopen_preview = {}\nview_3d = {}\nread_controller_travel = {}\nbusy_wait = {}\nresume_plunge_feed = {}\nresume_backoff = {}\n",
            self.lang.code(),
            self.theme.key(),
            if self.inches { "in" } else { "mm" },
            self.step_mm,
            self.feed_mm_min,
            self.spindle_rpm,
            yes(self.show_buttons),
            yes(self.show_program),
            yes(self.show_log),
            yes(self.show_rapids),
            t[0].0,
            t[0].1,
            t[1].0,
            t[1].1,
            t[2].0,
            t[2].1,
            one_line(&self.last_dir),
            self.recent
                .iter()
                .map(|p| one_line(p))
                .collect::<Vec<_>>()
                .join("|"),
            one_line(&self.server_address),
            one_line(&self.server_token),
            one_line(&self.server_machine),
            yes(self.server_restore_outputs),
            self.resume_safe_z_mm,
            self.resume_spindle_wait_s,
            yes(self.confirm_start),
            yes(self.reject_out_of_travel),
            yes(self.program_system_commands),
            self.rapid_mm_min,
            self.summary_tiny_mm,
            self.summary_far_mm,
            self.summary_margin_mm,
            yes(self.shortcuts),
            yes(self.show_toolbar),
            yes(self.open_preview),
            yes(self.view_3d),
            yes(self.read_controller_travel),
            self.busy_wait_s,
            self.resume_plunge_feed_mm_min,
            self.resume_backoff_lines
        );
        out.push_str(&format!(
            "jog_soft_limits = {}\nlint = {}\nlint_rapid_floor = {}\nlint_max_feed = {}\nlint_max_rpm = {}\nprobe_feed = {}\nprobe_depth = {}\nprobe_retract = {}\nprobe_plate = {}\nprobe_clearance = {}\nprobe_grid = {}\nheightmap_segment = {}\n",
            self.jog_soft_limits.key(),
            yes(self.lint.enabled),
            self.lint.rapid_floor_mm,
            self.lint.max_feed_mm_min,
            self.lint.max_rpm,
            self.probe.feed_mm_min,
            self.probe.depth_mm,
            self.probe.retract_mm,
            self.probe.plate_mm,
            self.probe.clearance_mm,
            self.probe_grid_mm,
            self.heightmap_segment_mm
        ));
        out.push_str(&self.tuning.to_text());
        out
    }

    /// The checks of the program go back to their defaults (the keep-out boxes stay).
    pub fn reset_checks(&mut self) {
        let d = Settings::default();
        self.lint = d.lint;
        self.tuning.progcheck = d.tuning.progcheck;
        self.tuning.keepout_margin = d.tuning.keepout_margin;
        self.tuning.keepout_reject = d.tuning.keepout_reject;
    }

    /// The probe, the grid of the height map and the limits of the jog go back to their defaults.
    pub fn reset_probe(&mut self) {
        let d = Settings::default();
        self.probe = d.probe;
        self.probe_grid_mm = d.probe_grid_mm;
        self.heightmap_segment_mm = d.heightmap_segment_mm;
        self.jog_soft_limits = d.jog_soft_limits;
    }

    /// The queue, the simulation, the session log, the macros and the time of a tool change go back to their defaults (the keep-out boxes stay).
    pub fn reset_queue(&mut self) {
        let d = Tuning::default();
        let t = &mut self.tuning;
        t.repeat_max = d.repeat_max;
        t.confirm_between = d.confirm_between;
        t.pause_s = d.pause_s;
        t.queue_remember = d.queue_remember;
        t.sim_speed = d.sim_speed;
        t.log_max = d.log_max;
        t.macros_confirm = d.macros_confirm;
        t.macro_max_lines = d.macro_max_lines;
        t.tool_change_s = d.tool_change_s;
    }

    /// Puts a file at the head of the recent ones (it is not listed twice).
    pub fn remember_recent(&mut self, path: &str) {
        self.recent.retain(|p| p != path);
        self.recent.insert(0, path.to_string());
        self.recent.truncate(RECENT_MAX);
    }

    pub fn load(path: &Path) -> Settings {
        std::fs::read_to_string(path)
            .map(|t| Settings::parse(&t))
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, self.to_text())
    }
}

/// Where the file is: the settings folder of the user, with the other EH3D programs.
pub fn settings_path() -> PathBuf {
    // the tests never touch the settings of the user
    if cfg!(test) {
        return std::env::temp_dir()
            .join(format!("eh3d-phoenix-cnc-test-{}.cfg", std::process::id()));
    }
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("EH3D")
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .unwrap_or_else(|| PathBuf::from("."))
            .join("eh3d")
    };
    base.join("phoenix-cnc.cfg")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boxes::KeepOut;

    #[test]
    fn the_settings_are_written_and_read_back() {
        let mut s = Settings {
            lang: Lang::Ja,
            theme: ThemeId::Midnight,
            inches: true,
            travel: [(-10.0, 410.5), (0.0, 250.0), (-80.0, 20.0)],
            step_mm: 0.1,
            feed_mm_min: 1200.0,
            spindle_rpm: 18_000.0,
            show_buttons: false,
            show_program: true,
            show_log: false,
            show_rapids: false,
            recent: vec![],
            last_dir: "C:/jobs".into(),
            server_address: "192.168.0.180:18200".into(),
            server_token: "a-token".into(),
            server_machine: "router".into(),
            server_restore_outputs: true,
            resume_safe_z_mm: 7.5,
            resume_spindle_wait_s: 4.0,
            confirm_start: false,
            reject_out_of_travel: false,
            program_system_commands: true,
            rapid_mm_min: 4500.0,
            summary_tiny_mm: 2.5,
            summary_far_mm: 800.0,
            summary_margin_mm: 1.0,
            shortcuts: false,
            show_toolbar: false,
            open_preview: false,
            view_3d: true,
            read_controller_travel: false,
            busy_wait_s: 600.0,
            resume_plunge_feed_mm_min: 60.0,
            resume_backoff_lines: 4,
            jog_soft_limits: JogMode::Off,
            lint: LintCfg {
                enabled: false,
                rapid_floor_mm: -1.5,
                max_feed_mm_min: 2400.0,
                max_rpm: 24_000.0,
            },
            probe: ProbeCfg {
                feed_mm_min: 15.0,
                depth_mm: 6.0,
                retract_mm: 3.0,
                plate_mm: 1.6,
                clearance_mm: 8.0,
            },
            probe_grid_mm: 5.0,
            heightmap_segment_mm: 0.5,
            tuning: Tuning {
                progcheck: false,
                keepouts: vec![KeepOut {
                    name: "clamp".into(),
                    min: [0.0, 0.0, -5.0],
                    max: [20.0, 20.0, 40.0],
                }],
                repeat_max: 7,
                ..Tuning::default()
            },
        };
        s.remember_recent("C:/jobs/a.nc");
        s.remember_recent("C:/jobs/b.nc");
        let path = std::env::temp_dir().join(format!(
            "eh3d-phoenix-cnc-settings-{}.cfg",
            std::process::id()
        ));
        s.save(&path).unwrap();
        assert_eq!(Settings::load(&path), s);
        assert_eq!(
            Settings::parse(&Settings::default().to_text()),
            Settings::default()
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn saving_the_settings_keeps_the_keys_of_the_program_tools() {
        let file = "lang = de\nqueue_repeat_max = 9\nqueue_pause = 12\nkeepout = vise 0 0 0 10 10 10\nprogcheck = no\n";
        let mut s = Settings::parse(file);
        assert_eq!(s.tuning.repeat_max, 9);
        // the window changes the language and saves: the other group of keys must still be in the file
        s.lang = Lang::Fr;
        let path = std::env::temp_dir().join(format!(
            "eh3d-phoenix-cnc-tuning-keep-{}.cfg",
            std::process::id()
        ));
        s.save(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(text.contains("queue_repeat_max = 9") && text.contains("keepout = vise"));
        let back = Settings::parse(&text);
        assert_eq!(back.lang, Lang::Fr);
        assert_eq!(back.tuning, s.tuning);
        assert!(!back.tuning.progcheck);
        assert_eq!(back.tuning.pause_s, 12.0);
        assert_eq!(back.tuning.keepouts.len(), 1);
    }

    #[test]
    fn the_keys_of_the_two_groups_are_not_mixed_up() {
        // a key of the tools does not touch a setting and the other way round
        let s = Settings::parse("step = 3\nqueue_pause = 4\n");
        assert_eq!(s.step_mm, 3.0);
        assert_eq!(s.tuning.pause_s, 4.0);
        assert_eq!(Settings::default().tuning, Tuning::default());
    }

    #[test]
    fn the_server_is_in_the_settings_file_empty_by_default_and_a_line_break_cannot_add_keys() {
        let d = Settings::default();
        assert_eq!(
            (
                d.server_address.as_str(),
                d.server_token.as_str(),
                d.server_machine.as_str(),
                d.server_restore_outputs
            ),
            ("", "", "", false)
        );
        let s = Settings::parse(
            "server_address = 10.0.0.2:18200\nserver_token = t\nserver_machine = m1\nserver_restore_outputs = yes\n",
        );
        assert_eq!(
            (
                s.server_address.as_str(),
                s.server_token.as_str(),
                s.server_machine.as_str(),
                s.server_restore_outputs
            ),
            ("10.0.0.2:18200", "t", "m1", true)
        );
        let t = Settings {
            server_token: "x\nlang = ja".into(),
            ..Settings::default()
        };
        assert_eq!(Settings::parse(&t.to_text()).lang, Lang::En);
    }

    #[test]
    fn what_is_wrong_keeps_the_default() {
        let s = Settings::parse(
            "lang = xx\ntheme = pink\nstep = 0\nfeed = fast\ntravel_x = 10,5\ntravel_y = a,b\nspindle = -5\nsomething else\nunits = in\n",
        );
        let d = Settings::default();
        assert_eq!(
            (s.lang, s.theme, s.step_mm, s.feed_mm_min),
            (d.lang, d.theme, d.step_mm, d.feed_mm_min)
        );
        assert_eq!((s.travel, s.spindle_rpm), (d.travel, d.spindle_rpm));
        assert!(s.inches);
    }

    #[test]
    fn the_wait_for_a_busy_machine_and_the_way_a_program_is_taken_up_again_are_in_the_settings_file(
    ) {
        let d = Settings::default();
        assert_eq!(d.busy_wait_s, 14_400.0);
        assert_eq!(
            (d.resume_plunge_feed_mm_min, d.resume_backoff_lines),
            (100.0, 16)
        );
        let s = Settings::parse("busy_wait = 30\nresume_plunge_feed = 45\nresume_backoff = 8\n");
        assert_eq!(
            (
                s.busy_wait_s,
                s.resume_plunge_feed_mm_min,
                s.resume_backoff_lines
            ),
            (30.0, 45.0, 8)
        );
        // what is out of range or not a number keeps the default
        let s = Settings::parse("busy_wait = -5\nresume_plunge_feed = 0\nresume_backoff = 5000\n");
        assert_eq!(
            (
                s.busy_wait_s,
                s.resume_plunge_feed_mm_min,
                s.resume_backoff_lines
            ),
            (
                d.busy_wait_s,
                d.resume_plunge_feed_mm_min,
                d.resume_backoff_lines
            )
        );
        let s = Settings::parse("busy_wait = soon\nresume_backoff = many\n");
        assert_eq!(
            (s.busy_wait_s, s.resume_backoff_lines),
            (d.busy_wait_s, d.resume_backoff_lines)
        );
    }

    #[test]
    fn the_probe_the_height_map_the_jog_limits_and_the_checks_are_in_the_settings_file() {
        let d = Settings::default();
        assert!(d.jog_soft_limits == JogMode::AfterHoming && d.lint.enabled);
        assert_eq!(d.probe, ProbeCfg::default());
        assert_eq!((d.probe_grid_mm, d.heightmap_segment_mm), (10.0, 1.0));
        assert_eq!((d.lint.max_feed_mm_min, d.lint.max_rpm), (5000.0, 24_000.0));
        let s = Settings::parse(
            "jog_soft_limits = no\nlint = off\nlint_rapid_floor = -2\nlint_max_feed = 1800\nlint_max_rpm = 18000\nprobe_feed = 12\nprobe_depth = 4\nprobe_retract = 1\nprobe_plate = 1.2\nprobe_clearance = 6\nprobe_grid = 8\nheightmap_segment = 0.25\n",
        );
        assert!(s.jog_soft_limits == JogMode::Off && !s.lint.enabled);
        assert_eq!(
            Settings::parse("jog_soft_limits = always\n").jog_soft_limits,
            JogMode::Always
        );
        // a word that is not one keeps the default
        assert_eq!(
            Settings::parse("jog_soft_limits = maybe\n").jog_soft_limits,
            JogMode::AfterHoming
        );
        assert_eq!(s.lint.rapid_floor_mm, -2.0);
        assert_eq!((s.lint.max_feed_mm_min, s.lint.max_rpm), (1800.0, 18_000.0));
        assert_eq!(
            s.probe,
            ProbeCfg {
                feed_mm_min: 12.0,
                depth_mm: 4.0,
                retract_mm: 1.0,
                plate_mm: 1.2,
                clearance_mm: 6.0
            }
        );
        assert_eq!((s.probe_grid_mm, s.heightmap_segment_mm), (8.0, 0.25));
        // what is out of range or not a number keeps the default
        let s = Settings::parse(
            "probe_feed = 0\nprobe_depth = deep\nprobe_retract = -1\nprobe_plate = 900\nprobe_clearance = 0\nprobe_grid = 0\nheightmap_segment = 0\nlint_rapid_floor = 1e9\nlint_max_feed = -5\nlint_max_rpm = fast\n",
        );
        assert_eq!(s.probe, d.probe);
        assert_eq!((s.probe_grid_mm, s.heightmap_segment_mm), (10.0, 1.0));
        assert_eq!(s.lint, d.lint);
    }

    #[test]
    fn only_the_last_files_are_remembered_and_none_twice() {
        let mut s = Settings::default();
        for i in 0..12 {
            s.remember_recent(&format!("C:/jobs/{i}.nc"));
        }
        assert_eq!(s.recent.len(), RECENT_MAX);
        assert_eq!(s.recent[0], "C:/jobs/11.nc");
        s.remember_recent("C:/jobs/9.nc");
        assert_eq!(s.recent[0], "C:/jobs/9.nc");
        assert_eq!(s.recent.iter().filter(|p| p.ends_with("/9.nc")).count(), 1);
        assert_eq!(Settings::parse(&s.to_text()).recent, s.recent);
    }

    #[test]
    fn the_safety_and_interface_settings_default_to_the_careful_choice_and_what_is_wrong_keeps_it()
    {
        let d = Settings::default();
        assert!(d.confirm_start && d.reject_out_of_travel && d.shortcuts);
        assert!(d.read_controller_travel);
        let s = Settings::parse(
            "rapid = 0\nsummary_far = -3\nsummary_margin = lots\nconfirm_start = no\n",
        );
        assert_eq!(s.rapid_mm_min, d.rapid_mm_min);
        assert_eq!(s.summary_far_mm, d.summary_far_mm);
        assert_eq!(s.summary_margin_mm, d.summary_margin_mm);
        assert!(!s.confirm_start);
    }
    fn changed() -> Settings {
        let mut s = Settings::parse(
            "lint = no\nlint_max_feed = 100\nprogcheck = no\nkeepout_margin = 9\nkeepout_reject = no\nkeepout = clamp 0 0 -5 20 20 40\n             probe_feed = 50\nprobe_grid = 40\nheightmap_segment = 5\njog_soft_limits = always\n             queue_repeat_max = 7\nqueue_pause = 60\nqueue_confirm_between = no\nqueue_remember = no\nsim_speed = 99\n             log_max_entries = 77\nmacros_confirm = no\nmacros_max_lines = 9\ntool_change_time = 5\nstep = 0.1\nserver_token = t\n",
        );
        s.recent = vec!["a.nc".into()];
        s
    }

    #[test]
    fn each_section_of_the_program_tools_goes_back_to_its_defaults_and_leaves_the_rest() {
        let d = Settings::default();
        let mut s = changed();
        s.reset_checks();
        assert_eq!((s.lint, s.tuning.progcheck), (d.lint, true));
        assert_eq!(s.tuning.keepout_margin, 1.0);
        assert!(s.tuning.keepout_reject);
        // the boxes, the other sections and the rest of the settings stay
        assert_eq!(s.tuning.keepouts.len(), 1);
        assert_eq!((s.probe.feed_mm_min, s.tuning.repeat_max), (50.0, 7));
        assert_eq!((s.step_mm, s.server_token.as_str()), (0.1, "t"));
        s.reset_probe();
        assert_eq!((s.probe, s.probe_grid_mm), (d.probe, d.probe_grid_mm));
        assert_eq!(s.heightmap_segment_mm, d.heightmap_segment_mm);
        assert_eq!(s.jog_soft_limits, d.jog_soft_limits);
        assert_eq!(s.tuning.repeat_max, 7);
        s.reset_queue();
        let want = Tuning {
            keepouts: s.tuning.keepouts.clone(),
            ..Tuning::default()
        };
        assert_eq!(s.tuning, want);
        assert_eq!(s.recent, ["a.nc"]);
        // what was reset is what the file says too
        assert_eq!(Settings::parse(&s.to_text()), s);
    }
}
