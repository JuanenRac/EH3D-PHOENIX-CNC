//! Boards: the Gerber and Excellon files of a printed circuit become the G-code that cuts them. A copper layer becomes the isolation
//! (the paths of the tool a radius away from the copper, pass after pass), an outline layer becomes the profile (the tool goes round the
//! board on the outside and round the cut-outs on the inside), and a drill file becomes the holes and the slots, tool after tool. The
//! files are read by the readers of EH3D-CORE-LIBS (which the other programs of the suite use for the same files) and the paths are made by
//! its CAM, so what this program draws for a board is what the others draw. Every number that tunes the cut is in the `pcb_` keys of the
//! motion file. The readers are bounded (the size of the file, the number of shapes and the size of the grid) and a file that is wrong ends
//! in a [`Fail`], never in a panic. Nothing here moves a machine.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use eh3d_cam::isolate::{self, Params};
use eh3d_cam::order::{arrange, Cut, Order};
use eh3d_geometry::offset::{offset, OffsetOptions};
use eh3d_geometry::polygon::{area, Region};
use eh3d_geometry::scene::{from_excellon, from_gerber, Scene, Shape};
use eh3d_parser_excellon as excellon;
use eh3d_parser_gerber as gerber;

use crate::import::num;
use crate::motioncfg::{ImportCfg, PcbCfg};
use crate::notes::{Fail, Note};

type Pt = (f64, f64);

/// The kinds of file of a board.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Gerber,
    Excellon,
}

/// What a layer is cut as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The copper: the tool goes round it, a radius away.
    Isolation,
    /// The outline: the tool goes round the board on the outside and round the cut-outs on the inside.
    Profile,
    /// The holes and the slots of a drill file.
    Drill,
}

impl Kind {
    pub const ALL: [Kind; 3] = [Kind::Isolation, Kind::Profile, Kind::Drill];

    pub fn key(self) -> &'static str {
        match self {
            Kind::Isolation => "isolation",
            Kind::Profile => "profile",
            Kind::Drill => "drill",
        }
    }

    /// The kind by its key; `auto` and the empty text are `None` (the file says).
    pub fn from_key(key: &str) -> Option<Kind> {
        let k = key.trim().to_ascii_lowercase();
        Kind::ALL.into_iter().find(|kind| kind.key() == k)
    }
}

/// What the operator asks about a layer; what is left out, the file says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Ask {
    pub kind: Option<Kind>,
    /// Mirrors the layer about the Y axis (the layer is cut from the other side of the board).
    pub mirror: bool,
}

/// What a cutting routine gives back.
struct Cutting {
    lines: Vec<String>,
    cuts: usize,
    tools: usize,
    size: (f64, f64),
}

/// A board layer turned into a program.
#[derive(Debug, Clone, PartialEq)]
pub struct Job {
    pub kind: Kind,
    pub lines: Vec<String>,
    /// The paths the tool cuts along (isolation and profile) or the holes and slots it drills.
    pub cuts: usize,
    /// The different tools a drill file asks for (0 for the other kinds).
    pub tools: usize,
    /// The size of what is cut in mm.
    pub size_mm: (f64, f64),
    pub notes: Vec<Note>,
}

const GERBER_EXT: [&str; 14] = [
    "gbr", "ger", "gtl", "gbl", "gts", "gbs", "gto", "gbo", "gtp", "gbp", "gko", "gm1", "gm2",
    "gml",
];
const DRILL_EXT: [&str; 4] = ["drl", "xln", "exc", "ncd"];

fn extension(name: &str) -> String {
    Path::new(name)
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

/// The format of a file by the end of its name, or by how it starts.
pub fn detect(name: &str, head: &[u8]) -> Option<Format> {
    let ext = extension(name);
    if GERBER_EXT.contains(&ext.as_str()) {
        return Some(Format::Gerber);
    }
    if DRILL_EXT.contains(&ext.as_str()) {
        return Some(Format::Excellon);
    }
    let start = String::from_utf8_lossy(&head[..head.len().min(256)]).to_string();
    let t = start.trim_start_matches('\u{feff}').trim_start();
    if t.starts_with("M48") {
        return Some(Format::Excellon);
    }
    [
        "%FS", "%MO", "%TF", "%ADD", "G04", "G01", "G75", "G70", "G71",
    ]
    .iter()
    .any(|p| t.starts_with(p))
    .then_some(Format::Gerber)
}

/// The format of the file at a path, from its name and the first bytes of it.
pub fn detect_path(path: &Path) -> Option<Format> {
    use std::io::Read;
    let name = path.to_string_lossy();
    let mut head = Vec::new();
    if let Ok(f) = std::fs::File::open(path) {
        let _ = f.take(256).read_to_end(&mut head);
    }
    detect(&name, &head)
}

/// What a Gerber file is: its kind (`None` for a layer that is not cut: the mask, the legend, the paste), whether it is a layer of the
/// bottom and whether the kind was only assumed because the file does not say.
fn gerber_layer(file: &gerber::GerberFile, name: &str) -> (Option<Kind>, bool, bool) {
    let function = file
        .attributes
        .iter()
        .find(|a| a.kind == "TF" && a.name.eq_ignore_ascii_case(".FileFunction"));
    if let Some(a) = function {
        let bottom = a.values.iter().any(|v| v.eq_ignore_ascii_case("bot"));
        let kind = match a.values.first().map(|v| v.to_ascii_lowercase()).as_deref() {
            Some("copper") => Some(Kind::Isolation),
            Some("profile") => Some(Kind::Profile),
            _ => None,
        };
        return (kind, bottom, false);
    }
    match extension(name).as_str() {
        "gtl" => (Some(Kind::Isolation), false, false),
        "gbl" => (Some(Kind::Isolation), true, false),
        "gko" | "gm1" | "gm2" | "gml" => (Some(Kind::Profile), false, false),
        "gts" | "gto" | "gtp" => (None, false, false),
        "gbs" | "gbo" | "gbp" => (None, true, false),
        _ => (Some(Kind::Isolation), false, true),
    }
}

fn mirror_scene(scene: &mut Scene) {
    for s in &mut scene.shapes {
        match s {
            Shape::Disc { x, .. } | Shape::Rect { x, .. } | Shape::Obround { x, .. } => *x = -*x,
            Shape::Stroke { x0, x1, .. } => {
                *x0 = -*x0;
                *x1 = -*x1;
            }
            Shape::Region { pts } => pts.iter_mut().for_each(|p| p.0 = -p.0),
        }
    }
}

fn move_scene(scene: &mut Scene, dx: f64, dy: f64) {
    for s in &mut scene.shapes {
        match s {
            Shape::Disc { x, y, .. } | Shape::Rect { x, y, .. } | Shape::Obround { x, y, .. } => {
                *x += dx;
                *y += dy;
            }
            Shape::Stroke { x0, y0, x1, y1, .. } => {
                *x0 += dx;
                *x1 += dx;
                *y0 += dy;
                *y1 += dy;
            }
            Shape::Region { pts } => pts.iter_mut().for_each(|p| {
                p.0 += dx;
                p.1 += dy;
            }),
        }
    }
}

fn header(pcb: &PcbCfg, safe_z: f64) -> Vec<String> {
    let mut g = vec!["G21 G90 G17".to_string(), format!("G0 Z{}", num(safe_z))];
    if pcb.rpm > 0.0 {
        g.push(format!("M3 S{}", num(pcb.rpm)));
    }
    g
}

fn footer(g: &mut Vec<String>, pcb: &PcbCfg, safe_z: f64) {
    g.push(format!("G0 Z{}", num(safe_z)));
    if pcb.rpm > 0.0 {
        g.push("M5".to_string());
    }
}

/// The lines that cut along `cuts` down to `depth`, in passes of `step`.
fn cut_lines(
    g: &mut Vec<String>,
    cuts: &[Cut],
    depth: f64,
    step: f64,
    feed: f64,
    plunge: f64,
    safe_z: f64,
) {
    let passes = (((depth / step) - 1e-9).ceil() as usize).clamp(1, 1000);
    for c in cuts {
        let Some(first) = c.pts.first() else { continue };
        g.push(format!("G0 X{} Y{}", num(first.0), num(first.1)));
        for k in 1..=passes {
            let z = (step * k as f64).min(depth);
            g.push(format!("G1 Z{} F{}", num(-z), num(plunge)));
            let mut feed = Some(feed);
            for q in &c.pts[1..] {
                let f = feed
                    .take()
                    .map_or(String::new(), |f| format!(" F{}", num(f)));
                g.push(format!("G1 X{} Y{}{f}", num(q.0), num(q.1)));
            }
            if c.closed {
                g.push(format!("G1 X{} Y{}", num(first.0), num(first.1)));
            } else if k < passes {
                g.push(format!("G0 Z{}", num(safe_z)));
                g.push(format!("G0 X{} Y{}", num(first.0), num(first.1)));
            }
        }
        g.push(format!("G0 Z{}", num(safe_z)));
    }
}

fn size_of(points: impl Iterator<Item = Pt>) -> (f64, f64) {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    let mut any = false;
    for (x, y) in points {
        any = true;
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    if any {
        (x1 - x0, y1 - y0)
    } else {
        (0.0, 0.0)
    }
}

// ------------------------------------------------------------------------------------------------------------------ isolation

fn isolation(scene: &Scene, pcb: &PcbCfg, safe_z: f64) -> Result<Cutting, Fail> {
    let params = Params {
        tool_diameter_mm: pcb.tool_mm,
        passes: pcb.passes,
        stepover_mm: pcb.stepover_mm,
        res_mm: pcb.res_mm,
    };
    params.validate().map_err(|_| Fail::BadPcbSetting)?;
    let b = scene.bounds();
    if b.is_empty() {
        return Err(Fail::Empty);
    }
    // the grid the CAM draws the copper on: refused before it is made if it would be bigger than the setting allows
    let reach = params.reach_mm() * 2.0 + 2.0;
    let cells = ((b.width() + reach) / pcb.res_mm) * ((b.height() + reach) / pcb.res_mm);
    if !cells.is_finite() || cells > pcb.max_cells as f64 {
        return Err(Fail::GridTooBig);
    }
    let passes = isolate::isolate(scene, &params).map_err(|_| Fail::GridTooBig)?;
    let mut g = header(pcb, safe_z);
    let mut at = (0.0, 0.0);
    let mut count = 0;
    let mut all: Vec<Pt> = Vec::new();
    for pass in &passes {
        let order = Order {
            start: at,
            ..Order::default()
        };
        let (cuts, end) = arrange(&pass.cuts(), &order, at);
        at = end;
        count += cuts.len();
        all.extend(cuts.iter().flat_map(|c| c.pts.iter().copied()));
        cut_lines(
            &mut g,
            &cuts,
            pcb.depth_mm,
            pcb.depth_mm,
            pcb.feed_mm_min,
            pcb.plunge_mm_min,
            safe_z,
        );
    }
    if count == 0 {
        return Err(Fail::Empty);
    }
    footer(&mut g, pcb, safe_z);
    Ok(Cutting {
        lines: g,
        cuts: count,
        tools: 0,
        size: size_of(all.into_iter()),
    })
}

// ------------------------------------------------------------------------------------------------------------------ profile

type Key = (i64, i64);

fn key(p: Pt, q: f64) -> Key {
    ((p.0 / q).round() as i64, (p.1 / q).round() as i64)
}

/// The closed loops and the open runs that the strokes and the regions of an outline layer make, joined where their ends meet (within `join`).
fn outline_paths(scene: &Scene, join: f64) -> (Vec<Vec<Pt>>, Vec<Vec<Pt>>) {
    let q = join.max(1e-6);
    let mut closed: Vec<Vec<Pt>> = Vec::new();
    let mut segs: Vec<(Pt, Pt)> = Vec::new();
    for s in &scene.shapes {
        match s {
            Shape::Stroke { x0, y0, x1, y1, .. } => segs.push(((*x0, *y0), (*x1, *y1))),
            Shape::Region { pts } if pts.len() >= 3 => closed.push(pts.clone()),
            _ => {}
        }
    }
    let mut ends: HashMap<Key, Vec<usize>> = HashMap::new();
    for (i, (a, b)) in segs.iter().enumerate() {
        ends.entry(key(*a, q)).or_default().push(i);
        ends.entry(key(*b, q)).or_default().push(i);
    }
    let mut used = vec![false; segs.len()];
    let mut open: Vec<Vec<Pt>> = Vec::new();
    // walks from the end of a chain to the next segment that has an end there and was not used
    let extend = |chain: &mut Vec<Pt>, used: &mut Vec<bool>| loop {
        let tail = chain[chain.len() - 1];
        let next = ends
            .get(&key(tail, q))
            .and_then(|v| v.iter().copied().find(|&j| !used[j]));
        let Some(j) = next else { break };
        used[j] = true;
        let (a, b) = segs[j];
        chain.push(if key(a, q) == key(tail, q) { b } else { a });
    };
    for i in 0..segs.len() {
        if used[i] {
            continue;
        }
        used[i] = true;
        let mut chain = vec![segs[i].0, segs[i].1];
        extend(&mut chain, &mut used);
        chain.reverse();
        extend(&mut chain, &mut used);
        let first = chain[0];
        let last = chain[chain.len() - 1];
        if chain.len() >= 4 && key(first, q) == key(last, q) {
            chain.pop();
            closed.push(chain);
        } else {
            open.push(chain);
        }
    }
    (closed, open)
}

fn profile(
    scene: &Scene,
    pcb: &PcbCfg,
    safe_z: f64,
    notes: &mut Vec<Note>,
) -> Result<Cutting, Fail> {
    let (loops, open) = outline_paths(scene, pcb.join_mm);
    if loops.is_empty() && open.is_empty() {
        return Err(Fail::Empty);
    }
    let r = pcb.profile_tool_mm / 2.0;
    let opts = OffsetOptions {
        tolerance: (pcb.profile_tool_mm / 100.0).clamp(0.001, 0.02),
        ..OffsetOptions::default()
    };
    // the biggest loop is the board: the tool goes outside it; the others are cut-outs: the tool stays inside them
    let outer = loops
        .iter()
        .enumerate()
        .max_by(|a, b| area(a.1).total_cmp(&area(b.1)))
        .map(|(i, _)| i);
    let mut cuts: Vec<Cut> = Vec::new();
    let mut uncompensated = false;
    for (i, ring) in loops.iter().enumerate() {
        let delta = if Some(i) == outer { r } else { -r };
        let moved = offset(&Region::from_ring(ring.clone()), delta, &opts)
            .ok()
            .and_then(|reg| {
                reg.rings
                    .into_iter()
                    .filter(|g| g.len() >= 3)
                    .max_by(|a, b| area(a).total_cmp(&area(b)))
            });
        match moved {
            Some(g) => cuts.push(Cut {
                pts: g,
                closed: true,
            }),
            None => {
                uncompensated = true;
                cuts.push(Cut {
                    pts: ring.clone(),
                    closed: true,
                });
            }
        }
    }
    if uncompensated {
        notes.push(Note::PcbNoCompensation);
    }
    if !open.is_empty() {
        notes.push(Note::PcbOutlineOpen);
        cuts.extend(open.into_iter().map(|pts| Cut { pts, closed: false }));
    }
    // the cut-outs first, the board last, so that the board is still held while they are cut
    let mut ordered: Vec<Cut> = Vec::new();
    let (mut inner, mut rest): (Vec<Cut>, Vec<Cut>) = (Vec::new(), Vec::new());
    for (i, c) in cuts.into_iter().enumerate() {
        if Some(i) == outer {
            rest.push(c);
        } else {
            inner.push(c);
        }
    }
    let (arranged, end) = arrange(&inner, &Order::default(), (0.0, 0.0));
    ordered.extend(arranged);
    ordered.extend(arrange(&rest, &Order::default(), end).0);
    let mut g = header(pcb, safe_z);
    cut_lines(
        &mut g,
        &ordered,
        pcb.profile_depth_mm,
        pcb.profile_pass_mm,
        pcb.profile_feed_mm_min,
        pcb.plunge_mm_min,
        safe_z,
    );
    footer(&mut g, pcb, safe_z);
    let size = size_of(ordered.iter().flat_map(|c| c.pts.iter().copied()));
    Ok(Cutting {
        lines: g,
        cuts: ordered.len(),
        tools: 0,
        size,
    })
}

// ------------------------------------------------------------------------------------------------------------------ drill

struct Group {
    diameter: f64,
    holes: Vec<Pt>,
    slots: Vec<(Pt, Pt)>,
}

fn drilling(
    file: &excellon::ExcellonFile,
    pcb: &PcbCfg,
    safe_z: f64,
    mirror: bool,
    zero: bool,
    notes: &mut Vec<Note>,
) -> Result<Cutting, Fail> {
    let k = file.effective_units().to_mm(1.0);
    let sign = if mirror { -1.0 } else { 1.0 };
    let diameter = |tool: u32| file.tools.get(&tool).map_or(0.0, |t| t.diameter * k);
    let at = |p: &excellon::Point| (p.x * k * sign, p.y * k);
    let mut groups: BTreeMap<i64, Group> = BTreeMap::new();
    let mut skipped = false;
    for h in &file.drills {
        let d = diameter(h.tool);
        if !(d.is_finite() && d > 0.0) {
            skipped = true;
            continue;
        }
        groups
            .entry((d * 1000.0).round() as i64)
            .or_insert_with(|| Group {
                diameter: d,
                holes: Vec::new(),
                slots: Vec::new(),
            })
            .holes
            .push(at(&h.at));
    }
    for s in &file.slots {
        let d = diameter(s.tool);
        if !(d.is_finite() && d > 0.0) {
            skipped = true;
            continue;
        }
        groups
            .entry((d * 1000.0).round() as i64)
            .or_insert_with(|| Group {
                diameter: d,
                holes: Vec::new(),
                slots: Vec::new(),
            })
            .slots
            .push((at(&s.from), at(&s.to)));
    }
    if skipped {
        notes.push(Note::PcbHoleSkipped);
    }
    if groups.is_empty() {
        return Err(Fail::Empty);
    }
    if zero {
        let (mut mx, mut my) = (f64::MAX, f64::MAX);
        for g in groups.values() {
            for p in g
                .holes
                .iter()
                .chain(g.slots.iter().flat_map(|s| [&s.0, &s.1]))
            {
                mx = mx.min(p.0);
                my = my.min(p.1);
            }
        }
        for g in groups.values_mut() {
            for p in g.holes.iter_mut() {
                *p = (p.0 - mx, p.1 - my);
            }
            for s in g.slots.iter_mut() {
                s.0 = (s.0 .0 - mx, s.0 .1 - my);
                s.1 = (s.1 .0 - mx, s.1 .1 - my);
            }
        }
    }
    let mut g = header(pcb, safe_z);
    let pecks = (((pcb.drill_depth_mm / pcb.peck_mm) - 1e-9).ceil() as usize).clamp(1, 1000);
    let mut last = (0.0, 0.0);
    let mut count = 0;
    let mut all: Vec<Pt> = Vec::new();
    for (n, group) in groups.values().enumerate() {
        if pcb.tool_change_m6 {
            g.push(format!("T{} M6", n + 1));
        } else {
            g.push("M5".to_string());
            g.push(format!("G0 Z{}", num(safe_z)));
            g.push(format!("M0 (change to drill {} mm)", num(group.diameter)));
            if pcb.rpm > 0.0 {
                g.push(format!("M3 S{}", num(pcb.rpm)));
            }
        }
        // the nearest hole next
        let mut left = group.holes.clone();
        while !left.is_empty() {
            let (i, _) = left
                .iter()
                .enumerate()
                .map(|(i, p)| (i, (p.0 - last.0).hypot(p.1 - last.1)))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .unwrap_or((0, 0.0));
            let p = left.swap_remove(i);
            last = p;
            all.push(p);
            count += 1;
            g.push(format!("G0 X{} Y{}", num(p.0), num(p.1)));
            for k in 1..=pecks {
                let z = (pcb.peck_mm * k as f64).min(pcb.drill_depth_mm);
                g.push(format!("G1 Z{} F{}", num(-z), num(pcb.drill_feed_mm_min)));
                if k < pecks {
                    g.push(format!("G0 Z{}", num(pcb.retract_mm)));
                }
            }
            g.push(format!("G0 Z{}", num(safe_z)));
        }
        for (a, b) in &group.slots {
            count += 1;
            all.push(*a);
            all.push(*b);
            g.push(format!("G0 X{} Y{}", num(a.0), num(a.1)));
            let mut here = *a;
            for k in 1..=pecks {
                let z = (pcb.peck_mm * k as f64).min(pcb.drill_depth_mm);
                g.push(format!("G1 Z{} F{}", num(-z), num(pcb.drill_feed_mm_min)));
                let to = if here == *a { *b } else { *a };
                g.push(format!("G1 X{} Y{}", num(to.0), num(to.1)));
                here = to;
            }
            last = here;
            g.push(format!("G0 Z{}", num(safe_z)));
        }
    }
    footer(&mut g, pcb, safe_z);
    let size = size_of(all.into_iter());
    Ok(Cutting {
        lines: g,
        cuts: count,
        tools: groups.len(),
        size,
    })
}

// ------------------------------------------------------------------------------------------------------------------ the whole file

/// Turns the bytes of a Gerber or Excellon file into the program that cuts it.
pub fn import_bytes(
    name: &str,
    bytes: &[u8],
    import: &ImportCfg,
    pcb: &PcbCfg,
    ask: &Ask,
) -> Result<Job, Fail> {
    if bytes.len() as u64 > import.max_bytes {
        return Err(Fail::TooBig);
    }
    let format = detect(name, bytes).ok_or(Fail::NotRecognised)?;
    let mut notes: Vec<Note> = Vec::new();
    match format {
        Format::Gerber => {
            let file = gerber::parse_bytes(bytes).map_err(|_| Fail::TooBig)?;
            let (auto, bottom, assumed) = gerber_layer(&file, name);
            let kind = ask.kind.or(auto).ok_or(Fail::NotACutLayer)?;
            if kind == Kind::Drill {
                return Err(Fail::NotACutLayer);
            }
            if assumed && ask.kind.is_none() {
                notes.push(Note::PcbAssumedCopper);
            }
            let mut scene = from_gerber(&file);
            if scene.shapes.is_empty() {
                let broken = file
                    .diagnostics
                    .iter()
                    .any(|d| d.severity >= gerber::Severity::Error);
                return Err(if broken { Fail::Syntax } else { Fail::Empty });
            }
            if scene.shapes.len() > import.max_points {
                return Err(Fail::TooManyPoints);
            }
            if scene.is_lossy() {
                notes.push(Note::PcbNotDrawn);
            }
            let mirror = ask.mirror || (bottom && kind == Kind::Isolation && pcb.mirror_bottom);
            if mirror {
                mirror_scene(&mut scene);
                notes.push(Note::PcbMirrored);
            }
            if pcb.zero_origin {
                let b = scene.bounds();
                if !b.is_empty() {
                    move_scene(&mut scene, -b.min_x, -b.min_y);
                }
            }
            let done = match kind {
                Kind::Isolation => isolation(&scene, pcb, import.safe_z_mm)?,
                _ => profile(&scene, pcb, import.safe_z_mm, &mut notes)?,
            };
            Ok(Job {
                kind,
                lines: done.lines,
                cuts: done.cuts,
                tools: done.tools,
                size_mm: done.size,
                notes,
            })
        }
        Format::Excellon => {
            if ask.kind.is_some_and(|k| k != Kind::Drill) {
                return Err(Fail::NotACutLayer);
            }
            let file = excellon::parse_bytes(bytes).map_err(|_| Fail::TooBig)?;
            if file.drills.len() + file.slots.len() > import.max_points {
                return Err(Fail::TooManyPoints);
            }
            if file.drills.is_empty() && file.slots.is_empty() {
                let broken = file
                    .diagnostics
                    .iter()
                    .any(|d| d.severity >= excellon::Severity::Error);
                return Err(if broken { Fail::Syntax } else { Fail::Empty });
            }
            if from_excellon(&file).is_lossy() {
                notes.push(Note::PcbNotDrawn);
            }
            if ask.mirror {
                notes.push(Note::PcbMirrored);
            }
            let done = drilling(
                &file,
                pcb,
                import.safe_z_mm,
                ask.mirror,
                pcb.zero_origin,
                &mut notes,
            )?;
            Ok(Job {
                kind: Kind::Drill,
                lines: done.lines,
                cuts: done.cuts,
                tools: done.tools,
                size_mm: done.size,
                notes,
            })
        }
    }
}

/// As [`import_bytes`] for the file at a path, looking at no more than `import.max_bytes` of it.
pub fn import_file(path: &Path, import: &ImportCfg, pcb: &PcbCfg, ask: &Ask) -> Result<Job, Fail> {
    use std::io::Read;
    let name = path.to_string_lossy();
    let f = std::fs::File::open(path).map_err(|_| Fail::Unreadable)?;
    let mut bytes = Vec::new();
    f.take(import.max_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Fail::Unreadable)?;
    import_bytes(&name, &bytes, import, pcb, ask)
}

#[cfg(test)]
mod tests {
    use super::*;

    const COPPER: &str = "%FSLAX34Y34*%\n%MOMM*%\n%TF.FileFunction,Copper,L1,Top*%\n%ADD10C,0.500*%\n%ADD11R,2.0X1.5*%\nD10*\nX0Y0D02*\nX100000Y0D01*\nD11*\nX50000Y30000D03*\nM02*\n";
    const OUTLINE: &str = "%FSLAX34Y34*%\n%MOMM*%\n%TF.FileFunction,Profile,NP*%\n%ADD10C,0.100*%\nD10*\nX0Y0D02*\nX400000Y0D01*\nX400000Y300000D01*\nX0Y300000D01*\nX0Y0D01*\nM02*\n";
    const DRILL: &str =
        "M48\nMETRIC,TZ\nT1C0.800\nT2C1.000\n%\nT1\nX5.0Y5.0\nX10.0Y5.0\nT2\nX20.0Y10.0\nM30\n";

    fn cfgs() -> (ImportCfg, PcbCfg) {
        (ImportCfg::default(), PcbCfg::default())
    }

    fn run(name: &str, text: &str, ask: Ask) -> Result<Job, Fail> {
        let (i, p) = cfgs();
        import_bytes(name, text.as_bytes(), &i, &p, &ask)
    }

    /// The numbers after a letter in the lines, for a bounds check.
    fn values(lines: &[String], letter: char) -> Vec<f64> {
        lines
            .iter()
            .flat_map(|l| l.split_whitespace())
            .filter_map(|w| w.strip_prefix(letter).and_then(|v| v.parse::<f64>().ok()))
            .collect()
    }

    #[test]
    fn the_format_comes_from_the_name_or_from_how_the_file_starts() {
        assert_eq!(detect("a.GTL", b""), Some(Format::Gerber));
        assert_eq!(detect("a.drl", b""), Some(Format::Excellon));
        assert_eq!(detect("a.txt", b"M48\nMETRIC"), Some(Format::Excellon));
        assert_eq!(
            detect("a.txt", b"\xEF\xBB\xBF%FSLAX34Y34*%"),
            Some(Format::Gerber)
        );
        assert_eq!(detect("a.svg", b"<svg/>"), None);
        assert_eq!(detect("noext", b"hello"), None);
        assert_eq!(Kind::from_key(" Drill "), Some(Kind::Drill));
        assert_eq!(Kind::from_key("auto"), None);
    }

    #[test]
    fn a_copper_layer_becomes_the_isolation_round_the_copper() {
        let job = run("top.gbr", COPPER, Ask::default()).unwrap();
        assert_eq!(job.kind, Kind::Isolation);
        assert!(job.cuts >= 2, "two passes round the copper: {}", job.cuts);
        assert!(job.lines[0].starts_with("G21 G90 G17"));
        // no cut goes below the depth of the setting and every plunge is at that depth
        let zs = values(&job.lines, 'Z');
        assert!(zs.iter().all(|z| *z >= -0.05 - 1e-9 && *z <= 5.0));
        assert!(zs.contains(&-0.05));
        // the paths are round the copper: all of them are outside the track (0 to 10 mm, 0.5 wide) by at least the radius of the tool
        let cut: Vec<(f64, f64)> = job
            .lines
            .iter()
            .filter(|l| l.starts_with("G1 X"))
            .filter_map(|l| {
                let x = values(std::slice::from_ref(l), 'X').first().copied()?;
                let y = values(std::slice::from_ref(l), 'Y').first().copied()?;
                Some((x, y))
            })
            .collect();
        assert!(!cut.is_empty());
        for (x, y) in cut {
            let dx = if x < 0.0 {
                -x
            } else if x > 10.0 {
                x - 10.0
            } else {
                0.0
            };
            let inside_pad = (x - 5.0).abs() < 1.0 && (y - 3.0).abs() < 0.75;
            let near_track = dx.hypot(y.abs()) < 0.30;
            assert!(!near_track && !inside_pad, "({x}, {y}) is on the copper");
        }
        assert!(job.size_mm.0 > 10.0 && job.size_mm.1 > 1.0);
        assert!(job.notes.is_empty(), "{:?}", job.notes);
    }

    #[test]
    fn the_settings_of_the_motion_file_change_the_cut() {
        let (i, mut p) = cfgs();
        p.depth_mm = 0.1;
        p.passes = 1;
        p.feed_mm_min = 333.0;
        p.rpm = 12000.0;
        let job = import_bytes("a.gbr", COPPER.as_bytes(), &i, &p, &Ask::default()).unwrap();
        assert!(values(&job.lines, 'Z').contains(&-0.1));
        assert!(job.lines.iter().any(|l| l.contains("F333")));
        assert!(
            job.lines.contains(&"M3 S12000".to_string())
                && job.lines.last() == Some(&"M5".to_string())
        );
        let two = run("a.gbr", COPPER, Ask::default()).unwrap().cuts;
        assert!(job.cuts < two, "one pass makes fewer paths than two");
        // a setting the CAM cannot use is a failure of its own, not a panic
        p.res_mm = 0.5;
        assert_eq!(
            import_bytes("a.gbr", COPPER.as_bytes(), &i, &p, &Ask::default()),
            Err(Fail::BadPcbSetting)
        );
    }

    #[test]
    fn a_grid_bigger_than_the_setting_is_refused_before_it_is_made() {
        let (i, mut p) = cfgs();
        p.max_cells = 1000;
        assert_eq!(
            import_bytes("a.gbr", COPPER.as_bytes(), &i, &p, &Ask::default()),
            Err(Fail::GridTooBig)
        );
    }

    #[test]
    fn the_bottom_of_a_board_is_mirrored_and_a_layer_can_be_mirrored_on_request() {
        let bottom = COPPER.replace("Copper,L1,Top", "Copper,L2,Bot");
        let job = run("b.gbr", &bottom, Ask::default()).unwrap();
        assert!(job.notes.contains(&Note::PcbMirrored));
        let xs = values(&job.lines, 'X');
        assert!(xs.iter().all(|x| *x <= 0.5), "mirrored about the Y axis");
        let top = run(
            "a.gbr",
            COPPER,
            Ask {
                kind: None,
                mirror: true,
            },
        )
        .unwrap();
        assert!(top.notes.contains(&Note::PcbMirrored));
        let (i, mut p) = cfgs();
        p.mirror_bottom = false;
        let plain = import_bytes("b.gbr", bottom.as_bytes(), &i, &p, &Ask::default()).unwrap();
        assert!(!plain.notes.contains(&Note::PcbMirrored));
    }

    #[test]
    fn the_zero_of_the_work_is_the_zero_of_the_file_unless_the_setting_moves_it() {
        let shifted = COPPER
            .replace("X0Y0D02*", "X200000Y100000D02*")
            .replace("X100000Y0D01*", "X300000Y100000D01*")
            .replace("X50000Y30000D03*", "X250000Y130000D03*");
        let kept = run("a.gbr", &shifted, Ask::default()).unwrap();
        assert!(values(&kept.lines, 'X').iter().all(|x| *x > 15.0));
        let (i, mut p) = cfgs();
        p.zero_origin = true;
        let moved = import_bytes("a.gbr", shifted.as_bytes(), &i, &p, &Ask::default()).unwrap();
        assert!(values(&moved.lines, 'X').iter().all(|x| *x < 15.0));
    }

    #[test]
    fn the_outline_is_cut_on_the_outside_of_the_board_and_in_passes() {
        let job = run("edge.gko", OUTLINE, Ask::default()).unwrap();
        assert_eq!(job.kind, Kind::Profile);
        assert_eq!(job.cuts, 1);
        let xs = values(&job.lines, 'X');
        let ys = values(&job.lines, 'Y');
        let (x0, x1) = (
            xs.iter().cloned().fold(f64::MAX, f64::min),
            xs.iter().cloned().fold(f64::MIN, f64::max),
        );
        let (y0, y1) = (
            ys.iter().cloned().fold(f64::MAX, f64::min),
            ys.iter().cloned().fold(f64::MIN, f64::max),
        );
        // the board is 40 x 30 and the tool is 1 mm wide: the centre of the tool goes half a millimetre outside
        assert!(
            (x0 + 0.5).abs() < 0.01 && (x1 - 40.5).abs() < 0.01,
            "{x0} {x1}"
        );
        assert!(
            (y0 + 0.5).abs() < 0.01 && (y1 - 30.5).abs() < 0.01,
            "{y0} {y1}"
        );
        // 1.7 mm in passes of 0.6 is three passes
        let plunges = job.lines.iter().filter(|l| l.starts_with("G1 Z-")).count();
        assert_eq!(plunges, 3);
        assert!(values(&job.lines, 'Z').contains(&-1.7));
        assert!((job.size_mm.0 - 41.0).abs() < 0.05);
    }

    #[test]
    fn a_cut_out_is_cut_on_the_inside_and_before_the_board() {
        let with_hole = OUTLINE.replace(
            "M02*",
            "X100000Y100000D02*\nX200000Y100000D01*\nX200000Y200000D01*\nX100000Y200000D01*\nX100000Y100000D01*\nM02*",
        );
        let job = run("edge.gko", &with_hole, Ask::default()).unwrap();
        assert_eq!(job.cuts, 2);
        // the first path is the cut-out: 10 x 10 at (10, 10), the tool half a millimetre inside it
        let first = job
            .lines
            .iter()
            .position(|l| l.starts_with("G0 X"))
            .unwrap();
        let line = &job.lines[first];
        let x = values(std::slice::from_ref(line), 'X')[0];
        assert!((10.4..=19.6).contains(&x), "{line}");
    }

    #[test]
    fn an_outline_that_is_not_closed_is_cut_as_it_is_and_says_so() {
        let open = OUTLINE.replace("X0Y0D01*\n", "");
        let job = run("edge.gko", &open, Ask::default()).unwrap();
        assert!(job.notes.contains(&Note::PcbOutlineOpen));
        assert_eq!(job.cuts, 1);
    }

    #[test]
    fn a_drill_file_becomes_the_holes_tool_after_tool_with_the_pecks_of_the_setting() {
        let job = run("a.drl", DRILL, Ask::default()).unwrap();
        assert_eq!((job.kind, job.cuts, job.tools), (Kind::Drill, 3, 2));
        // the smaller drill first, a pause to change it, then the bigger
        let pauses: Vec<&String> = job.lines.iter().filter(|l| l.starts_with("M0 ")).collect();
        assert_eq!(pauses.len(), 2);
        assert!(
            pauses[0].contains("0.8") && pauses[1].contains("1 mm"),
            "{pauses:?}"
        );
        // 1.8 mm in pecks of 0.6: three plunges a hole, the last at the full depth, and a retract between them
        let plunges = job.lines.iter().filter(|l| l.starts_with("G1 Z-")).count();
        assert_eq!(plunges, 9);
        assert!(values(&job.lines, 'Z').contains(&-1.8));
        assert_eq!(job.lines.iter().filter(|l| *l == "G0 Z1").count(), 6);
        // the nearest hole next: (5, 5) then (10, 5)
        let moves: Vec<&String> = job.lines.iter().filter(|l| l.starts_with("G0 X")).collect();
        assert_eq!(moves[0], "G0 X5 Y5");
        assert_eq!(moves[1], "G0 X10 Y5");
        let (i, mut p) = cfgs();
        p.tool_change_m6 = true;
        let m6 = import_bytes("a.drl", DRILL.as_bytes(), &i, &p, &Ask::default()).unwrap();
        assert!(m6.lines.contains(&"T1 M6".to_string()) && m6.lines.contains(&"T2 M6".to_string()));
        assert!(!m6.lines.iter().any(|l| l.starts_with("M0 ")));
    }

    #[test]
    fn a_hole_with_no_tool_is_left_out_and_inches_are_converted() {
        let odd = "M48\nMETRIC,TZ\nT1C0.800\n%\nT1\nX5.0Y5.0\nT9\nX7.0Y7.0\nM30\n";
        let job = run("a.drl", odd, Ask::default()).unwrap();
        assert_eq!(job.cuts, 1);
        assert!(
            job.notes.contains(&Note::PcbHoleSkipped) || job.notes.contains(&Note::PcbNotDrawn)
        );
        let inch = "M48\nINCH,TZ\nT1C0.0315\n%\nT1\nX0.2Y0.4\nM30\n";
        let job = run("a.drl", inch, Ask::default()).unwrap();
        let moves: Vec<&String> = job.lines.iter().filter(|l| l.starts_with("G0 X")).collect();
        assert_eq!(moves[0], "G0 X5.08 Y10.16");
    }

    #[test]
    fn a_slot_is_routed_between_its_ends_pass_after_pass() {
        let mut file = excellon::parse_bytes(DRILL.as_bytes()).unwrap();
        file.drills.clear();
        file.slots.push(excellon::Slot {
            line: 1,
            tool: 2,
            from: excellon::Point { x: 5.0, y: 5.0 },
            to: excellon::Point { x: 15.0, y: 5.0 },
        });
        let (_, p) = cfgs();
        let mut notes = Vec::new();
        let done = drilling(&file, &p, 5.0, false, false, &mut notes).unwrap();
        // 1.8 mm in pecks of 0.6: three passes, along the slot and back and along again
        let along: Vec<&String> = done
            .lines
            .iter()
            .filter(|l| l.starts_with("G1 X"))
            .collect();
        assert_eq!(along, ["G1 X15 Y5", "G1 X5 Y5", "G1 X15 Y5"]);
        assert_eq!((done.cuts, done.tools), (1, 1));
        assert!(notes.is_empty());
    }

    #[test]
    fn what_is_not_a_cut_layer_or_not_a_board_file_is_a_failure_in_words() {
        let mask = COPPER.replace("Copper,L1,Top", "Soldermask,Top");
        assert_eq!(run("a.gts", &mask, Ask::default()), Err(Fail::NotACutLayer));
        // unless the operator says what to cut
        assert!(run(
            "a.gts",
            &mask,
            Ask {
                kind: Some(Kind::Isolation),
                mirror: false
            }
        )
        .is_ok());
        assert_eq!(
            run(
                "a.gbr",
                COPPER,
                Ask {
                    kind: Some(Kind::Drill),
                    mirror: false
                }
            ),
            Err(Fail::NotACutLayer)
        );
        assert_eq!(
            run(
                "a.drl",
                DRILL,
                Ask {
                    kind: Some(Kind::Profile),
                    mirror: false
                }
            ),
            Err(Fail::NotACutLayer)
        );
        assert_eq!(
            run("a.txt", "hello", Ask::default()),
            Err(Fail::NotRecognised)
        );
        let nothing = run("a.gbr", "%FSLAX34Y34*%\n%MOMM*%\nM02*\n", Ask::default());
        assert!(
            matches!(nothing, Err(Fail::Empty | Fail::Syntax)),
            "{nothing:?}"
        );
        let no_holes = run("a.drl", "M48\nMETRIC\n%\nM30\n", Ask::default());
        assert!(
            matches!(no_holes, Err(Fail::Empty | Fail::Syntax)),
            "{no_holes:?}"
        );
        let (mut i, p) = (ImportCfg::default(), PcbCfg::default());
        i.max_bytes = 1024;
        assert_eq!(
            import_bytes("a.gbr", &[b' '; 2000], &i, &p, &Ask::default()),
            Err(Fail::TooBig)
        );
        i.max_bytes = 8 * 1024 * 1024;
        i.max_points = 16;
        assert_eq!(
            import_bytes("e.gko", OUTLINE.as_bytes(), &i, &p, &Ask::default()).map(|j| j.cuts),
            Ok(1)
        );
        i.max_points = 1;
        assert_eq!(
            import_bytes("e.gko", OUTLINE.as_bytes(), &i, &p, &Ask::default()),
            Err(Fail::TooManyPoints)
        );
    }

    #[test]
    fn a_layer_the_file_does_not_name_is_taken_for_copper_and_says_so() {
        let plain = COPPER.replace("%TF.FileFunction,Copper,L1,Top*%\n", "");
        let job = run("plain.gbr", &plain, Ask::default()).unwrap();
        assert_eq!(job.kind, Kind::Isolation);
        assert!(job.notes.contains(&Note::PcbAssumedCopper));
    }

    #[test]
    fn made_up_bytes_from_a_fixed_seed_never_panic_and_always_end_in_an_answer() {
        let mut seed = eh3d_controller::wire::Seed::new(0xC0FFEE);
        let pieces = [
            "%FSLAX34Y34*%",
            "%MOMM*%",
            "%ADD10C,0.5*%",
            "D10*",
            "X1Y1D02*",
            "X5000Y5000D01*",
            "G02*",
            "I100J0D01*",
            "M48",
            "METRIC,TZ",
            "T1C0.8",
            "%",
            "T1",
            "X5.0Y5.0",
            "M30",
            "M02*",
            "garbage \u{0}\u{1}",
            "X99999999999D01*",
            "%TF.FileFunction,Profile*%",
        ];
        for _ in 0..60 {
            let n = 1 + seed.below(12);
            let mut text = String::new();
            for _ in 0..n {
                text.push_str(pieces[seed.below(pieces.len())]);
                text.push('\n');
            }
            for name in ["x.gbr", "x.gko", "x.drl", "x.txt"] {
                // no panic, and an answer: a program or a failure
                let _ = run(name, &text, Ask::default());
                let _ = run(
                    name,
                    &text,
                    Ask {
                        kind: Some(Kind::Profile),
                        mirror: true,
                    },
                );
            }
        }
    }
}
