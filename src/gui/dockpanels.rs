//! The bodies of the panels of the docking window. The frame (`eh3d-dock`) gives each one the room of its group and calls it only for the tab on
//! show; a body must not assume its size. The things the panels decide are in `workflow` and `panels`, which have no window and are tested
//! there; here are only the widgets.

use super::*;
use crate::notes::Label;
use crate::paneltext::{fill, text, Pt};
use crate::progcheck::Level;
use crate::words::Msg;
use crate::workflow::{self, Imported};

/// The words of the choice of the layer of a board, in the order of `ImportUi::layer`.
const LAYER_WORDS: [Pt; 4] = [
    Pt::ImportLayerAuto,
    Pt::ImportLayerIsolation,
    Pt::ImportLayerProfile,
    Pt::ImportLayerDrill,
];

/// The word of a kind of layer.
fn layer_word(kind: crate::pcb::Kind) -> Pt {
    match kind {
        crate::pcb::Kind::Isolation => Pt::ImportLayerIsolation,
        crate::pcb::Kind::Profile => Pt::ImportLayerProfile,
        crate::pcb::Kind::Drill => Pt::ImportLayerDrill,
    }
}

/// What the panel of the import holds between frames.
#[derive(Default)]
pub(super) struct ImportUi {
    /// The path of the drawing and the name (or path) of the post-processor, as typed.
    pub(super) path: String,
    pub(super) post: String,
    /// The layer a board file is cut as (0: what the file says, 1: isolation, 2: profile, 3: drill) and whether it is mirrored.
    pub(super) layer: usize,
    pub(super) mirror: bool,
    /// What reading the drawing gave: the program that cuts along it, or why it could not be read.
    pub(super) result: Option<Result<Imported, String>>,
}

/// What the panel of the tool setter holds between frames.
pub(super) struct SetterUi {
    /// The number of the tool in the spindle (the line of the table the length goes to).
    pub(super) tool: u32,
    /// The operator said the setter is there and clear; it holds for one measure only.
    pub(super) confirmed: bool,
    /// What happened the last time the length was written (or why it was not).
    pub(super) message: Option<String>,
}

impl Default for SetterUi {
    fn default() -> Self {
        SetterUi {
            tool: 1,
            confirmed: false,
            message: None,
        }
    }
}

impl App {
    /// A word of this part of the window in the language of the window.
    fn pt(&self, w: Pt) -> &'static str {
        text(self.lang.code(), w)
    }

    fn heading(&self, ui: &mut egui::Ui, s: &str) {
        ui.label(RichText::new(s.to_uppercase()).color(col(self.theme.accent)));
    }

    /// The machine is free: nothing runs and nothing is being sent.
    pub(super) fn machine_idle(&self) -> bool {
        !self.session.running && !self.session.busy() && !self.server_busy()
    }

    // ------------------------------------------------------------------------------------------------------------ the attachments

    /// The attachments of the project of the window; what it writes is read again by the window.
    pub(super) fn panel_attachments(&mut self, ui: &mut egui::Ui) {
        let file = self.suite.file.clone();
        if self.attachments.show(ui, file.as_deref(), self.lang.code()) {
            self.suite_refresh();
        }
    }

    // ------------------------------------------------------------------------------------------------------------ the state of the machine

    /// The position, the speeds, the codes in force and the guard of the controller.
    pub(super) fn panel_position(&mut self, ui: &mut egui::Ui) {
        let compact = ui.available_height() < COMPACT_STATE_BELOW;
        egui::ScrollArea::vertical()
            .id_salt("positionscroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                self.position_block(ui, compact);
                ui.add_space(4.0);
                self.speed_rows(ui, compact);
                ui.add_space(4.0);
                self.info_block(ui, compact);
                ui.separator();
                self.guard_row(ui);
            });
    }

    /// Whether the guard watches the controller, and the all clear when it stopped the machine.
    fn guard_row(&mut self, ui: &mut egui::Ui) {
        match (&self.guard, self.guard_trip) {
            (None, _) => {
                ui.label(RichText::new(self.pt(Pt::GuardOff)).color(col(self.theme.dim)));
            }
            (Some(_), None) => {
                ui.label(RichText::new(self.pt(Pt::GuardWatching)).color(col(self.theme.ok)));
            }
            (Some(g), Some(trip)) => {
                ui.label(
                    RichText::new(self.pt(Pt::GuardStopped))
                        .strong()
                        .color(col(self.theme.danger)),
                );
                ui.label(crate::guard::trip_text(trip, self.lang.code()));
                if ui.button(self.pt(Pt::GuardReset)).clicked() {
                    g.reset();
                }
            }
        }
    }

    // ------------------------------------------------------------------------------------------------------------ the hand control

    /// The jog: the step and the speed, the buttons of the axes, and (when the settings show them) the column of zeros and stored positions.
    pub(super) fn panel_jog(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::both()
            .id_salt("jogscroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    if self.settings.show_buttons {
                        ui.vertical(|ui| {
                            ui.set_width(74.0);
                            self.icon_column(ui);
                        });
                        ui.separator();
                    }
                    ui.vertical(|ui| self.jog_block(ui));
                });
            });
    }

    // ------------------------------------------------------------------------------------------------------------ the files and the program

    /// The file that is loaded, the recent ones and the queue.
    pub(super) fn panel_files(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .id_salt("filesscroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                self.heading(ui, self.t(Text::McProgram));
                ui.horizontal(|ui| {
                    let room = (ui.available_width() - 110.0).clamp(120.0, 900.0);
                    clipmenu::text_box(
                        ui,
                        egui::TextEdit::singleline(&mut self.path)
                            .desired_width(room)
                            .hint_text("job.nc"),
                    );
                    if ui.button(self.t(Text::McOpenGcode)).clicked() && !self.path.is_empty() {
                        let p = PathBuf::from(self.path.clone());
                        self.open(&p);
                    }
                });
                let recent: Vec<String> = self
                    .settings
                    .recent
                    .iter()
                    .filter(|p| Path::new(p.as_str()).exists())
                    .cloned()
                    .collect();
                if !recent.is_empty() {
                    ui.add_space(4.0);
                    self.heading(ui, self.t(Text::Recent));
                    for p in &recent {
                        let name = Path::new(p)
                            .file_name()
                            .map_or_else(|| p.clone(), |n| n.to_string_lossy().to_string());
                        if ui.link(name).on_hover_text(p).clicked() {
                            let p = PathBuf::from(p);
                            self.open(&p);
                        }
                    }
                }
                self.queue_panel(ui);
            });
    }

    /// The run (start, pause, stop, send to the server), where the program is, and its lines.
    pub(super) fn panel_job(&mut self, ui: &mut egui::Ui) {
        // the buttons go to a further row when their words do not fit in the panel
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(self.can_start(), egui::Button::new(self.t(Text::Start)))
                .clicked()
            {
                self.request_start();
            }
            if ui
                .add_enabled(self.can_pause(), egui::Button::new(self.t(Text::Pause)))
                .clicked()
            {
                self.program_pause();
            }
            if ui
                .add_enabled(self.can_stop(), egui::Button::new(self.t(Text::Stop)))
                .clicked()
            {
                self.program_stop();
            }
            if ui
                .add_enabled(
                    self.can_send_to_server(),
                    egui::Button::new(self.t(Text::McSendServer)),
                )
                .clicked()
            {
                self.request_send();
            }
        });
        ui.label(format!(
            "{} {} / {}",
            self.t(Text::McLine),
            self.program_line(),
            self.session.program.len()
        ));
        if let Some(left) = self.time_left_text() {
            ui.label(RichText::new(left).color(col(self.theme.dim)));
        }
        ui.separator();
        let next = self.program_line();
        let running = self.session.running || self.server_busy();
        let (focus, scroll_to_focus) = (self.focus_line, self.focus_scroll);
        let mut scrolled = false;
        let mut clicked: Option<usize> = None;
        let mut double_clicked: Option<usize> = None;
        egui::ScrollArea::vertical()
            .id_salt("program")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (n, line) in self.session.program.iter().enumerate() {
                    let current = running && n == next;
                    let marked = focus == Some(n);
                    // a bookmarked line has a star after its number
                    let star = if self.bookmarks.contains(n) { '*' } else { ' ' };
                    let text = RichText::new(format!("{:>4}{star} {line}", n + 1)).monospace();
                    let text = if n < next {
                        text.color(col(self.theme.ok))
                    } else if current {
                        text.color(col(self.theme.accent)).strong()
                    } else {
                        text.color(col(self.theme.dim))
                    };
                    // a line that a window asked to show is marked until another one is
                    let text = if marked {
                        text.strong()
                            .background_color(alpha(col(self.theme.accent), 70))
                    } else {
                        text
                    };
                    let r = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                    if r.double_clicked() {
                        double_clicked = Some(n);
                    } else if r.clicked() {
                        clicked = Some(n);
                    }
                    if current {
                        r.scroll_to_me(None);
                    } else if marked && scroll_to_focus {
                        r.scroll_to_me(None);
                        scrolled = true;
                    }
                }
            });
        if scrolled {
            self.focus_scroll = false;
        }
        // a click selects a line and a double click bookmarks it
        if let Some(n) = double_clicked {
            self.focus_line = Some(n);
            self.bookmarks.toggle(n);
        } else if let Some(n) = clicked {
            self.focus_line = Some(n);
        }
    }

    // ------------------------------------------------------------------------------------------------------------ the console

    /// The line of commands of the operator and the log of the session.
    pub(super) fn panel_console(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(self.t(Text::McMdi));
            // Tab finishes the code being typed (`complete`), only while the box has the keyboard
            let id = egui::Id::unique("command-box");
            let completed = ui.memory(|m| m.has_focus(id))
                && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Tab))
                && self.complete_command();
            let r = clipmenu::text_box(
                ui,
                egui::TextEdit::singleline(&mut self.mdi)
                    .id(id)
                    .desired_width((ui.available_width() - 90.0).clamp(120.0, 600.0))
                    .font(egui::TextStyle::Monospace)
                    .hint_text("G0 X10 Y10"),
            );
            if completed {
                // the cursor goes to the end of the text that was finished
                if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), id) {
                    let end = egui::text::CCursor::new(self.mdi.chars().count());
                    state
                        .cursor
                        .set_char_range(Some(egui::text::CCursorRange::one(end)));
                    state.store(ui.ctx(), id);
                }
            } else if r.changed() {
                self.mdi_options.clear();
            }
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui.button(self.t(Text::McSend)).clicked() || enter {
                let line = std::mem::take(&mut self.mdi);
                self.mdi_options.clear();
                self.session.mdi(&line);
                r.request_focus();
            }
            if !self.mdi_options.is_empty() {
                ui.label(
                    RichText::new(self.mdi_options.join("  "))
                        .monospace()
                        .color(col(self.theme.dim)),
                );
            }
        });
        ui.separator();
        ui.label(RichText::new(self.t(Text::McLog).to_uppercase()).color(col(self.theme.dim)));
        egui::ScrollArea::vertical()
            .id_salt("log")
            .stick_to_bottom(true)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for l in &self.session.log {
                    let c = if l.contains(':') && !l.starts_with('>') {
                        col(self.theme.warn)
                    } else {
                        col(self.theme.dim)
                    };
                    ui.label(RichText::new(l).monospace().color(c));
                }
            });
    }

    // ------------------------------------------------------------------------------------------------------------ the tools and the tool setter

    /// The table of tools of the operator (read-only: it is a text file the operator keeps).
    pub(super) fn panel_tool_table(&mut self, ui: &mut egui::Ui) {
        let table = self.settings.tuning.tools.clone();
        let path = crate::tools::table_path(&self.settings_path);
        egui::ScrollArea::vertical()
            .id_salt("toolsscroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if table.tools.is_empty() {
                    ui.label(
                        RichText::new(fill(
                            self.lang.code(),
                            Pt::ToolsEmpty,
                            &[path.display().to_string()],
                        ))
                        .color(col(self.theme.dim)),
                    );
                }
                for t in &table.tools {
                    let length = if t.length_mm != 0.0 {
                        format!("  L {:.3} mm", t.length_mm)
                    } else {
                        String::new()
                    };
                    ui.label(
                        RichText::new(format!(
                            "T{:<3} {}  {:.3} mm{}",
                            t.number, t.name, t.diameter_mm, length
                        ))
                        .monospace(),
                    );
                }
                ui.add_space(6.0);
                if ui.button(self.pt(Pt::ToolsReload)).clicked() {
                    self.apply_settings();
                }
            });
    }

    /// The tool setter: the numbers of the cycle, the operator's confirmation, the measure and the write into the table.
    pub(super) fn panel_toolsetter(&mut self, ui: &mut egui::Ui) {
        let cfg = self.motion.setter;
        let idle = self.machine_idle();
        let has_probe = self.session.has_probe();
        egui::ScrollArea::vertical()
            .id_salt("setterscroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.label(
                    RichText::new(format!(
                        "X {} Y {}  Z {}  ({} mm, {} mm/min)",
                        cfg.x_mm, cfg.y_mm, cfg.start_z_mm, cfg.max_travel_mm, cfg.feed_mm_min
                    ))
                    .monospace()
                    .color(col(self.theme.dim)),
                );
                ui.label(RichText::new(self.pt(Pt::SetterDisabled)).color(col(self.theme.dim)));
                ui.horizontal(|ui| {
                    ui.label(self.pt(Pt::SetterTool));
                    ui.add(egui::DragValue::new(&mut self.setter_ui.tool).range(1..=255));
                });
                if cfg.confirm {
                    let confirm = self.pt(Pt::SetterConfirm);
                    ui.checkbox(&mut self.setter_ui.confirmed, confirm);
                }
                let can = workflow::can_measure(&cfg, self.setter_ui.confirmed, idle, has_probe);
                if ui
                    .add_enabled(can, egui::Button::new(self.pt(Pt::SetterMeasure)))
                    .clicked()
                {
                    // the confirmation holds for this measure only
                    self.setter_ui.confirmed = false;
                    self.setter_ui.message = None;
                    let _ = self.session.measure_tool(&cfg, true);
                }
                if let Some(length) = self.session.setter_length {
                    ui.separator();
                    ui.label(
                        RichText::new(fill(
                            self.lang.code(),
                            Pt::SetterLength,
                            &[format!("{length:.3}")],
                        ))
                        .strong(),
                    );
                    ui.horizontal(|ui| {
                        if ui.button(self.pt(Pt::SetterWrite)).clicked() {
                            self.write_setter_length(length);
                        }
                        if ui.button(self.pt(Pt::SetterDrop)).clicked() {
                            self.session.setter_length = None;
                        }
                    });
                }
                if let Some(m) = &self.setter_ui.message {
                    ui.label(RichText::new(m).color(col(self.theme.warn)));
                }
            });
    }

    /// Writes the length that was measured into the line of the tool in the table of tools, after the operator pressed the button.
    fn write_setter_length(&mut self, length: f64) {
        let tool = self.setter_ui.tool;
        let lang = self.lang.code();
        self.setter_ui.message = Some(
            match workflow::write_tool_length(&self.settings_path, tool, length) {
                Ok(()) => {
                    self.session.setter_length = None;
                    // the table is read again, so the panel and the checks see the new length
                    self.apply_settings();
                    fill(
                        lang,
                        Pt::SetterWritten,
                        &[tool.to_string(), format!("{length:.3}")],
                    )
                }
                Err(workflow::WriteError::NoTool) => text(lang, Pt::SetterNoTool).to_string(),
                Err(workflow::WriteError::NoTable) => fill(
                    lang,
                    Pt::SetterNoTable,
                    &[crate::tools::table_path(&self.settings_path)
                        .display()
                        .to_string()],
                ),
                Err(workflow::WriteError::Other(e)) => fill(lang, Pt::SetterNoTable, &[e]),
            },
        );
    }

    // ------------------------------------------------------------------------------------------------------------ the probe and the height map

    /// The probe cycles, the height map and the files of the map.
    pub(super) fn panel_probe(&mut self, ui: &mut egui::Ui) {
        use probe::{word, Ask, W};
        let idle = self.machine_idle();
        egui::ScrollArea::vertical()
            .id_salt("probescroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if ui
                    .add_enabled(idle, egui::Button::new(word(self.lang, W::MenuZ)))
                    .clicked()
                {
                    self.win = Some(Win::Probe(Ask::Z));
                }
                if ui
                    .add_enabled(idle, egui::Button::new(word(self.lang, W::MenuGrid)))
                    .clicked()
                {
                    if let Some(ask) = self.grid_ask() {
                        self.win = Some(Win::Probe(ask));
                    }
                }
                ui.separator();
                let span = self.session.height_map.as_ref().map(|m| m.span());
                match span {
                    Some((lo, hi)) => {
                        ui.label(fill(
                            self.lang.code(),
                            Pt::ProbeMap,
                            &[format!("{lo:.3}"), format!("{hi:.3}")],
                        ));
                    }
                    None => {
                        ui.label(RichText::new(self.pt(Pt::ProbeNoMap)).color(col(self.theme.dim)));
                    }
                }
                if self.session.corrected() {
                    ui.label(RichText::new(self.pt(Pt::ProbeCorrected)).color(col(self.theme.ok)));
                }
                let can_use = idle
                    && span.is_some()
                    && !self.session.corrected()
                    && !self.session.program.is_empty();
                if ui
                    .add_enabled(can_use, egui::Button::new(word(self.lang, W::MenuUseMap)))
                    .clicked()
                {
                    self.use_height_map();
                }
                if ui
                    .add_enabled(
                        idle && self.session.corrected(),
                        egui::Button::new(word(self.lang, W::MenuDropMap)),
                    )
                    .clicked()
                    && self.session.drop_height_correction()
                {
                    self.refresh_path();
                }
                ui.separator();
                self.map_files_menu(ui);
            });
    }

    // ------------------------------------------------------------------------------------------------------------ the import

    /// A drawing (HPGL, DXF or SVG) read and turned into a program, with the post-processor, and the choice to use it as the program.
    pub(super) fn panel_import(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .id_salt("importscroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.label(self.pt(Pt::ImportFile));
                let room = (ui.available_width() - 20.0).clamp(120.0, 900.0);
                clipmenu::text_box(
                    ui,
                    egui::TextEdit::singleline(&mut self.import_ui.path)
                        .desired_width(room)
                        .hint_text("drawing.svg"),
                );
                ui.label(self.pt(Pt::ImportPost));
                clipmenu::text_box(
                    ui,
                    egui::TextEdit::singleline(&mut self.import_ui.post)
                        .desired_width(room)
                        .hint_text("shop"),
                );
                ui.label(self.pt(Pt::ImportLayer));
                let layers = LAYER_WORDS.map(|w| self.pt(w));
                let chosen = layers[self.import_ui.layer.min(layers.len() - 1)];
                egui::ComboBox::from_id_salt("importlayer")
                    .selected_text(chosen)
                    .show_ui(ui, |ui| {
                        for (i, name) in layers.iter().enumerate() {
                            ui.selectable_value(&mut self.import_ui.layer, i, *name);
                        }
                    });
                let mirror = self.pt(Pt::ImportMirror);
                ui.checkbox(&mut self.import_ui.mirror, mirror);
                ui.label(RichText::new(self.pt(Pt::ImportCfgNote)).color(col(self.theme.dim)));
                if ui.button(self.pt(Pt::ImportRead)).clicked() {
                    self.read_drawing();
                }
                ui.separator();
                match &self.import_ui.result {
                    None => {
                        ui.label(RichText::new(self.pt(Pt::ImportNone)).color(col(self.theme.dim)));
                    }
                    Some(Err(why)) => {
                        ui.label(RichText::new(why).color(col(self.theme.danger)));
                    }
                    Some(Ok(got)) => {
                        ui.label(fill(
                            self.lang.code(),
                            Pt::ImportSize,
                            &[
                                format!("{:.2}", got.size_mm.0),
                                format!("{:.2}", got.size_mm.1),
                            ],
                        ));
                        ui.label(format!(
                            "{}: {}   {}: {}",
                            Label::Paths.text(self.lang.code()),
                            got.paths,
                            Label::Lines.text(self.lang.code()),
                            got.lines.len()
                        ));
                        if let Some(b) = &got.board {
                            let kind = self.pt(layer_word(b.kind));
                            ui.label(fill(
                                self.lang.code(),
                                Pt::ImportBoard,
                                &[kind.to_string(), b.cuts.to_string(), b.tools.to_string()],
                            ));
                        }
                        for n in &got.notes {
                            ui.label(RichText::new(n).color(col(self.theme.warn)));
                        }
                    }
                }
                let ready = matches!(&self.import_ui.result, Some(Ok(_)));
                if ui
                    .add_enabled(
                        ready && self.machine_idle(),
                        egui::Button::new(self.pt(Pt::ImportUse)),
                    )
                    .clicked()
                {
                    self.use_drawing();
                }
            });
    }

    /// Reads the drawing of the panel with the post-processor it names.
    fn read_drawing(&mut self) {
        let lang = self.lang.code();
        let path = self.import_ui.path.trim().to_string();
        if path.is_empty() {
            self.import_ui.result = Some(Err(self.pt(Pt::ImportNeedsFile).to_string()));
            return;
        }
        let post = if self.import_ui.post.trim().is_empty() {
            None
        } else {
            match workflow::load_post(&self.import_ui.post, &self.settings_path, lang) {
                Ok(p) => Some(p),
                Err(e) => {
                    self.import_ui.result = Some(Err(e));
                    return;
                }
            }
        };
        let ask = crate::pcb::Ask {
            kind: [
                None,
                Some(crate::pcb::Kind::Isolation),
                Some(crate::pcb::Kind::Profile),
                Some(crate::pcb::Kind::Drill),
            ][self.import_ui.layer.min(3)],
            mirror: self.import_ui.mirror,
        };
        self.import_ui.result = Some(workflow::import_file_with(
            Path::new(&path),
            &self.motion,
            post.as_ref(),
            lang,
            &ask,
        ));
    }

    /// The drawing that was read becomes the program (a queue that was there is dropped, as when a file is opened).
    fn use_drawing(&mut self) {
        let Some(Ok(got)) = self.import_ui.result.as_ref() else {
            return;
        };
        let (text, n) = (got.lines.join("\n"), got.lines.len());
        self.queue = Queue::default();
        self.queue_seen = false;
        self.queue_gate = None;
        self.session.load(&text);
        self.bookmarks.clear();
        self.focus_line = None;
        self.refresh_path();
        self.path.clear();
        let said = fill(self.lang.code(), Pt::ImportUsed, &[n.to_string()]);
        self.session.log.push(said);
    }

    // ------------------------------------------------------------------------------------------------------------ the planning and the time

    /// The time of the program with the look-ahead and without it, and what the look-ahead saves.
    pub(super) fn panel_plan(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang.code();
        egui::ScrollArea::vertical()
            .id_salt("planscroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if self.session.program.is_empty() {
                    ui.label(RichText::new(self.pt(Pt::PlanEmpty)).color(col(self.theme.dim)));
                    return;
                }
                if ui.button(self.pt(Pt::PlanCalculate)).clicked() {
                    self.plan_ui = Some(workflow::time_compare(
                        &self.session.program,
                        &self.settings,
                        &self.motion,
                    ));
                }
                let Some(t) = self.plan_ui.clone() else {
                    return;
                };
                if t.is_stale(&self.session.program) {
                    ui.label(RichText::new(self.pt(Pt::PlanStale)).color(col(self.theme.warn)));
                }
                let row = |ui: &mut egui::Ui, label: Label, v: String| {
                    ui.label(format!("{}: {v}", label.text(lang)));
                };
                row(
                    ui,
                    Label::Time,
                    format!("{} ({:.1} s)", crate::sim::hms(t.with_s), t.with_s),
                );
                row(
                    ui,
                    Label::TimeStopToStop,
                    format!("{} ({:.1} s)", crate::sim::hms(t.without_s), t.without_s),
                );
                if t.saved_s() > 0.0 {
                    ui.label(
                        RichText::new(fill(
                            lang,
                            Pt::PlanSaved,
                            &[
                                format!("{:.1}", t.saved_s()),
                                format!("{:.0}", t.saved_percent()),
                            ],
                        ))
                        .color(col(self.theme.ok)),
                    );
                } else {
                    ui.label(
                        RichText::new(self.pt(Pt::PlanSavedNothing)).color(col(self.theme.dim)),
                    );
                }
                row(ui, Label::Blocks, t.blocks.to_string());
                row(ui, Label::Stops, t.stops.to_string());
                row(ui, Label::Length, format!("{:.1}", t.length_mm));
                if t.arcs_rebuilt > 0 {
                    row(ui, Label::ArcsRebuilt, t.arcs_rebuilt.to_string());
                }
            });
    }

    // ------------------------------------------------------------------------------------------------------------ the report

    /// What looks wrong in the program, with the line of each finding to jump to.
    pub(super) fn panel_report(&mut self, ui: &mut egui::Ui) {
        if self.session.program.is_empty() {
            ui.label(RichText::new(self.t(Text::NoFile)).color(col(self.theme.dim)));
            return;
        }
        let notes = self.program_notes();
        let problems = notes.iter().filter(|n| n.problem).count();
        ui.label(self.ws(
            Msg::CheckCounts,
            &[problems.to_string(), (notes.len() - problems).to_string()],
        ));
        if notes.is_empty() {
            ui.colored_label(col(self.theme.ok), self.w(Msg::ReportClean));
        }
        let mut jump: Option<usize> = None;
        egui::ScrollArea::vertical()
            .id_salt("reportnotes")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for n in &notes {
                    let color = match n.level {
                        Level::Danger => self.theme.danger,
                        Level::Warning => self.theme.warn,
                        Level::Info => self.theme.dim,
                    };
                    ui.horizontal(|ui| {
                        if let Some(l) = n.line {
                            let tip = self.ws(Msg::CheckJump, &[(l + 1).to_string()]);
                            if ui
                                .small_button(format!("{:>5}", l + 1))
                                .on_hover_text(tip)
                                .clicked()
                            {
                                jump = Some(l);
                            }
                        }
                        ui.colored_label(col(color), &n.text);
                    });
                }
            });
        if let Some(l) = jump {
            self.show_line(l);
        }
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;

    /// A text that was painted, its place and the part of the screen it could use.
    type Painted = (String, Rect, Rect);

    /// Draws a panel in a screen of this size, with no window, and gives every text that was painted.
    fn painted(size: Vec2, mut draw: impl FnMut(&mut egui::Ui)) -> Vec<Painted> {
        let ctx = egui::Context::default();
        let screen = Rect::from_min_size(egui::Pos2::ZERO, size);
        let input = || egui::RawInput {
            screen_rect: Some(screen),
            ..Default::default()
        };
        // the first frames only measure
        for _ in 0..3 {
            ctx.run_ui(input(), &mut draw)
                .drop_without_applying_deltas();
        }
        let out = ctx.run_ui(input(), &mut draw);
        let texts = out
            .shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Text(t) => Some((
                    t.galley.text().to_string(),
                    c.shape.visual_bounding_rect(),
                    c.clip_rect.intersect(screen),
                )),
                _ => None,
            })
            .collect();
        out.drop_without_applying_deltas();
        texts
    }

    fn whole(texts: &[Painted], label: &str, at: Vec2) {
        let found: Vec<_> = texts.iter().filter(|t| t.0 == label).collect();
        assert!(!found.is_empty(), "{at:?}: {label} is not drawn");
        for (_, place, room) in found {
            assert!(
                room.expand(0.5).contains_rect(*place),
                "{at:?}: {label} is cut: {place:?} in {room:?}"
            );
        }
    }

    #[test]
    fn the_head_of_the_path_view_and_the_size_of_the_table_are_whole_in_every_language() {
        let mut a = App::isolated();
        for lang in Lang::ALL {
            a.lang = lang;
            for width in [320.0, 412.0, 676.0, 900.0] {
                let size = Vec2::new(width, 520.0);
                let texts = painted(size, |ui| a.panel_viewer(ui));
                for label in [
                    Text::ViewPlan,
                    Text::ViewMachine,
                    Text::ResetView,
                    Text::NavHint2d,
                ] {
                    whole(&texts, a.t(label), size);
                }
                let table = texts
                    .iter()
                    .find(|t| t.0.ends_with(" mm") && t.0.contains(" x "))
                    .map(|t| t.0.clone())
                    .expect("the size of the table is drawn");
                whole(&texts, &table, size);
            }
        }
    }

    #[test]
    fn the_jog_pad_cuts_no_button_in_a_narrow_panel() {
        let mut a = App::isolated();
        let tall = [220.0, 260.0, 300.0, 360.0, 412.0, 440.0, 520.0, 700.0].map(|w| (w, 700.0));
        // the height of the panel in the window of the pictures
        let low = [260.0, 300.0, 412.0].map(|w| (w, 345.0));
        for (width, height) in tall.into_iter().chain(low) {
            let size = Vec2::new(width, height);
            let texts = painted(size, |ui| a.panel_jog(ui));
            for label in ["X-", "X+", "Y-", "Y+", "Z-", "Z+", "A-", "A+"] {
                whole(&texts, label, size);
            }
        }
    }

    #[test]
    fn the_buttons_of_the_program_are_whole_in_a_narrow_panel_in_every_language() {
        let mut a = App::isolated();
        for lang in Lang::ALL {
            a.lang = lang;
            for width in [260.0, 300.0, 412.0] {
                let size = Vec2::new(width, 700.0);
                let texts = painted(size, |ui| a.panel_job(ui));
                for label in [Text::Start, Text::Pause, Text::Stop, Text::McSendServer] {
                    whole(&texts, a.t(label), size);
                }
            }
        }
    }

    #[test]
    fn the_codes_in_force_are_in_sight_in_a_low_panel_of_the_state() {
        let mut a = App::isolated();
        let codes = [
            a.session.modal.g_codes().join("  "),
            a.session.modal.m_codes().join("  "),
        ];
        for lang in Lang::ALL {
            a.lang = lang;
            for size in [
                Vec2::new(412.0, 320.0),
                Vec2::new(520.0, 360.0),
                Vec2::new(412.0, 600.0),
            ] {
                let texts = painted(size, |ui| a.panel_position(ui));
                for label in [a.t(Text::McActiveG), a.t(Text::McActiveM)] {
                    whole(&texts, label, size);
                }
                for label in &codes {
                    whole(&texts, label, size);
                }
            }
        }
    }
}
