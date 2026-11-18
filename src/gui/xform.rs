//! The Transform window of the Program menu: shift, turn, scale and mirror the program that is loaded, and put it back as it was. The
//! change itself is `transform`, which is tested without a window; here there are the fields, the words of the refusals and the copy of the
//! program that Undo brings back. Nothing here moves a machine, and a program that runs is never changed.

use super::*;
use crate::transform::{Refusal, Transform, Why};
use crate::words::Msg;

/// What the window says after the last Apply.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Outcome {
    Done,
    Nothing,
    Refused(Refusal),
}

/// The Transform window as it is: the values and what the last Apply said.
#[derive(Default)]
pub(super) struct XformState {
    pub t: Transform,
    pub said: Option<Outcome>,
}

impl App {
    /// Whether the program can be changed now (not while it runs here or on the server).
    pub(super) fn program_free(&self) -> bool {
        !self.session.running && !self.server_busy()
    }

    /// Transforms the program that is loaded and keeps the one it had for Undo. Says what happened; a program that is refused is not changed.
    pub(super) fn transform_program(&mut self, t: &Transform) -> Outcome {
        if !self.program_free() {
            return Outcome::Nothing;
        }
        let before = self.session.program.clone();
        match t.apply(&before) {
            Err(r) => Outcome::Refused(r),
            Ok(after) if after == before => Outcome::Nothing,
            Ok(after) => {
                self.session.load(&after.join("\n"));
                self.refresh_path();
                self.transform_undo = Some((before, self.session.program.clone()));
                Outcome::Done
            }
        }
    }

    /// Whether the last transformation can be undone: the program is still the one it made.
    pub(super) fn can_undo_transform(&self) -> bool {
        self.program_free()
            && self
                .transform_undo
                .as_ref()
                .is_some_and(|(_, after)| *after == self.session.program)
    }

    /// Puts the program back as it was before the last transformation.
    pub(super) fn undo_transform(&mut self) {
        if !self.can_undo_transform() {
            return;
        }
        if let Some((before, _)) = self.transform_undo.take() {
            self.session.load(&before.join("\n"));
            self.refresh_path();
        }
    }

    /// The sentence for a refusal.
    pub(super) fn refusal_text(&self, r: &Refusal) -> String {
        let msg = match r.why {
            Why::BadScale => Msg::TfWhyScale,
            Why::Relative => Msg::TfWhyRelative,
            Why::Plane => Msg::TfWhyPlane,
            Why::Cycle => Msg::TfWhyCycle,
            Why::Zero => Msg::TfWhyZero,
            Why::AbsoluteCentre => Msg::TfWhyCentre,
        };
        self.ws(msg, &[r.line.to_string()])
    }

    pub(super) fn xform_window(
        &mut self,
        ctx: &egui::Context,
        mut st: Box<XformState>,
    ) -> Option<Win> {
        let mut close = false;
        let mut apply = false;
        let free = self.program_free() && !self.session.program.is_empty();
        let t = self.theme.clone();
        self.window(self.w(Msg::TfTitle))
            .default_width(460.0)
            .show(ctx, |ui| {
                let big = (-1_000_000.0, 1_000_000.0);
                egui::Grid::new("xform")
                    .num_columns(2)
                    .spacing([10.0, 6.0])
                    .show(ui, |ui| {
                        ui.label(self.w(Msg::TfShift));
                        ui.horizontal(|ui| {
                            for (i, n) in ["X", "Y", "Z"].iter().enumerate() {
                                ui.label(*n);
                                numfield::field(
                                    ui,
                                    ("xform-shift", i),
                                    &mut st.t.shift[i],
                                    big,
                                    1.0,
                                    "",
                                );
                            }
                        });
                        ui.end_row();
                        ui.label(self.w(Msg::TfRotate));
                        ui.horizontal(|ui| {
                            numfield::field(
                                ui,
                                "xform-rotate",
                                &mut st.t.rotate_deg,
                                (-360.0, 360.0),
                                5.0,
                                "",
                            );
                            ui.label(self.w(Msg::TfCentre));
                            ui.label("X");
                            numfield::field(ui, "xform-centre-x", &mut st.t.centre.0, big, 1.0, "");
                            ui.label("Y");
                            numfield::field(ui, "xform-centre-y", &mut st.t.centre.1, big, 1.0, "");
                        });
                        ui.end_row();
                        ui.label(self.w(Msg::TfScale));
                        numfield::field(
                            ui,
                            "xform-scale",
                            &mut st.t.scale,
                            (0.001, 1000.0),
                            0.1,
                            "",
                        );
                        ui.end_row();
                    });
                ui.checkbox(&mut st.t.mirror_x, self.w(Msg::TfMirrorX));
                ui.checkbox(&mut st.t.mirror_y, self.w(Msg::TfMirrorY));
                ui.add_space(4.0);
                ui.label(RichText::new(self.w(Msg::TfHint)).color(col(t.dim)).small());
                match st.said {
                    Some(Outcome::Done) => {
                        ui.colored_label(col(t.ok), self.w(Msg::TfDone));
                    }
                    Some(Outcome::Nothing) => {
                        ui.colored_label(col(t.dim), self.w(Msg::TfNothing));
                    }
                    Some(Outcome::Refused(r)) => {
                        ui.colored_label(col(t.danger), self.refusal_text(&r));
                    }
                    None => {}
                }
                if !free {
                    ui.colored_label(col(t.warn), self.w(Msg::EdRunning));
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(free, egui::Button::new(self.t(Text::Apply)))
                        .clicked()
                    {
                        apply = true;
                    }
                    if ui.button(self.t(Text::Close)).clicked() {
                        close = true;
                    }
                });
            });
        if apply {
            st.said = Some(self.transform_program(&st.t));
        }
        (!close).then_some(Win::Transform(st))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_with(program: &str) -> App {
        let mut a = App::isolated();
        a.session.load(program);
        a.refresh_path();
        a
    }

    #[test]
    fn the_program_is_transformed_and_comes_back_with_undo() {
        let mut a = app_with("G21 G90\nG0 X10 Y5\nG1 X20 F100\nM30");
        let t = Transform {
            shift: [1.0, 2.0, 0.0],
            ..Transform::default()
        };
        assert!(!a.can_undo_transform());
        assert_eq!(a.transform_program(&t), Outcome::Done);
        assert_eq!(a.session.program[1], "G0 X11 Y7");
        assert_eq!(a.session.program[2], "G1 X21 F100");
        // the path in the window follows the new program
        assert_eq!(a.path_toolpath[0].to, (11.0, 7.0, 0.0));
        assert!(a.can_undo_transform());
        a.undo_transform();
        assert_eq!(a.session.program[1], "G0 X10 Y5");
        assert_eq!(a.path_toolpath[0].to, (10.0, 5.0, 0.0));
        assert!(!a.can_undo_transform());
        // nothing to change says so and changes nothing
        assert_eq!(a.transform_program(&Transform::default()), Outcome::Nothing);
        assert!(!a.can_undo_transform());
    }

    #[test]
    fn a_program_that_cannot_be_transformed_is_left_as_it_is_and_the_reason_is_said_in_the_language(
    ) {
        let mut a = app_with("G21\nG91\nG1 X1");
        let t = Transform {
            rotate_deg: 10.0,
            ..Transform::default()
        };
        let before = a.session.program.clone();
        let r = a.transform_program(&t);
        assert_eq!(
            r,
            Outcome::Refused(Refusal {
                why: Why::Relative,
                line: 2
            })
        );
        assert_eq!(a.session.program, before);
        assert!(!a.can_undo_transform());
        let Outcome::Refused(r) = r else {
            unreachable!()
        };
        assert_eq!(
            a.refusal_text(&r),
            "Line 2: relative distances (G91) cannot be transformed"
        );
        a.settings.lang = Lang::Es;
        a.apply_settings();
        assert!(a.refusal_text(&r).starts_with("Línea 2:"));
    }

    #[test]
    fn a_program_that_runs_is_never_changed_and_undo_goes_when_the_program_is_not_the_one_it_made()
    {
        let mut a = app_with("G0 X1");
        let t = Transform {
            shift: [1.0, 0.0, 0.0],
            ..Transform::default()
        };
        a.session.running = true;
        assert_eq!(a.transform_program(&t), Outcome::Nothing);
        assert_eq!(a.session.program[0], "G0 X1");
        a.session.running = false;
        assert_eq!(a.transform_program(&t), Outcome::Done);
        // another program loaded: the copy is not offered any more
        a.session.load("G0 X50");
        assert!(!a.can_undo_transform());
        a.undo_transform();
        assert_eq!(a.session.program[0], "G0 X50");
    }

    #[test]
    fn the_window_opens_by_its_name_and_draws_with_every_kind_of_result() {
        let mut a = app_with("G21 G90\nG1 X1 F10");
        a.open_window("transform");
        assert!(matches!(a.win, Some(Win::Transform(_))));
        let ctx = egui::Context::default();
        for said in [
            None,
            Some(Outcome::Done),
            Some(Outcome::Nothing),
            Some(Outcome::Refused(Refusal {
                why: Why::Zero,
                line: 3,
            })),
        ] {
            if let Some(Win::Transform(st)) = a.win.as_mut() {
                st.said = said;
            }
            ctx.run_pass(egui::RawInput::default(), |ctx| {
                a.windows(ctx);
            })
            .textures_delta
            .clear();
            assert!(matches!(a.win, Some(Win::Transform(_))));
        }
    }
}
