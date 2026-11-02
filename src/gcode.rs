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
    let mut out = Vec::new();
    let (mut pos, mut absolute, mut scale, mut rapid) = ((0.0, 0.0, 0.0), true, 1.0, true);
    let (mut feed, mut arc) = (0.0, false);
    for (n, raw) in lines.iter().enumerate() {
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
                    20 | 30 => (rapid, arc) = (false, true),
                    900 => absolute = true,
                    910 => absolute = false,
                    200 => scale = 25.4,
                    210 => scale = 1.0,
                    _ => {}
                },
                "X" => target[0] = Some(v * scale),
                "Y" => target[1] = Some(v * scale),
                "Z" => target[2] = Some(v * scale),
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
    out
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
        }
    }
}

impl Modal {
    /// Takes a line that the controller accepted.
    pub fn apply(&mut self, line: &str) {
        for (l, v) in words(line) {
            // a G code is read in tenths (G91.1 is 911, not G91); an M code is a whole number
            let tenths = (v * 10.0).round() as i32;
            match (l, v as i32) {
                ('G', _) if tenths == 0 => self.motion = 0,
                ('G', _) if tenths == 10 => self.motion = 1,
                ('G', _) if tenths == 200 => self.inches = true,
                ('G', _) if tenths == 210 => self.inches = false,
                ('G', _) if tenths == 900 => self.absolute = true,
                ('G', _) if tenths == 910 => self.absolute = false,
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
            g("G17"),
            g(if self.inches { "G20" } else { "G21" }),
            g("G40"),
            g("G49"),
            g("G54"),
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
}
