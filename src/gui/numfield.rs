//! A number the operator can set in two ways: step it with the minus and plus buttons, or click it and write the value (Enter or a click
//! elsewhere accepts it, Esc leaves it as it was). What is written is read with a point or a comma and kept inside the range of the field.

use super::*;

/// Reads what was written: a point or a comma for the decimals, spaces around, and the unit if it was typed; `None` when it is not a number.
/// A number out of the range is brought to the nearest end of it.
pub(super) fn parse(text: &str, (lo, hi): (f64, f64)) -> Option<f64> {
    let t = text.trim().replace(',', ".");
    let t = t.trim_end_matches(|c: char| c.is_alphabetic() || c == '/' || c == ' ' || c == '%');
    let v: f64 = t.parse().ok()?;
    v.is_finite().then(|| v.clamp(lo, hi))
}

/// The number as it is written in the box: no trailing zeros, no noise of the binary fractions.
pub(super) fn show(v: f64) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-" || s == "-0" {
        "0".to_string()
    } else {
        s.to_string()
    }
}

/// The value one step up (`dir` > 0) or down, kept inside the range. The result is rounded to the decimals of the step, so that 0.1 + 0.2
/// is 0.3 and not 0.30000000000000004.
pub(super) fn stepped(v: f64, dir: f64, step: f64, (lo, hi): (f64, f64)) -> f64 {
    let raw = v + dir.signum() * step;
    let k = 1.0e6;
    ((raw * k).round() / k).clamp(lo, hi)
}

/// `[-] [value] [+]` with the unit after it. Returns whether the value changed. `id` tells this field from the others of the window.
pub(super) fn field(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    value: &mut f64,
    range: (f64, f64),
    step: f64,
    suffix: &str,
) -> bool {
    let mut changed = false;
    let tid = ui.make_persistent_id(id);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        if ui.small_button("-").clicked() {
            *value = stepped(*value, -1.0, step, range);
            changed = true;
        }
        // while the box has the keyboard it keeps what is being written; otherwise it shows the value
        let editing = ui.memory(|m| m.has_focus(tid));
        let mut text = if editing {
            ui.data_mut(|d| d.get_temp::<String>(tid))
                .unwrap_or_else(|| show(*value))
        } else {
            show(*value)
        };
        let r = super::clipmenu::text_box(
            ui,
            egui::TextEdit::singleline(&mut text)
                .id(tid)
                .desired_width(70.0)
                .align(egui::Align2::RIGHT_CENTER),
        );
        if r.gained_focus() {
            // all of it is selected, so that writing replaces the value
            if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), tid) {
                state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::two(
                        egui::text::CCursor::new(0),
                        egui::text::CCursor::new(text.chars().count()),
                    )));
                state.store(ui.ctx(), tid);
            }
        }
        if r.has_focus() {
            ui.data_mut(|d| d.insert_temp(tid, text.clone()));
        }
        if r.lost_focus() {
            let escaped = ui.input(|i| i.key_pressed(egui::Key::Escape));
            ui.data_mut(|d| d.remove::<String>(tid));
            if !escaped {
                if let Some(v) = parse(&text, range) {
                    if (v - *value).abs() > 1.0e-12 {
                        *value = v;
                        changed = true;
                    }
                }
            }
        }
        if ui.small_button("+").clicked() {
            *value = stepped(*value, 1.0, step, range);
            changed = true;
        }
        if !suffix.is_empty() {
            ui.label(suffix);
        }
    });
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_written_is_read_with_a_point_or_a_comma_and_kept_in_the_range() {
        let r = (-10.0, 100.0);
        assert_eq!(parse("12.5", r), Some(12.5));
        assert_eq!(parse(" 12,5 ", r), Some(12.5));
        assert_eq!(parse("-3", r), Some(-3.0));
        assert_eq!(parse("250 mm", r), Some(100.0));
        assert_eq!(parse("-99", r), Some(-10.0));
        assert_eq!(parse("1500 mm/min", (0.0, 5000.0)), Some(1500.0));
        assert_eq!(parse("", r), None);
        assert_eq!(parse("abc", r), None);
        assert_eq!(parse("1.2.3", r), None);
        assert_eq!(parse("inf", r), None);
        assert_eq!(parse("NaN", r), None);
    }

    #[test]
    fn a_value_is_shown_without_trailing_zeros_or_binary_noise() {
        assert_eq!(show(12.0), "12");
        assert_eq!(show(0.1 + 0.2), "0.3");
        assert_eq!(show(-0.00001), "0");
        assert_eq!(show(2.5), "2.5");
        assert_eq!(show(-7.25), "-7.25");
    }

    #[test]
    fn the_buttons_step_the_value_and_stop_at_the_ends_of_the_range() {
        let r = (0.0, 1.0);
        assert_eq!(stepped(0.1, 1.0, 0.2, r), 0.3);
        assert_eq!(stepped(0.9, 1.0, 0.2, r), 1.0);
        assert_eq!(stepped(0.1, -1.0, 0.2, r), 0.0);
        assert_eq!(stepped(5.0, -3.0, 1.0, (0.0, 10.0)), 4.0);
    }
}
