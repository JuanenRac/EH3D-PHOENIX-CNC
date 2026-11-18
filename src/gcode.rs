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
    trace(lines).1
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
fn trace(lines: &[String]) -> (Vec<Segment>, Vec<(usize, f64)>) {
    let mut out = Vec::new();
    let mut arcs: Vec<(usize, f64)> = Vec::new();
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
            let (letter, num) = w.split_at(1);
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
                    if let Some(l) = arc_length(pos, to, plane, cw, off, radius, centre_absolute) {
                        arcs.push((n, l));
                    }
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
}
