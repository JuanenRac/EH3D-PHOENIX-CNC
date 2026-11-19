//! How the path of a program is looked at: the plan from above that zooms about the pointer and is dragged, and the 3D view that is turned
//! and zoomed. Only numbers and no window, so the movements of the pointer are tested on their own; the window draws with what is worked
//! out here. The 3D projection itself is the isometric one of `eh3d-view`, shared with the other programs of the suite.

use eh3d_view::scene::{Iso, Table};

/// The least and the most the plan can be magnified.
pub const ZOOM_MIN: f64 = 0.2;
pub const ZOOM_MAX: f64 = 60.0;

/// Zoom and shift of the plan, over the view that fits the table (or the program).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plan2d {
    pub zoom: f64,
    /// How far the view is dragged, in pixels.
    pub pan: (f64, f64),
}

impl Default for Plan2d {
    fn default() -> Self {
        Plan2d {
            zoom: 1.0,
            pan: (0.0, 0.0),
        }
    }
}

impl Plan2d {
    /// A point of the fitted view (pixels) to the point of the screen, about the `centre` of the drawing area.
    pub fn apply(&self, fitted: (f64, f64), centre: (f64, f64)) -> (f64, f64) {
        (
            centre.0 + (fitted.0 - centre.0) * self.zoom + self.pan.0,
            centre.1 + (fitted.1 - centre.1) * self.zoom + self.pan.1,
        )
    }

    /// The wheel: `lines` > 0 zooms in. The point of the plan under the pointer (`cursor`, pixels, `centre` of the area) stays where it is.
    pub fn wheel(&mut self, lines: f64, cursor: (f64, f64), centre: (f64, f64)) {
        let zoom = (self.zoom * 1.0015f64.powf(lines)).clamp(ZOOM_MIN, ZOOM_MAX);
        let ratio = zoom / self.zoom;
        let (cx, cy) = (cursor.0 - centre.0, cursor.1 - centre.1);
        self.pan = (
            cx - (cx - self.pan.0) * ratio,
            cy - (cy - self.pan.1) * ratio,
        );
        self.zoom = zoom;
    }

    /// Dragging moves the view with the pointer.
    pub fn drag(&mut self, dx: f64, dy: f64) {
        self.pan = (self.pan.0 + dx, self.pan.1 + dy);
    }

    /// Zooms and shifts the view so that `target` (a rectangle of the fitted view, in pixels: left, top, right, bottom) is in the middle of
    /// an area of `size` pixels about `centre`, filling most of it (a tiny one is magnified no more than the limit of the zoom).
    pub fn zoom_to(&mut self, target: (f64, f64, f64, f64), centre: (f64, f64), size: (f64, f64)) {
        let w = (target.2 - target.0).abs().max(1.0);
        let h = (target.3 - target.1).abs().max(1.0);
        let zoom = (size.0 * 0.7 / w)
            .min(size.1 * 0.7 / h)
            .clamp(ZOOM_MIN, ZOOM_MAX);
        let middle = ((target.0 + target.2) / 2.0, (target.1 + target.3) / 2.0);
        self.zoom = zoom;
        self.pan = (-(middle.0 - centre.0) * zoom, -(middle.1 - centre.1) * zoom);
    }

    pub fn reset(&mut self) {
        *self = Plan2d::default();
    }

    pub fn is_default(&self) -> bool {
        *self == Plan2d::default()
    }
}

/// Turn and zoom of the 3D view.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Orbit {
    pub yaw_deg: f64,
    pub pitch_deg: f64,
    pub zoom: f64,
    /// A point of the table in mm (x and y from the corner of the travel, z from its floor) that is kept in the middle of the area, in
    /// place of the middle of the table: where the view was zoomed to. `None` keeps the middle of the table.
    pub focus: Option<(f64, f64, f64)>,
}

/// The view the 3D starts in: the usual isometric one.
pub const ORBIT_HOME: Orbit = Orbit {
    yaw_deg: 0.0,
    pitch_deg: 30.0,
    zoom: 1.0,
    focus: None,
};

/// The views the 3D view can be put in with one click or key: from above (the plan), from the front, from the side, and the usual isometric.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    Top,
    Front,
    Side,
    Iso,
}

impl Preset {
    pub const ALL: [Preset; 4] = [Preset::Top, Preset::Front, Preset::Side, Preset::Iso];

    /// The word of the command line (`--view`).
    pub fn key(self) -> &'static str {
        match self {
            Preset::Top => "top",
            Preset::Front => "front",
            Preset::Side => "side",
            Preset::Iso => "iso",
        }
    }

    pub fn from_key(key: &str) -> Option<Preset> {
        Preset::ALL
            .into_iter()
            .find(|p| p.key().eq_ignore_ascii_case(key.trim()))
    }
}

/// Degrees of turn per pixel dragged.
const TURN_DEG_PER_PX: f64 = 0.4;

impl Default for Orbit {
    fn default() -> Self {
        ORBIT_HOME
    }
}

impl Orbit {
    /// Dragging turns the view around the vertical axis (sideways) and tilts it (up and down); it never goes below the table or over the top.
    pub fn drag(&mut self, dx: f64, dy: f64) {
        self.yaw_deg = (self.yaw_deg + dx * TURN_DEG_PER_PX).rem_euclid(360.0);
        self.pitch_deg = (self.pitch_deg + dy * TURN_DEG_PER_PX).clamp(5.0, 89.0);
    }

    pub fn wheel(&mut self, lines: f64) {
        self.zoom = (self.zoom * 1.0015f64.powf(lines)).clamp(ZOOM_MIN, ZOOM_MAX);
    }

    pub fn reset(&mut self) {
        *self = ORBIT_HOME;
    }

    /// Puts the middle of the line from `a` to `b` (points of the table in mm) in the middle of the area and magnifies the view, at the
    /// turn and tilt it has, until the line takes most of the width or the height (as much as the limit of the zoom allows).
    pub fn zoom_to_line(
        &mut self,
        table: &Table,
        height_mm: f64,
        a: (f64, f64, f64),
        b: (f64, f64, f64),
        width: f64,
        height: f64,
    ) {
        self.focus = Some(((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0, (a.2 + b.2) / 2.0));
        self.zoom = 1.0;
        let iso = self.iso(table, height_mm, width, height);
        let (pa, pb) = (iso.project(a.0, a.1, a.2), iso.project(b.0, b.1, b.2));
        let (dx, dy) = ((pa.0 - pb.0).abs(), (pa.1 - pb.1).abs());
        let fit = |size: f64, d: f64| if d > 1e-9 { size * 0.7 / d } else { ZOOM_MAX };
        self.zoom = fit(width, dx)
            .min(fit(height, dy))
            .clamp(ZOOM_MIN, ZOOM_MAX);
    }

    pub fn is_home(&self) -> bool {
        *self == ORBIT_HOME
    }

    /// Puts the view in a preset one, at zoom 1 and with the middle of the table in the middle. The view from above is the plan of the window
    /// (the 3D projection is not a true view from above), so `Top` changes nothing here. The front looks along Y with X to the right and the side looks along X
    /// with Y to the right, both level with the table; the isometric one is the view the window starts in.
    pub fn preset(&mut self, preset: Preset) {
        let (yaw_deg, pitch_deg) = match preset {
            Preset::Top => return,
            Preset::Front => (315.0, 0.0),
            Preset::Side => (225.0, 0.0),
            Preset::Iso => (ORBIT_HOME.yaw_deg, ORBIT_HOME.pitch_deg),
        };
        *self = Orbit {
            yaw_deg,
            pitch_deg,
            zoom: 1.0,
            focus: None,
        };
    }

    /// The projection of the table and the machine volume (`height_mm` over the table) in an area of `width` x `height` pixels: the whole
    /// volume fitted at zoom 1, turned and tilted as the operator left it, with the middle of the table at the middle of the area.
    pub fn iso(&self, table: &Table, height_mm: f64, width: f64, height: f64) -> Iso {
        let mut iso = Iso::fit(table, height_mm.max(1.0), width, height);
        iso.yaw_deg = self.yaw_deg;
        iso.pitch_deg = self.pitch_deg;
        iso.pivot = (table.width_mm / 2.0, table.depth_mm / 2.0);
        // the volume fits at any turn and tilt: the diagonal of the table is its biggest reach sideways, and the tilt can show the height up to 1.16 times
        let diagonal = table.width_mm.hypot(table.depth_mm);
        let h = height_mm.max(1.0);
        let fit_k = (width / (diagonal * 1.3)).min(height / (diagonal * 1.45 + h * 1.2));
        iso.k = fit_k.min(iso.k).max(f64::MIN_POSITIVE) * self.zoom;
        // the middle of the table (half way up the volume), or the point the view was zoomed to, to the middle of the area
        let aim = self
            .focus
            .unwrap_or((iso.pivot.0, iso.pivot.1, height_mm.max(1.0) / 2.0));
        let (mx, my) = iso.project(aim.0, aim.1, aim.2);
        iso.origin = (
            iso.origin.0 + width / 2.0 - mx,
            iso.origin.1 + height / 2.0 - my,
        );
        iso
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zooming_to_a_rectangle_puts_it_in_the_middle_and_fills_most_of_the_area() {
        let mut v = Plan2d::default();
        let (centre, size) = ((400.0, 300.0), (800.0, 600.0));
        v.zoom_to((100.0, 100.0, 200.0, 150.0), centre, size);
        // the middle of the rectangle is the middle of the area
        let m = v.apply((150.0, 125.0), centre);
        assert!(
            (m.0 - 400.0).abs() < 1e-9 && (m.1 - 300.0).abs() < 1e-9,
            "{m:?}"
        );
        // 100 px wide in 800 px: the width limits it to 0.7 * 800 / 100
        assert!((v.zoom - 5.6).abs() < 1e-9, "{}", v.zoom);
        let (a, b) = (
            v.apply((100.0, 100.0), centre),
            v.apply((200.0, 150.0), centre),
        );
        assert!(a.0 > 0.0 && b.0 < 800.0 && a.1 > 0.0 && b.1 < 600.0);
        // a point or a thin line is not magnified past the limit, and a huge one not below it
        v.zoom_to((10.0, 10.0, 10.0, 10.0), centre, size);
        assert_eq!(v.zoom, ZOOM_MAX);
        v.zoom_to((0.0, 0.0, 1e9, 1e9), centre, size);
        assert_eq!(v.zoom, ZOOM_MIN);
    }

    #[test]
    fn the_wheel_zooms_about_the_pointer_and_keeps_the_point_under_it() {
        let mut v = Plan2d::default();
        let centre = (400.0, 300.0);
        let cursor = (550.0, 220.0);
        // the point of the fitted view under the cursor
        let before = v.apply(cursor, centre);
        v.wheel(300.0, cursor, centre);
        assert!(v.zoom > 1.0);
        let after = v.apply(cursor, centre);
        assert!((before.0 - after.0).abs() < 1e-9 && (before.1 - after.1).abs() < 1e-9);
        // the point under the cursor is the same one: zooming about the centre would have moved it
        let mut c = Plan2d::default();
        c.wheel(300.0, centre, centre);
        assert!((c.apply(cursor, centre).0 - cursor.0).abs() > 1.0);
    }

    #[test]
    fn the_zoom_stays_between_its_limits_and_the_view_comes_back() {
        let mut v = Plan2d::default();
        for _ in 0..50 {
            v.wheel(1000.0, (0.0, 0.0), (10.0, 10.0));
        }
        assert_eq!(v.zoom, ZOOM_MAX);
        for _ in 0..100 {
            v.wheel(-1000.0, (0.0, 0.0), (10.0, 10.0));
        }
        assert_eq!(v.zoom, ZOOM_MIN);
        v.drag(30.0, -12.0);
        assert!(!v.is_default());
        v.reset();
        assert!(v.is_default());
    }

    #[test]
    fn dragging_moves_the_drawing_with_the_pointer() {
        let mut v = Plan2d::default();
        v.drag(10.0, -5.0);
        v.drag(5.0, 5.0);
        assert_eq!(v.apply((100.0, 100.0), (50.0, 50.0)), (115.0, 100.0));
    }

    #[test]
    fn the_presets_look_along_an_axis_and_the_isometric_one_is_home() {
        let table = Table::new(300.0, 200.0);
        let screen = |o: &Orbit, p: (f64, f64, f64)| {
            let iso = o.iso(&table, 100.0, 800.0, 600.0);
            iso.project(p.0, p.1, p.2)
        };
        // front: X goes across the screen, Y does not move anything, Z goes up
        let mut front = Orbit::default();
        front.preset(Preset::Front);
        let (a, b) = (
            screen(&front, (50.0, 0.0, 0.0)),
            screen(&front, (50.0, 90.0, 0.0)),
        );
        assert!(
            (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6,
            "{a:?} {b:?}"
        );
        let (c, d) = (
            screen(&front, (0.0, 0.0, 0.0)),
            screen(&front, (80.0, 0.0, 0.0)),
        );
        assert!(d.0 > c.0 + 50.0 && (d.1 - c.1).abs() < 1e-6, "{c:?} {d:?}");
        let up = screen(&front, (0.0, 0.0, 40.0));
        assert!(
            up.1 < c.1 - 30.0 && (up.0 - c.0).abs() < 1e-6,
            "{c:?} {up:?}"
        );
        // side: Y goes across the screen to the right, X does not move anything
        let mut side = Orbit::default();
        side.preset(Preset::Side);
        let (a, b) = (
            screen(&side, (0.0, 30.0, 0.0)),
            screen(&side, (200.0, 30.0, 0.0)),
        );
        assert!(
            (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6,
            "{a:?} {b:?}"
        );
        let (c, d) = (
            screen(&side, (0.0, 0.0, 0.0)),
            screen(&side, (0.0, 80.0, 0.0)),
        );
        assert!(d.0 > c.0 + 50.0 && (d.1 - c.1).abs() < 1e-6, "{c:?} {d:?}");
        // the middle of the table stays in the middle of the area
        let m = screen(&front, (150.0, 100.0, 50.0));
        assert!(
            (m.0 - 400.0).abs() < 1e-6 && (m.1 - 300.0).abs() < 1e-6,
            "{m:?}"
        );
        // a preset starts again from zoom 1 and no focus; the isometric one is home; the one from above is the plan, so it leaves the orbit alone
        let mut o = Orbit::default();
        o.wheel(500.0);
        o.focus = Some((1.0, 2.0, 3.0));
        o.preset(Preset::Top);
        assert!(o.zoom > 1.0 && o.focus.is_some());
        o.preset(Preset::Side);
        assert_eq!((o.zoom, o.focus), (1.0, None));
        o.preset(Preset::Iso);
        assert!(o.is_home());
        // dragging from a level view tilts it within the limits
        front.drag(0.0, -1.0e6);
        assert_eq!(front.pitch_deg, 5.0);
    }

    #[test]
    fn the_presets_have_words_for_the_command_line() {
        for p in Preset::ALL {
            assert_eq!(Preset::from_key(p.key()), Some(p));
        }
        assert_eq!(Preset::from_key(" Front "), Some(Preset::Front));
        assert_eq!(Preset::from_key("back"), None);
        assert_eq!(Preset::from_key(""), None);
    }

    #[test]
    fn the_orbit_turns_all_the_way_round_and_tilts_within_its_limits() {
        let mut o = Orbit::default();
        assert!(o.is_home());
        o.drag(1000.0, 0.0);
        assert!((0.0..360.0).contains(&o.yaw_deg));
        o.drag(-5000.0, 0.0);
        assert!((0.0..360.0).contains(&o.yaw_deg));
        o.drag(0.0, 1.0e6);
        assert_eq!(o.pitch_deg, 89.0);
        o.drag(0.0, -1.0e6);
        assert_eq!(o.pitch_deg, 5.0);
        o.wheel(1.0e6);
        assert_eq!(o.zoom, ZOOM_MAX);
        o.reset();
        assert!(o.is_home());
    }

    #[test]
    fn the_3d_view_keeps_the_middle_of_the_table_in_the_middle_at_any_turn_and_zoom() {
        let table = Table::new(500.0, 300.0);
        for (yaw, pitch, zoom) in [(0.0, 30.0, 1.0), (75.0, 55.0, 1.0), (200.0, 10.0, 3.0)] {
            let o = Orbit {
                yaw_deg: yaw,
                pitch_deg: pitch,
                zoom,
                focus: None,
            };
            let iso = o.iso(&table, 150.0, 800.0, 600.0);
            let (x, y) = iso.project(250.0, 150.0, 75.0);
            assert!(
                (x - 400.0).abs() < 1e-6 && (y - 300.0).abs() < 1e-6,
                "{yaw} {pitch}"
            );
        }
        // zooming in enlarges, and the corner of the table moves away from the middle
        let near = Orbit::default().iso(&table, 150.0, 800.0, 600.0);
        let far = Orbit {
            zoom: 2.0,
            ..Orbit::default()
        }
        .iso(&table, 150.0, 800.0, 600.0);
        assert!((far.k - near.k * 2.0).abs() < 1e-9);
    }

    #[test]
    fn zooming_the_3d_view_to_a_line_puts_its_middle_in_the_middle_and_fills_most_of_the_area() {
        let table = Table::new(500.0, 300.0);
        let (a, b) = ((100.0, 50.0, 10.0), (140.0, 50.0, 10.0));
        for (yaw, pitch) in [(0.0, 30.0), (75.0, 55.0), (200.0, 10.0)] {
            let mut o = Orbit {
                yaw_deg: yaw,
                pitch_deg: pitch,
                ..Orbit::default()
            };
            o.zoom_to_line(&table, 150.0, a, b, 800.0, 600.0);
            assert!(o.zoom > 1.0 && !o.is_home(), "{yaw} {pitch}");
            let iso = o.iso(&table, 150.0, 800.0, 600.0);
            let (px, py) = iso.project(120.0, 50.0, 10.0);
            assert!(
                (px - 400.0).abs() < 1e-6 && (py - 300.0).abs() < 1e-6,
                "{yaw} {pitch}: {px} {py}"
            );
            // both ends are inside the area, and the line takes 70% of one of its sides
            let (ea, eb) = (iso.project(a.0, a.1, a.2), iso.project(b.0, b.1, b.2));
            for e in [ea, eb] {
                assert!((0.0..=800.0).contains(&e.0) && (0.0..=600.0).contains(&e.1));
            }
            let (dx, dy) = ((ea.0 - eb.0).abs(), (ea.1 - eb.1).abs());
            assert!(
                (dx - 560.0).abs() < 1e-6 || (dy - 420.0).abs() < 1e-6,
                "{yaw} {pitch}: {dx} {dy}"
            );
        }
        // a point is magnified up to the limit, a line longer than the table down to the other
        let mut o = Orbit::default();
        o.zoom_to_line(&table, 150.0, a, a, 800.0, 600.0);
        assert_eq!(o.zoom, ZOOM_MAX);
        o.zoom_to_line(
            &table,
            150.0,
            (0.0, 0.0, 0.0),
            (1e9, 1e9, 0.0),
            800.0,
            600.0,
        );
        assert_eq!(o.zoom, ZOOM_MIN);
        // the focus goes with the reset, and the wheel and the turn keep it
        o.zoom_to_line(&table, 150.0, a, b, 800.0, 600.0);
        o.drag(30.0, 5.0);
        o.wheel(10.0);
        assert!(o.focus.is_some());
        o.reset();
        assert!(o.is_home() && o.focus.is_none());
    }

    #[test]
    fn at_zoom_one_the_whole_table_is_inside_the_area_at_every_turn() {
        let table = Table::new(500.0, 300.0);
        for yaw in (0..360).step_by(30) {
            let o = Orbit {
                yaw_deg: yaw as f64,
                ..Orbit::default()
            };
            let iso = o.iso(&table, 150.0, 800.0, 600.0);
            for (x, y) in [(0.0, 0.0), (500.0, 0.0), (0.0, 300.0), (500.0, 300.0)] {
                for z in [0.0, 150.0] {
                    let (px, py) = iso.project(x, y, z);
                    assert!(
                        (0.0..=800.0).contains(&px) && (0.0..=600.0).contains(&py),
                        "yaw {yaw}: {px} {py}"
                    );
                }
            }
        }
    }
}
