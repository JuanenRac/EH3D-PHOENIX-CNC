//! The menu of the right button over a text box (Cut, Copy, Paste, Select all, Undo) for every box of the program. The keys Ctrl+X, Ctrl+C and Ctrl+V and
//! Ctrl+Insert, Shift+Insert and Shift+Delete are handled by the window itself (the clipboard is on); the menu asks the window for the same things, and greys
//! out what does not apply.

use eframe::egui;
use eh3d_view::clip::{menu_state, BoxState};
use eh3d_view::texts::{tr, Lang, Text};

/// Does the clipboard hold text? True when that cannot be known (so that Paste is not taken away for nothing).
fn clipboard_has_text() -> bool {
    match arboard::Clipboard::new().and_then(|mut c| c.get_text()) {
        Ok(t) => !t.is_empty(),
        Err(arboard::Error::ContentNotAvailable) => false,
        Err(_) => true,
    }
}

/// A key the menu asks a box to receive in the next frame (Select all, Undo).
#[derive(Clone, Copy)]
struct PendingKey {
    id: egui::Id,
    key: egui::Key,
}

/// Tells the menus the language of the program (once a frame) and gives a box the key its menu asked for.
pub(super) fn start_frame(ctx: &egui::Context, lang: Lang) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique("ui-language"), lang));
    let key_id = egui::Id::unique("pending-text-key");
    let pending = ctx.data(|d| d.get_temp::<PendingKey>(key_id));
    if let Some(p) = pending {
        ctx.data_mut(|d| d.remove::<PendingKey>(key_id));
        ctx.memory_mut(|m| m.request_focus(p.id));
        ctx.input_mut(|i| {
            i.events.push(egui::Event::Key {
                key: p.key,
                physical_key: Some(p.key),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::COMMAND,
            })
        });
    }
}

/// A text box with the menu of the right button.
pub(super) fn text_box(ui: &mut egui::Ui, edit: egui::TextEdit<'_>) -> egui::Response {
    text_box_out(ui, edit).response.response
}

/// The same, giving everything the box says of itself (its state, to move the cursor, and where the cursor is).
pub(super) fn text_box_out(
    ui: &mut egui::Ui,
    edit: egui::TextEdit<'_>,
) -> egui::text_edit::TextEditOutput {
    let out = edit.show(ui);
    let r = out.response.response.clone();
    let ctx = ui.ctx().clone();
    let state_id = r.id.with("menu-state");
    let clip_id = egui::Id::unique("menu-clipboard-has-text");
    // what the box was like while it had the keyboard (the menu is used after it has lost it)
    if let Some(range) = out.cursor_range {
        let has_text = !out.galley.text().is_empty();
        ctx.data_mut(|d| d.insert_temp(state_id, (has_text, !range.is_empty())));
    }
    if r.secondary_clicked() {
        let has = clipboard_has_text();
        ctx.data_mut(|d| d.insert_temp(clip_id, has));
        r.request_focus();
    }
    let (has_text, has_selection) = ctx
        .data(|d| d.get_temp::<(bool, bool)>(state_id))
        .unwrap_or((false, false));
    let st = menu_state(BoxState {
        has_text,
        has_selection,
        read_only: !ui.is_enabled(),
        clipboard_has_text: ctx.data(|d| d.get_temp::<bool>(clip_id)).unwrap_or(true),
    });
    let lang = ctx
        .data(|d| d.get_temp::<Lang>(egui::Id::unique("ui-language")))
        .unwrap_or(Lang::En);
    let word = |t: Text| tr(lang, t);
    let id = r.id;
    r.context_menu(|ui| {
        let mut chosen: Option<egui::ViewportCommand> = None;
        let mut key: Option<egui::Key> = None;
        if ui
            .add_enabled(st.cut, egui::Button::new(word(Text::ClipCut)))
            .clicked()
        {
            chosen = Some(egui::ViewportCommand::RequestCut);
        }
        if ui
            .add_enabled(st.copy, egui::Button::new(word(Text::ClipCopy)))
            .clicked()
        {
            chosen = Some(egui::ViewportCommand::RequestCopy);
        }
        if ui
            .add_enabled(st.paste, egui::Button::new(word(Text::ClipPaste)))
            .clicked()
        {
            chosen = Some(egui::ViewportCommand::RequestPaste);
        }
        ui.separator();
        if ui
            .add_enabled(st.select_all, egui::Button::new(word(Text::ClipSelectAll)))
            .clicked()
        {
            key = Some(egui::Key::A);
        }
        if ui
            .add_enabled(st.undo, egui::Button::new(word(Text::ClipUndo)))
            .clicked()
        {
            key = Some(egui::Key::Z);
        }
        if chosen.is_some() || key.is_some() {
            // the box gets the keyboard back so that it receives what was asked
            ctx.memory_mut(|m| m.request_focus(id));
            if let Some(c) = chosen {
                ctx.send_viewport_cmd(c);
            }
            if let Some(key) = key {
                ctx.data_mut(|d| {
                    d.insert_temp(egui::Id::unique("pending-text-key"), PendingKey { id, key })
                });
                ctx.request_repaint();
            }
            ui.close();
        }
    });
    out
}
