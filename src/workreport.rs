//! The work report: one HTML page that holds everything in it (no script, no link, no picture file), with the statistics and the time of the
//! program, the tools, what looks wrong (with the line), the path seen from above with the lines that have a warning marked, and the height map
//! as a grid of colours when one was measured. It is made from the same numbers as `--report` and the statistics window and it only reads:
//! writing it never goes over a file that is there.

use std::fmt::Write as _;
use std::path::Path;

use crate::gcode::{toolpath, Segment};
use crate::heightmap::HeightMap;
use crate::lint::LintCfg;
use crate::progcheck::Level;
use crate::report::{notes, Cfg};
use crate::sim::{hms, Rates};
use crate::stats::measure;
use crate::tuning::Tuning;
use crate::words::{say, text, Msg};

/// The most moves drawn in the picture of the path: a longer program is drawn with every n-th move so the page stays small.
const MAX_DRAWN: usize = 40_000;

/// What the report is made of.
pub struct Input<'a> {
    /// The name of the program (a file name, not a path); it may be empty.
    pub name: &'a str,
    pub lines: &'a [String],
    pub tuning: &'a Tuning,
    pub lint: &'a LintCfg,
    pub cfg: &'a Cfg,
    pub height_map: Option<&'a HeightMap>,
    /// The version of the program that made the report.
    pub version: &'a str,
}

/// The text as HTML text: the characters with a meaning in HTML are written as entities.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c if c.is_control() && c != '\n' && c != '\t' => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

/// A number for a picture: at most two decimals, no noise.
fn n2(v: f64) -> String {
    let s = format!("{v:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-" || s == "-0" {
        "0".to_string()
    } else {
        s.to_string()
    }
}

/// The picture of the path from above as SVG: cuts in one colour, rapids dashed, and a ring on the first move of every line that has a
/// warning. The Y axis points up, as on the table.
fn path_svg(path: &[Segment], marked: &[usize], lang: &str) -> String {
    let mut b: Option<[f64; 4]> = None;
    for s in path {
        for p in [s.from, s.to] {
            b = Some(match b {
                None => [p.0, p.1, p.0, p.1],
                Some([x0, y0, x1, y1]) => [x0.min(p.0), y0.min(p.1), x1.max(p.0), y1.max(p.1)],
            });
        }
    }
    let Some([x0, y0, x1, y1]) = b else {
        return String::new();
    };
    let (w, h) = ((x1 - x0).max(1.0), (y1 - y0).max(1.0));
    let pad = w.max(h) * 0.04;
    let ring = w.max(h) * 0.015;
    let step = path.len().div_ceil(MAX_DRAWN).max(1);
    let (mut cut, mut rapid) = (String::new(), String::new());
    let mut last: [Option<(f64, f64)>; 2] = [None, None];
    for (i, s) in path.iter().enumerate() {
        if i % step != 0 {
            continue;
        }
        let (d, k) = if s.rapid {
            (&mut rapid, 1)
        } else {
            (&mut cut, 0)
        };
        let from = (s.from.0, s.from.1);
        if last[k] != Some(from) {
            let _ = write!(d, "M{} {}", n2(from.0), n2(from.1));
        }
        let _ = write!(d, "L{} {}", n2(s.to.0), n2(s.to.1));
        last[k] = Some((s.to.0, s.to.1));
    }
    let mut svg = String::new();
    // the group flips the Y axis (up on the table is up on the page) and puts the corner of the path at the corner of the picture
    let _ = write!(
        svg,
        "<svg class=\"pic\" xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\" role=\"img\" aria-label=\"{}\"><g transform=\"translate({} {}) scale(1 -1)\">",
        n2(w + 2.0 * pad),
        n2(h + 2.0 * pad),
        escape(text(lang, Msg::RepPath)),
        n2(pad - x0),
        n2(y1 + pad),
    );
    let _ = write!(
        svg,
        "<path d=\"{rapid}\" fill=\"none\" stroke=\"var(--rapid)\" stroke-width=\"1\" stroke-dasharray=\"4 3\" vector-effect=\"non-scaling-stroke\"/>"
    );
    let _ = write!(
        svg,
        "<path d=\"{cut}\" fill=\"none\" stroke=\"var(--cut)\" stroke-width=\"1.6\" stroke-linejoin=\"round\" vector-effect=\"non-scaling-stroke\"/>"
    );
    for line in marked {
        if let Some(s) = path.iter().find(|s| s.line >= *line) {
            let _ = write!(
                svg,
                "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"none\" stroke=\"var(--bad)\" stroke-width=\"2\" vector-effect=\"non-scaling-stroke\"/>",
                n2(s.to.0),
                n2(s.to.1),
                n2(ring)
            );
        }
    }
    svg.push_str("</g></svg>");
    svg
}

/// The colour of a height: blue for the lowest, red for the highest, in `#rrggbb`.
fn ramp(t: f64) -> String {
    let t = if t.is_finite() {
        t.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let lerp = |a: f64, b: f64| (a + (b - a) * t).round() as u8;
    format!(
        "#{:02x}{:02x}{:02x}",
        lerp(37.0, 239.0),
        lerp(99.0, 68.0),
        lerp(235.0, 68.0)
    )
}

/// The height map as a grid of coloured cells, row 0 at the bottom.
fn map_svg(m: &HeightMap, lang: &str) -> String {
    let (lo, hi) = m.span();
    let span = (hi - lo).max(1e-9);
    let (cw, ch) = (m.dx.abs().max(1e-6), m.dy.abs().max(1e-6));
    let (width, height) = (cw * m.nx as f64, ch * m.ny as f64);
    let mut svg = String::new();
    let _ = write!(
        svg,
        "<svg class=\"pic\" xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\" role=\"img\" aria-label=\"{}\">",
        n2(width),
        n2(height),
        escape(text(lang, Msg::RepHeight))
    );
    for j in 0..m.ny {
        for i in 0..m.nx {
            let z = m.z.get(j * m.nx + i).copied().unwrap_or(0.0);
            // row 0 is the smallest Y, which is drawn at the bottom
            let y = height - ch * (j as f64 + 1.0);
            let _ = write!(
                svg,
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"><title>{} mm</title></rect>",
                n2(cw * i as f64),
                n2(y),
                n2(cw),
                n2(ch),
                ramp((z - lo) / span),
                n2(z)
            );
        }
    }
    svg.push_str("</svg>");
    svg
}

const STYLE: &str = "\
:root{color-scheme:light dark;--bg:#ffffff;--fg:#1c2733;--dim:#5b6b7b;--line:#d5dce4;--cut:#0f6cbd;--rapid:#8a97a5;--bad:#d13438;--warn:#b8860b;--card:#f5f7fa}\
@media (prefers-color-scheme:dark){:root{--bg:#10161d;--fg:#e6edf3;--dim:#9aa7b4;--line:#2a3541;--cut:#4cc2ff;--rapid:#7d8a98;--bad:#ff6b6b;--warn:#e3b341;--card:#18212b}}\
body{margin:0 auto;max-width:900px;padding:24px 16px;font:15px/1.5 system-ui,Segoe UI,sans-serif;background:var(--bg);color:var(--fg)}\
h1{margin:0 0 4px;font-size:1.6rem}h2{margin:28px 0 8px;font-size:1.15rem;border-bottom:1px solid var(--line);padding-bottom:4px}\
.sub{margin:0;color:var(--dim)}table{border-collapse:collapse;width:100%}th,td{text-align:left;padding:5px 8px;border-bottom:1px solid var(--line)}\
th{color:var(--dim);font-weight:600}td.n,th.n{text-align:right;font-variant-numeric:tabular-nums}\
.pic{width:100%;height:auto;max-height:560px;background:var(--card);border:1px solid var(--line);border-radius:6px}\
.legend{display:flex;gap:16px;flex-wrap:wrap;margin:6px 0;color:var(--dim);font-size:.9rem}\
.danger{color:var(--bad);font-weight:600}.warning{color:var(--warn);font-weight:600}.info{color:var(--dim)}\
footer{margin-top:32px;color:var(--dim);font-size:.85rem}";

/// The whole page.
pub fn html(i: &Input) -> String {
    let lang = i.cfg.lang.as_str();
    let w = |m: Msg| escape(text(lang, m));
    let rates = Rates {
        rapid: i.cfg.rapid_mm_min,
        default_feed: i.cfg.default_feed,
    };
    let stats = measure(i.lines, rates, i.tuning.change_seconds(i.cfg.rapid_mm_min));
    let found = notes(i.lines, i.tuning, i.lint, i.cfg);
    let path = toolpath(i.lines);
    let mut out = String::new();
    let _ = write!(
        out,
        "<!DOCTYPE html>\n<html lang=\"{}\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{}</title><style>{STYLE}</style></head><body>\n",
        escape(lang),
        w(Msg::RepTitle)
    );
    let _ = writeln!(out, "<h1>{}</h1>", w(Msg::RepTitle));
    if !i.name.is_empty() {
        let _ = writeln!(
            out,
            "<p class=\"sub\">{}: {}</p>",
            w(Msg::RepProgram),
            escape(i.name)
        );
    }
    // the summary
    let _ = writeln!(out, "<h2>{}</h2>\n<table>", w(Msg::RepSummary));
    let row = |out: &mut String, label: Msg, value: String| {
        let _ = writeln!(
            out,
            "<tr><th>{}</th><td class=\"n\">{}</td></tr>",
            w(label),
            escape(&value)
        );
    };
    row(&mut out, Msg::RepLines, stats.program_lines.to_string());
    row(&mut out, Msg::RepMoves, path.len().to_string());
    row(&mut out, Msg::RepCut, format!("{:.1} mm", stats.cut_mm));
    row(
        &mut out,
        Msg::RepRapidLen,
        format!("{:.1} mm", stats.rapid_mm),
    );
    row(&mut out, Msg::RepChanges, stats.changes.to_string());
    row(&mut out, Msg::RepTime, hms(stats.seconds));
    out.push_str("</table>\n");
    // the tools
    if !stats.tools.is_empty() {
        let _ = writeln!(
            out,
            "<h2>{}</h2>\n<table><tr><th></th><th class=\"n\">{}</th><th class=\"n\">{}</th><th class=\"n\">{}</th></tr>",
            w(Msg::RepTools),
            w(Msg::RepCut),
            w(Msg::RepRapidLen),
            w(Msg::RepTime)
        );
        for t in &stats.tools {
            let name = match t.tool {
                Some(n) => say(lang, Msg::StatsTool, &[n.to_string()]),
                None => say(lang, Msg::StatsNoTool, &[] as &[&str]),
            };
            let _ = writeln!(
                out,
                "<tr><td>{}</td><td class=\"n\">{:.1} mm</td><td class=\"n\">{:.1} mm</td><td class=\"n\">{}</td></tr>",
                escape(&name),
                t.cut_mm,
                t.rapid_mm,
                escape(&hms(t.seconds))
            );
        }
        out.push_str("</table>\n");
    }
    // what looks wrong
    let _ = writeln!(out, "<h2>{}</h2>", w(Msg::RepWarnings));
    if found.is_empty() {
        let _ = writeln!(
            out,
            "<p>{}</p>",
            escape(&say(lang, Msg::ReportClean, &[] as &[&str]))
        );
    } else {
        let _ = writeln!(
            out,
            "<table><tr><th>{}</th><th class=\"n\">{}</th><th></th></tr>",
            w(Msg::RepLevel),
            w(Msg::RepLine)
        );
        for n in &found {
            let (class, label) = match n.level {
                Level::Danger => ("danger", Msg::RepDanger),
                Level::Warning => ("warning", Msg::RepWarning),
                Level::Info => ("info", Msg::RepInfo),
            };
            let line = n.line.map_or(String::new(), |l| (l + 1).to_string());
            let _ = writeln!(
                out,
                "<tr><td class=\"{class}\">{}</td><td class=\"n\">{line}</td><td>{}</td></tr>",
                w(label),
                escape(&n.text)
            );
        }
        out.push_str("</table>\n");
    }
    // the path
    let marked: Vec<usize> = found.iter().filter_map(|n| n.line).collect();
    let svg = path_svg(&path, &marked, lang);
    if !svg.is_empty() {
        let _ = writeln!(
            out,
            "<h2>{}</h2>\n<div class=\"legend\"><span style=\"color:var(--cut)\">&#9632; {}</span><span style=\"color:var(--rapid)\">&#9632; {}</span><span style=\"color:var(--bad)\">&#9675; {}</span></div>\n{svg}",
            w(Msg::RepPath),
            w(Msg::RepLegendCut),
            w(Msg::RepLegendRapid),
            w(Msg::RepLegendWarn)
        );
    }
    // the height map
    if let Some(m) = i.height_map.filter(|m| m.nx >= 2 && m.ny >= 2) {
        let (lo, hi) = m.span();
        let info = say(
            lang,
            Msg::RepHeightInfo,
            &[
                m.nx.to_string(),
                m.ny.to_string(),
                n2(m.dx),
                format!("{lo:.3}"),
                format!("{hi:.3}"),
            ],
        );
        let _ = writeln!(
            out,
            "<h2>{}</h2>\n<p class=\"sub\">{}</p>\n{}",
            w(Msg::RepHeight),
            escape(&info),
            map_svg(m, lang)
        );
    }
    let _ = writeln!(
        out,
        "<footer>{}</footer>\n</body></html>",
        escape(&say(lang, Msg::RepFooter, &[i.version.to_string()]))
    );
    out
}

/// Writes the page to a new file: a file that is there is never written over (the error is `AlreadyExists`).
pub fn write_new(path: &Path, page: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    f.write_all(page.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(lang: &str) -> Cfg {
        Cfg {
            travel: [(0.0, 300.0), (0.0, 200.0), (-50.0, 0.0)],
            offset: [0.0; 3],
            rapid_mm_min: 6000.0,
            default_feed: 300.0,
            lang: lang.to_string(),
        }
    }

    fn prog(text: &str) -> Vec<String> {
        text.lines().map(str::to_string).collect()
    }

    fn page(program: &str, lang: &str, map: Option<&HeightMap>) -> String {
        let lines = prog(program);
        let (tuning, lint, cfg) = (Tuning::default(), LintCfg::default(), cfg(lang));
        html(&Input {
            name: "job <1>.nc",
            lines: &lines,
            tuning: &tuning,
            lint: &lint,
            cfg: &cfg,
            height_map: map,
            version: "9.9.9",
        })
    }

    const GOOD: &str =
        "G21 G90\nM3 S10000\nG0 X10 Y10\nT1 M6\nG1 Z-1 F200\nG1 X20\nG1 Y40\nG0 Z0\nM5\nM30";

    #[test]
    fn the_page_is_one_document_with_no_script_and_no_link_and_the_name_is_escaped() {
        let p = page(GOOD, "en", None);
        assert!(p.starts_with("<!DOCTYPE html>"));
        assert!(p.contains("<title>Work report</title>"));
        assert!(p.contains("job &lt;1&gt;.nc"));
        assert!(!p.contains("job <1>"));
        for forbidden in ["<script", "http://", "https://", "src=", "href="] {
            // the only address in the page is the XML namespace of the pictures
            let text = p.replace("http://www.w3.org/2000/svg", "");
            assert!(!text.contains(forbidden), "{forbidden}");
        }
        assert!(p.contains("</html>"));
    }

    #[test]
    fn the_numbers_of_the_summary_are_those_of_the_statistics() {
        let p = page(GOOD, "en", None);
        let s = measure(
            &prog(GOOD),
            Rates {
                rapid: 6000.0,
                default_feed: 300.0,
            },
            30.0,
        );
        assert!(p.contains(&format!("{:.1} mm", s.cut_mm)));
        assert!(p.contains(&hms(s.seconds)));
        assert!(p.contains("Tool 1"));
        assert!(p.contains("<h2>Tools</h2>"));
    }

    #[test]
    fn what_looks_wrong_is_listed_with_its_line_and_a_clean_program_says_so() {
        // a rapid move below the surface of the work
        let bad = page(
            "G21 G90\nM3 S1000\nG0 X1 Y1 Z-5\nG1 X5 F100\nM5\nM30",
            "en",
            None,
        );
        assert!(bad.contains("class=\"danger\""), "{bad}");
        assert!(bad.contains("<td class=\"n\">3</td>"));
        assert!(bad.contains("var(--bad)"));
        // the tool change of the sample program is told with its line too
        assert!(page(GOOD, "en", None).contains("<td class=\"n\">4</td>"));
        let clean = page(
            "G21 G90\nM3 S10000\nG0 X10 Y10\nG1 Z-1 F200\nG1 X20\nG1 Y40\nG0 Z0\nM5\nM30",
            "en",
            None,
        );
        assert!(clean.contains("<h2>What looks wrong</h2>"));
        assert!(clean.contains("Nothing looks wrong"), "{clean}");
    }

    #[test]
    fn the_path_is_drawn_and_a_program_with_no_move_has_no_picture() {
        let p = page(GOOD, "en", None);
        assert!(p.contains("<svg class=\"pic\""));
        assert!(p.contains("stroke-dasharray"));
        assert!(p.contains("Path from above"));
        let none = page("M3 S1000\nM5", "en", None);
        assert!(!none.contains("<svg"));
        assert!(!none.contains("Path from above"));
    }

    #[test]
    fn the_height_map_is_a_grid_of_cells_with_its_heights() {
        let readings: Vec<f64> = (0..6).map(|i| f64::from(i) * 0.1).collect();
        let m = HeightMap::from_readings([0.0, 0.0, 10.0, 5.0], 3, 2, &readings).unwrap();
        let p = page(GOOD, "en", Some(&m));
        assert!(p.contains("<h2>Height map</h2>"));
        assert_eq!(p.matches("<rect ").count(), 6);
        assert!(p.contains("3 x 2 points, 5 mm apart; heights from 0.000 to 0.500 mm"));
        // the lowest cell is blue and the highest red
        assert!(p.contains("#2563eb") && p.contains("#ef4444"));
        assert!(!page(GOOD, "en", None).contains("<rect "));
    }

    #[test]
    fn the_page_follows_the_language() {
        let es = page(GOOD, "es", None);
        assert!(es.contains("<html lang=\"es\">"));
        assert!(es.contains("<title>Informe de trabajo</title>"));
        assert!(es.contains("Resumen"));
        let ja = page(GOOD, "ja", None);
        assert!(ja.contains("作業レポート"));
        assert!(page(GOOD, "pt", None).contains("Relatório de trabalho"));
    }

    #[test]
    fn a_long_program_is_drawn_with_a_bounded_number_of_moves() {
        let mut text = String::from("G21 G90\nG1 F600\n");
        for i in 0..(MAX_DRAWN * 2 + 10) {
            text.push_str(&format!("G1 X{} Y{}\n", i % 100, (i / 100) % 100));
        }
        let lines = prog(&text);
        let path = toolpath(&lines);
        let svg = path_svg(&path, &[], "en");
        // one `L` for every move drawn
        assert!(
            svg.matches('L').count() <= MAX_DRAWN + 1,
            "{}",
            svg.matches('L').count()
        );
        assert!(svg.len() < 2_000_000);
    }

    #[test]
    fn the_escape_keeps_markup_out() {
        assert_eq!(
            escape("<a href=\"x\">&'"),
            "&lt;a href=&quot;x&quot;&gt;&amp;&#39;"
        );
        assert_eq!(escape("a\u{0}b\u{7}c"), "a b c");
        assert_eq!(escape("línea 日本"), "línea 日本");
    }

    #[test]
    fn a_report_is_never_written_over_a_file() {
        let d = std::env::temp_dir().join(format!("eh3d_workreport_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join("r.html");
        write_new(&f, "one").unwrap();
        let e = write_new(&f, "two").unwrap_err();
        assert_eq!(e.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "one");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_colours_of_the_ramp_stay_in_range() {
        assert_eq!(ramp(0.0), "#2563eb");
        assert_eq!(ramp(1.0), "#ef4444");
        assert_eq!(ramp(f64::NAN), "#2563eb");
        assert_eq!(ramp(-3.0), "#2563eb");
        assert_eq!(ramp(7.0), "#ef4444");
    }
}
