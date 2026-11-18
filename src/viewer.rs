//! The numbers behind the viewer of the path: what kind of move each segment is, which tool is cutting it, how far along the program it is, how
//! fast it is fed, the colours for each way of colouring, which tools are shown (the layers), and which segment is under a click. Only numbers
//! and no window, so every rule is tested on its own; the window (`gui/viewer.rs`) draws with what is worked out here and nothing here moves a
//! machine.

use crate::gcode::Segment;
use crate::stats::tool_by_line;

/// How the path is coloured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorBy {
    /// What is done and what is still to do (the colours the window always had).
    #[default]
    State,
    /// Rapid, cutting, plunging and arc moves.
    Kind,
    /// How far along the program a move is, from its start to its end.
    Progress,
    /// One colour for each tool.
    Tool,
    /// From the slowest to the fastest cutting feed.
    Feed,
}

impl ColorBy {
    pub const ALL: [ColorBy; 5] = [
        ColorBy::State,
        ColorBy::Kind,
        ColorBy::Progress,
        ColorBy::Tool,
        ColorBy::Feed,
    ];
}

/// The kind of a move.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Rapid,
    /// A cut along the work.
    Cut,
    /// A cutting move that goes down with no sideways travel.
    Plunge,
    /// An arc (shown by its chord).
    Arc,
}

/// Tells the kind of a move.
pub fn kind_of(s: &Segment) -> Kind {
    if s.rapid {
        Kind::Rapid
    } else if s.arc {
        Kind::Arc
    } else if (s.to.0 - s.from.0).abs() < 1e-9
        && (s.to.1 - s.from.1).abs() < 1e-9
        && s.to.2 < s.from.2
    {
        Kind::Plunge
    } else {
        Kind::Cut
    }
}

/// The length of a move in mm.
pub fn length_of(s: &Segment) -> f64 {
    ((s.to.0 - s.from.0).powi(2) + (s.to.1 - s.from.1).powi(2) + (s.to.2 - s.from.2).powi(2)).sqrt()
}

/// The tool that makes each move of the path (nothing for the moves before the first tool is taken).
pub fn tools_of(lines: &[String], path: &[Segment]) -> Vec<Option<u32>> {
    let by_line = tool_by_line(lines);
    path.iter()
        .map(|s| by_line.get(s.line).copied().flatten())
        .collect()
}

/// The tools of the path in the order they first move.
pub fn distinct(per_move: &[Option<u32>]) -> Vec<Option<u32>> {
    let mut out: Vec<Option<u32>> = Vec::new();
    for t in per_move {
        if !out.contains(t) {
            out.push(*t);
        }
    }
    out
}

/// The colour of a tool: a few that tell each other apart, used again after eight; a move before any tool is grey.
pub fn tool_color(tool: Option<u32>) -> (u8, u8, u8) {
    const PALETTE: [(u8, u8, u8); 8] = [
        (76, 194, 255),
        (255, 176, 0),
        (124, 214, 92),
        (235, 100, 160),
        (170, 130, 255),
        (255, 120, 70),
        (60, 210, 190),
        (220, 220, 90),
    ];
    match tool {
        Some(n) => PALETTE[(n as usize) % PALETTE.len()],
        None => (150, 160, 170),
    }
}

/// From blue (0) through to red (1).
pub fn ramp(t: f64) -> (u8, u8, u8) {
    let t = if t.is_finite() {
        t.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let lerp = |a: f64, b: f64| (a + (b - a) * t).round() as u8;
    (lerp(37.0, 239.0), lerp(99.0, 68.0), lerp(235.0, 68.0))
}

/// How far along the path the middle of each move is, from 0 to 1, by the length travelled (every move counts, rapids too). A path with no
/// length is spread evenly by the number of moves.
pub fn progress(path: &[Segment]) -> Vec<f64> {
    let total: f64 = path.iter().map(length_of).sum();
    let mut done = 0.0;
    let n = path.len().max(1) as f64;
    path.iter()
        .enumerate()
        .map(|(i, s)| {
            let l = length_of(s);
            let at = if total > 0.0 {
                (done + l / 2.0) / total
            } else {
                (i as f64 + 0.5) / n
            };
            done += l;
            at.clamp(0.0, 1.0)
        })
        .collect()
}

/// The slowest and the fastest feed among the cutting moves that have one (nothing when none has).
pub fn feed_span(path: &[Segment]) -> Option<(f64, f64)> {
    let mut span: Option<(f64, f64)> = None;
    for s in path.iter().filter(|s| !s.rapid && s.feed > 0.0) {
        span = Some(match span {
            None => (s.feed, s.feed),
            Some((lo, hi)) => (lo.min(s.feed), hi.max(s.feed)),
        });
    }
    span
}

/// Where a feed falls between the slowest (0) and the fastest (1); the middle when they are all the same or the move has none.
pub fn feed_fraction(feed: f64, span: Option<(f64, f64)>) -> f64 {
    match span {
        Some((lo, hi)) if hi > lo && feed > 0.0 => ((feed - lo) / (hi - lo)).clamp(0.0, 1.0),
        _ => 0.5,
    }
}

/// The distance in pixels from the point `p` to the segment `a`-`b`.
pub fn distance_to_segment(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let (abx, aby) = (b[0] - a[0], b[1] - a[1]);
    let len2 = abx * abx + aby * aby;
    let t = if len2 > 0.0 {
        (((p[0] - a[0]) * abx + (p[1] - a[1]) * aby) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (cx, cy) = (a[0] + abx * t, a[1] + aby * t);
    (p[0] - cx).hypot(p[1] - cy)
}

/// The nearest of the segments (their number and the two ends on the screen) to the point, if one is within `tolerance` pixels; the first
/// of two at the same distance.
pub fn nearest(
    segments: impl Iterator<Item = (usize, [f32; 2], [f32; 2])>,
    p: [f32; 2],
    tolerance: f32,
) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for (i, a, b) in segments {
        let d = distance_to_segment(p, a, b);
        if d <= tolerance && best.is_none_or(|(_, bd)| d < bd) {
            best = Some((i, d));
        }
    }
    best.map(|(i, _)| i)
}

/// `[min x, min y, max x, max y]` of a move.
pub fn bounds_of(s: &Segment) -> [f64; 4] {
    [
        s.from.0.min(s.to.0),
        s.from.1.min(s.to.1),
        s.from.0.max(s.to.0),
        s.from.1.max(s.to.1),
    ]
}

/// What the viewer remembers: the way of colouring, the tools that are hidden, the move that is selected, and the numbers worked out from the
/// path (made again when the program changes).
#[derive(Debug, Clone, Default)]
pub struct State {
    pub color_by: ColorBy,
    /// The tools whose moves are not drawn (`None` is the moves before any tool).
    pub hidden: Vec<Option<u32>>,
    /// The move that was clicked (its place in the path).
    pub selected: Option<usize>,
    /// Asks the plan to zoom to the selection on the next frame.
    pub zoom_request: bool,
    /// The tool of each move, the progress of each move, the tools of the path and the span of the feeds.
    pub tool_per_move: Vec<Option<u32>>,
    pub progress: Vec<f64>,
    pub tools: Vec<Option<u32>>,
    pub feeds: Option<(f64, f64)>,
}

impl State {
    /// Works the numbers out again for a program and its path. The selection goes (its move is not the same any more), a tool that is not in
    /// the path any more is not kept hidden, and the way of colouring stays.
    pub fn refresh(&mut self, lines: &[String], path: &[Segment]) {
        self.tool_per_move = tools_of(lines, path);
        self.progress = progress(path);
        self.tools = distinct(&self.tool_per_move);
        self.feeds = feed_span(path);
        self.selected = None;
        self.zoom_request = false;
        let tools = self.tools.clone();
        self.hidden.retain(|t| tools.contains(t));
        // with every tool hidden there would be nothing to see
        if tools.iter().all(|t| self.hidden.contains(t)) {
            self.hidden.clear();
        }
    }

    /// Whether the moves of a tool are drawn.
    pub fn shows(&self, tool: Option<u32>) -> bool {
        !self.hidden.contains(&tool)
    }

    /// Whether the move number `i` is drawn.
    pub fn visible(&self, i: usize) -> bool {
        self.shows(self.tool_per_move.get(i).copied().flatten())
    }

    /// Shows a tool if it is hidden and hides it if it is shown. The last tool shown cannot be hidden: a viewer with nothing in it is no layer.
    pub fn toggle(&mut self, tool: Option<u32>) {
        if let Some(i) = self.hidden.iter().position(|t| *t == tool) {
            self.hidden.remove(i);
        } else if self.tools.iter().filter(|t| self.shows(**t)).count() > 1 {
            self.hidden.push(tool);
        }
    }

    /// The tool of a move.
    pub fn tool_of(&self, i: usize) -> Option<u32> {
        self.tool_per_move.get(i).copied().flatten()
    }

    /// How far along the program a move is (0 when it is not known).
    pub fn progress_of(&self, i: usize) -> f64 {
        self.progress.get(i).copied().unwrap_or(0.0)
    }

    /// The colour of a move for the way of colouring (nothing for `State`, whose colours depend on how far the program has run, and for
    /// rapids, which are always drawn dim).
    pub fn color_of(&self, i: usize, s: &Segment) -> Option<(u8, u8, u8)> {
        if s.rapid {
            return None;
        }
        match self.color_by {
            ColorBy::State => None,
            ColorBy::Kind => None,
            ColorBy::Progress => Some(ramp(self.progress_of(i))),
            ColorBy::Tool => Some(tool_color(self.tool_of(i))),
            ColorBy::Feed => Some(ramp(feed_fraction(s.feed, self.feeds))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gcode::toolpath;

    fn prog(text: &str) -> Vec<String> {
        text.lines().map(str::to_string).collect()
    }

    const TWO: &str = "G21 G90\nT1 M6\nG0 X10\nG1 Z-2 F100\nG1 X30 F600\nG1 Y20\nT2 M6\nG0 X0\nG1 X10 F300\nG2 X20 Y10 I5 J0";

    fn state(text: &str) -> (State, Vec<Segment>) {
        let lines = prog(text);
        let path = toolpath(&lines);
        let mut s = State::default();
        s.refresh(&lines, &path);
        (s, path)
    }

    #[test]
    fn a_move_is_a_rapid_a_plunge_an_arc_or_a_cut() {
        let (_, path) = state(TWO);
        let kinds: Vec<Kind> = path.iter().map(kind_of).collect();
        assert_eq!(
            kinds,
            [
                Kind::Rapid,
                Kind::Plunge,
                Kind::Cut,
                Kind::Cut,
                Kind::Rapid,
                Kind::Cut,
                Kind::Arc
            ]
        );
        // a move up with no sideways travel is a cut (the retract of a G1), not a plunge
        let up = toolpath(&prog("G1 Z-2 F100\nG1 Z1"));
        assert_eq!(kind_of(&up[1]), Kind::Cut);
    }

    #[test]
    fn each_move_has_the_tool_that_makes_it_and_the_tools_come_in_order() {
        let (s, _) = state(TWO);
        assert_eq!(
            s.tool_per_move,
            [
                Some(1),
                Some(1),
                Some(1),
                Some(1),
                Some(2),
                Some(2),
                Some(2)
            ]
        );
        assert_eq!(s.tools, [Some(1), Some(2)]);
        // moves before any tool are told apart
        let (before, _) = state("G1 X1 F100\nT3 M6\nG1 X2");
        assert_eq!(before.tools, [None, Some(3)]);
    }

    #[test]
    fn the_progress_grows_with_the_length_travelled() {
        let (s, path) = state(TWO);
        assert_eq!(s.progress.len(), path.len());
        assert!(
            s.progress.windows(2).all(|w| w[0] < w[1]),
            "{:?}",
            s.progress
        );
        assert!(s.progress[0] > 0.0 && *s.progress.last().unwrap() < 1.0);
        // a path with no length is spread by the number of moves
        let flat = progress(&[Segment {
            from: (0.0, 0.0, 0.0),
            to: (0.0, 0.0, 0.0),
            rapid: false,
            line: 0,
            feed: 0.0,
            arc: false,
        }]);
        assert_eq!(flat, [0.5]);
        assert!(progress(&[]).is_empty());
    }

    #[test]
    fn the_feed_goes_from_the_slowest_to_the_fastest() {
        let (s, path) = state(TWO);
        assert_eq!(s.feeds, Some((100.0, 600.0)));
        assert_eq!(feed_fraction(100.0, s.feeds), 0.0);
        assert_eq!(feed_fraction(600.0, s.feeds), 1.0);
        assert!((feed_fraction(350.0, s.feeds) - 0.5).abs() < 1e-9);
        // all the same, none, and a move with no feed
        assert_eq!(feed_fraction(200.0, Some((200.0, 200.0))), 0.5);
        assert_eq!(feed_fraction(200.0, None), 0.5);
        assert_eq!(feed_fraction(0.0, s.feeds), 0.5);
        assert_eq!(feed_span(&path[..1]), None);
    }

    #[test]
    fn the_ways_of_colouring_give_rapids_no_colour_and_cuts_their_own() {
        let (mut s, path) = state(TWO);
        // by state or kind the window picks the colour from its theme
        assert_eq!(s.color_of(2, &path[2]), None);
        s.color_by = ColorBy::Kind;
        assert_eq!(s.color_of(2, &path[2]), None);
        s.color_by = ColorBy::Progress;
        assert_eq!(s.color_of(0, &path[0]), None, "a rapid has none");
        let first = s.color_of(1, &path[1]).unwrap();
        let last = s.color_of(6, &path[6]).unwrap();
        assert_ne!(first, last);
        s.color_by = ColorBy::Tool;
        assert_eq!(s.color_of(2, &path[2]), Some(tool_color(Some(1))));
        assert_eq!(s.color_of(5, &path[5]), Some(tool_color(Some(2))));
        assert_ne!(tool_color(Some(1)), tool_color(Some(2)));
        s.color_by = ColorBy::Feed;
        assert_eq!(s.color_of(1, &path[1]), Some(ramp(0.0)));
        assert_eq!(s.color_of(2, &path[2]), Some(ramp(1.0)));
    }

    #[test]
    fn the_colours_stay_in_range_and_the_palette_repeats() {
        assert_eq!(ramp(0.0), (37, 99, 235));
        assert_eq!(ramp(1.0), (239, 68, 68));
        assert_eq!(ramp(f64::NAN), ramp(0.0));
        assert_eq!(ramp(9.0), ramp(1.0));
        assert_eq!(tool_color(Some(1)), tool_color(Some(9)));
        assert_ne!(tool_color(None), tool_color(Some(0)));
    }

    #[test]
    fn a_tool_can_be_hidden_and_shown_but_the_last_one_shown_stays() {
        let (mut s, _) = state(TWO);
        assert!(s.visible(0) && s.visible(5));
        s.toggle(Some(1));
        assert!(!s.visible(0) && !s.visible(3) && s.visible(5));
        // the last one cannot be hidden
        s.toggle(Some(2));
        assert!(s.shows(Some(2)));
        s.toggle(Some(1));
        assert!(s.visible(0));
        s.toggle(Some(2));
        assert!(!s.visible(5));
        // a move that is not in the list is shown
        assert!(s.visible(99) == s.shows(None));
    }

    #[test]
    fn a_new_program_clears_the_selection_and_the_hidden_tools_that_are_gone() {
        let (mut s, _) = state(TWO);
        s.selected = Some(3);
        s.zoom_request = true;
        s.hidden = vec![Some(1), Some(7)];
        s.color_by = ColorBy::Tool;
        let lines = prog("T1 M6\nG1 X5 F100\nT2 M6\nG1 X9");
        let path = toolpath(&lines);
        s.refresh(&lines, &path);
        assert_eq!(s.selected, None);
        assert!(!s.zoom_request);
        assert_eq!(s.hidden, [Some(1)], "a tool that is gone is not kept");
        assert_eq!(s.color_by, ColorBy::Tool, "the way of colouring stays");
        // a program with only the hidden tool would show nothing: all are shown
        let lines = prog("T1 M6\nG1 X5 F100");
        let path = toolpath(&lines);
        s.refresh(&lines, &path);
        assert!(s.hidden.is_empty());
    }

    #[test]
    fn a_click_finds_the_nearest_move_within_the_tolerance() {
        let segs = vec![
            (0usize, [0.0f32, 0.0], [100.0, 0.0]),
            (1, [100.0, 0.0], [100.0, 100.0]),
            (2, [0.0, 50.0], [100.0, 50.0]),
        ];
        assert_eq!(nearest(segs.clone().into_iter(), [50.0, 3.0], 6.0), Some(0));
        assert_eq!(
            nearest(segs.clone().into_iter(), [98.0, 70.0], 6.0),
            Some(1)
        );
        assert_eq!(
            nearest(segs.clone().into_iter(), [50.0, 48.0], 6.0),
            Some(2)
        );
        // too far
        assert_eq!(nearest(segs.clone().into_iter(), [50.0, 25.0], 6.0), None);
        // the nearer of two close ones, and the first of two at the same distance
        assert_eq!(
            nearest(segs.clone().into_iter(), [100.0, 4.0], 6.0),
            Some(1)
        );
        assert_eq!(
            nearest(segs.clone().into_iter(), [101.0, 0.0], 6.0),
            Some(0)
        );
        assert_eq!(nearest(std::iter::empty(), [0.0, 0.0], 6.0), None);
    }

    #[test]
    fn the_distance_to_a_segment_handles_the_ends_and_a_point_of_no_length() {
        assert_eq!(
            distance_to_segment([5.0, 3.0], [0.0, 0.0], [10.0, 0.0]),
            3.0
        );
        assert_eq!(
            distance_to_segment([-4.0, 3.0], [0.0, 0.0], [10.0, 0.0]),
            5.0
        );
        assert_eq!(
            distance_to_segment([13.0, 4.0], [0.0, 0.0], [10.0, 0.0]),
            5.0
        );
        assert_eq!(distance_to_segment([3.0, 4.0], [0.0, 0.0], [0.0, 0.0]), 5.0);
    }

    #[test]
    fn the_bounds_of_a_move_are_its_two_ends() {
        let (_, path) = state("G1 X10 Y-5 F100\nG1 X2 Y8");
        assert_eq!(bounds_of(&path[0]), [0.0, -5.0, 10.0, 0.0]);
        assert_eq!(bounds_of(&path[1]), [2.0, -5.0, 10.0, 8.0]);
        assert!((length_of(&path[0]) - 125.0f64.sqrt()).abs() < 1e-9);
    }
}
