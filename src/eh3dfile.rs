//! The branch `cnc/` of an `.eh3d` project: one file with a branch for each program of the suite (the format is the crate `eh3d-project` of EH3D-COMMON).
//!
//! This program writes only `cnc/`. What it takes from another branch is a copy with a reference that remembers the content hash and the format of its source, so that a later change
//! of the source is told as out of date. Every other branch, known or not, keeps its bytes. Nothing in a project is run and nothing here moves a machine.

use std::collections::BTreeMap;
use std::fs;
use std::hash::{BuildHasher, Hasher, RandomState};
use std::io::Read;
use std::path::Path;

use eh3d_container::{check_entry_name, sha256_hex, AtomicOptions};
use eh3d_project::meta::{
    ConversionReport, Converter, Counted, SourceFile, ORIGIN_DIR, REPORT_FILE,
};
use eh3d_project::{
    BranchUpdate, Checks, Dependency, NewProject, OpenOptions, Project, SaveOptions, Writer,
};
use serde_json::{Map, Value};

use crate::import::{self, Format};
use crate::motioncfg::{ImportCfg, MotionCfg};
use crate::tools::ToolTable;

/// The branch this program owns.
pub const BRANCH: &str = "cnc";
/// The format of the branch, `major.minor`.
pub const FORMAT: &str = "1.0";
/// The record of the branch that says which of its files is what.
pub const MANIFEST: &str = "cnc.json";
/// The extension of a project.
pub const EXTENSION: &str = "eh3d";
/// A program larger than this is not put into a project.
const MAX_PROGRAM_BYTES: u64 = 64 << 20;
const PROGRAM_DIR: &str = "programs/";
const DRAWING_DIR: &str = "drawing/";
const TAKEN_DIR: &str = "taken/";

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

/// The name and the identifier of a project that is being created.
#[derive(Debug, Default, Clone)]
pub struct Identity {
    /// The name (the name of the file when there is none).
    pub name: Option<String>,
    /// The identifier (a random UUID when there is none).
    pub id: Option<String>,
}

/// One branch, as the list shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The name of the branch.
    pub id: String,
    /// The program that wrote it (`-` without a record).
    pub app: String,
    /// The format of the branch.
    pub format: String,
    /// The version of that program.
    pub app_version: String,
    /// The files of the branch, `branch.json` included.
    pub files: usize,
    /// The content hash.
    pub hash: String,
    /// What is worth saying about it.
    pub notes: Vec<&'static str>,
}

/// What `list` finds.
#[derive(Debug, Clone)]
pub struct Listing {
    /// The name of the project.
    pub name: String,
    /// Its identifier.
    pub id: String,
    /// The branches.
    pub rows: Vec<Row>,
    /// The derived branches whose sources changed, and what the checks found wrong.
    pub warnings: Vec<String>,
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
}

/// The branch as it is read, changed and written back.
#[derive(Default)]
struct Own {
    files: BTreeMap<String, Vec<u8>>,
    manifest: Map<String, Value>,
    depends: Vec<Dependency>,
    extra: BTreeMap<String, Value>,
}

impl Own {
    fn text(&self, key: &str) -> Option<String> {
        self.manifest
            .get(key)
            .and_then(Value::as_str)
            .map(str::to_owned)
    }

    fn set(&mut self, key: &str, value: &str) {
        self.manifest
            .insert(key.to_owned(), Value::from(value.to_owned()));
    }

    /// Puts a file in the branch, checking that its name can live in a project.
    fn put(&mut self, rel: &str, bytes: Vec<u8>) -> Result<(), String> {
        check_entry_name(&format!("{BRANCH}/{rel}"))
            .map_err(|_| format!("'{rel}' is not a name that can be kept in a project"))?;
        self.files.insert(rel.to_owned(), bytes);
        Ok(())
    }

    /// Takes a file out of the branch, with the references that only it used.
    fn remove(&mut self, rel: &str) {
        self.files.remove(rel);
        for dep in &mut self.depends {
            dep.uses.retain(|u| u != rel);
        }
        self.depends.retain(|d| !d.uses.is_empty());
    }

    /// Makes `rel` the program or the drawing (`key`). The one before goes when it was a copy of this program's own (`dir`) or a taken one;
    /// the original of an import stays with its report.
    fn choose(&mut self, key: &str, dir: &str, rel: &str) {
        if let Some(old) = self.text(key) {
            if old != rel && (old.starts_with(dir) || old.starts_with(TAKEN_DIR)) {
                self.remove(&old);
            }
        }
        self.set(key, rel);
    }
}

fn describe(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn open_project(path: &Path) -> Result<Project, String> {
    Project::open(path, &OpenOptions::default()).map_err(|e| format!("{}: {e}", path.display()))
}

fn name_of(path: &Path) -> Result<String, String> {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(str::to_owned)
        .ok_or_else(|| format!("{}: the file has no usable name", path.display()))
}

fn extension_of(name: &str) -> String {
    Path::new(name)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

fn is_program(name: &str) -> bool {
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

/// Reads a file whole, never more than `limit` bytes of it.
fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let file = fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    if bytes.len() as u64 > limit {
        return Err(format!(
            "{}: the file is larger than {limit} bytes",
            path.display()
        ));
    }
    Ok(bytes)
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

/// The motion file given to a command, or the usual values. A file with mistakes is refused.
pub fn motion_of(path: Option<&str>) -> Result<MotionCfg, String> {
    let Some(path) = path else {
        return Ok(MotionCfg::default());
    };
    let bytes = read_bounded(Path::new(path), crate::motioncfg::MAX_FILE_BYTES)?;
    let (cfg, problems) = MotionCfg::parse(&String::from_utf8_lossy(&bytes));
    if problems.is_empty() {
        Ok(cfg)
    } else {
        Err(format!("{path}: {} line(s) are not valid", problems.len()))
    }
}

/// A random identifier in the shape of a UUID of version 4.
fn new_id() -> String {
    let mut b = [0u8; 16];
    for half in b.chunks_mut(8) {
        half.copy_from_slice(&RandomState::new().build_hasher().finish().to_le_bytes());
    }
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}

/// Opens the project at `path`, or creates it when there is no file there.
fn open_or_create(path: &Path, identity: &Identity) -> Result<Project, String> {
    if path.exists() {
        if identity.name.is_some() || identity.id.is_some() {
            return Err("--name and --id are only for a project that is being created".to_string());
        }
        return open_project(path);
    }
    if extension_of(&name_of(path)?) != EXTENSION {
        return Err(format!(
            "{}: a new project is a file that ends in .{EXTENSION}",
            path.display()
        ));
    }
    let stem = path.file_stem().map_or_else(
        || "project".to_string(),
        |s| s.to_string_lossy().into_owned(),
    );
    let new = NewProject {
        id: identity.id.clone().unwrap_or_else(new_id),
        name: identity.name.clone().unwrap_or(stem),
        units: "millimetres".to_string(),
        created: None,
    };
    Project::create(
        path,
        new,
        &OpenOptions::default(),
        &AtomicOptions::default(),
    )
    .map_err(|e| format!("{}: {e}", path.display()))
}

/// Reads the branch of this program, empty when the project has none yet.
fn load_own(project: &mut Project) -> Result<Own, String> {
    let Some(branch) = project.branch(BRANCH) else {
        return Ok(Own::default());
    };
    let record = branch
        .record
        .clone()
        .ok_or_else(|| format!("EH3D-PKG-015: {BRANCH}/branch.json is missing or not valid"))?;
    if record.format != FORMAT {
        return Err(format!(
            "EH3D-PKG-024: the branch {BRANCH} has the format {}, and this program handles {FORMAT}",
            record.format
        ));
    }
    let names: Vec<String> = branch
        .files
        .keys()
        .filter(|n| *n != "branch.json")
        .cloned()
        .collect();
    let mut own = Own {
        depends: record.depends,
        extra: record.extra,
        ..Own::default()
    };
    for name in names {
        let bytes = project.read_entry(BRANCH, &name).map_err(describe)?;
        own.files.insert(name, bytes);
    }
    if let Some(bytes) = own.files.get(MANIFEST) {
        own.manifest = serde_json::from_slice(bytes)
            .map_err(|e| format!("{BRANCH}/{MANIFEST} is not a valid record: {e}"))?;
    }
    Ok(own)
}

/// Writes the branch of this program. Nothing else of the project changes but its two records.
fn write_own(project: Project, mut own: Own) -> Result<Vec<String>, String> {
    let mut notes = Vec::new();
    if project.record().signature.is_some() {
        notes.push(
            "the signature of the project does not hold after a change and is removed".into(),
        );
    }
    own.manifest.insert("schema".to_string(), Value::from(1));
    let mut text = serde_json::to_vec_pretty(&own.manifest).map_err(describe)?;
    text.push(b'\n');
    own.files.insert(MANIFEST.to_string(), text);
    let mut update = BranchUpdate::new(BRANCH, FORMAT, env!("CARGO_PKG_VERSION"));
    update.depends = own
        .depends
        .into_iter()
        .filter(|d| project.branch(&d.branch).is_some())
        .collect();
    update.entries = own.files;
    update.extra = own.extra;
    project
        .save_branch(&Writer::for_app(BRANCH), update, &SaveOptions::default())
        .map_err(describe)?;
    Ok(notes)
}

/// Puts the work of the machine into the branch `cnc/` of the project at `path`, creating the project when it is not there. What is not given stays as it was.
pub fn save(path: &Path, inputs: &Inputs, identity: &Identity) -> Result<Vec<String>, String> {
    if inputs.program.is_none()
        && inputs.drawing.is_none()
        && inputs.tools.is_none()
        && inputs.motion.is_none()
    {
        return Err(
            "there is nothing to save: give a program, a drawing, tools or a motion file".into(),
        );
    }
    // every file is read and checked before anything is written
    let motion = motion_of(inputs.motion.as_deref())?;
    let program = match &inputs.program {
        Some(file) => {
            let bytes = read_bounded(Path::new(file), MAX_PROGRAM_BYTES)?;
            Some((format!("{PROGRAM_DIR}{}", name_of(Path::new(file))?), bytes))
        }
        None => None,
    };
    let drawing = match &inputs.drawing {
        Some(file) => {
            let bytes = read_bounded(Path::new(file), motion.import.max_bytes)?;
            let name = name_of(Path::new(file))?;
            read_drawing(&name, &bytes, &motion.import)?;
            Some((format!("{DRAWING_DIR}{name}"), bytes))
        }
        None => None,
    };
    let tools = match &inputs.tools {
        Some(file) => {
            let bytes = read_bounded(Path::new(file), crate::motioncfg::MAX_FILE_BYTES)?;
            let table = ToolTable::parse(&String::from_utf8_lossy(&bytes));
            if !table.issues.is_empty() {
                return Err(format!(
                    "{file}: {} line(s) of the table of tools are not valid",
                    table.issues.len()
                ));
            }
            Some(bytes)
        }
        None => None,
    };
    let motion_bytes = match &inputs.motion {
        Some(file) => Some(read_bounded(
            Path::new(file),
            crate::motioncfg::MAX_FILE_BYTES,
        )?),
        None => None,
    };
    let mut checked = Own::default();
    for (rel, _) in program.iter().chain(drawing.iter()) {
        checked.put(rel, Vec::new())?;
    }

    let mut project = open_or_create(path, identity)?;
    let mut own = load_own(&mut project)?;
    if let Some((rel, bytes)) = program {
        own.put(&rel, bytes)?;
        own.choose("program", PROGRAM_DIR, &rel);
    }
    if let Some((rel, bytes)) = drawing {
        own.put(&rel, bytes)?;
        own.choose("drawing", DRAWING_DIR, &rel);
    }
    if let Some(bytes) = tools {
        own.put("settings/tools.txt", bytes)?;
        own.set("tools", "settings/tools.txt");
    }
    if let Some(bytes) = motion_bytes {
        own.put("settings/motion.txt", bytes)?;
        own.set("motion", "settings/motion.txt");
    }
    write_own(project, own)
}

fn counted(item: &str, count: usize, reason: Option<&str>) -> Counted {
    Counted {
        item: item.to_string(),
        count: count as u64,
        reason: reason.map(str::to_owned),
    }
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
    let report = ConversionReport {
        schema: 1,
        source: SourceFile {
            format,
            name: name.to_string(),
            sha256: sha256_hex(bytes),
            size: bytes.len() as u64,
        },
        converter: Converter {
            name: "phoenix-cnc".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        },
        original: Some(format!("{ORIGIN_DIR}{name}")),
        read: got,
        not_read,
        export_losses,
        extra: BTreeMap::new(),
    };
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
    let name = name_of(source)?;
    let bytes = read_bounded(source, MAX_PROGRAM_BYTES)?;
    let (report, program) = convert(&name, &bytes, motion)?;
    let original = format!("{ORIGIN_DIR}{name}");
    Own::default().put(&original, Vec::new())?;

    let mut project = open_or_create(path, identity)?;
    let mut own = load_own(&mut project)?;
    let before: Vec<String> = own
        .files
        .keys()
        .filter(|n| n.starts_with(ORIGIN_DIR))
        .cloned()
        .collect();
    for old in &before {
        own.remove(old);
    }
    own.put(&original, bytes)?;
    let mut text = serde_json::to_vec_pretty(&report).map_err(describe)?;
    text.push(b'\n');
    own.put(REPORT_FILE, text)?;
    match program {
        Some((rel, made)) => {
            own.choose("drawing", DRAWING_DIR, &original);
            own.put(&rel, made)?;
            own.choose("program", PROGRAM_DIR, &rel);
        }
        None => own.choose("program", PROGRAM_DIR, &original),
    }
    // a name of the record that pointed at the import that is gone does not stay
    for key in ["program", "drawing"] {
        if own.text(key).is_some_and(|v| !own.files.contains_key(&v)) {
            own.manifest.remove(key);
        }
    }
    let mut notes = write_own(project, own)?;
    if before.iter().any(|n| n != REPORT_FILE && *n != original) {
        notes.push("the import that was there before was replaced".to_string());
    }
    Ok((report, notes))
}

/// Copies a file of another branch into `cnc/taken/`, with a reference that remembers the state of its source. A program becomes the program of the work and a drawing the drawing.
pub fn take(
    path: &Path,
    branch: &str,
    rel: &str,
    motion: &MotionCfg,
) -> Result<(String, Vec<String>), String> {
    if branch == BRANCH {
        return Err("a branch takes from another branch, not from itself".to_string());
    }
    let mut project = open_project(path)?;
    let bytes = project.read_entry(branch, rel).map_err(describe)?;
    let artifact = artifact_of(rel);
    if artifact == "outline2d" {
        read_drawing(rel, &bytes, &motion.import)?;
    } else if artifact == "toolpath" && bytes.len() as u64 > MAX_PROGRAM_BYTES {
        return Err(format!("{branch}/{rel}: the program is too large"));
    }
    let target = format!("{TAKEN_DIR}{branch}/{rel}");
    let dep = project
        .dependency_on(branch, artifact, 1, vec![target.clone()])
        .map_err(describe)?;
    let mut own = load_own(&mut project)?;
    own.remove(&target);
    own.put(&target, bytes)?;
    own.depends.push(dep);
    match artifact {
        "outline2d" => own.choose("drawing", DRAWING_DIR, &target),
        "toolpath" => own.choose("program", PROGRAM_DIR, &target),
        _ => {}
    }
    let notes = write_own(project, own)?;
    Ok((target, notes))
}

/// The derived branches whose sources changed, and what the checks of the container find wrong.
fn warnings_of(project: &mut Project) -> Vec<String> {
    let mut out: Vec<String> = project
        .stale_refs()
        .iter()
        .map(|s| {
            format!(
                "OUT OF DATE: '{}' was derived from '{}' ({}: {}) before it changed",
                s.branch,
                s.dependency.branch,
                s.dependency.artifact,
                s.dependency.uses.join(", ")
            )
        })
        .collect();
    for f in project.diagnose(&Checks::default()) {
        if f.is_error() {
            out.push(format!(
                "{}: {} [{}]",
                f.code,
                f.detail,
                f.branch.as_deref().unwrap_or("project")
            ));
        }
    }
    out
}

/// The branches of a project, with what is out of date.
pub fn list(path: &Path) -> Result<Listing, String> {
    let mut project = open_project(path)?;
    let warnings = warnings_of(&mut project);
    let stale = project.stale_refs();
    let rows = project
        .branches()
        .map(|b| {
            let r = b.record.as_ref();
            let mut notes = Vec::new();
            if !b.is_known() {
                notes.push("not known by name, kept as it is");
            }
            if b.is_sealed() {
                notes.push("sealed");
            }
            if b.is_encrypted() {
                notes.push("encrypted");
            }
            if stale.iter().any(|s| s.branch == b.id) {
                notes.push("OUT OF DATE");
            }
            let of = |v: Option<&String>| v.cloned().unwrap_or_else(|| "-".to_string());
            Row {
                id: b.id.clone(),
                app: of(r.map(|r| &r.app)),
                format: of(r.map(|r| &r.format)),
                app_version: of(r.map(|r| &r.app_version)),
                files: b.files.len(),
                hash: b.hash.clone(),
                notes,
            }
        })
        .collect();
    Ok(Listing {
        name: project.record().name.clone(),
        id: project.record().id.clone(),
        rows,
        warnings,
    })
}

/// Opens the branch `cnc/`: reads and checks what it holds and, with `into`, writes its files there (a file that is there is never written over).
pub fn open(path: &Path, into: Option<&Path>) -> Result<Opened, String> {
    let mut project = open_project(path)?;
    if project.branch(BRANCH).is_none() {
        return Err(format!(
            "{}: the project has no branch {BRANCH}",
            path.display()
        ));
    }
    let own = load_own(&mut project)?;
    let mut opened = Opened {
        program: own.text("program"),
        drawing: own.text("drawing"),
        tools: own.text("tools"),
        motion: own.text("motion"),
        warnings: warnings_of(&mut project),
        ..Opened::default()
    };
    if let Some(rel) = &opened.program {
        let bytes = own
            .files
            .get(rel)
            .ok_or_else(|| format!("{BRANCH}/{rel}: the program is not in the branch"))?;
        let lines = lines_of(bytes);
        opened.moves = Some((lines.len(), crate::gcode::toolpath(&lines).len()));
    }
    if let Some(bytes) = own.files.get(REPORT_FILE) {
        opened.report = Some(
            serde_json::from_slice(bytes)
                .map_err(|e| format!("{BRANCH}/{REPORT_FILE} is not a valid record: {e}"))?,
        );
    }
    if let Some(dir) = into {
        opened.written = Some(project.extract_branch(BRANCH, dir).map_err(describe)?);
    }
    Ok(opened)
}

/// Writes the files of any branch into a folder, byte for byte (a file that is there is never written over).
pub fn extract(path: &Path, branch: &str, dir: &Path) -> Result<usize, String> {
    let mut project = open_project(path)?;
    project.extract_branch(branch, dir).map_err(describe)
}
