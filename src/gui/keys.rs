//! The keys of the keyboard: open, save, edit, settings, start (F5), pause (Space), stop (Esc), quit, the list of the keys and the view. Esc also closes a
//! window that is open (it never stops the job then), and with the keyboard in a box of text Space and Esc belong to the box.
//! The table is the one the menus and the list of keys read, so what is written beside a menu entry is what the key does. They can be turned
//! off in the settings (only the mouse then).

use super::*;
use dialogs::Draft;

/// What a key asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Shortcut {
    Open,
    Save,
    Edit,
    Settings,
    Start,
    Pause,
    Stop,
    Quit,
    Help,
    ResetView,
    ToggleView,
}

/// A key, the words it is shown with and the text of what it does.
pub(super) struct Binding {
    pub shortcut: Shortcut,
    pub modifiers: egui::Modifiers,
    pub key: egui::Key,
    pub label: &'static str,
    pub text: Text,
}

const CMD: egui::Modifiers = egui::Modifiers::COMMAND;
const NONE: egui::Modifiers = egui::Modifiers::NONE;

pub(super) const BINDINGS: [Binding; 11] = [
    Binding {
        shortcut: Shortcut::Open,
        modifiers: CMD,
        key: egui::Key::O,
        label: "Ctrl+O",
        text: Text::McOpenGcode,
    },
    Binding {
        shortcut: Shortcut::Save,
        modifiers: CMD,
        key: egui::Key::S,
        label: "Ctrl+S",
        text: Text::McSaveGcode,
    },
    Binding {
        shortcut: Shortcut::Edit,
        modifiers: CMD,
        key: egui::Key::E,
        label: "Ctrl+E",
        text: Text::McEditProgram,
    },
    Binding {
        shortcut: Shortcut::Settings,
        modifiers: CMD,
        key: egui::Key::Comma,
        label: "Ctrl+,",
        text: Text::SettingsTitle,
    },
    Binding {
        shortcut: Shortcut::Start,
        modifiers: NONE,
        key: egui::Key::F5,
        label: "F5",
        text: Text::Start,
    },
    Binding {
        shortcut: Shortcut::Pause,
        modifiers: NONE,
        key: egui::Key::Space,
        label: "Space",
        text: Text::Pause,
    },
    Binding {
        shortcut: Shortcut::Stop,
        modifiers: NONE,
        key: egui::Key::Escape,
        label: "Esc",
        text: Text::Stop,
    },
    Binding {
        shortcut: Shortcut::Quit,
        modifiers: CMD,
        key: egui::Key::Q,
        label: "Ctrl+Q",
        text: Text::MenuExit,
    },
    Binding {
        shortcut: Shortcut::Help,
        modifiers: NONE,
        key: egui::Key::F1,
        label: "F1",
        text: Text::McShortcuts,
    },
    Binding {
        shortcut: Shortcut::ResetView,
        modifiers: CMD,
        key: egui::Key::Num0,
        label: "Ctrl+0",
        text: Text::ResetView,
    },
    Binding {
        shortcut: Shortcut::ToggleView,
        modifiers: NONE,
        key: egui::Key::F3,
        label: "F3",
        text: Text::ViewMachine,
    },
];

/// The words of the key that does this, for the menus.
pub(super) fn label_of(s: Shortcut) -> &'static str {
    BINDINGS
        .iter()
        .find(|b| b.shortcut == s)
        .map_or("", |b| b.label)
}

/// What a key asks for, or nothing when it is not one of ours.
#[cfg(test)]
pub(super) fn shortcut_of(key: egui::Key, modifiers: egui::Modifiers) -> Option<Shortcut> {
    BINDINGS
        .iter()
        .find(|b| b.key == key && b.modifiers == modifiers)
        .map(|b| b.shortcut)
}

impl App {
    /// The key pressed this frame that is one of ours (it is taken, so that no box types it).
    fn pressed_shortcut(&self, ctx: &egui::Context) -> Option<Shortcut> {
        // Space and Esc belong to a box of text that has the keyboard (Space is a letter there, Esc leaves what is being written), and Esc does
        // not close a window while any widget of it has the focus
        // egui lets go of the focus when Esc is pressed, before this runs: the focus of the frame before counts too
        let typing_text = ctx.text_edit_focused() || self.typing_prev;
        let focused = ctx.memory(|m| m.focused().is_some()) || self.focus_prev;
        let window = self.win.is_some();
        ctx.input_mut(|i| {
            BINDINGS
                .iter()
                .filter(|b| {
                    let job_key = matches!(b.shortcut, Shortcut::Pause | Shortcut::Stop);
                    !(typing_text && job_key)
                        && !(focused && window && b.shortcut == Shortcut::Stop)
                })
                .find(|b| i.consume_key(b.modifiers, b.key))
                .map(|b| b.shortcut)
        })
    }

    /// An entry of a menu, with the key written beside it when the keys are on.
    pub(super) fn menu_item(
        &self,
        ui: &mut egui::Ui,
        text: &str,
        s: Shortcut,
        enabled: bool,
    ) -> bool {
        let mut b = egui::Button::new(text);
        if self.settings.shortcuts {
            b = b.shortcut_text(label_of(s));
        }
        ui.add_enabled(enabled, b).clicked()
    }

    /// Called at the end of each frame: whether a box of text, or any widget, had the keyboard (see `pressed_shortcut`).
    pub(super) fn note_focus(&mut self, ctx: &egui::Context) {
        self.typing_prev = ctx.text_edit_focused();
        self.focus_prev = ctx.memory(|m| m.focused().is_some());
    }

    /// Reads the keyboard and does what it asked for.
    pub(super) fn keys(&mut self, ctx: &egui::Context) {
        if !self.settings.shortcuts {
            return;
        }
        if let Some(s) = self.pressed_shortcut(ctx) {
            self.shortcut(s);
        }
    }

    /// Does what a key asks for. While a window is open only Esc (closes it), the list of the keys and the exit work, so that a key does not start
    /// a program behind the window the operator is looking at.
    pub(super) fn shortcut(&mut self, s: Shortcut) {
        if let Some(win) = &self.win {
            match s {
                // Esc is Stop without a window; with one it closes it
                Shortcut::Stop => {
                    // the questions that need an answer are not closed by accident
                    if !matches!(win, Win::Resume(_) | Win::Replace(..)) {
                        self.dialog_error = None;
                        self.win = None;
                    }
                }
                Shortcut::Quit => self.quit = true,
                Shortcut::Help => self.win = Some(Win::Shortcuts),
                _ => {}
            }
            return;
        }
        match s {
            Shortcut::Open => self.open_window("open"),
            Shortcut::Save => self.open_window("save"),
            Shortcut::Edit => self.open_window("edit"),
            Shortcut::Settings => {
                self.win = Some(Win::Settings(Box::new(Draft {
                    s: self.settings.clone(),
                    section: 0,
                })))
            }
            Shortcut::Start => {
                if self.can_start() {
                    self.request_start();
                }
            }
            Shortcut::Pause => {
                if self.can_pause() {
                    self.program_pause();
                }
            }
            Shortcut::Stop => {
                if self.can_stop() {
                    self.program_stop();
                }
            }
            Shortcut::Quit => self.quit = true,
            Shortcut::Help => self.win = Some(Win::Shortcuts),
            Shortcut::ResetView => self.reset_view(),
            Shortcut::ToggleView => self.toggle_view(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        App::isolated()
    }

    #[test]
    fn every_key_of_the_table_is_its_own_and_has_words() {
        for (i, a) in BINDINGS.iter().enumerate() {
            assert!(!a.label.is_empty());
            for b in &BINDINGS[i + 1..] {
                assert!(
                    (a.key, a.modifiers) != (b.key, b.modifiers),
                    "{} and {} are the same key",
                    a.label,
                    b.label
                );
                assert_ne!(a.shortcut, b.shortcut);
            }
        }
        assert_eq!(
            shortcut_of(egui::Key::O, egui::Modifiers::COMMAND),
            Some(Shortcut::Open)
        );
        assert_eq!(
            shortcut_of(egui::Key::F5, egui::Modifiers::NONE),
            Some(Shortcut::Start)
        );
        assert_eq!(shortcut_of(egui::Key::O, egui::Modifiers::NONE), None);
        assert_eq!(label_of(Shortcut::Stop), "Esc");
        assert_eq!(label_of(Shortcut::Pause), "Space");
        assert_eq!(label_of(Shortcut::Start), "F5");
        assert_eq!(
            shortcut_of(egui::Key::Space, egui::Modifiers::NONE),
            Some(Shortcut::Pause)
        );
        assert_eq!(
            shortcut_of(egui::Key::Escape, egui::Modifiers::NONE),
            Some(Shortcut::Stop)
        );
        assert_eq!(shortcut_of(egui::Key::F6, egui::Modifiers::NONE), None);
        assert_eq!(shortcut_of(egui::Key::F8, egui::Modifiers::NONE), None);
    }

    #[test]
    fn the_keys_open_the_windows_and_start_the_program_through_the_summary() {
        let mut a = app();
        a.shortcut(Shortcut::Open);
        assert!(matches!(a.win, Some(Win::Open(_))));
        // while a window is open a key does not reach what is behind it
        a.session.load("G1 X10 F100");
        a.shortcut(Shortcut::Start);
        assert!(!a.session.running);
        a.shortcut(Shortcut::Stop);
        assert!(a.win.is_none());
        a.shortcut(Shortcut::Help);
        assert!(matches!(a.win, Some(Win::Shortcuts)));
        a.shortcut(Shortcut::Stop);
        a.shortcut(Shortcut::Start);
        assert!(
            matches!(a.win, Some(Win::Summary(_))),
            "the summary comes before the start"
        );
        assert!(!a.session.running);
    }

    #[test]
    fn a_question_that_needs_an_answer_is_not_closed_by_esc() {
        let mut a = app();
        a.win = Some(Win::Resume(crate::recovery::Recovery {
            file: "x.nc".into(),
            next: 3,
            total: 9,
            job: String::new(),
            sha256: String::new(),
        }));
        a.shortcut(Shortcut::Stop);
        assert!(matches!(a.win, Some(Win::Resume(_))));
    }

    #[test]
    fn with_the_keys_turned_off_nothing_happens() {
        let mut a = app();
        a.settings.shortcuts = false;
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input.events.push(egui::Event::Key {
            key: egui::Key::F1,
            physical_key: Some(egui::Key::F1),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        ctx.run_pass(input, |ctx| a.keys(ctx))
            .textures_delta
            .clear();
        assert!(a.win.is_none());
        a.settings.shortcuts = true;
        let mut input = egui::RawInput::default();
        input.events.push(egui::Event::Key {
            key: egui::Key::F1,
            physical_key: Some(egui::Key::F1),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        ctx.run_pass(input, |ctx| a.keys(ctx))
            .textures_delta
            .clear();
        assert!(matches!(a.win, Some(Win::Shortcuts)));
    }

    /// One frame of the window with a key pressed; `text_box` shows a box of text that keeps the keyboard.
    fn frame(a: &mut App, ctx: &egui::Context, key: Option<egui::Key>, text_box: bool) {
        let mut input = egui::RawInput::default();
        if let Some(key) = key {
            input.events.push(egui::Event::Key {
                key,
                physical_key: Some(key),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            });
        }
        let mut text = String::new();
        ctx.run_pass(input, |ctx| {
            a.keys(ctx);
            if text_box {
                egui::Area::new(egui::Id::NULL.with("box")).show(ctx, |ui| {
                    let r =
                        ui.add(egui::TextEdit::singleline(&mut text).id(egui::Id::NULL.with("t")));
                    if !r.has_focus() {
                        r.request_focus();
                    }
                });
            }
            a.note_focus(ctx);
        })
        .textures_delta
        .clear();
    }

    fn running_app() -> App {
        let mut a = app();
        a.settings.confirm_start = false;
        a.session.load("G1 X10 F100\nG1 X20\nG1 X30\n");
        a.request_start();
        assert!(a.session.running);
        a
    }

    #[test]
    fn space_pauses_and_escape_stops_the_job() {
        let mut a = running_app();
        let ctx = egui::Context::default();
        frame(&mut a, &ctx, Some(egui::Key::Space), false);
        assert!(a.session.paused, "Space pauses");
        frame(&mut a, &ctx, Some(egui::Key::Escape), false);
        assert!(!a.session.running && !a.session.paused, "Esc stops");
        // F6 and F8 no longer do anything
        let mut a = running_app();
        frame(&mut a, &ctx, Some(egui::Key::F6), false);
        frame(&mut a, &ctx, Some(egui::Key::F8), false);
        assert!(a.session.running && !a.session.paused);
        // F5 starts
        let mut a = app();
        a.session.load("G1 X1 F100");
        frame(&mut a, &ctx, Some(egui::Key::F5), false);
        assert!(matches!(a.win, Some(Win::Summary(_))));
    }

    #[test]
    fn with_the_keyboard_in_a_box_of_text_space_and_escape_do_not_touch_the_job() {
        let mut a = running_app();
        let ctx = egui::Context::default();
        // first frame gives the box the keyboard, the next ones press the keys
        frame(&mut a, &ctx, None, true);
        frame(&mut a, &ctx, None, true);
        assert!(ctx.text_edit_focused(), "the box has the keyboard");
        frame(&mut a, &ctx, Some(egui::Key::Space), true);
        assert!(!a.session.paused, "Space is a letter in the box");
        frame(&mut a, &ctx, Some(egui::Key::Escape), true);
        assert!(
            a.session.running,
            "Esc leaves the box, it does not stop the job"
        );
    }

    #[test]
    fn with_a_window_open_escape_closes_it_and_the_job_goes_on() {
        let mut a = running_app();
        let ctx = egui::Context::default();
        a.win = Some(Win::About);
        frame(&mut a, &ctx, Some(egui::Key::Escape), false);
        assert!(a.win.is_none(), "Esc closes the window");
        assert!(a.session.running, "and it does not stop the job");
        a.win = Some(Win::About);
        frame(&mut a, &ctx, Some(egui::Key::Space), false);
        assert!(
            !a.session.paused,
            "Space does not pause the job behind a window"
        );
        frame(&mut a, &ctx, Some(egui::Key::Escape), false);
        frame(&mut a, &ctx, Some(egui::Key::Escape), false);
        assert!(!a.session.running, "with no window Esc stops");
    }
}
