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
}

/// The view the 3D starts in: the usual isometric one.
pub const ORBIT_HOME: Orbit = Orbit {
    yaw_deg: 0.0,
    pitch_deg: 30.0,
    zoom: 1.0,
};

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

    pub fn is_home(&self) -> bool {
        *self == ORBIT_HOME
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
        // the middle of the table (half way up the volume) to the middle of the area
        let (mx, my) = iso.project(iso.pivot.0, iso.pivot.1, height_mm.max(1.0) / 2.0);
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
