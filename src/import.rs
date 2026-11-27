//! The importers: a drawing in HPGL (a plotter file), DXF (lines, polylines, circles and arcs) or SVG (lines, shapes and paths) is read
//! into paths of points in mm, and the paths are turned into a G-code program that cuts along them, pass by pass, for basic milling.
//! The readers are written for files from anywhere: each looks at no more than the limits of the motion file say (bytes and points),
//! expands nothing (an SVG entity or a DXF block is never expanded), uses no recursion, and says what it did not understand; a file that
//! is wrong ends in an [`Fail`] and never in a panic. Nothing here moves a machine.

use crate::motioncfg::ImportCfg;
use crate::notes::{Fail, Note};

/// A way to cut that the file asks for: a run of points in mm, closed or not.
#[derive(Debug, Clone, PartialEq)]
pub struct Path {
    pub points: Vec<(f64, f64)>,
    pub closed: bool,
}

/// What a file held.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Drawing {
    pub paths: Vec<Path>,
    /// What was not understood or was assumed, once each.
    pub notes: Vec<Note>,
}

/// The formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Hpgl,
    Dxf,
    Svg,
}

/// The biggest coordinate taken, in mm: past it a number is a mistake of the file.
pub const MAX_COORD_MM: f64 = 1.0e6;

impl Format {
    /// The format by the end of the name of the file, or by how the file starts.
    pub fn detect(name: &str, head: &[u8]) -> Option<Format> {
        let lower = name.to_ascii_lowercase();
        if lower.ends_with(".plt") || lower.ends_with(".hpgl") || lower.ends_with(".hgl") {
            return Some(Format::Hpgl);
        }
        if lower.ends_with(".dxf") {
            return Some(Format::Dxf);
        }
        if lower.ends_with(".svg") {
            return Some(Format::Svg);
        }
        let start = String::from_utf8_lossy(&head[..head.len().min(256)]).to_string();
        let t = start.trim_start();
        if t.starts_with("<?xml") || t.starts_with("<svg") {
            Some(Format::Svg)
        } else if t.starts_with("0\nSECTION") || t.starts_with("0\r\nSECTION") {
            Some(Format::Dxf)
        } else if t.starts_with("IN;") || t.starts_with("PA") || t.starts_with("SP") {
            Some(Format::Hpgl)
        } else {
            None
        }
    }
}

struct Sink<'a> {
    out: Drawing,
    points: usize,
    cfg: &'a ImportCfg,
}

impl<'a> Sink<'a> {
    fn new(cfg: &'a ImportCfg) -> Self {
        Sink {
            out: Drawing::default(),
            points: 0,
            cfg,
        }
    }

    fn note(&mut self, n: Note) {
        if !self.out.notes.contains(&n) {
            self.out.notes.push(n);
        }
    }

    fn path(&mut self, points: Vec<(f64, f64)>, closed: bool) -> Result<(), Fail> {
        let mut clean: Vec<(f64, f64)> = Vec::with_capacity(points.len());
        for p in points {
            if !(p.0.is_finite() && p.1.is_finite())
                || p.0.abs() > MAX_COORD_MM
                || p.1.abs() > MAX_COORD_MM
            {
                return Err(Fail::Syntax);
            }
            if clean
                .last()
                .is_none_or(|l| (l.0 - p.0).hypot(l.1 - p.1) > 1e-9)
            {
                clean.push(p);
            }
        }
        if clean.len() < 2 {
            return Ok(());
        }
        self.points += clean.len();
        if self.points > self.cfg.max_points {
            return Err(Fail::TooManyPoints);
        }
        self.out.paths.push(Path {
            points: clean,
            closed,
        });
        Ok(())
    }

    fn finish(self) -> Result<Drawing, Fail> {
        if self.out.paths.is_empty() {
            Err(Fail::Empty)
        } else {
            Ok(self.out)
        }
    }
}

/// Reads a drawing from the bytes of a file.
pub fn read(format: Format, bytes: &[u8], cfg: &ImportCfg) -> Result<Drawing, Fail> {
    if bytes.len() as u64 > cfg.max_bytes {
        return Err(Fail::TooBig);
    }
    let text = String::from_utf8_lossy(bytes);
    match format {
        Format::Hpgl => read_hpgl(&text, cfg),
        Format::Dxf => read_dxf(&text, cfg),
        Format::Svg => read_svg(&text, cfg),
    }
}

/// Reads a drawing from a file, looking at no more than `cfg.max_bytes` of it.
pub fn read_file(path: &std::path::Path, cfg: &ImportCfg) -> Result<Drawing, Fail> {
    use std::io::Read;
    let name = path.to_string_lossy();
    let f = std::fs::File::open(path).map_err(|_| Fail::Unreadable)?;
    let mut bytes = Vec::new();
    f.take(cfg.max_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Fail::Unreadable)?;
    let format = Format::detect(&name, &bytes).ok_or(Fail::NotRecognised)?;
    read(format, &bytes, cfg)
}

// ---------------------------------------------------------------- HPGL

/// A plotter unit is 0.025 mm (40 to the millimetre).
const HPGL_UNIT_MM: f64 = 0.025;

fn read_hpgl(text: &str, cfg: &ImportCfg) -> Result<Drawing, Fail> {
    let mut s = Sink::new(cfg);
    let (mut x, mut y) = (0.0f64, 0.0f64);
    let mut absolute = true;
    let mut down = false;
    let mut run: Vec<(f64, f64)> = Vec::new();
    for cmd in text.split([';', '\n']) {
        let cmd = cmd.trim();
        if cmd.len() < 2 || !cmd.is_char_boundary(2) {
            continue;
        }
        let (mn, args) = cmd.split_at(2);
        let mn = mn.to_ascii_uppercase();
        let nums: Vec<f64> = args
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|t| !t.is_empty())
            .filter_map(|t| t.parse::<f64>().ok())
            .filter(|v| v.is_finite())
            .collect();
        match mn.as_str() {
            "IN" | "DF" => {
                absolute = true;
                down = false;
            }
            "SP" | "LT" | "VS" | "FS" | "PW" | "IP" | "IW" | "PG" | "NR" => {}
            // arcs about a centre (`AA` absolute, `AR` from the pen) move the pen; a circle (`CI`) is drawn about it and leaves it there
            "AA" | "AR" | "CI" => {
                let written = args
                    .split(|c: char| c == ',' || c.is_whitespace())
                    .filter(|t| !t.is_empty())
                    .count();
                let needed = if mn == "CI" { 1 } else { 3 };
                if nums.len() != written || !(needed..=needed + 1).contains(&written) {
                    return Err(Fail::Syntax);
                }
                if mn == "CI" {
                    let r = nums[0].abs() * HPGL_UNIT_MM;
                    if r > 0.0 {
                        if r > MAX_COORD_MM {
                            return Err(Fail::Syntax);
                        }
                        let centre = (x * HPGL_UNIT_MM, y * HPGL_UNIT_MM);
                        let mut pts = flatten_arc(
                            centre,
                            r,
                            0.0,
                            std::f64::consts::TAU,
                            cfg.curve_tolerance_mm,
                        );
                        pts.pop();
                        s.path(pts, true)?;
                    }
                    continue;
                }
                let centre = if mn == "AA" {
                    (nums[0], nums[1])
                } else {
                    (x + nums[0], y + nums[1])
                };
                let r = (x - centre.0).hypot(y - centre.1);
                let start = (y - centre.1).atan2(x - centre.0);
                // a turn of more than a circle goes over the same path again
                let sweep = nums[2].clamp(-360.0, 360.0).to_radians();
                if !(r.is_finite() && r * HPGL_UNIT_MM <= MAX_COORD_MM) {
                    return Err(Fail::Syntax);
                }
                if r > 0.0 && sweep != 0.0 {
                    let pts = flatten_arc(
                        (centre.0 * HPGL_UNIT_MM, centre.1 * HPGL_UNIT_MM),
                        r * HPGL_UNIT_MM,
                        start,
                        sweep,
                        cfg.curve_tolerance_mm,
                    );
                    if down {
                        if run.is_empty() {
                            run.push((x * HPGL_UNIT_MM, y * HPGL_UNIT_MM));
                        }
                        run.extend(pts.iter().skip(1));
                        if run.len() > cfg.max_points {
                            return Err(Fail::TooManyPoints);
                        }
                    }
                    (x, y) = (
                        centre.0 + r * (start + sweep).cos(),
                        centre.1 + r * (start + sweep).sin(),
                    );
                }
            }
            // a scaling or a turn of the whole drawing, and the commands that move the pen in ways that are not read
            "SC" | "RO" | "PE" | "AT" | "BZ" | "BR" | "PM" => return Err(Fail::Unmeasured),
            "PA" | "PR" | "PU" | "PD" => {
                // a number left out would pair the ones after it the wrong way round, so the drawing is not taken
                let written = args
                    .split(|c: char| c == ',' || c.is_whitespace())
                    .filter(|t| !t.is_empty())
                    .count();
                if nums.len() != written || written % 2 != 0 {
                    return Err(Fail::Syntax);
                }
                match mn.as_str() {
                    "PA" => absolute = true,
                    "PR" => absolute = false,
                    "PU" => {
                        if down && run.len() > 1 {
                            s.path(std::mem::take(&mut run), false)?;
                        }
                        run.clear();
                        down = false;
                    }
                    _ => {
                        if !down {
                            down = true;
                            run.clear();
                            run.push((x * HPGL_UNIT_MM, y * HPGL_UNIT_MM));
                        }
                    }
                }
                for pair in nums.as_chunks::<2>().0 {
                    if absolute {
                        (x, y) = (pair[0], pair[1]);
                    } else {
                        x += pair[0];
                        y += pair[1];
                    }
                    if down {
                        run.push((x * HPGL_UNIT_MM, y * HPGL_UNIT_MM));
                        if run.len() > cfg.max_points {
                            return Err(Fail::TooManyPoints);
                        }
                    }
                }
            }
            _ => s.note(Note::UnsupportedCommand),
        }
    }
    if down && run.len() > 1 {
        s.path(run, false)?;
    }
    // a run that ends where it started is a closed shape
    for p in &mut s.out.paths {
        if p.points.len() > 2 {
            let (a, b) = (p.points[0], *p.points.last().unwrap());
            if (a.0 - b.0).hypot(a.1 - b.1) < 1e-6 {
                p.points.pop();
                p.closed = true;
            }
        }
    }
    s.note(Note::UnitsPlotter);
    s.finish()
}

// ---------------------------------------------------------------- DXF

/// Points of an arc of the circle `centre`, `r` from `a0` to `a1` radians (counter-clockwise), the pieces leaving the arc by no more than `tol`.
fn arc_points(c: (f64, f64), r: f64, a0: f64, a1: f64, tol: f64) -> Vec<(f64, f64)> {
    let mut sweep = a1 - a0;
    while sweep <= 0.0 {
        sweep += std::f64::consts::TAU;
    }
    flatten_arc(c, r, a0, sweep, tol)
}

fn flatten_arc(c: (f64, f64), r: f64, a0: f64, sweep: f64, tol: f64) -> Vec<(f64, f64)> {
    let r = r.abs();
    let piece = if tol >= r {
        std::f64::consts::FRAC_PI_2
    } else {
        2.0 * (1.0 - tol / r).clamp(-1.0, 1.0).acos()
    };
    let n = if piece > 1e-9 {
        ((sweep.abs() / piece).ceil() as usize).clamp(1, 4096)
    } else {
        4096
    };
    (0..=n)
        .map(|k| {
            let a = a0 + sweep * k as f64 / n as f64;
            (c.0 + r * a.cos(), c.1 + r * a.sin())
        })
        .collect()
}

/// The points of a polyline segment with a bulge (tan of a quarter of the angle it sweeps) from `p` to `q`.
fn bulge_points(p: (f64, f64), q: (f64, f64), bulge: f64, tol: f64) -> Vec<(f64, f64)> {
    if bulge.abs() < 1e-9 || !bulge.is_finite() {
        return vec![q];
    }
    let (dx, dy) = (q.0 - p.0, q.1 - p.1);
    let d = dx.hypot(dy);
    if d < 1e-12 {
        return vec![q];
    }
    let theta = 4.0 * bulge.atan();
    let r = d / (2.0 * (theta / 2.0).sin().abs());
    let h = r * (theta / 2.0).cos();
    let (mx, my) = ((p.0 + q.0) / 2.0, (p.1 + q.1) / 2.0);
    // the centre is on the left of the chord for a counter-clockwise (positive) bulge
    let side = if bulge > 0.0 { 1.0 } else { -1.0 };
    let c = (mx - side * h * dy / d, my + side * h * dx / d);
    let a0 = (p.1 - c.1).atan2(p.0 - c.0);
    let mut pts = flatten_arc(c, r, a0, theta, tol);
    pts.remove(0);
    if let Some(last) = pts.last_mut() {
        *last = q;
    }
    pts
}

fn read_dxf(text: &str, cfg: &ImportCfg) -> Result<Drawing, Fail> {
    let mut s = Sink::new(cfg);
    let mut it = text.lines();
    let mut pairs: Vec<(i32, &str)> = Vec::new();
    while let (Some(c), Some(v)) = (it.next(), it.next()) {
        let code: i32 = c.trim().parse().map_err(|_| Fail::Syntax)?;
        pairs.push((code, v.trim()));
        if pairs.len() > cfg.max_points.saturating_mul(16) {
            return Err(Fail::TooManyPoints);
        }
    }
    if pairs.is_empty() {
        return Err(Fail::NotRecognised);
    }
    // the unit the file says (INSUNITS: 1 inch, 4 mm, 5 cm, 6 m); nothing said is mm
    let mut scale = 1.0;
    let mut said_units = false;
    for w in pairs.windows(2) {
        if w[0] == (9, "$INSUNITS") && w[1].0 == 70 {
            said_units = true;
            scale = match w[1].1.parse::<i32>().map_err(|_| Fail::Syntax)? {
                0 => {
                    said_units = false;
                    1.0
                }
                1 => 25.4,
                2 => 304.8,
                4 => 1.0,
                5 => 10.0,
                6 => 1000.0,
                8 => 0.000_025_4,
                9 => 0.025_4,
                10 => 914.4,
                13 => 0.001,
                14 => 100.0,
                _ => return Err(Fail::Unmeasured),
            };
        }
    }
    if !said_units {
        s.note(Note::UnitsAssumedMm);
    }
    // only the section of the entities is read
    let mut i = 0;
    let mut in_entities = false;
    while i < pairs.len() {
        match pairs[i] {
            (0, "SECTION") if pairs.get(i + 1) == Some(&(2, "ENTITIES")) => {
                in_entities = true;
                i += 2;
                continue;
            }
            (0, "ENDSEC") => in_entities = false,
            _ => {}
        }
        if !(in_entities && pairs[i].0 == 0) {
            i += 1;
            continue;
        }
        let kind = pairs[i].1;
        let mut j = i + 1;
        while j < pairs.len() && pairs[j].0 != 0 {
            j += 1;
        }
        let body = &pairs[i + 1..j];
        // what is drawn on a sheet of paper (a frame, a title) is not part of the work
        if body.iter().any(|p| p.0 == 67 && p.1 == "1") {
            s.note(Note::UnsupportedCommand);
            i = j;
            continue;
        }
        let get = |code: i32| value(body, code);
        let need = |code: i32| value(body, code)?.ok_or(Fail::Syntax);
        let bulged = matches!(kind, "CIRCLE" | "ARC" | "LWPOLYLINE" | "POLYLINE");
        // a shape seen from behind (extrusion -Z) is mirrored in X; one in a tilted plane is not read
        let mut flip = 1.0;
        if bulged {
            let (nx, ny, nz) = (get(210)?, get(220)?, get(230)?);
            if nx.unwrap_or(0.0).abs() > 1e-6 || ny.unwrap_or(0.0).abs() > 1e-6 {
                return Err(Fail::Unmeasured);
            }
            if nz.is_some_and(|z| z < 0.0) {
                flip = -1.0;
            }
        }
        let seen = move |pts: Vec<(f64, f64)>| -> Vec<(f64, f64)> {
            pts.into_iter().map(|(x, y)| (x * flip, y)).collect()
        };
        match kind {
            "LINE" => {
                let (x0, y0, x1, y1) = (need(10)?, need(20)?, need(11)?, need(21)?);
                s.path(
                    vec![(x0 * scale, y0 * scale), (x1 * scale, y1 * scale)],
                    false,
                )?;
                i = j;
            }
            "CIRCLE" => {
                let (x, y, r) = (need(10)?, need(20)?, need(40)?);
                if r > 0.0 {
                    let mut pts = flatten_arc(
                        (x * scale, y * scale),
                        r * scale,
                        0.0,
                        std::f64::consts::TAU,
                        cfg.curve_tolerance_mm,
                    );
                    pts.pop();
                    s.path(seen(pts), true)?;
                }
                i = j;
            }
            "ARC" => {
                let (x, y, r) = (need(10)?, need(20)?, need(40)?);
                let (a0, a1) = (need(50)?, need(51)?);
                if r > 0.0 {
                    let pts = arc_points(
                        (x * scale, y * scale),
                        r * scale,
                        a0.to_radians(),
                        a1.to_radians(),
                        cfg.curve_tolerance_mm,
                    );
                    s.path(seen(pts), false)?;
                }
                i = j;
            }
            "LWPOLYLINE" => {
                let closed = get(70)?.is_some_and(|f| (f as i64) & 1 == 1);
                let mut verts: Vec<((f64, f64), f64)> = Vec::new();
                let mut pending_x: Option<f64> = None;
                for &(code, v) in body {
                    if !matches!(code, 10 | 20 | 42) {
                        continue;
                    }
                    let number = number_of(v)?;
                    match code {
                        10 => {
                            if pending_x.replace(number).is_some() {
                                return Err(Fail::Syntax);
                            }
                        }
                        20 => {
                            let x = pending_x.take().ok_or(Fail::Syntax)?;
                            verts.push(((x * scale, number * scale), 0.0));
                        }
                        _ => {
                            if let Some(last) = verts.last_mut() {
                                last.1 = number;
                            }
                        }
                    }
                }
                if pending_x.is_some() {
                    return Err(Fail::Syntax);
                }
                s.path(
                    seen(polyline(&verts, closed, cfg.curve_tolerance_mm)),
                    closed,
                )?;
                i = j;
            }
            "POLYLINE" => {
                let closed = get(70)?.is_some_and(|f| (f as i64) & 1 == 1);
                let mut verts: Vec<((f64, f64), f64)> = Vec::new();
                let mut k = j;
                while k < pairs.len() && pairs[k] != (0, "SEQEND") {
                    if pairs[k] == (0, "VERTEX") {
                        let mut e = k + 1;
                        while e < pairs.len() && pairs[e].0 != 0 {
                            e += 1;
                        }
                        let vb = &pairs[k + 1..e];
                        let x = value(vb, 10)?.ok_or(Fail::Syntax)?;
                        let y = value(vb, 20)?.ok_or(Fail::Syntax)?;
                        let bulge = value(vb, 42)?.unwrap_or(0.0);
                        verts.push(((x * scale, y * scale), bulge));
                        k = e;
                    } else {
                        k += 1;
                    }
                }
                s.path(
                    seen(polyline(&verts, closed, cfg.curve_tolerance_mm)),
                    closed,
                )?;
                i = k;
            }
            "VERTEX" | "SEQEND" => i = j,
            _ => {
                s.note(Note::UnsupportedCommand);
                i = j;
            }
        }
    }
    s.finish()
}

/// A number of a DXF group; one that cannot be read refuses the file, so no shape is drawn from what is left.
fn number_of(text: &str) -> Result<f64, Fail> {
    text.parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or(Fail::Syntax)
}

/// The number of the first group `code` of an entity, if it has one.
fn value(body: &[(i32, &str)], code: i32) -> Result<Option<f64>, Fail> {
    body.iter()
        .find(|p| p.0 == code)
        .map(|p| number_of(p.1))
        .transpose()
}

fn polyline(verts: &[((f64, f64), f64)], closed: bool, tol: f64) -> Vec<(f64, f64)> {
    let mut out: Vec<(f64, f64)> = Vec::new();
    for (k, &(p, bulge)) in verts.iter().enumerate() {
        if k == 0 {
            out.push(p);
        }
        let next = if k + 1 < verts.len() {
            Some(verts[k + 1].0)
        } else if closed && verts.len() > 2 {
            Some(verts[0].0)
        } else {
            None
        };
        if let Some(q) = next {
            let pts = bulge_points(p, q, bulge, tol);
            out.extend(pts);
        }
    }
    if closed && out.len() > 2 {
        let (a, b) = (out[0], *out.last().unwrap());
        if (a.0 - b.0).hypot(a.1 - b.1) < 1e-9 {
            out.pop();
        }
    }
    out
}

// ---------------------------------------------------------------- SVG

/// The attributes of a tag: `name="value"` pairs (either kind of quote), the names in lower case.
fn attributes(tag: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let b: Vec<char> = tag.chars().collect();
    let mut i = 0;
    // skip the name of the element
    while i < b.len() && !b[i].is_whitespace() {
        i += 1;
    }
    while i < b.len() {
        while i < b.len() && (b[i].is_whitespace() || b[i] == '/') {
            i += 1;
        }
        let start = i;
        while i < b.len() && b[i] != '=' && !b[i].is_whitespace() && b[i] != '/' {
            i += 1;
        }
        let name: String = b[start..i].iter().collect();
        while i < b.len() && b[i].is_whitespace() {
            i += 1;
        }
        if i < b.len() && b[i] == '=' {
            i += 1;
            while i < b.len() && b[i].is_whitespace() {
                i += 1;
            }
            if i < b.len() && (b[i] == '"' || b[i] == '\'') {
                let q = b[i];
                i += 1;
                let vs = i;
                while i < b.len() && b[i] != q {
                    i += 1;
                }
                out.push((name.to_ascii_lowercase(), b[vs..i].iter().collect()));
                i += 1;
            }
        } else if name.is_empty() {
            i += 1;
        }
    }
    out
}

fn attr<'a>(a: &'a [(String, String)], name: &str) -> Option<&'a str> {
    a.iter().find(|p| p.0 == name).map(|p| p.1.as_str())
}

/// A length with its unit, in user units of the file (`px`, `mm`, `cm`, `in`, `pt`) as mm.
fn length_mm(v: &str) -> Option<f64> {
    let v = v.trim();
    let split = v
        .find(|c: char| c.is_ascii_alphabetic() || c == '%')
        .unwrap_or(v.len());
    let n: f64 = v[..split].trim().parse().ok()?;
    let k = match v[split..].trim() {
        "" | "px" => 25.4 / 96.0,
        "mm" => 1.0,
        "cm" => 10.0,
        "in" => 25.4,
        "pt" => 25.4 / 72.0,
        "pc" => 25.4 / 6.0,
        _ => return None,
    };
    (n.is_finite()).then_some(n * k)
}

struct Numbers<'a> {
    b: &'a [u8],
    i: usize,
}

impl Numbers<'_> {
    fn skip(&mut self) {
        while self.i < self.b.len()
            && (self.b[self.i].is_ascii_whitespace() || self.b[self.i] == b',')
        {
            self.i += 1;
        }
    }

    fn number(&mut self) -> Option<f64> {
        self.skip();
        let start = self.i;
        let b = self.b;
        let mut i = self.i;
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            i += 1;
        }
        let mut digits = 0;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
            digits += 1;
        }
        if i < b.len() && b[i] == b'.' {
            i += 1;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
                digits += 1;
            }
        }
        if digits == 0 {
            return None;
        }
        if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
            let mut j = i + 1;
            if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
                j += 1;
            }
            let ds = j;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            if j > ds {
                i = j;
            }
        }
        self.i = i;
        std::str::from_utf8(&b[start..i])
            .ok()?
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
    }

    fn flag(&mut self) -> Option<bool> {
        self.skip();
        match self.b.get(self.i) {
            Some(b'0') => {
                self.i += 1;
                Some(false)
            }
            Some(b'1') => {
                self.i += 1;
                Some(true)
            }
            _ => None,
        }
    }

    fn command(&mut self) -> Option<u8> {
        self.skip();
        let c = *self.b.get(self.i)?;
        if c.is_ascii_alphabetic() {
            self.i += 1;
            Some(c)
        } else {
            None
        }
    }

    fn at_end(&mut self) -> bool {
        self.skip();
        self.i >= self.b.len()
    }
}

fn flatten_cubic(p: [(f64, f64); 4], tol: f64, out: &mut Vec<(f64, f64)>) {
    // the number of pieces from the size of the control polygon: the error of a piece falls with the square of their number
    let d = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).hypot(a.1 - b.1);
    let poly = d(p[0], p[1]) + d(p[1], p[2]) + d(p[2], p[3]);
    let n = ((poly / (tol.max(1e-4) * 8.0).sqrt() / 2.0).ceil() as usize).clamp(2, 512);
    for k in 1..=n {
        let t = k as f64 / n as f64;
        let u = 1.0 - t;
        let x = u * u * u * p[0].0
            + 3.0 * u * u * t * p[1].0
            + 3.0 * u * t * t * p[2].0
            + t * t * t * p[3].0;
        let y = u * u * u * p[0].1
            + 3.0 * u * u * t * p[1].1
            + 3.0 * u * t * t * p[2].1
            + t * t * t * p[3].1;
        out.push((x, y));
    }
}

/// An elliptic arc of a path in the form of the end points, as points (the F.6 conversion of the SVG specification).
#[allow(clippy::too_many_arguments)]
fn svg_arc(
    from: (f64, f64),
    to: (f64, f64),
    rx: f64,
    ry: f64,
    rot_deg: f64,
    large: bool,
    sweep: bool,
    tol: f64,
) -> Vec<(f64, f64)> {
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx < 1e-12 || ry < 1e-12 || from == to {
        return vec![to];
    }
    let phi = rot_deg.to_radians();
    let (cp, sp) = (phi.cos(), phi.sin());
    let (dx, dy) = ((from.0 - to.0) / 2.0, (from.1 - to.1) / 2.0);
    let (x1, y1) = (cp * dx + sp * dy, -sp * dx + cp * dy);
    let lam = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
    if lam > 1.0 {
        let s = lam.sqrt();
        rx *= s;
        ry *= s;
    }
    let num = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let mut co = if den > 0.0 {
        (num / den).max(0.0).sqrt()
    } else {
        0.0
    };
    if large == sweep {
        co = -co;
    }
    let (cxp, cyp) = (co * rx * y1 / ry, -co * ry * x1 / rx);
    let c = (
        cp * cxp - sp * cyp + (from.0 + to.0) / 2.0,
        sp * cxp + cp * cyp + (from.1 + to.1) / 2.0,
    );
    let ang = |ux: f64, uy: f64, vx: f64, vy: f64| (ux * vy - uy * vx).atan2(ux * vx + uy * vy);
    let a0 = ang(1.0, 0.0, (x1 - cxp) / rx, (y1 - cyp) / ry);
    let mut d = ang(
        (x1 - cxp) / rx,
        (y1 - cyp) / ry,
        (-x1 - cxp) / rx,
        (-y1 - cyp) / ry,
    );
    if !sweep && d > 0.0 {
        d -= std::f64::consts::TAU;
    } else if sweep && d < 0.0 {
        d += std::f64::consts::TAU;
    }
    let r = rx.max(ry);
    let n = {
        let piece = if tol >= r {
            1.0
        } else {
            2.0 * (1.0 - tol / r).clamp(-1.0, 1.0).acos()
        };
        if piece > 1e-9 {
            ((d.abs() / piece).ceil() as usize).clamp(1, 4096)
        } else {
            4096
        }
    };
    (1..=n)
        .map(|k| {
            let a = a0 + d * k as f64 / n as f64;
            let (ex, ey) = (rx * a.cos(), ry * a.sin());
            (cp * ex - sp * ey + c.0, sp * ex + cp * ey + c.1)
        })
        .collect()
}

/// A sub-path of the data of a `d` attribute: its points and whether it is closed.
type Shape = (Vec<(f64, f64)>, bool);

/// The sub-paths of the data of a `d` attribute, in user units: (points, closed).
fn path_data(d: &str, tol: f64, budget: &mut usize) -> Result<Vec<Shape>, Fail> {
    let mut n = Numbers {
        b: d.as_bytes(),
        i: 0,
    };
    let mut out: Vec<Shape> = Vec::new();
    let mut cur: Vec<(f64, f64)> = Vec::new();
    let (mut x, mut y) = (0.0, 0.0);
    let (mut sx, mut sy) = (0.0, 0.0);
    let mut cmd = b'M';
    let mut last_ctrl: Option<(f64, f64)> = None;
    let mut last_quad: Option<(f64, f64)> = None;
    let bad = || Fail::Syntax;
    let mut first = true;
    let mut moved = false;
    loop {
        if n.at_end() {
            break;
        }
        if let Some(c) = n.command() {
            cmd = c;
            first = false;
            if matches!(cmd, b'Z' | b'z') {
                if cur.len() > 1 {
                    out.push((std::mem::take(&mut cur), true));
                }
                (x, y) = (sx, sy);
                last_ctrl = None;
                last_quad = None;
                continue;
            }
        } else if first {
            return Err(bad());
        }
        if *budget == 0 {
            return Err(Fail::TooManyPoints);
        }
        *budget -= 1;
        let rel = cmd.is_ascii_lowercase();
        let (ox, oy) = if rel { (x, y) } else { (0.0, 0.0) };
        if !cmd.eq_ignore_ascii_case(&b'M') {
            if !moved {
                return Err(bad());
            }
            // after a close (`Z`) the next piece begins where the closed one did
            if cur.is_empty() {
                cur.push((x, y));
            }
        }
        moved = true;
        let before = cur.len();
        let mut new_ctrl = None;
        let mut new_quad = None;
        match cmd.to_ascii_uppercase() {
            b'M' => {
                if cur.len() > 1 {
                    out.push((std::mem::take(&mut cur), false));
                }
                let (a, b) = (n.number().ok_or_else(bad)?, n.number().ok_or_else(bad)?);
                (x, y) = (ox + a, oy + b);
                (sx, sy) = (x, y);
                cur = vec![(x, y)];
                cmd = if rel { b'l' } else { b'L' };
            }
            b'L' => {
                let (a, b) = (n.number().ok_or_else(bad)?, n.number().ok_or_else(bad)?);
                (x, y) = (ox + a, oy + b);
                cur.push((x, y));
            }
            b'H' => {
                let a = n.number().ok_or_else(bad)?;
                x = ox + a;
                cur.push((x, y));
            }
            b'V' => {
                let b = n.number().ok_or_else(bad)?;
                y = oy + b;
                cur.push((x, y));
            }
            b'C' | b'S' => {
                let c1 = if cmd.eq_ignore_ascii_case(&b'C') {
                    let (a, b) = (n.number().ok_or_else(bad)?, n.number().ok_or_else(bad)?);
                    (ox + a, oy + b)
                } else {
                    // the reflection of the last control point
                    last_ctrl.map_or((x, y), |c| (2.0 * x - c.0, 2.0 * y - c.1))
                };
                let (a, b) = (n.number().ok_or_else(bad)?, n.number().ok_or_else(bad)?);
                let c2 = (ox + a, oy + b);
                let (a, b) = (n.number().ok_or_else(bad)?, n.number().ok_or_else(bad)?);
                let end = (ox + a, oy + b);
                flatten_cubic([(x, y), c1, c2, end], tol, &mut cur);
                new_ctrl = Some(c2);
                (x, y) = end;
            }
            b'Q' | b'T' => {
                let c = if cmd.eq_ignore_ascii_case(&b'Q') {
                    let (a, b) = (n.number().ok_or_else(bad)?, n.number().ok_or_else(bad)?);
                    (ox + a, oy + b)
                } else {
                    last_quad.map_or((x, y), |c| (2.0 * x - c.0, 2.0 * y - c.1))
                };
                let (a, b) = (n.number().ok_or_else(bad)?, n.number().ok_or_else(bad)?);
                let end = (ox + a, oy + b);
                // a quadratic is a cubic with the control points two thirds of the way
                let c1 = (x + 2.0 / 3.0 * (c.0 - x), y + 2.0 / 3.0 * (c.1 - y));
                let c2 = (
                    end.0 + 2.0 / 3.0 * (c.0 - end.0),
                    end.1 + 2.0 / 3.0 * (c.1 - end.1),
                );
                flatten_cubic([(x, y), c1, c2, end], tol, &mut cur);
                new_quad = Some(c);
                (x, y) = end;
            }
            b'A' => {
                let rx = n.number().ok_or_else(bad)?;
                let ry = n.number().ok_or_else(bad)?;
                let rot = n.number().ok_or_else(bad)?;
                let large = n.flag().ok_or_else(bad)?;
                let sweep = n.flag().ok_or_else(bad)?;
                let (a, b) = (n.number().ok_or_else(bad)?, n.number().ok_or_else(bad)?);
                let end = (ox + a, oy + b);
                cur.extend(svg_arc((x, y), end, rx, ry, rot, large, sweep, tol));
                (x, y) = end;
            }
            _ => return Err(bad()),
        }
        last_ctrl = new_ctrl;
        last_quad = new_quad;
        // the points a curve added count against the budget, so that a path of curves cannot grow without a limit
        *budget = budget.saturating_sub(cur.len().saturating_sub(before));
    }
    if cur.len() > 1 {
        out.push((cur, false));
    }
    Ok(out)
}

/// A transformation of SVG: `[a, b, c, d, e, f]` takes `(x, y)` to `(a x + c y + e, b x + d y + f)`.
type Matrix = [f64; 6];

const IDENTITY: Matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

fn apply(m: Matrix, p: (f64, f64)) -> (f64, f64) {
    (
        m[0] * p.0 + m[2] * p.1 + m[4],
        m[1] * p.0 + m[3] * p.1 + m[5],
    )
}

/// The transformation that does `inner` first and then `outer`.
fn compose(outer: Matrix, inner: Matrix) -> Matrix {
    let (ax, ay) = (
        outer[0] * inner[0] + outer[2] * inner[1],
        outer[1] * inner[0] + outer[3] * inner[1],
    );
    let (cx, cy) = (
        outer[0] * inner[2] + outer[2] * inner[3],
        outer[1] * inner[2] + outer[3] * inner[3],
    );
    let (ex, ey) = apply(outer, (inner[4], inner[5]));
    [ax, ay, cx, cy, ex, ey]
}

/// The value of a `transform` attribute: `translate`, `scale`, `rotate`, `matrix`, `skewX` and `skewY`, the one written first outermost.
/// Anything else refuses the file, which would otherwise be cut in another place.
fn transform_of(text: &str) -> Result<Matrix, Fail> {
    let mut out = IDENTITY;
    let mut rest = text.trim_matches(|c: char| c.is_whitespace() || c == ',');
    while !rest.is_empty() {
        let opening = rest.find('(').ok_or(Fail::Unmeasured)?;
        let closing = rest
            .find(')')
            .filter(|c| *c > opening)
            .ok_or(Fail::Unmeasured)?;
        let name = rest[..opening].trim();
        let mut numbers = Numbers {
            b: rest[opening + 1..closing].as_bytes(),
            i: 0,
        };
        let mut v: Vec<f64> = Vec::new();
        while !numbers.at_end() {
            v.push(numbers.number().ok_or(Fail::Unmeasured)?);
            if v.len() > 6 {
                return Err(Fail::Unmeasured);
            }
        }
        let one: Matrix = match (name, v.as_slice()) {
            ("matrix", &[a, b, c, d, e, f]) => [a, b, c, d, e, f],
            ("translate", &[x]) => [1.0, 0.0, 0.0, 1.0, x, 0.0],
            ("translate", &[x, y]) => [1.0, 0.0, 0.0, 1.0, x, y],
            ("scale", &[k]) => [k, 0.0, 0.0, k, 0.0, 0.0],
            ("scale", &[x, y]) => [x, 0.0, 0.0, y, 0.0, 0.0],
            ("rotate", &[a]) => {
                let (sin, cos) = a.to_radians().sin_cos();
                [cos, sin, -sin, cos, 0.0, 0.0]
            }
            ("rotate", &[a, x, y]) => {
                let (sin, cos) = a.to_radians().sin_cos();
                [
                    cos,
                    sin,
                    -sin,
                    cos,
                    x - cos * x + sin * y,
                    y - sin * x - cos * y,
                ]
            }
            ("skewX", &[a]) => [1.0, 0.0, a.to_radians().tan(), 1.0, 0.0, 0.0],
            ("skewY", &[a]) => [1.0, a.to_radians().tan(), 0.0, 1.0, 0.0, 0.0],
            _ => return Err(Fail::Unmeasured),
        };
        out = compose(out, one);
        rest = rest[closing + 1..].trim_start_matches(|c: char| c.is_whitespace() || c == ',');
    }
    Ok(out)
}

/// Whether an element says it is not shown, by its attributes or by its style.
fn is_hidden(a: &[(String, String)]) -> bool {
    let style: String = attr(a, "style")
        .unwrap_or("")
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    attr(a, "display").is_some_and(|v| v.trim() == "none")
        || attr(a, "visibility").is_some_and(|v| v.trim() == "hidden")
        || style
            .split(';')
            .any(|d| d == "display:none" || d == "visibility:hidden")
}

/// The outline of a rectangle `[x, y, width, height]`, with its corners rounded by the radii when both are above zero.
fn rect_points(r: [f64; 4], rx: f64, ry: f64, tol: f64) -> Vec<(f64, f64)> {
    let [x, y, w, h] = r;
    if rx <= 0.0 || ry <= 0.0 {
        return vec![(x, y), (x + w, y), (x + w, y + h), (x, y + h)];
    }
    let big = rx.max(ry);
    let piece = if tol >= big {
        std::f64::consts::FRAC_PI_2
    } else {
        2.0 * (1.0 - tol / big).clamp(-1.0, 1.0).acos()
    };
    let n = if piece > 1e-9 {
        ((std::f64::consts::FRAC_PI_2 / piece).ceil() as usize).clamp(1, 1024)
    } else {
        1024
    };
    // the centres of the corners, clockwise on the screen from the top right one, and the angle each begins at
    let corners = [
        (x + w - rx, y + ry, -1.0),
        (x + w - rx, y + h - ry, 0.0),
        (x + rx, y + h - ry, 1.0),
        (x + rx, y + ry, 2.0),
    ];
    let mut out = Vec::with_capacity(4 * (n + 1));
    for (cx, cy, quarter) in corners {
        for k in 0..=n {
            let angle = std::f64::consts::FRAC_PI_2 * (quarter + k as f64 / n as f64);
            out.push((cx + rx * angle.cos(), cy + ry * angle.sin()));
        }
    }
    out
}

fn read_svg(text: &str, cfg: &ImportCfg) -> Result<Drawing, Fail> {
    let mut s = Sink::new(cfg);
    let tol = cfg.curve_tolerance_mm;
    let mut scale = 25.4 / 96.0; // user unit to mm
    let mut height_units = 0.0f64;
    let mut have_root = false;
    let mut skip_depth = 0usize;
    let mut budget = cfg.max_points;
    let mut shapes: Vec<(Vec<(f64, f64)>, bool)> = Vec::new();
    // the elements that are open: the transformation in force inside each one and whether it is hidden
    let mut open: Vec<(Matrix, bool)> = Vec::new();
    let mut rest = text;
    let mut tags = 0usize;
    while let Some(lt) = rest.find('<') {
        rest = &rest[lt..];
        if rest.starts_with("<!--") {
            rest = &rest[rest.find("-->").map_or(rest.len(), |e| e + 3)..];
            continue;
        }
        if rest.starts_with("<![CDATA[") {
            rest = &rest[rest.find("]]>").map_or(rest.len(), |e| e + 3)..];
            continue;
        }
        let Some(gt) = rest.find('>') else { break };
        let inner = &rest[1..gt];
        rest = &rest[gt + 1..];
        tags += 1;
        if tags > 2_000_000 {
            return Err(Fail::TooManyPoints);
        }
        if inner.starts_with('?') || inner.starts_with('!') {
            continue;
        }
        let closing = inner.starts_with('/');
        let selfclosing = inner.ends_with('/');
        let name_src = inner.trim_start_matches('/');
        let name: String = name_src
            .chars()
            .take_while(|c| !c.is_whitespace() && *c != '/')
            .collect::<String>()
            .to_ascii_lowercase();
        let name = name.rsplit(':').next().unwrap_or("").to_string();
        let hidden = matches!(
            name.as_str(),
            "defs"
                | "clippath"
                | "mask"
                | "symbol"
                | "pattern"
                | "marker"
                | "metadata"
                | "title"
                | "desc"
                | "style"
                | "script"
                | "foreignobject"
        );
        if hidden {
            if closing {
                skip_depth = skip_depth.saturating_sub(1);
            } else if !selfclosing {
                skip_depth += 1;
            }
            continue;
        }
        if skip_depth > 0 {
            continue;
        }
        if closing {
            open.pop();
            continue;
        }
        let a = attributes(inner);
        let (outer, outer_hidden) = open.last().copied().unwrap_or((IDENTITY, false));
        let matrix = match attr(&a, "transform") {
            Some(t) => compose(outer, transform_of(t)?),
            None => outer,
        };
        // what is not shown (a hidden layer) is not cut
        let hidden = outer_hidden || is_hidden(&a);
        if !selfclosing {
            open.push((matrix, hidden));
        }
        if hidden {
            continue;
        }
        let f = |name: &str| -> Result<Option<f64>, Fail> {
            attr(&a, name)
                .map(|v| {
                    let v = v.trim();
                    number_of(v.strip_suffix("px").unwrap_or(v).trim_end())
                })
                .transpose()
        };
        let stretch = matrix[0]
            .hypot(matrix[1])
            .max(matrix[2].hypot(matrix[3]))
            .max(1e-9);
        let first_new = shapes.len();
        match name.as_str() {
            "svg" if !have_root => {
                have_root = true;
                let vb: Vec<f64> = attr(&a, "viewbox")
                    .map(|v| {
                        v.split(|c: char| c == ',' || c.is_whitespace())
                            .filter_map(|t| t.parse::<f64>().ok())
                            .collect()
                    })
                    .unwrap_or_default();
                let w = attr(&a, "width").and_then(length_mm);
                if vb.len() == 4 && vb[2] > 0.0 {
                    if let Some(w) = w {
                        scale = w / vb[2];
                    }
                    height_units = vb[1] + vb[3];
                } else if let Some(h) = attr(&a, "height").and_then(length_mm) {
                    height_units = h / scale;
                }
                if w.is_none() && vb.len() != 4 {
                    s.note(Note::UnitsAssumedPixels);
                }
            }
            "line" => {
                if let (Some(x1), Some(y1), Some(x2), Some(y2)) =
                    (f("x1")?, f("y1")?, f("x2")?, f("y2")?)
                {
                    shapes.push((vec![(x1, y1), (x2, y2)], false));
                }
            }
            "rect" => {
                if let (Some(w), Some(h)) = (f("width")?, f("height")?) {
                    let (x, y) = (f("x")?.unwrap_or(0.0), f("y")?.unwrap_or(0.0));
                    if w > 0.0 && h > 0.0 {
                        let (rx, ry) = (f("rx")?, f("ry")?);
                        let rx = rx.or(ry).unwrap_or(0.0).clamp(0.0, w / 2.0);
                        let ry = ry.unwrap_or(rx).clamp(0.0, h / 2.0);
                        let pts = rect_points([x, y, w, h], rx, ry, tol / scale / stretch);
                        budget = budget.checked_sub(pts.len()).ok_or(Fail::TooManyPoints)?;
                        shapes.push((pts, true));
                    }
                }
            }
            "circle" | "ellipse" => {
                let (cx, cy) = (f("cx")?.unwrap_or(0.0), f("cy")?.unwrap_or(0.0));
                let (rx, ry) = if name == "circle" {
                    let r = f("r")?.unwrap_or(0.0);
                    (r, r)
                } else {
                    (f("rx")?.unwrap_or(0.0), f("ry")?.unwrap_or(0.0))
                };
                if rx > 0.0 && ry > 0.0 {
                    let u = tol / scale / stretch;
                    let r = rx.max(ry);
                    let piece = if u >= r {
                        1.0
                    } else {
                        2.0 * (1.0 - u / r).clamp(-1.0, 1.0).acos()
                    };
                    let n = if piece > 1e-9 {
                        ((std::f64::consts::TAU / piece).ceil() as usize).clamp(8, 4096)
                    } else {
                        4096
                    };
                    if n > budget {
                        return Err(Fail::TooManyPoints);
                    }
                    budget -= n;
                    shapes.push((
                        (0..n)
                            .map(|k| {
                                let t = std::f64::consts::TAU * k as f64 / n as f64;
                                (cx + rx * t.cos(), cy + ry * t.sin())
                            })
                            .collect(),
                        true,
                    ));
                }
            }
            "polyline" | "polygon" => {
                if let Some(v) = attr(&a, "points") {
                    let mut n = Numbers {
                        b: v.as_bytes(),
                        i: 0,
                    };
                    let mut pts = Vec::new();
                    while !n.at_end() {
                        // a point that cannot be read, or half a point, refuses the file
                        let px = n.number().ok_or(Fail::Syntax)?;
                        let py = n.number().ok_or(Fail::Syntax)?;
                        pts.push((px, py));
                        if pts.len() > budget {
                            return Err(Fail::TooManyPoints);
                        }
                    }
                    budget -= pts.len().min(budget);
                    shapes.push((pts, name == "polygon"));
                }
            }
            "path" => {
                if let Some(d) = attr(&a, "d") {
                    shapes.extend(path_data(d, tol / scale / stretch, &mut budget)?);
                }
            }
            "g" | "a" | "switch" | "use" | "image" | "text" | "tspan" | "linearGradient"
            | "radialGradient" | "stop" | "filter" => {
                if matches!(name.as_str(), "use" | "image" | "text") {
                    s.note(Note::UnsupportedCommand);
                }
            }
            _ => {}
        }
        for (pts, _) in shapes.iter_mut().skip(first_new) {
            for p in pts.iter_mut() {
                *p = apply(matrix, *p);
            }
        }
    }
    // the picture is upside down for a machine: Y grows up
    for (pts, closed) in shapes {
        let pts: Vec<(f64, f64)> = pts
            .into_iter()
            .map(|(x, y)| (x * scale, (height_units - y) * scale))
            .collect();
        s.path(pts, closed)?;
    }
    s.finish()
}

// ---------------------------------------------------------------- toolpaths

/// The bounds of a drawing: (min x, min y, max x, max y).
pub fn bounds(d: &Drawing) -> Option<(f64, f64, f64, f64)> {
    let mut it = d.paths.iter().flat_map(|p| p.points.iter());
    let first = it.next()?;
    let mut b = (first.0, first.1, first.0, first.1);
    for p in it {
        b = (b.0.min(p.0), b.1.min(p.1), b.2.max(p.0), b.3.max(p.1));
    }
    Some(b)
}

/// Moves the drawing so that its lower left corner is at the zero of the work.
pub fn to_origin(d: &mut Drawing) {
    if let Some((x0, y0, _, _)) = bounds(d) {
        for p in d.paths.iter_mut().flat_map(|p| p.points.iter_mut()) {
            p.0 -= x0;
            p.1 -= y0;
        }
    }
}

pub(crate) fn num(v: f64) -> String {
    let t = format!("{v:.3}");
    let t = t.trim_end_matches('0').trim_end_matches('.');
    if t.is_empty() || t == "-0" {
        "0".to_string()
    } else {
        t.to_string()
    }
}

/// The G-code that cuts along the paths of a drawing, pass by pass down to the depth of the settings.
pub fn to_gcode(d: &Drawing, cfg: &ImportCfg) -> Vec<String> {
    let mut g = vec![
        "G21 G90 G17".to_string(),
        format!("G0 Z{}", num(cfg.safe_z_mm)),
    ];
    let passes = ((cfg.depth_mm / cfg.pass_mm).ceil() as usize).clamp(1, 1000);
    for p in &d.paths {
        let first = p.points[0];
        g.push(format!("G0 X{} Y{}", num(first.0), num(first.1)));
        for k in 1..=passes {
            let depth = (cfg.pass_mm * k as f64).min(cfg.depth_mm);
            g.push(format!("G1 Z{} F{}", num(-depth), num(cfg.plunge_mm_min)));
            let mut feed = Some(cfg.feed_mm_min);
            for q in &p.points[1..] {
                let f = feed
                    .take()
                    .map_or(String::new(), |f| format!(" F{}", num(f)));
                g.push(format!("G1 X{} Y{}{f}", num(q.0), num(q.1)));
            }
            if p.closed {
                g.push(format!("G1 X{} Y{}", num(first.0), num(first.1)));
            } else if k < passes {
                // an open path is cut from its start every time
                g.push(format!("G0 Z{}", num(cfg.safe_z_mm)));
                g.push(format!("G0 X{} Y{}", num(first.0), num(first.1)));
            }
        }
        g.push(format!("G0 Z{}", num(cfg.safe_z_mm)));
    }
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> ImportCfg {
        ImportCfg::default()
    }

    #[test]
    fn hpgl_paths_are_read_in_plotter_units_absolute_and_relative() {
        let d = read(
            Format::Hpgl,
            b"IN;SP1;PU0,0;PD400,0,400,400,0,400,0,0;PU;PU800,0;PR;PD40,0,0,40;PU;",
            &cfg(),
        )
        .unwrap();
        assert_eq!(d.paths.len(), 2);
        // 400 units are 10 mm; the first goes round and ends where it began, so it is closed
        assert!(d.paths[0].closed);
        assert_eq!(d.paths[0].points.len(), 4);
        assert_eq!(d.paths[0].points[1], (10.0, 0.0));
        // the second is relative from (800, 0) = 20 mm
        assert_eq!(
            d.paths[1].points,
            vec![(20.0, 0.0), (21.0, 0.0), (21.0, 1.0)]
        );
        assert!(d.notes.contains(&Note::UnitsPlotter));
        assert_eq!(read(Format::Hpgl, b"IN;SP1;", &cfg()), Err(Fail::Empty));
    }

    #[test]
    fn a_plotter_file_with_a_number_that_cannot_be_read_is_not_drawn_from_what_is_left() {
        for bad in [
            "PU0,0;PD10,x,20,20;",
            "PU0,0;PD10,10,20;",
            "PU0,0PD10,10;",
            "PA1e999,0;PD1,1;",
        ] {
            assert_eq!(
                read(Format::Hpgl, bad.as_bytes(), &cfg()).err(),
                Some(Fail::Syntax),
                "{bad}"
            );
        }
        assert!(read(Format::Hpgl, b"PU 0 0;PD 10,10,;\r\n", &cfg()).is_ok());
    }

    #[test]
    fn dxf_lines_polylines_circles_and_arcs_are_read_with_bulges_and_units() {
        let dxf =
            "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n\
0\nLINE\n8\n0\n10\n0\n20\n0\n11\n10\n21\n0\n\
0\nLWPOLYLINE\n90\n4\n70\n1\n10\n0\n20\n0\n10\n10\n20\n0\n10\n10\n20\n10\n10\n0\n20\n10\n\
0\nCIRCLE\n10\n50\n20\n50\n40\n5\n\
0\nARC\n10\n0\n20\n0\n40\n10\n50\n0\n51\n90\n\
0\nPOLYLINE\n70\n0\n0\nVERTEX\n10\n0\n20\n0\n0\nVERTEX\n10\n5\n20\n5\n0\nSEQEND\n\
0\nSPLINE\n10\n0\n20\n0\n\
0\nENDSEC\n0\nEOF\n";
        let d = read(Format::Dxf, dxf.as_bytes(), &cfg()).unwrap();
        assert_eq!(d.paths.len(), 5);
        assert!(d.paths[1].closed && d.paths[1].points.len() == 4);
        // the circle is closed and its points are all 5 mm from the centre
        let c = &d.paths[2];
        assert!(c.closed);
        assert!(c
            .points
            .iter()
            .all(|p| ((p.0 - 50.0).hypot(p.1 - 50.0) - 5.0).abs() < 1e-6));
        // the arc goes from (10, 0) to (0, 10)
        let a = &d.paths[3];
        assert!((a.points[0].0 - 10.0).abs() < 1e-6 && a.points.last().unwrap().1 > 9.99);
        assert!(d.notes.contains(&Note::UnsupportedCommand));
        assert!(!d.notes.contains(&Note::UnitsAssumedMm));
        // inches say so
        let inches = dxf.replacen("$INSUNITS\n70\n4", "$INSUNITS\n70\n1", 1);
        let d2 = read(Format::Dxf, inches.as_bytes(), &cfg()).unwrap();
        assert!((d2.paths[0].points[1].0 - 254.0).abs() < 1e-9);
    }

    #[test]
    fn a_bulge_makes_the_arc_the_sign_says() {
        // a bulge of 1 is a half circle: from (0,0) to (10,0) counter-clockwise it bows below the chord
        let pts = bulge_points((0.0, 0.0), (10.0, 0.0), 1.0, 0.01);
        assert!(pts.iter().any(|p| p.1 < -4.9));
        let neg = bulge_points((0.0, 0.0), (10.0, 0.0), -1.0, 0.01);
        assert!(neg.iter().any(|p| p.1 > 4.9));
        assert_eq!(*pts.last().unwrap(), (10.0, 0.0));
        assert_eq!(
            bulge_points((0.0, 0.0), (1.0, 0.0), 0.0, 0.01),
            vec![(1.0, 0.0)]
        );
    }

    #[test]
    fn svg_shapes_and_paths_are_read_in_mm_with_y_up() {
        let svg = r#"<?xml version="1.0"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="50mm" viewBox="0 0 100 50">
<defs><path d="M0 0 L99 99"/></defs>
<rect x="10" y="10" width="20" height="10"/>
<circle cx="60" cy="25" r="5"/>
<path d="M 0 0 L 10 0 l 0 10 H 0 Z"/>
<path d="M0 50 C 10 40 20 40 30 50"/>
<path d="M 40 10 a 5 5 0 0 1 10 0"/>
<polyline points="0,0 5,5 10,0"/>
<text>hi</text>
</svg>"#;
        let d = read(Format::Svg, svg.as_bytes(), &cfg()).unwrap();
        // the rect, the circle, the path, the curve, the arc and the polyline (the one in defs is not drawn)
        assert_eq!(
            d.paths.len(),
            6,
            "{:?}",
            d.paths.iter().map(|p| p.points.len()).collect::<Vec<_>>()
        );
        // the rect: y = 10..20 down is 40..30 up
        let r = &d.paths[0];
        assert!(r.closed);
        assert!(r.points.contains(&(10.0, 40.0)) && r.points.contains(&(30.0, 30.0)));
        assert!(d.paths[1]
            .points
            .iter()
            .all(|p| ((p.0 - 60.0).hypot(p.1 - 25.0) - 5.0).abs() < 1e-6));
        assert!(d.paths[2].closed);
        // the curve ends where it said, at (30, 0)
        let curve = &d.paths[3];
        let end = curve.points.last().unwrap();
        assert!((end.0 - 30.0).abs() < 1e-9 && end.1.abs() < 1e-9);
        // the arc goes from (40, 40) to (50, 40) and bows up in the picture (y smaller), so up in Y
        let arc = &d.paths[4];
        assert!((arc.points.last().unwrap().0 - 50.0).abs() < 1e-6);
        assert!(d.notes.contains(&Note::UnsupportedCommand));
        assert!(d
            .paths
            .iter()
            .all(|p| p.points.iter().all(|q| q.0.is_finite() && q.1.is_finite())));
    }

    #[test]
    fn svg_path_data_in_every_form_of_number_and_command() {
        let mut b = 1000;
        let p = path_data(
            "M1e1,2 3.5.5-2 1 L4 4 q1 1 2 0 t2 0 s1 1 2 0 z m1 1 l1 0",
            0.01,
            &mut b,
        )
        .unwrap();
        assert!(!p.is_empty());
        // compact flags of an arc
        let mut b = 1000;
        assert!(path_data("M0 0a1 1 0 00 1 1", 0.01, &mut b).is_ok());
        // a command with too few numbers or a letter that is not one is a syntax failure
        let mut b = 1000;
        assert_eq!(path_data("M 1", 0.01, &mut b), Err(Fail::Syntax));
        let mut b = 1000;
        assert_eq!(path_data("M 0 0 X 1 1", 0.01, &mut b), Err(Fail::Syntax));
        let mut b = 1000;
        assert_eq!(path_data("1 2", 0.01, &mut b), Err(Fail::Syntax));
        // a budget of points is kept
        let mut b = 3;
        assert_eq!(
            path_data("M0 0 L1 1 L2 2 L3 3 L4 4", 0.01, &mut b),
            Err(Fail::TooManyPoints)
        );
    }

    fn near(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6
    }

    /// A DXF file with these entities, in millimetres unless another unit code is given.
    fn dxf(unit: &str, entities: &str) -> String {
        format!("0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n{unit}\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n{entities}0\nENDSEC\n0\nEOF\n")
    }

    const LINE: &str = "0\nLINE\n10\n0\n20\n0\n11\n1000\n21\n0\n";

    #[test]
    fn dxf_units_are_taken_as_they_are_said_or_the_file_is_refused() {
        for (unit, mm) in [
            ("4", 1000.0),
            ("9", 25.4),
            ("13", 1.0),
            ("14", 100_000.0),
            ("8", 0.0254),
        ] {
            let d = read(Format::Dxf, dxf(unit, LINE).as_bytes(), &cfg()).unwrap();
            assert!((d.paths[0].points[1].0 - mm).abs() < 1e-9, "{unit}");
        }
        // no unit: millimetres are taken and said
        let d = read(Format::Dxf, dxf("0", LINE).as_bytes(), &cfg()).unwrap();
        assert!(d.notes.contains(&Note::UnitsAssumedMm));
        for unit in ["3", "7", "11", "99"] {
            let e = read(Format::Dxf, dxf(unit, LINE).as_bytes(), &cfg());
            assert_eq!(e, Err(Fail::Unmeasured), "{unit}");
        }
    }

    #[test]
    fn dxf_shapes_seen_from_behind_are_mirrored_and_paper_is_not_cut() {
        let arc = "0\nARC\n10\n10\n20\n0\n40\n5\n50\n0\n51\n90\n230\n-1\n";
        let d = read(Format::Dxf, dxf("4", arc).as_bytes(), &cfg()).unwrap();
        assert!(
            near(d.paths[0].points[0], (-15.0, 0.0)),
            "{:?}",
            d.paths[0].points[0]
        );
        assert!(near(*d.paths[0].points.last().unwrap(), (-10.0, 5.0)));
        let paper = format!("{LINE}0\nLINE\n67\n1\n10\n0\n20\n0\n11\n9\n21\n9\n");
        let d = read(Format::Dxf, dxf("4", &paper).as_bytes(), &cfg()).unwrap();
        assert_eq!(d.paths.len(), 1);
        assert!(d.notes.contains(&Note::UnsupportedCommand));
        let tilted = "0\nCIRCLE\n10\n0\n20\n0\n40\n5\n210\n0.5\n";
        let e = read(Format::Dxf, dxf("4", tilted).as_bytes(), &cfg());
        assert_eq!(e, Err(Fail::Unmeasured));
    }

    #[test]
    fn a_dxf_shape_with_a_number_that_cannot_be_read_refuses_the_file() {
        for bad in [
            "0\nLINE\n10\n0\n20\n0\n11\nx\n21\n0\n",
            "0\nLINE\n10\n0\n20\n0\n11\n5\n",
            "0\nCIRCLE\n10\n0\n20\n0\n40\n5,5\n",
            "0\nLWPOLYLINE\n10\n0\n20\n0\n10\n5\n10\n5\n20\n5\n",
            "0\nLWPOLYLINE\n10\n0\n20\n0\n10\n5\n",
            "0\nPOLYLINE\n0\nVERTEX\n10\n0\n0\nSEQEND\n",
        ] {
            let text = dxf("4", &format!("{LINE}{bad}"));
            assert_eq!(
                read(Format::Dxf, text.as_bytes(), &cfg()),
                Err(Fail::Syntax),
                "{bad}"
            );
        }
    }

    /// An SVG drawing 100 mm square whose user units are millimetres.
    fn square(body: &str) -> String {
        format!(r#"<svg width="100mm" height="100mm" viewBox="0 0 100 100">{body}</svg>"#)
    }

    fn points(body: &str) -> Vec<Vec<(f64, f64)>> {
        let d = read(Format::Svg, square(body).as_bytes(), &cfg()).unwrap();
        d.paths.into_iter().map(|p| p.points).collect()
    }

    #[test]
    fn svg_transformations_of_groups_and_shapes_are_applied() {
        let line = r#"<line x1="0" y1="0" x2="10" y2="0"/>"#;
        let moved = points(&format!(
            r#"<g transform="translate(5, 10)">{line}</g>{line}"#
        ));
        assert!(
            near(moved[0][0], (5.0, 90.0)) && near(moved[0][1], (15.0, 90.0)),
            "{moved:?}"
        );
        // the group is over: the next line is where it was written
        assert!(near(moved[1][0], (0.0, 100.0)), "{moved:?}");
        // the one written first is outermost: the line is turned, then scaled, then moved
        let both = points(&format!(
            r#"<g transform="translate(10,0)"><g transform="scale(2)"><line transform="rotate(90)" x1="0" y1="0" x2="10" y2="0"/></g></g>"#
        ));
        assert!(near(both[0][1], (10.0, 80.0)), "{both:?}");
        let about =
            points(r#"<line transform="rotate(180 10 10)" x1="0" y1="10" x2="10" y2="10"/>"#);
        assert!(near(about[0][0], (20.0, 90.0)), "{about:?}");
        let matrix =
            points(r#"<line transform="matrix(1 0 0 1 3 4)" x1="0" y1="0" x2="1" y2="0"/>"#);
        assert!(near(matrix[0][0], (3.0, 96.0)), "{matrix:?}");
        for bad in ["perspective(3)", "translate(a)", "scale(1,2,3)", "rotate"] {
            let svg = square(&format!(r#"<g transform="{bad}">{line}</g>"#));
            assert_eq!(
                read(Format::Svg, svg.as_bytes(), &cfg()),
                Err(Fail::Unmeasured),
                "{bad}"
            );
        }
    }

    #[test]
    fn what_an_svg_does_not_show_is_not_cut() {
        let line = r#"<line x1="0" y1="0" x2="10" y2="0"/>"#;
        let body = format!(
            r#"<g style="display: none">{line}<g>{line}</g></g><g display="none">{line}</g><line visibility="hidden" x1="0" y1="0" x2="1" y2="0"/><line style="stroke:red;visibility:hidden" x1="0" y1="0" x2="1" y2="0"/>{line}"#
        );
        assert_eq!(points(&body).len(), 1);
    }

    #[test]
    fn an_svg_path_goes_on_from_its_start_after_a_close_and_a_round_rectangle_is_round() {
        let p = points(r#"<path d="M10 10 L20 10 L20 20 Z L0 0"/>"#);
        assert_eq!(p.len(), 2);
        assert!(
            near(p[1][0], (10.0, 90.0)) && near(p[1][1], (0.0, 100.0)),
            "{p:?}"
        );
        let r = points(r#"<rect x="10" y="10" width="20" height="10" rx="3"/>"#);
        // no point is on a corner of the sharp rectangle, and the sides keep their places
        assert!(r[0].len() > 8);
        assert!(r[0]
            .iter()
            .all(|q| !near(*q, (10.0, 90.0)) && !near(*q, (30.0, 80.0))));
        assert!(r[0].iter().any(|q| near(*q, (13.0, 90.0))));
        assert!(r[0].iter().any(|q| near(*q, (30.0, 87.0))));
        let d = read(
            Format::Svg,
            square(r#"<path d="L1 1 L2 2"/>"#).as_bytes(),
            &cfg(),
        );
        assert_eq!(d, Err(Fail::Syntax));
    }

    #[test]
    fn an_svg_shape_with_a_size_that_cannot_be_read_refuses_the_file() {
        for bad in [
            r#"<rect width="10mm" height="5"/>"#,
            r#"<circle cx="x" cy="0" r="5"/>"#,
            r#"<line x1="0" y1="0" x2="50%" y2="0"/>"#,
            r#"<polyline points="0,0 5,5 10"/>"#,
            r#"<polygon points="0,0 5,five 10,0"/>"#,
        ] {
            let svg = square(&format!(r#"<line x1="0" y1="0" x2="1" y2="0"/>{bad}"#));
            assert_eq!(
                read(Format::Svg, svg.as_bytes(), &cfg()),
                Err(Fail::Syntax),
                "{bad}"
            );
        }
        assert_eq!(points(r#"<rect width="10px" height="5 px"/>"#).len(), 1);
    }

    #[test]
    fn plotter_arcs_and_circles_are_drawn_and_the_pen_ends_where_they_end() {
        // 400 units are 10 mm: a quarter turn about (0, 0) from (10, 0), then a straight line from where it ended
        let d = read(Format::Hpgl, b"PU400,0;PD;AA0,0,90;PD0,0;PU;", &cfg()).unwrap();
        let p = &d.paths[0].points;
        assert!(near(p[0], (10.0, 0.0)));
        assert!(p.iter().any(|q| near(*q, (0.0, 10.0))), "{p:?}");
        assert!(near(*p.last().unwrap(), (0.0, 0.0)));
        // the relative centre is the same arc
        let r = read(Format::Hpgl, b"PU400,0;PD;AR-400,0,90;PD0,0;PU;", &cfg()).unwrap();
        assert_eq!(r.paths[0].points.len(), p.len());
        let c = read(Format::Hpgl, b"PU400,400;CI200;PU;", &cfg()).unwrap();
        assert!(c.paths[0].closed);
        assert!(c.paths[0]
            .points
            .iter()
            .all(|q| ((q.0 - 10.0).hypot(q.1 - 10.0) - 5.0).abs() < 1e-6));
        for bad in ["AA0,0;", "CI;", "AA0,x,90;"] {
            assert_eq!(
                read(Format::Hpgl, bad.as_bytes(), &cfg()),
                Err(Fail::Syntax),
                "{bad}"
            );
        }
        for scaled in ["SC0,100,0,100;PD1,1;", "RO90;PD1,1;", "PE=;", "PM0;PD1,1;"] {
            let e = read(Format::Hpgl, scaled.as_bytes(), &cfg());
            assert_eq!(e, Err(Fail::Unmeasured), "{scaled}");
        }
    }

    #[test]
    fn the_limits_hold_a_file_that_is_too_big_or_has_too_many_points_is_refused() {
        let small = ImportCfg {
            max_bytes: 64,
            ..cfg()
        };
        assert_eq!(read(Format::Hpgl, &[b'P'; 65], &small), Err(Fail::TooBig));
        let few = ImportCfg {
            max_points: 16,
            ..cfg()
        };
        let mut hpgl = String::from("PU0,0;PD");
        for i in 0..100 {
            hpgl.push_str(&format!("{},{},", i, i));
        }
        hpgl.push(';');
        assert_eq!(
            read(Format::Hpgl, hpgl.as_bytes(), &few),
            Err(Fail::TooManyPoints)
        );
        let svg = format!(r#"<svg><path d="M0 0{}"/></svg>"#, " L1 1".repeat(100));
        assert_eq!(
            read(Format::Svg, svg.as_bytes(), &few),
            Err(Fail::TooManyPoints)
        );
        // a huge circle asks for many points and is cut at the limit of the points
        let big = r#"<svg><circle cx="0" cy="0" r="100000"/></svg>"#;
        assert!(read(
            Format::Svg,
            big.as_bytes(),
            &ImportCfg {
                max_points: 100,
                ..cfg()
            }
        )
        .is_err());
        // a coordinate that is absurd is a failure of the file
        let far = r#"<svg><line x1="0" y1="0" x2="1e30" y2="0"/></svg>"#;
        assert_eq!(read(Format::Svg, far.as_bytes(), &cfg()), Err(Fail::Syntax));
    }

    #[test]
    fn an_entity_of_xml_is_never_expanded_and_a_bomb_of_them_costs_nothing() {
        let bomb = r#"<?xml version="1.0"?><!DOCTYPE svg [<!ENTITY a "aaaaaaaaaa"><!ENTITY b "&a;&a;&a;&a;&a;&a;&a;&a;&a;&a;">]><svg><path d="&b;&b;"/></svg>"#;
        assert!(read(Format::Svg, bomb.as_bytes(), &cfg()).is_err());
        let deep = format!(
            "<svg>{}{}</svg>",
            "<g>".repeat(100_000),
            "</g>".repeat(100_000)
        );
        assert_eq!(read(Format::Svg, deep.as_bytes(), &cfg()), Err(Fail::Empty));
    }

    #[test]
    fn the_formats_are_known_by_the_name_and_by_the_beginning_of_the_file() {
        assert_eq!(Format::detect("a.PLT", b""), Some(Format::Hpgl));
        assert_eq!(Format::detect("a.dxf", b""), Some(Format::Dxf));
        assert_eq!(Format::detect("a.svg", b""), Some(Format::Svg));
        assert_eq!(Format::detect("a", b"<?xml version"), Some(Format::Svg));
        assert_eq!(Format::detect("a", b"0\r\nSECTION\r\n"), Some(Format::Dxf));
        assert_eq!(Format::detect("a", b"IN;SP1;"), Some(Format::Hpgl));
        assert_eq!(Format::detect("a.txt", b"hello"), None);
    }

    #[test]
    fn a_drawing_becomes_a_program_that_cuts_pass_by_pass_and_goes_to_the_origin() {
        let mut d = Drawing {
            paths: vec![
                Path {
                    points: vec![(10.0, 10.0), (20.0, 10.0), (20.0, 20.0)],
                    closed: true,
                },
                Path {
                    points: vec![(0.0, 0.0), (5.0, 0.0)],
                    closed: false,
                },
            ],
            notes: vec![],
        };
        to_origin(&mut d);
        assert_eq!(bounds(&d), Some((0.0, 0.0, 20.0, 20.0)));
        let g = to_gcode(
            &d,
            &ImportCfg {
                depth_mm: 1.0,
                pass_mm: 0.5,
                ..cfg()
            },
        );
        assert_eq!(g[0], "G21 G90 G17");
        let plunges: Vec<&String> = g.iter().filter(|l| l.starts_with("G1 Z")).collect();
        assert_eq!(plunges.len(), 4); // two passes of two paths
        assert!(plunges[0].starts_with("G1 Z-0.5 F100") && plunges[1].starts_with("G1 Z-1 "));
        // the whole program is read by the path of the program
        let path = crate::gcode::toolpath(&g);
        assert!(path.iter().any(|s| !s.rapid));
        assert!(g.iter().all(|l| !l.contains("NaN")));
    }

    #[test]
    fn a_made_up_file_of_any_kind_never_panics() {
        let mut seed: u64 = 0xd1ce;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let pieces = [
            "<svg>",
            "</svg>",
            "<path d=\"",
            "M",
            "L",
            "C",
            "A",
            "a",
            "z",
            "1",
            "-2.5",
            "e",
            "\"/>",
            " ",
            ",",
            ".",
            "0\n",
            "SECTION\n",
            "ENTITIES\n",
            "LINE\n",
            "10\n",
            "LWPOLYLINE\n",
            "ENDSEC\n",
            "PD",
            "PU",
            ";",
            "PA",
            "<",
            ">",
            "<!--",
            "-->",
            "ü",
            "\u{0}",
            "9\n$INSUNITS\n70\n",
        ];
        for _ in 0..400 {
            let mut t = String::new();
            for _ in 0..(1 + next() % 40) {
                t.push_str(pieces[(next() % pieces.len() as u64) as usize]);
            }
            for f in [Format::Hpgl, Format::Dxf, Format::Svg] {
                if let Ok(d) = read(f, t.as_bytes(), &cfg()) {
                    let _ = to_gcode(&d, &cfg());
                }
            }
        }
        // bytes that are not text
        for f in [Format::Hpgl, Format::Dxf, Format::Svg] {
            let junk: Vec<u8> = (0..2000u32).map(|i| (i * 31 % 256) as u8).collect();
            let _ = read(f, &junk, &cfg());
        }
    }

    #[test]
    fn a_file_is_read_from_disk_with_the_limit_and_a_missing_one_is_a_failure() {
        let dir = std::env::temp_dir().join(format!("cnc_import_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("a.svg");
        std::fs::write(&f, r#"<svg width="10mm" height="10mm" viewBox="0 0 10 10"><line x1="0" y1="0" x2="10" y2="0"/></svg>"#).unwrap();
        assert!(read_file(&f, &cfg()).is_ok());
        assert_eq!(
            read_file(
                &f,
                &ImportCfg {
                    max_bytes: 10,
                    ..cfg()
                }
            ),
            Err(Fail::TooBig)
        );
        assert_eq!(
            read_file(&dir.join("none.svg"), &cfg()),
            Err(Fail::Unreadable)
        );
        let t = dir.join("a.txt");
        std::fs::write(&t, "hello").unwrap();
        assert_eq!(read_file(&t, &cfg()), Err(Fail::NotRecognised));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
