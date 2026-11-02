//! The pictures of the buttons, drawn with lines (no image files, so they take the colour of the theme and stay sharp at any size), and the
//! button of the toolbar that has a picture and its name.

use super::*;

/// The pictures of the toolbar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Glyph {
    Open,
    Save,
    Edit,
    Play,
    Pause,
    Stop,
    Rewind,
    Settings,
}

impl Glyph {
    #[cfg(test)]
    pub(super) const ALL: [Glyph; 8] = [
        Glyph::Open,
        Glyph::Save,
        Glyph::Edit,
        Glyph::Play,
        Glyph::Pause,
        Glyph::Stop,
        Glyph::Rewind,
        Glyph::Settings,
    ];
}

fn v(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}

/// Draws a picture inside the square of side `2 * r` around `c`.
pub(super) fn draw(p: &egui::Painter, g: Glyph, c: Pos2, r: f32, color: Color32) {
    let line = Stroke::new((r / 5.0).max(1.4), color);
    let s = r / 8.0;
    match g {
        Glyph::Open => {
            // a folder: the tab on the left of the back, then the front
            p.line_segment([c + v(-8.0, -5.0) * s, c + v(-8.0, 6.0) * s], line);
            p.line_segment([c + v(-8.0, -5.0) * s, c + v(-3.0, -5.0) * s], line);
            p.line_segment([c + v(-3.0, -5.0) * s, c + v(-1.0, -3.0) * s], line);
            p.line_segment([c + v(-1.0, -3.0) * s, c + v(7.0, -3.0) * s], line);
            p.line_segment([c + v(7.0, -3.0) * s, c + v(7.0, 6.0) * s], line);
            p.line_segment([c + v(-8.0, 6.0) * s, c + v(7.0, 6.0) * s], line);
        }
        Glyph::Save => {
            // a disk: the outline, the label on top and the shutter at the bottom
            p.rect_stroke(
                Rect::from_center_size(c, v(15.0, 15.0) * s),
                1.0,
                line,
                StrokeKind::Inside,
            );
            p.line_segment([c + v(-4.0, -7.5) * s, c + v(-4.0, -2.0) * s], line);
            p.line_segment([c + v(-4.0, -2.0) * s, c + v(4.0, -2.0) * s], line);
            p.line_segment([c + v(4.0, -2.0) * s, c + v(4.0, -7.5) * s], line);
            p.rect_stroke(
                Rect::from_center_size(c + v(0.0, 4.0) * s, v(8.0, 5.0) * s),
                0.0,
                line,
                StrokeKind::Inside,
            );
        }
        Glyph::Edit => {
            // a pencil
            p.line_segment([c + v(-6.0, 6.0) * s, c + v(5.0, -5.0) * s], line);
            p.line_segment([c + v(-6.0, 6.0) * s, c + v(-7.0, 8.0) * s], line);
            p.line_segment([c + v(-7.0, 8.0) * s, c + v(-4.0, 7.0) * s], line);
            p.line_segment([c + v(-3.0, 8.0) * s, c + v(7.0, 8.0) * s], line);
            p.line_segment([c + v(5.0, -5.0) * s, c + v(7.0, -3.0) * s], line);
            p.line_segment([c + v(7.0, -3.0) * s, c + v(-4.0, 8.0) * s], line);
        }
        Glyph::Play => {
            p.add(egui::Shape::convex_polygon(
                vec![
                    c + v(-5.0, -8.0) * s,
                    c + v(-5.0, 8.0) * s,
                    c + v(8.0, 0.0) * s,
                ],
                color,
                Stroke::NONE,
            ));
        }
        Glyph::Pause => {
            for x in [-6.0f32, 1.5] {
                p.rect_filled(
                    Rect::from_min_size(c + v(x, -7.0) * s, v(4.5, 14.0) * s),
                    0.0,
                    color,
                );
            }
        }
        Glyph::Stop => {
            p.rect_filled(Rect::from_center_size(c, v(14.0, 14.0) * s), 1.0, color);
        }
        Glyph::Rewind => {
            p.line_segment([c + v(-7.0, -7.0) * s, c + v(-7.0, 7.0) * s], line);
            p.add(egui::Shape::convex_polygon(
                vec![
                    c + v(8.0, -7.0) * s,
                    c + v(8.0, 7.0) * s,
                    c + v(-4.0, 0.0) * s,
                ],
                color,
                Stroke::NONE,
            ));
        }
        Glyph::Settings => {
            // a gear: the ring and eight teeth
            p.circle_stroke(c, 4.0 * s, line);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::FRAC_PI_4;
                let d = Vec2::angled(a);
                p.line_segment([c + d * 6.5 * s, c + d * 9.0 * s], line);
            }
            p.circle_stroke(c, 6.5 * s, Stroke::new(line.width, color));
        }
    }
}

/// A button of the toolbar: the picture, then the name; the explanation shows when the pointer rests on it. Greyed out when it does not apply.
pub(super) fn tool_button(
    ui: &mut egui::Ui,
    theme: &Theme,
    g: Glyph,
    caption: &str,
    tip: &str,
    enabled: bool,
) -> bool {
    let color = if enabled {
        col(theme.text)
    } else {
        alpha(col(theme.dim), 150)
    };
    let galley =
        ui.painter()
            .layout_no_wrap(caption.to_string(), egui::FontId::proportional(13.0), color);
    let width = 8.0 + 18.0 + 6.0 + galley.size().x + 10.0;
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 28.0), sense);
    let p = ui.painter_at(rect);
    if enabled && resp.hovered() {
        let fill = if resp.is_pointer_button_down_on() {
            110
        } else {
            50
        };
        p.rect_filled(rect, 5.0, alpha(col(theme.accent), fill));
    }
    draw(
        &p,
        g,
        Pos2::new(rect.left() + 8.0 + 9.0, rect.center().y),
        8.0,
        if enabled && g == Glyph::Stop {
            col(theme.danger)
        } else {
            color
        },
    );
    p.galley(
        Pos2::new(
            rect.left() + 8.0 + 18.0 + 6.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        color,
    );
    resp.on_hover_text(tip).clicked() && enabled
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What a picture adds to the screen, in shapes.
    fn shapes_of(g: Glyph) -> usize {
        let ctx = egui::Context::default();
        let mut out = ctx.run_pass(egui::RawInput::default(), |ctx| {
            let p = ctx.layer_painter(egui::LayerId::background());
            draw(&p, g, Pos2::new(50.0, 50.0), 8.0, Color32::WHITE);
        });
        out.textures_delta.clear();
        out.shapes.len()
    }

    #[test]
    fn every_picture_of_the_toolbar_draws_something() {
        for g in Glyph::ALL {
            assert!(shapes_of(g) > 0, "{g:?} draws nothing");
        }
    }
}
