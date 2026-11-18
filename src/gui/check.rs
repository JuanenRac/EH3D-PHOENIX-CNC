//! The check of the program in the window: everything the two checkers, the keep-out boxes and the travel find in the program that is
//! loaded, one line each, with the number of the line it is about. A click on the number shows that line in the program panel. The check
//! itself is `report::notes`; here there is only the list and the words (those of `words`, in the eight languages of the interface).

use super::*;
use crate::progcheck::Level;
use crate::report::{notes, Cfg, Note};
use crate::words::{say, text, Msg};

/// What the open Check window shows: the notes and the size of the program they were made for (the list is made again when it changes).
pub(super) struct CheckState {
    pub program_len: usize,
    pub notes: Vec<Note>,
}

impl App {
    /// A message of the program tools in the language of the window.
    pub(super) fn w(&self, m: Msg) -> &'static str {
        text(self.lang.code(), m)
    }

    /// A message of the program tools with its holes filled.
    pub(super) fn ws(&self, m: Msg, args: &[String]) -> String {
        say(self.lang.code(), m, args)
    }

    /// What the report needs to know about the machine, from the settings and from where the zero of the work is now.
    pub(super) fn report_cfg(&self) -> Cfg {
        let c = self.checks();
        Cfg {
            travel: c.travel,
            offset: c.offset,
            rapid_mm_min: c.rapid_mm_min,
            default_feed: c.default_feed,
            lang: self.lang.code().to_string(),
        }
    }

    /// Everything that looks wrong in the program that is loaded.
    pub(super) fn program_notes(&self) -> Vec<Note> {
        notes(
            &self.session.program,
            &self.settings.tuning,
            &self.settings.lint,
            &self.report_cfg(),
        )
    }

    /// The Check window for the program that is loaded.
    pub(super) fn check_state(&self) -> CheckState {
        CheckState {
            program_len: self.session.program.len(),
            notes: self.program_notes(),
        }
    }

    /// Shows a line of the program in the program panel (the panel scrolls to it once).
    pub(super) fn show_line(&mut self, line: usize) {
        self.focus_line = Some(line);
        self.focus_scroll = true;
    }

    pub(super) fn check_window(
        &mut self,
        ctx: &egui::Context,
        mut st: Box<CheckState>,
    ) -> Option<Win> {
        if st.program_len != self.session.program.len() {
            st = Box::new(self.check_state());
        }
        let problems = st.notes.iter().filter(|n| n.problem).count();
        let mut close = false;
        let mut jump: Option<usize> = None;
        self.window(self.w(Msg::CheckTitle))
            .default_width(600.0)
            .show(ctx, |ui| {
                if self.session.program.is_empty() {
                    ui.label(self.t(Text::NoFile));
                } else {
                    ui.label(self.ws(
                        Msg::CheckCounts,
                        &[
                            problems.to_string(),
                            (st.notes.len() - problems).to_string(),
                        ],
                    ));
                    if st.notes.is_empty() {
                        ui.colored_label(col(self.theme.ok), self.w(Msg::ReportClean));
                    }
                    egui::ScrollArea::vertical()
                        .id_salt("checknotes")
                        .max_height(360.0)
                        .show(ui, |ui| {
                            for n in &st.notes {
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
                }
                ui.separator();
                if ui.button(self.t(Text::Close)).clicked() {
                    close = true;
                }
            });
        if let Some(l) = jump {
            self.show_line(l);
        }
        (!close).then_some(Win::Check(st))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        App::isolated()
    }

    #[test]
    fn the_notes_of_the_loaded_program_come_with_their_lines() {
        let mut a = app();
        a.session.load("G21 G90\nG0 X1 Z-3\nM30");
        let n = a.program_notes();
        // the rapid move under the surface is on the second line (index 1)
        assert!(
            n.iter()
                .any(|x| x.line == Some(1) && x.level == Level::Danger),
            "{n:?}"
        );
        // a good program has nothing to say
        a.session
            .load("G21 G90\nM3 S10000\nG0 X10 Y10\nG1 Z-1 F200\nG1 X20\nG0 Z0\nM5\nM30");
        assert!(a.program_notes().is_empty(), "{:?}", a.program_notes());
    }

    #[test]
    fn the_window_opens_by_its_name_and_a_line_is_shown_once() {
        let mut a = app();
        a.session.load("G21\nG1 X1 F100");
        a.open_window("check");
        assert!(matches!(a.win, Some(Win::Check(_))));
        a.show_line(1);
        assert_eq!((a.focus_line, a.focus_scroll), (Some(1), true));
    }

    #[test]
    fn the_notes_follow_the_language_and_the_keep_out_boxes_of_the_settings() {
        let mut a = app();
        a.settings.lang = Lang::De;
        a.apply_settings();
        a.settings.tuning.keepouts =
            vec![crate::boxes::KeepOut::parse("clamp -5 -5 -5 5 5 5").unwrap()];
        a.session
            .load("G21 G90\nM3 S10000\nG0 X1 Y1\nG1 Z1 F100\nM30");
        let n = a.program_notes();
        assert!(n.iter().any(|x| x.text.contains("Sperrbox")), "{n:?}");
    }

    #[test]
    fn the_messages_of_the_tools_are_in_the_language_of_the_window() {
        let mut a = app();
        assert_eq!(a.w(Msg::CheckTitle), "Program check");
        a.settings.lang = Lang::Pt;
        a.apply_settings();
        assert_eq!(a.w(Msg::CheckTitle), "Verificação do programa");
        assert_eq!(
            a.ws(Msg::CheckCounts, &["2".to_string(), "5".to_string()]),
            "2 para ler, 5 notas"
        );
    }
}
