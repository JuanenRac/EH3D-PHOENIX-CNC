//! The three sections of the settings window for the program tools: the checks (the first and the second checker, the keep-out boxes), the
//! probe with the height map and the limits of the jog, and the queue with the simulation, the session log, the macros and the time of a
//! tool change. Every number is offered inside the range the settings file accepts (`ranges`), with the range as its tooltip; the buttons
//! and the file keep the same values, and what the window does not show (the keep-out boxes, the server, the recent files) is left as it is
//! because the window changes a copy of all the settings.

use super::dialogs::Draft;
use super::*;
use crate::jog::Mode as JogMode;
use crate::ranges::range_of;
use crate::words::Msg;

/// The number of the first of the three sections in the list of the settings window.
pub(super) const FIRST_SECTION: usize = 6;

impl App {
    /// The names of the three sections, for the list on the left.
    pub(super) fn tool_sections(&self) -> [&'static str; 3] {
        [
            self.w(Msg::SetSecChecks),
            self.w(Msg::SetSecProbe),
            self.w(Msg::SetSecQueue),
        ]
    }

    /// The machine profiles at the top of the General section: the ones saved (a click takes the values of one), the name of the one in use,
    /// which can be written to save the values as a new profile, and the button that takes it away. Only the copy of the settings that the
    /// window changes is touched: Ok keeps it and Cancel drops it.
    pub(super) fn set_profiles(&self, ui: &mut egui::Ui, d: &mut Draft) {
        let dim = col(self.theme.dim);
        ui.label(RichText::new(self.w(Msg::SetProfiles).to_uppercase()).color(dim));
        if d.s.profiles.is_empty() {
            ui.label(RichText::new(self.w(Msg::SetProfileNone)).color(dim));
        }
        let names: Vec<String> = d.s.profiles.iter().map(|p| p.name.clone()).collect();
        ui.horizontal_wrapped(|ui| {
            for n in &names {
                if ui.selectable_label(d.s.profile == *n, n.as_str()).clicked() {
                    d.s.use_profile(n);
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label(self.w(Msg::SetProfileName));
            ui.add(egui::TextEdit::singleline(&mut d.s.profile).desired_width(160.0));
            if ui.button(self.w(Msg::SetProfileSave)).clicked() {
                let name = d.s.profile.clone();
                d.s.save_profile(&name);
            }
            let there = d.s.profiles.iter().any(|p| p.name == d.s.profile);
            if ui
                .add_enabled(there, egui::Button::new(self.w(Msg::SetProfileDelete)))
                .clicked()
            {
                let name = d.s.profile.clone();
                d.s.delete_profile(&name);
            }
        });
        ui.label(
            RichText::new(self.w(Msg::SetProfileHelp))
                .color(dim)
                .small(),
        );
        ui.separator();
        ui.add_space(4.0);
    }

    /// A row of the grid: the name, a number with its unit inside the range of its key, and the range as the tooltip.
    #[allow(clippy::too_many_arguments)]
    fn set_row(
        &self,
        ui: &mut egui::Ui,
        id: &'static str,
        label: Msg,
        key: &'static str,
        value: &mut f64,
        step: f64,
        unit: &str,
    ) {
        let (lo, hi) = range_of(key);
        let tip = self.ws(Msg::SetRange, &[numfield::show(lo), numfield::show(hi)]);
        ui.label(self.w(label));
        ui.horizontal(|ui| {
            numfield::field(ui, ("set-tool", id), value, (lo, hi), step, unit);
        })
        .response
        .on_hover_text(tip);
        ui.end_row();
    }

    /// The same for a whole number.
    #[allow(clippy::too_many_arguments)]
    fn set_count(
        &self,
        ui: &mut egui::Ui,
        id: &'static str,
        label: Msg,
        key: &'static str,
        value: &mut usize,
        step: f64,
    ) {
        let mut v = *value as f64;
        self.set_row(ui, id, label, key, &mut v, step, "");
        *value = v.round() as usize;
    }

    /// The section of the settings window that `section` (6 to 8) names.
    pub(super) fn settings_tools(&self, ui: &mut egui::Ui, d: &mut Draft) {
        match d.section {
            FIRST_SECTION => self.set_checks(ui, d),
            s if s == FIRST_SECTION + 1 => self.set_probe(ui, d),
            _ => self.set_queue(ui, d),
        }
        ui.add_space(8.0);
        if ui.button(self.w(Msg::SetReset)).clicked() {
            match d.section {
                FIRST_SECTION => d.s.reset_checks(),
                s if s == FIRST_SECTION + 1 => d.s.reset_probe(),
                _ => d.s.reset_queue(),
            }
        }
    }

    fn set_checks(&self, ui: &mut egui::Ui, d: &mut Draft) {
        ui.checkbox(&mut d.s.lint.enabled, self.w(Msg::SetLint));
        ui.checkbox(&mut d.s.tuning.progcheck, self.w(Msg::SetProgcheck));
        ui.checkbox(&mut d.s.tuning.keepout_reject, self.w(Msg::SetKeepReject));
        ui.add_space(6.0);
        egui::Grid::new("set-checks")
            .num_columns(2)
            .spacing([10.0, 6.0])
            .show(ui, |ui| {
                self.set_row(
                    ui,
                    "rapid-floor",
                    Msg::SetRapidFloor,
                    "lint_rapid_floor",
                    &mut d.s.lint.rapid_floor_mm,
                    0.5,
                    "mm",
                );
                self.set_row(
                    ui,
                    "max-feed",
                    Msg::SetMaxFeed,
                    "lint_max_feed",
                    &mut d.s.lint.max_feed_mm_min,
                    100.0,
                    "mm/min",
                );
                self.set_row(
                    ui,
                    "max-rpm",
                    Msg::SetMaxRpm,
                    "lint_max_rpm",
                    &mut d.s.lint.max_rpm,
                    500.0,
                    "rpm",
                );
                self.set_row(
                    ui,
                    "keep-margin",
                    Msg::SetKeepMargin,
                    "keepout_margin",
                    &mut d.s.tuning.keepout_margin,
                    0.5,
                    "mm",
                );
            });
    }

    fn set_probe(&self, ui: &mut egui::Ui, d: &mut Draft) {
        egui::Grid::new("set-probe")
            .num_columns(2)
            .spacing([10.0, 6.0])
            .show(ui, |ui| {
                self.set_row(
                    ui,
                    "probe-feed",
                    Msg::SetProbeFeed,
                    "probe_feed",
                    &mut d.s.probe.feed_mm_min,
                    5.0,
                    "mm/min",
                );
                self.set_row(
                    ui,
                    "probe-depth",
                    Msg::SetProbeDepth,
                    "probe_depth",
                    &mut d.s.probe.depth_mm,
                    1.0,
                    "mm",
                );
                self.set_row(
                    ui,
                    "probe-retract",
                    Msg::SetProbeRetract,
                    "probe_retract",
                    &mut d.s.probe.retract_mm,
                    0.5,
                    "mm",
                );
                self.set_row(
                    ui,
                    "probe-plate",
                    Msg::SetProbePlate,
                    "probe_plate",
                    &mut d.s.probe.plate_mm,
                    0.1,
                    "mm",
                );
                self.set_row(
                    ui,
                    "probe-clear",
                    Msg::SetProbeClear,
                    "probe_clearance",
                    &mut d.s.probe.clearance_mm,
                    0.5,
                    "mm",
                );
                self.set_row(
                    ui,
                    "probe-grid",
                    Msg::SetProbeGrid,
                    "probe_grid",
                    &mut d.s.probe_grid_mm,
                    1.0,
                    "mm",
                );
                self.set_row(
                    ui,
                    "height-seg",
                    Msg::SetHeightSeg,
                    "heightmap_segment",
                    &mut d.s.heightmap_segment_mm,
                    0.25,
                    "mm",
                );
            });
        ui.add_space(8.0);
        ui.label(self.w(Msg::SetJogLimits));
        ui.horizontal(|ui| {
            for (mode, name) in [
                (JogMode::Always, Msg::SetJogAlways),
                (JogMode::AfterHoming, Msg::SetJogHoming),
                (JogMode::Off, Msg::SetJogNever),
            ] {
                ui.selectable_value(&mut d.s.jog_soft_limits, mode, self.w(name));
            }
        });
    }

    fn set_queue(&self, ui: &mut egui::Ui, d: &mut Draft) {
        ui.checkbox(
            &mut d.s.tuning.confirm_between,
            self.w(Msg::SetQueueConfirm),
        );
        ui.checkbox(
            &mut d.s.tuning.queue_remember,
            self.w(Msg::SetQueueRemember),
        );
        ui.checkbox(
            &mut d.s.tuning.macros_confirm,
            self.w(Msg::SetMacrosConfirm),
        );
        ui.add_space(6.0);
        egui::Grid::new("set-queue")
            .num_columns(2)
            .spacing([10.0, 6.0])
            .show(ui, |ui| {
                let mut repeats = d.s.tuning.repeat_max as usize;
                self.set_count(
                    ui,
                    "repeat-max",
                    Msg::SetQueueMax,
                    "queue_repeat_max",
                    &mut repeats,
                    1.0,
                );
                d.s.tuning.repeat_max = repeats as u32;
                self.set_row(
                    ui,
                    "queue-pause",
                    Msg::SetQueuePause,
                    "queue_pause",
                    &mut d.s.tuning.pause_s,
                    5.0,
                    "s",
                );
                self.set_row(
                    ui,
                    "sim-speed",
                    Msg::SetSimSpeed,
                    "sim_speed",
                    &mut d.s.tuning.sim_speed,
                    1.0,
                    "x",
                );
                self.set_count(
                    ui,
                    "log-max",
                    Msg::SetLogMax,
                    "log_max_entries",
                    &mut d.s.tuning.log_max,
                    100.0,
                );
                self.set_count(
                    ui,
                    "macro-lines",
                    Msg::SetMacrosLines,
                    "macros_max_lines",
                    &mut d.s.tuning.macro_max_lines,
                    10.0,
                );
                self.set_row(
                    ui,
                    "tool-change",
                    Msg::SetToolChange,
                    "tool_change_time",
                    &mut d.s.tuning.tool_change_s,
                    5.0,
                    "s",
                );
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_sections_have_a_name_in_every_language() {
        let mut a = App::isolated();
        let mut seen = std::collections::HashSet::new();
        for lang in Lang::ALL {
            a.lang = lang;
            let names = a.tool_sections();
            assert!(names.iter().all(|n| !n.is_empty()), "{lang:?}");
            if lang == Lang::En {
                assert_eq!(
                    names,
                    ["Checks", "Probe and height map", "Queue, macros and log"]
                );
            }
            seen.insert(names[0]);
        }
        // a language that is not English has its own words
        assert!(seen.len() > 3);
    }

    #[test]
    fn the_settings_window_opens_on_each_section_and_draws_it() {
        let ctx = egui::Context::default();
        for section in FIRST_SECTION..FIRST_SECTION + 3 {
            let a = App::isolated();
            let mut d = Draft {
                s: a.settings.clone(),
                section,
            };
            ctx.run_pass(egui::RawInput::default(), |ctx| {
                egui::Window::new("tools").show(ctx, |ui| a.settings_tools(ui, &mut d));
            })
            .textures_delta
            .clear();
            // drawing a section changes nothing by itself
            assert_eq!(d.s, a.settings, "section {section}");
        }
    }

    #[test]
    fn the_profiles_are_drawn_at_the_top_of_the_general_section_and_drawing_changes_nothing() {
        let ctx = egui::Context::default();
        for with_profiles in [false, true] {
            let mut a = App::isolated();
            if with_profiles {
                a.settings.save_profile("Small mill");
                a.settings.travel[0] = (0.0, 900.0);
                a.settings.save_profile("Router");
            }
            let mut d = Draft {
                s: a.settings.clone(),
                section: 0,
            };
            ctx.run_pass(egui::RawInput::default(), |ctx| {
                egui::Window::new("profiles").show(ctx, |ui| a.set_profiles(ui, &mut d));
            })
            .textures_delta
            .clear();
            assert_eq!(d.s, a.settings, "{with_profiles}");
        }
    }

    #[test]
    fn a_profile_chosen_in_the_window_reaches_the_settings_only_with_ok() {
        let mut a = App::isolated();
        a.settings.save_profile("Small mill");
        a.settings.travel[0] = (0.0, 900.0);
        a.settings.save_profile("Router");
        let mut d = Draft {
            s: a.settings.clone(),
            section: 0,
        };
        // the click on a name takes the values into the copy of the window
        assert!(d.s.use_profile("Small mill"));
        assert_eq!(d.s.travel[0], (0.0, 500.0));
        assert_eq!(
            a.settings.travel[0],
            (0.0, 900.0),
            "Cancel leaves the settings as they were"
        );
        // Ok: the settings take the copy, the machine gets the travel, and the file keeps both
        a.win = Some(Win::Settings(Box::new(d)));
        let Some(Win::Settings(d)) = a.win.take() else {
            panic!("no settings window");
        };
        a.settings = d.s;
        a.apply_settings();
        a.save_settings();
        let back = Settings::load(&a.settings_path);
        assert_eq!(back.profile, "Small mill");
        assert_eq!(back.profiles.len(), 2);
        assert_eq!(back.travel[0], (0.0, 500.0));
        let _ = std::fs::remove_file(&a.settings_path);
    }

    #[test]
    fn ok_keeps_every_value_of_the_settings_including_the_ones_the_window_does_not_show() {
        let mut a = App::isolated();
        a.settings
            .tuning
            .keepouts
            .push(crate::boxes::KeepOut::parse("clamp 0 0 -5 20 20 40").expect("a box"));
        a.settings.server_token = "token".into();
        a.settings.recent = vec!["a.nc".into()];
        let mut d = Draft {
            s: a.settings.clone(),
            section: FIRST_SECTION + 2,
        };
        d.s.tuning.repeat_max = 7;
        d.s.tuning.tool_change_s = 12.0;
        d.s.probe.feed_mm_min = 33.0;
        a.win = Some(Win::Settings(Box::new(d)));
        // Ok through the same path as the button
        let Some(Win::Settings(d)) = a.win.take() else {
            panic!("no settings window");
        };
        a.settings = d.s;
        a.apply_settings();
        a.save_settings();
        let back = Settings::load(&a.settings_path);
        assert_eq!(back.tuning.repeat_max, 7);
        assert_eq!(back.tuning.tool_change_s, 12.0);
        assert_eq!(back.probe.feed_mm_min, 33.0);
        assert_eq!(back.tuning.keepouts.len(), 1);
        assert_eq!(back.server_token, "token");
        assert_eq!(back.recent, ["a.nc"]);
        let _ = std::fs::remove_file(&a.settings_path);
    }
}
