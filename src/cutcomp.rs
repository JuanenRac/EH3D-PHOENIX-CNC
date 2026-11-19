//! The cutter compensation of a G-code program (`G41` to the left of the path, `G42` to the right, `G40` off), worked out in simulation:
//! where the centre of the tool goes when the program is written for the edge of the part, and what a controller would refuse or cut
//! wrong. The straight cutting moves of every stretch with the compensation on are shifted by the radius of the tool (the diameter of the
//! tool in the table of tools, for the tool in force or the one named by a `D` word), the corners are joined where the shifted lines meet
//! (a very sharp outside corner is cut off), and a move that comes out turned round, because the corner is tighter than the tool, is said.
//!
//! Arcs are not shifted (they break a stretch in two); a stretch needs the diameter of its tool in the table to be worked out. Nothing
//! here moves a machine.

use crate::gcode::{toolpath, words, Segment};
use crate::progcheck::{Finding, Level};
use crate::stats::tool_by_line;
use crate::tools::ToolTable;
use crate::words::{say, Msg};

/// An outside corner whose joint would lie farther than this many radii from the corner is cut off instead of mitred.
pub const MITRE_LIMIT: f64 = 4.0;

/// A point of the plane XY, in mm.
pub type P2 = (f64, f64);

/// Which side of the path the tool goes on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// `G41`.
    Left,
    /// `G42`.
    Right,
}

impl Side {
    pub fn code(self) -> &'static str {
        match self {
            Side::Left => "G41",
            Side::Right => "G42",
        }
    }
}

/// One stretch of the program with the compensation on.
#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub side: Side,
    /// The line (from 0) with the `G41` or `G42`.
    pub on_line: usize,
    /// The line with the `G40` that ends it, the line of a tool change or the end of the program that ended it, or nothing when the
    /// program runs out with the compensation still on.
    pub off_line: Option<usize>,
    /// The tool whose diameter is used: the `D` word of the line, or the tool in force there.
    pub tool: Option<u32>,
}

impl Span {
    /// The radius of its tool in mm, when the table has the tool.
    pub fn radius(&self, table: &ToolTable) -> Option<f64> {
        let d = table.find(self.tool?)?.diameter_mm;
        (d > 0.0).then_some(d / 2.0)
    }
}

/// The result of reading a program.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Scan {
    pub spans: Vec<Span>,
    pub findings: Vec<Finding>,
}

/// The path of the centre of a chain of straight moves, shifted by a radius.
#[derive(Debug, Clone, PartialEq)]
pub struct Offset {
    /// The shifted move of each straight move, as (start, end).
    pub pieces: Vec<(P2, P2)>,
    /// For each move: it comes out turned round (the corner is tighter than the tool).
    pub gouged: Vec<bool>,
}

fn dist(a: P2, b: P2) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

/// Shifts a chain of points (each point joined to the next by a straight move) to the left or the right by `r` mm. A repeated point is
/// taken once; fewer than two points give nothing.
pub fn offset_chain(points: &[P2], left: bool, r: f64) -> Offset {
    let mut p: Vec<P2> = Vec::with_capacity(points.len());
    for &q in points {
        if p.last().is_none_or(|&l| dist(l, q) > 1e-9) {
            p.push(q);
        }
    }
    let n = p.len().saturating_sub(1);
    if n == 0 {
        return Offset {
            pieces: Vec::new(),
            gouged: Vec::new(),
        };
    }
    let dir: Vec<P2> = (0..n)
        .map(|i| {
            let l = dist(p[i], p[i + 1]);
            ((p[i + 1].0 - p[i].0) / l, (p[i + 1].1 - p[i].1) / l)
        })
        .collect();
    let normal = |i: usize| -> P2 {
        let (dx, dy) = dir[i];
        if left {
            (-dy, dx)
        } else {
            (dy, -dx)
        }
    };
    let shift = |q: P2, i: usize| -> P2 {
        let nn = normal(i);
        (q.0 + r * nn.0, q.1 + r * nn.1)
    };
    let mut start: Vec<P2> = (0..n).map(|i| shift(p[i], i)).collect();
    let mut end: Vec<P2> = (0..n).map(|i| shift(p[i + 1], i)).collect();
    for k in 1..n {
        let (a, b) = (dir[k - 1], dir[k]);
        let cross = a.0 * b.1 - a.1 * b.0;
        if cross.abs() < 1e-9 {
            if a.0 * b.0 + a.1 * b.1 > 0.0 {
                // straight on: one point
                let q = shift(p[k], k);
                end[k - 1] = q;
                start[k] = q;
            }
            // a move straight back keeps the two ends apart
            continue;
        }
        let q0 = shift(p[k - 1], k - 1);
        let q1 = shift(p[k], k);
        let t = ((q1.0 - q0.0) * b.1 - (q1.1 - q0.1) * b.0) / cross;
        let joint = (q0.0 + t * a.0, q0.1 + t * a.1);
        if dist(joint, p[k]) <= MITRE_LIMIT * r.abs().max(1e-9) {
            end[k - 1] = joint;
            start[k] = joint;
        }
        // otherwise the corner is cut off: the ends stay where the two shifted moves end and start
    }
    let mut pieces = Vec::with_capacity(n);
    let mut gouged = Vec::with_capacity(n);
    for i in 0..n {
        let along = (end[i].0 - start[i].0) * dir[i].0 + (end[i].1 - start[i].1) * dir[i].1;
        pieces.push((start[i], end[i]));
        gouged.push(along < -1e-9);
    }
    Offset { pieces, gouged }
}

/// The length of the shifted path in mm, counting the straight lines that join one piece to the next when a corner is cut off.
pub fn offset_length(o: &Offset) -> f64 {
    let mut len = 0.0;
    for (i, &(s, e)) in o.pieces.iter().enumerate() {
        len += dist(s, e);
        if let Some(&(next, _)) = o.pieces.get(i + 1) {
            len += dist(e, next);
        }
    }
    len
}

/// The radius of an arc line in the plane `G17`: the `R` word, or the distance of the centre `I`, `J` from the start.
fn arc_radius(ws: &[(char, f64)]) -> Option<f64> {
    if let Some(&(_, r)) = ws.iter().find(|&&(c, _)| c == 'R') {
        return Some(r.abs());
    }
    let get = |l: char| ws.iter().find(|&&(c, _)| c == l).map(|&(_, v)| v);
    let (i, j) = (get('I'), get('J'));
    if i.is_none() && j.is_none() {
        return None;
    }
    Some(i.unwrap_or(0.0).hypot(j.unwrap_or(0.0)))
}

fn say_for(out: &mut Scan, line: usize, level: Level, msg: Msg, args: Vec<String>) {
    out.findings.push(Finding {
        line: Some(line),
        level,
        msg,
        args,
    });
}

/// Reads the program: the stretches with the compensation on and what is wrong with the way they start, end and turn. Whatever needs the
/// diameter of a tool is said only when the table has tools.
pub fn scan(lines: &[String], table: &ToolTable) -> Scan {
    let tools = tool_by_line(lines);
    let mut out = Scan::default();
    let mut motion = 0;
    let mut open: Option<usize> = None;
    let mut waiting = false;
    let mut plane = 17;
    for (n, raw) in lines.iter().enumerate() {
        let trimmed = raw.trim_start();
        if trimmed.starts_with('$') || trimmed.starts_with('%') {
            continue;
        }
        let ws = words(raw);
        let (mut on, mut off, mut xy, mut ends, mut d) = (None, false, false, false, None);
        for &(c, v) in &ws {
            let t = (v * 10.0).round() as i32;
            match c {
                'G' => match t {
                    0 | 10 | 20 | 30 => motion = t / 10,
                    170 | 180 | 190 => plane = t / 10,
                    400 => off = true,
                    410 => on = Some(Side::Left),
                    420 => on = Some(Side::Right),
                    _ => {}
                },
                'M' if matches!(v as i32, 2 | 6 | 30) => ends = true,
                'D' if v >= 0.0 && v.fract() == 0.0 && v < 1e6 => d = Some(v as u32),
                'X' | 'Y' => xy = true,
                _ => {}
            }
        }
        if let Some(side) = on {
            if let Some(i) = open.take() {
                out.spans[i].off_line = Some(n);
            }
            out.spans.push(Span {
                side,
                on_line: n,
                off_line: None,
                tool: d.or(tools[n]),
            });
            open = Some(out.spans.len() - 1);
            waiting = true;
            let (no_radius, tool) = {
                let s = &out.spans[out.spans.len() - 1];
                (s.radius(table).is_none(), s.tool)
            };
            if !table.tools.is_empty() && no_radius {
                if let Some(t) = tool {
                    say_for(
                        &mut out,
                        n,
                        Level::Info,
                        Msg::CompNoDiameter,
                        vec![side.code().to_string(), t.to_string()],
                    );
                }
            }
        }
        if xy && waiting {
            waiting = false;
            if motion != 1 {
                if let Some(i) = open {
                    let code = out.spans[i].side.code().to_string();
                    say_for(
                        &mut out,
                        n,
                        Level::Warning,
                        Msg::CompOnNotLinear,
                        vec![code],
                    );
                }
            }
        }
        if let (Some(i), true) = (open, plane == 17 && xy && matches!(motion, 2 | 3)) {
            let side = out.spans[i].side;
            let radius = out.spans[i].radius(table);
            let inside = matches!((side, motion), (Side::Left, 3) | (Side::Right, 2));
            if let (Some(rad), Some(arc), true) = (radius, arc_radius(&ws), inside) {
                if arc + 1e-9 < rad && n != out.spans[i].on_line {
                    say_for(
                        &mut out,
                        n,
                        Level::Warning,
                        Msg::CompArcSmall,
                        vec![fmt(arc), fmt(rad)],
                    );
                }
            }
        }
        if off {
            if let Some(i) = open.take() {
                out.spans[i].off_line = Some(n);
                if xy && matches!(motion, 2 | 3) {
                    say_for(&mut out, n, Level::Warning, Msg::CompOffNotLinear, vec![]);
                }
            }
            waiting = false;
        } else if ends {
            if let Some(i) = open.take() {
                out.spans[i].off_line = Some(n);
                let code = out.spans[i].side.code().to_string();
                let on_line = out.spans[i].on_line;
                say_for(
                    &mut out,
                    on_line,
                    Level::Warning,
                    Msg::CompStillOn,
                    vec![code],
                );
            }
            waiting = false;
        }
    }
    if let Some(i) = open {
        let code = out.spans[i].side.code().to_string();
        let on_line = out.spans[i].on_line;
        say_for(
            &mut out,
            on_line,
            Level::Warning,
            Msg::CompStillOn,
            vec![code],
        );
    }
    out.findings.sort_by_key(|f| f.line.unwrap_or(usize::MAX));
    out
}

/// A number of mm as short as it reads well (up to three decimals).
fn fmt(v: f64) -> String {
    let s = format!("{v:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// The chains of straight cutting moves of one stretch: moves with the compensation on that are not rapid, not arcs and whose line is
/// inside the stretch; a move that changes only Z stays in the chain, a rapid move, an arc or a jump breaks it.
fn chains(path: &[Segment], span: &Span) -> Vec<Vec<(P2, usize)>> {
    let mut out: Vec<Vec<(P2, usize)>> = Vec::new();
    let mut cur: Vec<(P2, usize)> = Vec::new();
    for s in path {
        let inside = s.line >= span.on_line && span.off_line.is_none_or(|o| s.line < o);
        if !inside || s.rapid || s.arc {
            if cur.len() > 1 {
                out.push(std::mem::take(&mut cur));
            } else {
                cur.clear();
            }
            continue;
        }
        let (a, b) = ((s.from.0, s.from.1), (s.to.0, s.to.1));
        if cur.last().is_some_and(|&(l, _)| dist(l, a) > 1e-9) {
            if cur.len() > 1 {
                out.push(std::mem::take(&mut cur));
            } else {
                cur.clear();
            }
        }
        if cur.is_empty() {
            cur.push((a, s.line));
        }
        if dist(a, b) > 1e-9 {
            cur.push((b, s.line));
        }
    }
    if cur.len() > 1 {
        out.push(cur);
    }
    out
}

/// The compensated path of the program: for every stretch whose tool has a diameter in the table, each chain of straight cutting moves
/// shifted by the radius. Moves are cutting moves at the Z of the chain they came from (the shift is in the plane XY only).
pub fn compensated(lines: &[String], table: &ToolTable) -> Vec<Segment> {
    let path = toolpath(lines);
    let mut out = Vec::new();
    for span in scan(lines, table).spans {
        let Some(r) = span.radius(table) else {
            continue;
        };
        for chain in chains(&path, &span) {
            let pts: Vec<P2> = chain.iter().map(|&(p, _)| p).collect();
            let o = offset_chain(&pts, span.side == Side::Left, r);
            for (i, &(s, e)) in o.pieces.iter().enumerate() {
                let line = chain.get(i + 1).map_or(span.on_line, |c| c.1);
                let z = path
                    .iter()
                    .find(|g| g.line == line && !g.rapid && !g.arc)
                    .map_or(0.0, |g| g.to.2);
                out.push(Segment {
                    from: (s.0, s.1, z),
                    to: (e.0, e.1, z),
                    rapid: false,
                    line,
                    feed: 0.0,
                    arc: false,
                });
            }
        }
    }
    out
}

/// What the compensation changes in the program: the stretches that are ended wrongly, the arcs that are too tight and the moves that
/// come out turned round.
pub fn findings(lines: &[String], table: &ToolTable) -> Vec<Finding> {
    let mut sc = scan(lines, table);
    let path = toolpath(lines);
    for span in &sc.spans {
        let Some(r) = span.radius(table) else {
            continue;
        };
        for chain in chains(&path, span) {
            let pts: Vec<P2> = chain.iter().map(|&(p, _)| p).collect();
            let o = offset_chain(&pts, span.side == Side::Left, r);
            let mut said = Vec::new();
            for (i, &bad) in o.gouged.iter().enumerate() {
                let line = chain.get(i + 1).map_or(span.on_line, |c| c.1);
                if bad && !said.contains(&line) {
                    said.push(line);
                    sc.findings.push(Finding {
                        line: Some(line),
                        level: Level::Danger,
                        msg: Msg::CompGouge,
                        args: vec![span.side.code().to_string(), fmt(r)],
                    });
                }
            }
        }
    }
    sc.findings.sort_by_key(|f| f.line.unwrap_or(usize::MAX));
    sc.findings
}

/// The line of the report about the compensation: how many stretches have it on and how long the path of the centre of the tool is next
/// to the programmed path. Nothing when no stretch can be worked out.
pub fn report_lines(lines: &[String], table: &ToolTable, lang: &str) -> Vec<String> {
    let path = toolpath(lines);
    let sc = scan(lines, table);
    let (mut counted, mut shifted, mut programmed) = (0usize, 0.0f64, 0.0f64);
    for span in &sc.spans {
        let Some(r) = span.radius(table) else {
            continue;
        };
        let mut any = false;
        for chain in chains(&path, span) {
            let pts: Vec<P2> = chain.iter().map(|&(p, _)| p).collect();
            let o = offset_chain(&pts, span.side == Side::Left, r);
            shifted += offset_length(&o);
            programmed += pts.windows(2).map(|w| dist(w[0], w[1])).sum::<f64>();
            any = true;
        }
        if any {
            counted += 1;
        }
    }
    if counted == 0 {
        return Vec::new();
    }
    vec![say(
        lang,
        Msg::CompPath,
        &[counted.to_string(), fmt(shifted), fmt(programmed)],
    )]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prog(text: &str) -> Vec<String> {
        text.lines().map(str::to_string).collect()
    }

    fn table(text: &str) -> ToolTable {
        ToolTable::parse(text)
    }

    fn msgs(f: &[Finding]) -> Vec<Msg> {
        f.iter().map(|f| f.msg).collect()
    }

    const T2: &str = "tool = 1 | diameter=2";

    #[test]
    fn a_square_cut_on_the_left_is_shifted_inside_by_the_radius() {
        let o = offset_chain(
            &[
                (0.0, 0.0),
                (10.0, 0.0),
                (10.0, 10.0),
                (0.0, 10.0),
                (0.0, 0.0),
            ],
            true,
            1.0,
        );
        assert_eq!(o.pieces.len(), 4);
        assert_eq!(o.pieces[0], ((0.0, 1.0), (9.0, 1.0)));
        assert_eq!(o.pieces[1], ((9.0, 1.0), (9.0, 9.0)));
        assert_eq!(o.pieces[3].1, (1.0, 0.0));
        assert!(o.gouged.iter().all(|&g| !g));
        assert!((offset_length(&o) - 34.0).abs() < 1e-9);
    }

    #[test]
    fn on_the_right_the_corners_are_mitred_outside() {
        let o = offset_chain(
            &[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)],
            true,
            1.0,
        );
        let r = offset_chain(
            &[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)],
            false,
            1.0,
        );
        // the right side of a path that turns left is the outside: the joints are the corners of the bigger square
        assert_eq!(r.pieces[0], ((0.0, -1.0), (11.0, -1.0)));
        assert_eq!(r.pieces[1], ((11.0, -1.0), (11.0, 11.0)));
        assert!(o.pieces[0].1 .0 < r.pieces[0].1 .0);
    }

    #[test]
    fn a_very_sharp_outside_corner_is_cut_off_and_a_straight_run_keeps_one_point() {
        // a spike: the joint on the outside would lie 20 radii from the corner
        let r = offset_chain(&[(0.0, 0.0), (10.0, 0.0), (0.0, 0.5)], false, 1.0);
        assert_eq!(r.pieces.len(), 2);
        assert_ne!(r.pieces[0].1, r.pieces[1].0);
        let s = offset_chain(&[(0.0, 0.0), (5.0, 0.0), (10.0, 0.0)], true, 2.0);
        assert_eq!(s.pieces[0].1, s.pieces[1].0);
        assert_eq!(s.pieces[0].1, (5.0, 2.0));
        // a repeated point is one point and a single point has no move
        assert_eq!(
            offset_chain(&[(1.0, 1.0), (1.0, 1.0)], true, 1.0)
                .pieces
                .len(),
            0
        );
        assert_eq!(offset_chain(&[], true, 1.0).pieces.len(), 0);
    }

    #[test]
    fn a_move_shorter_than_the_corner_needs_comes_out_turned_round() {
        let o = offset_chain(
            &[(0.0, 0.0), (10.0, 0.0), (10.0, 0.5), (0.0, 0.5)],
            true,
            1.0,
        );
        assert_eq!(o.gouged, [false, true, false]);
    }

    #[test]
    fn the_stretches_of_the_program_are_found_with_their_tool() {
        let t = table(T2);
        let p = prog("T1 M6\nG0 X0 Y0\nG41 D1\nG1 X10 F100\nG40\nG0 Z5\nG42\nG1 X0\nM30");
        let s = scan(&p, &t);
        assert_eq!(s.spans.len(), 2);
        assert_eq!(s.spans[0].side, Side::Left);
        assert_eq!((s.spans[0].on_line, s.spans[0].off_line), (2, Some(4)));
        assert_eq!(s.spans[0].tool, Some(1));
        assert_eq!(s.spans[0].radius(&t), Some(1.0));
        assert_eq!(s.spans[1].side, Side::Right);
        // the second one is ended by the end of the program and said
        assert_eq!(s.spans[1].off_line, Some(8));
        assert_eq!(msgs(&s.findings), vec![Msg::CompStillOn]);
        assert_eq!(s.findings[0].line, Some(6));
        assert_eq!(s.findings[0].args, ["G42"]);
    }

    #[test]
    fn the_compensation_must_start_and_end_with_straight_moves() {
        let t = ToolTable::default();
        let rapid = scan(&prog("G0 X0 Y0\nG41 D1\nG0 X5\nG40\nM30"), &t);
        assert_eq!(msgs(&rapid.findings), vec![Msg::CompOnNotLinear]);
        assert_eq!(rapid.findings[0].line, Some(2));
        // the move can be on the line of the G41 itself, and a Z move before it does not count
        let same = scan(&prog("G0 X0\nG1 G41 X5 Y5 F10\nG40\nM30"), &t);
        assert!(same.findings.is_empty());
        let later = scan(&prog("G1 F5\nG41\nZ-1\nX5\nG40\nM30"), &t);
        assert!(later.findings.is_empty());
        let arc = scan(
            &prog("G1 F5\nG41\nG1 X5\nG3 X10 Y5 I0 J5\nG40 G2 X15 Y0 I5 J-5\nM30"),
            &t,
        );
        assert_eq!(msgs(&arc.findings), vec![Msg::CompOffNotLinear]);
        // a G40 with a straight move or none is fine
        assert!(scan(&prog("G41\nG1 X5 F5\nG40 G0 X9\nM30"), &t)
            .findings
            .is_empty());
    }

    #[test]
    fn a_tool_change_and_the_end_of_the_program_end_a_stretch_that_is_still_on() {
        let t = ToolTable::default();
        let s = scan(&prog("G1 F5\nG41\nX5\nM6\nX9\nM30"), &t);
        assert_eq!(s.spans[0].off_line, Some(3));
        assert_eq!(msgs(&s.findings), vec![Msg::CompStillOn]);
        // running out with no end word
        let r = scan(&prog("G1 F5\nG42\nX5"), &t);
        assert_eq!(r.spans[0].off_line, None);
        assert_eq!(msgs(&r.findings), vec![Msg::CompStillOn]);
        // switching sides closes the first stretch
        let sw = scan(&prog("G1 F5\nG41\nX5\nG42\nX9\nG40\nM30"), &t);
        assert_eq!(sw.spans.len(), 2);
        assert_eq!(sw.spans[0].off_line, Some(3));
        assert!(sw.findings.is_empty());
    }

    #[test]
    fn a_tool_without_a_diameter_is_said_only_when_the_machine_has_a_table() {
        let none = scan(&prog("T1 M6\nG1 F5\nG41\nX5\nG40"), &ToolTable::default());
        assert!(none.findings.is_empty());
        let other = scan(&prog("T7 M6\nG1 F5\nG41\nX5\nG40"), &table(T2));
        assert_eq!(msgs(&other.findings), vec![Msg::CompNoDiameter]);
        assert_eq!(other.findings[0].args, ["G41", "7"]);
        // the D word names the tool when it is not the one in force
        let d = scan(&prog("T1 M6\nG1 F5\nG41 D9\nX5\nG40"), &table(T2));
        assert_eq!(d.findings[0].args, ["G41", "9"]);
    }

    #[test]
    fn an_arc_tighter_than_the_tool_on_the_inside_is_said() {
        let t = table("tool = 1 | diameter=4");
        // G41 with a counter-clockwise arc has the tool inside the arc: a radius of 1 is smaller than the radius of the tool, 2
        let tight = scan(
            &prog("T1 M6\nG1 F5\nG41\nG1 X5\nG3 X6 Y1 I0 J1\nG40\nM30"),
            &t,
        );
        assert_eq!(msgs(&tight.findings), vec![Msg::CompArcSmall]);
        assert_eq!(tight.findings[0].args, ["1", "2"]);
        // the same arc clockwise has the tool outside, and a big enough arc is fine
        let outside = scan(
            &prog("T1 M6\nG1 F5\nG41\nG1 X5\nG2 X6 Y-1 I0 J-1\nG40\nM30"),
            &t,
        );
        assert!(outside.findings.is_empty());
        let wide = scan(
            &prog("T1 M6\nG1 F5\nG41\nG1 X5\nG3 X5 Y8 I0 J4\nG40\nM30"),
            &t,
        );
        assert!(wide.findings.is_empty());
        // an arc by its radius and a G42 with a clockwise arc
        let by_r = scan(&prog("T1 M6\nG1 F5\nG42\nG1 X5\nG2 X6 Y1 R1\nG40\nM30"), &t);
        assert_eq!(msgs(&by_r.findings), vec![Msg::CompArcSmall]);
    }

    #[test]
    fn the_findings_include_the_moves_that_cut_into_the_part() {
        let t = table(T2);
        let p = prog(
            "T1 M6
G21 G90
G0 X0 Y0
G1 F100
G41
G1 X10
G1 Y0.5
G1 X0
G40
M30",
        );
        let f = findings(&p, &t);
        assert_eq!(msgs(&f), vec![Msg::CompGouge]);
        assert_eq!(f[0].line, Some(6));
        assert_eq!(f[0].level, Level::Danger);
        assert_eq!(f[0].args, ["G41", "1"]);
        // with no table there is nothing to shift by
        assert!(findings(&p, &ToolTable::default()).is_empty());
    }

    #[test]
    fn the_compensated_path_goes_round_the_part_at_the_radius() {
        let t = table(T2);
        let p = prog("T1 M6\nG0 X0 Y0 Z-1\nG1 F100\nG41\nG1 X10\nG1 Y10\nG1 X0\nG1 Y0\nG40\nM30");
        let path = compensated(&p, &t);
        assert_eq!(path.len(), 4);
        assert_eq!(
            (path[0].from, path[0].to),
            ((0.0, 1.0, -1.0), (9.0, 1.0, -1.0))
        );
        assert!(path.iter().all(|s| !s.rapid && !s.arc));
        assert_eq!(
            path.iter().map(|s| s.line).collect::<Vec<_>>(),
            [4, 5, 6, 7]
        );
        // a program with no compensation has no compensated path
        assert!(compensated(&prog("G1 X5 F1"), &t).is_empty());
    }

    #[test]
    fn the_report_line_counts_the_stretches_and_gives_both_lengths() {
        let t = table(T2);
        let p = prog("T1 M6\nG0 X0 Y0\nG1 F100\nG41\nG1 X10\nG1 Y10\nG1 X0\nG1 Y0\nG40\nM30");
        let l = report_lines(&p, &t, "en");
        assert_eq!(l.len(), 1);
        assert!(l[0].contains("1 stretches"), "{}", l[0]);
        assert!(l[0].contains("34 mm") && l[0].contains("40 mm"), "{}", l[0]);
        assert!(report_lines(&p, &ToolTable::default(), "en").is_empty());
        assert!(report_lines(&prog("G1 X5 F1"), &t, "en").is_empty());
    }

    #[test]
    fn an_arc_breaks_a_chain_and_a_rapid_move_ends_it() {
        let t = table(T2);
        let p = prog("T1 M6\nG1 F5 X0 Y0\nG41\nG1 X5\nG3 X10 Y5 I0 J5\nG1 X20\nG40\nM30");
        let path = toolpath(&p);
        let sp = scan(&p, &t).spans[0].clone();
        let c = chains(&path, &sp);
        // the straight move before the arc and the one after it are chains of their own
        assert_eq!(c.len(), 2, "{c:?}");
        assert!(c.iter().all(|k| k.len() == 2));
        let q = prog("T1 M6\nG1 F5 X0 Y0\nG41\nG1 X5\nG1 Y5\nG0 X20\nG1 Y9\nG1 X30\nG40\nM30");
        let c = chains(&toolpath(&q), &scan(&q, &t).spans[0]);
        assert_eq!(c.len(), 2);
        assert_eq!((c[0].len(), c[1].len()), (3, 3));
    }
}
