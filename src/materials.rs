//! The library of materials: for each material the range of cutting feed (mm per minute) and of spindle speed (rpm) that is a good place to
//! start with it. The values are not in the code: they are in a text file, `examples/materials.txt` (built into the program as the library
//! to use while the operator has none) or `phoenix-cnc-materials.txt` next to the settings file, which takes its place. The material chosen
//! in the settings gives the checker (`lint`) the ranges to hold a program to. Nothing here moves anything; a range is only a warning.
//!
//! A line of the file: `material = NAME | feed=LOW,HIGH | rpm=LOW,HIGH`, either range may be left out; empty lines, lines that start with
//! `#` or `;` and lines that are not valid are left out.

use std::path::{Path, PathBuf};

use crate::heightmap::num;
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

/// Why a material of the editor cannot be written to the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowError {
    /// The name is empty or has characters the file cannot keep (it would be read back as another name).
    Name,
    /// Neither a range of feed nor a range of speed.
    NoRange,
    /// A range that does not go from 0 or more up to something bigger, within 1000000.
    Range,
    /// The name is the one of an earlier material (capitals do not matter).
    Repeated,
    /// More than `MAX_MATERIALS`.
    TooMany,
}

/// Why the library could not be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveError {
    /// The file is there and nobody said it could be replaced.
    Exists,
    /// A row (from 0) cannot be written.
    Invalid(usize, RowError),
    /// The system refused (the text of its error).
    Io(String),
}

fn range_ok((lo, hi): (f64, f64)) -> bool {
    lo.is_finite() && hi.is_finite() && lo >= 0.0 && hi > lo && hi <= 1_000_000.0
}

impl Material {
    /// Whether this material is written to the file and read back as it is.
    pub fn check(&self) -> Result<(), RowError> {
        if clean_name(&self.name).as_deref() != Some(self.name.as_str()) {
            return Err(RowError::Name);
        }
        if self.feed.is_none() && self.rpm.is_none() {
            return Err(RowError::NoRange);
        }
        if self.feed.into_iter().chain(self.rpm).any(|r| !range_ok(r)) {
            return Err(RowError::Range);
        }
        Ok(())
    }

    /// The line of the file for this material.
    pub fn to_line(&self) -> String {
        let mut s = format!("material = {}", self.name);
        if let Some((lo, hi)) = self.feed {
            s.push_str(&format!(" | feed={},{}", num(lo), num(hi)));
        }
        if let Some((lo, hi)) = self.rpm {
            s.push_str(&format!(" | rpm={},{}", num(lo), num(hi)));
        }
        s
    }

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

/// Whether a library can be written as it is: the first row that cannot, with the reason.
pub fn check_library(rows: &[Material]) -> Result<(), (usize, RowError)> {
    for (i, m) in rows.iter().enumerate() {
        if i >= MAX_MATERIALS {
            return Err((i, RowError::TooMany));
        }
        m.check().map_err(|e| (i, e))?;
        if rows[..i]
            .iter()
            .any(|o| o.name.eq_ignore_ascii_case(&m.name))
        {
            return Err((i, RowError::Repeated));
        }
    }
    Ok(())
}

/// The text of the file for a library.
pub fn to_text(rows: &[Material]) -> String {
    let mut s = String::from(
        "# The library of materials (material = NAME | feed=LOW,HIGH | rpm=LOW,HIGH), written by the editor of the settings window.\n",
    );
    for m in rows {
        s.push_str(&m.to_line());
        s.push('\n');
    }
    s
}

/// Writes the library of the operator next to the settings. A file that is there is replaced only when `replace` is true; a library that
/// could not be read back as it is (see `check_library`) is not written.
pub fn save(settings: &Path, rows: &[Material], replace: bool) -> Result<(), SaveError> {
    check_library(rows).map_err(|(i, e)| SaveError::Invalid(i, e))?;
    write_file(&library_path(settings), &to_text(rows), replace)
}

/// Writes the library that is shipped as the file of the operator, to be edited. A file that is there is never written over.
pub fn create_sample(settings: &Path) -> Result<(), SaveError> {
    write_file(&library_path(settings), SHIPPED, false)
}

fn write_file(path: &Path, text: &str, replace: bool) -> Result<(), SaveError> {
    use std::io::Write;
    if text.len() as u64 > MAX_FILE_BYTES {
        return Err(SaveError::Io("too big".to_string()));
    }
    let mut open = std::fs::OpenOptions::new();
    open.write(true);
    if replace {
        open.create(true).truncate(true);
    } else {
        open.create_new(true);
    }
    let mut f = open.open(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::AlreadyExists => SaveError::Exists,
        _ => SaveError::Io(e.to_string()),
    })?;
    f.write_all(text.as_bytes())
        .map_err(|e| SaveError::Io(e.to_string()))
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

    fn mat(name: &str, feed: Option<(f64, f64)>, rpm: Option<(f64, f64)>) -> Material {
        Material {
            name: name.to_string(),
            feed,
            rpm,
        }
    }

    #[test]
    fn a_material_is_written_as_the_line_the_file_reads_back() {
        for m in [
            mat("Oak", Some((300.0, 900.0)), Some((9000.0, 18000.0))),
            mat("Felt", Some((10.5, 20.25)), None),
            mat("Cork board", None, Some((1.0, 2.0))),
        ] {
            assert_eq!(m.check(), Ok(()));
            assert_eq!(
                Material::parse(m.to_line().trim_start_matches("material = ")),
                Some(m)
            );
        }
        assert_eq!(
            mat("A", Some((1.0, 2.0)), Some((3.0, 4.0))).to_line(),
            "material = A | feed=1,2 | rpm=3,4"
        );
    }

    #[test]
    fn a_material_that_would_not_read_back_as_it_is_says_why() {
        let ok = Some((1.0, 2.0));
        for (m, why) in [
            (mat("", ok, None), RowError::Name),
            (mat(" A", ok, None), RowError::Name),
            (mat("A | B", ok, None), RowError::Name),
            (mat("A=B", ok, None), RowError::Name),
            (mat("A", None, None), RowError::NoRange),
            (mat("A", Some((5.0, 5.0)), None), RowError::Range),
            (mat("A", Some((9.0, 2.0)), None), RowError::Range),
            (mat("A", ok, Some((-1.0, 2.0))), RowError::Range),
            (mat("A", Some((1.0, 1.0e9)), None), RowError::Range),
            (mat("A", Some((f64::NAN, 2.0)), None), RowError::Range),
        ] {
            assert_eq!(m.check(), Err(why), "{m:?}");
        }
    }

    #[test]
    fn a_library_is_checked_row_by_row_and_written_as_text_that_reads_back() {
        let lib = vec![
            mat("Oak", Some((300.0, 900.0)), None),
            mat("Brass", None, Some((6000.0, 14000.0))),
        ];
        assert_eq!(check_library(&lib), Ok(()));
        assert_eq!(parse_library(&to_text(&lib)), lib);
        let mut bad = lib.clone();
        bad.push(mat("oak", Some((1.0, 2.0)), None));
        assert_eq!(check_library(&bad), Err((2, RowError::Repeated)));
        bad[2] = mat("Wood", None, None);
        assert_eq!(check_library(&bad), Err((2, RowError::NoRange)));
        let many: Vec<Material> = (0..=MAX_MATERIALS)
            .map(|i| mat(&format!("M{i}"), Some((1.0, 2.0)), None))
            .collect();
        assert_eq!(
            check_library(&many),
            Err((MAX_MATERIALS, RowError::TooMany))
        );
        assert!(to_text(&[]).starts_with('#'));
    }

    #[test]
    fn the_library_is_saved_next_to_the_settings_and_a_file_is_replaced_only_when_asked() {
        let d = std::env::temp_dir().join(format!("eh3d_materials_save_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let settings = d.join("phoenix-cnc.cfg");
        let first = vec![mat("Mine", Some((1.0, 2.0)), None)];
        assert_eq!(save(&settings, &first, false), Ok(()));
        assert_eq!(load(&settings), first);
        // the file is there: it is not written over unless the caller says so
        let second = vec![mat("Other", None, Some((5.0, 6.0)))];
        assert_eq!(save(&settings, &second, false), Err(SaveError::Exists));
        assert_eq!(load(&settings), first);
        assert_eq!(save(&settings, &second, true), Ok(()));
        assert_eq!(load(&settings), second);
        // a replacement that is shorter leaves nothing of the old file
        assert_eq!(save(&settings, &first, true), Ok(()));
        assert_eq!(
            std::fs::read_to_string(library_path(&settings)).unwrap(),
            to_text(&first)
        );
        // a library that cannot be read back is not written
        let bad = vec![mat("", Some((1.0, 2.0)), None)];
        assert_eq!(
            save(&settings, &bad, true),
            Err(SaveError::Invalid(0, RowError::Name))
        );
        assert_eq!(load(&settings), first);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_example_file_is_made_once_and_never_written_over() {
        let d = std::env::temp_dir().join(format!("eh3d_materials_sample_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let settings = d.join("phoenix-cnc.cfg");
        assert_eq!(create_sample(&settings), Ok(()));
        let file = library_path(&settings);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), SHIPPED);
        assert_eq!(load(&settings), parse_library(SHIPPED));
        // the operator edits it; asking again changes nothing
        std::fs::write(&file, "material = Mine | feed=1,2\n").unwrap();
        assert_eq!(create_sample(&settings), Err(SaveError::Exists));
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            "material = Mine | feed=1,2\n"
        );
        let _ = std::fs::remove_dir_all(&d);
    }
}
