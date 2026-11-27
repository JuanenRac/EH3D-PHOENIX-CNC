//! Bookmarks on the lines of the program and starting the program from one of its lines. The bookmarks are `bookmarks` and what a line has
//! in force is `startline`, both tested without a window; here there are the lists, the words (those of `words`, in the eight languages of the
//! interface) and the preparation of the start, which is the same one a program left in the middle uses: the controller is put back in the
//! state of the program and the program waits paused at the line, so the machine moves only when the operator presses Resume. Nothing moves
//! before that, and a program that runs, here or on the server, is never touched. The bookmarks can be kept in a file next to the program
//! and read again when it is opened (`bookmarks`): the file is written only by its button, and replaced only after a question (unless the
//! settings turn the question off).

use super::*;
use crate::bookmarks::{Loaded, SaveError, Saved};
use crate::startline::{self, Note, Refusal, StartCheck};
use crate::words::Msg;

/// The window as it is: the start being looked at, if any, whether the operator ticked the box that says the machine was checked, what the
/// last button of the file said, and whether the question about replacing the file is showing.
#[derive(Default)]
pub(super) struct LinesState {
    pub check: Option<StartCheck>,
    pub confirmed: bool,
    pub said: Option<(Msg, Vec<String>)>,
    pub ask_replace: bool,
}

/// What saving the bookmarks came to: a thing to say, or the question about replacing a file that is there.
pub(super) enum MarksOutcome {
    Said(Msg, Vec<String>),
    Ask,
}

/// What a click in the window asks for.
enum Act {
    Toggle,
    Clear,
    Remove(usize),
    Jump(usize),
    Look(usize),
    Back,
    Prepare,
    Close,
    Save,
    SaveReplace,
    CancelAsk,
    LoadFile,
}

impl App {
    /// The line the program panel has selected (from 0), when it is a line of the program.
    pub(super) fn selected_line(&self) -> Option<usize> {
        self.focus_line.filter(|&l| l < self.session.program.len())
    }

    /// Puts a bookmark on the selected line, or takes it away. Says whether the line is bookmarked now.
    pub(super) fn toggle_bookmark_selected(&mut self) -> Option<bool> {
        let line = self.selected_line()?;
        Some(self.bookmarks.toggle(line))
    }

    /// Goes to the next bookmark (or the previous one) and shows its line in the program panel.
    pub(super) fn bookmark_step(&mut self, forward: bool) {
        let to = if forward {
            self.bookmarks.next_after(self.focus_line)
        } else {
            self.bookmarks.prev_before(self.focus_line)
        };
        if let Some(l) = to.filter(|&l| l < self.session.program.len()) {
            self.show_line(l);
        }
    }

    /// Reads the bookmarks file next to a program that was just opened, when the settings ask for it, and says in the log what was found:
    /// the bookmarks that belong to this very program are taken, a file written for another version is left out.
    pub(super) fn load_marks_for(&mut self, path: &Path) {
        if !self.settings.tuning.bookmarks_load {
            return;
        }
        let file = crate::bookmarks::marks_path(path);
        let shown = file.display().to_string();
        let said = match crate::bookmarks::Bookmarks::load(&file, &self.session.program) {
            Loaded::Missing => return,
            Loaded::Marks(b) => {
                let n = b.len().to_string();
                self.bookmarks = b;
                self.ws(Msg::BmLoadedFile, &[n, shown])
            }
            Loaded::Stale => self.w(Msg::BmStale).to_string(),
            Loaded::Unusable => self.w(Msg::BmUnreadable).to_string(),
        };
        self.session.log.push(said);
    }

    /// Writes the bookmarks next to the program. A file that is there is replaced when `replace` is true or when the settings turn the
    /// question off; otherwise the answer is `Ask`. A file that has these bookmarks already is left alone.
    pub(super) fn save_marks(&mut self, replace: bool) -> MarksOutcome {
        if self.path.is_empty() {
            return MarksOutcome::Said(Msg::BmNoFile, Vec::new());
        }
        let file = crate::bookmarks::marks_path(Path::new(&self.path));
        let replace = replace || !self.settings.tuning.bookmarks_confirm_replace;
        match self.bookmarks.save(&file, &self.session.program, replace) {
            Ok(Saved::Written) => {
                MarksOutcome::Said(Msg::BmSaved, vec![file.display().to_string()])
            }
            Ok(Saved::Same) => MarksOutcome::Said(Msg::BmSame, Vec::new()),
            Err(SaveError::Exists) => MarksOutcome::Ask,
            Err(SaveError::Io(e)) => MarksOutcome::Said(Msg::FileError, vec![e]),
        }
    }

    /// Adds the bookmarks of the file next to the program to the ones in the window (the ones there are kept). Says what it found.
    pub(super) fn load_marks_button(&mut self) -> (Msg, Vec<String>) {
        if self.path.is_empty() {
            return (Msg::BmNoFile, Vec::new());
        }
        let file = crate::bookmarks::marks_path(Path::new(&self.path));
        let shown = file.display().to_string();
        match crate::bookmarks::Bookmarks::load(&file, &self.session.program) {
            Loaded::Missing => (Msg::BmNoMarksFile, vec![shown]),
            Loaded::Stale => (Msg::BmStale, Vec::new()),
            Loaded::Unusable => (Msg::BmUnreadable, Vec::new()),
            Loaded::Marks(b) => {
                for &l in b.lines() {
                    self.bookmarks.add(l);
                }
                (Msg::BmLoadedFile, vec![b.len().to_string(), shown])
            }
        }
    }

    /// The sentence for something the program has in force at the line it would start from.
    pub(super) fn start_note_text(&self, n: &Note) -> String {
        match *n {
            Note::Inches => self.w(Msg::SlInches).to_string(),
            Note::Plane(p) => self.ws(Msg::SlPlane, &[p.to_string()]),
            Note::Wcs(w) => self.ws(Msg::SlWcs, &[w.to_string()]),
            Note::ToolLength => self.w(Msg::SlToolLength).to_string(),
            Note::Tool(t) => self.ws(Msg::SlTool, &[t.to_string()]),
            Note::Spindle {
                code,
                rpm: Some(rpm),
            } => self.ws(Msg::SlSpindle, &[code.to_string(), rpm.to_string()]),
            Note::Spindle { code, rpm: None } => {
                self.ws(Msg::SlSpindleNoSpeed, &[code.to_string()])
            }
            Note::Coolant(c) => self.ws(Msg::SlCoolant, &[c.to_string()]),
            Note::NoFeed => self.w(Msg::SlNoFeed).to_string(),
            Note::Offset(l) => self.ws(Msg::SlOffset, &[l.to_string()]),
        }
    }

    /// The sentence for why the program cannot start from a line.
    pub(super) fn start_refusal_text(&self, r: &Refusal) -> String {
        match *r {
            Refusal::Empty => self.w(Msg::SlRefEmpty).to_string(),
            Refusal::Relative => self.w(Msg::SlRefRelative).to_string(),
            Refusal::Cycle(l) => self.ws(Msg::SlRefCycle, &[l.to_string()]),
            Refusal::Compensation(l) => self.ws(Msg::SlRefComp, &[l.to_string()]),
            Refusal::Motion(l) => self.ws(Msg::SlRefMotion, &[l.to_string()]),
            Refusal::FeedMode(l) => self.ws(Msg::SlRefFeedMode, &[l.to_string()]),
            Refusal::TableLength(l) => self.ws(Msg::SlRefTable, &[l.to_string()]),
        }
    }

    /// Prepares the program to start from a line (from 0): the controller is brought back to the state the program had there when the
    /// operator presses Resume, and the program waits paused. Nothing is done when a program runs, when the line cannot be started from,
    /// or when the settings ask for the tick (`startline_confirm`) and it was not given. Says whether the program waits at the line.
    pub(super) fn prepare_start_at(&mut self, index: usize, confirmed: bool) -> bool {
        if !self.program_free() || (self.settings.tuning.startline_confirm && !confirmed) {
            return false;
        }
        let c = startline::check(&self.session.program, index);
        if c.refusal.is_some() {
            return false;
        }
        self.session.resume_at_safe(
            c.line - 1,
            self.settings.resume_safe_z_mm,
            self.settings.resume_spindle_wait_s,
        )
    }

    pub(super) fn lines_window(
        &mut self,
        ctx: &egui::Context,
        mut st: Box<LinesState>,
    ) -> Option<Win> {
        let mut act: Option<Act> = None;
        let selected = self.selected_line();
        let free = self.program_free();
        let tick = self.settings.tuning.startline_confirm;
        let t = self.theme.clone();
        let check = st.check.clone();
        let said = st.said.clone();
        let ask = st.ask_replace;
        let has_file = !self.path.is_empty();
        self.window(self.w(Msg::BmTitle))
            .default_width(520.0)
            .show(ctx, |ui| {
                if let Some(c) = &check {
                    ui.label(
                        RichText::new(self.ws(Msg::SlHeading, &[c.line.to_string()])).strong(),
                    );
                    ui.label(RichText::new(c.text.as_str()).monospace());
                    ui.add_space(4.0);
                    match &c.refusal {
                        Some(r) => {
                            ui.colored_label(col(t.danger), self.start_refusal_text(r));
                        }
                        None => {
                            ui.label(self.w(Msg::SlWhat));
                            if c.notes.is_empty() {
                                ui.colored_label(col(t.dim), self.w(Msg::SlNothing));
                            }
                            for n in &c.notes {
                                ui.colored_label(col(t.warn), self.start_note_text(n));
                            }
                            match c.at {
                                Some((x, y)) => ui.label(
                                    self.ws(Msg::SlAt, &[format!("{x:.3}"), format!("{y:.3}")]),
                                ),
                                None => ui.label(self.w(Msg::SlAtNone)),
                            };
                            ui.colored_label(col(t.dim), self.w(Msg::SlWait));
                            if tick {
                                ui.checkbox(&mut st.confirmed, self.w(Msg::SlConfirm));
                            }
                        }
                    }
                    if !free {
                        ui.colored_label(col(t.warn), self.w(Msg::EdRunning));
                    }
                    ui.separator();
                    ui.horizontal(|ui| {
                        let ok = c.refusal.is_none() && free && (st.confirmed || !tick);
                        if ui
                            .add_enabled(ok, egui::Button::new(self.w(Msg::SlPrepare)))
                            .clicked()
                        {
                            act = Some(Act::Prepare);
                        }
                        if ui.button(self.w(Msg::SlBack)).clicked() {
                            act = Some(Act::Back);
                        }
                    });
                    return;
                }
                match selected {
                    Some(l) => {
                        ui.label(self.ws(Msg::BmSelected, &[(l + 1).to_string()]));
                    }
                    None => {
                        ui.colored_label(col(t.dim), self.w(Msg::BmNoSelected));
                    }
                }
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(
                            selected.is_some(),
                            egui::Button::new(self.w(Msg::MenuBookmarkToggle)),
                        )
                        .clicked()
                    {
                        act = Some(Act::Toggle);
                    }
                    if ui
                        .add_enabled(
                            selected.is_some() && free,
                            egui::Button::new(self.w(Msg::BmStartSelected)),
                        )
                        .clicked()
                    {
                        act = selected.map(Act::Look);
                    }
                    if ui
                        .add_enabled(
                            !self.bookmarks.is_empty(),
                            egui::Button::new(self.w(Msg::BmClear)),
                        )
                        .clicked()
                    {
                        act = Some(Act::Clear);
                    }
                    if ui
                        .add_enabled(
                            has_file && !self.bookmarks.is_empty(),
                            egui::Button::new(self.w(Msg::BmSave)),
                        )
                        .clicked()
                    {
                        act = Some(Act::Save);
                    }
                    if ui
                        .add_enabled(has_file, egui::Button::new(self.w(Msg::BmLoadFile)))
                        .clicked()
                    {
                        act = Some(Act::LoadFile);
                    }
                });
                if let Some((m, args)) = &said {
                    ui.colored_label(col(t.dim), self.ws(*m, args));
                }
                if ask {
                    ui.colored_label(col(t.warn), self.w(Msg::BmReplaceAsk));
                    ui.horizontal(|ui| {
                        if ui.button(self.w(Msg::MatReplace)).clicked() {
                            act = Some(Act::SaveReplace);
                        }
                        if ui.button(self.t(Text::McCancel)).clicked() {
                            act = Some(Act::CancelAsk);
                        }
                    });
                }
                ui.separator();
                if self.bookmarks.is_empty() {
                    ui.colored_label(col(t.dim), self.w(Msg::BmNone));
                } else {
                    egui::ScrollArea::vertical()
                        .id_salt("bookmarks")
                        .max_height(260.0)
                        .show(ui, |ui| {
                            for &b in self.bookmarks.lines() {
                                ui.horizontal(|ui| {
                                    if ui.small_button(format!("{:>5}", b + 1)).clicked() {
                                        act = Some(Act::Jump(b));
                                    }
                                    let line =
                                        self.session.program.get(b).map_or("", String::as_str);
                                    let shown: String = line.chars().take(40).collect();
                                    ui.label(RichText::new(shown).monospace());
                                    if ui
                                        .add_enabled(
                                            free,
                                            egui::Button::new(self.w(Msg::BmStartHere)),
                                        )
                                        .clicked()
                                    {
                                        act = Some(Act::Look(b));
                                    }
                                    if ui.small_button(self.w(Msg::BmRemove)).clicked() {
                                        act = Some(Act::Remove(b));
                                    }
                                });
                            }
                        });
                }
                ui.separator();
                if ui.button(self.t(Text::Close)).clicked() {
                    act = Some(Act::Close);
                }
            });
        match act {
            Some(Act::Toggle) => {
                self.toggle_bookmark_selected();
            }
            Some(Act::Clear) => self.bookmarks.clear(),
            Some(Act::Remove(b)) => {
                self.bookmarks.remove(b);
            }
            Some(Act::Jump(b)) => self.show_line(b),
            Some(Act::Look(b)) => {
                st.check = Some(startline::check(&self.session.program, b));
                st.confirmed = false;
            }
            Some(Act::Back) => st.check = None,
            Some(Act::Prepare) => {
                let index = st.check.as_ref().map(|c| c.line - 1);
                if let Some(i) = index {
                    if self.prepare_start_at(i, st.confirmed) {
                        return None;
                    }
                }
            }
            Some(Act::Close) => return None,
            Some(Act::Save) => match self.save_marks(false) {
                MarksOutcome::Said(m, args) => {
                    st.said = Some((m, args));
                    st.ask_replace = false;
                }
                MarksOutcome::Ask => {
                    st.said = None;
                    st.ask_replace = true;
                }
            },
            Some(Act::SaveReplace) => {
                if let MarksOutcome::Said(m, args) = self.save_marks(true) {
                    st.said = Some((m, args));
                }
                st.ask_replace = false;
            }
            Some(Act::CancelAsk) => st.ask_replace = false,
            Some(Act::LoadFile) => {
                st.said = Some(self.load_marks_button());
                st.ask_replace = false;
            }
            None => {}
        }
        Some(Win::Lines(st))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROGRAM: &str =
        "G21 G90\nT2 M6\nM3 S12000\nG0 X10 Y5 Z5\nG1 Z-1 F200\nG1 X20\nG1 X30\nM30";

    fn app_with(program: &str) -> App {
        let mut a = App::isolated();
        a.session.load(program);
        a.refresh_path();
        a
    }

    #[test]
    fn the_selected_line_is_bookmarked_and_the_steps_go_round() {
        let mut a = app_with(PROGRAM);
        assert_eq!(a.toggle_bookmark_selected(), None);
        for l in [6, 2, 4] {
            a.focus_line = Some(l);
            assert_eq!(a.toggle_bookmark_selected(), Some(true));
        }
        assert_eq!(a.bookmarks.lines(), [2, 4, 6]);
        a.focus_line = Some(4);
        a.bookmark_step(true);
        assert_eq!((a.focus_line, a.focus_scroll), (Some(6), true));
        a.bookmark_step(true);
        assert_eq!(a.focus_line, Some(2));
        a.bookmark_step(false);
        assert_eq!(a.focus_line, Some(6));
        a.focus_line = Some(4);
        assert_eq!(a.toggle_bookmark_selected(), Some(false));
        // a line past the end of the program is not a selected line
        a.focus_line = Some(99);
        assert_eq!(a.selected_line(), None);
        // the bookmarks belong to the program: another one drops them
        a.bookmarks.toggle(3);
        let d = std::env::temp_dir().join(format!("eh3d_lines_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let other = d.join("other.nc");
        std::fs::write(&other, "G21\nM30\n").unwrap();
        a.open(&other);
        assert!(a.bookmarks.is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_start_is_prepared_only_with_the_tick_and_waits_paused_at_the_line() {
        let mut a = app_with(PROGRAM);
        assert!(a.settings.tuning.startline_confirm);
        // without the tick nothing is done
        assert!(!a.prepare_start_at(5, false));
        assert!(!a.session.running && a.session.resume_preamble.is_empty());
        assert!(a.prepare_start_at(5, true));
        assert!(a.session.running && a.session.paused);
        assert_eq!(a.session.next, 5);
        let pre = a.session.resume_preamble.join("|");
        assert!(pre.contains("M3 S12000"), "{pre}");
        assert!(pre.contains("G0 X10.0000 Y5.0000"), "{pre}");
        // a program that runs is never touched
        assert!(!a.prepare_start_at(2, true));
        assert_eq!(a.session.next, 5);
        // the tick can be turned off in the settings
        let mut b = app_with(PROGRAM);
        b.settings.tuning.startline_confirm = false;
        assert!(b.prepare_start_at(3, false));
        assert_eq!(b.session.next, 3);
    }

    #[test]
    fn a_start_that_cannot_be_rebuilt_is_refused_and_nothing_is_prepared() {
        let mut a = app_with("G21 G90\nG91\nG1 X1 F10\nG1 X1");
        assert!(!a.prepare_start_at(3, true));
        assert!(!a.session.running && a.session.resume_preamble.is_empty());
        let mut b = app_with("G81 X1 Y1 Z-1 R1 F10\nX2 Y2\nG80\nG0 X0");
        assert!(!b.prepare_start_at(1, true));
        assert!(b.prepare_start_at(3, true));
        let mut e = App::isolated();
        assert!(!e.prepare_start_at(0, true));
    }

    #[test]
    fn what_a_line_has_in_force_is_said_in_the_language() {
        let mut a = app_with(PROGRAM);
        let c = startline::check(&a.session.program, 5);
        let texts: Vec<String> = c.notes.iter().map(|n| a.start_note_text(n)).collect();
        assert_eq!(texts.len(), 2);
        assert!(texts[0].contains("T2"), "{texts:?}");
        assert!(
            texts[1].contains("S12000") || texts[1].contains("12000"),
            "{texts:?}"
        );
        assert!(a.start_refusal_text(&Refusal::Cycle(4)).contains('4'));
        a.settings.lang = Lang::Es;
        a.apply_settings();
        assert!(a.start_note_text(&c.notes[0]).contains("herramienta"));
        // every note and every refusal has words in all the languages
        for lang in Lang::ALL {
            a.settings.lang = lang;
            a.apply_settings();
            for n in [
                Note::Inches,
                Note::Plane(18),
                Note::Wcs(55),
                Note::ToolLength,
                Note::Tool(3),
                Note::Spindle {
                    code: 3,
                    rpm: Some(100.0),
                },
                Note::Spindle { code: 4, rpm: None },
                Note::Coolant(8),
                Note::NoFeed,
                Note::Offset(2),
            ] {
                assert!(!a.start_note_text(&n).is_empty());
            }
            for r in [
                Refusal::Empty,
                Refusal::Relative,
                Refusal::Cycle(1),
                Refusal::Compensation(1),
                Refusal::Motion(1),
                Refusal::FeedMode(1),
                Refusal::TableLength(1),
            ] {
                assert!(!a.start_refusal_text(&r).is_empty());
            }
        }
    }

    #[test]
    fn the_longest_refusals_stay_inside_the_window_in_the_eight_languages() {
        let ctx = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 720.0));
        for lang in eh3d_view::texts::Lang::ALL {
            for program in [
                "G2 X2 Y0 I1 J0 F100\nX4 I1 J0",
                "G93\nG1 X1 F2",
                "G43 H1\nG0 X1",
            ] {
                let mut a = app_with(program);
                a.lang = lang;
                a.open_window("bookmarks");
                if let Some(Win::Lines(st)) = a.win.as_mut() {
                    st.check = Some(startline::check(&a.session.program, 1));
                    assert!(st.check.as_ref().unwrap().refusal.is_some(), "{program}");
                }
                let mut widest = 0.0f32;
                for _ in 0..3 {
                    let input = egui::RawInput {
                        screen_rect: Some(screen),
                        ..Default::default()
                    };
                    let mut out = ctx.run_ui(input, |ui| {
                        let ctx = ui.ctx().clone();
                        a.windows(&ctx);
                    });
                    out.textures_delta.clear();
                    // the window is anchored at the centre: half of it lies to the right of the middle
                    let right = ctx.globally_used_rect().right();
                    widest = 2.0 * (right - screen.center().x);
                }
                // the window is 520 wide: a sentence that did not wrap would stretch it
                assert!(
                    widest > 0.0 && widest <= 700.0,
                    "{lang:?} {program}: {widest}"
                );
            }
        }
    }

    #[test]
    fn the_window_opens_by_its_name_and_draws_in_both_of_its_views() {
        let mut a = app_with(PROGRAM);
        a.bookmarks.toggle(2);
        a.bookmarks.toggle(5);
        a.focus_line = Some(4);
        a.open_window("bookmarks");
        assert!(matches!(a.win, Some(Win::Lines(_))));
        let ctx = egui::Context::default();
        for check in [
            None,
            Some(startline::check(&a.session.program, 5)),
            Some(startline::check(&[], 0)),
        ] {
            if let Some(Win::Lines(st)) = a.win.as_mut() {
                st.check = check;
            }
            ctx.run_pass(egui::RawInput::default(), |ctx| {
                a.windows(ctx);
            })
            .textures_delta
            .clear();
            assert!(matches!(a.win, Some(Win::Lines(_))));
        }
    }

    fn marks_folder(tag: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("phoenix-cnc-marks-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn said(o: MarksOutcome) -> Option<Msg> {
        match o {
            MarksOutcome::Said(m, _) => Some(m),
            MarksOutcome::Ask => None,
        }
    }

    #[test]
    fn the_bookmarks_are_kept_next_to_the_program_and_come_back_when_it_is_opened_again() {
        let d = marks_folder("keep");
        let file = d.join("part.nc");
        std::fs::write(&file, "G21 G90\nG0 X1\nG0 X2\nG0 X3\nG0 X4\nM30\n").unwrap();
        let marks = crate::bookmarks::marks_path(&file);
        let mut a = App::isolated();
        // a program with no file has nowhere to keep them
        assert_eq!(said(a.save_marks(false)), Some(Msg::BmNoFile));
        a.open(&file);
        assert!(a.bookmarks.is_empty());
        a.bookmarks.toggle(1);
        a.bookmarks.toggle(3);
        assert_eq!(said(a.save_marks(false)), Some(Msg::BmSaved));
        assert!(marks.exists());
        // the same bookmarks again: nothing is written and nothing is asked
        assert_eq!(said(a.save_marks(false)), Some(Msg::BmSame));
        // other bookmarks over a file that is there: the question comes first and the file is as it was
        a.bookmarks.toggle(4);
        let before = std::fs::read_to_string(&marks).unwrap();
        assert!(matches!(a.save_marks(false), MarksOutcome::Ask));
        assert_eq!(std::fs::read_to_string(&marks).unwrap(), before);
        // the answer Replace writes it
        assert_eq!(said(a.save_marks(true)), Some(Msg::BmSaved));
        assert_ne!(std::fs::read_to_string(&marks).unwrap(), before);
        // opening the program again brings them back, and the log says so
        let mut b = App::isolated();
        b.open(&file);
        assert_eq!(b.bookmarks.lines(), [1, 3, 4]);
        assert!(b.session.log.iter().any(|l| l.contains("bookmarks read")));
        // with the question turned off the file is replaced at once
        a.settings.tuning.bookmarks_confirm_replace = false;
        a.bookmarks.toggle(0);
        assert_eq!(said(a.save_marks(false)), Some(Msg::BmSaved));
        let mut c = App::isolated();
        c.open(&file);
        assert_eq!(c.bookmarks.lines(), [0, 1, 3, 4]);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_bookmarks_file_is_read_only_when_the_settings_say_so_and_only_for_the_same_program() {
        let d = marks_folder("read");
        let file = d.join("part.nc");
        std::fs::write(&file, "G21 G90\nG0 X1\nG0 X2\nM30\n").unwrap();
        let mut a = App::isolated();
        a.open(&file);
        a.bookmarks.toggle(2);
        assert_eq!(said(a.save_marks(false)), Some(Msg::BmSaved));
        // the settings turn the reading off: the program opens with none, and the button loads them
        let mut off = App::isolated();
        off.settings.tuning.bookmarks_load = false;
        off.open(&file);
        assert!(off.bookmarks.is_empty());
        off.bookmarks.toggle(0);
        let (m, args) = off.load_marks_button();
        assert_eq!((m, args.len()), (Msg::BmLoadedFile, 2));
        assert_eq!(
            off.bookmarks.lines(),
            [0, 2],
            "the ones in the window are kept"
        );
        // a program that was edited since is another one: the file is left out and the log says so
        std::fs::write(&file, "G21 G90\nG0 X9\nG0 X2\nM30\n").unwrap();
        let mut edited = App::isolated();
        edited.open(&file);
        assert!(edited.bookmarks.is_empty());
        assert!(edited
            .session
            .log
            .iter()
            .any(|l| l.contains("another version")));
        assert_eq!(edited.load_marks_button().0, Msg::BmStale);
        // no file at all, and a file that is not a bookmarks file
        let other = d.join("other.nc");
        std::fs::write(&other, "G0 X1\n").unwrap();
        let mut c = App::isolated();
        c.open(&other);
        assert!(c.session.log.iter().all(|l| !l.contains("bookmarks")));
        assert_eq!(c.load_marks_button().0, Msg::BmNoMarksFile);
        std::fs::write(crate::bookmarks::marks_path(&other), "nonsense\n").unwrap();
        assert_eq!(c.load_marks_button().0, Msg::BmUnreadable);
        let _ = std::fs::remove_dir_all(&d);
    }
}
