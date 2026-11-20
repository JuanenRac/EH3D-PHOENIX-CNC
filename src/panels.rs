//! The panels, the workspaces and the menus of the window as data, with no window in them: which panels there are and what each is called in
//! the eight languages, the factory arrangement of each workspace (Prepare, Run, Review and Compact), the menu bar (File, Edit, View, Program,
//! Machine, Window and Help) and which of its entries can be chosen at a given moment, and the rule that decides when the arrangement is
//! written to the settings file. The window (`gui`) draws these with the frame of the suite (`eh3d-dock`) and only does what they say.

use eh3d_dock::core::PanelDef;
use eh3d_dock::geom::Side;
use eh3d_dock::menu::{Entry, Item, Label, Menu, MenuBar, StdAction};
use eh3d_dock::model::{Layout, Node};
use eh3d_dock::sectors::{titles, Sector};
use eh3d_dock::stdmenus::{edit_menu, file_menu, help_menu, view_menu, window_menu, HelpOptions};
use eh3d_dock::texts::Text8;
use eh3d_dock::workspaces::Workspace;
use eh3d_view::texts::{tr, Lang, Text};

use crate::paneltext::{text8, Pt};
use crate::words::{self, Msg};

/// The names of the panels (what the settings file stores, so they never change).
pub mod id {
    /// The path of the program on the table, in plan or in 3D.
    pub const VIEWER: &str = "viewer";
    /// The hand control: the step, the speed and the buttons of the axes, and the zeros.
    pub const JOG: &str = "jog";
    /// The state of the machine: the position, the speeds and the codes in force, and the guard.
    pub const POSITION: &str = "position";
    /// The program: its lines, the run and the queue.
    pub const JOB: &str = "job";
    /// The files: the one that is loaded, the recent ones and the queue.
    pub const FILES: &str = "files";
    /// The table of tools.
    pub const TOOL_TABLE: &str = "tool_table";
    /// The tool setter: the measure of the length of the tool.
    pub const TOOLSETTER: &str = "toolsetter";
    /// The probe and the height map.
    pub const PROBE: &str = "probe";
    /// The import of a drawing, with the post-processor.
    pub const IMPORT: &str = "import";
    /// The planning and the time of the program, with and without the look-ahead.
    pub const PLAN: &str = "plan";
    /// The report of the checks of the program.
    pub const REPORT: &str = "report";
    /// The commands typed by the operator and the log.
    pub const CONSOLE: &str = "console";

    /// Every panel, in the order of the View menu.
    pub const ALL: [&str; 12] = [
        VIEWER, JOG, POSITION, JOB, FILES, TOOL_TABLE, TOOLSETTER, PROBE, IMPORT, PLAN, REPORT,
        CONSOLE,
    ];
}

/// The workspace to prepare a job: the files, the import, the tools, the program and the time.
pub const WS_PREPARE: &str = "Prepare";
/// The workspace to run it: the state of the machine, the hand control, the path and the program.
pub const WS_RUN: &str = "Run";
/// The workspace to look the program over before it runs: the path, the checks, the time, the probe.
pub const WS_REVIEW: &str = "Review";
/// The workspace with the path and nothing else on show.
pub const WS_COMPACT: &str = "Compact";

/// The names of the workspaces, in the order of the Window menu.
pub const WORKSPACES: [&str; 4] = [WS_PREPARE, WS_RUN, WS_REVIEW, WS_COMPACT];

/// A word of the interface (`eh3d-view`) as the text of the frame: the eight languages in the order English, Spanish, Italian, German, French,
/// Portuguese (of Portugal), Chinese and Japanese.
pub fn t8(t: Text) -> Text8 {
    Text8([
        tr(Lang::En, t),
        tr(Lang::Es, t),
        tr(Lang::It, t),
        tr(Lang::De, t),
        tr(Lang::Fr, t),
        tr(Lang::Pt, t),
        tr(Lang::Zh, t),
        tr(Lang::Ja, t),
    ])
}

/// A message of the program-side tools as the text of the frame.
pub fn w8(m: Msg) -> Text8 {
    Text8([
        words::text("en", m),
        words::text("es", m),
        words::text("it", m),
        words::text("de", m),
        words::text("fr", m),
        words::text("pt", m),
        words::text("zh", m),
        words::text("ja", m),
    ])
}

/// The panels: the ones the suite proposes for machine control (the viewer, the console, the jog, the position, the tool table and the files),
/// the program under its own name, and the five that this program adds (the tool setter, the probe, the import, the planning and the report).
pub fn defs() -> Vec<PanelDef> {
    let mut v: Vec<PanelDef> = Sector::Cnc
        .panels()
        .into_iter()
        .filter(|p| p.id != id::JOB && p.id != id::POSITION)
        .collect();
    v.push(
        PanelDef::new(id::JOB, text8(Pt::PanelProgram))
            .icon("G")
            .min(240.0, 160.0),
    );
    v.push(
        PanelDef::new(id::POSITION, text8(Pt::PanelMachineState))
            .icon("X")
            .min(260.0, 180.0),
    );
    v.push(
        PanelDef::new(id::TOOLSETTER, text8(Pt::PanelToolSetter))
            .icon("S")
            .min(240.0, 160.0),
    );
    v.push(
        PanelDef::new(id::PROBE, text8(Pt::PanelProbe))
            .icon("P")
            .min(240.0, 160.0),
    );
    v.push(
        PanelDef::new(id::IMPORT, text8(Pt::PanelImport))
            .icon("I")
            .min(240.0, 180.0),
    );
    v.push(
        PanelDef::new(id::PLAN, text8(Pt::PanelPlan))
            .icon("T")
            .min(240.0, 140.0),
    );
    v.push(
        PanelDef::new(id::REPORT, titles::REPORT)
            .icon("R")
            .min(240.0, 140.0),
    );
    // the order of the View menu
    let order = |p: &PanelDef| {
        id::ALL
            .iter()
            .position(|n| *n == p.id)
            .unwrap_or(usize::MAX)
    };
    v.sort_by_key(order);
    v
}

fn ws(name: &str, root: Node) -> Workspace {
    Workspace::new(name, Layout::with_root(root))
}

/// The factory workspaces. Prepare: the files, the import and the tools on the left, the path and the console in the middle, the program, the
/// time and the report on the right. Run: the state and the hand control on the left, the path and the console, the program and the files.
/// Review: the path beside the checks, the time, the program and the probe. Compact: the path, with the console, the hand control and the
/// state folded into the edges.
pub fn workspaces() -> Vec<Workspace> {
    let t = |ids: &[&str]| Node::tabs(ids);
    vec![
        ws(
            WS_PREPARE,
            Node::row(
                0.22,
                t(&[id::FILES, id::IMPORT, id::TOOL_TABLE, id::TOOLSETTER]),
                Node::row(
                    0.7,
                    Node::column(0.74, t(&[id::VIEWER]), t(&[id::CONSOLE])),
                    Node::column(0.5, t(&[id::JOB]), t(&[id::PLAN, id::REPORT])),
                ),
            ),
        ),
        ws(
            WS_RUN,
            Node::row(
                0.24,
                Node::column(0.5, t(&[id::POSITION]), t(&[id::JOG])),
                Node::row(
                    0.7,
                    Node::column(0.74, t(&[id::VIEWER]), t(&[id::CONSOLE])),
                    t(&[id::JOB, id::FILES]),
                ),
            ),
        ),
        ws(
            WS_REVIEW,
            Node::row(
                0.6,
                Node::column(0.78, t(&[id::VIEWER]), t(&[id::CONSOLE])),
                Node::column(0.5, t(&[id::REPORT, id::PLAN]), t(&[id::JOB, id::PROBE])),
            ),
        ),
        Workspace::new(
            WS_COMPACT,
            Layout::with_root(t(&[id::VIEWER]))
                .with_auto_hidden(id::CONSOLE, Side::Bottom)
                .with_auto_hidden(id::JOG, Side::Left)
                .with_auto_hidden(id::POSITION, Side::Right),
        ),
    ]
}

/// What the program does when an entry of its menus is chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cmd {
    /// An action every program has (New, Open, Save, Quit...).
    Std(StdAction),
    /// Show the panel of the import and ask for a drawing.
    ImportDrawing,
    /// Save the log of the session.
    LogExport,
    /// Edit the program in its window.
    EditProgram,
    /// The window of the transformations of the program.
    Transform,
    /// The list of bookmarks.
    Bookmarks,
    /// Mark or unmark the selected line.
    BookmarkToggle,
    /// Go to the next bookmark.
    BookmarkNext,
    /// Go to the previous bookmark.
    BookmarkPrevious,
    /// The path in plan.
    ViewPlan,
    /// The path on the machine in 3D.
    View3d,
    /// Look at the path from the start.
    ResetView,
    /// The standard views.
    ViewTop,
    ViewFront,
    ViewSide,
    ViewIso,
    /// Fit the table or the program in the view.
    FitTable,
    FitProgram,
    /// Start, pause and stop the program, send it to the server, rewind it, clear it.
    Start,
    Pause,
    Stop,
    SendToServer,
    Rewind,
    ClearProgram,
    /// The checks, the keep-out boxes and the statistics of the program.
    Check,
    KeepOut,
    Stats,
    /// The panel of the time with and without the look-ahead.
    LookAhead,
    /// The panel of the post-processor.
    Post,
    /// The list of macros.
    Macros,
    /// The stop of the machine, and the release of the alarm.
    EmergencyStop,
    Unlock,
    /// Home, zero and go to the zero.
    Home,
    ZeroXyz,
    ZeroXy,
    GoZeroXy,
    GoZeroZ,
    /// The stored positions.
    GoG28,
    GoG30,
    SetG28,
    SetG30,
    /// The spindle and the coolant.
    SpindleOn,
    SpindleOff,
    CoolantOn,
    CoolantOff,
    /// The probe and the height map.
    ProbeZ,
    ProbeGrid,
    UseMap,
    DropMap,
    /// The panel of the tool setter.
    ToolSetter,
    /// The machine on the server.
    MachineOnServer,
    /// The operator's all clear for the guard.
    ResetGuard,
    /// The window of the keyboard shortcuts, and the window about the program.
    Shortcuts,
    About,
}

impl From<StdAction> for Cmd {
    fn from(a: StdAction) -> Cmd {
        Cmd::Std(a)
    }
}

fn own(id: &str, label: Text8, cmd: Cmd) -> Entry<Cmd> {
    Entry::app(id, Label::Own(label), cmd)
}

/// Takes the shortcuts off every entry of the menus: the keys of the program are its own (the scheme of the settings file), and the frame must
/// not act on the same key a second time.
fn without_keys(menus: &mut [Menu<Cmd>]) {
    fn walk(items: &mut [Item<Cmd>]) {
        for item in items {
            match item {
                Item::Entry(e) => e.shortcut = None,
                Item::Submenu(m) => walk(&mut m.items),
                _ => {}
            }
        }
    }
    for m in menus {
        walk(&mut m.items);
    }
}

/// The menu bar: File, Edit, View, Program, Machine, Window and Help.
pub fn menu_bar() -> MenuBar<Cmd> {
    let mut file: Menu<Cmd> = file_menu(true);
    let mut edit: Menu<Cmd> = edit_menu();
    let mut view: Menu<Cmd> = view_menu();
    let window: Menu<Cmd> = window_menu();
    let mut help: Menu<Cmd> = help_menu(HelpOptions {
        online_help: false,
        check_updates: false,
    });
    // File: the export is the report of the program; the import and the log of the session go with the files
    for item in &mut file.items {
        if let Item::Entry(e) = item {
            if e.id == "file.export" {
                e.label = Label::Own(w8(Msg::RepMenu));
            }
        }
    }
    let at = file
        .items
        .iter()
        .position(|i| matches!(i, Item::Entry(e) if e.id == "file.save"))
        .unwrap_or(file.items.len());
    file.items.insert(
        at,
        Item::Entry(own(
            "file.import",
            text8(Pt::MenuImport),
            Cmd::ImportDrawing,
        )),
    );
    file.items.insert(
        at + 1,
        Item::Entry(own("file.log", w8(Msg::MenuLogExport), Cmd::LogExport)),
    );
    // Edit: the program, its transformations and the bookmarks, after the standard entries
    edit = edit
        .sep()
        .entry(own(
            "edit.program",
            t8(Text::McEditProgram),
            Cmd::EditProgram,
        ))
        .entry(own(
            "edit.transform",
            w8(Msg::MenuTransform),
            Cmd::Transform,
        ))
        .sep()
        .entry(own(
            "edit.bookmarks",
            w8(Msg::MenuBookmarks),
            Cmd::Bookmarks,
        ))
        .entry(own(
            "edit.bookmark_toggle",
            w8(Msg::MenuBookmarkToggle),
            Cmd::BookmarkToggle,
        ))
        .entry(own(
            "edit.bookmark_next",
            w8(Msg::MenuBookmarkNext),
            Cmd::BookmarkNext,
        ))
        .entry(own(
            "edit.bookmark_prev",
            w8(Msg::MenuBookmarkPrev),
            Cmd::BookmarkPrevious,
        ));
    // View: after the panels and the layout, how the path is looked at
    view = view
        .sep()
        .entry(own("view.plan", t8(Text::ViewPlan), Cmd::ViewPlan))
        .entry(own("view.3d", t8(Text::ViewMachine), Cmd::View3d))
        .entry(own("view.reset", t8(Text::ResetView), Cmd::ResetView))
        .entry(own("view.top", w8(Msg::ViewTop), Cmd::ViewTop))
        .entry(own("view.front", w8(Msg::ViewFront), Cmd::ViewFront))
        .entry(own("view.side", w8(Msg::ViewSide), Cmd::ViewSide))
        .entry(own("view.iso", w8(Msg::ViewIso), Cmd::ViewIso))
        .sep()
        .entry(own("view.fit_table", t8(Text::McFitTable), Cmd::FitTable))
        .entry(own(
            "view.fit_program",
            t8(Text::McFitProgram),
            Cmd::FitProgram,
        ));
    // Program: the run, the checks, the time, the import and the macros
    let program = Menu::new("program", Label::Own(t8(Text::McProgram)))
        .entry(own("program.start", t8(Text::Start), Cmd::Start))
        .entry(own("program.pause", t8(Text::Pause), Cmd::Pause))
        .entry(own("program.stop", t8(Text::Stop), Cmd::Stop))
        .entry(own(
            "program.send",
            t8(Text::McSendServer),
            Cmd::SendToServer,
        ))
        .sep()
        .entry(own("program.rewind", t8(Text::McRewind), Cmd::Rewind))
        .entry(own(
            "program.clear",
            t8(Text::McClearProgram),
            Cmd::ClearProgram,
        ))
        .sep()
        .entry(own("program.check", w8(Msg::MenuCheck), Cmd::Check))
        .entry(own("program.keepout", w8(Msg::MenuKeepOut), Cmd::KeepOut))
        .entry(own("program.stats", w8(Msg::MenuStats), Cmd::Stats))
        .entry(own(
            "program.lookahead",
            text8(Pt::MenuLookAhead),
            Cmd::LookAhead,
        ))
        .sep()
        .entry(own(
            "program.import",
            text8(Pt::MenuImport),
            Cmd::ImportDrawing,
        ))
        .entry(own("program.post", text8(Pt::MenuPost), Cmd::Post))
        .entry(own("program.macros", w8(Msg::MenuMacros), Cmd::Macros));
    // Machine: the stop, the zeros, the stored positions, the spindle, the probe, the tool setter and the guard
    let machine = Menu::new("machine", Label::Own(t8(Text::McMachine)))
        .entry(own(
            "machine.estop",
            t8(Text::EmergencyStop),
            Cmd::EmergencyStop,
        ))
        .entry(own("machine.unlock", t8(Text::McUnlock), Cmd::Unlock))
        .entry(own(
            "machine.reset_guard",
            text8(Pt::MenuResetGuard),
            Cmd::ResetGuard,
        ))
        .sep()
        .entry(own("machine.home", t8(Text::McHome), Cmd::Home))
        .entry(own("machine.zero_xyz", t8(Text::McZeroXyz), Cmd::ZeroXyz))
        .entry(own("machine.zero_xy", t8(Text::McZeroXy), Cmd::ZeroXy))
        .entry(own(
            "machine.go_zero_xy",
            t8(Text::McGoZeroXy),
            Cmd::GoZeroXy,
        ))
        .entry(own("machine.go_zero_z", t8(Text::McGoZeroZ), Cmd::GoZeroZ))
        .sep()
        .entry(own("machine.go_g28", t8(Text::McGoG28), Cmd::GoG28))
        .entry(own("machine.go_g30", t8(Text::McGoG30), Cmd::GoG30))
        .entry(own("machine.set_g28", t8(Text::McSetG28), Cmd::SetG28))
        .entry(own("machine.set_g30", t8(Text::McSetG30), Cmd::SetG30))
        .sep()
        .entry(own(
            "machine.spindle_on",
            t8(Text::McSpindleOn),
            Cmd::SpindleOn,
        ))
        .entry(own(
            "machine.spindle_off",
            t8(Text::McSpindleOff),
            Cmd::SpindleOff,
        ))
        .entry(own(
            "machine.coolant_on",
            t8(Text::McCoolantOn),
            Cmd::CoolantOn,
        ))
        .entry(own(
            "machine.coolant_off",
            t8(Text::McCoolantOff),
            Cmd::CoolantOff,
        ))
        .sep()
        .entry(own("machine.probe_z", text8(Pt::MenuProbeZ), Cmd::ProbeZ))
        .entry(own(
            "machine.probe_grid",
            text8(Pt::MenuProbeGrid),
            Cmd::ProbeGrid,
        ))
        .entry(own("machine.use_map", text8(Pt::MenuUseMap), Cmd::UseMap))
        .entry(own(
            "machine.drop_map",
            text8(Pt::MenuDropMap),
            Cmd::DropMap,
        ))
        .entry(own(
            "machine.toolsetter",
            text8(Pt::MenuToolSetter),
            Cmd::ToolSetter,
        ))
        .sep()
        .entry(own(
            "machine.server",
            w8(Msg::CncMenu),
            Cmd::MachineOnServer,
        ));
    // Help: the window of the shortcuts and the one about the program are the ones of the program, which knows its own keys
    help.items.retain(
        |i| !matches!(i, Item::Entry(e) if e.id == "help.shortcuts" || e.id == "help.about"),
    );
    let at = help
        .items
        .iter()
        .position(|i| matches!(i, Item::Entry(_)))
        .unwrap_or(0);
    help.items.insert(
        at,
        Item::Entry(own("help.keys", t8(Text::McShortcuts), Cmd::Shortcuts)),
    );
    help.items
        .push(Item::Entry(own("help.about", t8(Text::About), Cmd::About)));
    let mut menus = vec![file, edit, view, program, machine, window, help];
    without_keys(&mut menus);
    MenuBar::new(menus)
}

/// What the window knows when it decides which entries of the menus can be chosen.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MenuFacts {
    /// There is a program.
    pub has_program: bool,
    /// The program can be started, paused, stopped, sent to the server.
    pub can_start: bool,
    pub can_pause: bool,
    pub can_stop: bool,
    pub can_send: bool,
    /// A line of the program is selected, and the program has bookmarks.
    pub has_line: bool,
    pub has_bookmarks: bool,
    /// A transformation can be undone.
    pub can_undo: bool,
    /// The machine is free (nothing runs and no batch is in flight).
    pub idle: bool,
    /// A height map was measured, and the program is corrected with it.
    pub has_map: bool,
    pub corrected: bool,
    /// The guard is on, and it stopped the machine.
    pub guard_on: bool,
    pub guard_stopped: bool,
    /// The path is looked at in 3D.
    pub view_3d: bool,
}

/// The ids of the entries that cannot be chosen now. The ones for things the program does not have (redo, cut, copy, paste) are always off.
pub fn disabled_entries(f: &MenuFacts) -> Vec<String> {
    let mut off: Vec<&str> = vec!["edit.redo", "edit.cut", "edit.copy", "edit.paste"];
    let mut when = |cond: bool, ids: &[&'static str]| {
        if cond {
            off.extend_from_slice(ids);
        }
    };
    when(
        !f.has_program,
        &[
            "file.export",
            "program.rewind",
            "program.clear",
            "program.check",
            "program.keepout",
            "program.stats",
            "program.lookahead",
            "program.post",
            "edit.transform",
            "edit.bookmarks",
        ],
    );
    when(!f.can_start, &["program.start"]);
    when(!f.can_pause, &["program.pause"]);
    when(!f.can_stop, &["program.stop"]);
    when(!f.can_send, &["program.send"]);
    when(!f.has_line, &["edit.bookmark_toggle"]);
    when(
        !f.has_bookmarks,
        &["edit.bookmark_next", "edit.bookmark_prev"],
    );
    when(!f.can_undo, &["edit.undo"]);
    when(
        !f.idle,
        &[
            "machine.home",
            "machine.zero_xyz",
            "machine.zero_xy",
            "machine.go_zero_xy",
            "machine.go_zero_z",
            "machine.go_g28",
            "machine.go_g30",
            "machine.set_g28",
            "machine.set_g30",
            "machine.probe_z",
            "machine.probe_grid",
            "machine.toolsetter",
        ],
    );
    when(
        !f.idle || !f.has_map || f.corrected || !f.has_program,
        &["machine.use_map"],
    );
    when(!f.corrected || !f.idle, &["machine.drop_map"]);
    when(!f.guard_on || !f.guard_stopped, &["machine.reset_guard"]);
    let mut v: Vec<String> = off.into_iter().map(str::to_string).collect();
    v.sort();
    v.dedup();
    v
}

/// The check marks of the entries that are choices of the view.
pub fn checks(f: &MenuFacts) -> Vec<(String, bool)> {
    vec![
        ("view.plan".to_string(), !f.view_3d),
        ("view.3d".to_string(), f.view_3d),
    ]
}

/// Decides when the arrangement of the panels is written to the settings file: when the lines of the frame are not the ones last written (or
/// read). It writes nothing when nothing changed, so dragging a panel writes once and a quiet window writes never.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LayoutWatch {
    saved: String,
}

impl LayoutWatch {
    /// A watch that knows the lines of the frame as they were read.
    pub fn new(pairs: &[(String, String)]) -> LayoutWatch {
        LayoutWatch {
            saved: LayoutWatch::flat(pairs),
        }
    }

    fn flat(pairs: &[(String, String)]) -> String {
        pairs.iter().map(|(k, v)| format!("{k}={v}\n")).collect()
    }

    /// Whether the lines changed since the last time this answered yes (or since the start); a yes counts them as written.
    pub fn changed(&mut self, pairs: &[(String, String)]) -> bool {
        let now = LayoutWatch::flat(pairs);
        if now == self.saved {
            return false;
        }
        self.saved = now;
        true
    }
}

/// How often the arrangement of the panels is compared with what was written, in frames.
pub const LAYOUT_CHECK_EVERY: u32 = 20;

#[cfg(test)]
mod tests {
    use super::*;
    use eh3d_dock::core::DockCore;
    use eh3d_dock::menu::{DockAction, MenuContext};
    use eh3d_dock::sectors::workspace_title;

    fn core() -> DockCore {
        DockCore::new(defs(), workspaces())
    }

    fn entries(bar: &MenuBar<Cmd>) -> Vec<&Entry<Cmd>> {
        fn walk<'a>(items: &'a [Item<Cmd>], out: &mut Vec<&'a Entry<Cmd>>) {
            for i in items {
                match i {
                    Item::Entry(e) => out.push(e),
                    Item::Submenu(m) => walk(&m.items, out),
                    _ => {}
                }
            }
        }
        let mut out = Vec::new();
        for m in &bar.menus {
            walk(&m.items, &mut out);
        }
        out
    }

    fn ids(bar: &MenuBar<Cmd>) -> Vec<String> {
        entries(bar).iter().map(|e| e.id.clone()).collect()
    }

    #[test]
    fn the_panels_have_names_that_hold_and_titles_in_the_eight_languages() {
        let d = defs();
        assert_eq!(d.len(), 12);
        let names: Vec<&str> = d.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(names, id::ALL.to_vec());
        for p in &d {
            assert!(
                p.id.len() <= 48
                    && p.id
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'),
                "{}",
                p.id
            );
            for t in p.title.0 {
                assert!(!t.trim().is_empty(), "{}", p.id);
            }
        }
        // the program and the state of the machine have the titles of this program, not the ones of the sector
        let job = d.iter().find(|p| p.id == id::JOB).unwrap();
        assert_eq!(job.title.0[1], "Programa");
        let position = d.iter().find(|p| p.id == id::POSITION).unwrap();
        assert_eq!(position.title.0[5], "Estado da máquina");
        // the panels the sector proposes keep their titles
        let viewer = d.iter().find(|p| p.id == id::VIEWER).unwrap();
        assert_eq!(viewer.title.0[1], "Visor");
    }

    #[test]
    fn the_four_workspaces_are_sound_and_have_a_name_in_eight_languages() {
        let c = core();
        assert_eq!(c.workspaces().names(), WORKSPACES.to_vec());
        assert!(c.warnings().is_empty(), "{:?}", c.warnings());
        assert!(c.layout_is_sound());
        for name in WORKSPACES {
            assert!(workspace_title(name).is_some(), "{name}");
        }
        // every panel is in some workspace, so the View menu can bring any of them back from a factory layout
        for p in id::ALL {
            let shown = c
                .workspaces()
                .factory()
                .iter()
                .any(|w| w.layout.is_shown(p));
            assert!(shown, "{p} is in no workspace");
        }
    }

    #[test]
    fn each_workspace_shows_what_its_task_needs() {
        let c = core();
        let shown = |ws: &str, p: &str| {
            c.workspaces()
                .factory()
                .iter()
                .find(|w| w.name == ws)
                .is_some_and(|w| w.layout.is_shown(p))
        };
        // Prepare: the files, the import, the tools, the time and the report
        for p in [
            id::FILES,
            id::IMPORT,
            id::TOOL_TABLE,
            id::TOOLSETTER,
            id::PLAN,
            id::REPORT,
            id::VIEWER,
        ] {
            assert!(shown(WS_PREPARE, p), "Prepare: {p}");
        }
        // Run: the state and the hand control beside the path
        for p in [id::POSITION, id::JOG, id::VIEWER, id::JOB, id::CONSOLE] {
            assert!(shown(WS_RUN, p), "Run: {p}");
        }
        assert!(!shown(WS_RUN, id::IMPORT));
        // Review: the checks, the time and the probe
        for p in [id::REPORT, id::PLAN, id::PROBE, id::JOB, id::VIEWER] {
            assert!(shown(WS_REVIEW, p), "Review: {p}");
        }
        // Compact: only the path on show; the console, the jog and the state are folded
        assert!(shown(WS_COMPACT, id::VIEWER));
        for p in [id::CONSOLE, id::JOG, id::POSITION] {
            assert!(!shown(WS_COMPACT, p), "Compact: {p}");
        }
    }

    #[test]
    fn the_menu_bar_has_the_menus_of_the_task_and_nothing_is_wrong_in_it() {
        let bar = menu_bar();
        let names: Vec<&str> = bar.menus.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(
            names,
            vec!["file", "edit", "view", "program", "machine", "window", "help"]
        );
        assert!(bar.validate().is_empty(), "{:?}", bar.validate());
        let all = ids(&bar);
        let mut sorted = all.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), all.len(), "an id is repeated");
        // the keys are the ones of the program: no entry of the menus has a shortcut of its own, so the frame never acts on a key twice
        for e in entries(&bar) {
            assert!(e.shortcut.is_none(), "{}", e.id);
        }
        // the entries of the standard menus that stay: new, open, save, save as, export, quit, undo, preferences
        for need in [
            "file.new",
            "file.open",
            "file.save",
            "file.save_as",
            "file.export",
            "file.quit",
            "edit.undo",
            "edit.preferences",
            "file.import",
            "program.lookahead",
            "program.post",
            "machine.toolsetter",
            "machine.reset_guard",
            "help.keys",
            "help.about",
        ] {
            assert!(all.iter().any(|i| i == need), "{need}");
        }
        // the online help and the update check are not offered: the program has neither
        assert!(!all
            .iter()
            .any(|i| i == "help.online" || i == "help.updates"));
    }

    #[test]
    fn every_entry_of_every_menu_has_a_text_in_each_of_the_eight_languages() {
        let bar = menu_bar();
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
            let mut ctx = MenuContext::empty(lang);
            ctx.workspaces = WORKSPACES.iter().map(|s| s.to_string()).collect();
            for m in bar.resolve(&ctx) {
                assert!(!m.title.trim().is_empty(), "{lang:?} {}", m.id);
            }
            for e in entries(&bar) {
                assert!(!e.label.get(lang).trim().is_empty(), "{lang:?} {}", e.id);
            }
        }
    }

    #[test]
    fn what_cannot_be_done_is_off_and_what_can_is_on() {
        // a window with nothing loaded and a machine at rest: the run, the checks and the time are off; the machine commands are on
        let none = disabled_entries(&MenuFacts {
            idle: true,
            ..MenuFacts::default()
        });
        for off in [
            "program.start",
            "program.pause",
            "program.stop",
            "program.send",
            "program.check",
            "program.lookahead",
            "file.export",
            "edit.bookmark_toggle",
            "machine.use_map",
            "machine.drop_map",
            "machine.reset_guard",
        ] {
            assert!(none.iter().any(|i| i == off), "{off} should be off");
        }
        for on in ["machine.home", "machine.probe_z", "machine.toolsetter"] {
            assert!(!none.iter().any(|i| i == on), "{on} should be on");
        }
        // a busy machine turns the moves off whatever else is true
        let busy = disabled_entries(&MenuFacts {
            has_program: true,
            can_stop: true,
            idle: false,
            ..MenuFacts::default()
        });
        for off in ["machine.home", "machine.probe_grid", "machine.toolsetter"] {
            assert!(busy.iter().any(|i| i == off), "{off}");
        }
        assert!(!busy.iter().any(|i| i == "program.stop"));
        assert!(!busy.iter().any(|i| i == "program.check"));
        // the all clear of the guard is on only when the guard stopped the machine
        let stopped = disabled_entries(&MenuFacts {
            guard_on: true,
            guard_stopped: true,
            idle: true,
            ..MenuFacts::default()
        });
        assert!(!stopped.iter().any(|i| i == "machine.reset_guard"));
        // the height map: the correction needs a map and a program, and the undo needs a correction
        let map = disabled_entries(&MenuFacts {
            has_program: true,
            has_map: true,
            idle: true,
            ..MenuFacts::default()
        });
        assert!(!map.iter().any(|i| i == "machine.use_map"));
        assert!(map.iter().any(|i| i == "machine.drop_map"));
        let corrected = disabled_entries(&MenuFacts {
            has_program: true,
            has_map: true,
            corrected: true,
            idle: true,
            ..MenuFacts::default()
        });
        assert!(corrected.iter().any(|i| i == "machine.use_map"));
        assert!(!corrected.iter().any(|i| i == "machine.drop_map"));
        // the list is sorted and has no repeats, and every id in it is an entry of the bar
        let bar_ids = ids(&menu_bar());
        for f in [none, busy, stopped, map, corrected] {
            let mut s = f.clone();
            s.sort();
            s.dedup();
            assert_eq!(s, f);
            for i in &f {
                assert!(bar_ids.contains(i), "{i} is not an entry");
            }
        }
    }

    #[test]
    fn the_view_choices_are_checked_by_the_way_the_path_is_looked_at() {
        let plan = checks(&MenuFacts::default());
        assert!(plan.contains(&("view.plan".to_string(), true)));
        assert!(plan.contains(&("view.3d".to_string(), false)));
        let three = checks(&MenuFacts {
            view_3d: true,
            ..MenuFacts::default()
        });
        assert!(three.contains(&("view.3d".to_string(), true)));
    }

    #[test]
    fn the_layout_is_written_only_when_it_changed() {
        let mut c = core();
        let mut w = LayoutWatch::new(&c.config_pairs());
        assert!(!w.changed(&c.config_pairs()));
        c.apply(&DockAction::HidePanel(id::CONSOLE.to_string()))
            .unwrap();
        assert!(w.changed(&c.config_pairs()));
        assert!(!w.changed(&c.config_pairs()), "a yes counts as written");
    }

    /// A small generator with a fixed seed (xorshift): the tests that use it give the same run every time.
    struct Seed(u64);
    impl Seed {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn below(&mut self, n: usize) -> usize {
            (self.next() % n as u64) as usize
        }
    }

    #[test]
    fn a_layout_made_by_the_person_comes_back_as_it_was_whatever_was_done_to_it() {
        let mut rng = Seed(0x5EED_C0DE_0000_0001);
        for round in 0..40 {
            let mut c = core();
            for _ in 0..(5 + rng.below(25)) {
                let panel = id::ALL[rng.below(id::ALL.len())].to_string();
                let side = [Side::Left, Side::Right, Side::Top, Side::Bottom][rng.below(4)];
                let action = match rng.below(9) {
                    0 => DockAction::HidePanel(panel),
                    1 => DockAction::ShowPanel(panel),
                    2 => DockAction::TogglePanel(panel),
                    3 => DockAction::FloatPanel(panel),
                    4 => DockAction::DockPanel(panel),
                    5 => DockAction::AutoHidePanel(panel, side),
                    6 => DockAction::MovePanel(panel, side),
                    7 => DockAction::SwitchWorkspace(
                        WORKSPACES[rng.below(WORKSPACES.len())].to_string(),
                    ),
                    _ => DockAction::RestoreLayout,
                };
                // a refused action (a panel that is already where it was asked to be) changes nothing
                let _ = c.apply(&action);
                assert!(c.layout_is_sound(), "round {round}: {action:?}");
            }
            // what is written is read back to the same lines, in a window that starts from the factory layout
            let pairs = c.config_pairs();
            let text = c.config_text("");
            let mut back = core();
            back.load_config(&text);
            assert!(
                back.warnings().is_empty(),
                "round {round}: {:?}",
                back.warnings()
            );
            assert_eq!(back.config_pairs(), pairs, "round {round}");
        }
    }

    #[test]
    fn a_damaged_layout_never_stops_the_window_and_the_factory_one_comes_back() {
        for junk in [
            "dock.version = 1\ndock.ws.1.tree = H200(T0(nothing\n",
            "dock.version = 9\n",
            "dock.version = 1\ndock.current = 99\ndock.ws.1.name = Prepare\ndock.ws.1.tree = T0(viewer,viewer)\n",
            "dock.ws.1.tree = \u{0}\u{1}\u{2}\n",
            "",
        ] {
            let mut c = core();
            c.load_config(junk);
            assert!(c.layout_is_sound(), "{junk:?}");
            assert!(!c.workspaces().names().is_empty(), "{junk:?}");
        }
    }
}
