//! The command `phoenix-cnc project ...`: the words after `project`, apart from the window so that it can be tried without one.

use std::path::{Path, PathBuf};

use crate::cli_tools::Outcome;
use crate::eh3dfile::{self, Identity, Inputs};

/// How `project` is written.
pub const USAGE: &str = "usage:\n  phoenix-cnc project list FILE.eh3d [--json]\n  phoenix-cnc project save FILE.eh3d [--program P.nc] [--drawing D] [--tools T] [--motion M] [--name NAME] [--id ID]\n  phoenix-cnc project import FILE.eh3d PROGRAM.nc|DRAWING [--motion M] [--name NAME] [--id ID]\n  phoenix-cnc project open FILE.eh3d [--into DIR]\n  phoenix-cnc project take FILE.eh3d BRANCH NAME [--motion M]\n  phoenix-cnc project extract FILE.eh3d BRANCH DIR\n";

fn flag(args: &mut Vec<String>, name: &str) -> Result<Option<String>, String> {
    let Some(i) = args.iter().position(|a| a == name) else {
        return Ok(None);
    };
    let value = args
        .get(i + 1)
        .ok_or_else(|| format!("{name} needs a value"))?
        .clone();
    args.drain(i..=i + 1);
    Ok(Some(value))
}

fn words<const N: usize>(args: &[String], what: &str) -> Result<[String; N], String> {
    if let Some(option) = args.iter().find(|a| a.starts_with("--")) {
        return Err(format!("{option}: unknown option"));
    }
    <[String; N]>::try_from(args.to_vec()).map_err(|_| format!("project {what}"))
}

fn identity(args: &mut Vec<String>) -> Result<Identity, String> {
    Ok(Identity {
        name: flag(args, "--name")?,
        id: flag(args, "--id")?,
    })
}

fn notes(out: &mut Outcome, list: &[String]) {
    for note in list {
        out.err.push_str(&format!("note: {note}\n"));
    }
}

fn job(mut args: Vec<String>) -> Result<Outcome, String> {
    if args.is_empty() {
        return Err("project needs list, save, import, open, take or extract".into());
    }
    let what = args.remove(0);
    let mut o = Outcome {
        out: String::new(),
        err: String::new(),
        code: 0,
    };
    match what.as_str() {
        "list" => {
            let json = match args.iter().position(|a| a == "--json") {
                Some(i) => {
                    args.remove(i);
                    true
                }
                None => false,
            };
            let [file] = words::<1>(&args, "list needs FILE.eh3d")?;
            let listing = eh3dfile::list(Path::new(&file))?;
            if json {
                let rows: Vec<serde_json::Value> = listing
                    .rows
                    .iter()
                    .map(|r| {
                        serde_json::json!({
                            "id": r.id, "app": r.app, "format": r.format, "app_version": r.app_version,
                            "files": r.files, "hash": r.hash, "notes": r.notes,
                        })
                    })
                    .collect();
                let document = serde_json::json!({
                    "name": listing.name, "id": listing.id, "branches": rows, "warnings": listing.warnings,
                });
                o.out = format!("{document:#}\n");
            } else {
                o.out
                    .push_str(&format!("project {} ({})\n", listing.name, listing.id));
                for r in &listing.rows {
                    o.out.push_str(&format!(
                        "  {:<16} {:>4} files  format {:<6} {} {}  {}\n",
                        r.id,
                        r.files,
                        r.format,
                        r.app,
                        r.app_version,
                        r.notes.join(", ")
                    ));
                }
                o.out
                    .push_str(&format!("{} branch(es)\n", listing.rows.len()));
                notes(&mut o, &listing.warnings);
            }
            o.code = i32::from(!listing.warnings.is_empty());
        }
        "save" => {
            let inputs = Inputs {
                program: flag(&mut args, "--program")?,
                drawing: flag(&mut args, "--drawing")?,
                tools: flag(&mut args, "--tools")?,
                motion: flag(&mut args, "--motion")?,
            };
            let who = identity(&mut args)?;
            let [file] = words::<1>(&args, "save needs FILE.eh3d")?;
            let said = eh3dfile::save(Path::new(&file), &inputs, &who)?;
            notes(&mut o, &said);
            o.out = format!("the branch cnc of {file} was saved\n");
        }
        "import" => {
            let motion = eh3dfile::motion_of(flag(&mut args, "--motion")?.as_deref())?;
            let who = identity(&mut args)?;
            let [file, source] = words::<2>(&args, "import needs FILE.eh3d and a file")?;
            let (report, said) =
                eh3dfile::import_file(Path::new(&file), Path::new(&source), &who, &motion)?;
            for c in &report.read {
                o.out.push_str(&format!("read: {} {}\n", c.count, c.item));
            }
            for c in &report.not_read {
                o.out.push_str(&format!(
                    "not read: {} {} ({})\n",
                    c.count,
                    c.item,
                    c.reason.as_deref().unwrap_or("-")
                ));
            }
            for c in &report.export_losses {
                o.out.push_str(&format!(
                    "lost on export: {}\n",
                    c.reason.as_deref().unwrap_or(&c.item)
                ));
            }
            notes(&mut o, &said);
            o.out.push_str(&format!(
                "{} was imported into {file}; the original is kept\n",
                report.source.name
            ));
            o.code = i32::from(!report.not_read.is_empty());
        }
        "open" => {
            let into = flag(&mut args, "--into")?.map(PathBuf::from);
            let [file] = words::<1>(&args, "open needs FILE.eh3d")?;
            let opened = eh3dfile::open(Path::new(&file), into.as_deref())?;
            match (&opened.program, opened.moves) {
                (Some(name), Some((lines, moves))) => o
                    .out
                    .push_str(&format!("program {name}: {lines} lines, {moves} moves\n")),
                _ => o.out.push_str("program none\n"),
            }
            for (what, name) in [
                ("drawing", &opened.drawing),
                ("tools", &opened.tools),
                ("motion", &opened.motion),
            ] {
                o.out
                    .push_str(&format!("{what} {}\n", name.as_deref().unwrap_or("none")));
            }
            if let Some(report) = &opened.report {
                o.out.push_str(&format!(
                    "imported {} ({}), {} item(s) not read\n",
                    report.source.name,
                    report.source.format,
                    report.not_read.len()
                ));
            }
            if let (Some(count), Some(dir)) = (opened.written, &into) {
                o.out
                    .push_str(&format!("{count} files written to {}\n", dir.display()));
            }
            notes(&mut o, &opened.warnings);
            o.code = i32::from(!opened.warnings.is_empty());
        }
        "take" => {
            let motion = eh3dfile::motion_of(flag(&mut args, "--motion")?.as_deref())?;
            let [file, branch, name] =
                words::<3>(&args, "take needs FILE.eh3d, a branch and a file of it")?;
            let (target, said) = eh3dfile::take(Path::new(&file), &branch, &name, &motion)?;
            notes(&mut o, &said);
            o.out = format!("{branch}/{name} was taken into cnc/{target}\n");
        }
        "extract" => {
            let [file, branch, dir] =
                words::<3>(&args, "extract needs FILE.eh3d, a branch and a folder")?;
            let count = eh3dfile::extract(Path::new(&file), &branch, Path::new(&dir))?;
            o.out = format!("{count} files written to {dir}\n");
        }
        other => {
            return Err(format!(
                "project {other}: it is list, save, import, open, take or extract"
            ))
        }
    }
    Ok(o)
}

/// Runs the words after `project`: what goes to the standard output and error, and the exit code (0 clean, 1 something to read, 2 error).
pub fn run(args: &[String]) -> Outcome {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        return Outcome {
            out: USAGE.to_string(),
            err: String::new(),
            code: 0,
        };
    }
    job(args.to_vec()).unwrap_or_else(|why| Outcome {
        out: String::new(),
        err: format!("phoenix-cnc: {why}\n"),
        code: 2,
    })
}
