//! The centre of the window: the path of the program seen from above (dragged with the pointer and zoomed with the wheel) or in 3D (turned
//! with the pointer and zoomed with the wheel), with the table, the volume the machine can reach and the tool where the machine is.

use eh3d_view::scene::Table;

use super::*;

impl App {
    /// The zero of the work in the machine: the program speaks in work coordinates and the tool and the table in machine ones.
    pub(super) fn work_offset(&self) -> [f64; 3] {
        let s = &self.session.status;
        [
            s.machine_mm[0] - s.work_mm[0],
            s.machine_mm[1] - s.work_mm[1],
            s.machine_mm[2] - s.work_mm[2],
        ]
    }

    /// Back to the view the window starts in (the plan fitted, or the usual isometric one).
    pub(super) fn reset_view(&mut self) {
        if self.settings.view_3d {
            self.orbit.reset();
        } else {
            self.plan2d.reset();
        }
    }

    /// Changes between the plan and the 3D view.
    pub(super) fn toggle_view(&mut self) {
        self.settings.view_3d = !self.settings.view_3d;
        self.save_settings();
    }

    /// The path of the program on the table, what is done, and where the tool is.
    pub(super) fn center(&mut self, ui: &mut egui::Ui) {
        let t = self.theme.clone();
        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(self.t(Text::McToolpath).to_uppercase()).color(col(t.accent)),
                );
                let before = self.settings.view_3d;
                let mut three = before;
                ui.selectable_value(&mut three, false, self.t(Text::ViewPlan));
                ui.selectable_value(&mut three, true, self.t(Text::ViewMachine));
                if three != before {
                    self.settings.view_3d = three;
                    self.save_settings();
                }
                let moved = if self.settings.view_3d {
                    !self.orbit.is_home()
                } else {
                    !self.plan2d.is_default()
                };
                if ui
                    .add_enabled(moved, egui::Button::new(self.t(Text::ResetView)))
                    .clicked()
                {
                    self.reset_view();
                }
                ui.label(
                    RichText::new(if self.settings.view_3d {
                        self.t(Text::NavHint3d)
                    } else {
                        self.t(Text::NavHint2d)
                    })
                    .color(col(t.dim)),
                );
            });
            let (rect, resp) =
                ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
            let p = ui.painter_at(rect);
            p.rect_filled(rect, 8.0, col(t.panel));
            // the pointer: drag, wheel and a double click to start again
            if resp.dragged() {
                let d = resp.drag_delta();
                if self.settings.view_3d {
                    self.orbit.drag(d.x as f64, d.y as f64);
                } else {
                    self.plan2d.drag(d.x as f64, d.y as f64);
                }
            }
            if resp.hovered() {
                let wheel = ui.input(|i| i.smooth_scroll_delta.y) as f64;
                if wheel != 0.0 {
                    if self.settings.view_3d {
                        self.orbit.wheel(wheel);
                    } else if let Some(c) = resp.hover_pos() {
                        let centre = rect.center();
                        self.plan2d.wheel(
                            wheel,
                            (c.x as f64, c.y as f64),
                            (centre.x as f64, centre.y as f64),
                        );
                    }
                }
            }
            if resp.double_clicked() {
                self.reset_view();
            }
            if self.settings.view_3d {
                self.paint_3d(&p, rect, &t);
            } else {
                self.paint_plan(&p, rect, &t);
            }
        });
    }

    fn paint_plan(&self, p: &egui::Painter, rect: Rect, t: &Theme) {
        let off = self.work_offset();
        let ((tx0, tx1), (ty0, ty1)) = (
            (self.settings.travel[0].0, self.settings.travel[0].1),
            (self.settings.travel[1].0, self.settings.travel[1].1),
        );
        // the view: the table, or the table and the program when it goes beyond it, or only the program
        let (mut minx, mut miny, mut maxx, mut maxy) = (tx0, ty0, tx1, ty1);
        if self.fit_program && !self.path_toolpath.is_empty() {
            (minx, miny, maxx, maxy) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        }
        for s in &self.path_toolpath {
            for pt in [s.from, s.to] {
                minx = minx.min(pt.0 + off[0]);
                miny = miny.min(pt.1 + off[1]);
                maxx = maxx.max(pt.0 + off[0]);
                maxy = maxy.max(pt.1 + off[1]);
            }
        }
        let (w, h) = ((maxx - minx).max(1.0), (maxy - miny).max(1.0));
        let inner = rect.shrink(18.0);
        let k = ((inner.width() as f64) / w).min((inner.height() as f64) / h);
        let centre = rect.center();
        let at = |x: f64, y: f64| {
            let fitted = (
                inner.left() as f64 + (x - minx) * k,
                inner.bottom() as f64 - (y - miny) * k,
            );
            let (sx, sy) = self
                .plan2d
                .apply(fitted, (centre.x as f64, centre.y as f64));
            Pos2::new(sx as f32, sy as f32)
        };
        // the table and its grid every 50 mm
        let table = Rect::from_two_pos(at(tx0, ty0), at(tx1, ty1));
        p.rect_filled(table, 2.0, col(t.table));
        let mut x = (tx0 / 50.0).ceil() * 50.0;
        while x <= tx1 {
            p.line_segment(
                [at(x, ty0), at(x, ty1)],
                Stroke::new(1.0, alpha(col(t.grid), 90)),
            );
            x += 50.0;
        }
        let mut y = (ty0 / 50.0).ceil() * 50.0;
        while y <= ty1 {
            p.line_segment(
                [at(tx0, y), at(tx1, y)],
                Stroke::new(1.0, alpha(col(t.grid), 90)),
            );
            y += 50.0;
        }
        p.rect_stroke(
            table,
            2.0,
            Stroke::new(1.5, col(t.line)),
            StrokeKind::Inside,
        );
        let next = self.session.next;
        for s in &self.path_toolpath {
            if s.rapid && !self.settings.show_rapids {
                continue;
            }
            let done = s.line < next;
            let color = match (s.rapid, done) {
                (true, _) => alpha(col(t.dim), 140),
                (false, true) => col(t.ok),
                (false, false) => col(t.accent),
            };
            p.line_segment(
                [
                    at(s.from.0 + off[0], s.from.1 + off[1]),
                    at(s.to.0 + off[0], s.to.1 + off[1]),
                ],
                Stroke::new(if s.rapid { 1.0 } else { 2.0 }, color),
            );
        }
        // the tool, where the machine is
        let m = self.session.status.machine_mm;
        let c = at(m[0], m[1]);
        p.circle_stroke(c, 9.0, Stroke::new(2.0, col(t.accent2)));
        p.line_segment(
            [c - Vec2::new(14.0, 0.0), c + Vec2::new(14.0, 0.0)],
            Stroke::new(1.0, col(t.accent2)),
        );
        p.line_segment(
            [c - Vec2::new(0.0, 14.0), c + Vec2::new(0.0, 14.0)],
            Stroke::new(1.0, col(t.accent2)),
        );
        p.text(
            rect.right_bottom() - Vec2::new(10.0, 8.0),
            Align2::RIGHT_BOTTOM,
            format!("{:.0} x {:.0} mm", w, h),
            egui::FontId::proportional(12.0),
            col(t.dim),
        );
    }

    /// The machine in 3D: the floor of the table with its grid, the edges of the volume the axes reach, the path of the program with the
    /// part that is done, and the tool with its line down to the table.
    fn paint_3d(&self, p: &egui::Painter, rect: Rect, t: &Theme) {
        let off = self.work_offset();
        let tr = self.settings.travel;
        let (x0, y0, z0) = (tr[0].0, tr[1].0, tr[2].0);
        let table = Table::new(tr[0].1 - tr[0].0, tr[1].1 - tr[1].0);
        let height = tr[2].1 - tr[2].0;
        let iso = self
            .orbit
            .iso(&table, height, rect.width() as f64, rect.height() as f64);
        // a point of the machine (mm) to the screen
        let at = |x: f64, y: f64, z: f64| {
            let (px, py) = iso.project(x - x0, y - y0, z - z0);
            Pos2::new(rect.left() + px as f32, rect.top() + py as f32)
        };
        let (w, d) = (table.width_mm, table.depth_mm);
        let floor = [
            at(x0, y0, z0),
            at(x0 + w, y0, z0),
            at(x0 + w, y0 + d, z0),
            at(x0, y0 + d, z0),
        ];
        p.add(egui::Shape::convex_polygon(
            floor.to_vec(),
            col(t.table),
            Stroke::new(1.5, col(t.line)),
        ));
        let grid = Stroke::new(1.0, alpha(col(t.grid), 90));
        let mut x = (x0 / 50.0).ceil() * 50.0;
        while x <= x0 + w {
            p.line_segment([at(x, y0, z0), at(x, y0 + d, z0)], grid);
            x += 50.0;
        }
        let mut y = (y0 / 50.0).ceil() * 50.0;
        while y <= y0 + d {
            p.line_segment([at(x0, y, z0), at(x0 + w, y, z0)], grid);
            y += 50.0;
        }
        // the volume the axes reach
        let edge = Stroke::new(1.0, alpha(col(t.dim), 110));
        let top = [
            at(x0, y0, z0 + height),
            at(x0 + w, y0, z0 + height),
            at(x0 + w, y0 + d, z0 + height),
            at(x0, y0 + d, z0 + height),
        ];
        for i in 0..4 {
            p.line_segment([floor[i], top[i]], edge);
            p.line_segment([top[i], top[(i + 1) % 4]], edge);
        }
        let next = self.session.next;
        for s in &self.path_toolpath {
            if s.rapid && !self.settings.show_rapids {
                continue;
            }
            let color = match (s.rapid, s.line < next) {
                (true, _) => alpha(col(t.dim), 140),
                (false, true) => col(t.ok),
                (false, false) => col(t.accent),
            };
            p.line_segment(
                [
                    at(s.from.0 + off[0], s.from.1 + off[1], s.from.2 + off[2]),
                    at(s.to.0 + off[0], s.to.1 + off[1], s.to.2 + off[2]),
                ],
                Stroke::new(if s.rapid { 1.0 } else { 2.0 }, color),
            );
        }
        // the tool: a ring on the table under it, the line down from it, and the tool itself
        let m = self.session.status.machine_mm;
        let (foot, head) = (at(m[0], m[1], z0), at(m[0], m[1], m[2]));
        p.line_segment([foot, head], Stroke::new(1.0, alpha(col(t.accent2), 140)));
        p.circle_stroke(foot, 5.0, Stroke::new(1.0, alpha(col(t.accent2), 140)));
        p.circle_filled(head, 5.0, col(t.accent2));
        p.text(
            rect.right_bottom() - Vec2::new(10.0, 8.0),
            Align2::RIGHT_BOTTOM,
            format!("{:.0} x {:.0} x {:.0} mm", w, d, height),
            egui::FontId::proportional(12.0),
            col(t.dim),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_work_zero_is_where_the_machine_and_the_work_differ() {
        let mut a = App::new(&Options::default());
        let mut machine = [0.0; eh3d_controller::AXES];
        let mut work = [0.0; eh3d_controller::AXES];
        (machine[0], machine[1], machine[2]) = (110.0, 60.0, 5.0);
        (work[0], work[1], work[2]) = (10.0, 10.0, 5.0);
        a.session.status.machine_mm = machine;
        a.session.status.work_mm = work;
        assert_eq!(a.work_offset(), [100.0, 50.0, 0.0]);
    }

    #[test]
    fn the_view_changes_between_the_plan_and_3d_and_goes_back_to_where_it_started() {
        let mut a = App::new(&Options::default());
        assert!(!a.settings.view_3d);
        a.plan2d.drag(40.0, 10.0);
        a.orbit.drag(40.0, 10.0);
        a.reset_view();
        assert!(a.plan2d.is_default() && !a.orbit.is_home());
        a.toggle_view();
        assert!(a.settings.view_3d);
        a.reset_view();
        assert!(a.orbit.is_home());
    }
}
