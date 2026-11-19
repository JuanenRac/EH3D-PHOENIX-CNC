//! The library of materials: for each material the range of cutting feed (mm per minute) and of spindle speed (rpm) that is a good place to
//! start with it. The values are not in the code: they are in a text file, `examples/materials.txt` (built into the program as the library
//! to use while the operator has none) or `phoenix-cnc-materials.txt` next to the settings file, which takes its place. The material chosen
//! in the settings gives the checker (`lint`) the ranges to hold a program to. Nothing here moves anything; a range is only a warning.
//!
//! A line of the file: `material = NAME | feed=LOW,HIGH | rpm=LOW,HIGH`, either range may be left out; empty lines, lines that start with
//! `#` or `;` and lines that are not valid are left out.

use std::path::{Path, PathBuf};

use crate::lint::LintCfg;
use crate::profiles::clean_name;

/// The library the program uses while the operator has none of their own.
const SHIPPED: &str = include_str!("../examples/materials.txt");

/// The most materials a library holds, and the biggest file that is read.
pub const MAX_MATERIALS: usize = 64;
pub const MAX_FILE_BYTES: u64 = 64 * 1024;

/// A material and what is a good place to start with it.
#[derive(Debug, Clone, PartialEq)]
pub struct Material {
    pub name: String,
    /// The range of cutting feed in mm per minute, if the material has one.
    pub feed: Option<(f64, f64)>,
    /// The range of spindle speed in rpm, if the material has one.
    pub rpm: Option<(f64, f64)>,
}

fn range(v: &str) -> Option<(f64, f64)> {
    let (lo, hi) = v.split_once(',')?;
    let (lo, hi) = (
        lo.trim().parse::<f64>().ok()?,
        hi.trim().parse::<f64>().ok()?,
    );
    (lo.is_finite() && hi.is_finite() && lo >= 0.0 && hi > lo && hi <= 1_000_000.0)
        .then_some((lo, hi))
}

impl Material {
    /// Reads what follows `material =` in a line of the file.
    pub fn parse(value: &str) -> Option<Material> {
        let mut parts = value.split('|');
        let name = clean_name(parts.next()?)?;
        let (mut feed, mut rpm) = (None, None);
        for p in parts {
            let (k, v) = p.split_once('=')?;
            match k.trim().to_ascii_lowercase().as_str() {
                "feed" => feed = Some(range(v)?),
                "rpm" => rpm = Some(range(v)?),
                _ => {}
            }
        }
        (feed.is_some() || rpm.is_some()).then_some(Material { name, feed, rpm })
    }

    /// Gives the checker the ranges of this material.
    pub fn apply(&self, lint: &mut LintCfg) {
        lint.material_feed = self.feed;
        lint.material_rpm = self.rpm;
    }
}

/// Reads a library from the text of its file. A name that comes again keeps its first line; there are never more than `MAX_MATERIALS`.
pub fn parse_library(text: &str) -> Vec<Material> {
    let mut out: Vec<Material> = Vec::new();
    for line in text.trim_start_matches('\u{feff}').lines() {
        let line = line.trim();
        if line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        if !k.trim().eq_ignore_ascii_case("material") {
            continue;
        }
        if let Some(m) = Material::parse(v) {
            let known = out.iter().any(|o| o.name.eq_ignore_ascii_case(&m.name));
            if !known && out.len() < MAX_MATERIALS {
                out.push(m);
            }
        }
    }
    out
}

/// Where the library of the operator is: next to the settings, with the name of the settings file in front (`phoenix-cnc-materials.txt`).
pub fn library_path(settings: &Path) -> PathBuf {
    let stem = settings.file_stem().map_or_else(
        || "phoenix-cnc".to_string(),
        |s| s.to_string_lossy().to_string(),
    );
    settings.with_file_name(format!("{stem}-materials.txt"))
}

/// The library to use: the file of the operator when there is one (empty if it cannot be read or is too big), or the one shipped.
pub fn load(settings: &Path) -> Vec<Material> {
    let path = library_path(settings);
    match std::fs::metadata(&path) {
        Err(_) => parse_library(SHIPPED),
        Ok(m) if m.len() > MAX_FILE_BYTES => Vec::new(),
        Ok(_) => std::fs::read_to_string(&path)
            .map(|t| parse_library(&t))
            .unwrap_or_default(),
    }
}

/// The material with this name (capitals do not matter).
pub fn find<'a>(library: &'a [Material], name: &str) -> Option<&'a Material> {
    library.iter().find(|m| m.name.eq_ignore_ascii_case(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_has_a_name_and_one_or_two_ranges() {
        let m = Material::parse(" Oak | feed=300, 900 | rpm=9000,18000").unwrap();
        assert_eq!(m.name, "Oak");
        assert_eq!(
            (m.feed, m.rpm),
            (Some((300.0, 900.0)), Some((9000.0, 18000.0)))
        );
        let f = Material::parse("Felt | feed=10,20").unwrap();
        assert_eq!((f.feed, f.rpm), (Some((10.0, 20.0)), None));
        // keys are not case sensitive and a key that is not known is left out
        assert!(Material::parse("X | RPM=1,2 | colour=blue").is_some());
        // no name, no range, a range that is wrong or a piece without a key is not a material
        assert!(Material::parse("| feed=1,2").is_none());
        assert!(Material::parse("Nothing").is_none());
        assert!(Material::parse("A | feed=5,5").is_none());
        assert!(Material::parse("A | feed=9,2").is_none());
        assert!(Material::parse("A | feed=-1,2").is_none());
        assert!(Material::parse("A | feed=1,x").is_none());
        assert!(Material::parse("A | feed=1,1e9").is_none());
        assert!(Material::parse("A | feed=1,2 | rpm").is_none());
    }

    #[test]
    fn the_library_skips_what_is_not_valid_and_keeps_the_first_of_a_name() {
        let text = "\u{feff}# a comment\n; another\nmaterial = A | feed=1,2\nbogus = B | feed=1,2\nmaterial = a | feed=5,6\nmaterial = C\nMATERIAL = D | rpm=1,2\nnot a line\n";
        let lib = parse_library(text);
        assert_eq!(
            lib.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(),
            ["A", "D"]
        );
        assert_eq!(lib[0].feed, Some((1.0, 2.0)));
        assert_eq!(find(&lib, "d").map(|m| m.name.as_str()), Some("D"));
        assert!(find(&lib, "Z").is_none());
        // never more than the limit
        let many: String = (0..100)
            .map(|i| format!("material = M{i} | feed=1,2\n"))
            .collect();
        assert_eq!(parse_library(&many).len(), MAX_MATERIALS);
    }

    #[test]
    fn the_shipped_library_is_valid_and_every_material_has_both_ranges() {
        let lib = parse_library(SHIPPED);
        assert!(lib.len() >= 8, "{}", lib.len());
        for m in &lib {
            assert!(m.feed.is_some() && m.rpm.is_some(), "{}", m.name);
        }
        // every material line of the file is read: none is dropped as invalid
        let lines = SHIPPED
            .lines()
            .filter(|l| l.trim_start().starts_with("material"))
            .count();
        assert_eq!(lib.len(), lines);
        assert!(find(&lib, "aluminium").is_some());
    }

    #[test]
    fn the_file_of_the_operator_takes_the_place_of_the_shipped_library() {
        let d = std::env::temp_dir().join(format!("eh3d_materials_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let settings = d.join("phoenix-cnc.cfg");
        assert_eq!(
            library_path(&settings).file_name().unwrap(),
            "phoenix-cnc-materials.txt"
        );
        // no file: the shipped one
        assert_eq!(load(&settings), parse_library(SHIPPED));
        std::fs::write(library_path(&settings), "material = Mine | feed=1,2\n").unwrap();
        let lib = load(&settings);
        assert_eq!(lib.len(), 1);
        assert_eq!(lib[0].name, "Mine");
        // a file with nothing valid is an empty library, not the shipped one
        std::fs::write(library_path(&settings), "nothing\n").unwrap();
        assert!(load(&settings).is_empty());
        // too big a file is not read
        std::fs::write(
            library_path(&settings),
            vec![b'#'; (MAX_FILE_BYTES + 1) as usize],
        )
        .unwrap();
        assert!(load(&settings).is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_material_gives_its_ranges_to_the_checker() {
        let mut lint = LintCfg::default();
        assert_eq!((lint.material_feed, lint.material_rpm), (None, None));
        let m = Material::parse("A | feed=100,600 | rpm=8000,16000").unwrap();
        m.apply(&mut lint);
        assert_eq!(lint.material_feed, Some((100.0, 600.0)));
        assert_eq!(lint.material_rpm, Some((8000.0, 16000.0)));
        Material::parse("B | feed=1,2").unwrap().apply(&mut lint);
        assert_eq!(lint.material_rpm, None);
    }
}
