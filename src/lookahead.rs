//! The planner of the motion on paper: the moves of a program are joined by the speed they may keep at each corner and every move gets a
//! profile of speed (it speeds up, cruises and slows down) with the acceleration of its axes, so the time of a program is the one a
//! controller with a look-ahead buffer would take and not the sum of moves that start and stop. The speed at a corner follows the
//! junction deviation of GRBL (how far the path may cut the corner): a straight line keeps its speed, a sharp corner slows down, a
//! reversal stops. The look-ahead is a window of moves: the end of the window must be a place where the machine can stop, as in a
//! controller whose buffer holds that many moves.
//!
//! Arcs are timed along their curve as they are, or (with `arcs = on` in the motion file) rebuilt as short straight moves that leave the
//! curve by no more than `arc_tolerance`, which then take the corners between the pieces like any other. Nothing here moves a machine.

use crate::gcode::{arc_curves_within, toolpath, Point, Segment};
use crate::motioncfg::PlannerCfg;
use crate::sim::{seconds_accel, Extras, Rates};

/// The most moves the planner takes and the most work (moves times window) it does: past them the window shrinks.
pub const MAX_MOVES: usize = 2_000_000;
const MAX_WORK: usize = 200_000_000;

/// One move with its profile.
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    /// The line of the program (from 0) and the length of the move in mm.
    pub line: usize,
    pub length_mm: f64,
    /// The speed it is meant to cruise at, the speed it enters at, the one it leaves at and the one it reaches, all in mm/s.
    pub nominal: f64,
    pub entry: f64,
    pub exit: f64,
    pub peak: f64,
    /// The acceleration along the move in mm/s2.
    pub accel: f64,
    /// The seconds spent speeding up, cruising and slowing down.
    pub t_up: f64,
    pub t_cruise: f64,
    pub t_down: f64,
    pub rapid: bool,
}

impl Block {
    pub fn seconds(&self) -> f64 {
        self.t_up + self.t_cruise + self.t_down
    }

    /// The speed in mm/s `t` seconds after the start of the move.
    pub fn speed_at(&self, t: f64) -> f64 {
        let t = t.clamp(0.0, self.seconds());
        if t < self.t_up {
            self.entry + self.accel * t
        } else if t < self.t_up + self.t_cruise {
            self.peak
        } else {
            (self.peak - self.accel * (t - self.t_up - self.t_cruise)).max(self.exit)
        }
    }
}

/// The plan of a program.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Plan {
    pub blocks: Vec<Block>,
    /// The seconds of the whole program, with the tool changes.
    pub total_s: f64,
    /// The seconds the same moves would take if each one started and stopped (what the estimate without look-ahead says).
    pub stop_to_stop_s: f64,
    /// Corners where the machine comes to a halt (reversals, tool changes, the end) and the arcs rebuilt as straight pieces.
    pub stops: usize,
    pub arcs_rebuilt: usize,
    pub length_mm: f64,
}

struct Move {
    line: usize,
    len: f64,
    unit: [f64; 3],
    nominal: f64,
    accel: f64,
    rapid: bool,
    /// The machine halts before this move (a tool change comes first).
    stop_before: bool,
}

fn unit_of(a: Point, b: Point) -> Option<([f64; 3], f64)> {
    let d = [b.0 - a.0, b.1 - a.1, b.2 - a.2];
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    (len.is_finite() && len > 1e-9).then(|| ([d[0] / len, d[1] / len, d[2] / len], len))
}

/// The most speed along a direction when every axis has its own limit: the lowest of `limit / |component|`.
fn along(unit: &[f64; 3], limits: &[f64; 3]) -> f64 {
    unit.iter()
        .zip(limits)
        .filter(|(u, _)| u.abs() > 1e-9)
        .map(|(u, l)| l / u.abs())
        .fold(f64::INFINITY, f64::min)
}

/// The speed in mm/s at which a corner between two directions may be taken (GRBL's junction deviation).
fn junction_speed(prev: &[f64; 3], next: &[f64; 3], accel: f64, deviation: f64, floor: f64) -> f64 {
    let cos = -(prev[0] * next[0] + prev[1] * next[1] + prev[2] * next[2]);
    if cos < -0.999_999 {
        return f64::INFINITY; // a straight line
    }
    if cos > 0.999_999 {
        return 0.0; // a reversal
    }
    let sin_half = (0.5 * (1.0 - cos)).sqrt();
    let v2 = accel * deviation * sin_half / (1.0 - sin_half);
    v2.sqrt().max(floor)
}

fn moves_of(path: &[Segment], extras: &Extras, rates: Rates, cfg: &PlannerCfg) -> Vec<Move> {
    let rate_limits = cfg.max_rate_mm_min.map(|r| r / 60.0);
    let mut out = Vec::with_capacity(path.len());
    let mut change = 0;
    let mut pending_stop = false;
    for s in path.iter().take(MAX_MOVES) {
        let mut stop_before = pending_stop;
        while change < extras.changes.len() && extras.changes[change] <= s.line {
            stop_before = true;
            change += 1;
        }
        let Some((unit, chord)) = unit_of(s.from, s.to) else {
            // a tool change before a move of no length still halts the next move
            pending_stop = stop_before;
            continue;
        };
        pending_stop = false;
        let len = if s.arc {
            extras.arc_len(s.line).unwrap_or(chord)
        } else {
            chord
        };
        let wanted = if s.rapid {
            rates.rapid / 60.0
        } else if s.feed > 0.0 {
            s.feed / 60.0
        } else {
            rates.default_feed / 60.0
        };
        let nominal = wanted.min(along(&unit, &rate_limits)).max(1e-6);
        let accel = along(&unit, &cfg.accel_mm_s2).max(1e-6);
        out.push(Move {
            line: s.line,
            len,
            unit,
            nominal,
            accel,
            rapid: s.rapid,
            stop_before,
        });
    }
    out
}

/// The profile of one move from the speed it enters at to the one it leaves at.
fn block_of(m: &Move, entry: f64, exit: f64) -> Block {
    let (v_in, v_out) = (entry.min(m.nominal), exit.min(m.nominal));
    let a = m.accel;
    let reach = (2.0 * a * m.len + v_in * v_in + v_out * v_out) / 2.0;
    let mut peak = reach.sqrt().max(v_in).max(v_out);
    let (t_up, t_cruise, t_down);
    if peak > m.nominal {
        peak = m.nominal;
        let d_up = (peak * peak - v_in * v_in) / (2.0 * a);
        let d_down = (peak * peak - v_out * v_out) / (2.0 * a);
        t_up = (peak - v_in) / a;
        t_down = (peak - v_out) / a;
        t_cruise = ((m.len - d_up - d_down) / peak).max(0.0);
    } else {
        t_up = (peak - v_in) / a;
        t_down = (peak - v_out) / a;
        t_cruise = 0.0;
    }
    Block {
        line: m.line,
        length_mm: m.len,
        nominal: m.nominal,
        entry: v_in,
        exit: v_out,
        peak,
        accel: a,
        t_up,
        t_cruise,
        t_down,
        rapid: m.rapid,
    }
}

/// The plan of a path. `extras` gives the length of the arcs that are not rebuilt and the lines where the tool is changed (each one halts
/// the machine and takes `change_s` seconds); `rates` the rapid and the feed of a cut that has none.
pub fn plan(
    path: &[Segment],
    extras: &Extras,
    rates: Rates,
    cfg: &PlannerCfg,
    change_s: f64,
) -> Plan {
    let moves = moves_of(path, extras, rates, cfg);
    let n = moves.len();
    let mut p = Plan::default();
    if n == 0 {
        return p;
    }
    let floor = (cfg.min_speed_mm_min / 60.0).max(0.0);
    let dev = cfg.junction_deviation_mm.max(1e-6);
    // the fastest each corner may be taken: corner[k] is the one between move k-1 and move k (corner[0] and corner[n] are stops)
    let mut corner = vec![0.0f64; n + 1];
    for k in 1..n {
        corner[k] = if moves[k].stop_before {
            0.0
        } else {
            let a = moves[k - 1].accel.min(moves[k].accel);
            junction_speed(&moves[k - 1].unit, &moves[k].unit, a, dev, floor)
                .min(moves[k - 1].nominal)
                .min(moves[k].nominal)
        };
    }
    let window = cfg.lookahead.clamp(1, n).min((MAX_WORK / n).max(1));
    let mut entry = 0.0f64;
    // back[k]: the fastest move k may be entered at so that the end of the window in view can still be a stop (back[n] is the end)
    let mut back = vec![0.0f64; n + 2];
    let mut start = 0;
    let mut computed = false;
    while start < n {
        let end = (start + window).min(n);
        if !computed {
            back[end] = 0.0;
            for k in (start + 1..end).rev() {
                let m = &moves[k];
                let allowed = (back[k + 1] * back[k + 1] + 2.0 * m.accel * m.len).sqrt();
                back[k] = corner[k].min(allowed);
            }
            // a window that holds the whole program is worked out once
            computed = window >= n;
        }
        let m = &moves[start];
        let reach = (entry * entry + 2.0 * m.accel * m.len).sqrt();
        let exit = back[start + 1].min(reach).min(corner[start + 1]);
        let b = block_of(m, entry, exit);
        if exit <= 1e-9 {
            p.stops += 1;
        }
        entry = b.exit;
        p.blocks.push(b);
        start += 1;
    }
    let changes_total = extras.changes.len() as f64;
    let change = if change_s.is_finite() && change_s > 0.0 {
        change_s
    } else {
        0.0
    };
    p.total_s = p.blocks.iter().map(Block::seconds).sum::<f64>() + change * changes_total;
    p.length_mm = p.blocks.iter().map(|b| b.length_mm).sum();
    p.stop_to_stop_s = moves
        .iter()
        .map(|m| seconds_accel(m.len, m.nominal * 60.0, m.accel))
        .sum::<f64>()
        + change * changes_total;
    p
}

/// The plan of a program of G-code. With `cfg.arcs` the arcs are rebuilt as straight pieces within `cfg.arc_tolerance_mm`.
pub fn plan_lines(lines: &[String], rates: Rates, cfg: &PlannerCfg, change_s: f64) -> Plan {
    let mut path = toolpath(lines);
    let extras = Extras::of(lines);
    let mut rebuilt = 0;
    if cfg.arcs && path.iter().any(|s| s.arc) {
        let curves = arc_curves_within(lines, cfg.arc_tolerance_mm);
        let mut out = Vec::with_capacity(path.len());
        for s in path {
            let curve = s
                .arc
                .then(|| curves.binary_search_by_key(&s.line, |c| c.0).ok())
                .flatten();
            match curve {
                Some(i) => {
                    rebuilt += 1;
                    for w in curves[i].1.windows(2) {
                        out.push(Segment {
                            from: w[0],
                            to: w[1],
                            arc: false,
                            ..s
                        });
                    }
                }
                None => out.push(s),
            }
        }
        path = out;
    }
    let mut p = plan(&path, &extras, rates, cfg, change_s);
    p.arcs_rebuilt = rebuilt;
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(str::to_string).collect()
    }

    fn rates() -> Rates {
        Rates {
            rapid: 1500.0,
            default_feed: 600.0,
        }
    }

    fn cfg() -> PlannerCfg {
        PlannerCfg {
            accel_mm_s2: [100.0; 3],
            max_rate_mm_min: [6000.0; 3],
            ..PlannerCfg::default()
        }
    }

    #[test]
    fn one_move_is_a_trapezoid_that_starts_and_stops() {
        let p = plan_lines(&lines("G21 G90\nG1 X100 F600"), rates(), &cfg(), 0.0);
        assert_eq!(p.blocks.len(), 1);
        let b = &p.blocks[0];
        // 10 mm/s, 100 mm/s2: 0.1 s up, 0.1 s down, 100 mm = 0.5 up + 0.5 down + 99 at 10 mm/s
        assert!((b.t_up - 0.1).abs() < 1e-9 && (b.t_down - 0.1).abs() < 1e-9);
        assert!((p.total_s - 10.1).abs() < 1e-6, "{}", p.total_s);
        assert_eq!((b.entry, b.exit), (0.0, 0.0));
        assert!((b.speed_at(0.05) - 5.0).abs() < 1e-9);
        assert!((b.speed_at(5.0) - 10.0).abs() < 1e-9);
        assert!(b.speed_at(b.seconds()).abs() < 1e-9);
        // the same as the stop-to-stop estimate for one move
        assert!((p.stop_to_stop_s - p.total_s).abs() < 1e-6);
        assert_eq!(p.stops, 1);
    }

    #[test]
    fn a_straight_line_in_pieces_keeps_its_speed_and_takes_less_than_the_pieces_alone() {
        let text = "G21 G90\nG1 X10 F600\nG1 X20\nG1 X30\nG1 X40\nG1 X50";
        let p = plan_lines(&lines(text), rates(), &cfg(), 0.0);
        assert_eq!(p.blocks.len(), 5);
        assert!(p.blocks[1].entry > 9.9 && p.blocks[0].exit > 9.9);
        assert!(
            p.total_s < p.stop_to_stop_s - 0.3,
            "{} {}",
            p.total_s,
            p.stop_to_stop_s
        );
        // it is as long as one move of 50 mm
        let one = plan_lines(&lines("G21 G90\nG1 X50 F600"), rates(), &cfg(), 0.0);
        assert!((p.total_s - one.total_s).abs() < 1e-6);
        assert_eq!(p.stops, 1);
    }

    #[test]
    fn a_reversal_halts_a_right_angle_slows_and_a_wide_corner_hardly_does() {
        let reversal = plan_lines(&lines("G21 G90\nG1 X20 F600\nG1 X0"), rates(), &cfg(), 0.0);
        assert_eq!(reversal.blocks[0].exit, 0.0);
        assert_eq!(reversal.stops, 2);
        let right = plan_lines(&lines("G21 G90\nG1 X20 F600\nG1 Y20"), rates(), &cfg(), 0.0);
        let wide = plan_lines(
            &lines("G21 G90\nG1 X20 F600\nG1 X40 Y2"),
            rates(),
            &cfg(),
            0.0,
        );
        let (v_right, v_wide) = (right.blocks[0].exit, wide.blocks[0].exit);
        assert!(v_right > 0.0 && v_right < 5.0, "{v_right}");
        assert!(v_wide > v_right, "{v_wide} {v_right}");
        // a larger deviation takes the corner faster
        let loose = PlannerCfg {
            junction_deviation_mm: 0.1,
            ..cfg()
        };
        let faster = plan_lines(&lines("G21 G90\nG1 X20 F600\nG1 Y20"), rates(), &loose, 0.0);
        assert!(faster.blocks[0].exit > v_right);
        assert!(faster.total_s < right.total_s);
    }

    #[test]
    fn the_acceleration_and_the_limits_of_each_axis_decide_the_speed_along_a_diagonal() {
        let slow_z = PlannerCfg {
            accel_mm_s2: [100.0, 100.0, 10.0],
            max_rate_mm_min: [6000.0, 6000.0, 600.0],
            ..PlannerCfg::default()
        };
        // straight down: Z alone sets the speed (10 mm/s) and the acceleration (10 mm/s2)
        let p = plan_lines(&lines("G21 G90\nG1 Z-20 F6000"), rates(), &slow_z, 0.0);
        assert!((p.blocks[0].nominal - 10.0).abs() < 1e-9);
        assert!((p.blocks[0].accel - 10.0).abs() < 1e-9);
        // the speed asked for is never exceeded
        let q = plan_lines(&lines("G21 G90\nG1 X50 F300"), rates(), &cfg(), 0.0);
        assert!((q.blocks[0].nominal - 5.0).abs() < 1e-9);
    }

    #[test]
    fn a_tool_change_halts_the_machine_and_adds_its_time() {
        let text = "G21 G90\nG1 X10 F600\nT2 M6\nG1 X20";
        let p = plan_lines(&lines(text), rates(), &cfg(), 7.0);
        assert_eq!(p.blocks[0].exit, 0.0);
        assert_eq!(p.stops, 2);
        let no_change = plan_lines(&lines("G21 G90\nG1 X10 F600\nG1 X20"), rates(), &cfg(), 7.0);
        assert!(p.total_s - no_change.total_s > 6.9);
    }

    #[test]
    fn arcs_are_timed_as_they_are_or_rebuilt_in_pieces_within_the_tolerance() {
        let text = "G21 G90\nG1 X10 F600\nG2 X20 Y0 I5 J0\nG1 X30";
        let off = plan_lines(&lines(text), rates(), &cfg(), 0.0);
        assert_eq!(off.arcs_rebuilt, 0);
        // the arc is a half circle of radius 5: 15.7 mm long, not its chord of 10 mm
        assert!((off.length_mm - (10.0 + std::f64::consts::PI * 5.0 + 10.0)).abs() < 0.01);
        let on = plan_lines(
            &lines(text),
            rates(),
            &PlannerCfg {
                arcs: true,
                arc_tolerance_mm: 0.005,
                ..cfg()
            },
            0.0,
        );
        assert_eq!(on.arcs_rebuilt, 1);
        assert!(on.blocks.len() > 10);
        // the pieces stay inside the tolerance of the curve, so their length is a little under the arc's
        assert!(on.length_mm < off.length_mm && off.length_mm - on.length_mm < 0.1);
        // a finer tolerance makes more pieces
        let finer = plan_lines(
            &lines(text),
            rates(),
            &PlannerCfg {
                arcs: true,
                arc_tolerance_mm: 0.0005,
                ..cfg()
            },
            0.0,
        );
        assert!(finer.blocks.len() > on.blocks.len());
    }

    #[test]
    fn the_window_of_the_look_ahead_can_only_make_the_plan_slower_never_unsafe() {
        // a long run of small steps along a curve
        let mut text = String::from("G21 G90\nG1 X0 Y0 F1200\n");
        for i in 1..=200 {
            let a = i as f64 * 0.05;
            text.push_str(&format!(
                "G1 X{:.3} Y{:.3}\n",
                a.cos() * 20.0,
                a.sin() * 20.0
            ));
        }
        let l = lines(&text);
        let full = plan_lines(
            &l,
            rates(),
            &PlannerCfg {
                lookahead: 4096,
                ..cfg()
            },
            0.0,
        );
        let small = plan_lines(
            &l,
            rates(),
            &PlannerCfg {
                lookahead: 2,
                ..cfg()
            },
            0.0,
        );
        assert!(small.total_s >= full.total_s - 1e-9);
        for p in [&full, &small] {
            for w in p.blocks.windows(2) {
                // the speed leaving a move is the speed entering the next, and no move gains more than its room allows
                assert!((w[0].exit - w[1].entry).abs() < 1e-9);
            }
            for b in &p.blocks {
                assert!(b.exit <= b.nominal + 1e-9 && b.entry <= b.nominal + 1e-9);
                assert!(b.exit * b.exit <= b.entry * b.entry + 2.0 * b.accel * b.length_mm + 1e-6);
                assert!(b.entry * b.entry <= b.exit * b.exit + 2.0 * b.accel * b.length_mm + 1e-6);
                assert!(b.t_up >= 0.0 && b.t_cruise >= 0.0 && b.t_down >= 0.0);
            }
            assert_eq!(p.blocks.last().unwrap().exit, 0.0);
        }
    }

    #[test]
    fn nothing_to_plan_and_nonsense_numbers_give_a_plan_and_not_a_panic() {
        assert_eq!(plan_lines(&[], rates(), &cfg(), 0.0), Plan::default());
        assert_eq!(
            plan_lines(&lines("G21\nM3 S1000"), rates(), &cfg(), 0.0)
                .blocks
                .len(),
            0
        );
        let odd = PlannerCfg {
            accel_mm_s2: [0.0, f64::NAN, -1.0],
            max_rate_mm_min: [0.0; 3],
            junction_deviation_mm: 0.0,
            lookahead: 0,
            ..cfg()
        };
        let p = plan_lines(&lines("G1 X5 Y5 F100\nG1 X0 Y9"), rates(), &odd, f64::NAN);
        assert!(p.total_s.is_finite());
        let zero_feed = plan_lines(
            &lines("G1 X5"),
            Rates {
                rapid: 0.0,
                default_feed: 0.0,
            },
            &cfg(),
            0.0,
        );
        assert!(zero_feed.total_s.is_finite());
    }

    #[test]
    fn many_made_up_programs_give_plans_that_obey_their_own_limits() {
        let mut seed: u64 = 0x00c0_ffee_1234;
        let mut next = move |m: u64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed % m
        };
        for _ in 0..60 {
            let mut text = String::from("G21 G90\nG1 F900\n");
            for _ in 0..(5 + next(40)) {
                text.push_str(&format!(
                    "G{} X{} Y{} Z{}\n",
                    next(2),
                    next(80) as f64 - 20.0,
                    next(80) as f64 - 20.0,
                    next(20) as f64 - 10.0
                ));
            }
            let l = lines(&text);
            let w = 1 + next(40) as usize;
            let p = plan_lines(
                &l,
                rates(),
                &PlannerCfg {
                    lookahead: w,
                    ..cfg()
                },
                0.0,
            );
            assert!(
                p.total_s.is_finite() && p.total_s <= p.stop_to_stop_s + 1e-6,
                "{text}"
            );
            for b in &p.blocks {
                assert!(b.peak <= b.nominal + 1e-9);
                assert!(b.speed_at(b.seconds() / 2.0) <= b.peak + 1e-9);
            }
        }
    }
}
