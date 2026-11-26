//! The branch `cnc/` of an `.eh3d` project: what is saved comes back byte for byte, every other branch (known or not) keeps its bytes, a copy taken from another branch is told as out of
//! date when its source changes, an imported file is kept with a report, and a damaged file is an error and never a crash. Every project here is made up.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use eh3d_container::{sha256_hex, AtomicOptions};
use eh3d_phoenix_cnc::eh3dcli;
use eh3d_phoenix_cnc::eh3dfile::{self, Identity, Inputs};
use eh3d_phoenix_cnc::motioncfg::MotionCfg;
use eh3d_project::{
    BranchUpdate, Ed25519Signer, NewProject, OpenOptions, Project, SaveOptions, Seal, Writer,
};

const DXF: &str = "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nLINE\n8\n0\n10\n0\n20\n0\n11\n10\n21\n0\n0\nCIRCLE\n10\n50\n20\n50\n40\n5\n0\nENDSEC\n0\nEOF\n";

fn example(name: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

/// A folder of its own for one test.
fn folder(tag: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("eh3dfile_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn drawing_in(dir: &Path) -> PathBuf {
    let path = dir.join("outline.dxf");
    fs::write(&path, DXF).unwrap();
    path
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn write_branch(path: &Path, branch: &str, entries: &[(&str, &[u8])]) {
    let project = Project::open(path, &OpenOptions::default()).unwrap();
    let mut update = BranchUpdate::new(branch, "1.0", "0.1.0");
    for (rel, bytes) in entries {
        update = update.with_entry(rel, bytes);
    }
    project
        .save_branch(
            &Writer::new(branch, &[branch]),
            update,
            &SaveOptions::default(),
        )
        .unwrap();
}

/// A project with branches of other programs, one this program does not know and an extension folder.
fn synthetic(path: &Path) {
    let new = NewProject {
        id: "00000000-0000-4000-8000-000000000002".into(),
        name: "Synthetic".into(),
        units: "millimetres".into(),
        created: None,
    };
    Project::create(
        path,
        new,
        &OpenOptions::default(),
        &AtomicOptions::default(),
    )
    .unwrap();
    write_branch(
        path,
        "cad",
        &[
            ("outline.dxf", DXF.as_bytes()),
            ("body.stl", b"solid body\nendsolid body\n"),
        ],
    );
    write_branch(path, "laser", &[("laser.json", b"{}")]);
    write_branch(path, "holography", &[("data.bin", &[0u8, 255, 7, 7, 7])]);
    write_branch(path, "x-vendor", &[("blob", b"anything at all")]);
}

/// Every file of every branch but `cnc`, the records included.
fn others(path: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut project = Project::open(path, &OpenOptions::default()).unwrap();
    let names: Vec<(String, String)> = project
        .branches()
        .filter(|b| b.id != "cnc")
        .flat_map(|b| b.files.keys().map(|f| (b.id.clone(), f.clone())))
        .collect();
    names
        .into_iter()
        .map(|(b, f)| (format!("{b}/{f}"), project.read_entry(&b, &f).unwrap()))
        .collect()
}

fn program() -> Inputs {
    Inputs {
        program: Some(example("01_square.nc")),
        ..Inputs::default()
    }
}

fn motion() -> MotionCfg {
    MotionCfg::default()
}

fn cli(args: &[&str]) -> (i32, String, String) {
    let words: Vec<String> = args.iter().map(|a| (*a).to_string()).collect();
    let done = eh3dcli::run(&words);
    (done.code, done.out, done.err)
}

#[test]
fn a_saved_branch_comes_back_byte_for_byte() {
    let dir = folder("roundtrip");
    let file = dir.join("job.eh3d");
    let drawing = drawing_in(&dir);
    let inputs = Inputs {
        program: Some(example("01_square.nc")),
        drawing: Some(text(&drawing)),
        tools: Some(example("tools.txt")),
        motion: Some(example("motion.txt")),
    };
    let who = Identity {
        name: Some("Bracket".into()),
        id: Some("bracket-1".into()),
    };
    eh3dfile::save(&file, &inputs, &who).unwrap();

    let listing = eh3dfile::list(&file).unwrap();
    assert_eq!(
        (listing.name.as_str(), listing.id.as_str()),
        ("Bracket", "bracket-1")
    );
    assert_eq!(listing.rows.len(), 1);
    assert_eq!(
        (
            listing.rows[0].app.as_str(),
            listing.rows[0].format.as_str()
        ),
        ("cnc", "1.0")
    );
    assert!(listing.warnings.is_empty(), "{:?}", listing.warnings);

    let out = dir.join("out");
    let opened = eh3dfile::open(&file, Some(&out)).unwrap();
    assert_eq!(opened.program.as_deref(), Some("programs/01_square.nc"));
    assert!(opened.moves.unwrap().1 > 0);
    for (stored, source) in [
        ("programs/01_square.nc", example("01_square.nc")),
        ("drawing/outline.dxf", text(&drawing)),
        ("settings/tools.txt", example("tools.txt")),
        ("settings/motion.txt", example("motion.txt")),
    ] {
        assert_eq!(
            fs::read(out.join(stored)).unwrap(),
            fs::read(source).unwrap(),
            "{stored}"
        );
    }
}

#[test]
fn a_new_program_replaces_the_old_one_and_the_rest_stays() {
    let dir = folder("replace");
    let file = dir.join("job.eh3d");
    let first = Inputs {
        program: Some(example("01_square.nc")),
        tools: Some(example("tools.txt")),
        ..Inputs::default()
    };
    eh3dfile::save(&file, &first, &Identity::default()).unwrap();
    let second = Inputs {
        program: Some(example("03_spiral.nc")),
        ..Inputs::default()
    };
    eh3dfile::save(&file, &second, &Identity::default()).unwrap();
    let project = Project::open(&file, &OpenOptions::default()).unwrap();
    let files = &project.branch("cnc").unwrap().files;
    assert!(files.contains_key("programs/03_spiral.nc"));
    assert!(!files.contains_key("programs/01_square.nc"));
    assert!(files.contains_key("settings/tools.txt"));
}

#[test]
fn the_other_branches_keep_their_bytes_whatever_this_program_does() {
    let dir = folder("others");
    let file = dir.join("job.eh3d");
    synthetic(&file);
    let before = others(&file);
    assert!(before.contains_key("holography/data.bin"));
    assert!(before.contains_key("x-vendor/blob"));

    eh3dfile::save(&file, &program(), &Identity::default()).unwrap();
    assert_eq!(others(&file), before);
    eh3dfile::import_file(&file, &drawing_in(&dir), &Identity::default(), &motion()).unwrap();
    assert_eq!(others(&file), before);
    eh3dfile::take(&file, "cad", "outline.dxf", &motion()).unwrap();
    assert_eq!(others(&file), before);

    let listing = eh3dfile::list(&file).unwrap();
    let ids: Vec<&str> = listing.rows.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, ["cad", "cnc", "holography", "laser", "x-vendor"]);
    assert!(listing.rows[2]
        .notes
        .iter()
        .any(|n| n.contains("not known")));
}

#[test]
fn a_copy_taken_from_cad_is_told_as_out_of_date_and_taken_again() {
    let dir = folder("stale");
    let file = dir.join("job.eh3d");
    synthetic(&file);
    let (target, _) = eh3dfile::take(&file, "cad", "outline.dxf", &motion()).unwrap();
    assert_eq!(target, "taken/cad/outline.dxf");

    let project = Project::open(&file, &OpenOptions::default()).unwrap();
    let record = project.branch("cnc").unwrap().record.clone().unwrap();
    assert_eq!(record.depends.len(), 1);
    let dep = &record.depends[0];
    assert_eq!(dep.branch, "cad");
    assert_eq!(dep.hash, project.branch("cad").unwrap().hash);
    assert_eq!(
        (dep.format.as_str(), dep.artifact.as_str()),
        ("1.0", "outline2d")
    );
    assert_eq!(dep.artifact_version, 1);
    drop(project);
    assert!(eh3dfile::list(&file).unwrap().warnings.is_empty());

    write_branch(&file, "cad", &[("outline.dxf", DXF.as_bytes())]);
    let listing = eh3dfile::list(&file).unwrap();
    assert!(listing
        .warnings
        .iter()
        .any(|w| w.starts_with("OUT OF DATE")));
    // saving something else keeps the old state of the reference, so the warning stays
    eh3dfile::save(&file, &program(), &Identity::default()).unwrap();
    assert!(!eh3dfile::open(&file, None).unwrap().warnings.is_empty());
    let (code, _, err) = cli(&["list", &text(&file)]);
    assert_eq!(code, 1);
    assert!(err.contains("OUT OF DATE"), "{err}");

    eh3dfile::take(&file, "cad", "outline.dxf", &motion()).unwrap();
    assert!(eh3dfile::list(&file).unwrap().warnings.is_empty());
}

#[test]
fn a_mesh_is_taken_as_a_reference_and_the_program_stays() {
    let dir = folder("mesh");
    let file = dir.join("job.eh3d");
    synthetic(&file);
    eh3dfile::save(&file, &program(), &Identity::default()).unwrap();
    eh3dfile::take(&file, "cad", "body.stl", &motion()).unwrap();
    let opened = eh3dfile::open(&file, None).unwrap();
    assert_eq!(opened.program.as_deref(), Some("programs/01_square.nc"));
    let project = Project::open(&file, &OpenOptions::default()).unwrap();
    let record = project.branch("cnc").unwrap().record.clone().unwrap();
    assert_eq!(record.depends[0].artifact, "mesh3d");
}

#[test]
fn a_take_that_cannot_be_done_changes_nothing() {
    let dir = folder("refuse");
    let file = dir.join("job.eh3d");
    synthetic(&file);
    eh3dfile::save(&file, &program(), &Identity::default()).unwrap();
    write_branch(&file, "cad", &[("broken.dxf", b"not a drawing at all")]);
    let before = fs::read(&file).unwrap();
    for (branch, rel) in [
        ("cnc", "programs/01_square.nc"),
        ("cad", "missing.dxf"),
        ("nowhere", "outline.dxf"),
        ("cad", "broken.dxf"),
    ] {
        assert!(
            eh3dfile::take(&file, branch, rel, &motion()).is_err(),
            "{branch}/{rel}"
        );
        assert_eq!(fs::read(&file).unwrap(), before, "{branch}/{rel}");
    }
}

#[test]
fn an_imported_program_is_kept_with_its_report() {
    let dir = folder("gcode");
    let file = dir.join("job.eh3d");
    let source = PathBuf::from(example("02_frame_and_pocket.nc"));
    let original = fs::read(&source).unwrap();
    let (report, _) =
        eh3dfile::import_file(&file, &source, &Identity::default(), &motion()).unwrap();
    assert_eq!(report.source.format, "gcode");
    assert_eq!(report.source.sha256, sha256_hex(&original));
    assert_eq!(report.source.size, original.len() as u64);
    assert!(report.read.iter().any(|c| c.item == "moves" && c.count > 0));

    let out = dir.join("out");
    let opened = eh3dfile::open(&file, Some(&out)).unwrap();
    assert_eq!(
        opened.program.as_deref(),
        Some("origin/02_frame_and_pocket.nc")
    );
    assert_eq!(opened.report.as_ref(), Some(&report));
    assert_eq!(
        fs::read(out.join("origin/02_frame_and_pocket.nc")).unwrap(),
        original
    );
    assert!(opened.warnings.is_empty(), "{:?}", opened.warnings);
}

#[test]
fn an_imported_drawing_becomes_a_program_beside_the_original() {
    let dir = folder("dxf");
    let file = dir.join("job.eh3d");
    let (report, _) =
        eh3dfile::import_file(&file, &drawing_in(&dir), &Identity::default(), &motion()).unwrap();
    assert_eq!(report.source.format, "dxf");
    assert!(report
        .read
        .iter()
        .any(|c| c.item == "paths" && c.count == 2));
    assert!(!report.export_losses.is_empty());
    let out = dir.join("out");
    let opened = eh3dfile::open(&file, Some(&out)).unwrap();
    assert_eq!(opened.drawing.as_deref(), Some("origin/outline.dxf"));
    assert_eq!(opened.program.as_deref(), Some("programs/outline.nc"));
    assert!(opened.moves.unwrap().1 > 0);
    assert_eq!(
        fs::read(out.join("origin/outline.dxf")).unwrap(),
        DXF.as_bytes()
    );

    // a second import replaces the first and says so; nothing keeps pointing at what is gone
    let (_, notes) = eh3dfile::import_file(
        &file,
        Path::new(&example("01_square.nc")),
        &Identity::default(),
        &motion(),
    )
    .unwrap();
    assert!(notes.iter().any(|n| n.contains("replaced")), "{notes:?}");
    let opened = eh3dfile::open(&file, None).unwrap();
    assert_eq!(opened.program.as_deref(), Some("origin/01_square.nc"));
    assert_eq!(opened.drawing, None);
}

#[test]
fn a_branch_of_a_newer_format_or_sealed_is_not_written() {
    let dir = folder("newer");
    let file = dir.join("job.eh3d");
    synthetic(&file);
    let project = Project::open(&file, &OpenOptions::default()).unwrap();
    let update = BranchUpdate::new("cnc", "2.0", "9.0.0").with_entry("cnc.json", b"{}");
    project
        .save_branch(&Writer::for_app("cnc"), update, &SaveOptions::default())
        .unwrap();
    let before = fs::read(&file).unwrap();
    let error = eh3dfile::save(&file, &program(), &Identity::default()).unwrap_err();
    assert!(error.contains("EH3D-PKG-024"), "{error}");
    assert_eq!(fs::read(&file).unwrap(), before);

    let sealed = dir.join("sealed.eh3d");
    synthetic(&sealed);
    let project = Project::open(&sealed, &OpenOptions::default()).unwrap();
    let mut update = BranchUpdate::new("cnc", "1.0", "0.1.0").with_entry("cnc.json", b"{}");
    update.seal = Some(Seal {
        by: "test".into(),
        at: "never".into(),
        reason: None,
    });
    project
        .save_branch(&Writer::for_app("cnc"), update, &SaveOptions::default())
        .unwrap();
    let before = fs::read(&sealed).unwrap();
    let error = eh3dfile::save(&sealed, &program(), &Identity::default()).unwrap_err();
    assert!(error.contains("EH3D-PKG-020"), "{error}");
    assert_eq!(fs::read(&sealed).unwrap(), before);
}

#[test]
fn fields_and_files_this_version_does_not_know_stay() {
    let dir = folder("unknown_fields");
    let file = dir.join("job.eh3d");
    synthetic(&file);
    let project = Project::open(&file, &OpenOptions::default()).unwrap();
    let mut update = BranchUpdate::new("cnc", "1.0", "0.9.0")
        .with_entry("cnc.json", b"{\"future\":{\"k\":1}}")
        .with_entry("later/thing.bin", &[1, 2, 3]);
    update
        .extra
        .insert("hint".into(), serde_json::Value::from("kept"));
    project
        .save_branch(&Writer::for_app("cnc"), update, &SaveOptions::default())
        .unwrap();
    eh3dfile::save(&file, &program(), &Identity::default()).unwrap();
    let mut project = Project::open(&file, &OpenOptions::default()).unwrap();
    assert_eq!(
        project.read_entry("cnc", "later/thing.bin").unwrap(),
        [1, 2, 3]
    );
    let manifest: serde_json::Value =
        serde_json::from_slice(&project.read_entry("cnc", "cnc.json").unwrap()).unwrap();
    assert_eq!(manifest["future"]["k"], 1);
    let record = project.branch("cnc").unwrap().record.clone().unwrap();
    assert_eq!(record.extra["hint"], "kept");
}

#[test]
fn a_signed_branch_is_listed_and_a_change_says_the_signature_is_gone() {
    let dir = folder("signed");
    let file = dir.join("job.eh3d");
    synthetic(&file);
    eh3dfile::save(&file, &program(), &Identity::default()).unwrap();
    let key = Ed25519Signer::from_seed(&[7; 32]);
    Project::open(&file, &OpenOptions::default())
        .unwrap()
        .sign_branch("cnc", &key, &SaveOptions::default())
        .unwrap();
    let signed = || {
        eh3dfile::list(&file)
            .unwrap()
            .rows
            .iter()
            .any(|r| r.id == "cnc" && r.notes.contains(&"signed"))
    };
    assert!(signed());
    let notes = eh3dfile::save(&file, &program(), &Identity::default()).unwrap();
    assert!(
        notes.iter().any(|n| n.contains("signature of the branch")),
        "{notes:?}"
    );
    assert!(!signed());
}

#[test]
fn damaged_projects_are_errors_and_never_a_crash() {
    let dir = folder("damaged");
    let file = dir.join("job.eh3d");
    synthetic(&file);
    eh3dfile::save(&file, &program(), &Identity::default()).unwrap();
    let good = fs::read(&file).unwrap();
    // a fixed seed: the same damage every run
    let mut seed: u64 = 0x5EED_C4C0;
    let mut next = move |limit: usize| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % limit.max(1) as u64) as usize
    };
    let broken = dir.join("broken.eh3d");
    for round in 0..150 {
        let mut bytes = good.clone();
        match round % 3 {
            0 => {
                for _ in 0..=next(8) {
                    let at = next(bytes.len());
                    bytes[at] ^= 1 << next(8);
                }
            }
            1 => bytes.truncate(next(bytes.len())),
            _ => {
                let at = next(bytes.len());
                let tail = bytes.split_off(at);
                bytes.extend((0..next(64)).map(|i| (i * 37) as u8));
                bytes.extend(tail);
            }
        }
        fs::write(&broken, &bytes).unwrap();
        let _ = eh3dfile::list(&broken);
        let _ = eh3dfile::open(&broken, None);
        let _ = eh3dfile::take(&broken, "cad", "outline.dxf", &motion());
        let _ = eh3dfile::save(&broken, &program(), &Identity::default());
    }
    fs::write(&broken, b"this is not a project").unwrap();
    assert!(eh3dfile::list(&broken).is_err());
}

#[test]
fn the_command_saves_lists_opens_and_extracts() {
    let dir = folder("cli");
    let file = text(&dir.join("job.eh3d"));
    let nc = example("01_square.nc");
    let (code, _, err) = cli(&["save", &file, "--program", &nc, "--name", "N"]);
    assert_eq!(code, 0, "{err}");
    let (code, out, err) = cli(&["list", &file]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("cnc") && out.contains("1 branch(es)"), "{out}");
    let (code, out, _) = cli(&["list", &file, "--json"]);
    assert_eq!(code, 0);
    let document: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(document["branches"][0]["id"], "cnc");

    let into = text(&dir.join("into"));
    let (code, out, err) = cli(&["open", &file, "--into", &into]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("programs/01_square.nc"), "{out}");
    // what is there is never written over
    assert_eq!(cli(&["extract", &file, "cnc", &into]).0, 2);
    let other = text(&dir.join("other"));
    assert_eq!(cli(&["extract", &file, "cnc", &other]).0, 0);
    assert_eq!(cli(&["--help"]).0, 0);
}

#[test]
fn a_wrong_request_creates_nothing() {
    let dir = folder("wrong");
    let file = text(&dir.join("a.eh3d"));
    let other = text(&dir.join("a.nc"));
    let junk = text(&dir.join("junk.dxf"));
    fs::write(&junk, "garbage").unwrap();
    let nc = example("01_square.nc");
    for args in [
        vec![],
        vec!["melt", &file],
        vec!["save", &file],
        vec!["save", &other, "--program", &nc],
        vec!["save", &file, "--drawing", &junk],
        vec!["save", &file, "--program", &nc, "--oops"],
        vec!["import", &file, &junk],
        vec!["open", &file],
        vec!["take", &file, "cad", "outline.dxf"],
        vec!["list"],
    ] {
        assert_eq!(cli(&args).0, 2, "{args:?}");
        assert!(!Path::new(&file).exists(), "{args:?}");
        assert!(!Path::new(&other).exists(), "{args:?}");
    }
}
