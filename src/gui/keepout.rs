//! The keep-out boxes in the window: the places of the table where the tool must not go (a clamp, a vise). A small editor lists the boxes of
//! the settings, takes one more (a name and the two corners in mm of the machine) and takes one out; a program with a move that enters a
//! box is refused at the start unless the settings say `keepout_reject = no`. The test of the path is `boxes`; the boxes are written in
//! the settings file with the other values of the tools.

use super::*;
use crate::boxes::{hits_curved, KeepOut};
use crate::heightmap::num;
use crate::words::Msg;

/// The most boxes the settings keep (the same limit as the reading of the file).
const MAX_BOXES: usize = 64;

/// What is being written in the editor.
#[derive(Default)]
pub(super) struct KeepState {
    pub name: String,
    /// The two corners: x0 y0 z0 x1 y1 z1.
    pub nums: [String; 6],
    /// The last box written was not valid.
    pub bad: bool,
}

impl App {
    /// Adds a box (a name that is there already is replaced). Says whether the box was valid and there was room; the settings are saved.
    pub(super) fn add_keepout(&mut self, name: &str, nums: &[String; 6]) -> bool {
        let text = format!(
            "{} {}",
            name.trim(),
            nums.iter()
                .map(|n| n.trim().replace(',', "."))
                .collect::<Vec<_>>()
                .join(" ")
        );
        let Some(b) = KeepOut::parse(&text) else {
            return false;
        };
        let boxes = &mut self.settings.tuning.keepouts;
        boxes.retain(|k| k.name != b.name);
        if boxes.len() >= MAX_BOXES {
            return false;
        }
        boxes.push(b);
        self.save_settings();
        true
    }

    /// Takes the box in place `i` out.
    pub(super) fn remove_keepout(&mut self, i: usize) -> bool {
        if i < self.settings.tuning.keepouts.len() {
            self.settings.tuning.keepouts.remove(i);
            self.save_settings();
            true
        } else {
            false
        }
    }

    /// The reason the start is refused when a move of the program enters a box (and the settings say to refuse), with the first line it happens at.
    pub(super) fn keepout_refusal(&self) -> Option<String> {
        let t = &self.settings.tuning;
        if !t.keepout_reject || t.keepouts.is_empty() {
            return None;
        }
        let offset = self.work_offset();
        let curves =
            crate::gcode::arc_curves(&self.session.program, crate::report::KEEPOUT_STEP_MM);
        let path = hits_curved(
            &self.path_toolpath,
            &curves,
            offset,
            &t.keepouts,
            t.keepout_margin,
        );
        if let Some(first) = path.into_iter().next() {
            return Some(self.ws(
                Msg::KeepRefused,
                &[(first.line + 1).to_string(), first.name],
            ));
        }
        // then the moves of the tool changes, when the changer is on
        let change = crate::atc::collisions(
            &self.session.program,
            &t.atc,
            offset,
            &t.keepouts,
            t.keepout_margin,
        )
        .into_iter()
        .next()?;
        Some(self.ws(
            Msg::AtcBoxRefused,
            &[
                (change.line + 1).to_string(),
                change.tool.to_string(),
                change.name,
            ],
        ))
    }

    pub(super) fn keepout_window(
        &mut self,
        ctx: &egui::Context,
        mut st: Box<KeepState>,
    ) -> Option<Win> {
        let mut close = false;
        let mut add = false;
        let mut remove: Option<usize> = None;
        let boxes = self.settings.tuning.keepouts.clone();
        let mut name = std::mem::take(&mut st.name);
        let mut nums = std::mem::take(&mut st.nums);
        let bad = st.bad;
        self.window(self.w(Msg::KeepTitle))
            .default_width(620.0)
            .show(ctx, |ui| {
                ui.label(self.ws(Msg::KeepHelp, &[num(self.settings.tuning.keepout_margin)]));
                ui.separator();
                if boxes.is_empty() {
                    ui.label(self.w(Msg::KeepNone));
                }
                egui::ScrollArea::vertical()
                    .id_salt("keepboxes")
                    .max_height(200.0)
                    .show(ui, |ui| {
                        for (i, b) in boxes.iter().enumerate() {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(format!(
                                        "{:<14} {} {} {}  ->  {} {} {}",
                                        b.name,
                                        num(b.min[0]),
                                        num(b.min[1]),
                                        num(b.min[2]),
                                        num(b.max[0]),
                                        num(b.max[1]),
                                        num(b.max[2])
                                    ))
                                    .monospace(),
                                );
                                if ui.small_button(self.w(Msg::KeepRemove)).clicked() {
                                    remove = Some(i);
                                }
                            });
                        }
                    });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(self.w(Msg::KeepName));
                    super::clipmenu::text_box(
                        ui,
                        egui::TextEdit::singleline(&mut name).desired_width(90.0),
                    );
                    for (k, label) in ["x0", "y0", "z0", "x1", "y1", "z1"].into_iter().enumerate() {
                        ui.label(label);
                        super::clipmenu::text_box(
                            ui,
                            egui::TextEdit::singleline(&mut nums[k]).desired_width(52.0),
                        );
                    }
                });
                if bad {
                    ui.colored_label(col(self.theme.danger), self.w(Msg::KeepBad));
                }
                ui.horizontal(|ui| {
                    if ui.button(self.w(Msg::KeepAdd)).clicked() {
                        add = true;
                    }
                    if ui.button(self.t(Text::Close)).clicked() {
                        close = true;
                    }
                });
            });
        if let Some(i) = remove {
            self.remove_keepout(i);
        }
        if add {
            if self.add_keepout(&name, &nums) {
                name.clear();
                nums = Default::default();
                st.bad = false;
            } else {
                st.bad = true;
            }
        }
        st.name = name;
        st.nums = nums;
        (!close).then_some(Win::KeepOut(st))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        App::isolated()
    }

    fn nums(v: [&str; 6]) -> [String; 6] {
        v.map(String::from)
    }

    #[test]
    fn a_box_is_added_replaced_saved_and_taken_out() {
        let mut a = app();
        assert!(a.add_keepout("clamp", &nums(["10", "0", "-10", "20", "20", "10"])));
        // a decimal comma is read, the corners may come in any order
        assert!(a.add_keepout("vise", &nums(["50,5", "0", "0", "40", "10", "5"])));
        assert_eq!(a.settings.tuning.keepouts.len(), 2);
        assert_eq!(a.settings.tuning.keepouts[1].min[0], 40.0);
        assert_eq!(a.settings.tuning.keepouts[1].max[0], 50.5);
        // the same name replaces the box
        assert!(a.add_keepout("clamp", &nums(["0", "0", "0", "1", "1", "1"])));
        assert_eq!(a.settings.tuning.keepouts.len(), 2);
        assert_eq!(a.settings.tuning.keepouts[1].name, "clamp");
        // the file has them, with the other values of the settings
        let text = std::fs::read_to_string(&a.settings_path).unwrap();
        assert!(text.contains("keepout = vise") && text.contains("keepout = clamp"));
        assert!(text.contains("lang = en"));
        assert!(a.remove_keepout(0));
        assert!(!a.remove_keepout(5));
        assert_eq!(a.settings.tuning.keepouts.len(), 1);
        let _ = std::fs::remove_file(&a.settings_path);
    }

    #[test]
    fn a_box_that_is_not_valid_is_refused_and_changes_nothing() {
        let mut a = app();
        assert!(!a.add_keepout("", &nums(["0", "0", "0", "1", "1", "1"])));
        assert!(!a.add_keepout("bad name!", &nums(["0", "0", "0", "1", "1", "1"])));
        assert!(!a.add_keepout("a", &nums(["0", "0", "0", "1", "1", ""])));
        assert!(!a.add_keepout("a", &nums(["0", "0", "0", "1", "1", "x"])));
        // a box with no volume
        assert!(!a.add_keepout("flat", &nums(["0", "0", "0", "1", "1", "0"])));
        assert!(a.settings.tuning.keepouts.is_empty());
    }

    #[test]
    fn there_is_room_for_sixty_four_boxes() {
        let mut a = app();
        for i in 0..64 {
            assert!(a.add_keepout(&format!("b{i}"), &nums(["0", "0", "0", "1", "1", "1"])));
        }
        assert!(!a.add_keepout("one_more", &nums(["0", "0", "0", "1", "1", "1"])));
        assert_eq!(a.settings.tuning.keepouts.len(), 64);
        let _ = std::fs::remove_file(&a.settings_path);
    }

    fn with_a_clamp() -> App {
        let mut a = app();
        a.add_keepout("clamp", &nums(["10", "0", "-10", "20", "20", "10"]));
        a.session
            .load("G21 G90\nM3 S10000\nG0 Y5\nG1 X15 F100\nM30");
        a.path_toolpath = gcode::toolpath(&a.session.program);
        a
    }

    #[test]
    fn a_program_that_enters_a_box_is_refused_with_its_line_and_the_box() {
        let mut a = with_a_clamp();
        let why = a.keepout_refusal().unwrap();
        assert!(why.contains('4') && why.contains("clamp"), "{why}");
        assert!(a.start_refused());
        assert!(a.session.log.iter().any(|l| l.contains("clamp")));
        // with the summary off the start is refused all the same
        a.settings.confirm_start = false;
        a.request_start();
        assert!(!a.session.running);
        let _ = std::fs::remove_file(&a.settings_path);
    }

    #[test]
    fn the_refusal_can_be_turned_off_and_a_program_clear_of_the_boxes_starts() {
        let mut a = with_a_clamp();
        a.settings.tuning.keepout_reject = false;
        assert_eq!(a.keepout_refusal(), None);
        a.settings.tuning.keepout_reject = true;
        a.session
            .load("G21 G90\nM3 S10000\nG0 Y50\nG1 X15 F100\nM30");
        a.path_toolpath = gcode::toolpath(&a.session.program);
        assert_eq!(a.keepout_refusal(), None);
        assert!(!a.start_refused());
        let _ = std::fs::remove_file(&a.settings_path);
    }

    #[test]
    fn the_editor_opens_by_its_name() {
        let mut a = app();
        a.open_window("keepout");
        assert!(matches!(a.win, Some(Win::KeepOut(_))));
    }

    #[test]
    fn a_tool_change_that_goes_through_a_box_refuses_the_start_while_the_changer_is_on() {
        let mut a = app();
        a.settings.tuning.atc = crate::atc::AtcCfg {
            enabled: true,
            safe_z: 10.0,
            rack: [100.0, 50.0, -5.0],
            pitch: [30.0, 0.0],
            ..Default::default()
        };
        a.add_keepout("post", &nums(["125", "45", "-8", "135", "55", "5"]));
        a.session
            .load("G21 G90\nM3 S10000\nG0 X0 Y0 Z10\nT1 M6\nG0 X0 Y0 Z10\nT2 M6\nM30");
        a.path_toolpath = gcode::toolpath(&a.session.program);
        let why = a.keepout_refusal().unwrap();
        assert!(why.contains("T2") && why.contains("post"), "{why}");
        assert!(a.start_refused());
        // with the changer off the program itself is clear of the box and starts
        a.settings.tuning.atc.enabled = false;
        assert_eq!(a.keepout_refusal(), None);
        assert!(!a.start_refused());
        // with the refusal turned off the changer is only said, not refused
        a.settings.tuning.atc.enabled = true;
        a.settings.tuning.keepout_reject = false;
        assert_eq!(a.keepout_refusal(), None);
        let _ = std::fs::remove_file(&a.settings_path);
    }
}
