//! The menu bar, the settings window, the Open and Save windows (made over `eh3d-filedialog`, the same state every program of the suite
//! uses), the editor of the program and the About window.

use eh3d_filedialog::{
    date_text, size_text, Action, Browser, Chosen, Filter, Mode, PlaceKind, Problem,
};

use super::*;
use crate::settings::Settings;

/// The extensions a G-code program usually has.
const GCODE_EXTENSIONS: [&str; 11] = [
    "nc", "gcode", "ngc", "tap", "cnc", "txt", "ncc", "gc", "eia", "anc", "mpf",
];

/// The window that is open over the program (there is one at a time).
pub(super) enum Win {
    Settings(Box<Draft>),
    Open(Box<Browser>),
    Save(Box<Browser>),
    /// Save: the file is there already and the operator is asked whether to replace it.
    Replace(Box<Browser>, PathBuf),
    Edit(String),
    About,
    /// A program was left in the middle: go on from its line or leave it.
    Resume(crate::recovery::Recovery),
    /// The summary of a program that is about to start: start it or leave it.
    Summary(Box<summary::PendingStart>),
    /// The list of the keys.
    Shortcuts,
}

/// The settings as they are being changed; they take effect with Ok.
pub(super) struct Draft {
    pub s: Settings,
    pub section: usize,
}

impl App {
    fn browser(&self, mode: Mode) -> Box<Browser> {
        let start = if mode == Mode::Save && !self.path.is_empty() {
            Path::new(&self.path).parent().map(Path::to_path_buf)
        } else {
            None
        }
        .or_else(|| {
            Some(PathBuf::from(&self.settings.last_dir))
                .filter(|p| !self.settings.last_dir.is_empty() && p.is_dir())
        })
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let base = std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| PathBuf::from("."));
        let mut b = Browser::new(
            mode,
            &start,
            &base,
            vec![Filter::new(&GCODE_EXTENSIONS), Filter::any()],
            Some("nc"),
        );
        if mode == Mode::Save {
            b.name_text = Path::new(&self.path).file_name().map_or_else(
                || "program.nc".to_string(),
                |n| n.to_string_lossy().to_string(),
            );
        }
        Box::new(b)
    }

    /// Opens a window by its name (for the pictures and the command line).
    pub(super) fn open_window(&mut self, name: &str) {
        self.win = match name {
            "settings" => Some(Win::Settings(Box::new(Draft {
                s: self.settings.clone(),
                section: 1,
            }))),
            "open" => Some(Win::Open(self.browser(Mode::Open))),
            "save" => Some(Win::Save(self.browser(Mode::Save))),
            "edit" => Some(Win::Edit(self.session.program.join(
                "
",
            ))),
            "about" => Some(Win::About),
            "summary" => Some(Win::Summary(Box::new(summary::PendingStart {
                report: self.report(),
                server: false,
            }))),
            "keys" => Some(Win::Shortcuts),
            _ => None,
        };
    }

    /// Saves the program as it is now, as text.
    fn save_program(&mut self, path: &Path) -> Result<(), String> {
        let mut text = self.session.program.join("\n");
        text.push('\n');
        std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))?;
        self.path = path.display().to_string();
        self.remember(path);
        Ok(())
    }

    /// A file that was opened or saved is a recent one, and its folder is where the windows start next time.
    pub(super) fn remember(&mut self, path: &Path) {
        self.settings.remember_recent(&path.to_string_lossy());
        if let Some(d) = path.parent() {
            self.settings.last_dir = d.display().to_string();
        }
        self.save_settings();
    }

    pub(super) fn save_settings(&self) {
        let _ = self.settings.save(&self.settings_path);
    }

    /// The effect of a change of the settings on the window and on the machine.
    pub(super) fn apply_settings(&mut self) {
        self.lang = self.settings.lang;
        if self.theme.id != self.settings.theme {
            self.theme = Theme::for_id(self.settings.theme);
        }
        let travel = self.settings.travel;
        self.session.set_travel(travel);
        if self.settings.read_controller_travel && !self.session.travel_asked {
            self.session.read_controller_travel();
        }
    }

    // ------------------------------------------------------------------------------------------------------------ the menu bar

    pub(super) fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("menu").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                self.menu_file(ui);
                self.menu_view(ui);
                self.menu_program(ui);
                self.menu_machine(ui);
                self.menu_help(ui);
            });
        });
    }

    fn menu_file(&mut self, ui: &mut egui::Ui) {
        ui.menu_button(self.t(Text::MenuFile), |ui| {
            if self.menu_item(ui, self.t(Text::McOpenGcode), Shortcut::Open, true) {
                self.win = Some(Win::Open(self.browser(Mode::Open)));
                ui.close();
            }
            let recent: Vec<String> = self
                .settings
                .recent
                .iter()
                .filter(|p| Path::new(p.as_str()).exists())
                .cloned()
                .collect();
            ui.add_enabled_ui(!recent.is_empty(), |ui| {
                ui.menu_button(self.t(Text::Recent), |ui| {
                    for p in &recent {
                        let name = Path::new(p)
                            .file_name()
                            .map_or_else(|| p.clone(), |n| n.to_string_lossy().to_string());
                        if ui.button(name).on_hover_text(p).clicked() {
                            let p = PathBuf::from(p);
                            self.open(&p);
                            ui.close();
                        }
                    }
                });
            });
            if self.menu_item(ui, self.t(Text::McSaveGcode), Shortcut::Save, true) {
                self.win = Some(Win::Save(self.browser(Mode::Save)));
                ui.close();
            }
            if self.menu_item(ui, self.t(Text::McEditProgram), Shortcut::Edit, true) {
                self.win = Some(Win::Edit(self.session.program.join("\n")));
                ui.close();
            }
            ui.separator();
            if self.menu_item(
                ui,
                &format!("{} ...", self.t(Text::SettingsTitle)),
                Shortcut::Settings,
                true,
            ) {
                self.win = Some(Win::Settings(Box::new(Draft {
                    s: self.settings.clone(),
                    section: 0,
                })));
                ui.close();
            }
            ui.separator();
            if self.menu_item(ui, self.t(Text::MenuExit), Shortcut::Quit, true) {
                self.quit = true;
            }
        });
    }

    fn menu_view(&mut self, ui: &mut egui::Ui) {
        ui.menu_button(self.t(Text::MenuView), |ui| {
            let (toolbar, buttons, program, log, rapids) = (
                self.t(Text::McShowToolbar),
                self.t(Text::McShowButtons),
                self.t(Text::McShowProgram),
                self.t(Text::McShowLog),
                self.t(Text::McShowRapids),
            );
            let mut changed = false;
            changed |= ui
                .checkbox(&mut self.settings.show_toolbar, toolbar)
                .changed();
            changed |= ui
                .checkbox(&mut self.settings.show_buttons, buttons)
                .changed();
            changed |= ui
                .checkbox(&mut self.settings.show_program, program)
                .changed();
            changed |= ui.checkbox(&mut self.settings.show_log, log).changed();
            changed |= ui
                .checkbox(&mut self.settings.show_rapids, rapids)
                .changed();
            ui.separator();
            if self.menu_item(ui, self.t(Text::ViewMachine), Shortcut::ToggleView, true) {
                self.toggle_view();
                ui.close();
            }
            if self.menu_item(ui, self.t(Text::ResetView), Shortcut::ResetView, true) {
                self.reset_view();
                ui.close();
            }
            ui.separator();
            if ui
                .radio(!self.fit_program, self.t(Text::McFitTable))
                .clicked()
            {
                self.fit_program = false;
            }
            if ui
                .radio(self.fit_program, self.t(Text::McFitProgram))
                .clicked()
            {
                self.fit_program = true;
            }
            if changed {
                self.save_settings();
            }
        });
    }

    fn menu_program(&mut self, ui: &mut egui::Ui) {
        ui.menu_button(self.t(Text::McProgram), |ui| {
            if self.menu_item(ui, self.t(Text::Start), Shortcut::Start, self.can_start()) {
                self.request_start();
                ui.close();
            }
            if self.menu_item(ui, self.t(Text::Pause), Shortcut::Pause, self.can_pause()) {
                self.program_pause();
                ui.close();
            }
            if self.menu_item(ui, self.t(Text::Stop), Shortcut::Stop, self.can_stop()) {
                self.program_stop();
                ui.close();
            }
            if ui
                .add_enabled(
                    self.can_send_to_server(),
                    egui::Button::new(self.t(Text::McSendServer)),
                )
                .clicked()
            {
                self.request_send();
                ui.close();
            }
            ui.separator();
            if ui.button(self.t(Text::McRewind)).clicked() {
                self.session.rewind();
                ui.close();
            }
            if ui.button(self.t(Text::McClearProgram)).clicked() {
                self.session.clear_program();
                self.path_toolpath.clear();
                ui.close();
            }
        });
    }

    fn menu_machine(&mut self, ui: &mut egui::Ui) {
        ui.menu_button(self.t(Text::McMachine), |ui| {
            if ui.button(self.t(Text::EmergencyStop)).clicked() {
                self.session.estop();
                ui.close();
            }
            if ui.button(self.t(Text::McUnlock)).clicked() {
                self.session.unlock();
                ui.close();
            }
            ui.separator();
            if ui.button(self.t(Text::McHome)).clicked() {
                self.session.home();
                ui.close();
            }
            if ui.button(self.t(Text::McZeroXyz)).clicked() {
                self.session.zero_axes(&[0, 1, 2]);
                ui.close();
            }
            if ui.button(self.t(Text::McZeroXy)).clicked() {
                self.session.zero_axes(&[0, 1]);
                ui.close();
            }
            if ui.button(self.t(Text::McGoZeroXy)).clicked() {
                self.session.go_zero_xy();
                ui.close();
            }
            if ui.button(self.t(Text::McGoZeroZ)).clicked() {
                self.session.go_zero_z();
                ui.close();
            }
            ui.separator();
            if ui.button(self.t(Text::McGoG28)).clicked() {
                self.session.go_g28();
                ui.close();
            }
            if ui.button(self.t(Text::McGoG30)).clicked() {
                self.session.go_g30();
                ui.close();
            }
            if ui.button(self.t(Text::McSetG28)).clicked() {
                self.session.store_position(28);
                ui.close();
            }
            if ui.button(self.t(Text::McSetG30)).clicked() {
                self.session.store_position(30);
                ui.close();
            }
            ui.separator();
            if ui.button(self.t(Text::McSpindleOn)).clicked() {
                self.session.spindle(Some(self.settings.spindle_rpm));
                ui.close();
            }
            if ui.button(self.t(Text::McSpindleOff)).clicked() {
                self.session.spindle(None);
                ui.close();
            }
            if ui.button(self.t(Text::McCoolantOn)).clicked() {
                self.session.coolant(true);
                ui.close();
            }
            if ui.button(self.t(Text::McCoolantOff)).clicked() {
                self.session.coolant(false);
                ui.close();
            }
        });
    }

    fn menu_help(&mut self, ui: &mut egui::Ui) {
        ui.menu_button(self.t(Text::MenuHelp), |ui| {
            if self.menu_item(ui, self.t(Text::McShortcuts), Shortcut::Help, true) {
                self.win = Some(Win::Shortcuts);
                ui.close();
            }
            if ui.button(self.t(Text::About)).clicked() {
                self.win = Some(Win::About);
                ui.close();
            }
        });
    }

    // ------------------------------------------------------------------------------------------------------------ the windows

    /// Draws the window that is open, if there is one.
    pub(super) fn windows(&mut self, ctx: &egui::Context) {
        let Some(win) = self.win.take() else { return };
        self.win = match win {
            Win::Settings(d) => self.settings_window(ctx, d),
            Win::Open(b) => self.file_window(ctx, b, Mode::Open),
            Win::Save(b) => self.file_window(ctx, b, Mode::Save),
            Win::Replace(b, p) => self.replace_window(ctx, b, p),
            Win::Edit(t) => self.edit_window(ctx, t),
            Win::About => self.about_window(ctx),
            Win::Resume(r) => self.resume_window(ctx, r),
            Win::Summary(p) => self.summary_window(ctx, p),
            Win::Shortcuts => self.shortcuts_window(ctx),
        };
    }

    pub(super) fn window(&self, title: &str) -> egui::Window<'static> {
        egui::Window::new(title.to_string())
            .collapsible(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
    }

    fn resume_window(&mut self, ctx: &egui::Context, r: crate::recovery::Recovery) -> Option<Win> {
        let mut answer: Option<bool> = None;
        self.window(self.t(Text::ResumeTitle))
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(self.t(Text::ResumeBody));
                ui.add_space(6.0);
                ui.label(format!(
                    "{}: {} / {}",
                    Path::new(&r.file)
                        .file_name()
                        .map_or_else(String::new, |n| n.to_string_lossy().to_string()),
                    r.next + 1,
                    r.total
                ));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button(self.t(Text::ResumeContinue)).clicked() {
                        answer = Some(true);
                    }
                    if ui.button(self.t(Text::ResumeCancel)).clicked() {
                        answer = Some(false);
                    }
                });
            });
        match answer {
            Some(true) => {
                self.open(Path::new(&r.file));
                let same_program = self.session.program.len() == r.total
                    && (r.sha256.is_empty()
                        || r.sha256 == crate::recovery::program_hash(&self.session.program));
                if same_program && !r.job.is_empty() {
                    // the program was sent to the server: its job is followed again, or made again from the line it reached; the server waits for the
                    // operator before the first move
                    self.recovery_saved = Some(r.next);
                    self.attach_to_server(r.job.clone(), r.next);
                } else if same_program {
                    // it waits paused: the operator checks the machine and presses Resume; the controller is first brought back to the state of the program
                    self.session.resume_at_safe(
                        r.next,
                        self.settings.resume_safe_z_mm,
                        self.settings.resume_spindle_wait_s,
                    );
                    self.recovery_saved = Some(r.next);
                } else {
                    // the file is not the one that was running (it changed): nothing is safe to go on from
                    self.forget_recovery();
                }
                None
            }
            Some(false) => {
                self.forget_recovery();
                None
            }
            None => Some(Win::Resume(r)),
        }
    }

    fn about_window(&mut self, ctx: &egui::Context) -> Option<Win> {
        let mut open = true;
        self.window(self.t(Text::About))
            .open(&mut open)
            .resizable(false)
            .show(ctx, |ui| {
                ui.heading(RichText::new("EH3D MOTIONCNC").color(col(self.theme.accent)));
                ui.label(format!(
                    "{} {}",
                    self.t(Text::Version),
                    env!("CARGO_PKG_VERSION")
                ));
                ui.label(format!("{}: GPL-3.0-or-later", self.t(Text::Licence)));
                ui.add_space(6.0);
                ui.label(self.t(Text::McAboutBody));
                ui.hyperlink("https://github.com/JuanenRac/EH3D-MOTIONCNC");
            });
        open.then_some(Win::About)
    }

    fn edit_window(&mut self, ctx: &egui::Context, mut text: String) -> Option<Win> {
        let mut keep = true;
        let mut apply = false;
        self.window(self.t(Text::McEditProgram))
            .default_size([620.0, 460.0])
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(380.0)
                    .show(ui, |ui| {
                        super::clipmenu::text_box(
                            ui,
                            egui::TextEdit::multiline(&mut text)
                                .font(egui::TextStyle::Monospace)
                                .desired_width(f32::INFINITY)
                                .desired_rows(20),
                        );
                    });
                ui.horizontal(|ui| {
                    if ui.button(self.t(Text::Apply)).clicked() {
                        apply = true;
                    }
                    if ui.button(self.t(Text::Close)).clicked() {
                        keep = false;
                    }
                    ui.label(format!(
                        "{} {}",
                        text.lines().count(),
                        self.t(Text::McLine).to_lowercase()
                    ));
                });
            });
        if apply {
            self.session.load(&text);
            self.path_toolpath = gcode::toolpath(&self.session.program);
        }
        keep.then_some(Win::Edit(text))
    }

    fn replace_window(
        &mut self,
        ctx: &egui::Context,
        b: Box<Browser>,
        path: PathBuf,
    ) -> Option<Win> {
        let mut answer = None;
        self.window(self.t(Text::Save))
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(format!(
                    "{} {}",
                    path.file_name()
                        .map_or_else(String::new, |n| n.to_string_lossy().to_string()),
                    self.t(Text::McReplaceAsk)
                ));
                ui.horizontal(|ui| {
                    if ui.button(self.t(Text::McReplace)).clicked() {
                        answer = Some(true);
                    }
                    if ui.button(self.t(Text::McCancel)).clicked() {
                        answer = Some(false);
                    }
                });
            });
        match answer {
            Some(true) => match self.save_program(&path) {
                Ok(()) => None,
                Err(e) => {
                    self.session.log.push(e);
                    None
                }
            },
            Some(false) => Some(Win::Save(b)),
            None => Some(Win::Replace(b, path)),
        }
    }

    fn place_name(&self, kind: PlaceKind, name: &str) -> String {
        match kind {
            PlaceKind::Examples => self.t(Text::PlaceExamples).to_string(),
            PlaceKind::Desktop => self.t(Text::PlaceDesktop).to_string(),
            PlaceKind::Documents => self.t(Text::PlaceDocuments).to_string(),
            PlaceKind::Downloads => self.t(Text::PlaceDownloads).to_string(),
            PlaceKind::Drive => name.to_string(),
        }
    }

    /// The Open and Save window: the places on the left, the path above, the list of the folder, the file name and the type below.
    fn file_window(&mut self, ctx: &egui::Context, mut b: Box<Browser>, mode: Mode) -> Option<Win> {
        let title = if mode == Mode::Open {
            self.t(Text::McOpenTitle)
        } else {
            self.t(Text::McSaveTitle)
        };
        let mut done: Option<Chosen> = None;
        // the files picked with Ctrl+click to run one after the other (the Open window only)
        let mut multi: Vec<PathBuf> = std::mem::take(&mut self.open_multi);
        let mut ctrl_click = false;
        let mut queue_chosen: Option<Vec<PathBuf>> = None;
        let mut cancel = false;
        let mut error: Option<&'static str> = None;
        let previewing = mode == Mode::Open && self.settings.open_preview;
        if previewing {
            self.update_preview(&b);
        }
        self.window(title)
            .default_size([if previewing { 1030.0 } else { 780.0 }, 500.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .button("<")
                        .on_hover_text(self.t(Text::NavBack))
                        .clicked()
                    {
                        b.back();
                    }
                    if ui.button("^").on_hover_text(self.t(Text::NavUp)).clicked() {
                        b.up();
                    }
                    let r = super::clipmenu::text_box(
                        ui,
                        egui::TextEdit::singleline(&mut b.path_text).desired_width(f32::INFINITY),
                    );
                    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        b.go_typed_path();
                    }
                });
                ui.separator();
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(150.0);
                        ui.label(
                            RichText::new(self.t(Text::Places).to_uppercase())
                                .color(col(self.theme.dim)),
                        );
                        let places = b.places.clone();
                        for (i, p) in places.iter().enumerate() {
                            let on = b.dir.as_ref() == Some(&p.path);
                            if ui
                                .selectable_label(on, self.place_name(p.kind, &p.name))
                                .clicked()
                            {
                                b.go_place(i);
                            }
                        }
                        let recent: Vec<String> = self
                            .settings
                            .recent
                            .iter()
                            .filter(|p| Path::new(p.as_str()).exists())
                            .take(6)
                            .cloned()
                            .collect();
                        if mode == Mode::Open && !recent.is_empty() {
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new(self.t(Text::Recent).to_uppercase())
                                    .color(col(self.theme.dim)),
                            );
                            for p in recent {
                                let name = Path::new(&p)
                                    .file_name()
                                    .map_or_else(|| p.clone(), |n| n.to_string_lossy().to_string());
                                if ui.selectable_label(false, name).on_hover_text(&p).clicked() {
                                    b.name_text = p;
                                }
                            }
                        }
                    });
                    ui.add_space(8.0);
                    if previewing {
                        ui.vertical(|ui| {
                            ui.set_width(240.0);
                            self.paint_file_preview(ui);
                        });
                        ui.add_space(8.0);
                    }
                    ui.vertical(|ui| {
                        let headings = [Text::ColName, Text::ColDate, Text::ColType, Text::ColSize];
                        egui::Grid::new("fileheads")
                            .num_columns(4)
                            .min_col_width(70.0)
                            .show(ui, |ui| {
                                for (c, h) in headings.into_iter().enumerate() {
                                    let mark = if b.sort.0 == c {
                                        if b.sort.1 {
                                            " ^"
                                        } else {
                                            " v"
                                        }
                                    } else {
                                        ""
                                    };
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                RichText::new(format!("{}{mark}", self.t(h)))
                                                    .color(col(self.theme.accent)),
                                            )
                                            .frame(false),
                                        )
                                        .clicked()
                                    {
                                        b.sort_by(c);
                                    }
                                }
                                ui.end_row();
                            });
                        let mut clicked = None;
                        let mut activated = None;
                        egui::ScrollArea::vertical()
                            .max_height(300.0)
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                egui::Grid::new("filelist")
                                    .num_columns(4)
                                    .striped(true)
                                    .min_col_width(70.0)
                                    .show(ui, |ui| {
                                        for (i, e) in b.entries.iter().enumerate() {
                                            let name = if e.is_dir {
                                                format!("> {}", e.name)
                                            } else {
                                                e.name.clone()
                                            };
                                            let picked = !e.is_dir
                                                && b.dir.as_ref().is_some_and(|d| {
                                                    multi.contains(&d.join(&e.name))
                                                });
                                            let r = ui
                                                .selectable_label(b.sel == Some(i) || picked, name);
                                            if r.clicked() {
                                                clicked = Some(i);
                                                ctrl_click = ui.input(|i| i.modifiers.command);
                                            }
                                            if r.double_clicked() {
                                                activated = Some(i);
                                            }
                                            ui.label(
                                                e.modified.map_or_else(String::new, date_text),
                                            );
                                            ui.label(if e.is_dir {
                                                self.t(Text::TypeFolder).to_string()
                                            } else {
                                                e.name
                                                    .rsplit_once('.')
                                                    .map_or_else(String::new, |(_, x)| {
                                                        x.to_uppercase()
                                                    })
                                            });
                                            ui.label(if e.is_dir {
                                                String::new()
                                            } else {
                                                size_text(e.size)
                                            });
                                            ui.end_row();
                                        }
                                    });
                            });
                        if let Some(i) = clicked {
                            let file = b
                                .entries
                                .get(i)
                                .filter(|e| !e.is_dir)
                                .and_then(|e| b.dir.as_ref().map(|d| d.join(&e.name)));
                            match file {
                                Some(path) if ctrl_click && mode == Mode::Open => {
                                    // the file already chosen with a plain click is the first of the queue
                                    if multi.is_empty() {
                                        if let Some(prev) = Self::previewed_path(&b) {
                                            multi.push(prev);
                                        }
                                    }
                                    if let Some(at) = multi.iter().position(|p| *p == path) {
                                        multi.remove(at);
                                    } else {
                                        multi.push(path);
                                    }
                                    b.select(i);
                                }
                                _ => {
                                    multi.clear();
                                    b.select(i);
                                }
                            }
                        }
                        if let Some(i) = activated {
                            match b.activate(i) {
                                Action::Chosen(p) => {
                                    b.name_text = p.file_name().map_or_else(String::new, |n| {
                                        n.to_string_lossy().to_string()
                                    });
                                    match b.confirm() {
                                        Ok(c) => done = Some(c),
                                        Err(_) => error = Some(self.t(Text::McNoSuchFile)),
                                    }
                                }
                                Action::Entered | Action::Nothing => {}
                            }
                        }
                    });
                });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(self.t(Text::FileNameLabel));
                    let r = super::clipmenu::text_box(
                        ui,
                        egui::TextEdit::singleline(&mut b.name_text).desired_width(300.0),
                    );
                    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    let mut filter = b.filter;
                    egui::ComboBox::from_id_salt("filefilter")
                        .selected_text(if filter == 0 {
                            self.t(Text::McFilterGcode)
                        } else {
                            self.t(Text::FilterAny)
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut filter, 0, self.t(Text::McFilterGcode));
                            ui.selectable_value(&mut filter, 1, self.t(Text::FilterAny));
                        });
                    if filter != b.filter {
                        b.set_filter(filter);
                    }
                    let preview_label = self.t(Text::PreviewShow);
                    if mode == Mode::Open
                        && ui
                            .checkbox(&mut self.settings.open_preview, preview_label)
                            .changed()
                    {
                        self.save_settings();
                    }
                    let label = if mode == Mode::Open {
                        self.t(Text::OpenFile)
                    } else {
                        self.t(Text::Save)
                    };
                    if mode == Mode::Open && multi.len() >= 2 {
                        ui.label(format!("{}: {}", self.t(Text::McQueue), multi.len()));
                    }
                    if ui.button(label).clicked() || enter {
                        if mode == Mode::Open && multi.len() >= 2 {
                            queue_chosen = Some(multi.clone());
                        } else {
                            match b.confirm() {
                                Ok(c) => done = Some(c),
                                Err(Problem::NoName) => error = Some(self.t(Text::McTypeName)),
                                Err(Problem::Missing(_)) => {
                                    error = Some(self.t(Text::McNoSuchFile))
                                }
                                Err(Problem::IsFolder(p)) => {
                                    b.go(Some(p));
                                    b.name_text.clear();
                                }
                                Err(Problem::NoFolder) => error = Some(self.t(Text::McTypeName)),
                            }
                        }
                    }
                    if ui.button(self.t(Text::McCancel)).clicked() {
                        cancel = true;
                    }
                });
                if let Some(e) = self.dialog_error {
                    ui.colored_label(col(self.theme.danger), e);
                }
            });
        self.dialog_error = error;
        if cancel {
            self.dialog_error = None;
            return None;
        }
        if let Some(paths) = queue_chosen {
            self.dialog_error = None;
            self.start_queue(paths);
            return None;
        }
        self.open_multi = multi;
        match done {
            Some(Chosen::File(p)) => {
                self.dialog_error = None;
                if mode == Mode::Open {
                    self.open(&p);
                } else if let Err(e) = self.save_program(&p) {
                    self.session.log.push(e);
                }
                None
            }
            Some(Chosen::Overwrite(p)) => {
                if mode == Mode::Open {
                    self.open(&p);
                    None
                } else {
                    Some(Win::Replace(b, p))
                }
            }
            None => Some(if mode == Mode::Open {
                Win::Open(b)
            } else {
                Win::Save(b)
            }),
        }
    }

    /// The file the Open window points at (the name typed below the list, in the folder being looked at), when it is a file.
    fn previewed_path(b: &Browser) -> Option<PathBuf> {
        let name = b.name_text.trim();
        if name.is_empty() {
            return None;
        }
        let typed = Path::new(name);
        let full = if typed.is_absolute() {
            typed.to_path_buf()
        } else {
            b.dir.as_ref()?.join(typed)
        };
        full.is_file().then_some(full)
    }

    /// Reads the picture of the file pointed at, once for each file (not on every frame).
    fn update_preview(&mut self, b: &Browser) {
        match Self::previewed_path(b) {
            None => self.preview = None,
            Some(p) => {
                if self.preview.as_ref().map(|(q, _)| q) != Some(&p) {
                    let pv = gcode::read_preview(&p);
                    self.preview = Some((p, pv));
                }
            }
        }
    }

    /// The path of the file pointed at in the Open window, fitted in a box, with its size and how many moves it has.
    fn paint_file_preview(&self, ui: &mut egui::Ui) {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(240.0, 220.0), egui::Sense::hover());
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 6.0, col(self.theme.panel));
        let shown = match &self.preview {
            Some((_, Some(pv))) => pv.bounds.map(|b| (pv, b)),
            _ => None,
        };
        let Some((pv, [x0, y0, x1, y1])) = shown else {
            p.text(
                rect.center(),
                Align2::CENTER_CENTER,
                self.t(Text::PreviewNone),
                egui::FontId::proportional(13.0),
                col(self.theme.dim),
            );
            return;
        };
        let inner = rect.shrink(12.0);
        let (w, h) = ((x1 - x0).max(1.0), (y1 - y0).max(1.0));
        let k = ((inner.width() as f64) / w).min((inner.height() as f64) / h);
        let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        let at = |x: f64, y: f64| {
            Pos2::new(
                inner.center().x + ((x - cx) * k) as f32,
                inner.center().y - ((y - cy) * k) as f32,
            )
        };
        for s in &pv.segments {
            let (color, width) = if s.rapid {
                (alpha(col(self.theme.dim), 120), 1.0)
            } else {
                (col(self.theme.accent), 1.6)
            };
            p.line_segment(
                [at(s.from.0, s.from.1), at(s.to.0, s.to.1)],
                Stroke::new(width, color),
            );
        }
        ui.label(format!(
            "{}: {:.1} x {:.1}",
            self.t(Text::SummarySize),
            x1 - x0,
            y1 - y0
        ));
        let rapid = pv.segments.iter().filter(|s| s.rapid).count();
        ui.label(format!(
            "{}: {} / {}",
            self.t(Text::McSumMoves),
            rapid,
            pv.moves - rapid
        ));
    }

    /// The settings: the sections on the left like the tree of PlanetCNC, the section on the right, Ok and Cancel.
    fn settings_window(&mut self, ctx: &egui::Context, mut d: Box<Draft>) -> Option<Win> {
        let mut ok = false;
        let mut cancel = false;
        self.window(self.t(Text::SettingsTitle))
            .default_size([620.0, 460.0])
            .show(ctx, |ui| {
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(150.0);
                        for (i, n) in [
                            Text::McSecGeneral,
                            Text::McSecInterface,
                            Text::McSecAxes,
                            Text::McSecJog,
                            Text::McSecServer,
                            Text::McSecSafety,
                        ]
                        .into_iter()
                        .enumerate()
                        {
                            if ui.selectable_label(d.section == i, self.t(n)).clicked() {
                                d.section = i;
                            }
                        }
                    });
                    ui.add_space(8.0);
                    ui.vertical(|ui| {
                        ui.set_min_width(420.0);
                        match d.section {
                            0 => {
                                ui.label(
                                    RichText::new(self.t(Text::Units).to_uppercase())
                                        .color(col(self.theme.dim)),
                                );
                                ui.radio_value(&mut d.s.inches, false, self.t(Text::UnitMm));
                                ui.radio_value(&mut d.s.inches, true, self.t(Text::McUnitInch));
                                ui.add_space(8.0);
                                ui.horizontal(|ui| {
                                    ui.label(self.t(Text::McSpindle));
                                    numfield::field(
                                        ui,
                                        "set-spindle",
                                        &mut d.s.spindle_rpm,
                                        (0.0, 1_000_000.0),
                                        100.0,
                                        "rpm",
                                    );
                                });
                                ui.checkbox(&mut d.s.show_rapids, self.t(Text::McShowRapids));
                            }
                            1 => {
                                ui.label(
                                    RichText::new(self.t(Text::Language).to_uppercase())
                                        .color(col(self.theme.dim)),
                                );
                                ui.horizontal_wrapped(|ui| {
                                    for l in Lang::ALL {
                                        ui.selectable_value(&mut d.s.lang, l, lang_name(l));
                                    }
                                });
                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new(self.t(Text::ThemeLabel).to_uppercase())
                                        .color(col(self.theme.dim)),
                                );
                                egui::Grid::new("themes")
                                    .num_columns(2)
                                    .spacing([10.0, 6.0])
                                    .show(ui, |ui| {
                                        for (i, id) in ThemeId::ALL.into_iter().enumerate() {
                                            ui.selectable_value(
                                                &mut d.s.theme,
                                                id,
                                                tr(d.s.lang, id.text()),
                                            );
                                            if i % 2 == 1 {
                                                ui.end_row();
                                            }
                                        }
                                    });
                                ui.add_space(8.0);
                                ui.checkbox(&mut d.s.show_toolbar, self.t(Text::McShowToolbar));
                                ui.checkbox(&mut d.s.show_buttons, self.t(Text::McShowButtons));
                                ui.checkbox(&mut d.s.show_program, self.t(Text::McShowProgram));
                                ui.checkbox(&mut d.s.show_log, self.t(Text::McShowLog));
                                ui.checkbox(&mut d.s.shortcuts, self.t(Text::McShortcuts));
                            }
                            2 => {
                                ui.label(
                                    RichText::new(self.t(Text::McLimits))
                                        .color(col(self.theme.dim)),
                                );
                                egui::Grid::new("limits")
                                    .num_columns(3)
                                    .spacing([10.0, 6.0])
                                    .show(ui, |ui| {
                                        for (i, n) in ["X", "Y", "Z"].iter().enumerate() {
                                            ui.label(*n);
                                            numfield::field(
                                                ui,
                                                ("set-travel-lo", i),
                                                &mut d.s.travel[i].0,
                                                (-100_000.0, 100_000.0),
                                                1.0,
                                                "mm",
                                            );
                                            numfield::field(
                                                ui,
                                                ("set-travel-hi", i),
                                                &mut d.s.travel[i].1,
                                                (-100_000.0, 100_000.0),
                                                1.0,
                                                "mm",
                                            );
                                            ui.end_row();
                                        }
                                    });
                                for t in &mut d.s.travel {
                                    if t.1 <= t.0 {
                                        t.1 = t.0 + 1.0;
                                    }
                                }
                            }
                            4 => {
                                egui::Grid::new("server")
                                    .num_columns(2)
                                    .spacing([10.0, 6.0])
                                    .show(ui, |ui| {
                                        ui.label(self.t(Text::McServerAddress));
                                        super::clipmenu::text_box(
                                            ui,
                                            egui::TextEdit::singleline(&mut d.s.server_address)
                                                .desired_width(260.0)
                                                .hint_text("192.168.0.180:18200"),
                                        );
                                        ui.end_row();
                                        ui.label(self.t(Text::McServerToken));
                                        super::clipmenu::text_box(
                                            ui,
                                            egui::TextEdit::singleline(&mut d.s.server_token)
                                                .desired_width(260.0)
                                                .password(true),
                                        );
                                        ui.end_row();
                                        ui.label(self.t(Text::McServerMachine));
                                        super::clipmenu::text_box(
                                            ui,
                                            egui::TextEdit::singleline(&mut d.s.server_machine)
                                                .desired_width(260.0),
                                        );
                                        ui.end_row();
                                    });
                            }
                            5 => {
                                ui.checkbox(&mut d.s.confirm_start, self.t(Text::McConfirmStart));
                                ui.checkbox(
                                    &mut d.s.reject_out_of_travel,
                                    self.t(Text::McSumReject),
                                );
                                ui.checkbox(
                                    &mut d.s.read_controller_travel,
                                    self.t(Text::McGrblTravel),
                                );
                                ui.add_space(8.0);
                                ui.horizontal(|ui| {
                                    ui.label(self.t(Text::McRapidFeed));
                                    numfield::field(
                                        ui,
                                        "set-rapid",
                                        &mut d.s.rapid_mm_min,
                                        (1.0, 1_000_000.0),
                                        100.0,
                                        "mm/min",
                                    );
                                });
                            }
                            _ => {
                                ui.label(self.t(Text::McDefaultStep));
                                ui.horizontal(|ui| {
                                    for s in STEPS {
                                        ui.selectable_value(&mut d.s.step_mm, s, format!("{s}"));
                                    }
                                });
                                ui.add_space(8.0);
                                ui.horizontal(|ui| {
                                    ui.label(self.t(Text::McDefaultFeed));
                                    numfield::field(
                                        ui,
                                        "set-feed",
                                        &mut d.s.feed_mm_min,
                                        (1.0, 100_000.0),
                                        10.0,
                                        "mm/min",
                                    );
                                });
                            }
                        }
                    });
                });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(self.t(Text::Ok)).clicked() {
                        ok = true;
                    }
                    if ui.button(self.t(Text::McCancel)).clicked() {
                        cancel = true;
                    }
                });
            });
        if ok {
            let before = self.settings.clone();
            self.settings = d.s;
            // what the operator chose for the jog is the jog now
            if (self.settings.step_mm, self.settings.feed_mm_min)
                != (before.step_mm, before.feed_mm_min)
            {
                self.session.step_mm = self.settings.step_mm;
                self.session.feed_mm_min = self.settings.feed_mm_min;
            }
            self.apply_settings();
            self.save_settings();
            return None;
        }
        if cancel {
            return None;
        }
        Some(Win::Settings(d))
    }
}

/// The name of a language, in that language.
pub(super) fn lang_name(l: Lang) -> &'static str {
    l.native_name()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        App::new(&Options::default())
    }

    fn work_folder(tag: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("motioncnc-dialogs-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_program_opened_is_a_recent_one_and_a_program_saved_comes_back_the_same() {
        let d = work_folder("save");
        let a_path = d.join("a.nc");
        std::fs::write(&a_path, "G21 G90\nG0 X10 Y10\nG1 X20 F300\n").unwrap();
        let mut a = app();
        a.open(&a_path);
        assert_eq!(a.session.program.len(), 3);
        assert_eq!(a.path_toolpath.len(), 2);
        assert_eq!(a.settings.recent[0], a_path.to_string_lossy());
        assert_eq!(a.settings.last_dir, d.display().to_string());
        // saved under another name, it reads back as the same program
        let b_path = d.join("b.nc");
        a.save_program(&b_path).unwrap();
        let mut b = app();
        b.open(&b_path);
        assert_eq!(b.session.program, a.session.program);
        assert_eq!(a.path, b_path.display().to_string());
        // the save window goes to the folder of the program and proposes its name
        a.open_window("save");
        match a.win.as_ref() {
            Some(Win::Save(br)) => {
                assert_eq!(br.dir.as_deref(), Some(d.as_path()));
                assert_eq!(br.name_text, "b.nc");
            }
            _ => panic!("the save window did not open"),
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_settings_reach_the_machine_the_language_and_the_theme() {
        let mut a = app();
        a.settings.travel[0] = (0.0, 100.0);
        a.settings.lang = Lang::De;
        a.settings.theme = ThemeId::Paper;
        a.apply_settings();
        assert_eq!((a.lang, a.theme.id), (Lang::De, ThemeId::Paper));
        a.session.mdi("G0 X150");
        assert!(a.session.alarm(), "the limit did not reach the machine");
        // the windows open by their names
        for (name, ok) in [
            ("settings", true),
            ("open", true),
            ("about", true),
            ("edit", true),
            ("nothing", false),
        ] {
            a.win = None;
            a.open_window(name);
            assert_eq!(a.win.is_some(), ok, "{name}");
        }
    }
}
