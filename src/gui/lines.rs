//! Bookmarks on the lines of the program and starting the program from one of its lines. The bookmarks are `bookmarks` and what a line has
//! in force is `startline`, both tested without a window; here there are the lists, the words (those of `words`, in the eight languages of the
//! interface) and the preparation of the start, which is the same one a program left in the middle uses: the controller is put back in the
//! state of the program and the program waits paused at the line, so the machine moves only when the operator presses Resume. Nothing moves
//! before that, and a program that runs, here or on the server, is never touched.

use super::*;
use crate::startline::{self, Note, Refusal, StartCheck};
use crate::words::Msg;

/// The window as it is: the start being looked at, if any, and whether the operator ticked the box that says the machine was checked.
#[derive(Default)]
pub(super) struct LinesState {
    pub check: Option<StartCheck>,
    pub confirmed: bool,
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
                });
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
            ] {
                assert!(!a.start_refusal_text(&r).is_empty());
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
}
