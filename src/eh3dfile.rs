//! The branch `cnc/` of an `.eh3d` project: one file with a branch for each program of the suite (the format is the crate `eh3d-project` of EH3D-COMMON).
//!
//! This program writes only `cnc/`. What it takes from another branch is a copy with a reference that remembers the content hash and the format of its source, so that a later change
//! of the source is told as out of date. Every other branch, known or not, keeps its bytes. Nothing in a project is run and nothing here moves a machine.

use std::collections::BTreeMap;
use std::path::Path;

use eh3d_project::appbranch::{
    self, counted, extension_of, file_name, read_bounded, BranchError, BranchKit, BranchResult,
    Conversion, Notice,
};
use eh3d_project::meta::ConversionReport;
use eh3d_project::Project;

use crate::import::{self, Format};
use crate::motioncfg::{ImportCfg, MotionCfg};
use crate::tools::ToolTable;

pub use eh3d_project::appbranch::{Identity, Listing, Row, EXTENSION};

/// The branch this program owns.
pub const BRANCH: &str = "cnc";
/// The format of the branch, `major.minor`.
pub const FORMAT: &str = "1.0";
/// The record of the branch that says which of its files is what.
pub const MANIFEST: &str = "cnc.json";
/// A program larger than this is not put into a project.
const MAX_PROGRAM_BYTES: u64 = 64 << 20;
const PROGRAM_DIR: &str = "programs/";
const DRAWING_DIR: &str = "drawing/";

const KIT: BranchKit<'static> = BranchKit::new(BRANCH, FORMAT, MANIFEST, env!("CARGO_PKG_VERSION"));

/// What `save` is given: only what is there replaces what the branch holds.
#[derive(Debug, Default, Clone)]
pub struct Inputs {
    /// The G-code program.
    pub program: Option<String>,
    /// The drawing (HPGL, DXF or SVG).
    pub drawing: Option<String>,
    /// The table of tools.
    pub tools: Option<String>,
    /// The motion file.
    pub motion: Option<String>,
}

/// What the branch `cnc/` holds.
#[derive(Debug, Clone, Default)]
pub struct Opened {
    /// The program of the work, as a name inside the branch.
    pub program: Option<String>,
    /// How many lines and moves it has.
    pub moves: Option<(usize, usize)>,
    /// The drawing.
    pub drawing: Option<String>,
    /// The table of tools.
    pub tools: Option<String>,
    /// The motion file.
    pub motion: Option<String>,
    /// The report of the last import.
    pub report: Option<ConversionReport>,
    /// What is out of date or wrong.
    pub warnings: Vec<String>,
    /// The files written out, when the branch was taken out.
    pub written: Option<usize>,
    /// The bytes of every file of the branch, by its name inside it.
    pub files: BTreeMap<String, Vec<u8>>,
}

/// A file on its way into the branch: the name it is told by and its bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    /// The name of the file, or the path it was read from.
    pub name: String,
    /// What it holds.
    pub bytes: Vec<u8>,
}

/// What [`save_contents`] puts into the branch: only what is there replaces what the branch holds.
#[derive(Debug, Default, Clone)]
pub struct Contents {
    /// The G-code program, under the name of its file.
    pub program: Option<Held>,
    /// The drawing, under the name of its file.
    pub drawing: Option<Held>,
    /// The table of tools.
    pub tools: Option<Held>,
    /// The motion file.
    pub motion: Option<Held>,
}

impl Contents {
    /// Whether there is nothing to put.
    pub fn is_empty(&self) -> bool {
        self.program.is_none()
            && self.drawing.is_none()
            && self.tools.is_none()
            && self.motion.is_none()
    }
}

fn describe(e: BranchError) -> String {
    e.to_string()
}

fn said(notices: Vec<Notice>) -> Vec<String> {
    notices.iter().map(ToString::to_string).collect()
}

pub(crate) fn is_program(name: &str) -> bool {
    matches!(
        extension_of(name).as_str(),
        "nc" | "ngc" | "gcode" | "gco" | "tap" | "cnc"
    )
}

fn is_drawing(name: &str) -> bool {
    matches!(
        extension_of(name).as_str(),
        "dxf" | "svg" | "plt" | "hpgl" | "hgl"
    )
}

/// What kind of thing a file taken from another branch is.
fn artifact_of(name: &str) -> &'static str {
    if is_program(name) {
        "toolpath"
    } else if is_drawing(name) {
        "outline2d"
    } else if matches!(
        extension_of(name).as_str(),
        "stl" | "obj" | "3mf" | "step" | "stp"
    ) {
        "mesh3d"
    } else {
        "file"
    }
}

/// The lines of a program: every byte is a character, so a file in any code page is read (as the report reads it).
fn lines_of(bytes: &[u8]) -> Vec<String> {
    let text: String = bytes.iter().map(|&b| b as char).collect();
    text.lines().map(str::to_string).collect()
}

fn read_drawing(name: &str, bytes: &[u8], cfg: &ImportCfg) -> Result<import::Drawing, String> {
    let format = Format::detect(name, bytes)
        .ok_or_else(|| format!("{name}: it is not an HPGL, a DXF or an SVG file"))?;
    import::read(format, bytes, cfg).map_err(|why| format!("{name}: {}", why.text("en")))
}

/// The line of the motion file that holds the credential of a link: a motion file with it never goes into a project.
pub const CREDENTIAL_KEY: &str = "klipper.api_key";

/// The motion file given to a command, or the usual values. A file with mistakes is refused.
pub fn motion_of(path: Option<&str>) -> Result<MotionCfg, String> {
    let Some(path) = path else {
        return Ok(MotionCfg::default());
    };
    let bytes =
        read_bounded(Path::new(path), crate::motioncfg::MAX_FILE_BYTES).map_err(describe)?;
    let (cfg, problems) = MotionCfg::parse(&String::from_utf8_lossy(&bytes));
    if problems.is_empty() {
        Ok(cfg)
    } else {
        Err(format!("{path}: {} line(s) are not valid", problems.len()))
    }
}

/// Puts the work of the machine into the branch `cnc/` of the project at `path`, creating the project when it is not there. What is not given stays as it was.
pub fn save(path: &Path, inputs: &Inputs, identity: &Identity) -> Result<Vec<String>, String> {
    // a motion file with mistakes is refused before the other files are read
    let motion = motion_of(inputs.motion.as_deref())?;
    let held = |file: &String, name: String, limit: u64| -> Result<Held, String> {
        let bytes = read_bounded(Path::new(file), limit).map_err(describe)?;
        Ok(Held { name, bytes })
    };
    let named = |file: &String| file_name(Path::new(file)).map_err(describe);
    let small = crate::motioncfg::MAX_FILE_BYTES;
    let contents = Contents {
        program: match &inputs.program {
            Some(file) => Some(held(file, named(file)?, MAX_PROGRAM_BYTES)?),
            None => None,
        },
        drawing: match &inputs.drawing {
            Some(file) => Some(held(file, named(file)?, motion.import.max_bytes)?),
            None => None,
        },
        tools: match &inputs.tools {
            Some(file) => Some(held(file, file.clone(), small)?),
            None => None,
        },
        motion: match &inputs.motion {
            Some(file) => Some(held(file, file.clone(), small)?),
            None => None,
        },
    };
    save_contents(path, &contents, identity, &motion)
}

/// The same as [`save`] with the files already read, which is how the window gives what it holds. A drawing is read with the importers of `motion`. Everything is checked before anything is written.
pub fn save_contents(
    path: &Path,
    contents: &Contents,
    identity: &Identity,
    motion: &MotionCfg,
) -> Result<Vec<String>, String> {
    if contents.is_empty() {
        return Err(
            "there is nothing to save: give a program, a drawing, tools or a motion file".into(),
        );
    }
    if let Some(held) = &contents.motion {
        // a project is a file that is passed on: the credential of a link never goes into it
        if eh3d_project::suiteview::sets_key(&held.bytes, CREDENTIAL_KEY) {
            return Err(format!(
                "{}: it holds {CREDENTIAL_KEY}, the credential of the link, so it is not put in the project (take the line out or leave the motion file out)",
                held.name
            ));
        }
        let (_, problems) = MotionCfg::parse(&String::from_utf8_lossy(&held.bytes));
        if !problems.is_empty() {
            return Err(format!(
                "{}: {} line(s) are not valid",
                held.name,
                problems.len()
            ));
        }
    }
    let program = contents
        .program
        .as_ref()
        .map(|held| (format!("{PROGRAM_DIR}{}", held.name), held.bytes.clone()));
    let drawing = match &contents.drawing {
        Some(held) => {
            read_drawing(&held.name, &held.bytes, &motion.import)?;
            Some((format!("{DRAWING_DIR}{}", held.name), held.bytes.clone()))
        }
        None => None,
    };
    if let Some(held) = &contents.tools {
        let table = ToolTable::parse(&String::from_utf8_lossy(&held.bytes));
        if !table.issues.is_empty() {
            return Err(format!(
                "{}: {} line(s) of the table of tools are not valid",
                held.name,
                table.issues.len()
            ));
        }
    }
    for (rel, _) in program.iter().chain(drawing.iter()) {
        KIT.check_name(rel).map_err(describe)?;
    }

    let (_, notices) = KIT
        .update(path, identity, |_, copy| {
            if let Some((rel, bytes)) = program {
                copy.put(&rel, bytes)?;
                copy.choose("program", PROGRAM_DIR, &rel);
            }
            if let Some((rel, bytes)) = drawing {
                copy.put(&rel, bytes)?;
                copy.choose("drawing", DRAWING_DIR, &rel);
            }
            if let Some(held) = &contents.tools {
                copy.put("settings/tools.txt", held.bytes.clone())?;
                copy.set("tools", "settings/tools.txt");
            }
            if let Some(held) = &contents.motion {
                copy.put("settings/motion.txt", held.bytes.clone())?;
                copy.set("motion", "settings/motion.txt");
            }
            Ok(())
        })
        .map_err(describe)?;
    Ok(said(notices))
}

/// The report of an import and, for a drawing, the name and the bytes of the program made from it.
type Converted = (ConversionReport, Option<(String, Vec<u8>)>);

/// What an import of one file gives.
fn convert(name: &str, bytes: &[u8], motion: &MotionCfg) -> Result<Converted, String> {
    let mut got = Vec::new();
    let mut not_read = Vec::new();
    let mut export_losses = Vec::new();
    let (format, program) = if is_program(name) {
        let lines = lines_of(bytes);
        let moves = crate::gcode::toolpath(&lines).len();
        if moves == 0 {
            return Err(format!("{name}: no move was found in it"));
        }
        got.push(counted("lines", lines.len(), None));
        got.push(counted("moves", moves, None));
        got.push(counted("arcs", crate::gcode::arc_moves(&lines), None));
        let faults = crate::gcode::arc_faults(&lines).len();
        if faults > 0 {
            not_read.push(counted(
                "arcs",
                faults,
                Some("arcs whose curve cannot be worked out as they are written"),
            ));
        }
        ("gcode".to_string(), None)
    } else if is_drawing(name) {
        let mut drawing = read_drawing(name, bytes, &motion.import)?;
        got.push(counted("paths", drawing.paths.len(), None));
        got.push(counted(
            "points",
            drawing.paths.iter().map(|p| p.points.len()).sum(),
            None,
        ));
        for note in &drawing.notes {
            not_read.push(counted("note", 1, Some(note.text("en"))));
        }
        export_losses.push(counted(
            "drawing",
            drawing.paths.len(),
            Some("a G-code program holds cutting moves, not the entities of the drawing; the original is kept"),
        ));
        import::to_origin(&mut drawing);
        let mut text = import::to_gcode(&drawing, &motion.import).join("\n");
        text.push('\n');
        let stem = Path::new(name).file_stem().map_or_else(
            || "drawing".to_string(),
            |s| s.to_string_lossy().into_owned(),
        );
        let format = extension_of(name);
        (
            format,
            Some((format!("{PROGRAM_DIR}{stem}.nc"), text.into_bytes())),
        )
    } else {
        return Err(format!(
            "{name}: it is not a G-code program, an HPGL, a DXF or an SVG file"
        ));
    };
    let conversion = Conversion {
        read: got,
        not_read,
        export_losses,
    };
    let report = appbranch::conversion_report(
        "phoenix-cnc",
        env!("CARGO_PKG_VERSION"),
        &format,
        name,
        bytes,
        conversion,
    );
    Ok((report, program))
}

/// Imports a G-code program or a drawing (DXF, SVG, HPGL) of another program: the original is kept byte for byte under `cnc/origin/` with a report of the conversion. A program becomes the
/// program of the work; a drawing becomes the drawing and the program that cuts along it, with the values of the motion file, is made beside it.
pub fn import_file(
    path: &Path,
    source: &Path,
    identity: &Identity,
    motion: &MotionCfg,
) -> Result<(ConversionReport, Vec<String>), String> {
    let name = file_name(source).map_err(describe)?;
    let bytes = read_bounded(source, MAX_PROGRAM_BYTES).map_err(describe)?;
    let (report, program) = convert(&name, &bytes, motion)?;
    let notices = KIT
        .import(path, identity, bytes, &report, |copy, original| {
            match program {
                Some((rel, made)) => {
                    copy.choose("drawing", DRAWING_DIR, original);
                    copy.put(&rel, made)?;
                    copy.choose("program", PROGRAM_DIR, &rel);
                }
                None => copy.choose("program", PROGRAM_DIR, original),
            }
            // a name of the record that pointed at the import that is gone does not stay
            copy.forget_missing(&["program", "drawing"]);
            Ok(())
        })
        .map_err(describe)?;
    Ok((report, said(notices)))
}

/// Copies a file of another branch into `cnc/taken/`, with a reference that remembers the state of its source. A program becomes the program of the work and a drawing the drawing.
pub fn take(
    path: &Path,
    branch: &str,
    rel: &str,
    motion: &MotionCfg,
) -> Result<(String, Vec<String>), String> {
    let artifact = artifact_of(rel);
    let check = |bytes: &[u8]| -> BranchResult<()> {
        if artifact == "outline2d" {
            read_drawing(rel, bytes, &motion.import).map_err(BranchError::Refused)?;
        } else if artifact == "toolpath" && bytes.len() as u64 > MAX_PROGRAM_BYTES {
            return Err(BranchError::Refused(format!(
                "{branch}/{rel}: the program is too large"
            )));
        }
        Ok(())
    };
    let (target, notices) = KIT
        .take(
            path,
            branch,
            rel,
            artifact,
            check,
            |copy, target| match artifact {
                "outline2d" => copy.choose("drawing", DRAWING_DIR, target),
                "toolpath" => copy.choose("program", PROGRAM_DIR, target),
                _ => {}
            },
        )
        .map_err(describe)?;
    Ok((target, said(notices)))
}

/// The branches of a project, with what is out of date.
pub fn list(path: &Path) -> Result<Listing, String> {
    appbranch::list(path).map_err(describe)
}

fn read_branch(path: &Path) -> Result<(Project, Opened), String> {
    let (project, copy, warnings) = KIT.open(path).map_err(describe)?;
    let opened = Opened {
        program: copy.text("program"),
        drawing: copy.text("drawing"),
        tools: copy.text("tools"),
        motion: copy.text("motion"),
        warnings,
        report: copy.report().map_err(describe)?,
        files: copy.files().clone(),
        ..Opened::default()
    };
    Ok((project, opened))
}

/// What the branch `cnc/` holds, with the bytes of its files and without reading the program.
pub fn held(path: &Path) -> Result<Opened, String> {
    read_branch(path).map(|(_, opened)| opened)
}

/// The lines of a program of a branch, read as the report reads them.
pub fn program_lines(bytes: &[u8]) -> Vec<String> {
    lines_of(bytes)
}

/// Opens the branch `cnc/`: reads and checks what it holds and, with `into`, writes its files there (a file that is there is never written over).
pub fn open(path: &Path, into: Option<&Path>) -> Result<Opened, String> {
    let (mut project, mut opened) = read_branch(path)?;
    if let Some(rel) = &opened.program {
        let bytes = opened
            .files
            .get(rel)
            .ok_or_else(|| format!("{BRANCH}/{rel}: the program is not in the branch"))?;
        let lines = lines_of(bytes);
        opened.moves = Some((lines.len(), crate::gcode::toolpath(&lines).len()));
    }
    if let Some(dir) = into {
        opened.written = Some(
            project
                .extract_branch(BRANCH, dir)
                .map_err(|e| e.to_string())?,
        );
    }
    Ok(opened)
}

/// Writes the files of any branch into a folder, byte for byte (a file that is there is never written over).
pub fn extract(path: &Path, branch: &str, dir: &Path) -> Result<usize, String> {
    appbranch::extract(path, branch, dir).map_err(describe)
}
