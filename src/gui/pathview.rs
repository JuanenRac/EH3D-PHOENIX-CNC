//! The viewer of the path in the centre of the window: the row under the title (how the path is coloured, which tools are shown, the zoom to the
//! selection and the line of the move that was clicked), the colour and width of each move, the highlight of the selected one, and the jump to
//! its line. The numbers are `viewer`; the plan and the 3D view draw with them (`views`). Nothing here moves a machine.

use super::*;
use crate::viewer::{kind_of, length_of, ColorBy, Kind};
use crate::words::{say, Msg};

impl App {
    /// The row under the title of the path.
    pub(super) fn viewer_bar(&mut self, ui: &mut egui::Ui) {
        let t = self.theme.clone();
        let mut mode = self.viewer.color_by;
        let mut toggle: Option<Option<u32>> = None;
        let mut zoom = false;
        let tools = self.viewer.tools.clone();
        let hidden = self.viewer.hidden.clone();
        let has_selection = self.viewer.selected.is_some();
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(self.w(Msg::VwColor)).color(col(t.dim)));
            for (m, name) in [
                (ColorBy::State, Msg::VwByState),
                (ColorBy::Kind, Msg::VwByKind),
                (ColorBy::Progress, Msg::VwByProgress),
                (ColorBy::Tool, Msg::VwByTool),
                (ColorBy::Feed, Msg::VwByFeed),
            ] {
                ui.selectable_value(&mut mode, m, self.w(name));
            }
            if tools.len() > 1 {
                ui.separator();
                ui.label(RichText::new(self.w(Msg::VwTools)).color(col(t.dim)));
                for tool in &tools {
                    let mut shown = !hidden.contains(tool);
                    let name = match tool {
                        Some(n) => format!("T{n}"),
                        None => "-".to_string(),
                    };
                    if ui.checkbox(&mut shown, name).changed() {
                        toggle = Some(*tool);
                    }
                }
            }
            ui.separator();
            if ui
                .add_enabled(has_selection, egui::Button::new(self.w(Msg::VwZoomSel)))
                .clicked()
            {
                zoom = true;
            }
        });
        self.viewer.color_by = mode;
        if let Some(tool) = toggle {
            self.viewer.toggle(tool);
        }
        if zoom {
            self.viewer.zoom_request = true;
        }
        let text = match self
            .viewer
            .selected
            .and_then(|i| self.path_toolpath.get(i).map(|s| (i, *s)))
        {
            Some((i, s)) => self.selection_text(i, &s),
            None => self.w(Msg::VwSelHint).to_string(),
        };
        ui.label(RichText::new(text).color(col(t.dim)));
    }

    /// What the selected move says of itself: its line, the code of the line, and its kind, length, feed and tool.
    pub(super) fn selection_text(&self, i: usize, s: &Segment) -> String {
        let code = self
            .session
            .program
            .get(s.line)
            .map(|l| l.trim().to_string())
            .unwrap_or_default();
        let kind = match kind_of(s) {
            Kind::Rapid => self.w(Msg::RepLegendRapid),
            Kind::Cut => self.w(Msg::RepLegendCut),
            Kind::Plunge => self.w(Msg::VwPlunge),
            Kind::Arc => self.w(Msg::VwArc),
        };
        let tool = match self.viewer.tool_of(i) {
            Some(n) => say(self.lang.code(), Msg::StatsTool, &[n.to_string()]),
            None => say(self.lang.code(), Msg::StatsNoTool, &[] as &[&str]),
        };
        let feed = if !s.rapid && s.feed > 0.0 {
            format!(", {:.0} mm/min", s.feed)
        } else {
            String::new()
        };
        let details = format!("{kind}, {:.1} mm{feed}, {tool}", length_of(s));
        self.ws(Msg::VwSel, &[(s.line + 1).to_string(), code, details])
    }

    /// The colour and the width of a move, for the way the path is coloured. Rapids are always dim and thin.
    pub(super) fn seg_style(
        &self,
        i: usize,
        s: &Segment,
        t: &Theme,
        next: usize,
    ) -> (Color32, f32) {
        if s.rapid {
            return (alpha(col(t.dim), 140), 1.0);
        }
        match self.viewer.color_by {
            ColorBy::State => (
                if s.line < next {
                    col(t.ok)
                } else {
                    col(t.accent)
                },
                2.0,
            ),
            ColorBy::Kind => match kind_of(s) {
                Kind::Plunge => (col(t.warn), 2.5),
                Kind::Arc => (col(t.accent2), 2.0),
                _ => (col(t.accent), 2.0),
            },
            _ => match self.viewer.color_of(i, s) {
                Some((r, g, b)) => (Color32::from_rgb(r, g, b), 2.0),
                None => (col(t.accent), 2.0),
            },
        }
    }

    /// The selected move drawn over the others: thick, with a dot at each end.
    pub(super) fn paint_selected(&self, p: &egui::Painter, t: &Theme, a: Pos2, b: Pos2) {
        p.line_segment([a, b], Stroke::new(5.0, alpha(col(t.warn), 200)));
        p.circle_filled(a, 4.0, col(t.warn));
        p.circle_filled(b, 4.0, col(t.warn));
    }

    /// A click on a move selects it and shows its line in the program panel; a click on nothing takes the selection away.
    pub(super) fn select_segment(&mut self, i: Option<usize>) {
        self.viewer.selected = i;
        if let Some(s) = i.and_then(|i| self.path_toolpath.get(i)) {
            let line = s.line;
            self.show_line(line);
        }
    }

    /// Zooms the plan to the selected move when the operator asked for it.
    pub(super) fn zoom_to_selection(&mut self, rect: Rect) {
        if !self.viewer.zoom_request {
            return;
        }
        self.viewer.zoom_request = false;
        let Some(s) = self.viewer.selected.and_then(|i| self.path_toolpath.get(i)) else {
            return;
        };
        let off = self.work_offset();
        if self.settings.view_3d {
            // the move in the table of the 3D view (from the corner and the floor of the travel)
            let tr = self.settings.travel;
            let table = eh3d_view::scene::Table::new(tr[0].1 - tr[0].0, tr[1].1 - tr[1].0);
            let rel = |p: crate::gcode::Point| {
                (
                    p.0 + off[0] - tr[0].0,
                    p.1 + off[1] - tr[1].0,
                    p.2 + off[2] - tr[2].0,
                )
            };
            self.orbit.zoom_to_line(
                &table,
                tr[2].1 - tr[2].0,
                rel(s.from),
                rel(s.to),
                rect.width() as f64,
                rect.height() as f64,
            );
            return;
        }
        let b = crate::viewer::bounds_of(s);
        let frame = self.plan_frame(rect);
        let a = frame.fitted(b[0] + off[0], b[1] + off[1]);
        let c = frame.fitted(b[2] + off[0], b[3] + off[1]);
        let centre = rect.center();
        self.plan2d.zoom_to(
            (a.0.min(c.0), a.1.min(c.1), a.0.max(c.0), a.1.max(c.1)),
            (centre.x as f64, centre.y as f64),
            (rect.width() as f64, rect.height() as f64),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_with_two_tools() -> App {
        let mut a = App::isolated();
        a.session
            .load("G21 G90\nT1 M6\nG0 X10\nG1 Z-2 F100\nG1 X30 F600\nT2 M6\nG1 Y20 F300");
        a.refresh_path();
        a
    }

    #[test]
    fn a_click_on_a_move_selects_it_and_shows_its_line() {
        let mut a = app_with_two_tools();
        assert_eq!(a.viewer.tools, [Some(1), Some(2)]);
        a.select_segment(Some(2));
        assert_eq!(a.viewer.selected, Some(2));
        assert_eq!((a.focus_line, a.focus_scroll), (Some(4), true));
        // a click on nothing takes the selection away and leaves the panel where it is
        a.focus_line = None;
        a.select_segment(None);
        assert_eq!(a.viewer.selected, None);
        assert_eq!(a.focus_line, None);
        // a move that is not there selects nothing to show
        a.select_segment(Some(99));
        assert_eq!(a.focus_line, None);
    }

    #[test]
    fn the_selection_says_its_line_kind_length_feed_and_tool() {
        let mut a = app_with_two_tools();
        let s = a.path_toolpath[2];
        let text = a.selection_text(2, &s);
        assert_eq!(
            text,
            "Line 5: G1 X30 F600  [Cutting, 20.0 mm, 600 mm/min, Tool 1]"
        );
        let rapid = a.path_toolpath[0];
        assert!(a
            .selection_text(0, &rapid)
            .ends_with("[Rapid, 10.0 mm, Tool 1]"));
        let plunge = a.path_toolpath[1];
        assert!(a
            .selection_text(1, &plunge)
            .contains("Plunge, 2.0 mm, 100 mm/min"));
        // in another language
        a.settings.lang = Lang::Es;
        a.apply_settings();
        assert!(a.selection_text(2, &s).starts_with("Línea 5: G1 X30 F600"));
        assert!(a.selection_text(2, &s).contains("Herramienta 1"));
    }

    #[test]
    fn a_new_program_takes_the_selection_away() {
        let mut a = app_with_two_tools();
        a.select_segment(Some(1));
        a.viewer.toggle(Some(2));
        a.session.load("T1 M6\nG1 X5 F100");
        a.refresh_path();
        assert_eq!(a.viewer.selected, None);
        assert!(a.viewer.hidden.is_empty());
    }

    #[test]
    fn the_colours_follow_the_way_of_colouring_and_rapids_stay_dim() {
        let mut a = app_with_two_tools();
        let t = a.theme.clone();
        let (rapid, cut) = (a.path_toolpath[0], a.path_toolpath[2]);
        assert_eq!(a.seg_style(0, &rapid, &t, 0).1, 1.0);
        // by state: to do now, done once the program is past the line
        assert_eq!(a.seg_style(2, &cut, &t, 0), (col(t.accent), 2.0));
        assert_eq!(a.seg_style(2, &cut, &t, 9), (col(t.ok), 2.0));
        a.viewer.color_by = ColorBy::Kind;
        let plunge = a.path_toolpath[1];
        assert_eq!(a.seg_style(1, &plunge, &t, 0), (col(t.warn), 2.5));
        a.viewer.color_by = ColorBy::Tool;
        let (c1, c2) = (
            a.seg_style(2, &cut, &t, 0).0,
            a.seg_style(3, &a.path_toolpath[3], &t, 0).0,
        );
        assert_ne!(c1, c2);
        a.viewer.color_by = ColorBy::Feed;
        assert_eq!(
            a.seg_style(2, &cut, &t, 0).0,
            Color32::from_rgb(239, 68, 68)
        );
        // the rapid is the same whatever the way
        assert_eq!(a.seg_style(0, &rapid, &t, 0).0, alpha(col(t.dim), 140));
    }

    #[test]
    fn the_zoom_to_the_selection_changes_the_view_only_when_asked() {
        let mut a = app_with_two_tools();
        let rect = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(800.0, 600.0));
        a.select_segment(Some(2));
        a.zoom_to_selection(rect);
        assert!(a.plan2d.is_default(), "nothing was asked");
        a.viewer.zoom_request = true;
        a.zoom_to_selection(rect);
        assert!(a.plan2d.zoom > 1.0);
        assert!(!a.viewer.zoom_request);
        // in 3D the view that is turned is the one that is magnified, with the move in the middle
        a.plan2d.reset();
        a.settings.view_3d = true;
        a.select_segment(Some(2));
        a.zoom_to_selection(rect);
        assert!(a.orbit.is_home(), "nothing was asked");
        a.viewer.zoom_request = true;
        a.zoom_to_selection(rect);
        assert!(a.plan2d.is_default() && !a.viewer.zoom_request);
        assert!(a.orbit.zoom > 1.0 && a.orbit.focus.is_some());
        a.reset_view();
        assert!(a.orbit.is_home());
    }

    #[test]
    fn the_viewer_bar_and_both_views_draw_with_a_selection_and_every_way_of_colouring() {
        let ctx = egui::Context::default();
        let rect = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(800.0, 600.0));
        for mode in ColorBy::ALL {
            for three in [false, true] {
                let mut a = app_with_two_tools();
                a.viewer.color_by = mode;
                a.viewer.selected = Some(2);
                a.settings.view_3d = three;
                let t = a.theme.clone();
                ctx.run_pass(egui::RawInput::default(), |ctx| {
                    egui::Window::new("v").show(ctx, |ui| a.viewer_bar(ui));
                    let p = ctx.layer_painter(egui::LayerId::background());
                    let click = Some(Pos2::new(5.0, 5.0));
                    let picked = if three {
                        a.paint_3d(&p, rect, &t, click)
                    } else {
                        a.paint_plan(&p, rect, &t, click)
                    };
                    assert!(picked.is_none_or(|i| i < a.path_toolpath.len()));
                })
                .textures_delta
                .clear();
                assert_eq!(a.viewer.color_by, mode);
            }
        }
    }

    #[test]
    fn a_click_on_a_move_of_the_plan_picks_that_move_and_a_hidden_tool_cannot_be_picked() {
        let ctx = egui::Context::default();
        let rect = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(800.0, 600.0));
        let mut a = app_with_two_tools();
        let t = a.theme.clone();
        // the middle of the cut X10 to X30, on the screen
        let off = a.work_offset();
        let fitted = a.plan_frame(rect).fitted(20.0 + off[0], 0.0 + off[1]);
        let centre = rect.center();
        let (sx, sy) = a
            .plan2d
            .apply(fitted, (f64::from(centre.x), f64::from(centre.y)));
        let click = Some(Pos2::new(sx as f32, sy as f32 + 2.0));
        let mut picked = None;
        ctx.run_pass(egui::RawInput::default(), |ctx| {
            let p = ctx.layer_painter(egui::LayerId::background());
            picked = a.paint_plan(&p, rect, &t, click);
        })
        .textures_delta
        .clear();
        assert_eq!(picked, Some(2));
        // far from every move, nothing; no click, nothing
        ctx.run_pass(egui::RawInput::default(), |ctx| {
            let p = ctx.layer_painter(egui::LayerId::background());
            picked = a.paint_plan(&p, rect, &t, Some(Pos2::new(700.0, 20.0)));
        })
        .textures_delta
        .clear();
        assert_eq!(picked, None);
        ctx.run_pass(egui::RawInput::default(), |ctx| {
            let p = ctx.layer_painter(egui::LayerId::background());
            picked = a.paint_plan(&p, rect, &t, None);
        })
        .textures_delta
        .clear();
        assert_eq!(picked, None);
        // the tool 1 layer is hidden: its move is not picked
        a.viewer.toggle(Some(1));
        ctx.run_pass(egui::RawInput::default(), |ctx| {
            let p = ctx.layer_painter(egui::LayerId::background());
            picked = a.paint_plan(&p, rect, &t, click);
        })
        .textures_delta
        .clear();
        assert_eq!(picked, None);
    }
}
