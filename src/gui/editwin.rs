//! The editor of the program in its own window: the text with the numbers of its lines in the margin (the lines the check has something to say
//! about are coloured), find and replace, go to a line, the notes of the check for the text as it is being written, and Apply, which puts the
//! text in the program. Saving goes through the Save window, which asks before it replaces a file. The rules are `editor`, which is tested
//! without a window; this only shows them and does what the buttons ask.

use super::*;
use crate::editor::{self, EditState, Said};
use crate::progcheck::Level;
use crate::report::notes;
use crate::words::Msg;
use eh3d_filedialog::Mode;

impl App {
    /// The editor for the program that is loaded.
    pub(super) fn edit_state(&self) -> Box<EditState> {
        Box::new(EditState::new(&self.session.program))
    }

    /// What the check finds in a text, with the machine and the checks of the settings.
    fn notes_of_text(&self, st: &EditState) -> Vec<crate::report::Note> {
        notes(
            &st.lines(),
            &self.settings.tuning,
            &self.settings.lint,
            &self.report_cfg(),
        )
    }

    /// Puts the text of the editor in the program (what the Apply button does).
    pub(super) fn apply_edit(&mut self, st: &mut EditState) {
        self.session.load(&st.text);
        self.refresh_path();
        st.mark_applied();
    }

    /// The sentence for what the last search or jump said.
    fn said_text(&self, said: &Said) -> String {
        match said {
            Said::Found { count, line } => {
                self.ws(Msg::EdFound, &[count.to_string(), line.to_string()])
            }
            Said::NotFound => self.w(Msg::EdNotFound).to_string(),
            Said::Replaced(n) => self.ws(Msg::EdReplaced, &[n.to_string()]),
            Said::NoLine(n) => self.ws(Msg::EdNoLine, &[n.to_string()]),
        }
    }

    pub(super) fn edit_window(
        &mut self,
        ctx: &egui::Context,
        mut st: Box<EditState>,
    ) -> Option<Win> {
        if st.notes_stale() {
            let n = self.notes_of_text(&st);
            st.set_notes(n);
        }
        let running = self.session.running;
        let mut close = false;
        let mut apply = false;
        let mut save_here = false;
        let mut save_as = false;
        let mut jump_to_line: Option<usize> = None;
        let t = self.theme.clone();
        self.window(self.t(Text::McEditProgram))
            .default_size([720.0, 560.0])
            .show(ctx, |ui| {
                let enter = |r: &egui::Response, ui: &egui::Ui| {
                    r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))
                };
                ui.horizontal_wrapped(|ui| {
                    ui.label(self.w(Msg::EdFind));
                    let f = ui.add(egui::TextEdit::singleline(&mut st.find).desired_width(150.0));
                    if f.changed() {
                        st.from = 0;
                        st.said = None;
                    }
                    if enter(&f, ui) {
                        st.find_next();
                    }
                    ui.label(self.w(Msg::EdReplaceWith));
                    ui.add(egui::TextEdit::singleline(&mut st.with).desired_width(150.0));
                    ui.checkbox(&mut st.match_case, self.w(Msg::EdMatchCase));
                });
                ui.horizontal_wrapped(|ui| {
                    if ui.button(self.w(Msg::EdNext)).clicked() {
                        st.find_next();
                    }
                    if ui.button(self.w(Msg::EdReplaceOne)).clicked() {
                        st.replace_next();
                    }
                    if ui.button(self.w(Msg::EdReplaceAll)).clicked() {
                        st.replace_everything();
                    }
                    ui.separator();
                    ui.label(self.w(Msg::EdGoto));
                    let g = ui.add(egui::TextEdit::singleline(&mut st.goto).desired_width(60.0));
                    if ui.button(self.w(Msg::EdGo)).clicked() || enter(&g, ui) {
                        st.go_to_typed();
                    }
                });
                if let Some(said) = &st.said {
                    ui.colored_label(col(t.dim), self.said_text(said));
                }
                // the text with the numbers of its lines at the side
                let row_h = ui.text_style_height(&egui::TextStyle::Monospace);
                let mut area = egui::ScrollArea::vertical()
                    .id_salt("editwin")
                    .max_height(330.0)
                    .auto_shrink([false, false]);
                let select = st.select.take();
                if let Some((s, _)) = select {
                    let line = editor::line_at(&st.text, s);
                    area = area.vertical_scroll_offset(line.saturating_sub(4) as f32 * row_h);
                }
                area.show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        let font = egui::TextStyle::Monospace.resolve(ui.style());
                        let mut margin = egui::text::LayoutJob::default();
                        for (i, n) in editor::margin_numbers(&st.text).iter().enumerate() {
                            let worst = editor::notes_of_line(&st.notes, i + 1)
                                .map(|x| x.level)
                                .max();
                            let color = match worst {
                                Some(Level::Danger) => t.danger,
                                Some(Level::Warning) => t.warn,
                                _ => t.dim,
                            };
                            let row = if i == 0 { n.clone() } else { format!("\n{n}") };
                            margin.append(
                                &row,
                                0.0,
                                egui::TextFormat {
                                    font_id: font.clone(),
                                    color: col(color),
                                    ..Default::default()
                                },
                            );
                        }
                        ui.label(margin);
                        let mut out = super::clipmenu::text_box_out(
                            ui,
                            egui::TextEdit::multiline(&mut st.text)
                                .font(egui::TextStyle::Monospace)
                                .desired_width(f32::INFINITY)
                                .desired_rows(20),
                        );
                        if let Some((s, e)) = select {
                            let (a, b) = (
                                editor::char_index(&st.text, s),
                                editor::char_index(&st.text, e),
                            );
                            out.state
                                .cursor
                                .set_char_range(Some(egui::text::CCursorRange::two(
                                    egui::text::CCursor::new(a),
                                    egui::text::CCursor::new(b),
                                )));
                            let id = out.response.response.id;
                            out.state.store(ui.ctx(), id);
                            ui.ctx().memory_mut(|m| m.request_focus(id));
                        }
                    });
                });
                // what the check finds in the text, a line each; the number goes to the line
                ui.separator();
                if st.notes.is_empty() {
                    ui.colored_label(col(t.ok), self.w(Msg::EdNoWarnings));
                } else {
                    ui.label(self.ws(Msg::EdWarnings, &[st.notes.len().to_string()]));
                    egui::ScrollArea::vertical()
                        .id_salt("editnotes")
                        .max_height(90.0)
                        .show(ui, |ui| {
                            for n in &st.notes {
                                let color = match n.level {
                                    Level::Danger => t.danger,
                                    Level::Warning => t.warn,
                                    Level::Info => t.dim,
                                };
                                ui.horizontal(|ui| {
                                    if let Some(l) = n.line {
                                        let tip = self.ws(Msg::CheckJump, &[(l + 1).to_string()]);
                                        if ui
                                            .small_button(format!("{:>5}", l + 1))
                                            .on_hover_text(tip)
                                            .clicked()
                                        {
                                            jump_to_line = Some(l + 1);
                                        }
                                    }
                                    ui.colored_label(col(color), &n.text);
                                });
                            }
                        });
                }
                ui.separator();
                if running {
                    ui.colored_label(col(t.warn), self.w(Msg::EdRunning));
                }
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(!running, egui::Button::new(self.t(Text::Apply)))
                        .clicked()
                    {
                        apply = true;
                    }
                    if ui
                        .add_enabled(
                            !running && !self.path.is_empty(),
                            egui::Button::new(self.w(Msg::EdApplySaveHere)),
                        )
                        .clicked()
                    {
                        save_here = true;
                    }
                    if ui
                        .add_enabled(!running, egui::Button::new(self.w(Msg::EdApplySave)))
                        .clicked()
                    {
                        save_as = true;
                    }
                    if ui.button(self.t(Text::Close)).clicked() {
                        close = true;
                    }
                    ui.label(self.ws(Msg::EdLines, &[editor::line_count(&st.text).to_string()]));
                    if st.dirty() {
                        ui.colored_label(col(t.warn), self.w(Msg::EdNotApplied));
                    }
                });
            });
        if let Some(n) = jump_to_line {
            st.go_to(n);
        }
        if apply || save_here || save_as {
            self.apply_edit(&mut st);
        }
        // a file that is there is never written over without the question of the Save window
        if save_here {
            let b = self.browser(Mode::Save);
            return Some(Win::Replace(b, PathBuf::from(&self.path)));
        }
        if save_as {
            return Some(Win::Save(self.browser(Mode::Save)));
        }
        (!close).then_some(Win::Edit(st))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_editor_opens_with_the_program_and_applying_puts_the_text_back() {
        let mut a = App::isolated();
        a.session.load("G21 G90\nG1 X1 F100");
        a.open_window("edit");
        let Some(Win::Edit(mut st)) = a.win.take() else {
            panic!("the editor did not open")
        };
        assert_eq!(st.text, "G21 G90\nG1 X1 F100");
        assert!(!st.dirty());
        st.find = "X1".to_string();
        st.with = "X25".to_string();
        st.replace_everything();
        assert!(st.dirty());
        // nothing changes in the program until it is applied
        assert_eq!(a.session.program[1], "G1 X1 F100");
        a.apply_edit(&mut st);
        assert_eq!(a.session.program, ["G21 G90", "G1 X25 F100"]);
        assert!(!st.dirty());
    }

    #[test]
    fn the_notes_are_those_of_the_text_in_the_editor() {
        let mut a = App::isolated();
        a.session.load("G21 G90\nM3 S10000\nM30");
        let mut st = a.edit_state();
        let n = a.notes_of_text(&st);
        assert!(n.iter().all(|x| !x.problem), "{n:?}");
        st.text = "G21 G90\nG0 X1 Z-3\nM30".to_string();
        let n = a.notes_of_text(&st);
        assert!(
            n.iter()
                .any(|x| x.line == Some(1) && x.level == Level::Danger),
            "{n:?}"
        );
    }

    #[test]
    fn the_window_draws_with_the_margin_the_notes_and_a_selection() {
        let mut a = App::isolated();
        a.session.load("G21 G90\nG0 X1 Z-3\nG1 X5 F100\nM30");
        let ctx = egui::Context::default();
        for select in [None, Some((8, 17))] {
            a.open_window("edit");
            if let Some(Win::Edit(st)) = a.win.as_mut() {
                st.select = select;
            }
            ctx.run_pass(egui::RawInput::default(), |ctx| {
                a.windows(ctx);
            })
            .textures_delta
            .clear();
            assert!(matches!(a.win, Some(Win::Edit(_))));
            if let Some(Win::Edit(st)) = a.win.as_ref() {
                assert_eq!(st.select, None, "the selection is used once");
                assert!(!st.notes.is_empty());
            }
        }
    }
}
