//! The menu bar, the settings window, the Open and Save windows (made over `eh3d-filedialog`, the same state every program of the suite
//! uses), the editor of the program and the About window.

use eh3d_dock::about::{AboutBrand, Phoenix};
use eh3d_dock::gui::about_contents;
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
    Edit(Box<crate::editor::EditState>),
    About,
    /// A program was left in the middle: go on from its line or leave it.
    Resume(crate::recovery::Recovery),
    /// The summary of a program that is about to start: start it or leave it.
    Summary(Box<summary::PendingStart>),
    /// The list of the keys.
    Shortcuts,
    /// The question before the probe moves the machine.
    Probe(probe::Ask),
    /// What the checkers find in the program that is loaded.
    Check(Box<check::CheckState>),
    /// The editor of the keep-out boxes.
    KeepOut(Box<keepout::KeepState>),
    /// The statistics of the program and its simulation.
    Sim(Box<sim::SimState>),
    /// Between two runs of a program of the queue: the operator places the work and confirms.
    Between(usize),
    /// The question before a macro runs (its name).
    Macro(String),
    /// The list of the macros.
    Macros,
    /// Shift, turn, scale and mirror the program.
    Transform(Box<xform::XformState>),
    /// The bookmarks and the start from a line.
    Lines(Box<lines::LinesState>),
    /// The machine on the EH3D server: its position, a jog, the emergency stop and the release of its latch.
    Machine(Box<machine::MachineState>),
}

/// The settings as they are being changed; they take effect with Ok.
pub(super) struct Draft {
    pub s: Settings,
    pub section: usize,
    /// The editors of the tool changer table and of the library of materials, inside the settings window.
    pub mats: prefs::MatEdit,
}

impl App {
    pub(super) fn browser(&mut self, mode: Mode) -> Box<Browser> {
        self.browser_for(mode, Purpose::Program)
    }

    /// A file window for what the file is for: a G-code program, a height map or the session log.
    pub(super) fn browser_for(&mut self, mode: Mode, purpose: Purpose) -> Box<Browser> {
        self.file_purpose = purpose;
        let program = purpose == Purpose::Program;
        let start = if mode == Mode::Save && program && !self.path.is_empty() {
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
        let extensions: &[&str] = match purpose {
            Purpose::Program => &GCODE_EXTENSIONS,
            Purpose::MapSave | Purpose::MapLoad => &["map", "txt"],
            Purpose::LogSave => &["txt", "csv", "json", "log"],
            Purpose::ReportSave => &["html", "htm"],
            Purpose::SuiteOpen | Purpose::SuiteSaveAs | Purpose::SuiteExtract => &["eh3d"],
            Purpose::SuiteImport => &[
                "nc", "ngc", "gcode", "gco", "tap", "cnc", "dxf", "svg", "plt", "hpgl", "hgl",
            ],
        };
        let default_ext = match purpose {
            Purpose::Program => "nc",
            Purpose::MapSave | Purpose::MapLoad => "map",
            Purpose::LogSave => "txt",
            Purpose::ReportSave => "html",
            Purpose::SuiteOpen | Purpose::SuiteSaveAs | Purpose::SuiteImport => "eh3d",
            Purpose::SuiteExtract => "",
        };
        let mut b = Browser::new(
            mode,
            &start,
            &base,
            vec![Filter::new(extensions), Filter::any()],
            // a folder that a branch is written into has no extension of its own
            (!default_ext.is_empty()).then_some(default_ext),
        );
        if mode == Mode::Save {
            b.name_text = match purpose {
                // a program that came from a project is offered under its own name, never as the project
                Purpose::Program if crate::suitecore::is_project(Path::new(&self.path)) => {
                    self.suite_suggested_name().replace(".eh3d", ".nc")
                }
                Purpose::Program => Path::new(&self.path).file_name().map_or_else(
                    || "program.nc".to_string(),
                    |n| n.to_string_lossy().to_string(),
                ),
                Purpose::MapSave => "heightmap.map".to_string(),
                Purpose::LogSave => "session-log.txt".to_string(),
                Purpose::ReportSave => Path::new(&self.path).file_stem().map_or_else(
                    || "work-report.html".to_string(),
                    |n| format!("{}-report.html", n.to_string_lossy()),
                ),
                Purpose::SuiteSaveAs => self.suite_suggested_name(),
                Purpose::SuiteExtract => self.suite_extract_name(),
                Purpose::MapLoad | Purpose::SuiteOpen | Purpose::SuiteImport => String::new(),
            };
        }
        Box::new(b)
    }

    /// Opens a window by its name (for the pictures and the command line).
    pub(super) fn open_window(&mut self, name: &str) {
        self.win = match name {
            "settings" | "settings-checks" | "settings-probe" | "settings-queue"
            | "settings-atc" | "settings-materials" => Some(Win::Settings(Box::new(Draft {
                s: self.settings.clone(),
                section: match name {
                    "settings-checks" => prefs::FIRST_SECTION,
                    "settings-probe" => prefs::FIRST_SECTION + 1,
                    "settings-queue" => prefs::FIRST_SECTION + 2,
                    "settings-atc" => prefs::FIRST_SECTION + 3,
                    "settings-materials" => prefs::FIRST_SECTION + 4,
                    _ => 1,
                },
                mats: Default::default(),
            }))),
            "open" => Some(Win::Open(self.browser(Mode::Open))),
            "save" => Some(Win::Save(self.browser(Mode::Save))),
            "edit" => Some(Win::Edit(self.edit_state())),
            "about" => Some(Win::About),
            "summary" => Some(Win::Summary(Box::new(summary::PendingStart {
                report: self.report(),
                server: false,
            }))),
            "keys" => Some(Win::Shortcuts),
            "check" => Some(Win::Check(Box::new(self.check_state()))),
            "keepout" => Some(Win::KeepOut(Box::default())),
            "stats" => Some(Win::Sim(Box::new(self.sim_state()))),
            "macros" => Some(Win::Macros),
            "transform" => Some(Win::Transform(Box::default())),
            "bookmarks" => Some(Win::Lines(Box::default())),
            "machine" => Some(Win::Machine(Box::new(self.machine_state()))),
            "probe" => Some(Win::Probe(probe::Ask::Z)),
            "probe-grid" => Some(Win::Probe(probe::Ask::Grid {
                area: [0.0, 0.0, 50.0, 40.0],
                nx: 6,
                ny: 5,
            })),
            "resume" => Some(Win::Resume(crate::recovery::Recovery {
                file: "job.nc".to_string(),
                next: 120,
                total: 400,
                ..Default::default()
            })),
            _ => None,
        };
    }

    /// Saves the program as it is now, as text.
    pub(super) fn save_program(&mut self, path: &Path) -> Result<(), String> {
        if crate::suitecore::is_project(path) {
            let why = crate::suitetext::S::NotOverProject.tr(self.lang.code());
            return Err(format!("{}: {why}", path.display()));
        }
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
        self.recent_added.push(path.to_string_lossy().to_string());
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
        // the library may have been edited: the material in use is looked for again
        self.materials = crate::materials::load(&self.settings_path);
        self.settings.resolve_material(&self.materials);
        // the same for the table of slots of the tool changer
        self.settings
            .resolve_atc_table(crate::atc::load_table(&self.settings_path));
        self.settings
            .resolve_tool_table(crate::tools::load(&self.settings_path));
        if self.theme.id != self.settings.theme {
            self.theme = Theme::for_id(self.settings.theme);
        }
        // the time estimate follows the acceleration of the settings
        self.path_extras.accel_mm_s2 = self.settings.tuning.accel_mm_s2;
        let travel = self.settings.travel;
        self.session.set_travel(travel);
        if self.settings.read_controller_travel && !self.session.travel_asked {
            self.session.read_controller_travel();
        }
    }

    // ------------------------------------------------------------------------------------------------------------ the windows

    /// Draws the window that is open, if there is one.
    pub(super) fn windows(&mut self, ctx: &egui::Context) {
        let Some(win) = self.win.take() else { return };
        let next = match win {
            Win::Settings(d) => self.settings_window(ctx, d),
            Win::Open(b) => self.file_window(ctx, b, Mode::Open),
            Win::Save(b) => self.file_window(ctx, b, Mode::Save),
            Win::Replace(b, p) => self.replace_window(ctx, b, p),
            Win::Edit(s) => self.edit_window(ctx, s),
            Win::About => self.about_window(ctx),
            Win::Resume(r) => self.resume_window(ctx, r),
            Win::Summary(p) => self.summary_window(ctx, p),
            Win::Shortcuts => self.shortcuts_window(ctx),
            Win::Probe(a) => self.probe_window(ctx, a),
            Win::Check(c) => self.check_window(ctx, c),
            Win::KeepOut(k) => self.keepout_window(ctx, k),
            Win::Sim(s) => self.sim_window(ctx, s),
            Win::Between(i) => self.between_window(ctx, i),
            Win::Macro(n) => self.macro_window(ctx, n),
            Win::Macros => self.macros_list_window(ctx),
            Win::Transform(s) => self.xform_window(ctx, s),
            Win::Lines(s) => self.lines_window(ctx, s),
            Win::Machine(s) => self.machine_window(ctx, s),
        };
        // a window that opened another one while it closed (the end of a run asks for the summary of the next) leaves that one open
        if next.is_some() || self.win.is_none() {
            self.win = next;
        }
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
                    if self.session.resume_at_safe(
                        r.next,
                        self.settings.resume_safe_z_mm,
                        self.settings.resume_spindle_wait_s,
                    ) {
                        self.recovery_saved = Some(r.next);
                    } else {
                        // the point it reached cannot be rebuilt (relative distances): nothing is safe to go on from
                        self.forget_recovery();
                    }
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
                let mut info = Phoenix::Cnc.info(env!("CARGO_PKG_VERSION"));
                info.extra.push(self.t(Text::McAboutBody).to_string());
                about_contents(ui, self.lang, &info, &about_brand());
            });
        open.then_some(Win::About)
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
            Some(true) => {
                self.file_chosen(Mode::Save, &path);
                None
            }
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
        let program = self.file_purpose == Purpose::Program;
        let open_prog = mode == Mode::Open && program;
        let title = match self.purpose_title() {
            Some(t) => t,
            None if mode == Mode::Open => self.t(Text::McOpenTitle),
            None => self.t(Text::McSaveTitle),
        };
        let first_filter = self.first_filter_label();
        let mut done: Option<Chosen> = None;
        // the files picked with Ctrl+click to run one after the other (the Open window only)
        let mut multi: Vec<PathBuf> = std::mem::take(&mut self.open_multi);
        let mut ctrl_click = false;
        let mut queue_chosen: Option<Vec<PathBuf>> = None;
        let mut cancel = false;
        let mut error: Option<&'static str> = None;
        let previewing = open_prog && self.settings.open_preview;
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
                        if open_prog && !recent.is_empty() {
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
                                Some(path) if ctrl_click && open_prog => {
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
                            first_filter
                        } else {
                            self.t(Text::FilterAny)
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut filter, 0, first_filter);
                            ui.selectable_value(&mut filter, 1, self.t(Text::FilterAny));
                        });
                    if filter != b.filter {
                        b.set_filter(filter);
                    }
                    let preview_label = self.t(Text::PreviewShow);
                    if open_prog
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
                    if open_prog && multi.len() >= 2 {
                        ui.label(format!("{}: {}", self.t(Text::McQueue), multi.len()));
                    }
                    if ui.button(label).clicked() || enter {
                        if open_prog && multi.len() >= 2 {
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
                self.file_chosen(mode, &p);
                None
            }
            Some(Chosen::Overwrite(p)) => {
                if mode == Mode::Open
                    || matches!(
                        self.file_purpose,
                        Purpose::LogSave
                            | Purpose::ReportSave
                            | Purpose::SuiteSaveAs
                            | Purpose::SuiteExtract
                    )
                {
                    // the log and the report are never written over a file: the export says so
                    self.file_chosen(mode, &p);
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

    /// The settings: the sections on the left as a tree, the section on the right, Ok and Cancel.
    fn settings_window(&mut self, ctx: &egui::Context, mut d: Box<Draft>) -> Option<Win> {
        let mut ok = false;
        let mut cancel = false;
        let mut pane = self.server_pane.take().unwrap_or_else(|| {
            crate::serverlink::ServerPane::new(&self.settings, self.server_login.clone())
        });
        self.window(self.t(Text::SettingsTitle))
            .default_size([620.0, 460.0])
            .show(ctx, |ui| {
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(150.0);
                        let tools = self.tool_sections();
                        for (i, n) in [
                            self.t(Text::McSecGeneral),
                            self.t(Text::McSecInterface),
                            self.t(Text::McSecAxes),
                            self.t(Text::McSecJog),
                            self.t(Text::McSecServer),
                            self.t(Text::McSecSafety),
                            tools[0],
                            tools[1],
                            tools[2],
                            tools[3],
                            tools[4],
                        ]
                        .into_iter()
                        .enumerate()
                        {
                            if ui.selectable_label(d.section == i, n).clicked() {
                                d.section = i;
                            }
                        }
                    });
                    ui.add_space(8.0);
                    ui.vertical(|ui| {
                        ui.set_min_width(420.0);
                        match d.section {
                            0 => {
                                self.set_profiles(ui, &mut d);
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
                                // the panel of the suite: the boxes, the state of the link, the login and the logout
                                pane.refresh();
                                if let Some(action) =
                                    eh3d_view::server_gui::show(ui, &mut pane.panel, d.s.lang)
                                {
                                    pane.handle(action, &mut d.s);
                                }
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
                            prefs::FIRST_SECTION..=prefs::LAST_SECTION => {
                                egui::ScrollArea::vertical()
                                    .id_salt("set-tools-scroll")
                                    .max_height(340.0)
                                    .show(ui, |ui| self.settings_tools(ui, &mut d));
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
            pane.reset(&self.settings);
            self.server_pane = Some(pane);
            return None;
        }
        if cancel {
            pane.reset(&self.settings);
            self.server_pane = Some(pane);
            return None;
        }
        // the buttons of the tool changer and of the materials act on files, which needs the whole program
        self.tool_file_actions(&mut d);
        self.server_pane = Some(pane);
        Some(Win::Settings(d))
    }
}

/// The name of a language, in that language.
pub(super) fn lang_name(l: Lang) -> &'static str {
    l.native_name()
}

/// The look of the About window: which program this is and its own picture, drawn in a ring of its colour.
fn about_brand() -> AboutBrand {
    AboutBrand {
        program: Phoenix::Cnc,
        logo_png: super::ICON_PNG,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        App::new(&Options::default())
    }

    fn work_folder(tag: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("phoenix-cnc-dialogs-{tag}-{}", std::process::id()));
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
    fn tab_in_the_command_box_finishes_the_code_and_lists_the_ones_that_fit() {
        let mut a = App::isolated();
        a.mdi = "G38".to_string();
        assert!(a.complete_command());
        assert_eq!(a.mdi, "G38.");
        assert_eq!(a.mdi_options, ["G38.2", "G38.3", "G38.4", "G38.5"]);
        // nothing to add: the box is the same and the list stays for the operator to read
        a.mdi = "G5".to_string();
        assert!(!a.complete_command());
        assert_eq!(a.mdi_options.len(), 7);
        // one code that fits is written in full and the list goes
        a.mdi = "M30".to_string();
        assert!(a.complete_command());
        assert_eq!((a.mdi.as_str(), a.mdi_options.len()), ("M30 ", 0));
        // nothing is sent by completing
        assert!(a.session.log.iter().all(|l| !l.contains("M30")));
        a.mdi = String::new();
        assert!(!a.complete_command());
    }

    #[test]
    fn every_window_the_command_line_names_opens() {
        let mut a = App::isolated();
        for name in crate::cli::WINDOWS {
            a.win = None;
            a.open_window(name);
            assert!(a.win.is_some(), "{name} did not open");
        }
    }

    #[test]
    fn the_window_about_has_the_name_the_picture_and_draws_in_every_language() {
        let brand = about_brand();
        assert_eq!(brand.program.name(), "EH3D-PHOENIX-CNC");
        assert_eq!(brand.logo_png.get(1..4), Some(&b"PNG"[..]));
        let mut a = app();
        let ctx = egui::Context::default();
        for lang in Lang::ALL {
            a.lang = lang;
            a.open_window("about");
            let mut kept = None;
            let out = ctx.run_ui(egui::RawInput::default(), |ui| {
                kept = a.about_window(ui.ctx());
            });
            out.drop_without_applying_deltas();
            assert!(matches!(kept, Some(Win::About)), "{lang:?}");
        }
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
            ("settings-checks", true),
            ("settings-probe", true),
            ("settings-queue", true),
            ("settings-atc", true),
            ("settings-materials", true),
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

    #[test]
    fn the_material_chosen_in_the_settings_reaches_the_checks_of_the_program() {
        let mut a = App::isolated();
        a.session.load("M3 S12000\nG1 X5 F20\nM30");
        let found = |a: &App| {
            a.program_notes()
                .iter()
                .any(|n| n.text.contains("suggested"))
        };
        assert!(!found(&a));
        // the library shipped has aluminium, which holds the feed above 150 mm/min
        a.settings.material = "Aluminium".to_string();
        a.apply_settings();
        assert!(!a.materials.is_empty());
        assert!(a.settings.lint.material_feed.is_some());
        assert!(found(&a));
        // none puts the ranges away again
        a.settings.material.clear();
        a.apply_settings();
        assert!(a.settings.lint.material_feed.is_none() && !found(&a));
    }

    #[test]
    fn a_machine_profile_asked_for_on_the_command_line_is_said_when_it_is_not_there() {
        let o = Options {
            profile: Some("no such machine profile 7q".to_string()),
            ..Options::default()
        };
        let a = App::new(&o);
        assert!(
            a.session
                .log
                .iter()
                .any(|l| l.contains("no such machine profile 7q")),
            "{:?}",
            a.session.log
        );
        assert!(!App::new(&Options::default())
            .session
            .log
            .iter()
            .any(|l| l.contains("7q")));
    }
}
