//! The path of a G-code program, for drawing it before it runs: the straight moves (`G0` rapid and `G1` cutting), absolute or relative,
//! in millimetres or inches. Arcs and the other codes are not drawn (a controller that cannot take them refuses them when they run).

/// A point of the machine: X, Y and Z in mm.
pub type Point = (f64, f64, f64);

/// One straight move of the program.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Segment {
    pub from: Point,
    pub to: Point,
    /// A rapid move (`G0`), which is not cutting.
    pub rapid: bool,
    /// The line of the program it comes from (from 0).
    pub line: usize,
    /// The feed rate in force (`F`, in mm per minute) when the move was made; 0 when the program had not set one yet.
    pub feed: f64,
    /// The line is an arc (`G2`/`G3`): the segment is the chord between its ends, not the arc, so the time and the reach leave it out.
    pub arc: bool,
}

/// The straight moves of the program, in order, starting at the origin.
pub fn toolpath(lines: &[String]) -> Vec<Segment> {
    trace(lines).0
}

/// The length of the curve of an arc move, in mm: the lines that have one (their number from 0) and the length along the arc, with the
/// rise of a helix. An arc that cannot be measured (no centre and no radius, or a centre on the start) has no entry, and its chord counts.
pub fn arc_lengths(lines: &[String]) -> Vec<(usize, f64)> {
    trace(lines)
        .1
        .iter()
        .filter_map(|a| {
            arc_length(
                a.from,
                a.to,
                a.plane,
                a.cw,
                a.off,
                a.radius,
                a.centre_absolute,
            )
            .map(|l| (a.line, l))
        })
        .collect()
}

/// One arc move as the program wrote it, kept to work out its curve, its length and what is wrong with it.
#[derive(Debug, Clone, Copy)]
struct ArcRec {
    line: usize,
    from: Point,
    to: Point,
    plane: u8,
    cw: bool,
    off: [Option<f64>; 3],
    radius: Option<f64>,
    centre_absolute: bool,
}

/// The circle of an arc: its centre in the two axes of the plane, its radius, the angle it starts at and the signed sweep (positive
/// counter-clockwise), with the index of the three axes (the two of the plane and the one it rises along).
struct ArcGeo {
    axes: (usize, usize, usize),
    centre: (f64, f64),
    radius: f64,
    start: f64,
    sweep: f64,
    /// How far the end of the arc is from the circle that starts at the start of the arc, in mm (0 for an arc given by its radius).
    end_gap: f64,
    /// An arc given by its radius that is smaller than half the distance between its ends: by how much, in mm.
    too_small: f64,
}

fn arc_geo(a: &ArcRec) -> Option<ArcGeo> {
    let axes = match a.plane {
        18 => (2, 0, 1),
        19 => (1, 2, 0),
        _ => (0, 1, 2),
    };
    let at = |p: Point, i: usize| [p.0, p.1, p.2][i];
    let (sa, sb, ea, eb) = (
        at(a.from, axes.0),
        at(a.from, axes.1),
        at(a.to, axes.0),
        at(a.to, axes.1),
    );
    let tau = std::f64::consts::TAU;
    let (centre, radius, end_gap, too_small) = if let Some(rr) = a.radius {
        let (dx, dy) = (ea - sa, eb - sb);
        let d = dx.hypot(dy);
        let r = rr.abs();
        if d <= 1e-9 || r <= 1e-9 {
            return None;
        }
        let small = (d / 2.0 - r).max(0.0);
        let h = (r * r - d * d / 4.0).max(0.0).sqrt();
        let (ux, uy) = (dx / d, dy / d);
        // the centre is to the left of the chord for a short counter-clockwise arc or a long clockwise one
        let left = (!a.cw) != (rr < 0.0);
        let side = if left { 1.0 } else { -1.0 };
        let centre = (sa + dx / 2.0 - side * uy * h, sb + dy / 2.0 + side * ux * h);
        (centre, r, 0.0, small)
    } else {
        if a.off[axes.0].is_none() && a.off[axes.1].is_none() {
            return None;
        }
        let centre = if a.centre_absolute {
            (a.off[axes.0].unwrap_or(sa), a.off[axes.1].unwrap_or(sb))
        } else {
            (
                sa + a.off[axes.0].unwrap_or(0.0),
                sb + a.off[axes.1].unwrap_or(0.0),
            )
        };
        let r = (sa - centre.0).hypot(sb - centre.1);
        if r <= 1e-9 {
            return None;
        }
        let r_end = (ea - centre.0).hypot(eb - centre.1);
        (centre, r, (r_end - r).abs(), 0.0)
    };
    let start = (sb - centre.1).atan2(sa - centre.0);
    let mut d = (eb - centre.1).atan2(ea - centre.0) - start;
    if a.cw {
        while d > 1e-12 {
            d -= tau;
        }
    } else {
        while d < -1e-12 {
            d += tau;
        }
    }
    if d.abs() < 1e-9 {
        d = if a.cw { -tau } else { tau };
    }
    Some(ArcGeo {
        axes,
        centre,
        radius,
        start,
        sweep: d,
        end_gap,
        too_small,
    })
    .filter(|g| g.centre.0.is_finite() && g.centre.1.is_finite() && g.sweep.is_finite())
}

/// The curve of each arc of the program as a run of points (`from` and `to` included, the rise of a helix spread along it): the
/// lines that have one (from 0) and the points, none further apart along the curve than `step_mm` (and never more than 720 pieces).
/// An arc that cannot be worked out (no centre and no radius, or a centre on the start) has no entry and only its chord is known.
pub fn arc_curves(lines: &[String], step_mm: f64) -> Vec<(usize, Vec<Point>)> {
    let step = if step_mm.is_finite() && step_mm > 0.01 {
        step_mm
    } else {
        0.5
    };
    curves_with(lines, 720, |g| {
        (g.sweep.abs() * g.radius / step).ceil() as usize
    })
}

/// The curve of each arc as straight pieces that leave the arc by no more than `tolerance_mm` (the sagitta of a piece), with the
/// angle of a piece worked out from the radius: a small arc gets few pieces and a large one many (never more than 4096 for an arc).
/// A tolerance that is not a positive number is taken as 0.01 mm.
pub fn arc_curves_within(lines: &[String], tolerance_mm: f64) -> Vec<(usize, Vec<Point>)> {
    let tol = if tolerance_mm.is_finite() && tolerance_mm > 0.0 {
        tolerance_mm
    } else {
        0.01
    };
    curves_with(lines, 4096, |g| {
        if tol >= g.radius {
            return 4;
        }
        let piece = 2.0 * (1.0 - tol / g.radius).clamp(-1.0, 1.0).acos();
        if piece > 1e-9 {
            (g.sweep.abs() / piece).ceil() as usize
        } else {
            4096
        }
    })
}

fn curves_with(
    lines: &[String],
    most: usize,
    pieces: impl Fn(&ArcGeo) -> usize,
) -> Vec<(usize, Vec<Point>)> {
    let mut out = Vec::new();
    for a in trace(lines).1 {
        let Some(g) = arc_geo(&a) else { continue };
        let at = |p: Point, i: usize| [p.0, p.1, p.2][i];
        let rise = at(a.to, g.axes.2) - at(a.from, g.axes.2);
        let n = pieces(&g).clamp(1, most);
        let mut pts: Vec<Point> = Vec::with_capacity(n + 1);
        pts.push(a.from);
        for k in 1..n {
            let t = k as f64 / n as f64;
            let ang = g.start + g.sweep * t;
            let mut v = [0.0f64; 3];
            v[g.axes.0] = g.centre.0 + g.radius * ang.cos();
            v[g.axes.1] = g.centre.1 + g.radius * ang.sin();
            v[g.axes.2] = at(a.from, g.axes.2) + rise * t;
            pts.push((v[0], v[1], v[2]));
        }
        pts.push(a.to);
        out.push((a.line, pts));
    }
    out
}

/// What is wrong with an arc: a controller refuses it with a radius error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArcFault {
    /// The arc is given by its centre and its end is not at the same distance from the centre as its start.
    EndRadius,
    /// The arc is given by its radius (`R`) and the radius is less than half the distance between its ends.
    RadiusTooSmall,
}

/// The arcs of the program that a controller would refuse: the line (from 0), what is wrong and by how many mm. The end of an arc given
/// by its centre may be off by 0.005 mm up to a thousandth of its radius and never more than 0.5 mm (what common controllers accept); an arc given
/// by its radius may be smaller than half its chord by 0.0005 mm (the rounding of a number written with three decimals).
pub fn arc_faults(lines: &[String]) -> Vec<(usize, ArcFault, f64)> {
    arc_faults_within(lines, ARC_GAP_MM)
}

/// The gap in mm that an arc given by its centre may end off its circle before it is a fault (see `arc_faults_within`).
pub const ARC_GAP_MM: f64 = 0.005;

/// As `arc_faults`, with the smallest gap that counts (0.005 mm above) given: a bigger one is more forgiving of a program written with few
/// decimals, a smaller one stricter. The gap is never taken above 0.5 mm, the most any controller accepts.
pub fn arc_faults_within(lines: &[String], gap_mm: f64) -> Vec<(usize, ArcFault, f64)> {
    let gap = if gap_mm.is_finite() {
        gap_mm.clamp(0.0, 0.5)
    } else {
        ARC_GAP_MM
    };
    let mut out = Vec::new();
    for a in trace(lines).1 {
        let Some(g) = arc_geo(&a) else { continue };
        if g.too_small > 0.0005 {
            out.push((a.line, ArcFault::RadiusTooSmall, g.too_small));
        } else if g.end_gap > 0.5 || (g.end_gap > gap && g.end_gap > 0.001 * g.radius) {
            out.push((a.line, ArcFault::EndRadius, g.end_gap));
        }
    }
    out
}

/// The length along an arc: from `from` to `to` in the plane `plane` (17, 18 or 19), clockwise or not, with the offsets of the centre
/// `off` (I, J, K) or the radius `radius`. A helix adds the rise of the third axis.
fn arc_length(
    from: Point,
    to: Point,
    plane: u8,
    cw: bool,
    off: [Option<f64>; 3],
    radius: Option<f64>,
    centre_absolute: bool,
) -> Option<f64> {
    let (a, b, c) = match plane {
        18 => (2, 0, 1),
        19 => (1, 2, 0),
        _ => (0, 1, 2),
    };
    let at = |p: Point, i: usize| [p.0, p.1, p.2][i];
    let (sa, sb, ea, eb) = (at(from, a), at(from, b), at(to, a), at(to, b));
    let rise = at(to, c) - at(from, c);
    let tau = std::f64::consts::TAU;
    let (r, sweep) = if let Some(rr) = radius {
        let half = (ea - sa).hypot(eb - sb) / 2.0;
        let r = rr.abs().max(half);
        if r <= 1e-9 {
            return None;
        }
        let minor = 2.0 * (half / r).clamp(0.0, 1.0).asin();
        (r, if rr < 0.0 { tau - minor } else { minor })
    } else {
        if off[a].is_none() && off[b].is_none() {
            return None;
        }
        let (ca, cb) = if centre_absolute {
            (off[a].unwrap_or(sa), off[b].unwrap_or(sb))
        } else {
            (sa + off[a].unwrap_or(0.0), sb + off[b].unwrap_or(0.0))
        };
        let r = (sa - ca).hypot(sb - cb);
        if r <= 1e-9 {
            return None;
        }
        let mut d = (eb - cb).atan2(ea - ca) - (sb - cb).atan2(sa - ca);
        if cw {
            while d > 1e-12 {
                d -= tau;
            }
        } else {
            while d < -1e-12 {
                d += tau;
            }
        }
        let d = d.abs();
        (r, if d < 1e-9 { tau } else { d })
    };
    Some((r * sweep).hypot(rise)).filter(|l| l.is_finite())
}

/// The straight moves of the program and the length of the curve of its arcs.
fn trace(lines: &[String]) -> (Vec<Segment>, Vec<ArcRec>) {
    let mut out = Vec::new();
    let mut arcs: Vec<ArcRec> = Vec::new();
    let (mut pos, mut absolute, mut scale, mut rapid) = ((0.0, 0.0, 0.0), true, 1.0, true);
    let (mut feed, mut arc) = (0.0, false);
    let (mut plane, mut cw, mut centre_absolute) = (17u8, true, false);
    for (n, raw) in lines.iter().enumerate() {
        let mut off: [Option<f64>; 3] = [None; 3];
        let mut radius: Option<f64> = None;
        let text = raw.split(';').next().unwrap_or("");
        let mut target = [None; 3];
        let mut word = String::new();
        let mut words: Vec<String> = Vec::new();
        let mut in_comment = false;
        for c in text.chars() {
            match c {
                '(' => in_comment = true,
                ')' => in_comment = false,
                c if in_comment || c.is_whitespace() => {}
                c if c.is_ascii_alphabetic() => {
                    if !word.is_empty() {
                        words.push(std::mem::take(&mut word));
                    }
                    word.push(c.to_ascii_uppercase());
                }
                c => word.push(c),
            }
        }
        if !word.is_empty() {
            words.push(word);
        }
        for w in &words {
            // the first character may be one that is not a letter of A to Z (and is more than one byte long)
            let (letter, num) = w.split_at(w.chars().next().map_or(0, char::len_utf8));
            let Ok(v) = num.parse::<f64>() else { continue };
            match letter {
                // in tenths, so that G90.1 and G91.1 (the arc centres) are not read as G90 and G91
                "G" => match (v * 10.0).round() as i32 {
                    0 => (rapid, arc) = (true, false),
                    10 => (rapid, arc) = (false, false),
                    20 | 30 => {
                        (rapid, arc) = (false, true);
                        cw = (v * 10.0).round() as i32 == 20;
                    }
                    900 => absolute = true,
                    910 => absolute = false,
                    901 => centre_absolute = true,
                    911 => centre_absolute = false,
                    170 => plane = 17,
                    180 => plane = 18,
                    190 => plane = 19,
                    200 => scale = 25.4,
                    210 => scale = 1.0,
                    _ => {}
                },
                "X" => target[0] = Some(v * scale),
                "Y" => target[1] = Some(v * scale),
                "Z" => target[2] = Some(v * scale),
                "I" => off[0] = Some(v * scale),
                "J" => off[1] = Some(v * scale),
                "K" => off[2] = Some(v * scale),
                "R" => radius = Some(v * scale),
                "F" => feed = v * scale,
                _ => {}
            }
        }
        if target.iter().any(Option::is_some) {
            let pick = |t: Option<f64>, now: f64| match (t, absolute) {
                (Some(v), true) => v,
                (Some(v), false) => now + v,
                (None, _) => now,
            };
            let to = (
                pick(target[0], pos.0),
                pick(target[1], pos.1),
                pick(target[2], pos.2),
            );
            if to != pos {
                if arc {
                    arcs.push(ArcRec {
                        line: n,
                        from: pos,
                        to,
                        plane,
                        cw,
                        off,
                        radius,
                        centre_absolute,
                    });
                }
                out.push(Segment {
                    from: pos,
                    to,
                    rapid,
                    line: n,
                    feed,
                    arc,
                });
            }
            pos = to;
        }
    }
    (out, arcs)
}

/// A program seen before it is opened: its path and how big it is.
#[derive(Debug, Clone, PartialEq)]
pub struct Preview {
    pub segments: Vec<Segment>,
    /// (min x, min y, max x, max y) of the path in mm, when there is one.
    pub bounds: Option<[f64; 4]>,
    pub moves: usize,
}

/// The biggest file the preview reads: a bigger one would make the window wait.
pub const PREVIEW_MAX_BYTES: u64 = 2_000_000;

/// The path of the G-code file at `path`, or nothing when it cannot be read or is too big for a preview.
pub fn read_preview(path: &std::path::Path) -> Option<Preview> {
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > PREVIEW_MAX_BYTES {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    // the same reading as the program: every byte is a character, so a file in any code page is read
    let text: String = bytes.iter().map(|&b| b as char).collect();
    let lines: Vec<String> = text.lines().map(str::to_string).collect();
    let segments = toolpath(&lines);
    let mut bounds: Option<[f64; 4]> = None;
    for s in &segments {
        for p in [s.from, s.to] {
            bounds = Some(match bounds {
                None => [p.0, p.1, p.0, p.1],
                Some([x0, y0, x1, y1]) => [x0.min(p.0), y0.min(p.1), x1.max(p.0), y1.max(p.1)],
            });
        }
    }
    Some(Preview {
        moves: segments.len(),
        segments,
        bounds,
    })
}

/// How many lines of the program move along an arc (`G2` or `G3`): the path does not draw them and the time and the reach leave them out.
pub fn arc_moves(lines: &[String]) -> usize {
    let mut motion = 0;
    let mut count = 0;
    for raw in lines {
        let mut moves = false;
        for (letter, v) in words(raw) {
            match letter {
                'G' => match (v * 10.0).round() as i32 {
                    0 | 10 => motion = 0,
                    20 | 30 => motion = 2,
                    _ => {}
                },
                'X' | 'Y' | 'Z' | 'I' | 'J' | 'K' | 'R' => moves = true,
                _ => {}
            }
        }
        if motion == 2 && moves {
            count += 1;
        }
    }
    count
}

/// The words of a line of G-code (a letter and its number), without the comments; what is not a word is left out.
pub fn words(line: &str) -> Vec<(char, f64)> {
    let text = line.split(';').next().unwrap_or("");
    let mut out = Vec::new();
    let mut in_comment = false;
    let mut cur: Option<(char, String)> = None;
    let flush = |cur: &mut Option<(char, String)>, out: &mut Vec<(char, f64)>| {
        if let Some((l, n)) = cur.take() {
            if let Ok(v) = n.parse::<f64>() {
                out.push((l, v));
            }
        }
    };
    for c in text.chars() {
        match c {
            '(' => in_comment = true,
            ')' => in_comment = false,
            c if in_comment || c.is_whitespace() => {}
            c if c.is_ascii_alphabetic() => {
                flush(&mut cur, &mut out);
                cur = Some((c.to_ascii_uppercase(), String::new()));
            }
            c => {
                if let Some((_, n)) = cur.as_mut() {
                    n.push(c);
                }
            }
        }
    }
    flush(&mut cur, &mut out);
    out
}

/// The codes that stay on until another one of their group changes them, as the panel of a machine-control program shows them.
#[derive(Debug, Clone, PartialEq)]
pub struct Modal {
    /// `G0` or `G1`.
    pub motion: u8,
    pub inches: bool,
    pub absolute: bool,
    /// `M3`, `M4` or `M5`.
    pub spindle: u8,
    /// `M7`, `M8` or `M9`.
    pub coolant: u8,
    /// The last `F` and `S` words taken, when there was one.
    pub feed: Option<f64>,
    pub speed: Option<f64>,
    /// The work coordinate system in force: 54 to 59 (`G54` to `G59`).
    pub wcs: u8,
    /// The plane of the arcs: 17, 18 or 19 (`G17` to `G19`).
    pub plane: u8,
    /// The dynamic tool length offset (`G43.1 Zn`) in force: the Z it was given; `None` when there is none (`G49`).
    pub tool_length: Option<f64>,
}

impl Default for Modal {
    fn default() -> Self {
        Modal {
            motion: 0,
            inches: false,
            absolute: true,
            spindle: 5,
            coolant: 9,
            feed: None,
            speed: None,
            wcs: 54,
            plane: 17,
            tool_length: None,
        }
    }
}

impl Modal {
    /// Takes a line that the controller accepted.
    pub fn apply(&mut self, line: &str) {
        let all = words(line);
        for &(l, v) in &all {
            // a G code is read in tenths (G91.1 is 911, not G91); an M code is a whole number
            let tenths = (v * 10.0).round() as i32;
            match (l, v as i32) {
                ('G', _) if tenths == 0 => self.motion = 0,
                ('G', _) if tenths == 10 => self.motion = 1,
                ('G', _) if tenths == 200 => self.inches = true,
                ('G', _) if tenths == 210 => self.inches = false,
                ('G', _) if tenths == 900 => self.absolute = true,
                ('G', _) if tenths == 910 => self.absolute = false,
                ('G', _) if (540..=590).contains(&tenths) && tenths % 10 == 0 => {
                    self.wcs = (tenths / 10) as u8
                }
                ('G', _) if matches!(tenths, 170 | 180 | 190) => self.plane = (tenths / 10) as u8,
                ('G', _) if tenths == 431 => {
                    // the offset is the Z of the same line
                    let z = all.iter().find(|(c, _)| *c == 'Z').map_or(0.0, |&(_, z)| z);
                    self.tool_length = Some(z);
                }
                ('G', _) if tenths == 490 => self.tool_length = None,
                ('F', _) if v > 0.0 => self.feed = Some(v),
                ('S', _) if v >= 0.0 => self.speed = Some(v),
                ('M', n @ (3..=5)) => self.spindle = n as u8,
                ('M', n @ (7..=9)) => self.coolant = n as u8,
                ('M', 2 | 30) => {
                    self.spindle = 5;
                    self.coolant = 9;
                }
                _ => {}
            }
        }
    }

    /// The G-codes in force, in the order of the panel.
    pub fn g_codes(&self) -> Vec<String> {
        let g = |n: &str| n.to_string();
        vec![
            format!("G{}", self.motion),
            format!("G{}", self.plane),
            g(if self.inches { "G20" } else { "G21" }),
            g("G40"),
            g(if self.tool_length.is_some() {
                "G43.1"
            } else {
                "G49"
            }),
            format!("G{}", self.wcs),
            g("G64"),
            g("G80"),
            g(if self.absolute { "G90" } else { "G91" }),
            g("G94"),
            g("G97"),
            g("G99"),
        ]
    }

    /// The M-codes in force.
    pub fn m_codes(&self) -> Vec<String> {
        vec![
            format!("M{}", self.spindle),
            format!("M{}", self.coolant),
            "M48".to_string(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(s: &str) -> Vec<String> {
        s.lines().map(str::to_string).collect()
    }

    #[test]
    fn text_that_is_not_a_letter_of_a_to_z_does_not_stop_the_path() {
        // a character of another alphabet outside a comment (a file in a code page is read byte by byte) is not a word and is left out
        let p = toolpath(&lines("G1 \u{416}5 X10 F100\n\u{6728} Y5\nG1 \u{e9}3 X20"));
        assert_eq!(p.len(), 3, "{p:?}");
        assert_eq!(p[2].to, (20.0, 5.0, 0.0));
        assert!(arc_lengths(&lines("G2 X5 Y5 I2.5 J0\n\u{416} X1")).len() <= 1);
    }

    #[test]
    fn moves_are_drawn_in_order_with_rapids_told_from_cuts() {
        let p = toolpath(&lines(
            "G0 X10 Y10\nG1 Z-2 F100\nG1 X20\n(a comment) M3 S1000\nG0 Z5",
        ));
        assert_eq!(p.len(), 4);
        assert!(p[0].rapid && !p[1].rapid && !p[2].rapid && p[3].rapid);
        assert_eq!(p[0].to, (10.0, 10.0, 0.0));
        assert_eq!(p[2].from, (10.0, 10.0, -2.0));
        assert_eq!(p[2].line, 2);
    }

    #[test]
    fn relative_moves_and_inches_are_followed() {
        let p = toolpath(&lines("G91\nG1 X5\nG1 X5\nG90 G20\nG0 X1"));
        assert_eq!(p[1].to.0, 10.0);
        assert!((p[2].to.0 - 25.4).abs() < 1e-9);
    }

    #[test]
    fn the_arc_centre_modes_are_not_the_distance_modes() {
        // G91.1 (relative arc centres) in the header must not make the moves incremental
        let p = toolpath(&lines("G90 G91.1\nG1 X5\nG1 X8"));
        assert_eq!(p[1].to.0, 8.0);
        let mut m = Modal::default();
        m.apply("G90 G91.1");
        assert!(m.absolute);
        m.apply("G91");
        assert!(!m.absolute);
        m.apply("G90.1");
        assert!(
            !m.absolute,
            "G90.1 does not bring back the absolute distances"
        );
    }

    #[test]
    fn the_codes_in_force_follow_the_lines_taken() {
        let mut m = Modal::default();
        assert!(
            m.g_codes().contains(&"G90".to_string()) && m.g_codes().contains(&"G21".to_string())
        );
        assert_eq!(m.m_codes()[..2], ["M5", "M9"]);
        m.apply("G91 G20 G1 X1 F10");
        m.apply("M3 S1000 (on) M8");
        let g = m.g_codes();
        assert_eq!(
            (g[0].as_str(), g[2].as_str(), g[8].as_str()),
            ("G1", "G20", "G91")
        );
        assert_eq!(m.m_codes()[..2], ["M3", "M8"]);
        m.apply("M30");
        assert_eq!(m.m_codes()[..2], ["M5", "M9"]);
    }

    #[test]
    fn the_words_of_a_line_leave_the_comments_out() {
        assert_eq!(
            words("g1 x10.5 (skip x9) Y-2 ; z3"),
            vec![('G', 1.0), ('X', 10.5), ('Y', -2.0)]
        );
        assert!(words("; nothing").is_empty());
    }

    #[test]
    fn lines_without_a_move_draw_nothing() {
        assert!(toolpath(&lines("M3 S1000\n; nothing\nG21\nM5")).is_empty());
        // a move to where it already is is not a segment
        assert_eq!(toolpath(&lines("G0 X0 Y0")).len(), 0);
    }
    #[test]
    fn each_move_keeps_the_feed_in_force_in_mm_per_minute() {
        let p = toolpath(&lines("G0 X1\nG1 X5 F200\nG1 X9\nG20\nG1 X1 F10"));
        assert_eq!(
            p.iter().map(|s| s.feed).collect::<Vec<_>>(),
            [0.0, 200.0, 200.0, 254.0]
        );
    }

    #[test]
    fn an_arc_is_marked_so_that_the_estimate_can_leave_it_out() {
        let p = toolpath(&lines("G1 X10 F100\nG2 X20 Y10 I5 J0\nG1 X30"));
        let arcs: Vec<bool> = p.iter().map(|s| s.arc).collect();
        assert_eq!(arcs, [false, true, false]);
    }

    #[test]
    fn the_preview_reads_a_file_and_leaves_out_what_is_not_one_or_is_too_big() {
        let dir = std::env::temp_dir().join(format!("eh3d-gcode-preview-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("a.nc");
        std::fs::write(&file, "G0 X10 Y5\nG1 X20 Y25 F100\n").unwrap();
        let pv = read_preview(&file).expect("a preview");
        assert_eq!(pv.moves, 2);
        assert_eq!(pv.bounds, Some([0.0, 0.0, 20.0, 25.0]));
        // a folder, a file that is not there and a file that is too big have none
        assert!(read_preview(&dir).is_none());
        assert!(read_preview(&dir.join("missing.nc")).is_none());
        let big = dir.join("big.nc");
        std::fs::write(&big, vec![b'\n'; PREVIEW_MAX_BYTES as usize + 1]).unwrap();
        assert!(read_preview(&big).is_none());
        // a file with nothing to move has a preview with no bounds
        std::fs::write(&file, "M3 S100\n").unwrap();
        assert_eq!(read_preview(&file).unwrap().bounds, None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn arcs_are_counted_once_per_line_that_moves() {
        let program = lines("G1 X1\nG2 X2 Y2 I1 J0\nX3 Y3 I1 J1\nG1 X4\nG3 X0 R5\nG2");
        assert_eq!(arc_moves(&program), 3);
        assert_eq!(arc_moves(&lines("G0 X1\nG1 X2 F100")), 0);
    }

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn a_half_circle_is_as_long_as_its_curve_and_not_its_chord() {
        // from (0,0) to (10,0) round the centre (5,0): half of a circle of radius 5
        let l = arc_lengths(&lines("G21 G90\nG3 X10 Y0 I5 J0"));
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].0, 1);
        assert!(near(l[0].1, 5.0 * std::f64::consts::PI), "{l:?}");
        // the other way round it is the same half
        let l = arc_lengths(&lines("G2 X10 Y0 I5 J0"));
        assert!(near(l[0].1, 5.0 * std::f64::consts::PI), "{l:?}");
    }

    #[test]
    fn a_quarter_the_long_way_and_the_radius_form_are_measured() {
        // from (0,0) to (5,5) round (0,5): counter-clockwise it is a quarter turn of radius 5 (7.854 mm), clockwise the other three
        let cw = arc_lengths(&lines("G2 X5 Y5 I0 J5"));
        let ccw = arc_lengths(&lines("G3 X5 Y5 I0 J5"));
        assert!(near(cw[0].1, 7.5 * std::f64::consts::PI), "{cw:?}");
        assert!(near(ccw[0].1, 2.5 * std::f64::consts::PI), "{ccw:?}");
        // the radius form: a positive radius is the short way and a negative one the long way
        let short = arc_lengths(&lines("G3 X10 Y0 R5"));
        assert!(near(short[0].1, 5.0 * std::f64::consts::PI));
        let small = arc_lengths(&lines("G3 X10 Y0 R10"));
        let long = arc_lengths(&lines("G3 X10 Y0 R-10"));
        let full = 2.0 * std::f64::consts::PI * 10.0;
        assert!(near(small[0].1 + long[0].1, full), "{small:?} {long:?}");
        // a radius smaller than half the chord is taken as half the chord
        let tiny = arc_lengths(&lines("G3 X10 Y0 R1"));
        assert!(near(tiny[0].1, 5.0 * std::f64::consts::PI));
    }

    #[test]
    fn a_helix_adds_its_rise_and_inches_and_the_plane_are_followed() {
        let helix = arc_lengths(&lines("G3 X10 Y0 Z5 I5 J0"));
        let flat = 5.0 * std::f64::consts::PI;
        assert!(near(helix[0].1, flat.hypot(5.0)), "{helix:?}");
        // in inches the length is in mm
        let inch = arc_lengths(&lines("G20\nG3 X1 Y0 I0.5 J0"));
        assert!(near(inch[0].1, 12.7 * std::f64::consts::PI), "{inch:?}");
        // the plane XZ (G18) takes the offsets I and K
        let xz = arc_lengths(&lines("G18\nG3 X10 Z0 I5 K0"));
        assert!(near(xz[0].1, 5.0 * std::f64::consts::PI), "{xz:?}");
        // an absolute centre (G90.1) is a point, not an offset
        let abs = arc_lengths(&lines("G90.1\nG1 X10 F100\nG3 X0 Y0 I5 J0"));
        assert!(near(abs[0].1, 5.0 * std::f64::consts::PI), "{abs:?}");
    }

    #[test]
    fn a_full_circle_that_ends_where_it_starts_has_no_move_and_an_arc_with_no_centre_has_no_length()
    {
        // the path has no segment for a move that ends where it is
        assert!(arc_lengths(&lines("G1 X10 F100\nG2 X10 Y0 I-5 J0")).is_empty());
        // no centre and no radius, or a centre on the start
        assert!(arc_lengths(&lines("G2 X10 Y0")).is_empty());
        assert!(arc_lengths(&lines("G2 X10 Y0 I0 J0")).is_empty());
        // a program with no arc has none
        assert!(arc_lengths(&lines("G0 X1\nG1 X5 F10")).is_empty());
        // the path itself did not change
        assert_eq!(toolpath(&lines("G2 X10 Y0 I5 J0")).len(), 1);
    }

    fn on_circle(pts: &[Point], c: (f64, f64), r: f64) -> bool {
        pts.iter()
            .all(|p| ((p.0 - c.0).hypot(p.1 - c.1) - r).abs() < 1e-6)
    }

    #[test]
    fn an_arc_by_its_centre_is_followed_along_its_curve_from_end_to_end() {
        // the upper half of a circle of radius 5 round (5, 0), clockwise from (0, 0) to (10, 0)
        let c = arc_curves(&lines("G2 X10 Y0 I5 J0"), 0.5);
        assert_eq!(c.len(), 1);
        let (line, pts) = &c[0];
        assert_eq!(*line, 0);
        assert_eq!(pts[0], (0.0, 0.0, 0.0));
        assert_eq!(*pts.last().unwrap(), (10.0, 0.0, 0.0));
        assert!(on_circle(pts, (5.0, 0.0), 5.0));
        assert!(
            pts.iter().all(|p| p.1 >= -1e-9),
            "clockwise goes over the top"
        );
        // no piece is longer than the step, and the half circle has about 31 mm of curve
        assert!(pts.len() > 30);
        let longest = pts
            .windows(2)
            .map(|w| (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1))
            .fold(0.0, f64::max);
        assert!(longest <= 0.5 + 1e-9, "{longest}");
        // counter-clockwise goes under
        let ccw = arc_curves(&lines("G3 X10 Y0 I5 J0"), 0.5);
        assert!(ccw[0].1.iter().all(|p| p.1 <= 1e-9));
    }

    #[test]
    fn an_arc_by_its_radius_takes_the_short_or_the_long_way_and_a_helix_rises() {
        // R positive: the short way, R negative: the long way (the same ends, the same direction)
        let short = arc_curves(&lines("G3 X10 Y0 R5"), 0.5);
        let long = arc_curves(&lines("G3 X10 Y0 R-6"), 0.5);
        assert!(on_circle(&short[0].1, (5.0, 0.0), 5.0));
        // a half circle: the centre is on the chord, under it for a counter-clockwise arc
        assert!(short[0].1.iter().all(|p| p.1 <= 1e-9));
        // the long way round a bigger circle: the centre is on the other side, below the chord, and the arc swings far under it
        let (cx, cy) = (5.0, -(36.0f64 - 25.0).sqrt());
        assert!(on_circle(&long[0].1, (cx, cy), 6.0), "{:?}", long[0].1[3]);
        assert!(long[0].1.iter().any(|p| p.1 < -5.0));
        // the rise of a helix is spread along it
        let h = arc_curves(&lines("G3 X10 Y0 Z4 I5 J0"), 0.5);
        let n = h[0].1.len();
        assert!(near(h[0].1[n / 2].2, 4.0 * (n / 2) as f64 / (n - 1) as f64));
        assert_eq!(h[0].1[n - 1].2, 4.0);
        // the plane XZ turns the circle into the other axes
        let xz = arc_curves(&lines("G18\nG3 X10 Z0 I5 K0"), 0.5);
        assert!(xz[0].1.iter().all(|p| p.1 == 0.0));
        assert!(xz[0].1.iter().any(|p| p.2.abs() > 4.0));
    }

    #[test]
    fn an_arc_that_cannot_be_worked_out_has_no_curve_and_the_step_is_kept_in_bounds() {
        assert!(arc_curves(&lines("G2 X10 Y0"), 0.5).is_empty());
        assert!(arc_curves(&lines("G2 X10 Y0 I0 J0"), 0.5).is_empty());
        assert!(arc_curves(&lines("G1 X5 F10"), 0.5).is_empty());
        // a silly step falls back to the default; a huge circle is cut in at most 720 pieces
        assert!(arc_curves(&lines("G2 X10 Y0 I5 J0"), f64::NAN)[0].1.len() > 30);
        assert!(arc_curves(&lines("G2 X10 Y0 I5 J0"), -3.0)[0].1.len() > 30);
        let big = arc_curves(&lines("G2 X2000 Y0 I1000 J0"), 0.5);
        assert_eq!(big[0].1.len(), 721);
    }

    #[test]
    fn an_arc_that_ends_off_its_circle_or_is_too_small_is_a_fault() {
        // ends 1 mm farther from the centre than it started
        let f = arc_faults(&lines("G2 X11 Y0 I5 J0"));
        assert_eq!(f.len(), 1);
        assert_eq!((f[0].0, f[0].1), (0, ArcFault::EndRadius));
        assert!(near(f[0].2, 1.0), "{f:?}");
        // a tenth of a mm on a circle of radius 5 is more than a thousandth and more than 0.005
        assert_eq!(arc_faults(&lines("G2 X10.1 Y0 I5 J0")).len(), 1);
        // the rounding of a number with three decimals is accepted
        assert!(arc_faults(&lines("G2 X10.003 Y0 I5 J0")).is_empty());
        // a bigger circle accepts a bigger gap, up to its thousandth: 0.04 of 100 mm, but not 0.2
        assert!(arc_faults(&lines("G2 X200.04 Y0 I100 J0")).is_empty());
        assert_eq!(arc_faults(&lines("G2 X200.2 Y0 I100 J0")).len(), 1);
        // a radius smaller than half the chord
        let r = arc_faults(&lines("G1 X1 F10\nG3 X11 Y0 R4"));
        assert_eq!((r[0].0, r[0].1), (1, ArcFault::RadiusTooSmall));
        assert!(near(r[0].2, 1.0), "{r:?}");
        // a radius of exactly half the chord is a half circle and is fine
        assert!(arc_faults(&lines("G3 X10 Y0 R5")).is_empty());
        // a good arc, a program with none and an arc with no centre say nothing
        assert!(arc_faults(&lines("G2 X10 Y0 I5 J0")).is_empty());
        assert!(arc_faults(&lines("G1 X5 F10")).is_empty());
        assert!(arc_faults(&lines("G2 X10 Y0")).is_empty());
        // an absolute centre is measured from the point, in the plane given
        assert!(arc_faults(&lines("G90.1\nG1 X10 F100\nG3 X0 Y0 I5 J0")).is_empty());
        assert_eq!(
            arc_faults(&lines("G90.1\nG1 X10 F100\nG3 X0 Y1 I5 J0")).len(),
            1
        );
    }

    #[test]
    fn the_gap_that_counts_can_be_made_stricter_or_more_forgiving() {
        // 0.02 mm off on a circle of radius 5: a fault at the usual gap, fine with a gap of 0.05
        let p = lines("G2 X10.02 Y0 I5 J0");
        assert_eq!(arc_faults(&p).len(), 1);
        assert_eq!(arc_faults_within(&p, ARC_GAP_MM), arc_faults(&p));
        assert!(arc_faults_within(&p, 0.05).is_empty());
        // on a big circle the thousandth of its radius still saves a small gap, whatever the setting
        assert!(arc_faults_within(&lines("G2 X200.04 Y0 I100 J0"), 0.0).is_empty());
        // a stricter gap catches what the usual one lets through
        let tight = lines("G2 X2.003 Y0 I1 J0");
        assert!(arc_faults(&tight).is_empty());
        assert_eq!(arc_faults_within(&tight, 0.0005).len(), 1);
        // nothing above 0.5 mm is ever accepted, and a number that is not a number falls back to the usual gap
        assert_eq!(arc_faults_within(&lines("G2 X10.6 Y0 I5 J0"), 5.0).len(), 1);
        assert_eq!(arc_faults_within(&p, f64::NAN).len(), 1);
        // the radius of an arc given by R does not depend on the gap
        assert_eq!(arc_faults_within(&lines("G3 X11 Y0 R4"), 0.5).len(), 1);
    }

    #[test]
    fn the_words_are_read_with_no_spaces_signs_and_forms_of_numbers() {
        assert_eq!(words("G1X5Y-3"), vec![('G', 1.0), ('X', 5.0), ('Y', -3.0)]);
        assert_eq!(
            words("X.5 Y+2 Z-.25"),
            vec![('X', 0.5), ('Y', 2.0), ('Z', -0.25)]
        );
        assert_eq!(
            words("n10 g0 x1"),
            vec![('N', 10.0), ('G', 0.0), ('X', 1.0)]
        );
        // a letter with no number, a program marker and an empty line have no word
        assert!(words("X Y Z").is_empty());
        assert!(words("%").is_empty());
        assert!(words("").is_empty());
        assert!(words("   \t ").is_empty());
        // a comment in brackets that is never closed runs to the end of the line, and one that is closed lets the line go on
        assert_eq!(words("G1 X5 (open Y6"), vec![('G', 1.0), ('X', 5.0)]);
        assert_eq!(
            words("G1 X5 (a) Y6 (b) Z7"),
            vec![('G', 1.0), ('X', 5.0), ('Y', 6.0), ('Z', 7.0)]
        );
        // a semicolon cuts the line even inside a bracket
        assert_eq!(words("G1 (a ; b) X5"), vec![('G', 1.0)]);
    }

    #[test]
    fn a_number_that_is_not_one_is_left_out_and_the_next_word_still_reads() {
        // a double sign or two dots make no number; the words around them are kept
        assert_eq!(words("X--5 Y3"), vec![('Y', 3.0)]);
        assert_eq!(words("X1.2.3 Y3"), vec![('Y', 3.0)]);
        // a letter inside a number starts a word of its own
        assert_eq!(words("X1e3"), vec![('X', 1.0), ('E', 3.0)]);
    }

    #[test]
    fn the_arcs_that_move_are_counted_even_when_the_g_code_is_in_force_from_before() {
        assert_eq!(arc_moves(&lines("G2 X1\nG3 X2\nG1 X3\nG0 X4")), 2);
        // G2 alone moves nothing, but the next line with an axis is an arc by the code in force
        assert_eq!(arc_moves(&lines("G2\nX5 I1")), 1);
        // a centre alone is a move of the arc too, and a line with no axis, centre or radius is not
        assert_eq!(arc_moves(&lines("G3 I1 J0")), 1);
        assert_eq!(arc_moves(&lines("G2\nF100\nM3")), 0);
        assert_eq!(arc_moves(&[]), 0);
        // a straight move after the arcs ends them
        assert_eq!(arc_moves(&lines("G2 X1\nG1 X2\nX3")), 1);
    }

    #[test]
    fn the_feed_is_read_in_the_units_in_force_and_kept_in_mm_per_minute() {
        let p = toolpath(&lines("G20\nG1 X1 F10\nG21\nG1 X2 F10\nG1 X3\nG20\nG1 X4"));
        // F10 in inches is 254 mm/min; a feed is kept in mm/min once it is read, so a change of units later does not change it
        assert_eq!(p[0].feed, 254.0);
        assert_eq!(p[1].feed, 10.0);
        assert_eq!(p[2].feed, 10.0);
        assert_eq!(p[3].feed, 10.0);
    }
}
