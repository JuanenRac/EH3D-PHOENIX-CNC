//! A map of the height of the stock: the Z of a grid of points measured with the probe, and the program corrected to follow it. A cut that
//! must keep a constant depth over a board that is not flat (a printed circuit, a sheet of wood) is cut into short moves and the Z of each is
//! raised or lowered by the height the map has under it (bilinear between the four nearest points).
//!
//! The map holds heights relative to the point probed that is nearest to the zero of the work (the first of the grid when two are equally near),
//! so the Z of the work is zeroed there. It knows nothing of the controller or the
//! window: the readings come in as numbers (the report `[PRB:x,y,z:1]` of GRBL read by [`parse_probe_report`], or a closure in the probe
//! module), and it is tested on its own.

use crate::gcode::words;

/// The most points of a grid and the most lines a corrected program may have (a bound against a setting typed wrong).
pub const MAX_POINTS: usize = 10_000;
pub const MAX_LINES: usize = 2_000_000;

/// How far outside the probed area a point may lie before the program is refused, in mm.
const OUTSIDE_TOLERANCE_MM: f64 = 0.5;

/// A grid of heights.
#[derive(Debug, Clone, PartialEq)]
pub struct HeightMap {
    /// The corner with the smallest X and Y, and the distance between points along X and Y, in mm.
    pub x0: f64,
    pub y0: f64,
    pub dx: f64,
    pub dy: f64,
    /// How many points along X and along Y.
    pub nx: usize,
    pub ny: usize,
    /// The heights relative to the point nearest to the zero of the work (mm), row by row from the smallest Y; `z[j * nx + i]`.
    pub z: Vec<f64>,
}

/// The points of the grid in the order they are probed: row by row from the smallest Y, the direction of X alternating, so the head never
/// goes back across the whole board between two rows. `area` is `[x0, y0, x1, y1]`.
pub fn probe_points(area: [f64; 4], nx: usize, ny: usize) -> Result<Vec<(f64, f64)>, String> {
    check_grid(area, nx, ny)?;
    let (dx, dy) = steps(area, nx, ny);
    let mut out = Vec::with_capacity(nx * ny);
    for j in 0..ny {
        for k in 0..nx {
            let i = if j % 2 == 0 { k } else { nx - 1 - k };
            out.push((area[0] + dx * i as f64, area[1] + dy * j as f64));
        }
    }
    Ok(out)
}

fn check_grid(area: [f64; 4], nx: usize, ny: usize) -> Result<(), String> {
    if area.iter().any(|v| !v.is_finite()) {
        return Err("the area has a number that is not finite".to_string());
    }
    if area[2] <= area[0] || area[3] <= area[1] {
        return Err("the area is empty (the second corner must be past the first)".to_string());
    }
    if nx < 2 || ny < 2 {
        return Err("the grid needs at least 2 points along each side".to_string());
    }
    if nx.saturating_mul(ny) > MAX_POINTS {
        return Err(format!("the grid has more than {MAX_POINTS} points"));
    }
    Ok(())
}

fn steps(area: [f64; 4], nx: usize, ny: usize) -> (f64, f64) {
    (
        (area[2] - area[0]) / (nx - 1) as f64,
        (area[3] - area[1]) / (ny - 1) as f64,
    )
}

/// The Z of every point of a report of the probe, in the order the report has them (`[PRB:x,y,z:1]` lines of GRBL, the last number says whether the
/// probe touched). A point that did not touch is `None`; anything that is not a `PRB` line is left out.
pub fn parse_probe_report(text: &str) -> Vec<Option<f64>> {
    let mut out = Vec::new();
    for line in text.lines() {
        let Some(start) = line.find("PRB:") else {
            continue;
        };
        let body = line[start + 4..].trim().trim_end_matches(']');
        let Some((coords, ok)) = body.rsplit_once(':') else {
            continue;
        };
        let z = coords
            .split(',')
            .nth(2)
            .and_then(|v| v.trim().parse::<f64>().ok())
            .filter(|v| v.is_finite());
        out.push(if ok.trim() == "1" { z } else { None });
    }
    out
}

impl HeightMap {
    /// Builds the map from the readings (machine Z of each contact) in the order of [`probe_points`]. The heights are taken relative to the
    /// reading of the point nearest to the zero of the work (the first in that order when two are equally near).
    pub fn from_readings(
        area: [f64; 4],
        nx: usize,
        ny: usize,
        readings: &[f64],
    ) -> Result<HeightMap, String> {
        check_grid(area, nx, ny)?;
        if readings.len() != nx * ny {
            return Err(format!(
                "{} readings were given and the grid has {} points",
                readings.len(),
                nx * ny
            ));
        }
        if readings.iter().any(|v| !v.is_finite()) {
            return Err("a reading is not a finite number".to_string());
        }
        let (dx, dy) = steps(area, nx, ny);
        let mut nearest = (f64::INFINITY, 0);
        for (k, (x, y)) in probe_points(area, nx, ny)?.iter().enumerate() {
            let d = x.hypot(*y);
            if d < nearest.0 {
                nearest = (d, k);
            }
        }
        let reference = readings[nearest.1];
        let mut z = vec![0.0; nx * ny];
        for (k, r) in readings.iter().enumerate() {
            let (j, c) = (k / nx, k % nx);
            let i = if j % 2 == 0 { c } else { nx - 1 - c };
            z[j * nx + i] = r - reference;
        }
        Ok(HeightMap {
            x0: area[0],
            y0: area[1],
            dx,
            dy,
            nx,
            ny,
            z,
        })
    }

    /// Builds the map from the text of a report of the probe (see [`parse_probe_report`]); a point that did not touch is an error.
    pub fn from_report(
        area: [f64; 4],
        nx: usize,
        ny: usize,
        report: &str,
    ) -> Result<HeightMap, String> {
        let parsed = parse_probe_report(report);
        if let Some(k) = parsed.iter().position(Option::is_none) {
            return Err(format!("point {} of the report did not touch", k + 1));
        }
        let readings: Vec<f64> = parsed.into_iter().flatten().collect();
        HeightMap::from_readings(area, nx, ny, &readings)
    }

    /// The area the map covers: `[x0, y0, x1, y1]`.
    pub fn area(&self) -> [f64; 4] {
        [
            self.x0,
            self.y0,
            self.x0 + self.dx * (self.nx - 1) as f64,
            self.y0 + self.dy * (self.ny - 1) as f64,
        ]
    }

    /// Whether the point is in the area, with the tolerance of the program.
    pub fn contains(&self, x: f64, y: f64) -> bool {
        let a = self.area();
        let t = OUTSIDE_TOLERANCE_MM;
        (a[0] - t..=a[2] + t).contains(&x) && (a[1] - t..=a[3] + t).contains(&y)
    }

    /// The height under the point, interpolated between the four nearest ones (a point just outside the area takes the nearest edge).
    pub fn offset_at(&self, x: f64, y: f64) -> f64 {
        let fx = ((x - self.x0) / self.dx).clamp(0.0, (self.nx - 1) as f64);
        let fy = ((y - self.y0) / self.dy).clamp(0.0, (self.ny - 1) as f64);
        let (i, j) = (
            (fx.floor() as usize).min(self.nx - 2),
            (fy.floor() as usize).min(self.ny - 2),
        );
        let (tx, ty) = (fx - i as f64, fy - j as f64);
        let at = |i: usize, j: usize| self.z[j * self.nx + i];
        let bottom = at(i, j) * (1.0 - tx) + at(i + 1, j) * tx;
        let top = at(i, j + 1) * (1.0 - tx) + at(i + 1, j + 1) * tx;
        bottom * (1.0 - ty) + top * ty
    }

    /// The highest and the lowest height of the map, relative to the first point.
    pub fn span(&self) -> (f64, f64) {
        self.z
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &v| {
                (lo.min(v), hi.max(v))
            })
    }

    /// The map as text: a first line with the area and the grid, then one line of heights per row.
    pub fn to_text(&self) -> String {
        let a = self.area();
        let mut out = format!(
            "heightmap {} {} {} {} {} {}\n",
            a[0], a[1], a[2], a[3], self.nx, self.ny
        );
        for row in self.z.chunks(self.nx) {
            let cells: Vec<String> = row.iter().map(|v| format!("{v}")).collect();
            out.push_str(&cells.join(" "));
            out.push('\n');
        }
        out
    }

    /// Reads what [`HeightMap::to_text`] wrote; anything that does not fit the grid is an error.
    pub fn parse(text: &str) -> Result<HeightMap, String> {
        let mut lines = text.trim_start_matches('\u{feff}').lines();
        let head: Vec<&str> = lines
            .next()
            .ok_or("the map is empty")?
            .split_whitespace()
            .collect();
        if head.len() != 7 || head[0] != "heightmap" {
            return Err("the first line is not a height map".to_string());
        }
        let num = |s: &str| {
            s.parse::<f64>()
                .map_err(|_| format!("'{s}' is not a number"))
        };
        let area = [num(head[1])?, num(head[2])?, num(head[3])?, num(head[4])?];
        let size = |s: &str| {
            s.parse::<usize>()
                .map_err(|_| format!("'{s}' is not a size"))
        };
        let (nx, ny) = (size(head[5])?, size(head[6])?);
        check_grid(area, nx, ny)?;
        let mut z = Vec::with_capacity(nx * ny);
        for row in lines.filter(|l| !l.trim().is_empty()) {
            for cell in row.split_whitespace() {
                let v = num(cell)?;
                if !v.is_finite() {
                    return Err("a height is not a finite number".to_string());
                }
                z.push(v);
            }
        }
        if z.len() != nx * ny {
            return Err(format!(
                "the map has {} heights and its grid {}",
                z.len(),
                nx * ny
            ));
        }
        let (dx, dy) = steps(area, nx, ny);
        Ok(HeightMap {
            x0: area[0],
            y0: area[1],
            dx,
            dy,
            nx,
            ny,
            z,
        })
    }

    /// Corrects a program to follow the surface: every cutting move is cut into pieces of at most `segment_mm` and the Z of each piece is
    /// raised by the height under it; a rapid move that sets a Z is raised by the height under its end. The program must be in absolute
    /// distances and millimetres, with no arcs and no shift of the zero (the program is refused, saying the line, when it has any of them
    /// or when a move leaves the probed area), because the correction would not be right.
    pub fn compensate(&self, lines: &[String], segment_mm: f64) -> Result<Vec<String>, String> {
        if !segment_mm.is_finite() || segment_mm < 0.01 {
            return Err("the length of a piece must be at least 0.01 mm".to_string());
        }
        let mut out: Vec<String> = Vec::with_capacity(lines.len());
        let mut pos = (0.0, 0.0, 0.0);
        let mut motion = 0_u8;
        for (n, raw) in lines.iter().enumerate() {
            let ws = words(raw);
            let at = |msg: &str| format!("line {}: {msg}", n + 1);
            let mut target: [Option<f64>; 3] = [None; 3];
            let mut feed = None;
            let mut other: Vec<(char, f64)> = Vec::new();
            let mut skips_move = false;
            for &(l, v) in &ws {
                let tenths = (v * 10.0).round() as i32;
                match l {
                    'G' => match tenths {
                        0 => motion = 0,
                        10 => motion = 1,
                        20 | 30 => return Err(at("an arc cannot be corrected")),
                        200 => return Err(at("inches are not supported, use millimetres")),
                        910 => return Err(at("relative distances are not supported")),
                        530 | 920 | 921 | 922 | 923 => {
                            return Err(at("a shift of the zero cannot be corrected"))
                        }
                        // these take axis words that are not a cutting move
                        100 | 280 | 281 | 300 | 301 | 431 => {
                            skips_move = true;
                            other.push((l, v));
                        }
                        _ => other.push((l, v)),
                    },
                    'X' => target[0] = Some(v),
                    'Y' => target[1] = Some(v),
                    'Z' => target[2] = Some(v),
                    'F' => feed = Some(v),
                    _ => other.push((l, v)),
                }
            }
            if skips_move || target.iter().all(Option::is_none) {
                out.push(raw.clone());
                continue;
            }
            let to = (
                target[0].unwrap_or(pos.0),
                target[1].unwrap_or(pos.1),
                target[2].unwrap_or(pos.2),
            );
            if motion == 0 {
                match target[2] {
                    None => out.push(raw.clone()),
                    Some(z) => {
                        if !self.contains(to.0, to.1) {
                            return Err(at("the move leaves the probed area"));
                        }
                        out.push(rebuild(
                            0,
                            &other,
                            [target[0], target[1], Some(z + self.offset_at(to.0, to.1))],
                            None,
                        ));
                    }
                }
            } else {
                let flat = (to.0 - pos.0).hypot(to.1 - pos.1);
                let pieces = ((flat / segment_mm).ceil() as usize).max(1);
                if out.len().saturating_add(pieces) > MAX_LINES {
                    return Err(at("the corrected program would be too long"));
                }
                for k in 1..=pieces {
                    let f = k as f64 / pieces as f64;
                    let p = (
                        pos.0 + (to.0 - pos.0) * f,
                        pos.1 + (to.1 - pos.1) * f,
                        pos.2 + (to.2 - pos.2) * f,
                    );
                    if !self.contains(p.0, p.1) {
                        return Err(at("the move leaves the probed area"));
                    }
                    let first = k == 1;
                    out.push(rebuild(
                        1,
                        if first { &other } else { &[] },
                        [Some(p.0), Some(p.1), Some(p.2 + self.offset_at(p.0, p.1))],
                        if first { feed } else { None },
                    ));
                }
            }
            pos = to;
        }
        Ok(out)
    }
}

/// A number as short as it can be written: up to 4 decimals, no trailing zeros and no `-0`.
pub fn num(v: f64) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() {
        "0".to_string()
    } else {
        s.to_string()
    }
}

fn rebuild(motion: u8, other: &[(char, f64)], xyz: [Option<f64>; 3], feed: Option<f64>) -> String {
    let mut parts: Vec<String> = other
        .iter()
        .map(|&(l, v)| format!("{l}{}", num(v)))
        .collect();
    parts.push(format!("G{motion}"));
    for (axis, v) in ["X", "Y", "Z"].iter().zip(xyz) {
        if let Some(v) = v {
            parts.push(format!("{axis}{}", num(v)));
        }
    }
    if let Some(f) = feed {
        parts.push(format!("F{}", num(f)));
    }
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(s: &str) -> Vec<String> {
        s.lines().map(str::to_string).collect()
    }

    /// A 3 x 2 grid over 0..20 x 0..10 whose heights rise 0.1 mm per mm along X.
    fn ramp() -> HeightMap {
        let readings: Vec<f64> = probe_points([0.0, 0.0, 20.0, 10.0], 3, 2)
            .unwrap()
            .iter()
            .map(|p| -5.0 + p.0 * 0.1)
            .collect();
        HeightMap::from_readings([0.0, 0.0, 20.0, 10.0], 3, 2, &readings).unwrap()
    }

    #[test]
    fn the_points_go_row_by_row_and_back() {
        let p = probe_points([0.0, 0.0, 20.0, 10.0], 3, 2).unwrap();
        assert_eq!(
            p,
            vec![
                (0.0, 0.0),
                (10.0, 0.0),
                (20.0, 0.0),
                (20.0, 10.0),
                (10.0, 10.0),
                (0.0, 10.0)
            ]
        );
    }

    #[test]
    fn a_grid_that_makes_no_sense_is_refused() {
        assert!(probe_points([0.0, 0.0, 0.0, 10.0], 3, 3).is_err());
        assert!(probe_points([0.0, 0.0, 10.0, 10.0], 1, 3).is_err());
        assert!(probe_points([0.0, 0.0, 10.0, f64::NAN], 3, 3).is_err());
        assert!(probe_points([0.0, 0.0, 10.0, 10.0], 200, 200).is_err());
    }

    #[test]
    fn the_heights_are_relative_to_the_first_point_whatever_the_order_of_the_probe() {
        let m = ramp();
        assert_eq!(m.z, vec![0.0, 1.0, 2.0, 0.0, 1.0, 2.0]);
        assert_eq!(m.span(), (0.0, 2.0));
        assert_eq!(m.area(), [0.0, 0.0, 20.0, 10.0]);
    }

    #[test]
    fn the_heights_are_relative_to_the_point_nearest_to_the_zero_of_the_work() {
        // a 3 x 3 grid over -10..10 has the zero at its centre, where the surface is at 5.0 and rises 0.1 mm per mm along X
        let area = [-10.0, -10.0, 10.0, 10.0];
        let readings: Vec<f64> = probe_points(area, 3, 3)
            .unwrap()
            .iter()
            .map(|p| 5.0 + p.0 * 0.1)
            .collect();
        let m = HeightMap::from_readings(area, 3, 3, &readings).unwrap();
        assert!(m.offset_at(0.0, 0.0).abs() < 1e-9);
        assert!((m.offset_at(10.0, 0.0) - 1.0).abs() < 1e-9);
        assert!((m.offset_at(-10.0, 5.0) + 1.0).abs() < 1e-9);
    }

    #[test]
    fn two_points_equally_near_the_zero_take_the_first_of_the_grid() {
        // four corners, all at the same distance: the first probed is the reference
        let m =
            HeightMap::from_readings([-5.0, -5.0, 5.0, 5.0], 2, 2, &[1.0, 2.0, 3.0, 4.0]).unwrap();
        assert_eq!(m.z, vec![0.0, 1.0, 3.0, 2.0]);
        // a grid that does not hold the zero takes its nearest corner, the one at (10, 10)
        let m = HeightMap::from_readings([10.0, 10.0, 30.0, 30.0], 2, 2, &[7.0, 8.0, 9.0, 6.0])
            .unwrap();
        assert_eq!(m.z[0], 0.0);
    }

    #[test]
    fn the_height_is_interpolated_and_clamped_at_the_edge() {
        let m = ramp();
        assert!((m.offset_at(5.0, 3.0) - 0.5).abs() < 1e-9);
        assert!((m.offset_at(20.0, 10.0) - 2.0).abs() < 1e-9);
        assert!((m.offset_at(25.0, -4.0) - 2.0).abs() < 1e-9);
        assert!(m.contains(20.4, 0.0) && !m.contains(21.0, 0.0));
    }

    #[test]
    fn a_report_of_the_probe_is_read_and_a_miss_is_found() {
        let text = "ok\n[PRB:0.000,0.000,-5.000:1]\n[PRB:10.000,0.000,-4.000:1]\n[PRB:1.0,2.0,3.0:0]\nnoise";
        let got = parse_probe_report(text);
        assert_eq!(got, vec![Some(-5.0), Some(-4.0), None]);
        let err = HeightMap::from_report([0.0, 0.0, 10.0, 10.0], 2, 2, text).unwrap_err();
        assert!(err.contains("point 3"), "{err}");
        let ok = "[PRB:0,0,-5:1]\n[PRB:10,0,-4:1]\n[PRB:10,10,-3:1]\n[PRB:0,10,-6:1]";
        let m = HeightMap::from_report([0.0, 0.0, 10.0, 10.0], 2, 2, ok).unwrap();
        assert_eq!(m.z, vec![0.0, 1.0, -1.0, 2.0]);
    }

    #[test]
    fn a_wrong_number_of_readings_is_refused() {
        let a = [0.0, 0.0, 10.0, 10.0];
        assert!(HeightMap::from_readings(a, 2, 2, &[1.0, 2.0, 3.0]).is_err());
        assert!(HeightMap::from_readings(a, 2, 2, &[1.0, 2.0, 3.0, f64::NAN]).is_err());
    }

    #[test]
    fn the_map_is_written_and_read_back() {
        let m = ramp();
        assert_eq!(HeightMap::parse(&m.to_text()).unwrap(), m);
        assert!(HeightMap::parse("").is_err());
        assert!(HeightMap::parse("heightmap 0 0 10 10 2 2\n0 1\n2").is_err());
        assert!(HeightMap::parse("heightmap 0 0 10 10 2 2\n0 1\n2 inf").is_err());
        assert!(HeightMap::parse("grid 0 0 10 10 2 2").is_err());
    }

    #[test]
    fn a_cut_is_split_and_follows_the_surface() {
        let m = ramp();
        let out = m
            .compensate(&lines("G0 X0 Y0\nG1 Z-1 F100\nG1 X20 F300"), 5.0)
            .unwrap();
        // the plunge at the start (offset 0), then four pieces rising 0.5 mm each
        assert_eq!(out[0], "G0 X0 Y0");
        assert_eq!(out[1], "G1 X0 Y0 Z-1 F100");
        assert_eq!(out[2], "G1 X5 Y0 Z-0.5 F300");
        assert_eq!(out[5], "G1 X20 Y0 Z1");
        assert_eq!(out.len(), 6);
    }

    #[test]
    fn a_rapid_that_sets_a_z_is_raised_and_the_other_lines_pass_unchanged() {
        let m = ramp();
        let out = m
            .compensate(
                &lines("G21 G90\nM3 S1000\n(note)\nG0 X20 Y0 Z5\nG0 X0 Y0\nM5"),
                1.0,
            )
            .unwrap();
        assert_eq!(out[0], "G21 G90");
        assert_eq!(out[1], "M3 S1000");
        assert_eq!(out[3], "G0 X20 Y0 Z7");
        assert_eq!(out[4], "G0 X0 Y0");
        assert_eq!(out[5], "M5");
    }

    #[test]
    fn a_program_that_cannot_be_corrected_is_refused_with_its_line() {
        let m = ramp();
        for (p, why) in [
            ("G1 X1 F10\nG2 X5 Y5 I1 J1", "line 2"),
            ("G20\nG1 X1", "line 1"),
            ("G91\nG1 X1", "line 1"),
            ("G92 X0", "line 1"),
            ("G1 X30 F10", "probed area"),
            ("G1 X-3 F10", "probed area"),
        ] {
            let e = m.compensate(&lines(p), 1.0).unwrap_err();
            assert!(e.contains(why), "{p}: {e}");
        }
        assert!(m.compensate(&lines("G1 X1"), 0.0).is_err());
    }

    #[test]
    fn lines_that_take_axis_words_without_cutting_are_left_alone() {
        let m = ramp();
        let out = m
            .compensate(&lines("G10 L20 P1 Z0\nG43.1 Z2\nG28 Z0"), 1.0)
            .unwrap();
        assert_eq!(out, lines("G10 L20 P1 Z0\nG43.1 Z2\nG28 Z0"));
    }

    #[test]
    fn numbers_are_written_short() {
        assert_eq!(num(1.0), "1");
        assert_eq!(num(-0.00001), "0");
        assert_eq!(num(2.50004), "2.5");
        assert_eq!(num(-0.1234), "-0.1234");
    }
}
