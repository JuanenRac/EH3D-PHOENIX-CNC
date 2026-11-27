//! The window: the frame of the suite (`eh3d-dock`) with the panels of the program in it, the menu bar above, and the bars of the toolbar and of
//! the controller. The panels are the bodies in `dockpanels` and `views`; what is chosen in the menus is done by [`App::run_cmd`]; which panels
//! there are, their arrangement and the menus are data in `panels`, which has no window and is tested there.

use std::cell::RefCell;
use std::rc::Rc;

use eh3d_dock::about::Phoenix;
use eh3d_dock::gui::Dock;
use eh3d_dock::menu::{Chosen, DockAction, MenuBar, StdAction};

use super::*;
use crate::panels::{self, id as pid, Cmd, LayoutWatch, MenuFacts};
use crate::view::Preset;

/// How a panel of the window is drawn: with the program and the room the frame gives it.
type PanelDraw = fn(&mut App, &mut egui::Ui);

/// The window.
pub(super) struct Shell {
    app: Rc<RefCell<App>>,
    dock: Dock,
    bar: MenuBar<Cmd>,
    watch: LayoutWatch,
    /// How many frames were drawn, so the arrangement is looked at a few times a second and not at every frame.
    ticks: u32,
}

impl Shell {
    pub(super) fn new(app: App) -> Shell {
        let dock_text = app.settings.dock_text();
        let (lang, recent) = (app.lang, app.settings.recent.clone());
        let app = Rc::new(RefCell::new(app));
        let mut dock = Dock::new(panels::defs(), panels::workspaces());
        let draw: [(&str, PanelDraw); 13] = [
            (pid::VIEWER, App::panel_viewer),
            (pid::JOG, App::panel_jog),
            (pid::POSITION, App::panel_position),
            (pid::JOB, App::panel_job),
            (pid::FILES, App::panel_files),
            (pid::TOOL_TABLE, App::panel_tool_table),
            (pid::TOOLSETTER, App::panel_toolsetter),
            (pid::PROBE, App::panel_probe),
            (pid::IMPORT, App::panel_import),
            (pid::PLAN, App::panel_plan),
            (pid::REPORT, App::panel_report),
            (pid::CONSOLE, App::panel_console),
            (pid::ATTACHMENTS, App::panel_attachments),
        ];
        for (name, f) in draw {
            let a = app.clone();
            dock.register(name, move |ui| f(&mut a.borrow_mut(), ui));
        }
        dock.load_config(&dock_text);
        // the recent files of the settings file before the frame kept its own list
        if dock.core().recent().items().is_empty() {
            for p in recent.iter().rev() {
                dock.core_mut().recent_mut().push(p);
            }
        }
        dock.set_language(lang);
        // the accents of the window take the colour of the program; its window about is its own
        dock.set_program(Phoenix::Cnc);
        let watch = LayoutWatch::new(&dock.core().config_pairs());
        Shell {
            app,
            dock,
            bar: panels::menu_bar(),
            watch,
            ticks: 0,
        }
    }

    /// The list of recent projects lives in the folder of the user, whatever settings file is used.
    pub(super) fn keep_user_recent(&mut self) {
        self.dock.keep_user_recent(Phoenix::Cnc);
    }

    /// One frame: the controller and the keys first, then the menu bar, the bars, the panels, and last the windows.
    pub(super) fn draw(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let (lang, facts) = {
            let mut app = self.app.borrow_mut();
            app.begin_frame(&ctx);
            (app.lang, app.menu_facts())
        };
        self.dock.set_language(lang);
        self.dock
            .set_menu_state(panels::disabled_entries(&facts), panels::checks(&facts));
        let mut chosen = None;
        egui::Panel::top("menu").show(ui, |ui| {
            chosen = self.dock.menu_bar(ui, &self.bar);
        });
        {
            let mut app = self.app.borrow_mut();
            app.toolbar(ui);
            app.top(ui);
            app.suite_bar(ui);
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| self.dock.show(ui));
        self.after(chosen, &ctx);
        self.app.borrow_mut().end_frame(&ctx);
        // the arrangement is kept in the settings file when it changed
        self.ticks = self.ticks.wrapping_add(1);
        if self.ticks.is_multiple_of(panels::LAYOUT_CHECK_EVERY) {
            let pairs = self.dock.core().config_pairs();
            if self.watch.changed(&pairs) {
                self.app.borrow_mut().save_layout(&pairs);
            }
        }
    }

    /// Does what was chosen in the menus and what the commands asked of the frame, and keeps the list of recent files.
    fn after(&mut self, chosen: Option<Chosen<Cmd>>, ctx: &egui::Context) {
        let mut app = self.app.borrow_mut();
        match chosen {
            Some(Chosen::App(cmd)) => app.run_cmd(cmd, ctx),
            Some(Chosen::Recent(path)) => app.open(Path::new(&path)),
            Some(Chosen::Dock(_)) | None => {}
        }
        for action in std::mem::take(&mut app.pending_dock) {
            let _ = self.dock.apply(action);
        }
        for path in std::mem::take(&mut app.recent_added) {
            self.dock.core_mut().recent_mut().push(&path);
        }
    }
}

impl eframe::App for Shell {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.draw(ui);
    }
}

impl App {
    /// Tells the guard the time and renews the jog, says in the log what the guard did, and keeps why it stopped the machine for the panels.
    pub(super) fn feed_guard(&mut self) {
        let Some(g) = self.guard.clone() else { return };
        let now = u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let _ = g.tick_at(now);
        let lang = self.lang.code();
        for event in g.take_events() {
            if let Some(words) = crate::guard::event_text(&event, lang) {
                self.log_tell(crate::logwords::Lg::GuardSays, &[words.to_string()]);
            }
        }
        if let Some(trip) = g.try_tripped() {
            self.guard_trip = trip;
        }
    }

    /// Keeps the arrangement of the panels in the settings file, when the person gave one (a picture keeps nothing).
    pub(super) fn save_layout(&mut self, pairs: &[(String, String)]) {
        self.settings.set_dock(pairs);
        if self.shot.is_none() {
            self.save_settings();
        }
    }

    /// What the menus need to know to turn their entries on and off.
    pub(super) fn menu_facts(&self) -> MenuFacts {
        let has_program = !self.session.program.is_empty();
        MenuFacts {
            has_program,
            has_suite: self.suite.file.is_some(),
            can_start: has_program && self.can_start(),
            can_pause: self.can_pause(),
            can_stop: self.can_stop(),
            can_send: self.can_send_to_server(),
            has_line: self.selected_line().is_some(),
            has_bookmarks: !self.bookmarks.is_empty(),
            can_undo: self.can_undo_transform(),
            idle: self.machine_idle(),
            has_map: self.session.height_map.is_some(),
            corrected: self.session.corrected(),
            guard_on: self.guard.is_some(),
            guard_stopped: self.guard_trip.is_some(),
            view_3d: self.settings.view_3d,
        }
    }

    /// The program is taken out of the window (and the bookmarks, the path and the picture that went with it).
    pub(super) fn clear_program(&mut self) {
        if !self.may_change_program() {
            return;
        }
        self.session.clear_program();
        self.bookmarks.clear();
        self.path_toolpath.clear();
        self.path_extras = crate::sim::Extras::default();
        self.viewer.refresh(&[], &[]);
    }

    /// Asks the frame to show a panel (it comes back to its place, or to a group of tabs, and takes the focus).
    fn show_panel(&mut self, id: &str) {
        self.pending_dock
            .push(DockAction::ShowPanel(id.to_string()));
    }

    /// Does what an entry of the menus says.
    pub(super) fn run_cmd(&mut self, cmd: Cmd, _ctx: &egui::Context) {
        match cmd {
            Cmd::Std(action) => self.run_std(action),
            Cmd::ImportDrawing | Cmd::Post => self.show_panel(pid::IMPORT),
            Cmd::SuiteOpen => {
                self.suite_browse(eh3d_filedialog::Mode::Open, Purpose::SuiteOpen);
            }
            Cmd::SuiteSave => self.suite_save_entry(),
            Cmd::SuiteSaveAs => {
                self.suite_browse(eh3d_filedialog::Mode::Save, Purpose::SuiteSaveAs);
            }
            Cmd::SuiteImport => {
                self.suite_browse(eh3d_filedialog::Mode::Open, Purpose::SuiteImport);
            }
            Cmd::SuiteTake => self.suite_show_list(suite::SuiteList::Take),
            Cmd::SuiteExtract => self.suite_show_list(suite::SuiteList::Extract),
            Cmd::SuiteBranches => self.suite_show_list(suite::SuiteList::Branches),
            Cmd::LogExport => {
                self.win = Some(Win::Save(
                    self.browser_for(eh3d_filedialog::Mode::Save, Purpose::LogSave),
                ));
            }
            Cmd::EditProgram => self.open_window("edit"),
            Cmd::Transform => self.open_window("transform"),
            Cmd::Bookmarks => self.open_window("bookmarks"),
            Cmd::BookmarkToggle => {
                let _ = self.toggle_bookmark_selected();
            }
            Cmd::BookmarkNext => self.bookmark_step(true),
            Cmd::BookmarkPrevious => self.bookmark_step(false),
            Cmd::ViewPlan | Cmd::View3d => {
                self.settings.view_3d = cmd == Cmd::View3d;
                self.save_settings();
            }
            Cmd::ResetView => self.reset_view(),
            Cmd::ViewTop => self.set_preset(Preset::Top),
            Cmd::ViewFront => self.set_preset(Preset::Front),
            Cmd::ViewSide => self.set_preset(Preset::Side),
            Cmd::ViewIso => self.set_preset(Preset::Iso),
            Cmd::FitTable => self.fit_program = false,
            Cmd::FitProgram => self.fit_program = true,
            Cmd::Start => self.request_start(),
            Cmd::Pause => self.program_pause(),
            Cmd::Stop => self.program_stop(),
            Cmd::SendToServer => self.request_send(),
            Cmd::Rewind => self.rewind_program(),
            Cmd::ClearProgram => self.clear_program(),
            Cmd::Check => self.open_window("check"),
            Cmd::KeepOut => self.open_window("keepout"),
            Cmd::Stats => self.open_window("stats"),
            Cmd::LookAhead => self.show_panel(pid::PLAN),
            Cmd::Macros => self.open_window("macros"),
            Cmd::EmergencyStop => self.emergency_stop(),
            Cmd::Unlock => self.session.unlock(),
            Cmd::Home => self.session.home(),
            Cmd::ZeroXyz => self.session.zero_axes(&[0, 1, 2]),
            Cmd::ZeroXy => self.session.zero_axes(&[0, 1]),
            Cmd::GoZeroXy => self.session.go_zero_xy(),
            Cmd::GoZeroZ => self.session.go_zero_z(),
            Cmd::GoG28 => self.session.go_g28(),
            Cmd::GoG30 => self.session.go_g30(),
            Cmd::SetG28 => self.session.store_position(28),
            Cmd::SetG30 => self.session.store_position(30),
            Cmd::SpindleOn => self.session.spindle(Some(self.settings.spindle_rpm)),
            Cmd::SpindleOff => self.session.spindle(None),
            Cmd::CoolantOn => self.session.coolant(true),
            Cmd::CoolantOff => self.session.coolant(false),
            Cmd::ProbeZ => self.open_window("probe"),
            Cmd::ProbeGrid => {
                if let Some(ask) = self.grid_ask() {
                    self.win = Some(Win::Probe(ask));
                }
            }
            Cmd::UseMap => self.use_height_map(),
            Cmd::DropMap => {
                if self.session.drop_height_correction() {
                    self.refresh_path();
                }
            }
            Cmd::ToolSetter => self.show_panel(pid::TOOLSETTER),
            Cmd::MachineOnServer => self.open_window("machine"),
            Cmd::ResetGuard => {
                if let Some(g) = &self.guard {
                    g.reset();
                    self.guard_trip = None;
                }
            }
            Cmd::Shortcuts => self.open_window("keys"),
            Cmd::About => self.open_window("about"),
        }
    }

    /// The actions every program of the suite has in its File, Edit and Help menus.
    fn run_std(&mut self, action: StdAction) {
        match action {
            StdAction::New => self.clear_program(),
            StdAction::Open => self.open_window("open"),
            StdAction::Save | StdAction::SaveAs => self.open_window("save"),
            StdAction::Export => {
                self.win = Some(Win::Save(
                    self.browser_for(eh3d_filedialog::Mode::Save, Purpose::ReportSave),
                ));
            }
            StdAction::Quit => self.quit = true,
            StdAction::Undo => {
                if self.can_undo_transform() {
                    self.undo_transform();
                }
            }
            StdAction::Preferences => self.open_window("settings"),
            // the program has no redo, no clipboard of its own to cut and paste, no online help and no updater yet
            StdAction::Redo
            | StdAction::Cut
            | StdAction::Copy
            | StdAction::Paste
            | StdAction::OnlineHelp
            | StdAction::CheckUpdates => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guard::Trip;
    use crate::link;
    use crate::motioncfg::GuardCfg;
    use eh3d_controller::faulty::{Fault, FaultyController};
    use eh3d_controller::sim::SimController;

    /// A window with its own settings file, a program and the frame around it.
    fn shell() -> Shell {
        let mut a = App::isolated();
        a.session
            .load("G21 G90\nG0 Z5\nG0 X0 Y0\nG1 Z-1 F100\nG1 X10 Y0\nG1 X10 Y10\nG1 X0 Y10\nG1 X0 Y0\nG0 Z5\nM30");
        a.refresh_path();
        Shell::new(a)
    }

    fn frames(s: &mut Shell, n: usize) {
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::Vec2::new(1400.0, 800.0),
            )),
            ..Default::default()
        };
        for _ in 0..n {
            ctx.run_ui(input.clone(), |ui| s.draw(ui))
                .textures_delta
                .clear();
        }
    }

    #[test]
    fn every_workspace_draws_with_a_program_loaded_and_without_one() {
        for with_program in [true, false] {
            let mut s = shell();
            if !with_program {
                s.app.borrow_mut().clear_program();
            }
            for name in panels::WORKSPACES {
                s.dock
                    .apply(DockAction::SwitchWorkspace(name.to_string()))
                    .unwrap();
                frames(&mut s, 3);
                assert_eq!(s.dock.core().workspaces().current_name(), name);
            }
        }
    }

    #[test]
    fn every_panel_can_be_shown_and_drawn_in_turn() {
        let mut s = shell();
        for p in pid::ALL {
            s.dock.apply(DockAction::ShowPanel(p.to_string())).unwrap();
            frames(&mut s, 2);
            assert!(s.dock.core().layout().is_shown(p), "{p}");
        }
        // and again with the window in each of the eight languages
        for lang in [
            Lang::En,
            Lang::Es,
            Lang::It,
            Lang::De,
            Lang::Fr,
            Lang::Pt,
            Lang::Zh,
            Lang::Ja,
        ] {
            s.app.borrow_mut().lang = lang;
            frames(&mut s, 1);
        }
    }

    #[test]
    fn the_arrangement_is_written_to_the_settings_once_when_it_changes_and_comes_back_at_the_next_start(
    ) {
        let mut s = shell();
        let path = s.app.borrow().settings_path.clone();
        let _ = std::fs::remove_file(&path);
        frames(&mut s, 2 * panels::LAYOUT_CHECK_EVERY as usize);
        assert!(!path.exists(), "a quiet window writes nothing");
        s.dock
            .apply(DockAction::HidePanel(pid::CONSOLE.to_string()))
            .unwrap();
        frames(&mut s, panels::LAYOUT_CHECK_EVERY as usize + 1);
        let text = std::fs::read_to_string(&path).expect("the layout was written");
        assert!(text.contains("dock.version = 1"), "{text}");
        // the next start reads it
        let mut a = App::isolated();
        a.settings_path = path.clone();
        a.settings = Settings::load(&path);
        let again = Shell::new(a);
        assert!(!again.dock.core().layout().is_shown(pid::CONSOLE));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn the_commands_of_the_menus_do_what_they_say() {
        let ctx = egui::Context::default();
        let mut a = App::isolated();
        a.session.load("G21\nG1 X5 F100\nM30");
        a.run_cmd(Cmd::LookAhead, &ctx);
        a.run_cmd(Cmd::ImportDrawing, &ctx);
        a.run_cmd(Cmd::ToolSetter, &ctx);
        assert_eq!(
            a.pending_dock,
            vec![
                DockAction::ShowPanel(pid::PLAN.to_string()),
                DockAction::ShowPanel(pid::IMPORT.to_string()),
                DockAction::ShowPanel(pid::TOOLSETTER.to_string()),
            ]
        );
        a.run_cmd(Cmd::Std(StdAction::Open), &ctx);
        assert!(matches!(a.win, Some(Win::Open(_))));
        a.run_cmd(Cmd::Std(StdAction::Preferences), &ctx);
        assert!(matches!(a.win, Some(Win::Settings(_))));
        a.run_cmd(Cmd::About, &ctx);
        assert!(matches!(a.win, Some(Win::About)));
        a.run_cmd(Cmd::Shortcuts, &ctx);
        assert!(matches!(a.win, Some(Win::Shortcuts)));
        a.run_cmd(Cmd::View3d, &ctx);
        assert!(a.settings.view_3d);
        a.run_cmd(Cmd::ViewPlan, &ctx);
        assert!(!a.settings.view_3d);
        a.run_cmd(Cmd::FitProgram, &ctx);
        assert!(a.fit_program);
        a.run_cmd(Cmd::Std(StdAction::New), &ctx);
        assert!(a.session.program.is_empty());
        assert!(!a.quit);
        a.run_cmd(Cmd::Std(StdAction::Quit), &ctx);
        assert!(a.quit);
        // the commands the program has no use for do nothing
        let before = a.session.log.len();
        for s in [
            StdAction::Redo,
            StdAction::Cut,
            StdAction::Copy,
            StdAction::Paste,
        ] {
            a.run_cmd(Cmd::Std(s), &ctx);
        }
        assert_eq!(a.session.log.len(), before);
        let _ = std::fs::remove_file(&a.settings_path);
    }

    #[test]
    fn the_entries_of_the_menus_follow_what_the_window_can_do() {
        let mut a = App::isolated();
        let f = a.menu_facts();
        assert!(!f.has_program && !f.can_start && f.idle && !f.guard_on);
        a.session.load("G21\nG1 X5 F100\nM30");
        let f = a.menu_facts();
        assert!(f.has_program && f.can_start);
        let off = panels::disabled_entries(&f);
        assert!(!off.iter().any(|i| i == "program.start"));
        assert!(off.iter().any(|i| i == "machine.reset_guard"));
        let _ = std::fs::remove_file(&a.settings_path);
    }

    /// A window over a controller that goes silent, inside the guard.
    fn silent_window() -> App {
        let silent = Box::new(FaultyController::new(
            SimController::new(),
            vec![Fault::Silence {
                from: 0,
                calls: None,
            }],
        ));
        let linked = link::wrap(silent, false, &GuardCfg::default());
        let mut a = App::with_link(&crate::cli::Options::default(), linked);
        a.settings_path =
            std::env::temp_dir().join(format!("eh3d-phoenix-cnc-guard-{}.cfg", std::process::id()));
        a
    }

    #[test]
    fn a_controller_that_falls_silent_stops_the_machine_and_the_window_says_so_until_the_operator_clears_it(
    ) {
        let ctx = egui::Context::default();
        let mut a = silent_window();
        a.feed_guard();
        assert_eq!(a.guard_trip, Some(Trip::Deaf));
        assert!(
            a.session.log.iter().any(|l| l.starts_with("guard: ")),
            "{:?}",
            a.session.log
        );
        // the menu offers the all clear, and the panel of the state shows the stop
        assert!(a.menu_facts().guard_stopped);
        let mut s = Shell::new(a);
        frames(&mut s, 2);
        // the all clear: the guard takes orders again
        s.app.borrow_mut().run_cmd(Cmd::ResetGuard, &ctx);
        assert_eq!(s.app.borrow().guard_trip, None);
        assert!(!s.app.borrow().menu_facts().guard_stopped);
    }

    #[test]
    fn with_the_guard_off_in_the_motion_file_the_window_talks_to_the_controller_directly() {
        let off = GuardCfg {
            enabled: false,
            ..GuardCfg::default()
        };
        let linked = link::wrap(Box::new(SimController::new()), true, &off);
        let mut a = App::with_link(&crate::cli::Options::default(), linked);
        assert!(a.guard.is_none());
        a.feed_guard();
        assert_eq!(a.guard_trip, None);
        let f = a.menu_facts();
        assert!(!f.guard_on);
    }

    #[test]
    fn jogs_that_cross_through_the_window_do_not_stop_the_machine() {
        let linked = link::wrap(Box::new(SimController::new()), true, &GuardCfg::default());
        let mut a = App::with_link(&crate::cli::Options::default(), linked);
        for (axis, dir) in [(0, 1.0), (1, 1.0), (0, -1.0), (2, 1.0), (1, -1.0)] {
            a.session.jog(axis, dir);
            a.feed_guard();
        }
        assert_eq!(a.guard_trip, None, "{:?}", a.session.log);
        assert!(
            a.session.log.iter().all(|l| !l.contains("jog:")),
            "{:?}",
            a.session.log
        );
    }

    #[test]
    fn the_dock_menu_keeps_the_open_window_and_the_recent_list_in_step() {
        let mut s = shell();
        {
            let mut app = s.app.borrow_mut();
            app.recent_added.push("C:/jobs/a.nc".to_string());
        }
        frames(&mut s, 1);
        assert_eq!(s.dock.core().recent().items(), ["C:/jobs/a.nc"]);
        assert!(s.app.borrow().recent_added.is_empty());
    }
}
