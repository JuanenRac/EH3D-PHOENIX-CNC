//! The materials of the shared database (`eh3d-materials` of EH3D-COMMON, the machining part of it) that a router works: the starting point of
//! a material for an operation and a cutter of a diameter (`--feeds`), the list of the materials with the points each has (`--materials`), the
//! chip load and the cutting speed that go with a point, and the material of the settings read from that database when the operator's own
//! library has none of that name. Nothing is extrapolated and nothing is called safe: the points of the database are starting values, the seed
//! of it has never been tried on a machine, and the output says so. Nothing here moves anything.

use std::sync::OnceLock;

use eh3d_materials::i18n::Lang;
use eh3d_materials::machining::{
    CncOp, CncPoint, CncStart, Family, MachiningDatabase, StartWarning, Tool, Workpiece,
};

use crate::words::{say, Msg};

/// Below this share of the diameter the chip of each tooth is so thin that the cutter rubs (percent of the diameter).
pub const CHIP_LOW_PCT: f64 = 0.2;
/// Above this share of the diameter the chip of each tooth is so thick that a small cutter may break (percent of the diameter).
pub const CHIP_HIGH_PCT: f64 = 8.0;

/// The operations and the cutters, in the order a request that does not name the cutter tries them.
pub const OPS: [CncOp; 4] = [CncOp::Contour, CncOp::Pocket, CncOp::Engrave, CncOp::Drill];
pub const TOOLS: [Tool; 5] = [
    Tool::EndMill,
    Tool::Compression,
    Tool::BallNose,
    Tool::VBit,
    Tool::Drill,
];

/// The database embedded in the program, read once. The crate checks it when it reads it; an error would be a bug of the data and is said.
pub fn database() -> Result<&'static MachiningDatabase, String> {
    static DB: OnceLock<Result<MachiningDatabase, String>> = OnceLock::new();
    DB.get_or_init(|| MachiningDatabase::embedded().map_err(|e| e.to_string()))
        .as_ref()
        .map_err(Clone::clone)
}

fn lang_of(code: &str) -> Lang {
    Lang::from_code(code).unwrap_or(Lang::En)
}

/// What the chip load of a point says about the cutter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChipVerdict {
    /// Thinner than `CHIP_LOW_PCT` of the diameter.
    Low,
    /// Between the two.
    Fine,
    /// Thicker than `CHIP_HIGH_PCT` of the diameter.
    High,
}

/// The thickness of the chip of each tooth in mm: the feed divided by the revolutions and by the number of teeth. `None` when a number is not
/// usable (no teeth, no speed, a value that is not a finite number above zero).
pub fn chip_load(feed_mm_min: f64, rpm: f64, flutes: u8) -> Option<f64> {
    let ok = |v: f64| v.is_finite() && v > 0.0;
    (ok(feed_mm_min) && ok(rpm) && flutes > 0).then(|| feed_mm_min / (rpm * f64::from(flutes)))
}

/// The speed of the edge of the cutter along the material in metres per minute.
pub fn surface_speed(diameter_mm: f64, rpm: f64) -> f64 {
    std::f64::consts::PI * diameter_mm * rpm / 1000.0
}

/// A chip load against the diameter of the cutter. A rule of thumb for small cutters, to read and not to obey.
pub fn chip_verdict(chip_mm: f64, diameter_mm: f64) -> ChipVerdict {
    if diameter_mm <= 0.0 || !chip_mm.is_finite() {
        return ChipVerdict::Fine;
    }
    let pct = chip_mm / diameter_mm * 100.0;
    if pct < CHIP_LOW_PCT {
        ChipVerdict::Low
    } else if pct > CHIP_HIGH_PCT {
        ChipVerdict::High
    } else {
        ChipVerdict::Fine
    }
}

/// A request: a material, and perhaps an operation, a diameter and a cutter.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub material: String,
    pub op: Option<CncOp>,
    pub diameter_mm: Option<f64>,
    pub tool: Option<Tool>,
}

/// Reads `MATERIAL[,OPERATION[,DIAMETER[,CUTTER]]]`. The error says what part is wrong.
pub fn parse_request(spec: &str) -> Result<Request, String> {
    let parts: Vec<&str> = spec.split(',').map(str::trim).collect();
    if parts.len() > 4 || parts[0].is_empty() || parts[0].len() > 80 {
        return Err(spec.trim().to_string());
    }
    let op = match parts.get(1).filter(|p| !p.is_empty()) {
        None => None,
        Some(p) => Some(
            OPS.into_iter()
                .find(|o| o.as_str().eq_ignore_ascii_case(p))
                .ok_or_else(|| (*p).to_string())?,
        ),
    };
    let diameter_mm = match parts.get(2).filter(|p| !p.is_empty()) {
        None => None,
        Some(p) => Some(
            p.parse::<f64>()
                .ok()
                .filter(|d| d.is_finite() && *d > 0.0 && *d <= 1000.0)
                .ok_or_else(|| (*p).to_string())?,
        ),
    };
    let tool = match parts.get(3).filter(|p| !p.is_empty()) {
        None => None,
        Some(p) => Some(
            TOOLS
                .into_iter()
                .find(|t| t.as_str().eq_ignore_ascii_case(p))
                .ok_or_else(|| (*p).to_string())?,
        ),
    };
    Ok(Request {
        material: parts[0].to_string(),
        op,
        diameter_mm,
        tool,
    })
}

/// The material a name stands for: the identifier (`wood/mdf`), what follows the slash of an identifier, or the name in any of the eight languages (the
/// capitals do not matter). The first in the order of the database when two have the same short name.
pub fn find<'a>(db: &'a MachiningDatabase, name: &str) -> Option<&'a Workpiece> {
    let n = name.trim();
    if n.is_empty() {
        return None;
    }
    if let Some(w) = db.get(&n.to_ascii_lowercase()) {
        return Some(w);
    }
    let low = n.to_lowercase();
    db.materials()
        .iter()
        .find(|w| w.id.rsplit('/').next().is_some_and(|s| s == low))
        .or_else(|| {
            db.materials().iter().find(|w| {
                Lang::ALL
                    .into_iter()
                    .any(|l| w.names.get(l).to_lowercase() == low)
            })
        })
}

/// The number as the tools write it: whole above 100, up to three decimals below.
fn num(v: f64) -> String {
    if v.abs() >= 100.0 {
        format!("{v:.0}")
    } else {
        let t = format!("{v:.3}");
        t.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn dash(v: Option<f64>) -> String {
    v.map_or_else(|| "-".to_string(), num)
}

/// What a tool prints, what it says on the other stream and its exit code (0 it went through, 2 it could not be answered).
pub struct Said {
    pub out: String,
    pub err: String,
    pub code: i32,
}

impl Said {
    fn fail(text: String) -> Said {
        Said {
            out: String::new(),
            err: format!("phoenix-cnc: {text}\n"),
            code: 2,
        }
    }
}

/// The lines that tell a starting point: what it is, its values, the chip load and the cutting speed, and what a person should know.
pub fn start_lines(w: &Workpiece, s: &CncStart, lang: &str) -> Vec<String> {
    let l = lang_of(lang);
    let mut v = vec![
        say(
            lang,
            Msg::FeedsTitle,
            &[
                w.names.get(l),
                &w.id,
                s.op.as_str(),
                s.tool.as_str(),
                &num(s.diameter_mm),
            ],
        ),
        say(
            lang,
            Msg::FeedsValues,
            &[
                num(s.rpm),
                num(s.feed_mm_min),
                dash(s.plunge_mm_min),
                num(s.depth_per_pass_mm),
                dash(s.stepover_pct),
            ],
        ),
    ];
    match s.flutes.and_then(|f| chip_load(s.feed_mm_min, s.rpm, f)) {
        Some(chip) => {
            let pct = chip / s.diameter_mm * 100.0;
            v.push(say(
                lang,
                Msg::FeedsChip,
                &[
                    format!("{chip:.4}"),
                    format!("{pct:.1}"),
                    num(surface_speed(s.diameter_mm, s.rpm)),
                ],
            ));
            match chip_verdict(chip, s.diameter_mm) {
                ChipVerdict::Low => v.push(say::<&str>(lang, Msg::FeedsChipLow, &[])),
                ChipVerdict::High => v.push(say::<&str>(lang, Msg::FeedsChipHigh, &[])),
                ChipVerdict::Fine => {}
            }
        }
        None => v.push(say::<&str>(lang, Msg::FeedsNoFlutes, &[])),
    }
    v.extend(s.notes().into_iter().map(|n| n.text(l).to_string()));
    v
}

fn warning_text(w: &StartWarning, lang: &str) -> String {
    w.text(lang_of(lang))
}

/// `--feeds`: the starting point the request asks for, or the points of the material when it does not say a diameter.
pub fn feeds(spec: &str, lang: &str) -> Said {
    let req = match parse_request(spec) {
        Ok(r) => r,
        Err(bad) => return Said::fail(say(lang, Msg::FeedsBadRequest, &[bad])),
    };
    let db = match database() {
        Ok(d) => d,
        Err(e) => return Said::fail(e),
    };
    let Some(w) = find(db, &req.material) else {
        return Said::fail(say(lang, Msg::FeedsUnknown, &[req.material.as_str()]));
    };
    match (req.op, req.diameter_mm) {
        (op, None) => list_points(w, op, lang),
        (op, Some(d)) => {
            let op = op.unwrap_or(CncOp::Contour);
            let tools: Vec<Tool> = match req.tool {
                Some(t) => vec![t],
                None => TOOLS.into_iter().filter(|t| has(w, op, *t)).collect(),
            };
            let mut first_err = None;
            for t in tools {
                match w.cnc_start(op, t, d) {
                    Ok(s) => {
                        let mut out = start_lines(w, &s, lang).join("\n");
                        out.push('\n');
                        return Said {
                            out,
                            err: String::new(),
                            code: 0,
                        };
                    }
                    Err(e) => {
                        first_err.get_or_insert(e);
                    }
                }
            }
            let why = first_err.unwrap_or(StartWarning::NoSuchProcess);
            Said::fail(warning_text(&why, lang))
        }
    }
}

fn has(w: &Workpiece, op: CncOp, tool: Tool) -> bool {
    w.cnc.iter().any(|p| p.op == op && p.tool == tool)
}

fn list_points(w: &Workpiece, op: Option<CncOp>, lang: &str) -> Said {
    let l = lang_of(lang);
    let mut points: Vec<&CncPoint> = w
        .cnc
        .iter()
        .filter(|p| op.is_none_or(|o| p.op == o))
        .collect();
    if points.is_empty() {
        return Said::fail(warning_text(
            &if w.cnc.is_empty() {
                StartWarning::NoPoints
            } else {
                StartWarning::NoSuchProcess
            },
            lang,
        ));
    }
    points.sort_by(|a, b| {
        (a.op.as_str(), a.tool.as_str())
            .cmp(&(b.op.as_str(), b.tool.as_str()))
            .then(a.diameter_mm.total_cmp(&b.diameter_mm))
    });
    let mut out = say(
        lang,
        Msg::FeedsListTitle,
        &[
            w.names.get(l).to_string(),
            w.id.clone(),
            points.len().to_string(),
        ],
    );
    out.push('\n');
    for p in points {
        let tried = if p.verified {
            Msg::FeedsTried
        } else {
            Msg::FeedsNotTried
        };
        out.push_str(&say(
            lang,
            Msg::FeedsRow,
            &[
                p.op.as_str().to_string(),
                p.tool.as_str().to_string(),
                num(p.diameter_mm),
                num(p.rpm),
                num(p.feed_mm_min),
                say::<&str>(lang, tried, &[]),
            ],
        ));
        out.push('\n');
    }
    Said {
        out,
        err: String::new(),
        code: 0,
    }
}

/// The family a name stands for (`all` is none).
fn family_of(name: &str) -> Result<Option<Family>, ()> {
    let n = name.trim().to_ascii_lowercase();
    if n == "all" || n.is_empty() {
        return Ok(None);
    }
    Family::ALL
        .into_iter()
        .find(|f| f.as_str() == n)
        .map(Some)
        .ok_or(())
}

/// `--materials`: the materials of the database that have starting points for a router, of a family or of all.
pub fn materials(family: &str, lang: &str) -> Said {
    let fam = match family_of(family) {
        Ok(f) => f,
        Err(()) => {
            let names = Family::ALL.map(Family::as_str).join(", ");
            return Said::fail(say(
                lang,
                Msg::SharedUnknownFamily,
                &[family.trim().to_string(), names],
            ));
        }
    };
    let db = match database() {
        Ok(d) => d,
        Err(e) => return Said::fail(e),
    };
    let l = lang_of(lang);
    let label = fam.map_or_else(|| "all".to_string(), |f| f.name(l).to_string());
    let mut rows: Vec<&Workpiece> = db
        .materials()
        .iter()
        .filter(|w| !w.cnc.is_empty() && fam.is_none_or(|f| w.family == f))
        .collect();
    if rows.is_empty() {
        return Said::fail(say::<&str>(lang, Msg::SharedNone, &[]));
    }
    rows.sort_by(|a, b| a.id.cmp(&b.id));
    let mut out = say(lang, Msg::SharedListTitle, &[label]);
    out.push('\n');
    for w in rows {
        let tried = w.cnc.iter().filter(|p| p.verified).count();
        out.push_str(&say(
            lang,
            Msg::SharedRow,
            &[
                w.id.clone(),
                w.names.get(l).to_string(),
                w.cnc.len().to_string(),
                tried.to_string(),
            ],
        ));
        out.push('\n');
    }
    Said {
        out,
        err: String::new(),
        code: 0,
    }
}

/// The ranges of a material of the database as a material of the library of the program: the lowest and the highest feed and speed of its points
/// (the ranges the table gives where it gives them), whatever the operation. A range that does not span anything is left out, and a material with
/// no range at all gives none.
pub fn as_library_material(w: &Workpiece) -> Option<crate::materials::Material> {
    let mut feed: Option<(f64, f64)> = None;
    let mut rpm: Option<(f64, f64)> = None;
    let widen = |acc: &mut Option<(f64, f64)>, lo: f64, hi: f64| {
        *acc = Some(match *acc {
            None => (lo, hi),
            Some((a, b)) => (a.min(lo), b.max(hi)),
        });
    };
    for p in &w.cnc {
        let r = p.ranges.as_ref();
        let f = r
            .and_then(|r| r.feed_mm_min)
            .map_or((p.feed_mm_min, p.feed_mm_min), |s| (s[0], s[1]));
        let s = r
            .and_then(|r| r.rpm)
            .map_or((p.rpm, p.rpm), |s| (s[0], s[1]));
        widen(&mut feed, f.0, f.1);
        widen(&mut rpm, s.0, s.1);
    }
    let usable = |r: Option<(f64, f64)>| {
        r.filter(|(lo, hi)| {
            lo.is_finite() && hi.is_finite() && *lo >= 0.0 && hi > lo && *hi <= 1_000_000.0
        })
    };
    let (feed, rpm) = (usable(feed), usable(rpm));
    (feed.is_some() || rpm.is_some()).then(|| crate::materials::Material {
        name: w.names.get(Lang::En).to_string(),
        feed,
        rpm,
    })
}

/// The material of the shared database a name of the settings stands for, as a material of the library. Used when the library of the operator has no
/// material of that name.
pub fn shared_material(name: &str) -> Option<crate::materials::Material> {
    let db = database().ok()?;
    as_library_material(find(db, name)?)
}
