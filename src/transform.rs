//! Transforming a G-code program: shift it, rotate it, scale it and mirror it, as the program menu of a machine-control program does, so a
//! job can be moved to where the stock is without going back to the CAM tool. The lines are rewritten in place (a word that is not changed,
//! the comments and the spacing stay as they were), arcs keep their centres and their direction (a mirror turns a clockwise arc into a
//! counter-clockwise one), and a program that the change could make wrong is refused, saying the line: relative distances, planes other
//! than XY, canned cycles and probing moves, and codes that take machine positions or shift the zero. Nothing here moves a machine.

use crate::heightmap::num;

/// The change to make. The order is: mirror, scale and rotate about the centre, then shift.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    /// The shift in X, Y and Z, in the units of the program.
    pub shift: [f64; 3],
    /// The turn about the centre, in degrees, counter-clockwise seen from above.
    pub rotate_deg: f64,
    /// The point (X, Y) the turn, the scale and the mirror are about.
    pub centre: (f64, f64),
    /// The scale of all three axes (above 0; to turn a shape over use the mirror).
    pub scale: f64,
    /// X changes sign about the centre / Y changes sign about the centre.
    pub mirror_x: bool,
    pub mirror_y: bool,
}

impl Default for Transform {
    fn default() -> Self {
        Transform {
            shift: [0.0; 3],
            rotate_deg: 0.0,
            centre: (0.0, 0.0),
            scale: 1.0,
            mirror_x: false,
            mirror_y: false,
        }
    }
}

/// Why a program was not transformed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Why {
    /// The scale is not a number above 0.
    BadScale,
    /// Relative distances (`G91`).
    Relative,
    /// A plane other than XY (`G18`, `G19`).
    Plane,
    /// A canned cycle (`G81` to `G89`) or a probing move (`G38.x`).
    Cycle,
    /// A code that takes machine positions or shifts the zero (`G10`, `G28`, `G30`, `G53`, `G92`, `G43.1`).
    Zero,
    /// Absolute centres of the arcs (`G90.1`).
    AbsoluteCentre,
}

/// A refusal: the reason and the line (from 1; 0 when it is not about a line).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Refusal {
    pub why: Why,
    pub line: usize,
}

/// One word of a line: its letter, the bytes of its number in the line and its value.
#[derive(Debug, Clone, Copy)]
struct Word {
    letter: char,
    /// Where the letter is.
    at: usize,
    start: usize,
    end: usize,
    value: f64,
}

/// Puts a new value for a word in the list of edits, unless it is the value the word has (then the text stays as it was written).
fn put(edits: &mut Vec<(usize, usize, String)>, w: &Word, new: f64) {
    if (w.value - new).abs() >= 1e-9 {
        edits.push((w.start, w.end, num(new)));
    }
}

/// The words of a line with the place of their numbers; comments (in brackets or after a `;`) have none.
fn scan(line: &str) -> Vec<Word> {
    let b = line.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b';' {
            break;
        }
        if c == b'(' {
            while i < b.len() && b[i] != b')' {
                i += 1;
            }
            i += 1;
            continue;
        }
        if c.is_ascii_alphabetic() {
            let letter = c.to_ascii_uppercase() as char;
            let mut j = i + 1;
            while j < b.len() && (b[j] == b' ' || b[j] == b'\t') {
                j += 1;
            }
            let start = j;
            if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
                j += 1;
            }
            let digits = j;
            while j < b.len() && (b[j].is_ascii_digit() || b[j] == b'.') {
                j += 1;
            }
            if j > digits {
                if let Ok(value) = line[start..j].parse::<f64>() {
                    out.push(Word {
                        letter,
                        at: i,
                        start,
                        end: j,
                        value,
                    });
                    i = j;
                    continue;
                }
            }
            i += 1;
            continue;
        }
        i += 1;
    }
    out
}

impl Transform {
    /// Whether the change changes nothing.
    pub fn is_identity(&self) -> bool {
        *self == Transform::default()
    }

    fn valid(&self) -> bool {
        self.scale.is_finite()
            && self.scale > 0.0
            && self.scale <= 1.0e6
            && self.shift.iter().all(|v| v.is_finite())
            && self.rotate_deg.is_finite()
            && self.centre.0.is_finite()
            && self.centre.1.is_finite()
    }

    /// Where a point goes (X and Y; the centre is the fixed point of the mirror, the scale and the turn).
    pub fn point(&self, x: f64, y: f64) -> (f64, f64) {
        let (dx, dy) = self.vector(x - self.centre.0, y - self.centre.1);
        (
            self.centre.0 + dx + self.shift[0],
            self.centre.1 + dy + self.shift[1],
        )
    }

    /// Where a distance (X, Y) goes: the same but for the centre and the shift.
    pub fn vector(&self, dx: f64, dy: f64) -> (f64, f64) {
        let (mut x, mut y) = (dx, dy);
        if self.mirror_x {
            x = -x;
        }
        if self.mirror_y {
            y = -y;
        }
        x *= self.scale;
        y *= self.scale;
        let (s, c) = self.rotate_deg.to_radians().sin_cos();
        (x * c - y * s, x * s + y * c)
    }

    /// Whether the direction of the arcs is turned over (one mirror, not two).
    fn flips(&self) -> bool {
        self.mirror_x != self.mirror_y
    }

    /// Transforms the program. A program that cannot be transformed right is refused with the line; with no change the lines come back as
    /// they were.
    pub fn apply(&self, lines: &[String]) -> Result<Vec<String>, Refusal> {
        if !self.valid() {
            return Err(Refusal {
                why: Why::BadScale,
                line: 0,
            });
        }
        if self.is_identity() {
            return Ok(lines.to_vec());
        }
        let turns = self.rotate_deg.rem_euclid(360.0) != 0.0;
        let mut out = Vec::with_capacity(lines.len());
        // the position in the program's own coordinates (the program starts at the origin, as the path of the window assumes)
        let (mut px, mut py) = (0.0_f64, 0.0_f64);
        let mut motion: Option<u8> = None;
        for (n, raw) in lines.iter().enumerate() {
            let refuse = |why| Refusal { why, line: n + 1 };
            let ws = scan(raw);
            let mut edits: Vec<(usize, usize, String)> = Vec::new();
            let mut explicit_arc: Option<&Word> = None;
            let mut line_motion = motion;
            for w in &ws {
                if w.letter != 'G' {
                    continue;
                }
                match (w.value * 10.0).round() as i32 {
                    0 => line_motion = Some(0),
                    10 => line_motion = Some(1),
                    20 => {
                        line_motion = Some(2);
                        explicit_arc = Some(w);
                    }
                    30 => {
                        line_motion = Some(3);
                        explicit_arc = Some(w);
                    }
                    180 | 190 => return Err(refuse(Why::Plane)),
                    910 => return Err(refuse(Why::Relative)),
                    901 => return Err(refuse(Why::AbsoluteCentre)),
                    380..=385 | 810..=890 => return Err(refuse(Why::Cycle)),
                    100 | 280 | 281 | 300 | 301 | 530 | 431 | 920..=923 => {
                        return Err(refuse(Why::Zero))
                    }
                    _ => {}
                }
            }
            motion = line_motion;
            let get = |l: char| ws.iter().find(|w| w.letter == l);
            let (wx, wy, wz) = (get('X'), get('Y'), get('Z'));
            let is_arc = matches!(line_motion, Some(2) | Some(3));
            if wx.is_some() || wy.is_some() {
                let (ox, oy) = (wx.map_or(px, |w| w.value), wy.map_or(py, |w| w.value));
                let (nx, ny) = self.point(ox, oy);
                // a turn changes both coordinates, so a missing one is written
                let both = turns;
                match (wx, wy) {
                    (Some(a), Some(b)) => {
                        put(&mut edits, a, nx);
                        put(&mut edits, b, ny);
                    }
                    (Some(a), None) => {
                        put(&mut edits, a, nx);
                        if both {
                            edits.push((a.end, a.end, format!(" Y{}", num(ny))));
                        }
                    }
                    (None, Some(b)) => {
                        put(&mut edits, b, ny);
                        if both {
                            // in front of the letter of the word
                            edits.push((b.at, b.at, format!("X{} ", num(nx))));
                        }
                    }
                    (None, None) => {}
                }
                px = ox;
                py = oy;
            }
            if let Some(w) = wz {
                put(&mut edits, w, w.value * self.scale + self.shift[2]);
            }
            if is_arc {
                let (wi, wj, wk, wr) = (get('I'), get('J'), get('K'), get('R'));
                if wi.is_some() || wj.is_some() {
                    let (i, j) = (wi.map_or(0.0, |w| w.value), wj.map_or(0.0, |w| w.value));
                    let (ni, nj) = self.vector(i, j);
                    match (wi, wj) {
                        (Some(a), Some(b)) => {
                            put(&mut edits, a, ni);
                            put(&mut edits, b, nj);
                        }
                        (Some(a), None) => {
                            put(&mut edits, a, ni);
                            if turns {
                                edits.push((a.end, a.end, format!(" J{}", num(nj))));
                            }
                        }
                        (None, Some(b)) => {
                            put(&mut edits, b, nj);
                            if turns {
                                edits.push((b.at, b.at, format!("I{} ", num(ni))));
                            }
                        }
                        (None, None) => {}
                    }
                }
                if let Some(w) = wk {
                    put(&mut edits, w, w.value * self.scale);
                }
                if let Some(w) = wr {
                    put(&mut edits, w, w.value * self.scale);
                }
                if self.flips() {
                    let now = if line_motion == Some(2) { "3" } else { "2" };
                    match explicit_arc {
                        Some(g) => edits.push((g.start, g.end, now.to_string())),
                        None => {
                            // the arc keeps the code of an earlier line: it is written here
                            let first = ws
                                .iter()
                                .filter(|w| "XYZIJKR".contains(w.letter))
                                .map(|w| w.at)
                                .min()
                                .unwrap_or(0);
                            edits.push((first, first, format!("G{now} ")));
                        }
                    }
                }
            }
            if edits.is_empty() {
                out.push(raw.clone());
                continue;
            }
            // from the end, so that the places of the first ones stay right
            edits.sort_by_key(|e| std::cmp::Reverse((e.0, e.1)));
            let mut line = raw.clone();
            for (s, e, text) in edits {
                line.replace_range(s..e, &text);
            }
            out.push(line);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gcode::toolpath;

    fn lines(s: &str) -> Vec<String> {
        s.lines().map(str::to_string).collect()
    }

    fn run(t: &Transform, s: &str) -> Vec<String> {
        t.apply(&lines(s)).expect("transformed")
    }

    fn refused(t: &Transform, s: &str) -> Refusal {
        t.apply(&lines(s)).expect_err("refused")
    }

    fn shifted(x: f64, y: f64, z: f64) -> Transform {
        Transform {
            shift: [x, y, z],
            ..Transform::default()
        }
    }

    #[test]
    fn a_shift_moves_every_point_and_leaves_the_rest_of_the_line_alone() {
        let t = shifted(5.0, -2.0, 1.0);
        let out = run(
            &t,
            "G21 G90\nM3 S1000\nG0 X10 Y5\nG1 Z-1 F100 (plunge X99)\nG1 X20 ; to the end Y7\nM30",
        );
        assert_eq!(
            out,
            [
                "G21 G90",
                "M3 S1000",
                "G0 X15 Y3",
                "G1 Z0 F100 (plunge X99)",
                "G1 X25 ; to the end Y7",
                "M30"
            ]
        );
        // a shift of nothing is the program as it was, character for character
        let same = "G1  X1.50   Y2 F10 (a)";
        assert_eq!(run(&Transform::default(), same), [same]);
        assert!(Transform::default().is_identity() && !t.is_identity());
    }

    #[test]
    fn the_scale_works_on_the_three_axes_and_the_arcs_but_not_on_the_feed() {
        let t = Transform {
            scale: 2.0,
            ..Transform::default()
        };
        let out = run(&t, "G1 X10 Y5 Z-1 F300\nG2 X20 Y5 I5 J0\nG2 X0 Y0 R10");
        assert_eq!(out[0], "G1 X20 Y10 Z-2 F300");
        assert_eq!(out[1], "G2 X40 Y10 I10 J0");
        assert_eq!(out[2], "G2 X0 Y0 R20");
    }

    #[test]
    fn a_mirror_turns_the_arcs_over_and_two_mirrors_do_not() {
        let x = Transform {
            mirror_x: true,
            ..Transform::default()
        };
        let out = run(&x, "G1 X10 Y0\nG2 X20 Y0 I5 J0\nG3 X10 Y0 I-5 J0");
        assert_eq!(out, ["G1 X-10 Y0", "G3 X-20 Y0 I-5 J0", "G2 X-10 Y0 I5 J0"]);
        let y = Transform {
            mirror_y: true,
            ..Transform::default()
        };
        assert_eq!(
            run(&y, "G1 X10 Y4\nG2 X10 Y8 I0 J2")[1],
            "G3 X10 Y-8 I0 J-2"
        );
        let both = Transform {
            mirror_x: true,
            mirror_y: true,
            ..Transform::default()
        };
        assert_eq!(
            run(&both, "G1 X10 Y4\nG2 X10 Y8 I0 J2")[1],
            "G2 X-10 Y-8 I0 J-2"
        );
        // an arc that takes its code from an earlier line gets it written
        let out = run(&x, "G2\nX2 Y0 I1 J0\nX4 Y0 I1 J0");
        assert_eq!(out, ["G3", "G3 X-2 Y0 I-1 J0", "G3 X-4 Y0 I-1 J0"]);
        // the mirror is about the centre
        let c = Transform {
            mirror_x: true,
            centre: (10.0, 0.0),
            ..Transform::default()
        };
        assert_eq!(run(&c, "G1 X12 Y1"), ["G1 X8 Y1"]);
    }

    #[test]
    fn a_turn_writes_both_coordinates_even_when_the_line_has_one() {
        let t = Transform {
            rotate_deg: 90.0,
            ..Transform::default()
        };
        let out = run(&t, "G1 X10 Y5 F100\nG1 X20\nG1 Y3\nG1 Z-1");
        assert_eq!(out[0], "G1 X-5 Y10 F100");
        // (20, 5) turned a quarter is (-5, 20)
        assert_eq!(out[1], "G1 X-5 Y20");
        // (20, 3) turned is (-3, 20); the X comes before the Y
        assert_eq!(out[2], "G1 X-3 Y20");
        assert_eq!(out[3], "G1 Z-1");
        // an arc's centre turns with it
        let arc = run(&t, "G1 X10 Y0\nG3 X0 Y10 I-10 J0");
        assert_eq!(arc, ["G1 X0 Y10", "G3 X-10 Y0 I0 J-10"]);
        // a turn about a centre and a shift
        let c = Transform {
            rotate_deg: 180.0,
            centre: (10.0, 10.0),
            shift: [1.0, 2.0, 0.0],
            ..Transform::default()
        };
        assert_eq!(run(&c, "G1 X12 Y11"), ["G1 X9 Y11"]);
    }

    #[test]
    fn the_path_of_the_new_program_is_the_old_path_transformed() {
        let src = "G21 G90\nG0 X10 Y0 Z0\nG1 Z-1 F100\nG1 X30 Y10\nG2 X40 Y20 I10 J0\nG3 X30 Y30 R10\nG1 Y40\nG0 Z5";
        let t = Transform {
            shift: [3.0, -4.0, 2.0],
            rotate_deg: 37.0,
            centre: (5.0, 6.0),
            scale: 1.5,
            mirror_y: true,
            ..Transform::default()
        };
        let before = toolpath(&lines(src));
        let after = toolpath(&run(&t, src));
        assert_eq!(before.len(), after.len());
        for (i, (a, b)) in before.iter().zip(&after).enumerate() {
            assert_eq!((a.rapid, a.arc, a.line), (b.rapid, b.arc, b.line));
            // the program starts at the origin of the window whatever the change, so the start of the first move is not compared
            let ends = if i == 0 {
                vec![(a.to, b.to)]
            } else {
                vec![(a.from, b.from), (a.to, b.to)]
            };
            for (p, q) in ends {
                let (x, y) = t.point(p.0, p.1);
                assert!(
                    (q.0 - x).abs() < 1e-3 && (q.1 - y).abs() < 1e-3,
                    "{p:?} {q:?}"
                );
                assert!((q.2 - (p.2 * 1.5 + 2.0)).abs() < 1e-3, "{p:?} {q:?}");
            }
        }
    }

    #[test]
    fn what_the_change_could_make_wrong_is_refused_with_its_line() {
        let t = shifted(1.0, 1.0, 0.0);
        for (src, why, line) in [
            ("G21\nG91\nG1 X1", Why::Relative, 2),
            ("G1 X1\nG18\nG2 X2 Z0 I1 K0", Why::Plane, 2),
            ("G19", Why::Plane, 1),
            ("G1 X1\nG81 X5 Y5 Z-3 R2", Why::Cycle, 2),
            ("G38.2 Z-10 F20", Why::Cycle, 1),
            ("G1 X1\nG1 X2\nG92 X0 Y0", Why::Zero, 3),
            ("G28", Why::Zero, 1),
            ("G53 G0 X0", Why::Zero, 1),
            ("G10 L2 P1 X0", Why::Zero, 1),
            ("G90.1\nG2 X1 Y1 I0 J1", Why::AbsoluteCentre, 1),
        ] {
            assert_eq!(refused(&t, src), Refusal { why, line }, "{src}");
        }
        // the relative arc centres (G91.1) and the work systems are fine
        assert!(t.apply(&lines("G91.1\nG54\nG2 X1 Y1 I0 J1")).is_ok());
        // a scale that is not a number above 0
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY, 1.0e9] {
            let b = Transform {
                scale: bad,
                shift: [1.0, 0.0, 0.0],
                ..Transform::default()
            };
            assert_eq!(
                refused(&b, "G1 X1"),
                Refusal {
                    why: Why::BadScale,
                    line: 0
                }
            );
        }
        let nan = Transform {
            rotate_deg: f64::NAN,
            ..Transform::default()
        };
        assert!(nan.apply(&lines("G1 X1")).is_err());
        // a program that is refused is not changed (the caller keeps the one it has)
        let p = lines("G1 X1\nG91\nG1 X1");
        assert!(t.apply(&p).is_err());
        assert_eq!(p.len(), 3);
    }

    #[test]
    fn words_in_comments_and_odd_spacing_are_read_right() {
        let t = shifted(1.0, 0.0, 0.0);
        let out = run(
            &t,
            "g1 x 10 y-5.5 (move X3) ; X4\nG1 X-.5\n(X1 Y1)\nN10 G0 X1.\n%",
        );
        assert_eq!(out[0], "g1 x 11 y-5.5 (move X3) ; X4");
        assert_eq!(out[1], "G1 X0.5");
        assert_eq!(out[2], "(X1 Y1)");
        assert_eq!(out[3], "N10 G0 X2");
        assert_eq!(out[4], "%");
        // an empty program and an empty line
        assert!(t.apply(&[]).unwrap().is_empty());
        assert_eq!(run(&t, "\n\nG1 X1"), ["", "", "G1 X2"]);
    }

    #[test]
    fn a_turn_and_its_opposite_bring_the_program_back() {
        let src = "G1 X10 Y5 F100\nG1 X20\nG3 X10 Y15 I-10 J0\nG1 Y2";
        let t = Transform {
            rotate_deg: 33.0,
            centre: (4.0, 2.0),
            shift: [7.0, -3.0, 0.0],
            ..Transform::default()
        };
        // the opposite: turn back about the shifted centre
        let back = Transform {
            rotate_deg: -33.0,
            centre: (4.0 + 7.0, 2.0 - 3.0),
            shift: [-7.0, 3.0, 0.0],
            ..Transform::default()
        };
        let there = run(&t, src);
        let again = run(&back, &there.join("\n"));
        let (a, b) = (toolpath(&lines(src)), toolpath(&again));
        assert_eq!(a.len(), b.len());
        for (p, q) in a.iter().zip(&b) {
            assert!(
                (p.to.0 - q.to.0).abs() < 1e-2 && (p.to.1 - q.to.1).abs() < 1e-2,
                "{p:?} {q:?}"
            );
        }
    }
}
