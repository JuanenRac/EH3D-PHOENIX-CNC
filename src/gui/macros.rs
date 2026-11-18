//! The macros in the window: the Macros menu lists the macros of the file `phoenix-cnc-macros.txt` next to the settings, and one that moves the machine, that
//! the checker has a warning for or that asks for it (every one does, unless `macros_confirm = no`) shows its lines and the warnings and waits
//! for the operator's yes. The lines go out like typed ones, one by one, and the macro stops at the first line the controller does not take.
//! The file and its rules are `macros`; the words are those of `words`.

use super::*;
use crate::macros::{Library, Macro};
use crate::progcheck::Level;
use crate::words::Msg;

/// Where the macros are: next to the settings, with the name of the settings file in front (`phoenix-cnc-macros.txt`).
pub(super) fn macros_path(settings: &Path) -> PathBuf {
    let stem = settings.file_stem().map_or_else(
        || "phoenix-cnc".to_string(),
        |s| s.to_string_lossy().to_string(),
    );
    settings.with_file_name(format!("{stem}-macros.txt"))
}

impl App {
    /// The macros of the file now (an empty library when the file is not there or is bigger than 1 MiB).
    pub(super) fn macro_library(&self) -> Library {
        let file = macros_path(&self.settings_path);
        let text = match std::fs::metadata(&file) {
            Ok(m) if m.len() <= 1024 * 1024 => std::fs::read_to_string(&file).unwrap_or_default(),
            _ => String::new(),
        };
        Library::parse(&text, self.settings.tuning.macro_max_lines)
    }

    fn macros_idle(&self) -> bool {
        !self.session.running && !self.session.busy()
    }

    /// The operator chose a macro: it runs at once when nothing needs a confirmation, otherwise its question is asked first.
    pub(super) fn request_macro(&mut self, name: &str) {
        let lib = self.macro_library();
        let Some(m) = lib.get(name) else {
            return;
        };
        if m.needs_confirmation(&self.settings.tuning) {
            self.win = Some(Win::Macro(m.name.clone()));
        } else {
            let name = m.name.clone();
            self.run_macro(&name);
        }
    }

    /// Sends the lines of the macro one by one, like typed ones. It stops at the first line that is not taken and says where; the machine must be
    /// idle (no program running, the controller not busy).
    pub(super) fn run_macro(&mut self, name: &str) {
        let lib = self.macro_library();
        let Some(m) = lib.get(name) else {
            return;
        };
        if !self.macros_idle() {
            let said = self.w(Msg::MacroBusy).to_string();
            self.session.log.push(said);
            return;
        }
        let (macro_name, lines) = (m.name.clone(), m.lines.clone());
        for (n, line) in lines.iter().enumerate() {
            if !self.session.mdi_checked(line) {
                let said = self.ws(
                    Msg::MacroStopped,
                    &[macro_name.clone(), (n + 1).to_string()],
                );
                self.session.log.push(said);
                return;
            }
        }
        let said = self.ws(Msg::MacroSent, &[macro_name, lines.len().to_string()]);
        self.session.log.push(said);
    }

    /// The Macros menu.
    pub(super) fn menu_macros(&mut self, ui: &mut egui::Ui) {
        let mut chosen: Option<String> = None;
        ui.menu_button(self.w(Msg::MenuMacros), |ui| {
            let lib = self.macro_library();
            let idle = self.macros_idle();
            if lib.macros.is_empty() {
                let file = macros_path(&self.settings_path).display().to_string();
                ui.add_enabled(false, egui::Button::new(self.ws(Msg::MacroNone, &[file])));
            }
            if !lib.problems.is_empty() {
                ui.colored_label(
                    col(self.theme.warn),
                    self.ws(Msg::MacroProblems, &[lib.problems.len().to_string()]),
                );
                for p in lib.problems.iter().take(5) {
                    ui.label(RichText::new(p.text(self.lang.code())).color(col(self.theme.dim)));
                }
            }
            for m in &lib.macros {
                let r = ui.add_enabled(idle, egui::Button::new(m.name.as_str()));
                let r = if m.info.is_empty() {
                    r
                } else {
                    r.on_hover_text(m.info.as_str())
                        .on_disabled_hover_text(m.info.as_str())
                };
                if r.clicked() {
                    chosen = Some(m.name.clone());
                    ui.close();
                }
            }
        });
        if let Some(n) = chosen {
            self.request_macro(&n);
        }
    }

    /// The list of the macros in a window: the name, what it says of itself and its lines, with a button to run each one (which asks first when
    /// it has to). Opened by `--window macros` for the pictures.
    pub(super) fn macros_list_window(&mut self, ctx: &egui::Context) -> Option<Win> {
        let lib = self.macro_library();
        let idle = self.macros_idle();
        let mut open = true;
        let mut chosen: Option<String> = None;
        self.window(self.w(Msg::MenuMacros))
            .open(&mut open)
            .default_width(460.0)
            .show(ctx, |ui| {
                if lib.macros.is_empty() {
                    let file = macros_path(&self.settings_path).display().to_string();
                    ui.label(self.ws(Msg::MacroNone, &[file]));
                }
                if !lib.problems.is_empty() {
                    ui.colored_label(
                        col(self.theme.warn),
                        self.ws(Msg::MacroProblems, &[lib.problems.len().to_string()]),
                    );
                    for p in lib.problems.iter().take(5) {
                        ui.label(
                            RichText::new(p.text(self.lang.code())).color(col(self.theme.dim)),
                        );
                    }
                }
                egui::ScrollArea::vertical()
                    .id_salt("macrolist")
                    .max_height(300.0)
                    .show(ui, |ui| {
                        for m in &lib.macros {
                            ui.horizontal(|ui| {
                                if ui
                                    .add_enabled(idle, egui::Button::new(m.name.as_str()))
                                    .clicked()
                                {
                                    chosen = Some(m.name.clone());
                                }
                                if !m.info.is_empty() {
                                    ui.label(
                                        RichText::new(m.info.as_str()).color(col(self.theme.dim)),
                                    );
                                }
                            });
                            for l in m.lines.iter().take(6) {
                                ui.label(RichText::new(l).monospace());
                            }
                            ui.add_space(4.0);
                        }
                    });
            });
        if let Some(n) = chosen {
            self.request_macro(&n);
            return None;
        }
        open.then_some(Win::Macros)
    }

    /// The question before a macro runs: its lines, what the checker says about them, and Run or Cancel.
    pub(super) fn macro_window(&mut self, ctx: &egui::Context, name: String) -> Option<Win> {
        let lib = self.macro_library();
        let m: Macro = lib.get(&name).cloned()?;
        let findings = m.preflight(&self.settings.tuning);
        let lang = self.lang.code();
        let mut answer: Option<bool> = None;
        self.window(self.w(Msg::MacroTitle))
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(m.question(lang));
                if !m.info.is_empty() {
                    ui.label(RichText::new(m.info.as_str()).color(col(self.theme.dim)));
                }
                egui::ScrollArea::vertical()
                    .id_salt("macrolines")
                    .max_height(160.0)
                    .show(ui, |ui| {
                        for l in &m.lines {
                            ui.label(RichText::new(l).monospace());
                        }
                    });
                for f in &findings {
                    let c = if f.level == Level::Danger {
                        self.theme.danger
                    } else {
                        self.theme.warn
                    };
                    ui.colored_label(col(c), f.text(lang));
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button(self.w(Msg::MacroRun)).clicked() {
                        answer = Some(true);
                    }
                    if ui.button(self.t(Text::McCancel)).clicked() {
                        answer = Some(false);
                    }
                });
            });
        match answer {
            Some(true) => {
                self.run_macro(&name);
                None
            }
            Some(false) => None,
            None => Some(Win::Macro(name)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "# macros\n[park]\ninfo = Go to the parking point\nG90\nG53 G0 Z0\n\n[off]\nconfirm = no\nM5\nM9\n\n[bad name]\nM5\n";

    fn app() -> App {
        let a = App::isolated();
        std::fs::write(macros_path(&a.settings_path), FILE).unwrap();
        a
    }

    fn clean(a: &App) {
        let _ = std::fs::remove_file(macros_path(&a.settings_path));
    }

    #[test]
    fn the_macros_are_read_from_the_file_next_to_the_settings() {
        let a = app();
        let lib = a.macro_library();
        assert_eq!(lib.macros.len(), 2);
        assert_eq!(lib.problems.len(), 1, "the macro with a bad name is listed");
        assert!(lib.get("PARK").is_some());
        clean(&a);
        // no file, no macros
        assert!(a.macro_library().macros.is_empty());
    }

    #[test]
    fn a_macro_that_moves_always_asks_and_a_quiet_one_asks_only_when_the_settings_say() {
        let mut a = app();
        a.request_macro("park");
        assert!(matches!(&a.win, Some(Win::Macro(n)) if n == "park"));
        a.win = None;
        // by default every macro asks
        a.request_macro("off");
        assert!(matches!(a.win, Some(Win::Macro(_))));
        a.win = None;
        // with the setting off the quiet one runs at once and the one that moves still asks
        a.settings.tuning.macros_confirm = false;
        a.request_macro("off");
        assert!(a.win.is_none());
        assert!(a
            .session
            .log
            .iter()
            .any(|l| l.contains("macro off: 2 lines sent")));
        a.request_macro("park");
        assert!(matches!(a.win, Some(Win::Macro(_))));
        // a name that is not there does nothing
        a.win = None;
        a.request_macro("nothing");
        assert!(a.win.is_none());
        clean(&a);
    }

    #[test]
    fn the_list_of_macros_opens_by_its_name_and_a_chosen_macro_goes_through_the_same_question() {
        let mut a = app();
        a.open_window("macros");
        assert!(matches!(a.win, Some(Win::Macros)));
        // a window with no file opens too
        clean(&a);
        a.open_window("macros");
        assert!(matches!(a.win, Some(Win::Macros)));
    }

    #[test]
    fn the_lines_go_out_like_typed_ones_and_the_log_says_so() {
        let mut a = app();
        a.run_macro("off");
        let log = a.session.log.join("\n");
        assert!(log.contains("> M5") && log.contains("> M9"), "{log}");
        assert!(log.contains("macro off: 2 lines sent"), "{log}");
        // a line the controller takes says so, an empty one does not
        assert!(a.session.mdi_checked("M5"));
        assert!(!a.session.mdi_checked("   "));
        clean(&a);
    }

    #[test]
    fn a_macro_does_not_run_while_a_program_runs() {
        let mut a = app();
        a.session.running = true;
        a.run_macro("off");
        assert!(a
            .session
            .log
            .iter()
            .any(|l| l.contains("the machine must be idle")));
        assert!(!a.session.log.iter().any(|l| l.contains("> M5")));
        a.session.running = false;
        clean(&a);
    }

    #[test]
    fn the_words_of_the_macros_follow_the_language() {
        let mut a = app();
        a.settings.lang = Lang::De;
        a.apply_settings();
        a.run_macro("off");
        assert!(a
            .session
            .log
            .iter()
            .any(|l| l.contains("Makro off: 2 Zeilen")));
        clean(&a);
    }
}
