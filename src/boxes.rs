//! Keep-out boxes and travel limits for the path of a program: the places of the table where the tool must not go (a clamp, a vise, a
//! fixture) are boxes written in the settings file, and every straight move of the program is tested against them and against the travel
//! of each axis, so the line that goes wrong is named before anything moves. It knows nothing of the window or of the controller.

use crate::gcode::Segment;

/// A box of the machine where the tool must not enter, in mm of machine coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct KeepOut {
    pub name: String,
    pub min: [f64; 3],
    pub max: [f64; 3],
}

impl KeepOut {
    /// Reads `name x0 y0 z0 x1 y1 z1`; the corners may be given in any order. Nothing when the name is missing, a number is not finite
    /// or the box is flat in some axis (a box with no volume cannot be entered).
    pub fn parse(text: &str) -> Option<KeepOut> {
        let mut parts = text.split_whitespace();
        let name = parts.next()?.to_string();
        if name.len() > 32
            || !name
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return None;
        }
        let mut n = [0.0f64; 6];
        for slot in &mut n {
            *slot = parts
                .next()?
                .replace(',', ".")
                .parse::<f64>()
                .ok()
                .filter(|x| x.is_finite())?;
        }
        if parts.next().is_some() {
            return None;
        }
        let min = [n[0].min(n[3]), n[1].min(n[4]), n[2].min(n[5])];
        let max = [n[0].max(n[3]), n[1].max(n[4]), n[2].max(n[5])];
        if (0..3).any(|i| max[i] - min[i] <= 0.0) {
            return None;
        }
        Some(KeepOut { name, min, max })
    }

    /// The line of the settings file that gives this box back.
    pub fn to_setting(&self) -> String {
        format!(
            "keepout = {} {} {} {} {} {} {}",
            self.name, self.min[0], self.min[1], self.min[2], self.max[0], self.max[1], self.max[2]
        )
    }

    /// The same box grown by `margin` mm on every side.
    pub fn grown(&self, margin: f64) -> KeepOut {
        let m = margin.max(0.0);
        KeepOut {
            name: self.name.clone(),
            min: [self.min[0] - m, self.min[1] - m, self.min[2] - m],
            max: [self.max[0] + m, self.max[1] + m, self.max[2] + m],
        }
    }

    /// Whether the straight line from `a` to `b` goes through the box (touching it counts).
    pub fn crossed_by(&self, a: [f64; 3], b: [f64; 3]) -> bool {
        let (mut t0, mut t1) = (0.0f64, 1.0f64);
        for i in 0..3 {
            let d = b[i] - a[i];
            if d.abs() < 1e-12 {
                if a[i] < self.min[i] || a[i] > self.max[i] {
                    return false;
                }
            } else {
                let (mut u, mut v) = ((self.min[i] - a[i]) / d, (self.max[i] - a[i]) / d);
                if u > v {
                    std::mem::swap(&mut u, &mut v);
                }
                t0 = t0.max(u);
                t1 = t1.min(v);
                if t0 > t1 {
                    return false;
                }
            }
        }
        true
    }
}

/// A move of the program that enters a box.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    /// The line of the program (from 0).
    pub line: usize,
    pub name: String,
    /// The move is an arc and only its chord was tested: the arc itself may go farther.
    pub chord_only: bool,
}

/// The moves of the path that enter a box. `offset` is where the zero of the work is in the machine (machine position minus work
/// position); `margin` makes every box bigger by that much. A line is reported once for each box.
pub fn hits(segs: &[Segment], offset: [f64; 3], boxes: &[KeepOut], margin: f64) -> Vec<Hit> {
    let grown: Vec<KeepOut> = boxes.iter().map(|b| b.grown(margin)).collect();
    let mut out: Vec<Hit> = Vec::new();
    for s in segs {
        let a = [
            s.from.0 + offset[0],
            s.from.1 + offset[1],
            s.from.2 + offset[2],
        ];
        let b = [s.to.0 + offset[0], s.to.1 + offset[1], s.to.2 + offset[2]];
        for g in &grown {
            if g.crossed_by(a, b) && !out.iter().any(|h| h.line == s.line && h.name == g.name) {
                out.push(Hit {
                    line: s.line,
                    name: g.name.clone(),
                    chord_only: s.arc,
                });
            }
        }
    }
    out
}

/// How many moves are arcs, which are only tested by their chord.
pub fn arc_chords(segs: &[Segment]) -> usize {
    segs.iter().filter(|s| s.arc).count()
}

/// A move that leaves the travel of an axis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Excess {
    pub line: usize,
    /// `X`, `Y` or `Z`.
    pub axis: char,
}

/// The first move of each line that leaves the travel of each axis. `travel` is (minimum, maximum) in mm of the machine for X, Y and Z.
pub fn travel_excess(segs: &[Segment], offset: [f64; 3], travel: [(f64, f64); 3]) -> Vec<Excess> {
    let mut out: Vec<Excess> = Vec::new();
    for s in segs {
        let to = [s.to.0 + offset[0], s.to.1 + offset[1], s.to.2 + offset[2]];
        for (i, axis) in ['X', 'Y', 'Z'].into_iter().enumerate() {
            let (lo, hi) = travel[i];
            if (to[i] < lo - 1e-9 || to[i] > hi + 1e-9)
                && !out.iter().any(|e| e.line == s.line && e.axis == axis)
            {
                out.push(Excess { line: s.line, axis });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(from: (f64, f64, f64), to: (f64, f64, f64), line: usize, arc: bool) -> Segment {
        Segment {
            from,
            to,
            rapid: false,
            line,
            feed: 100.0,
            arc,
        }
    }

    fn clamp() -> KeepOut {
        KeepOut::parse("clamp 10 10 -5 20 20 30").unwrap()
    }

    #[test]
    fn a_box_is_read_with_its_corners_in_any_order() {
        let b = KeepOut::parse("vise 20 20 30 10 10 -5").unwrap();
        assert_eq!(b.min, [10.0, 10.0, -5.0]);
        assert_eq!(b.max, [20.0, 20.0, 30.0]);
        assert_eq!(KeepOut::parse("a 1,5 0 0 3 2 2").unwrap().min[0], 1.5);
    }

    #[test]
    fn a_wrong_box_is_refused() {
        for bad in [
            "",
            "clamp",
            "clamp 1 2 3 4 5",
            "clamp 1 2 3 4 5 6 7",
            "clamp 1 2 3 4 5 x",
            "clamp 1 2 3 1 5 6",
            "bad/name 1 2 3 4 5 6",
            "clamp 1 2 3 4 5 NaN",
            "clamp 1 2 3 4 5 inf",
        ] {
            assert!(KeepOut::parse(bad).is_none(), "{bad}");
        }
        let long = format!("{} 0 0 0 1 1 1", "a".repeat(33));
        assert!(KeepOut::parse(&long).is_none());
    }

    #[test]
    fn a_box_is_written_back_as_it_was_read() {
        let b = clamp();
        assert_eq!(
            KeepOut::parse(b.to_setting().trim_start_matches("keepout = ")),
            Some(b)
        );
    }

    #[test]
    fn a_move_through_the_box_is_found_and_one_beside_it_is_not() {
        let b = clamp();
        assert!(b.crossed_by([0.0, 15.0, 0.0], [30.0, 15.0, 0.0]));
        assert!(!b.crossed_by([0.0, 5.0, 0.0], [30.0, 5.0, 0.0]));
        // above the box
        assert!(!b.crossed_by([0.0, 15.0, 40.0], [30.0, 15.0, 40.0]));
        // a plunge from above into the box
        assert!(b.crossed_by([15.0, 15.0, 50.0], [15.0, 15.0, 0.0]));
        // a move that stops short
        assert!(!b.crossed_by([0.0, 15.0, 0.0], [9.0, 15.0, 0.0]));
        // a point inside
        assert!(b.crossed_by([15.0, 15.0, 0.0], [15.0, 15.0, 0.0]));
    }

    #[test]
    fn touching_the_box_counts_and_a_margin_makes_it_bigger() {
        let b = clamp();
        assert!(b.crossed_by([0.0, 15.0, 0.0], [10.0, 15.0, 0.0]));
        let s = [seg((0.0, 15.0, 0.0), (8.0, 15.0, 0.0), 3, false)];
        assert!(hits(&s, [0.0; 3], std::slice::from_ref(&b), 0.0).is_empty());
        assert_eq!(hits(&s, [0.0; 3], &[b], 2.5).len(), 1);
    }

    #[test]
    fn the_zero_of_the_work_moves_the_path_into_the_machine() {
        let s = [seg((0.0, 0.0, 0.0), (5.0, 0.0, 0.0), 1, false)];
        assert!(hits(&s, [0.0; 3], &[clamp()], 0.0).is_empty());
        // with the work zero at 12, 15, 0 the move runs from 12 to 17 in X at Y 15: inside the clamp
        assert_eq!(hits(&s, [12.0, 15.0, 0.0], &[clamp()], 0.0).len(), 1);
    }

    #[test]
    fn a_line_is_reported_once_per_box_and_arcs_are_marked() {
        let s = [
            seg((0.0, 15.0, 0.0), (30.0, 15.0, 0.0), 4, false),
            seg((30.0, 15.0, 0.0), (0.0, 15.0, 0.0), 4, false),
            seg((0.0, 12.0, 0.0), (30.0, 12.0, 0.0), 5, true),
        ];
        let h = hits(&s, [0.0; 3], &[clamp()], 0.0);
        assert_eq!(h.len(), 2);
        assert!(!h[0].chord_only);
        assert_eq!(h[1].line, 5);
        assert!(h[1].chord_only);
        assert_eq!(arc_chords(&s), 1);
    }

    #[test]
    fn travel_excess_names_the_line_and_the_axis_once() {
        let s = [
            seg((0.0, 0.0, 0.0), (100.0, 0.0, 0.0), 0, false),
            seg((0.0, 0.0, 0.0), (400.0, 0.0, 0.0), 1, false),
            seg((0.0, 0.0, 0.0), (400.0, -5.0, 0.0), 2, false),
            seg((0.0, 0.0, 0.0), (400.0, 10.0, 0.0), 2, false),
        ];
        let e = travel_excess(&s, [0.0; 3], [(0.0, 300.0), (0.0, 200.0), (-50.0, 0.0)]);
        assert_eq!(
            e,
            vec![
                Excess { line: 1, axis: 'X' },
                Excess { line: 2, axis: 'X' },
                Excess { line: 2, axis: 'Y' },
            ]
        );
    }

    #[test]
    fn the_ends_of_the_travel_are_inside_it() {
        let s = [seg((0.0, 0.0, 0.0), (300.0, 200.0, -50.0), 0, false)];
        assert!(travel_excess(&s, [0.0; 3], [(0.0, 300.0), (0.0, 200.0), (-50.0, 0.0)]).is_empty());
    }
}
