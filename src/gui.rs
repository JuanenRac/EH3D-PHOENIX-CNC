//! The window of the machine-control program: the position of the machine, the jog, the program and its path, the line of commands and
//! the log, over a controller (the simulated one for now). The words are the ones of `eh3d-view` in the eight languages and the colours
//! are the ones of its themes.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use eframe::egui::{
    self, Align2, Color32, FontData, FontDefinitions, FontFamily, Pos2, Rect, RichText, Stroke,
    StrokeKind, Vec2,
};
use eh3d_attachpanel::{AttachPanel, Program};
use eh3d_controller::sim::SimController;
use eh3d_controller::{Family, State};
use eh3d_view::texts::{tr, Lang, Text};
use eh3d_view::theme::{Rgb, Theme, ThemeId};

use crate::cli::Options;
use crate::guard::GuardHandle;
use crate::motioncfg::{motion_path, MotionCfg};
use crate::queue::Queue;
use crate::recovery::{self, Recovery};
use crate::settings::{settings_path, Settings};

mod check;
mod clipmenu;
mod dialogs;
mod dockpanels;
mod editwin;
mod files;
mod icons;
mod keepout;
mod keys;
mod lines;
mod machine;
mod macros;
mod numfield;
mod pathview;
mod prefs;
mod probe;
mod queue;
mod remote;
mod shell;
mod sim;
mod suite;
mod summary;
mod views;
mod xform;
use crate::gcode::{self, Segment};
use crate::session::{Session, STEPS};

/// How many lines of the program are handed to the thread that sends them at a time.
const PUMP_BATCH: usize = 8;
/// Under this height the panel of the state draws its rows smaller, so that the codes in force stay in sight.
const COMPACT_STATE_BELOW: f32 = 440.0;
/// The size of a button of the jog pad with all the room it wants.
const PAD_BUTTON: Vec2 = Vec2::new(52.0, 38.0);
use crate::view::{Orbit, Plan2d};
use dialogs::{lang_name, Win};
use files::Purpose;

fn col(c: Rgb) -> Color32 {
    Color32::from_rgb(c.0, c.1, c.2)
}

fn alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

/// The colours of egui made from the theme.
fn apply_visuals(ctx: &egui::Context, t: &Theme) {
    ctx.set_visuals(visuals_of(t));
}

fn visuals_of(t: &Theme) -> egui::Visuals {
    // what is not set here (the title bar of a window, the bars of the tabs, the lines) comes from the base of the same lightness
    let mut v = if t.is_light() {
        egui::Visuals::light()
    } else {
        egui::Visuals::dark()
    };
    let (panel, raised, text, accent) = (
        col(t.panel),
        col(t.panel_raised),
        col(t.text),
        col(t.accent),
    );
    v.panel_fill = col(t.background.0);
    v.window_fill = panel;
    v.extreme_bg_color = raised;
    v.override_text_color = Some(text);
    v.selection.bg_fill = alpha(accent, 110);
    v.selection.stroke = Stroke::new(1.5, accent);
    for w in [
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
    ] {
        w.corner_radius = 6.0.into();
        w.bg_fill = raised;
        w.weak_bg_fill = raised;
    }
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, alpha(accent, 110));
    v.widgets.hovered.bg_stroke = Stroke::new(1.5, accent);
    v.widgets.hovered.weak_bg_fill = alpha(accent, 50);
    v.widgets.active.bg_stroke = Stroke::new(2.0, accent);
    v.widgets.active.weak_bg_fill = alpha(accent, 110);
    // words that stand out (`strong()`) do not take `override_text_color`: egui draws them in this colour
    v.widgets.active.fg_stroke.color = text;
    v
}

fn install_fonts(ctx: &egui::Context) {
    let candidates = [
        "C:/Windows/Fonts/msyh.ttc",
        "C:/Windows/Fonts/YuGothM.ttc",
        "C:/Windows/Fonts/meiryo.ttc",
        "C:/Windows/Fonts/simsun.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        "/System/Library/Fonts/PingFang.ttc",
    ];
    for path in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            let mut fonts = FontDefinitions::default();
            fonts
                .font_data
                .insert("cjk".into(), Arc::new(FontData::from_owned(bytes)));
            for family in [FontFamily::Monospace, FontFamily::Proportional] {
                fonts.families.entry(family).or_default().push("cjk".into());
            }
            ctx.set_fonts(fonts);
            return;
        }
    }
}

/// The icon of the program, for the title bar, the task bar and the About window.
const ICON_PNG: &[u8] = include_bytes!("../images/brand/icon-256.png");

/// Decodes the icon (a PNG of 8 bits per channel with alpha); `None` when it is anything else.
fn decode_icon() -> Option<egui::IconData> {
    let decoder = png::Decoder::new(std::io::Cursor::new(ICON_PNG));
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return None;
    }
    buf.truncate(info.buffer_size());
    Some(egui::IconData {
        rgba: buf,
        width: info.width,
        height: info.height,
    })
}

fn save_png(img: &egui::ColorImage, path: &Path) -> std::io::Result<()> {
    let file = std::io::BufWriter::new(std::fs::File::create(path)?);
    let mut enc = png::Encoder::new(file, img.size[0] as u32, img.size[1] as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut w = enc.write_header().map_err(std::io::Error::other)?;
    let data: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
    w.write_image_data(&data).map_err(std::io::Error::other)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PosTab {
    Work,
    Machine,
    Operator,
    GCode,
}

/// The little pictures of the buttons and the rows.
#[derive(Clone, Copy)]
enum Icon {
    Cross,
    ToZero,
    Home,
    Gauge,
    Spindle,
}

struct Shot {
    file: PathBuf,
    wait: u32,
    done: Arc<AtomicBool>,
}

pub struct App {
    session: Session,
    lang: Lang,
    theme: Theme,
    visuals_for: Option<ThemeId>,
    pos_tab: PosTab,
    info_tab: u8,
    settings: Settings,
    settings_path: PathBuf,
    win: Option<Win>,
    fit_program: bool,
    quit: bool,
    dialog_error: Option<&'static str>,
    mdi: String,
    /// The codes that fit what was typed in the command box when Tab found several (shown beside the box).
    mdi_options: Vec<&'static str>,
    path: String,
    path_toolpath: Vec<Segment>,
    /// The length of the arcs and the tool changes of the program, which the estimate of time adds to the path.
    path_extras: crate::sim::Extras,
    /// The program as it was before the last transformation and as the transformation left it, for Undo.
    transform_undo: Option<(Vec<String>, Vec<String>)>,
    /// How the viewer of the path colours it, which tools it shows and which move is selected.
    viewer: crate::viewer::State,
    frame: u32,
    start_at: Option<u32>,
    shot: Option<Shot>,
    /// The line at which the recovery file was last written (None: nothing is saved for the program that runs).
    recovery_saved: Option<usize>,
    /// The job on the EH3D server that is followed, what the server said of it the last time, and the last state and error that were written in the log.
    server: Option<crate::server::Follow>,
    server_snap: crate::server::Snapshot,
    server_key: Option<(String, String)>,
    server_err: Option<String>,
    /// The login the panel of the settings makes and the panel itself: the job that is sent to the server uses that login.
    server_login: crate::serverlink::LoginStore,
    server_pane: Option<crate::serverlink::ServerPane>,
    /// How the path is looked at: the zoom and shift of the plan, the turn and zoom of the 3D view.
    plan2d: Plan2d,
    orbit: Orbit,
    /// The file the Open window shows a picture of, and the picture (None: it cannot be drawn).
    preview: Option<(PathBuf, Option<gcode::Preview>)>,
    /// The programs that run one after the other, and whether the one that is loaded has been seen running.
    queue: Queue,
    queue_seen: bool,
    /// The end of the wait between two runs of a program (None: not waiting), and the text of the queue as it was last written to its file.
    queue_gate: Option<std::time::Instant>,
    queue_saved: String,
    /// The files picked with Ctrl+click in the Open window.
    open_multi: Vec<PathBuf>,
    /// A line of the program that a window asked to show (the Check window, the simulation), and whether the panel still has to scroll to it.
    focus_line: Option<usize>,
    /// The library of materials (the file of the operator or the one shipped), for the choice in the settings.
    materials: Vec<crate::materials::Material>,
    /// The bookmarks on the lines of the program that is loaded.
    bookmarks: crate::bookmarks::Bookmarks,
    focus_scroll: bool,
    /// What the file window that is open is for, the session log kept for export, how much of the log of the session it has taken, and the start of the session.
    file_purpose: Purpose,
    logbook: crate::logbook::Logbook,
    log_seen: usize,
    started: std::time::Instant,
    /// The estimates of time that are kept, and the line and the point of the path the simulation is at (None: no simulation).
    estimate: crate::estimate::Cache,
    sim_line: Option<usize>,
    sim_pos: Option<gcode::Point>,
    /// Whether a box of text, or any widget, had the keyboard at the end of the last frame.
    typing_prev: bool,
    focus_prev: bool,
    /// The values of the motion file (the planner, the guard, the importers, the tool setter) and the lines of it that could not be used.
    motion: MotionCfg,
    /// The guard of the controller, when the motion file has it on, and why it stopped the machine the last time it could be asked.
    guard: Option<GuardHandle>,
    guard_trip: Option<crate::guard::Trip>,
    /// What the panels of the docking window ask of the frame: panels to show, and files that were opened (for the list of recent files).
    pending_dock: Vec<eh3d_dock::menu::DockAction>,
    recent_added: Vec<String>,
    /// The project file of the suite (`.eh3d`) the window works on.
    suite: suite::SuiteState,
    /// The panel of the attachments of that project.
    attachments: AttachPanel,
    /// The state of the panels of the import, the time and the tool setter.
    import_ui: dockpanels::ImportUi,
    plan_ui: Option<crate::workflow::TimeCompare>,
    setter_ui: dockpanels::SetterUi,
}

impl App {
    /// A window for the tests that keeps its settings in a file of its own, so that what one test saves (the recent files, a setting it turned
    /// off) does not reach the others.
    #[cfg(test)]
    pub(crate) fn isolated() -> App {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let mut a = App::new(&Options::default());
        let n = N.fetch_add(1, Ordering::SeqCst);
        a.settings_path = std::env::temp_dir().join(format!(
            "eh3d-phoenix-cnc-isolated-{}-{n}.cfg",
            std::process::id()
        ));
        a.settings = Settings::default();
        a
    }

    pub fn new(o: &Options) -> App {
        App::with_controller(o, Box::new(SimController::new()))
    }

    /// The window over the controller that is given (the simulator, or the one the options asked for).
    pub fn with_controller(o: &Options, controller: crate::session::BoxedController) -> App {
        let path = settings_path();
        let mut settings = Settings::load(&path);
        let (motion, motion_problems) = MotionCfg::read(&motion_path(&path));
        if let Some(l) = o.lang {
            settings.lang = l;
        }
        if let Some(th) = o.theme {
            settings.theme = th;
        }
        // the values of a machine profile asked for on the command line; one that is not saved is said in the log
        let profile_missing = o
            .profile
            .as_ref()
            .filter(|name| !settings.use_profile(name))
            .cloned();
        if let Some(v) = o.view_3d {
            settings.view_3d = v;
        }
        let materials = crate::materials::load(&path);
        settings.resolve_material(&materials);
        settings.resolve_atc_table(crate::atc::load_table(&path));
        settings.resolve_tool_table(crate::tools::load(&path));
        let settings_log_max = settings.tuning.log_max;
        let mut session = Session::new(controller);
        session.step_mm = settings.step_mm;
        session.feed_mm_min = settings.feed_mm_min;
        session.cancel_crossing_jog = motion.guard.jog_cancel_crossing;
        session.resume_plunge_feed_mm_min = settings.resume_plunge_feed_mm_min;
        session.set_travel(settings.travel);
        if settings.read_controller_travel {
            session.read_controller_travel();
        }
        let panel_limits = settings.panel_limits();
        let mut app = App {
            session,
            lang: settings.lang,
            theme: Theme::for_id(settings.theme),
            visuals_for: None,
            pos_tab: PosTab::Work,
            info_tab: 0,
            settings,
            settings_path: path,
            win: None,
            fit_program: false,
            quit: false,
            dialog_error: None,
            mdi: String::new(),
            mdi_options: Vec::new(),
            path: String::new(),
            path_toolpath: Vec::new(),
            path_extras: crate::sim::Extras::default(),
            transform_undo: None,
            viewer: crate::viewer::State::default(),
            frame: 0,
            start_at: o.start.then_some(4),
            recovery_saved: None,
            server: None,
            server_snap: crate::server::Snapshot::default(),
            server_key: None,
            server_err: None,
            server_login: crate::serverlink::LoginStore::new(),
            server_pane: None,
            plan2d: Plan2d::default(),
            orbit: Orbit::default(),
            preview: None,
            queue: Queue::default(),
            queue_seen: false,
            queue_gate: None,
            queue_saved: String::new(),
            open_multi: Vec::new(),
            focus_line: None,
            materials,
            bookmarks: crate::bookmarks::Bookmarks::default(),
            focus_scroll: false,
            file_purpose: Purpose::Program,
            logbook: crate::logbook::Logbook::new(settings_log_max),
            log_seen: 0,
            started: std::time::Instant::now(),
            estimate: crate::estimate::Cache::new(64),
            sim_line: None,
            sim_pos: None,
            typing_prev: false,
            focus_prev: false,
            motion,
            guard: None,
            guard_trip: None,
            pending_dock: Vec::new(),
            recent_added: Vec::new(),
            suite: suite::SuiteState::default(),
            attachments: AttachPanel::new(Program::new(
                crate::eh3dfile::BRANCH,
                env!("CARGO_PKG_VERSION"),
            )),
            import_ui: dockpanels::ImportUi::default(),
            plan_ui: None,
            setter_ui: dockpanels::SetterUi::default(),
            shot: o.shot.clone().map(|file| Shot {
                file,
                wait: o.wait,
                done: Arc::new(AtomicBool::new(false)),
            }),
        };
        app.attachments.set_limits(panel_limits);
        if let Some(f) = &o.file {
            if o.more.is_empty() {
                app.open(f);
            } else {
                let mut all = vec![f.clone()];
                all.extend(o.more.iter().cloned());
                app.start_queue(all);
            }
        }
        // the pictures: the way the path is coloured and the move that is selected
        if let Some(p) = o.view_preset {
            app.orbit.preset(p);
        }
        if let Some(c) = o.color {
            app.viewer.color_by = c;
        }
        if let Some(n) = o.select {
            let i = crate::viewer::segment_at_line(&app.path_toolpath, n);
            app.select_segment(i);
        }
        // a program that was left in the middle by a crash or by a link that went down: the operator chooses to go on or to leave it
        if o.file.is_none() && o.window.is_none() && o.shot.is_none() && !cfg!(test) {
            if let Some(r) = Recovery::load(&recovery::recovery_path(&app.settings_path)) {
                if Path::new(&r.file).is_file() {
                    app.win = Some(dialogs::Win::Resume(r));
                } else {
                    Recovery::clear(&recovery::recovery_path(&app.settings_path));
                }
            }
        }
        // the queue of the last run comes back waiting: nothing starts until the operator says so
        if o.file.is_none() && o.window.is_none() && o.shot.is_none() && !cfg!(test) {
            app.restore_queue();
        }
        // the lines of the motion file that could not be used are said in the log, with the name of the file
        for p in &motion_problems {
            let said = format!(
                "{}: {}",
                motion_path(&app.settings_path).display(),
                p.text(app.lang.code())
            );
            app.session.log.push(said);
        }
        if let Some(name) = profile_missing {
            let said = app.ws(crate::words::Msg::ProfileUnknown, &[name]);
            app.session.log.push(said);
        }
        if let Some(w) = &o.window {
            app.open_window(w);
        }
        app
    }

    /// The window over the controller of the link: the guard it is in, when the motion file has it on, is kept to be told the time.
    pub fn with_link(o: &Options, linked: crate::link::Linked) -> App {
        let mut app = App::with_controller(o, linked.controller);
        app.guard = linked.guard;
        app
    }

    fn t(&self, text: Text) -> &'static str {
        tr(self.lang, text)
    }

    /// Keeps the line the program has reached in a file while it runs, and takes the file away when the program is over: a crash leaves it for the next start.
    fn track_recovery(&mut self) {
        let path = recovery::recovery_path(&self.settings_path);
        let s = &self.session;
        if s.running && !self.path.is_empty() && !cfg!(test) {
            if recovery::due(self.recovery_saved, s.next, 20) {
                let r = Recovery {
                    file: self.path.clone(),
                    // the controller may not have run the last lines that were sent (it keeps some in its buffer): the program is
                    // taken up again a few lines before, which is safe in absolute distances
                    next: recovery::backed_off(
                        &s.program,
                        s.next,
                        self.settings.resume_backoff_lines as usize,
                    ),
                    total: s.program.len(),
                    job: String::new(),
                    sha256: recovery::program_hash(&s.program),
                };
                if r.save(&path).is_ok() {
                    self.recovery_saved = Some(s.next);
                }
            }
        } else if self.recovery_saved.is_some() && s.next >= s.program.len() {
            // the program reached its end
            Recovery::clear(&path);
            self.recovery_saved = None;
        }
    }

    /// The operator stopped the program on purpose: nothing is left to go on from.
    fn forget_recovery(&mut self) {
        Recovery::clear(&recovery::recovery_path(&self.settings_path));
        self.recovery_saved = None;
    }

    /// Takes a G-code file as the program, by itself: a queue that was there is dropped.
    pub fn open(&mut self, path: &Path) {
        self.queue = Queue::default();
        self.queue_seen = false;
        self.queue_gate = None;
        self.load_file(path);
    }

    /// Reads the file as the program; says whether it could.
    fn load_file(&mut self, path: &Path) -> bool {
        if crate::suitecore::is_project(path) {
            return self.load_project(path);
        }
        match std::fs::read(path) {
            Ok(bytes) => {
                let text: String = bytes.iter().map(|&b| b as char).collect();
                self.session.load(&text);
                self.bookmarks.clear();
                self.focus_line = None;
                self.refresh_path();
                self.path = path.display().to_string();
                self.remember(path);
                self.load_marks_for(path);
                true
            }
            Err(e) => {
                let why = format!("{}: {e}", path.display());
                let said = self.t(Text::McOpenFailed).replacen("{}", &why, 1);
                self.session.log.push(said);
                false
            }
        }
    }

    /// Tab in the command box: finishes the code being typed as far as it is certain and keeps the codes that fit, to be listed. Says whether
    /// the text of the box changed.
    fn complete_command(&mut self) -> bool {
        let c = crate::complete::complete(&self.mdi);
        self.mdi_options = c.options;
        let changed = c.text != self.mdi;
        self.mdi = c.text;
        changed
    }

    fn state_text(&self) -> &'static str {
        if self.server_busy() {
            return if self.server_held() {
                self.t(Text::McStHold)
            } else {
                self.t(Text::McStRun)
            };
        }
        if self.session.running && !self.session.paused {
            return self.t(Text::McStRun);
        }
        match self.session.status.state {
            State::Idle => self.t(Text::McStIdle),
            State::Run | State::Jog => self.t(Text::McStRun),
            State::Hold => self.t(Text::McStHold),
            State::Home => self.t(Text::McStHome),
            State::Alarm => self.t(Text::McStAlarm),
            _ => self.t(Text::McStUnknown),
        }
    }

    fn top(&mut self, ui: &mut egui::Ui) {
        let accent = col(self.theme.accent);
        egui::Panel::top("top").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading(RichText::new("EH3D").color(col(self.theme.text)));
                ui.heading(RichText::new("PHOENIX-CNC").color(accent));
                ui.separator();
                ui.label(self.t(Text::McController));
                egui::ComboBox::from_id_salt("controller")
                    .selected_text(self.t(Text::McSimulator))
                    .show_ui(ui, |ui| {
                        let _ = ui.selectable_label(true, self.t(Text::McSimulator));
                        for f in Family::ALL {
                            if !f.is_driven() {
                                continue;
                            }
                            ui.add_enabled(
                                false,
                                egui::Button::selectable(false, format!("{} (serial)", f.name())),
                            );
                        }
                        for f in Family::ALL {
                            if f.is_driven() {
                                continue;
                            }
                            ui.add_enabled(
                                false,
                                egui::Button::selectable(
                                    false,
                                    format!("{} - {}", f.name(), self.t(Text::McNotDriven)),
                                ),
                            );
                        }
                    });
                ui.separator();
                let mut lang = self.lang;
                egui::ComboBox::from_id_salt("lang")
                    .selected_text(lang_name(lang))
                    .show_ui(ui, |ui| {
                        for l in Lang::ALL {
                            ui.selectable_value(&mut lang, l, lang_name(l));
                        }
                    });
                let mut id = self.theme.id;
                egui::ComboBox::from_id_salt("theme")
                    .selected_text(self.t(id.text()))
                    .show_ui(ui, |ui| {
                        for i in ThemeId::ALL {
                            ui.selectable_value(&mut id, i, self.t(i.text()));
                        }
                    });
                if lang != self.lang || id != self.theme.id {
                    self.settings.lang = lang;
                    self.settings.theme = id;
                    self.apply_settings();
                    self.save_settings();
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let estop = egui::Button::new(
                        RichText::new(self.t(Text::EmergencyStop).to_uppercase())
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(Color32::from_rgb(200, 30, 40))
                    .min_size(Vec2::new(150.0, 34.0));
                    if ui.add(estop).clicked() {
                        self.session.estop();
                    }
                    let alarm = self.session.alarm();
                    let color = if alarm {
                        col(self.theme.danger)
                    } else {
                        col(self.theme.ok)
                    };
                    ui.label(
                        RichText::new(format!("{}: {}", self.t(Text::McState), self.state_text()))
                            .strong()
                            .color(color),
                    );
                });
            });
        });
    }

    /// The toolbar: the pictures with their names for what is done most (open, save, edit, start, pause, stop, rewind, settings).
    fn toolbar(&mut self, ui: &mut egui::Ui) {
        if !self.settings.show_toolbar {
            return;
        }
        let t = self.theme.clone();
        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.horizontal(|ui| {
                use icons::{tool_button, Glyph};
                if tool_button(
                    ui,
                    &t,
                    Glyph::Open,
                    self.t(Text::OpenFile),
                    self.t(Text::McOpenGcode),
                    true,
                ) {
                    self.open_window("open");
                }
                if tool_button(
                    ui,
                    &t,
                    Glyph::Save,
                    self.t(Text::Save),
                    self.t(Text::McSaveGcode),
                    true,
                ) {
                    self.open_window("save");
                }
                if tool_button(
                    ui,
                    &t,
                    Glyph::Edit,
                    self.t(Text::McEditProgram),
                    self.t(Text::McEditProgram),
                    true,
                ) {
                    self.open_window("edit");
                }
                ui.separator();
                let can = self.can_start() && !self.session.program.is_empty();
                if tool_button(
                    ui,
                    &t,
                    Glyph::Play,
                    self.t(Text::Start),
                    self.t(Text::Start),
                    can,
                ) {
                    self.request_start();
                }
                if tool_button(
                    ui,
                    &t,
                    Glyph::Pause,
                    self.t(Text::Pause),
                    self.t(Text::Pause),
                    self.can_pause(),
                ) {
                    self.program_pause();
                }
                if tool_button(
                    ui,
                    &t,
                    Glyph::Stop,
                    self.t(Text::Stop),
                    self.t(Text::Stop),
                    self.can_stop(),
                ) {
                    self.program_stop();
                }
                if tool_button(
                    ui,
                    &t,
                    Glyph::Rewind,
                    self.t(Text::McRewind),
                    self.t(Text::McRewind),
                    true,
                ) {
                    self.session.rewind();
                }
                ui.separator();
                if tool_button(
                    ui,
                    &t,
                    Glyph::Settings,
                    self.t(Text::SettingsTitle),
                    self.t(Text::SettingsTitle),
                    true,
                ) {
                    self.open_window("settings");
                }
            });
        });
    }

    /// One square button of the column: an icon drawn with lines, a short caption under it and the explanation when the pointer rests on it.
    fn icon_button(&self, ui: &mut egui::Ui, icon: Icon, caption: &str, tip: &str) -> bool {
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(56.0, 52.0), egui::Sense::click());
        let t = &self.theme;
        let p = ui.painter_at(rect);
        let hot = resp.hovered();
        p.rect_filled(
            rect,
            6.0,
            if resp.is_pointer_button_down_on() {
                alpha(col(t.accent), 110)
            } else if hot {
                alpha(col(t.accent), 50)
            } else {
                col(t.panel_raised)
            },
        );
        p.rect_stroke(
            rect,
            6.0,
            Stroke::new(1.0, alpha(col(t.accent), if hot { 200 } else { 90 })),
            StrokeKind::Inside,
        );
        let c = rect.center() - Vec2::new(0.0, 8.0);
        let line = Stroke::new(1.8, col(t.text));
        match icon {
            Icon::Cross => {
                p.circle_stroke(c, 9.0, line);
                p.line_segment([c - Vec2::new(14.0, 0.0), c + Vec2::new(14.0, 0.0)], line);
                p.line_segment([c - Vec2::new(0.0, 14.0), c + Vec2::new(0.0, 14.0)], line);
            }
            Icon::ToZero => {
                p.line_segment([c - Vec2::new(14.0, 0.0), c + Vec2::new(8.0, 0.0)], line);
                p.line_segment([c + Vec2::new(8.0, 0.0), c + Vec2::new(2.0, -6.0)], line);
                p.line_segment([c + Vec2::new(8.0, 0.0), c + Vec2::new(2.0, 6.0)], line);
                p.line_segment([c + Vec2::new(13.0, -9.0), c + Vec2::new(13.0, 9.0)], line);
            }
            Icon::Home => {
                let top = c - Vec2::new(0.0, 12.0);
                p.line_segment([top, c + Vec2::new(-13.0, -1.0)], line);
                p.line_segment([top, c + Vec2::new(13.0, -1.0)], line);
                p.rect_stroke(
                    Rect::from_min_max(c + Vec2::new(-9.0, -1.0), c + Vec2::new(9.0, 11.0)),
                    0.0,
                    line,
                    StrokeKind::Inside,
                );
            }
            Icon::Gauge | Icon::Spindle => {}
        }
        p.text(
            rect.center_bottom() - Vec2::new(0.0, 3.0),
            Align2::CENTER_BOTTOM,
            caption,
            egui::FontId::proportional(11.0),
            col(t.dim),
        );
        resp.on_hover_text(tip).clicked()
    }

    fn icon_column(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            if self.icon_button(ui, Icon::Cross, "XYZ", self.t(Text::McZeroXyz)) {
                self.session.zero_axes(&[0, 1, 2]);
            }
            if self.icon_button(ui, Icon::Cross, "XY", self.t(Text::McZeroXy)) {
                self.session.zero_axes(&[0, 1]);
            }
            if self.icon_button(ui, Icon::Cross, "G28", self.t(Text::McGoG28)) {
                self.session.go_g28();
            }
            if self.icon_button(ui, Icon::Cross, "G30", self.t(Text::McGoG30)) {
                self.session.go_g30();
            }
            ui.separator();
            if self.icon_button(ui, Icon::ToZero, "XY", self.t(Text::McGoZeroXy)) {
                self.session.go_zero_xy();
            }
            if self.icon_button(ui, Icon::ToZero, "Z", self.t(Text::McGoZeroZ)) {
                self.session.go_zero_z();
            }
            ui.separator();
            if self.icon_button(ui, Icon::Home, "", self.t(Text::McHome)) {
                self.session.home();
            }
        });
    }

    /// The position: the tabs and the four axes, big. The letter of an axis sets its zero.
    fn position_block(&mut self, ui: &mut egui::Ui, compact: bool) {
        let (work, machine, operator, gcode) = (
            self.t(Text::McWork),
            self.t(Text::McMachine),
            self.t(Text::McOperator),
            self.t(Text::McGcodeTab),
        );
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.pos_tab, PosTab::Work, work);
            ui.selectable_value(&mut self.pos_tab, PosTab::Machine, machine);
            ui.selectable_value(&mut self.pos_tab, PosTab::Operator, operator);
            ui.selectable_value(&mut self.pos_tab, PosTab::GCode, gcode);
        });
        let st = self.session.status.clone();
        let mut values = match self.pos_tab {
            PosTab::Work | PosTab::GCode => st.work_mm,
            PosTab::Machine => st.machine_mm,
            PosTab::Operator => self.session.operator_mm(),
        };
        // the machine and the program go on in mm; only what is shown changes
        let (decimals, unit) = if self.settings.inches {
            for v in &mut values {
                *v /= 25.4;
            }
            (4, "in")
        } else {
            (3, "mm")
        };
        let tip = self.t(Text::McZeroAxis);
        let (letter_size, value_size, height, margin) = if compact {
            (18.0, 20.0, 22.0, 1)
        } else {
            (26.0, 30.0, 34.0, 3)
        };
        for (i, name) in ["X", "Y", "Z", "A"].iter().enumerate() {
            egui::Frame::new()
                .fill(col(self.theme.panel_raised))
                .corner_radius(6.0)
                .inner_margin(egui::Margin::symmetric(8, margin))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let letter = egui::Button::new(
                            RichText::new(*name)
                                .size(letter_size)
                                .color(col(self.theme.dim)),
                        )
                        .frame(false)
                        .min_size(Vec2::new(30.0, height));
                        if ui.add(letter).on_hover_text(tip).clicked() {
                            match self.pos_tab {
                                PosTab::Machine => {}
                                PosTab::Operator => self.session.zero_operator(&[i]),
                                _ => self.session.zero_axes(&[i]),
                            }
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                RichText::new(format!("{:9.1$} {unit}", values[i], decimals))
                                    .monospace()
                                    .size(value_size)
                                    .color(col(self.theme.accent)),
                            );
                        });
                    });
                });
        }
    }

    /// The speed of the move and the speed of the spindle, each with its little picture; side by side in a low panel.
    fn speed_rows(&mut self, ui: &mut egui::Ui, compact: bool) {
        let st = self.session.status.clone();
        let rows = [
            (Icon::Gauge, self.t(Text::McSpeed), st.feed_mm_min),
            (Icon::Spindle, self.t(Text::McSpindle), st.spindle_rpm),
        ];
        if compact {
            ui.columns(2, |columns| {
                for (column, (icon, tip, v)) in columns.iter_mut().zip(rows) {
                    self.speed_box(column, icon, tip, format!("{v:.2}"), true);
                }
            });
        } else {
            for (icon, tip, v) in rows {
                self.speed_box(ui, icon, tip, format!("{v:.2}"), false);
            }
        }
    }

    fn speed_box(&self, ui: &mut egui::Ui, icon: Icon, tip: &str, value: String, compact: bool) {
        // the picture keeps its shape at the smaller size
        let (scale, value_size, margin) = if compact {
            (0.75, 17.0, 1)
        } else {
            (1.0, 24.0, 3)
        };
        egui::Frame::new()
            .fill(col(self.theme.panel_raised))
            .corner_radius(6.0)
            .inner_margin(egui::Margin::symmetric(8, margin))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let (rect, resp) =
                        ui.allocate_exact_size(Vec2::new(34.0, 30.0) * scale, egui::Sense::hover());
                    resp.on_hover_text(tip);
                    let p = ui.painter_at(rect);
                    let line = Stroke::new(1.8, col(self.theme.dim));
                    let c = rect.center();
                    let at = |x: f32, y: f32| c + Vec2::new(x, y) * scale;
                    match icon {
                        Icon::Gauge => {
                            p.circle_stroke(at(0.0, 2.0), 12.0 * scale, line);
                            p.line_segment([at(0.0, 2.0), at(7.0, -6.0)], line);
                        }
                        _ => {
                            p.rect_stroke(
                                Rect::from_center_size(
                                    at(0.0, -5.0),
                                    Vec2::new(14.0, 10.0) * scale,
                                ),
                                2.0,
                                line,
                                StrokeKind::Inside,
                            );
                            p.line_segment([c, at(0.0, 11.0)], line);
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(value)
                                .monospace()
                                .size(value_size)
                                .color(col(self.theme.text)),
                        );
                    });
                });
            });
    }

    /// The codes in force (G and M), or the inputs and the outputs.
    fn info_block(&mut self, ui: &mut egui::Ui, compact: bool) {
        let (gc, io) = (self.t(Text::McGcodeTab), self.t(Text::McIoTab));
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.info_tab, 0, gc);
            ui.selectable_value(&mut self.info_tab, 1, io);
        });
        let dim = col(self.theme.dim);
        if self.info_tab == 0 {
            let lists = [
                (Text::McActiveG, self.session.modal.g_codes().join("  ")),
                (Text::McActiveM, self.session.modal.m_codes().join("  ")),
            ];
            let size = if compact { 13.0 } else { 15.0 };
            for (title, codes) in lists {
                let title = RichText::new(self.t(title)).color(dim);
                let codes = RichText::new(codes).monospace().strong().size(size);
                if compact {
                    // the codes follow their title and go to a further row when they do not fit
                    ui.horizontal_wrapped(|ui| {
                        ui.label(title);
                        ui.label(codes);
                    });
                } else {
                    ui.label(title);
                    ui.label(codes);
                }
            }
        } else {
            let st = self.session.status.clone();
            let pin = |c: char| st.pins.contains(c);
            let led = |ui: &mut egui::Ui, on: bool, label: &str, color: Color32| {
                ui.horizontal(|ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(Vec2::new(16.0, 16.0), egui::Sense::hover());
                    let c = rect.center();
                    ui.painter()
                        .circle_filled(c, 6.0, if on { color } else { alpha(color, 40) });
                    ui.painter()
                        .circle_stroke(c, 6.0, Stroke::new(1.0, alpha(color, 160)));
                    ui.label(label);
                });
            };
            let (ok, danger) = (col(self.theme.ok), col(self.theme.danger));
            ui.label(RichText::new(self.t(Text::McInputs)).color(dim));
            ui.horizontal_wrapped(|ui| {
                led(ui, pin('X'), "X", danger);
                led(ui, pin('Y'), "Y", danger);
                led(ui, pin('Z'), "Z", danger);
                led(ui, pin('P'), "Probe", danger);
                led(ui, pin('D'), "Door", danger);
            });
            ui.label(RichText::new(self.t(Text::McOutputs)).color(dim));
            ui.horizontal_wrapped(|ui| {
                led(ui, st.spindle_rpm > 0.0, self.t(Text::McSpindle), ok);
                led(ui, self.session.modal.coolant != 9, "Coolant", ok);
            });
        }
    }

    /// The jog: the steps, the speed, the pad and the unlock.
    fn jog_block(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new(self.t(Text::McJog).to_uppercase()).color(col(self.theme.accent)));
        ui.label(self.t(Text::McStep));
        ui.horizontal_wrapped(|ui| {
            for s in STEPS {
                if ui
                    .selectable_label(self.session.step_mm == s, format!("{s}"))
                    .clicked()
                {
                    self.session.step_mm = s;
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(self.t(Text::McJogSpeed));
            ui.add(
                egui::DragValue::new(&mut self.session.feed_mm_min)
                    .range(1.0..=20000.0)
                    .speed(10.0),
            );
            if ui.button(self.t(Text::McUnlock)).clicked() {
                self.session.unlock();
            }
        });
        self.jog_pad(ui);
    }

    /// The buttons of the jog. With room for it, X and Y stand apart from Z and A; in a narrow panel the buttons draw closer and smaller, and in a narrower
    /// one Z and A go under X and Y, so that no button is cut.
    fn jog_pad(&mut self, ui: &mut egui::Ui) {
        const GAP: f32 = 6.0;
        // the empty column between the two blocks of the wide pad
        const APART: f32 = 40.0;
        let room = ui.available_width();
        let wide = room >= 5.0 * PAD_BUTTON.x + APART + 5.0 * GAP;
        let one_block = room >= 5.0 * 30.0 + 4.0 * GAP;
        let columns = if one_block { 5.0 } else { 3.0 };
        let width = ((room - (columns - 1.0) * GAP) / columns).clamp(30.0, PAD_BUTTON.x);
        // two blocks, one over the other, take lower buttons so that both stay in sight
        let height = if one_block { PAD_BUTTON.y } else { 30.0 };
        let size = Vec2::new(width, height);
        let mut pad = |ui: &mut egui::Ui, label: &str, axis: usize, dir: f64| {
            if ui.add(egui::Button::new(label).min_size(size)).clicked() {
                self.session.jog(axis, dir);
            }
        };
        let empty = |ui: &mut egui::Ui| {
            ui.label("");
        };
        let grid = |name: &str| egui::Grid::new(name).spacing([GAP, GAP]).min_col_width(0.0);
        // the rows of the two blocks: what is on the left, in the middle and on the right of X and Y, and the axes Z and A
        let xy = [
            [None, Some(("Y+", 1, 1.0)), None],
            [Some(("X-", 0, -1.0)), None, Some(("X+", 0, 1.0))],
            [None, Some(("Y-", 1, -1.0)), None],
        ];
        let za = [
            [("Z+", 2, 1.0), ("A+", 3, 1.0)],
            [("Z-", 2, -1.0), ("A-", 3, -1.0)],
        ];
        if one_block {
            grid("pad").show(ui, |ui| {
                for (row, buttons) in xy.into_iter().enumerate() {
                    for button in buttons {
                        match button {
                            Some((label, axis, dir)) => pad(ui, label, axis, dir),
                            None => empty(ui),
                        }
                    }
                    if wide {
                        ui.allocate_exact_size(Vec2::new(APART, 1.0), egui::Sense::hover());
                    }
                    // Z and A stand on the first and on the last row
                    if row != 1 {
                        for (label, axis, dir) in za[row / 2] {
                            pad(ui, label, axis, dir);
                        }
                    }
                    ui.end_row();
                }
            });
        } else {
            grid("pad_xy").show(ui, |ui| {
                for buttons in xy {
                    for button in buttons {
                        match button {
                            Some((label, axis, dir)) => pad(ui, label, axis, dir),
                            None => empty(ui),
                        }
                    }
                    ui.end_row();
                }
            });
            ui.add_space(GAP);
            grid("pad_za").show(ui, |ui| {
                for buttons in za {
                    for (label, axis, dir) in buttons {
                        pad(ui, label, axis, dir);
                    }
                    ui.end_row();
                }
            });
        }
    }
}

impl App {
    /// Everything of a frame that comes before the panels are drawn: the size of the window, the lines that go out, the queue, the guard, the keys.
    pub(super) fn begin_frame(&mut self, ctx: &egui::Context) {
        let ctx = ctx.clone();
        self.frame += 1;
        clipmenu::start_frame(&ctx, self.lang);
        self.session.set_lang(self.lang.code());
        // the window follows its size: from 720p to 1080p and on to the large and the wide screens everything grows or shrinks with it
        let native = ctx.content_rect().size() * ctx.zoom_factor();
        let zoom = (native.x / 1400.0).min(native.y / 800.0).clamp(0.75, 3.0);
        if (zoom - ctx.zoom_factor()).abs() > 0.01 {
            ctx.set_zoom_factor(zoom);
        }
        if self.visuals_for != Some(self.theme.id) {
            apply_visuals(&ctx, &self.theme);
            self.visuals_for = Some(self.theme.id);
        }
        if self.start_at == Some(self.frame) && !self.start_refused() {
            self.session.start();
        }
        // a file dropped on the panel of the attachments is attached there
        let dropped = self.attachments.take_drop(&ctx, dialogs::opens_file);
        // several files dropped together are a queue
        self.start_queue(dropped);
        // the lines go out by a thread of their own: the window only hands them over and looks at what was done
        self.session.allow_system_lines = self.settings.program_system_commands;
        // the jog stops at the end of the travel (the travel of the settings, shortened to the one the controller reported)
        self.session.limits_need_homing =
            self.settings.jog_soft_limits == crate::jog::Mode::AfterHoming;
        self.session.soft_limits =
            (self.settings.jog_soft_limits != crate::jog::Mode::Off).then(|| {
                crate::session::effective_travel(
                    self.settings.travel,
                    self.settings
                        .read_controller_travel
                        .then_some(self.session.controller_travel)
                        .flatten(),
                )
            });
        self.session.pump_async(PUMP_BATCH);
        if self.session.running || self.session.busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(60));
        }
        self.queue_tick();
        self.queue_persist();
        self.log_collect();
        if self.queue_gate.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
        self.track_recovery();
        self.server_tick(&ctx);
        self.keys(&ctx);
        self.feed_guard();
    }

    /// Everything of a frame that comes after the panels: the windows, the focus, the close and the picture.
    pub(super) fn end_frame(&mut self, ctx: &egui::Context) {
        let ctx = ctx.clone();
        self.sim_guard();
        self.windows(&ctx);
        self.suite_windows(&ctx);
        self.note_focus(&ctx);
        if self.quit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        let shot = self
            .shot
            .as_ref()
            .map(|s| (s.file.clone(), s.wait, s.done.clone()));
        if let Some((file, wait, done)) = shot {
            ctx.request_repaint();
            if self.frame == 3 + wait {
                let (f2, d2, c2) = (file.clone(), done.clone(), ctx.clone());
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(
                    egui::ScreenshotCallback::new(move |img| {
                        if let Err(e) = save_png(&img, &f2) {
                            eprintln!("cannot save {}: {e}", f2.display());
                        }
                        d2.store(true, Ordering::SeqCst);
                        c2.request_repaint();
                    }),
                ));
            }
            if done.load(Ordering::SeqCst) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }
}

pub fn run(o: &Options) -> eframe::Result {
    if o.ports {
        print!("{}", crate::link::ports_text());
        return Ok(());
    }
    let busy = std::time::Duration::from_secs_f64(Settings::load(&settings_path()).busy_wait_s);
    let (motion, _) = MotionCfg::read(&motion_path(&settings_path()));
    let linked = match crate::link::guarded_for_motion(o, busy, &motion) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("phoenix-cnc: {e}");
            std::process::exit(2);
        }
    };
    let picture = o.shot.is_some();
    let app = App::with_link(o, linked);
    let size = if o.shot.is_some() {
        Vec2::new(1400.0, 800.0)
    } else {
        Vec2::new(1300.0, 780.0)
    };
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size(size)
        .with_min_inner_size([1000.0, 600.0])
        .with_maximized(o.shot.is_none())
        .with_title("EH3D-PHOENIX-CNC");
    if let Some(icon) = decode_icon() {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "EH3D-PHOENIX-CNC",
        options,
        Box::new(move |cc| {
            install_fonts(&cc.egui_ctx);
            let mut shell = shell::Shell::new(app);
            // the list of recent projects lives in the folder of the user; a picture reads and writes none
            if !picture {
                shell.keep_user_recent();
            }
            Ok(Box::new(shell))
        }),
    )
}

#[cfg(test)]
mod visuals_tests {
    use super::*;

    fn luminance(c: Color32) -> f32 {
        let f = |v: u8| {
            let v = f32::from(v) / 255.0;
            if v <= 0.040_45 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
    }

    fn contrast(a: Color32, b: Color32) -> f32 {
        let (a, b) = (luminance(a), luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    #[test]
    fn the_words_that_stand_out_are_read_on_every_theme() {
        let mut themes: Vec<Theme> = ThemeId::ALL.into_iter().map(Theme::for_id).collect();
        themes.push(Theme::from_system_values(true, None));
        themes.push(Theme::from_system_values(false, None));
        for t in themes {
            let v = visuals_of(&t);
            assert_eq!(v.strong_text_color(), col(t.text), "{:?}", t.id);
            // the last two are the title bar of a window and the bar of the tabs
            for ground in [
                v.panel_fill,
                v.window_fill,
                v.extreme_bg_color,
                v.widgets.open.weak_bg_fill,
                v.widgets.noninteractive.bg_fill,
            ] {
                let ratio = contrast(v.strong_text_color(), ground);
                assert!(ratio >= 4.5, "{:?}: {ratio} on {ground:?}", t.id);
            }
        }
    }
}
