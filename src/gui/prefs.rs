//! The sections of the settings window for the program tools: the checks (the first and the second checker, the keep-out boxes), the
//! probe with the height map and the limits of the jog, the queue with the simulation, the session log, the macros and the time of a
//! tool change, the tool changer (its rack, lock and blow, and the table of slots of its file) and the materials (the one chosen, its
//! ranges, the file of the library, a button that writes the example file and a small editor of the table). Every number is offered inside
//! the range the settings file accepts (`ranges`, `atc`), with the range as its tooltip; the buttons and the file keep the same values, and
//! what the window does not show (the keep-out boxes, the server, the recent files) is left as it is because the window changes a copy of
//! all the settings. The files of the tool changer and of the library are written only by their own buttons, never over a file that is
//! there unless the operator says so.

use super::dialogs::Draft;
use super::*;
use crate::jog::Mode as JogMode;
use crate::materials::{self, Material, RowError, SaveError};
use crate::ranges::range_of;
use crate::words::Msg;

/// The number of the first of the sections for the program tools in the list of the settings window, how many there are and the last one.
pub(super) const FIRST_SECTION: usize = 6;
pub(super) const TOOL_SECTIONS: usize = 5;
pub(super) const LAST_SECTION: usize = FIRST_SECTION + TOOL_SECTIONS - 1;

/// What a button of the tool changer or of the materials asked for; it is done once the window is drawn, because it needs the files and the
/// library in use.
pub(super) enum MatAct {
    /// Write the example table of slots.
    CreateTable,
    /// Write the example library of materials.
    CreateLibrary,
    /// Open the editor on the library in use.
    Edit,
    /// Close the editor (what was not saved is dropped).
    Close,
    /// Write the library of the editor; `true` when the operator said that a file that is there may be replaced.
    Save(bool),
}

/// The editor of the library of materials and the last thing a button said, inside the settings window.
#[derive(Default)]
pub(super) struct MatEdit {
    /// The materials being edited; nothing while the editor is closed.
    pub rows: Option<Vec<Material>>,
    /// The file is there and the operator is being asked whether to replace it.
    pub ask_replace: bool,
    /// What the last button did, as a message and its arguments.
    pub said: Option<(Msg, Vec<String>)>,
    /// What a click asked for in this frame.
    pub act: Option<MatAct>,
}

/// The words for what is wrong with a row of the editor.
fn row_error_msg(e: RowError) -> Msg {
    match e {
        RowError::Name => Msg::MatErrName,
        RowError::NoRange => Msg::MatErrNoRange,
        RowError::Range => Msg::MatErrRange,
        RowError::Repeated => Msg::MatErrRepeated,
        RowError::TooMany => Msg::MatErrTooMany,
    }
}

impl App {
    /// The names of the sections for the program tools, for the list on the left.
    pub(super) fn tool_sections(&self) -> [&'static str; TOOL_SECTIONS] {
        [
            self.w(Msg::SetSecChecks),
            self.w(Msg::SetSecProbe),
            self.w(Msg::SetSecQueue),
            self.w(Msg::SetSecAtc),
            self.w(Msg::SetSecMaterials),
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

    /// The section of the settings window that `section` (`FIRST_SECTION` to `LAST_SECTION`) names.
    pub(super) fn settings_tools(&self, ui: &mut egui::Ui, d: &mut Draft) {
        match d.section {
            FIRST_SECTION => self.set_checks(ui, d),
            s if s == FIRST_SECTION + 1 => self.set_probe(ui, d),
            s if s == FIRST_SECTION + 3 => self.set_atc(ui, d),
            s if s == FIRST_SECTION + 4 => self.set_materials(ui, d),
            _ => self.set_queue(ui, d),
        }
        ui.add_space(8.0);
        if ui.button(self.w(Msg::SetReset)).clicked() {
            match d.section {
                FIRST_SECTION => d.s.reset_checks(),
                s if s == FIRST_SECTION + 1 => d.s.reset_probe(),
                s if s == FIRST_SECTION + 3 => d.s.reset_atc(),
                s if s == FIRST_SECTION + 4 => d.s.reset_materials(),
                _ => d.s.reset_queue(),
            }
        }
    }

    /// The list of materials with the one in use chosen (shared by the checks section and the materials one).
    fn material_picker(&self, ui: &mut egui::Ui, d: &mut Draft) {
        ui.horizontal(|ui| {
            ui.label(self.w(Msg::SetMaterial));
            let shown = if d.s.material.is_empty() {
                self.w(Msg::MatNone).to_string()
            } else {
                d.s.material.clone()
            };
            egui::ComboBox::from_id_salt("material")
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut d.s.material, String::new(), self.w(Msg::MatNone));
                    for m in &self.materials {
                        ui.selectable_value(&mut d.s.material, m.name.clone(), m.name.as_str());
                    }
                });
        });
    }

    /// What the last button of the tool changer or of the materials said, if anything.
    fn said_line(&self, ui: &mut egui::Ui, d: &Draft) {
        if let Some((m, args)) = &d.mats.said {
            ui.label(RichText::new(self.ws(*m, args)).color(col(self.theme.dim)));
        }
    }

    /// A row of the grid of the tool changer: the name, a number with its unit inside its range, and the range as the tooltip.
    #[allow(clippy::too_many_arguments)]
    fn atc_row(
        &self,
        ui: &mut egui::Ui,
        id: &'static str,
        label: Msg,
        value: &mut f64,
        range: (f64, f64),
        step: f64,
        unit: &str,
    ) {
        let tip = self.ws(
            Msg::SetRange,
            &[numfield::show(range.0), numfield::show(range.1)],
        );
        ui.label(self.w(label));
        ui.horizontal(|ui| {
            numfield::field(ui, ("set-atc", id), value, range, step, unit);
        })
        .response
        .on_hover_text(tip);
        ui.end_row();
    }

    /// The tool changer: whether it is worked out, its rack, lock and blow, and the table of slots that its file gives.
    fn set_atc(&self, ui: &mut egui::Ui, d: &mut Draft) {
        let dim = col(self.theme.dim);
        ui.checkbox(&mut d.s.tuning.atc.enabled, self.w(Msg::AtcOn));
        ui.add_space(6.0);
        let (coord, pin, secs) = (
            crate::atc::COORD_RANGE,
            crate::atc::PIN_RANGE,
            crate::atc::SECONDS_RANGE,
        );
        egui::Grid::new("set-atc")
            .num_columns(2)
            .spacing([10.0, 6.0])
            .show(ui, |ui| {
                let a = &mut d.s.tuning.atc;
                self.atc_row(ui, "safe", Msg::AtcSafe, &mut a.safe_z, coord, 1.0, "mm");
                self.atc_row(
                    ui,
                    "rack-x",
                    Msg::AtcRackX,
                    &mut a.rack[0],
                    coord,
                    1.0,
                    "mm",
                );
                self.atc_row(
                    ui,
                    "rack-y",
                    Msg::AtcRackY,
                    &mut a.rack[1],
                    coord,
                    1.0,
                    "mm",
                );
                self.atc_row(
                    ui,
                    "rack-z",
                    Msg::AtcRackZ,
                    &mut a.rack[2],
                    coord,
                    1.0,
                    "mm",
                );
                self.atc_row(
                    ui,
                    "pitch-x",
                    Msg::AtcPitchX,
                    &mut a.pitch[0],
                    coord,
                    1.0,
                    "mm",
                );
                self.atc_row(
                    ui,
                    "pitch-y",
                    Msg::AtcPitchY,
                    &mut a.pitch[1],
                    coord,
                    1.0,
                    "mm",
                );
                let mut lock_pin = f64::from(a.lock_pin);
                self.atc_row(ui, "lock-pin", Msg::AtcLockPin, &mut lock_pin, pin, 1.0, "");
                a.lock_pin = lock_pin.round() as u32;
                self.atc_row(
                    ui,
                    "lock-wait",
                    Msg::AtcLockWait,
                    &mut a.lock_wait_s,
                    secs,
                    0.1,
                    "s",
                );
                let mut blow_pin = f64::from(a.blow_pin);
                self.atc_row(ui, "blow-pin", Msg::AtcBlowPin, &mut blow_pin, pin, 1.0, "");
                a.blow_pin = blow_pin.round() as u32;
                self.atc_row(
                    ui,
                    "blow-time",
                    Msg::AtcBlowTime,
                    &mut a.blow_s,
                    secs,
                    0.1,
                    "s",
                );
            });
        ui.add_space(8.0);
        let path = crate::atc::table_path(&self.settings_path);
        if d.s.tuning.atc.table.is_empty() {
            let none = self.ws(Msg::AtcTableNone, &[path.display().to_string()]);
            ui.label(RichText::new(none).color(dim).small());
        }
        for line in crate::atc::table_lines(&d.s.tuning.atc.table, self.lang.code()) {
            ui.label(line);
        }
        if ui
            .add_enabled(!path.exists(), egui::Button::new(self.w(Msg::AtcCreate)))
            .clicked()
        {
            d.mats.act = Some(MatAct::CreateTable);
        }
        self.said_line(ui, d);
    }

    /// The materials: the one in use and its ranges, the file of the library with a button that writes the example, and the editor.
    fn set_materials(&self, ui: &mut egui::Ui, d: &mut Draft) {
        let dim = col(self.theme.dim);
        self.material_picker(ui, d);
        if let Some(m) = materials::find(&self.materials, &d.s.material) {
            let show = |r: Option<(f64, f64)>, unit: &str| {
                r.map_or_else(
                    || self.w(Msg::MatNoRange).to_string(),
                    |(lo, hi)| format!("{} - {} {unit}", numfield::show(lo), numfield::show(hi)),
                )
            };
            ui.label(self.ws(Msg::MatFeedLine, &[show(m.feed, "mm/min")]));
            ui.label(self.ws(Msg::MatRpmLine, &[show(m.rpm, "rpm")]));
        }
        ui.add_space(8.0);
        let path = materials::library_path(&self.settings_path);
        let there = path.exists();
        let shown = path.display().to_string();
        let file = if there {
            self.ws(Msg::MatFileThere, &[shown])
        } else {
            self.ws(Msg::MatFileNone, &[shown])
        };
        ui.label(RichText::new(file).color(dim).small());
        let editing = d.mats.rows.is_some();
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(!there, egui::Button::new(self.w(Msg::MatCreate)))
                .clicked()
            {
                d.mats.act = Some(MatAct::CreateLibrary);
            }
            let name = if editing {
                Msg::MatEditClose
            } else {
                Msg::MatEdit
            };
            if ui.button(self.w(name)).clicked() {
                d.mats.act = Some(if editing { MatAct::Close } else { MatAct::Edit });
            }
        });
        self.said_line(ui, d);
        if d.mats.rows.is_some() {
            ui.separator();
            self.material_editor(ui, d);
        }
    }

    /// The table of materials being edited: a row for each material with its name and its two ranges, a button to add one, and Save.
    fn material_editor(&self, ui: &mut egui::Ui, d: &mut Draft) {
        let Some(rows) = d.mats.rows.as_mut() else {
            return;
        };
        let range = (0.0, 1_000_000.0);
        let mut remove: Option<usize> = None;
        for (i, m) in rows.iter_mut().enumerate() {
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("{:>2}", i + 1));
                super::clipmenu::text_box(
                    ui,
                    egui::TextEdit::singleline(&mut m.name).desired_width(140.0),
                );
                let mut has = m.feed.is_some();
                if ui.checkbox(&mut has, self.w(Msg::MatFeed)).changed() {
                    m.feed = has.then_some((100.0, 1000.0));
                }
                if let Some((lo, hi)) = m.feed.as_mut() {
                    numfield::field(ui, ("mat-feed-lo", i), lo, range, 50.0, "");
                    numfield::field(ui, ("mat-feed-hi", i), hi, range, 50.0, "mm/min");
                }
                let mut has = m.rpm.is_some();
                if ui.checkbox(&mut has, self.w(Msg::MatRpm)).changed() {
                    m.rpm = has.then_some((8000.0, 18000.0));
                }
                if let Some((lo, hi)) = m.rpm.as_mut() {
                    numfield::field(ui, ("mat-rpm-lo", i), lo, range, 500.0, "");
                    numfield::field(ui, ("mat-rpm-hi", i), hi, range, 500.0, "rpm");
                }
                if ui.small_button(self.w(Msg::MatRemove)).clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            rows.remove(i);
            d.mats.ask_replace = false;
        }
        let problem = materials::check_library(&rows[..]).err();
        if let Some((i, e)) = problem {
            ui.colored_label(
                col(self.theme.danger),
                self.ws(row_error_msg(e), &[(i + 1).to_string()]),
            );
        }
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    rows.len() < materials::MAX_MATERIALS,
                    egui::Button::new(self.w(Msg::MatAdd)),
                )
                .clicked()
            {
                let name = format!("{} {}", self.w(Msg::SetSecMaterials), rows.len() + 1);
                rows.push(Material {
                    name,
                    feed: Some((100.0, 1000.0)),
                    rpm: None,
                });
            }
            if ui
                .add_enabled(problem.is_none(), egui::Button::new(self.w(Msg::MatSave)))
                .clicked()
            {
                d.mats.act = Some(MatAct::Save(false));
            }
        });
        if d.mats.ask_replace {
            ui.colored_label(col(self.theme.warn), self.w(Msg::MatReplaceAsk));
            ui.horizontal(|ui| {
                if ui.button(self.w(Msg::MatReplace)).clicked() {
                    d.mats.act = Some(MatAct::Save(true));
                }
                if ui.button(self.t(Text::McCancel)).clicked() {
                    d.mats.ask_replace = false;
                }
            });
        }
    }

    /// Does what a button of the tool changer or of the materials asked for in the last frame: writes the example files (never over a file
    /// that is there), opens and closes the editor, and saves the library (asking first when its file is there). The tables are read again
    /// afterwards so that the window shows what is on disk.
    pub(super) fn tool_file_actions(&mut self, d: &mut Draft) {
        let Some(act) = d.mats.act.take() else {
            return;
        };
        let table = crate::atc::table_path(&self.settings_path)
            .display()
            .to_string();
        let library = materials::library_path(&self.settings_path)
            .display()
            .to_string();
        d.mats.said = None;
        match act {
            MatAct::CreateTable => {
                d.mats.said = Some(match crate::atc::create_sample(&self.settings_path) {
                    Ok(()) => (Msg::FileMade, vec![table]),
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                        (Msg::FileExists, vec![table])
                    }
                    Err(e) => (Msg::FileError, vec![e.to_string()]),
                });
                d.s.resolve_atc_table(crate::atc::load_table(&self.settings_path));
            }
            MatAct::CreateLibrary => {
                d.mats.said = Some(match materials::create_sample(&self.settings_path) {
                    Ok(()) => (Msg::FileMade, vec![library]),
                    Err(SaveError::Exists) => (Msg::FileExists, vec![library]),
                    Err(SaveError::Io(e)) => (Msg::FileError, vec![e]),
                    Err(SaveError::Invalid(i, e)) => (row_error_msg(e), vec![(i + 1).to_string()]),
                });
                self.reload_library(d);
            }
            MatAct::Edit => {
                d.mats.rows = Some(self.materials.clone());
                d.mats.ask_replace = false;
            }
            MatAct::Close => {
                d.mats.rows = None;
                d.mats.ask_replace = false;
            }
            MatAct::Save(replace) => {
                let Some(rows) = d.mats.rows.clone() else {
                    return;
                };
                match materials::save(&self.settings_path, &rows, replace) {
                    Ok(()) => {
                        d.mats.ask_replace = false;
                        d.mats.said = Some((Msg::MatSaved, vec![library]));
                        self.reload_library(d);
                    }
                    Err(SaveError::Exists) => d.mats.ask_replace = true,
                    Err(SaveError::Io(e)) => d.mats.said = Some((Msg::FileError, vec![e])),
                    Err(SaveError::Invalid(i, e)) => {
                        d.mats.said = Some((row_error_msg(e), vec![(i + 1).to_string()]))
                    }
                }
            }
        }
    }

    /// Reads the library of materials from its file again, for the list and for the ranges of the material in use.
    fn reload_library(&mut self, d: &mut Draft) {
        self.materials = materials::load(&self.settings_path);
        d.s.resolve_material(&self.materials);
    }

    fn set_checks(&self, ui: &mut egui::Ui, d: &mut Draft) {
        ui.checkbox(&mut d.s.lint.enabled, self.w(Msg::SetLint));
        ui.checkbox(&mut d.s.tuning.progcheck, self.w(Msg::SetProgcheck));
        ui.checkbox(&mut d.s.tuning.keepout_reject, self.w(Msg::SetKeepReject));
        self.material_picker(ui, d);
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
        ui.checkbox(
            &mut d.s.tuning.startline_confirm,
            self.w(Msg::SetStartConfirm),
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
    fn the_sections_have_a_name_in_every_language() {
        let mut a = App::isolated();
        let mut seen = std::collections::HashSet::new();
        for lang in Lang::ALL {
            a.lang = lang;
            let names = a.tool_sections();
            assert!(names.iter().all(|n| !n.is_empty()), "{lang:?}");
            if lang == Lang::En {
                assert_eq!(
                    names,
                    [
                        "Checks",
                        "Probe and height map",
                        "Queue, macros and log",
                        "Tool changer",
                        "Materials"
                    ]
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
        for section in FIRST_SECTION..=LAST_SECTION {
            let a = App::isolated();
            let mut d = Draft {
                s: a.settings.clone(),
                section,
                mats: Default::default(),
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
                mats: Default::default(),
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
            mats: Default::default(),
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
            mats: Default::default(),
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

    fn draft_at(a: &App, section: usize) -> Draft {
        Draft {
            s: a.settings.clone(),
            section,
            mats: Default::default(),
        }
    }

    #[test]
    fn the_tool_changer_table_is_made_once_read_again_and_never_written_over() {
        let mut a = App::isolated();
        let table = crate::atc::table_path(&a.settings_path);
        let _ = std::fs::remove_file(&table);
        let mut d = draft_at(&a, FIRST_SECTION + 3);
        d.mats.act = Some(MatAct::CreateTable);
        a.tool_file_actions(&mut d);
        assert!(table.exists());
        assert!(matches!(d.mats.said, Some((Msg::FileMade, _))));
        // the window shows what is on disk: the six tools of the example
        assert_eq!(d.s.tuning.atc.table.slots.len(), 6);
        assert!(d.mats.act.is_none());
        // the operator changes the file; asking again leaves it alone
        std::fs::write(&table, "tool = 9 | pos=1,1\n").unwrap();
        d.mats.act = Some(MatAct::CreateTable);
        a.tool_file_actions(&mut d);
        assert!(matches!(d.mats.said, Some((Msg::FileExists, _))));
        assert_eq!(
            std::fs::read_to_string(&table).unwrap(),
            "tool = 9 | pos=1,1\n"
        );
        assert_eq!(d.s.tuning.atc.table.slots.len(), 1);
        let _ = std::fs::remove_file(&table);
    }

    #[test]
    fn the_library_of_materials_is_made_edited_and_saved_asking_before_a_file_is_replaced() {
        let mut a = App::isolated();
        let file = materials::library_path(&a.settings_path);
        let _ = std::fs::remove_file(&file);
        let shipped = a.materials.len();
        assert!(shipped >= 8);
        let mut d = draft_at(&a, FIRST_SECTION + 4);
        // the example file is written once
        d.mats.act = Some(MatAct::CreateLibrary);
        a.tool_file_actions(&mut d);
        assert!(file.exists());
        assert!(matches!(d.mats.said, Some((Msg::FileMade, _))));
        let made = std::fs::read_to_string(&file).unwrap();
        d.mats.act = Some(MatAct::CreateLibrary);
        a.tool_file_actions(&mut d);
        assert!(matches!(d.mats.said, Some((Msg::FileExists, _))));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), made);
        // the editor opens on the library in use and closes
        d.mats.act = Some(MatAct::Edit);
        a.tool_file_actions(&mut d);
        assert_eq!(d.mats.rows.as_ref().map(Vec::len), Some(shipped));
        d.mats.act = Some(MatAct::Close);
        a.tool_file_actions(&mut d);
        assert!(d.mats.rows.is_none());
        d.mats.act = Some(MatAct::Edit);
        a.tool_file_actions(&mut d);
        // saving over a file that is there asks first and writes nothing
        d.mats.rows.as_mut().unwrap()[0].name = "Edited".to_string();
        d.mats.act = Some(MatAct::Save(false));
        a.tool_file_actions(&mut d);
        assert!(d.mats.ask_replace);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), made);
        // the answer Replace writes it, and the program reads it back
        d.mats.act = Some(MatAct::Save(true));
        a.tool_file_actions(&mut d);
        assert!(!d.mats.ask_replace);
        assert!(matches!(d.mats.said, Some((Msg::MatSaved, _))));
        assert_eq!(a.materials[0].name, "Edited");
        assert_eq!(a.materials.len(), shipped);
        // a library that cannot be read back is not written and says which row
        d.mats.rows.as_mut().unwrap()[1].feed = None;
        d.mats.rows.as_mut().unwrap()[1].rpm = None;
        let before = std::fs::read_to_string(&file).unwrap();
        d.mats.act = Some(MatAct::Save(true));
        a.tool_file_actions(&mut d);
        assert!(
            matches!(d.mats.said, Some((Msg::MatErrNoRange, ref v)) if v == &["2".to_string()])
        );
        assert_eq!(std::fs::read_to_string(&file).unwrap(), before);
        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn the_new_sections_draw_with_their_editors_open_and_drawing_changes_nothing() {
        let ctx = egui::Context::default();
        for section in [FIRST_SECTION + 3, FIRST_SECTION + 4] {
            let a = App::isolated();
            let mut d = draft_at(&a, section);
            d.s.tuning.atc.table = crate::atc::RackTable::parse("tool = 1 | pos=5,5\ntool = 2\n");
            let mut rows = a.materials.clone();
            rows.push(Material {
                name: String::new(),
                feed: None,
                rpm: None,
            });
            d.mats.rows = Some(rows);
            d.mats.ask_replace = true;
            d.mats.said = Some((Msg::FileMade, vec!["x.txt".to_string()]));
            let before = d.s.clone();
            ctx.run_pass(egui::RawInput::default(), |ctx| {
                egui::Window::new("new").show(ctx, |ui| a.settings_tools(ui, &mut d));
            })
            .textures_delta
            .clear();
            assert_eq!(d.s, before, "section {section}");
            assert!(d.mats.act.is_none(), "section {section}");
        }
    }

    #[test]
    fn the_new_sections_open_by_name_and_reset_to_their_defaults() {
        let mut a = App::isolated();
        a.open_window("settings-atc");
        assert!(matches!(&a.win, Some(Win::Settings(d)) if d.section == FIRST_SECTION + 3));
        a.open_window("settings-materials");
        assert!(matches!(&a.win, Some(Win::Settings(d)) if d.section == FIRST_SECTION + 4));
        let mut s = Settings::default();
        s.tuning.atc.enabled = true;
        s.tuning.atc.table = crate::atc::RackTable::parse("tool = 1 | pos=5,5\n");
        s.material = "Brass".to_string();
        s.reset_atc();
        assert!(!s.tuning.atc.enabled);
        assert_eq!(
            s.tuning.atc.table.slots.len(),
            1,
            "the table is a file of its own"
        );
        s.reset_materials();
        assert!(s.material.is_empty());
    }
}
