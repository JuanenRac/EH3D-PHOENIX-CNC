//! The statistics of the program and its simulation in the window: the numbers of `stats`, a slider over the time of the path with the line
//! the tool is at (the cursor of the program panel follows it, and the tool is drawn on the path), a line number to jump to and a play
//! button at the speed of the settings (`sim_speed` times real time). The time left of the program panel reads the same cached
//! estimate. Nothing here moves a machine: it plays the path on paper.

use super::*;
use crate::sim::{hms, Rates};
use crate::stats::measure_accel;
use crate::words::Msg;

/// What the open window shows: the lines of the statistics (made again when the program changes) and the moment of the simulation.
pub(super) struct SimState {
    pub program_len: usize,
    pub lines: Vec<String>,
    pub t: f64,
    pub playing: bool,
}

/// The moment after `dt` seconds of real time at `speed` times real time, and whether the simulation goes on (it stops at the end).
/// A step that is not a number or is negative changes nothing.
pub(super) fn advanced(t: f64, dt: f64, speed: f64, total: f64) -> (f64, bool) {
    let step = if dt.is_finite() && dt > 0.0 && speed.is_finite() && speed > 0.0 {
        dt * speed
    } else {
        0.0
    };
    let next = (t + step).clamp(0.0, total.max(0.0));
    (next, next < total)
}

impl App {
    /// Reads the path of the program again, with the length of its arcs and its tool changes.
    pub(super) fn refresh_path(&mut self) {
        self.path_toolpath = gcode::toolpath(&self.session.program);
        self.path_extras = crate::sim::Extras::of(&self.session.program)
            .with_accel(self.settings.tuning.accel_mm_s2);
        self.viewer
            .refresh(&self.session.program, &self.path_toolpath);
    }

    pub(super) fn sim_rates(&self) -> Rates {
        Rates {
            rapid: self.settings.rapid_mm_min,
            default_feed: self.settings.feed_mm_min,
        }
    }

    /// The seconds a tool change adds to the time: the changer's when it is on, `tool_change_time` when not.
    pub(super) fn change_seconds(&self) -> f64 {
        self.settings
            .tuning
            .change_seconds(self.settings.rapid_mm_min)
    }

    /// The statistics of the program that is loaded, at the start of the simulation.
    pub(super) fn sim_state(&self) -> SimState {
        SimState {
            program_len: self.session.program.len(),
            lines: {
                let stats = measure_accel(
                    &self.session.program,
                    self.sim_rates(),
                    self.change_seconds(),
                    self.settings.tuning.accel_mm_s2,
                );
                let mut lines = stats.lines(self.lang.code());
                lines.extend(stats.layer_lines(self.lang.code()));
                lines.extend(crate::atc::report_lines(
                    &self.session.program,
                    &self.settings.tuning.atc,
                    self.settings.rapid_mm_min,
                    self.lang.code(),
                ));
                lines.extend(crate::tools::report_lines(
                    &self.session.program,
                    &self.settings.tuning.tools,
                    self.lang.code(),
                ));
                lines.extend(crate::tools::wear_lines(
                    &stats,
                    &self.settings.tuning.tools,
                    self.lang.code(),
                ));
                lines
            },
            t: 0.0,
            playing: false,
        }
    }

    /// Puts the simulation at the moment `t`: the tool on the path and the cursor on the line, which the program panel shows when it changes.
    pub(super) fn sim_seek(&mut self, t: f64) {
        let rates = self.sim_rates();
        let (line, pos) = {
            let tl = self.estimate.timeline_with(
                &self.path_toolpath,
                rates,
                &self.path_extras,
                self.change_seconds(),
            );
            (tl.line_at(t), tl.position_at(t))
        };
        self.sim_pos = pos;
        if line != self.sim_line {
            self.sim_line = line;
            if let Some(l) = line {
                self.show_line(l);
            }
        }
    }

    /// Takes the tool and the cursor of the simulation away.
    pub(super) fn sim_end(&mut self) {
        self.sim_pos = None;
        self.sim_line = None;
    }

    /// Takes the simulation away when its window is not open any more (a key can close the window without asking it).
    pub(super) fn sim_guard(&mut self) {
        if self.sim_pos.is_some() && !matches!(self.win, Some(Win::Sim(_))) {
            self.sim_end();
        }
    }

    /// The time left in the program that is loaded, from the line it has reached, and the whole time of the path.
    pub(super) fn time_left_text(&mut self) -> Option<String> {
        if self.path_toolpath.is_empty() {
            return None;
        }
        let rates = self.sim_rates();
        let line = self.program_line();
        let (left, total) = {
            let tl = self.estimate.timeline_with(
                &self.path_toolpath,
                rates,
                &self.path_extras,
                self.change_seconds(),
            );
            let done = tl.begin_of_line(line);
            (tl.remaining(done), tl.total_s)
        };
        Some(self.ws(Msg::TimeLeft, &[hms(left), hms(total)]))
    }

    pub(super) fn sim_window(&mut self, ctx: &egui::Context, mut st: Box<SimState>) -> Option<Win> {
        if st.program_len != self.session.program.len() {
            st = Box::new(self.sim_state());
        }
        let rates = self.sim_rates();
        let total = self
            .estimate
            .timeline_with(
                &self.path_toolpath,
                rates,
                &self.path_extras,
                self.change_seconds(),
            )
            .total_s;
        let speed = self.settings.tuning.sim_speed;
        if st.playing {
            let dt = f64::from(ctx.input(|i| i.stable_dt));
            let (t, still) = advanced(st.t, dt, speed, total);
            st.t = t;
            st.playing = still;
            ctx.request_repaint();
        }
        st.t = st.t.clamp(0.0, total.max(0.0));
        self.sim_seek(st.t);
        let line_now = self.sim_line.map_or(1, |l| l + 1);
        let max_line = self.session.program.len().max(1);
        let mut new_t: Option<f64> = None;
        let mut to_line: Option<usize> = None;
        let mut toggle = false;
        let mut close = false;
        let mut new_speed = speed;
        self.window(self.w(Msg::StatsWindow))
            .default_width(520.0)
            .show(ctx, |ui| {
                for l in &st.lines {
                    ui.label(RichText::new(l).monospace());
                }
                ui.separator();
                if total <= 0.0 {
                    ui.label(self.w(Msg::SimNoPath));
                } else {
                    let mut t = st.t;
                    if ui
                        .add(egui::Slider::new(&mut t, 0.0..=total).show_value(false))
                        .changed()
                    {
                        new_t = Some(t);
                    }
                    ui.label(self.ws(Msg::SimTime, &[hms(st.t), hms(total), hms(total - st.t)]));
                    ui.horizontal(|ui| {
                        ui.label(self.w(Msg::SimLineLabel));
                        let mut line = line_now as f64;
                        if numfield::field(
                            ui,
                            "sim-line",
                            &mut line,
                            (1.0, max_line as f64),
                            1.0,
                            "",
                        ) {
                            to_line = Some((line as usize).saturating_sub(1));
                        }
                        let label = if st.playing {
                            self.w(Msg::SimPause)
                        } else {
                            self.w(Msg::SimPlay)
                        };
                        if ui.button(label).clicked() {
                            toggle = true;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label(self.w(Msg::SimSpeed));
                        numfield::field(
                            ui,
                            "sim-speed",
                            &mut new_speed,
                            (0.1, 10_000.0),
                            10.0,
                            "x",
                        );
                    });
                }
                ui.separator();
                if ui.button(self.t(Text::Close)).clicked() {
                    close = true;
                }
            });
        if toggle {
            st.playing = !st.playing && total > 0.0;
            if st.playing && st.t >= total {
                st.t = 0.0;
            }
        }
        if let Some(t) = new_t {
            st.t = t;
        }
        if let Some(l) = to_line {
            let at = self
                .estimate
                .timeline_with(
                    &self.path_toolpath,
                    rates,
                    &self.path_extras,
                    self.change_seconds(),
                )
                .time_of_line(l);
            if let Some(t) = at {
                st.t = t;
            }
        }
        if (new_speed - speed).abs() > 1.0e-9 {
            self.settings.tuning.sim_speed = new_speed.clamp(0.1, 10_000.0);
            self.save_settings();
        }
        if close {
            self.sim_end();
            None
        } else {
            Some(Win::Sim(st))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_with_a_program() -> App {
        let mut a = App::isolated();
        // two moves of 6 seconds each at 600 mm/min
        a.session.load("G21 G90\nG1 X60 F600\nG1 Y60");
        a.path_toolpath = gcode::toolpath(&a.session.program);
        a
    }

    #[test]
    fn the_simulation_advances_by_the_real_time_times_the_speed_and_stops_at_the_end() {
        assert_eq!(advanced(0.0, 0.5, 10.0, 12.0), (5.0, true));
        assert_eq!(advanced(11.0, 0.5, 10.0, 12.0), (12.0, false));
        // nothing that is not a positive step moves it
        assert_eq!(advanced(3.0, -1.0, 10.0, 12.0), (3.0, true));
        assert_eq!(advanced(3.0, f64::NAN, 10.0, 12.0), (3.0, true));
        assert_eq!(advanced(3.0, 1.0, 0.0, 12.0), (3.0, true));
        // a program with no time does not play
        assert_eq!(advanced(0.0, 1.0, 10.0, 0.0), (0.0, false));
    }

    #[test]
    fn the_cursor_and_the_tool_follow_the_moment() {
        let mut a = app_with_a_program();
        a.sim_seek(0.0);
        assert_eq!(a.sim_line, Some(1));
        a.sim_seek(9.0);
        assert_eq!(a.sim_line, Some(2));
        let p = a.sim_pos.unwrap();
        assert!(
            (p.0 - 60.0).abs() < 1e-9 && (p.1 - 30.0).abs() < 1e-9,
            "{p:?}"
        );
        // the program panel is told to show the line
        assert_eq!((a.focus_line, a.focus_scroll), (Some(2), true));
        a.sim_end();
        assert_eq!((a.sim_pos, a.sim_line), (None, None));
    }

    #[test]
    fn the_simulation_goes_away_when_its_window_is_not_open() {
        let mut a = app_with_a_program();
        a.open_window("stats");
        assert!(matches!(a.win, Some(Win::Sim(_))));
        a.sim_seek(3.0);
        a.sim_guard();
        assert!(a.sim_pos.is_some());
        // a key closed the window
        a.win = None;
        a.sim_guard();
        assert_eq!((a.sim_pos, a.sim_line), (None, None));
    }

    #[test]
    fn the_statistics_follow_the_program_and_the_language() {
        let mut a = app_with_a_program();
        let st = a.sim_state();
        assert_eq!(st.program_len, 3);
        assert!(st.lines[0].contains("Program statistics"), "{:?}", st.lines);
        a.settings.lang = Lang::Es;
        a.apply_settings();
        assert!(a.sim_state().lines[0].contains("Estadísticas"));
    }

    #[test]
    fn the_time_left_is_measured_once_and_shrinks_as_the_program_goes_on() {
        let mut a = app_with_a_program();
        assert_eq!(
            a.time_left_text().unwrap(),
            "Time left 12 s of 12 s".to_string()
        );
        a.session.next = 2;
        assert_eq!(
            a.time_left_text().unwrap(),
            "Time left 6 s of 12 s".to_string()
        );
        // the path was measured once for the two calls
        assert_eq!(a.estimate.misses, 1);
        assert_eq!(a.estimate.hits, 1);
        a.session.next = 3;
        assert_eq!(
            a.time_left_text().unwrap(),
            "Time left 0 s of 12 s".to_string()
        );
        // no program, no time
        a.path_toolpath.clear();
        assert_eq!(a.time_left_text(), None);
    }

    #[test]
    fn the_time_left_counts_the_tool_changes_and_the_curve_of_the_arcs() {
        let mut a = App::isolated();
        a.session
            .load("G21 G90\nT1 M6\nG1 X60 F600\nG3 X60 Y10 I0 J5");
        a.refresh_path();
        a.settings.tuning.tool_change_s = 30.0;
        // 30 s of change, 6 s of cut and the half circle (1.57 s) instead of its chord (1 s)
        assert_eq!(
            a.time_left_text().unwrap(),
            "Time left 38 s of 38 s".to_string()
        );
        // the change is done when the program is past its line
        a.session.next = 2;
        assert_eq!(
            a.time_left_text().unwrap(),
            "Time left 8 s of 38 s".to_string()
        );
        // a shorter change is another estimate
        a.settings.tuning.tool_change_s = 0.0;
        assert_eq!(
            a.time_left_text().unwrap(),
            "Time left 8 s of 8 s".to_string()
        );
    }
}
