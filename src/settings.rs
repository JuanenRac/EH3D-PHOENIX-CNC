//! What the program remembers between runs: the language and the theme, the units, the travel limits of the machine, the steps of the
//! jog, which parts of the window are shown and the files opened last. It is a small text of `key = value` lines in the settings folder of
//! the user, with the other EH3D programs.

use std::path::{Path, PathBuf};

use eh3d_view::texts::Lang;
use eh3d_view::theme::ThemeId;

use crate::jog::Mode as JogMode;
use crate::lint::LintCfg;
use crate::probe::ProbeCfg;
use crate::profiles::Profile;
use crate::tuning::Tuning;

/// How many recent files are kept.
pub const RECENT_MAX: usize = 8;

/// The most lines of the limits of the attachments that are kept.
const PANEL_LINES_MAX: usize = 32;

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
    /// The limits of the link to the server, the same keys and ranges every program of the suite reads: seconds one try may take (1 to 120), tries more
    /// after a failure on the network (0 to 5), whether to look for the server on the local network (off), and the most MiB read from one answer (1 to 256).
    pub server_timeout_s: u64,
    pub server_retries: u32,
    pub server_mdns: bool,
    pub server_max_answer_mib: u64,
    /// For a job sent to the EH3D server that goes on after a stop: also turn the coolant on again (off: it stays off). The spindle does not depend on
    /// this: the server starts the one the program had on, after the operator lets the job go.
    pub server_restore_outputs: bool,
    /// The limits of a jog sent to the machine of the server, the same keys and ranges as the machine of the server (`server_jog_max_mm` 0.01 to 500, `server_jog_max_feed` 1 to 30000),
    /// and the length above which the window asks before it sends a jog (`server_jog_confirm_mm`, 0 to 500; 0 asks for every jog).
    pub server_jog_max_mm: f64,
    pub server_jog_max_feed: f64,
    pub server_jog_confirm_mm: f64,
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
    /// The machine profiles (the travel, the spindle, the feeds and the probe of each machine) and the name of the one in use ("" when the
    /// values were set by hand).
    pub profiles: Vec<Profile>,
    pub profile: String,
    /// The name of the material in use, from the library of materials ("" for none): its ranges of feed and speed are held in `lint`.
    pub material: String,
    /// The lines of the docking window (`dock.*`): the arrangement of the panels, the workspaces, the shortcuts of the frame and the recent
    /// files. The program keeps them as they are; the frame reads and writes them.
    pub dock_lines: Vec<(String, String)>,
    /// The lines of the limits of the panel «Attachments» (`attachments.max_mb`, `viewer.max_*_mb`), kept as they were written.
    pub panel_lines: Vec<(String, String)>,
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
            server_timeout_s: 15,
            server_retries: 2,
            server_mdns: false,
            server_max_answer_mib: 8,
            server_restore_outputs: false,
            server_jog_max_mm: crate::cncroutes::DEFAULT_JOG_MAX_MM,
            server_jog_max_feed: crate::cncroutes::DEFAULT_JOG_MAX_FEED,
            server_jog_confirm_mm: crate::cncroutes::DEFAULT_CONFIRM_MM,
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
            profiles: Vec::new(),
            profile: String::new(),
            material: String::new(),
            dock_lines: Vec::new(),
            panel_lines: Vec::new(),
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
pub(crate) fn one_line(v: &str) -> String {
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
                "material" => {
                    s.material = crate::profiles::clean_name(v).unwrap_or_default();
                    s.tuning.material = s.material.clone();
                }
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
                "server_timeout_s" => {
                    s.server_timeout_s = v
                        .trim()
                        .parse::<u64>()
                        .ok()
                        .filter(|x| (1..=120).contains(x))
                        .unwrap_or(s.server_timeout_s)
                }
                "server_retries" => {
                    s.server_retries = v
                        .trim()
                        .parse::<u32>()
                        .ok()
                        .filter(|x| *x <= 5)
                        .unwrap_or(s.server_retries)
                }
                "server_mdns" => {
                    s.server_mdns = ["on", "yes", "true", "1"]
                        .iter()
                        .any(|t| v.trim().eq_ignore_ascii_case(t))
                }
                "server_max_answer_mib" => {
                    s.server_max_answer_mib = v
                        .trim()
                        .parse::<u64>()
                        .ok()
                        .filter(|x| (1..=256).contains(x))
                        .unwrap_or(s.server_max_answer_mib)
                }
                "server_restore_outputs" => s.server_restore_outputs = on(v),
                "server_jog_max_mm" => {
                    s.server_jog_max_mm = num(v)
                        .filter(|x| {
                            (crate::cncroutes::JOG_MAX_MM_RANGE.0
                                ..=crate::cncroutes::JOG_MAX_MM_RANGE.1)
                                .contains(x)
                        })
                        .unwrap_or(s.server_jog_max_mm)
                }
                "server_jog_max_feed" => {
                    s.server_jog_max_feed = num(v)
                        .filter(|x| {
                            (crate::cncroutes::JOG_MAX_FEED_RANGE.0
                                ..=crate::cncroutes::JOG_MAX_FEED_RANGE.1)
                                .contains(x)
                        })
                        .unwrap_or(s.server_jog_max_feed)
                }
                "server_jog_confirm_mm" => {
                    s.server_jog_confirm_mm = num(v)
                        .filter(|x| {
                            (crate::cncroutes::CONFIRM_MM_RANGE.0
                                ..=crate::cncroutes::CONFIRM_MM_RANGE.1)
                                .contains(x)
                        })
                        .unwrap_or(s.server_jog_confirm_mm)
                }
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
                "lint_arcs" => s.lint.arcs = on(v),
                "lint_arc_gap" => {
                    s.lint.arc_gap_mm = num(v)
                        .filter(|x| (0.0001..=0.5).contains(x))
                        .unwrap_or(s.lint.arc_gap_mm)
                }
                "lint_reversal" => s.lint.reversal = on(v),
                "lint_zero_feed" => s.lint.zero_feed = on(v),
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
                other if other.starts_with("dock.") => {
                    s.dock_lines.retain(|(k, _)| k != other);
                    s.dock_lines.push((other.to_string(), v.to_string()));
                }
                other if eh3d_attachpanel::PanelLimits::is_key(other) => {
                    s.panel_lines.retain(|(k, _)| k != other);
                    if s.panel_lines.len() < PANEL_LINES_MAX {
                        s.panel_lines.push((other.to_string(), v.to_string()));
                    }
                }
                _ => {}
            }
        }
        s.read_profiles(text);
        s
    }

    pub fn to_text(&self) -> String {
        let t = &self.travel;
        let mut out = format!(
            "lang = {}\ntheme = {}\nunits = {}\nstep = {}\nfeed = {}\nspindle = {}\nshow_buttons = {}\nshow_program = {}\nshow_log = {}\nshow_rapids = {}\ntravel_x = {},{}\ntravel_y = {},{}\ntravel_z = {},{}\nlast_dir = {}\nrecent = {}\nserver_address = {}\nserver_token = {}\nserver_machine = {}\nserver_timeout_s = {}\nserver_retries = {}\nserver_mdns = {}\nserver_max_answer_mib = {}\nserver_restore_outputs = {}\nserver_jog_max_mm = {}\nserver_jog_max_feed = {}\nserver_jog_confirm_mm = {}\nresume_safe_z = {}\nresume_spindle_wait = {}\nconfirm_start = {}\nreject_out_of_travel = {}\nprogram_system_commands = {}\nrapid = {}\nsummary_tiny = {}\nsummary_far = {}\nsummary_margin = {}\nshortcuts = {}\nshow_toolbar = {}\nopen_preview = {}\nview_3d = {}\nread_controller_travel = {}\nbusy_wait = {}\nresume_plunge_feed = {}\nresume_backoff = {}\n",
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
            self.server_timeout_s,
            self.server_retries,
            yes(self.server_mdns),
            self.server_max_answer_mib,
            yes(self.server_restore_outputs),
            self.server_jog_max_mm,
            self.server_jog_max_feed,
            self.server_jog_confirm_mm,
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
            "jog_soft_limits = {}\nlint = {}\nlint_rapid_floor = {}\nlint_max_feed = {}\nlint_max_rpm = {}\nlint_arcs = {}\nlint_arc_gap = {}\nlint_reversal = {}\nlint_zero_feed = {}\nprobe_feed = {}\nprobe_depth = {}\nprobe_retract = {}\nprobe_plate = {}\nprobe_clearance = {}\nprobe_grid = {}\nheightmap_segment = {}\n",
            self.jog_soft_limits.key(),
            yes(self.lint.enabled),
            self.lint.rapid_floor_mm,
            self.lint.max_feed_mm_min,
            self.lint.max_rpm,
            yes(self.lint.arcs),
            self.lint.arc_gap_mm,
            yes(self.lint.reversal),
            yes(self.lint.zero_feed),
            self.probe.feed_mm_min,
            self.probe.depth_mm,
            self.probe.retract_mm,
            self.probe.plate_mm,
            self.probe.clearance_mm,
            self.probe_grid_mm,
            self.heightmap_segment_mm
        ));
        out.push_str(&self.profiles_text());
        out.push_str(&format!("material = {}\n", one_line(&self.material)));
        out.push_str(&self.tuning.to_text());
        for (k, v) in self.dock_lines.iter().chain(&self.panel_lines) {
            out.push_str(&format!("{k} = {v}\n"));
        }
        out
    }

    /// The limits of the panel of the attachments that the file gives; a value that is not valid keeps the default.
    pub fn panel_limits(&self) -> eh3d_attachpanel::PanelLimits {
        let mut limits = eh3d_attachpanel::PanelLimits::default();
        for (k, v) in &self.panel_lines {
            limits.apply(k, v);
        }
        limits
    }

    /// The lines of the docking window, as the text the frame reads.
    pub fn dock_text(&self) -> String {
        self.dock_lines
            .iter()
            .map(|(k, v)| format!("{k} = {v}\n"))
            .collect()
    }

    /// Takes the lines the frame wrote (they replace the ones there were).
    pub fn set_dock(&mut self, pairs: &[(String, String)]) {
        self.dock_lines = pairs
            .iter()
            .filter(|(k, _)| k.starts_with("dock."))
            .cloned()
            .collect();
    }

    /// Gives the checker the ranges of the material in use (the name in `material`, looked for in the library and then in the shared database of
    /// materials), or none when there is no such material.
    pub fn resolve_material(&mut self, library: &[crate::materials::Material]) {
        self.lint.material_feed = None;
        self.lint.material_rpm = None;
        self.tuning.material = self.material.clone();
        if let Some(m) = crate::materials::find(library, &self.material) {
            m.apply(&mut self.lint);
        } else if let Some(m) = crate::sharedmat::shared_material(&self.material) {
            // no material of the library has this name: the shared database may (`wood/mdf`, or the name in any of the eight languages)
            m.apply(&mut self.lint);
        }
    }

    /// Gives the settings the table of tools read from its file (see `tools::load`).
    pub fn resolve_tool_table(&mut self, table: crate::tools::ToolTable) {
        self.tuning.tools = table;
    }

    /// Gives the tool changer the table of slots read from its file (see `atc::load_table`).
    pub fn resolve_atc_table(&mut self, table: crate::atc::RackTable) {
        self.tuning.atc.table = table;
    }

    /// The checks of the program go back to their defaults (the keep-out boxes stay).
    pub fn reset_checks(&mut self) {
        let d = Settings::default();
        self.lint = d.lint;
        self.material.clear();
        self.tuning.material.clear();
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

    /// The queue, the simulation, the session log, the macros and the time of a tool change go back to their defaults (the keep-out boxes
    /// stay).
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
        t.startline_confirm = d.startline_confirm;
        t.bookmarks_load = d.bookmarks_load;
        t.bookmarks_confirm_replace = d.bookmarks_confirm_replace;
    }

    /// The tool changer goes back to its defaults (off); the table of slots lives in its own file and stays.
    pub fn reset_atc(&mut self) {
        let table = std::mem::take(&mut self.tuning.atc.table);
        self.tuning.atc = Tuning::default().atc;
        self.tuning.atc.table = table;
    }

    /// No material is chosen (the library, which is a file of its own, stays).
    pub fn reset_materials(&mut self) {
        self.material.clear();
        self.tuning.material.clear();
        self.lint.material_feed = None;
        self.lint.material_rpm = None;
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
            server_timeout_s: 30,
            server_retries: 4,
            server_mdns: true,
            server_max_answer_mib: 64,
            server_restore_outputs: true,
            server_jog_max_mm: 25.0,
            server_jog_max_feed: 600.0,
            server_jog_confirm_mm: 3.0,
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
                ..LintCfg::default()
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
            profiles: Vec::new(),
            profile: String::new(),
            material: String::new(),
            dock_lines: Vec::new(),
            panel_lines: vec![("attachments.max_mb".into(), "64".into())],
        };
        s.save_profile("Small mill");
        s.spindle_rpm = 21_000.0;
        s.save_profile("Router");
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
    fn the_limits_of_a_jog_on_the_server_are_read_inside_their_ranges_and_written_back() {
        let limits = |s: &Settings| {
            (
                s.server_jog_max_mm,
                s.server_jog_max_feed,
                s.server_jog_confirm_mm,
            )
        };
        assert_eq!(limits(&Settings::default()), (50.0, 1000.0, 10.0));
        let top = "server_jog_max_mm = 500
server_jog_max_feed = 30000
server_jog_confirm_mm = 500
";
        assert_eq!(limits(&Settings::parse(top)), (500.0, 30000.0, 500.0));
        let bottom = "server_jog_max_mm = 0.01
server_jog_max_feed = 1
server_jog_confirm_mm = 0
";
        assert_eq!(limits(&Settings::parse(bottom)), (0.01, 1.0, 0.0));
        // a value out of its range, or not a number, changes nothing
        for bad in [
            "server_jog_max_mm = 0",
            "server_jog_max_mm = 500.1",
            "server_jog_max_mm = -3",
            "server_jog_max_mm = far",
            "server_jog_max_mm = inf",
            "server_jog_max_feed = 0.5",
            "server_jog_max_feed = 30001",
            "server_jog_confirm_mm = -1",
            "server_jog_confirm_mm = 501",
            "server_jog_confirm_mm = nan",
        ] {
            assert_eq!(limits(&Settings::parse(bad)), (50.0, 1000.0, 10.0), "{bad}");
        }
        // what is written is read back
        let s = Settings {
            server_jog_max_mm: 12.5,
            server_jog_max_feed: 450.0,
            server_jog_confirm_mm: 2.0,
            ..Settings::default()
        };
        assert_eq!(limits(&Settings::parse(&s.to_text())), (12.5, 450.0, 2.0));
    }

    #[test]
    fn the_four_limits_of_the_link_are_read_inside_their_ranges_and_written_back() {
        let limits = |s: &Settings| {
            (
                s.server_timeout_s,
                s.server_retries,
                s.server_mdns,
                s.server_max_answer_mib,
            )
        };
        assert_eq!(limits(&Settings::default()), (15, 2, false, 8));
        let top = "server_timeout_s = 120\nserver_retries = 5\nserver_mdns = on\nserver_max_answer_mib = 256\n";
        assert_eq!(limits(&Settings::parse(top)), (120, 5, true, 256));
        let bottom = "server_timeout_s = 1\nserver_retries = 0\nserver_max_answer_mib = 1\n";
        assert_eq!(limits(&Settings::parse(bottom)), (1, 0, false, 1));
        // a value out of its range, or not a number, changes nothing
        for bad in [
            "server_timeout_s = 0",
            "server_timeout_s = 121",
            "server_timeout_s = -5",
            "server_timeout_s = fast",
            "server_retries = 6",
            "server_retries = -1",
            "server_max_answer_mib = 0",
            "server_max_answer_mib = 257",
        ] {
            assert_eq!(limits(&Settings::parse(bad)), (15, 2, false, 8), "{bad}");
        }
        // the earlier good value stays when a later line is bad
        let kept = Settings::parse("server_timeout_s = 40\nserver_timeout_s = 500\n");
        assert_eq!(kept.server_timeout_s, 40);
        // the discovery on the network is off unless it is clearly turned on
        for (value, on) in [
            ("on", true),
            ("YES", true),
            ("true", true),
            ("1", true),
            ("off", false),
            ("no", false),
            ("maybe", false),
            ("", false),
        ] {
            let s = Settings::parse(&format!("server_mdns = {value}\n"));
            assert_eq!(s.server_mdns, on, "{value:?}");
        }
        // written back in the file, next to the other keys of the server
        let s = Settings {
            server_timeout_s: 7,
            server_retries: 3,
            server_mdns: true,
            server_max_answer_mib: 12,
            ..Settings::default()
        };
        let text = s.to_text();
        assert!(text.contains(
            "server_timeout_s = 7\nserver_retries = 3\nserver_mdns = yes\nserver_max_answer_mib = 12\n"
        ));
        assert_eq!(limits(&Settings::parse(&text)), (7, 3, true, 12));
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
    fn the_checks_of_arcs_spindle_reversal_and_zero_feed_have_their_switches_in_the_settings_file()
    {
        let d = Settings::default();
        assert!(d.lint.arcs && d.lint.reversal && d.lint.zero_feed);
        assert_eq!(d.lint.arc_gap_mm, crate::gcode::ARC_GAP_MM);
        let text = d.to_text();
        for line in [
            "lint_arcs = yes",
            "lint_arc_gap = 0.005",
            "lint_reversal = yes",
            "lint_zero_feed = yes",
        ] {
            assert!(text.lines().any(|l| l == line), "{line} is not written");
        }
        let s = Settings::parse(
            "lint_arcs = no\nlint_arc_gap = 0.05\nlint_reversal = off\nlint_zero_feed = no\n",
        );
        assert!(!s.lint.arcs && !s.lint.reversal && !s.lint.zero_feed);
        assert_eq!(s.lint.arc_gap_mm, 0.05);
        // written back and read again
        let back = Settings::parse(&s.to_text());
        assert_eq!(back.lint, s.lint);
        // a gap outside its range or a word that is not a number keeps the default, and the switches are independent
        for bad in ["0", "0.00001", "0.6", "-1", "wide"] {
            let s = Settings::parse(&format!("lint_arc_gap = {bad}\n"));
            assert_eq!(s.lint.arc_gap_mm, 0.005, "{bad}");
        }
        let one = Settings::parse("lint_reversal = no\n");
        assert!(one.lint.arcs && !one.lint.reversal && one.lint.zero_feed);
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
            "lint = no\nlint_max_feed = 100\nprogcheck = no\nkeepout_margin = 9\nkeepout_reject = no\nkeepout = clamp 0 0 -5 20 20 40\n             probe_feed = 50\nprobe_grid = 40\nheightmap_segment = 5\njog_soft_limits = always\n             queue_repeat_max = 7\nqueue_pause = 60\nqueue_confirm_between = no\nqueue_remember = no\nsim_speed = 99\n             log_max_entries = 77\nmacros_confirm = no\nmacros_max_lines = 9\ntool_change_time = 5\nstartline_confirm = no\nstep = 0.1\nserver_token = t\n",
        );
        s.recent = vec!["a.nc".into()];
        s
    }

    #[test]
    fn the_material_in_use_is_kept_in_the_file_and_gives_the_checker_its_ranges() {
        use crate::materials::parse_library;
        let lib = parse_library(
            "material = Oak | feed=300,900 | rpm=9000,18000\nmaterial = Foam | feed=800,3000\n",
        );
        let mut s = Settings::parse("material = oak\n");
        assert_eq!(s.material, "oak");
        // parsing alone does not look in the library
        assert_eq!(s.lint.material_feed, None);
        s.resolve_material(&lib);
        assert_eq!(s.lint.material_feed, Some((300.0, 900.0)));
        assert_eq!(s.lint.material_rpm, Some((9000.0, 18000.0)));
        // the name is what the file keeps, and the ranges are not written
        assert!(s.to_text().contains("material = oak\n"));
        assert!(!s.to_text().contains("9000"));
        assert_eq!(Settings::parse(&s.to_text()).material, "oak");
        // a material with one range clears the other; a name that is not in the library clears both
        s.material = "Foam".to_string();
        s.resolve_material(&lib);
        assert_eq!(
            (s.lint.material_feed.is_some(), s.lint.material_rpm),
            (true, None)
        );
        s.material = "Nothing".to_string();
        s.resolve_material(&lib);
        assert_eq!((s.lint.material_feed, s.lint.material_rpm), (None, None));
        // none is the default, a name with line breaks is cleaned, and the section of the checks puts it back
        assert_eq!(Settings::default().material, "");
        assert_eq!(Settings::parse("material = a|b\n").material, "a b");
        s.material = "Oak".to_string();
        s.reset_checks();
        assert_eq!(s.material, "");
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

    #[test]
    fn the_material_in_use_reaches_the_tuning_for_the_estimate_of_the_tools_and_the_resets_clear_it(
    ) {
        let mut s = Settings::parse("material = Aluminium\n");
        assert_eq!(s.tuning.material, "Aluminium");
        // a name that is not in the library still names the material of the estimate
        s.material = "Brass".to_string();
        s.resolve_material(&[]);
        assert_eq!(s.tuning.material, "Brass");
        s.reset_materials();
        assert_eq!((s.material.as_str(), s.tuning.material.as_str()), ("", ""));
        s.material = "Steel".to_string();
        s.resolve_material(&[]);
        s.reset_checks();
        assert_eq!(s.tuning.material, "");
        // it is not written as a key of its own: the file has `material` only
        let text = Settings::parse("material = Oak\n").to_text();
        assert_eq!(text.matches("material = Oak").count(), 1);
        assert_eq!(Settings::parse(&text), Settings::parse("material = Oak\n"));
    }

    #[test]
    fn the_lines_of_the_limits_of_the_attachments_are_kept_and_give_the_limits() {
        let s = Settings::parse(
            "attachments.max_mb = 16\nviewer.max_pdf_mb = 4\nviewer.max_pdf_mb = 8\nviewer.max_text_mb = no\nviewer.max_preview_mb = 32\n",
        );
        let limits = s.panel_limits();
        assert_eq!(limits.attachments.max_attachment_bytes, 16 << 20);
        assert_eq!(limits.viewer.max_pdf_bytes, 8 << 20);
        assert_eq!(limits.viewer.max_preview_bytes, 32 << 20);
        assert_eq!(Settings::parse(&s.to_text()).panel_lines, s.panel_lines);
        assert_eq!(
            Settings::default().panel_limits(),
            eh3d_attachpanel::PanelLimits::default()
        );
    }

    #[test]
    fn the_lines_of_the_docking_window_are_kept_and_written_back_with_the_rest() {
        let text = "lang = es\ndock.version = 1\ndock.current = 2\ndock.ws.1.name = Prepare\ndock.ws.1.tree = H220(T0(a),T0(b))\ndock.ws.1.tree = H300(T0(a),T0(b))\n";
        let s = Settings::parse(text);
        assert_eq!(s.lang, Lang::Es);
        // the last line of a key wins
        assert_eq!(s.dock_lines.len(), 4);
        assert!(s
            .dock_text()
            .contains("dock.ws.1.tree = H300(T0(a),T0(b))\n"));
        assert!(!s.dock_text().contains("H220"));
        // written with the other keys and read back the same
        let back = Settings::parse(&s.to_text());
        assert_eq!(back.dock_lines, s.dock_lines);
        assert_eq!(back.lang, Lang::Es);
        // the frame hands over its lines: only the ones of the frame are taken
        let mut t = Settings::default();
        t.set_dock(&[
            ("dock.current".to_string(), "0".to_string()),
            ("lang".to_string(), "fr".to_string()),
        ]);
        assert_eq!(
            t.dock_lines,
            vec![("dock.current".to_string(), "0".to_string())]
        );
        // a settings file with no layout writes none
        assert!(!Settings::default().to_text().contains("dock."));
    }
}
