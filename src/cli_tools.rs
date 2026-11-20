//! The tools of the command line that read a program and print a text, with no window: `--convert` (millimetres and inches), `--times` (the time by kind
//! of move), `--offsets` (the `H` and the `D` against the table of tools), `--format` (the format a controller takes, one of the three of the program or a
//! profile file of the user), `--compare` (two programs), `--holder` (the tool holder against the material), `--life` (the wear of the tools pass by pass),
//! `--restart` (the program as it restarts from a line) and `--template` / `--templates` (the settings of a kind of machine). The work is in `units`,
//! `timesplit`, `offsets`, `postcheck`, `compare`, `holder`, `life`, `restart` and `templates`; this module reads the files, picks the language and the
//! settings, and makes the text and the exit code (0 clean, 1 something to read, 2 error), so the binary only prints what it is given. With `--out` what
//! a tool prints goes to a new file instead, and never over one that is there. Nothing is sent to a machine.

use std::io::Write;
use std::path::Path;

use crate::cli::Options;
use crate::progcheck::{Finding, Level};
use crate::settings::Settings;
use crate::sim::Rates;
use crate::words::{say, Msg};

/// The most a program may be for these tools, in bytes.
pub const MAX_BYTES: u64 = 64 * 1024 * 1024;

/// What a tool prints and how it ends.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Outcome {
    /// What goes to the standard output.
    pub out: String,
    /// What goes to the standard error.
    pub err: String,
    /// 0 clean, 1 something to read, 2 error.
    pub code: i32,
}

impl Outcome {
    fn failed(why: String) -> Outcome {
        Outcome {
            out: String::new(),
            err: format!("phoenix-cnc: {why}\n"),
            code: 2,
        }
    }
}

/// Whether the options ask for one of these tools.
pub fn wanted(o: &Options) -> bool {
    o.convert.is_some()
        || o.compare.is_some()
        || o.times
        || o.offsets
        || o.format.is_some()
        || o.holder
        || o.life
        || o.restart.is_some()
        || o.lookahead
        || o.import
        || o.post.is_some()
        || o.template.is_some()
        || o.templates
}

/// The lines of a program file: every byte is a character, so a file in any code page is read (as the report reads it).
pub fn read_lines(path: &Path) -> Result<Vec<String>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(format!("{} is too big", path.display()));
    }
    let text: String = bytes.iter().map(|&b| b as char).collect();
    Ok(text.lines().map(str::to_string).collect())
}

fn texts(findings: &[Finding], lang: &str) -> String {
    findings
        .iter()
        .map(|f| format!("{}\n", f.text(lang)))
        .collect()
}

fn to_read(findings: &[Finding]) -> bool {
    findings.iter().any(|f| f.level >= Level::Warning)
}

fn rates(s: &Settings) -> Rates {
    Rates {
        rapid: s.rapid_mm_min,
        default_feed: s.feed_mm_min,
    }
}

/// `--convert`: the program in the other unit on the standard output, what it has to say on the standard error.
pub fn convert(lines: &[String], to_inches: bool, lang: &str) -> Outcome {
    let c = crate::units::convert(lines, to_inches);
    let unit = if to_inches { "G20 (in)" } else { "G21 (mm)" };
    let mut err = texts(&c.findings, lang);
    err.push_str(&say(
        lang,
        Msg::UnitsSummary,
        &[
            c.numbers.to_string(),
            c.changed_lines.to_string(),
            unit.to_string(),
        ],
    ));
    err.push('\n');
    let mut out = c.lines.join("\n");
    out.push('\n');
    Outcome {
        out,
        err,
        code: i32::from(to_read(&c.findings)),
    }
}

/// `--times`: the time of the program by kind of move.
pub fn times(lines: &[String], s: &Settings, lang: &str) -> Outcome {
    let change_s = s.tuning.change_seconds(s.rapid_mm_min);
    let split = crate::timesplit::split(lines, rates(s), change_s, s.tuning.accel_mm_s2);
    let mut out = split.lines(lang).join("\n");
    out.push('\n');
    Outcome {
        out,
        err: String::new(),
        code: 0,
    }
}

/// `--offsets`: the `H` and `D` words and the length offsets against the table of tools of the settings.
pub fn offsets(lines: &[String], s: &Settings, lang: &str) -> Outcome {
    let found = crate::offsets::check(lines, &s.tuning.tools);
    Outcome {
        out: texts(&found, lang),
        err: String::new(),
        code: i32::from(to_read(&found)),
    }
}

/// `--format`: the format of the program for a family of controllers.
pub fn format_check(lines: &[String], profile: &str, lang: &str) -> Outcome {
    format_check_in(lines, profile, lang, None)
}

/// As `format_check`, where the profile may also be a file of the user: found by its name next to the settings file `settings`, or by its path.
pub fn format_check_in(
    lines: &[String],
    profile: &str,
    lang: &str,
    settings: Option<&Path>,
) -> Outcome {
    use crate::postcheck::{resolve, Found};
    match resolve(profile, settings) {
        Found::Profile(p) => {
            let found = crate::postcheck::check(lines, &p);
            Outcome {
                out: texts(&found, lang),
                err: String::new(),
                code: i32::from(to_read(&found)),
            }
        }
        Found::NotFound => Outcome::failed(say(lang, Msg::FmtUnknownProfile, &[profile.trim()])),
        Found::Invalid(findings) => Outcome {
            out: String::new(),
            err: texts(&findings, lang),
            code: 2,
        },
    }
}

/// `--holder`: the tool holder against the material.
pub fn holder(lines: &[String], s: &Settings, lang: &str) -> Outcome {
    let found = crate::holder::check(lines, &s.tuning.tools);
    Outcome {
        out: texts(&found, lang),
        err: String::new(),
        code: i32::from(to_read(&found)),
    }
}

/// `--life`: the wear of the tools pass by pass. The material of the settings sets how fast it uses a tool up (the wood of the table when it is not known).
pub fn life(lines: &[String], s: &Settings, lang: &str) -> Outcome {
    let factor = crate::consumption::factor(&s.material).unwrap_or(1.0);
    let wear = crate::life::passes(
        lines,
        &s.tuning.tools,
        rates(s),
        s.tuning.accel_mm_s2,
        factor,
    );
    let mut out = crate::life::lines(&wear, lang).join("\n");
    out.push('\n');
    Outcome {
        out,
        err: String::new(),
        code: i32::from(!wear.worn_out.is_empty()),
    }
}

/// `--restart`: the program as it restarts from a line, with the state before it rebuilt; what was read goes to the other stream.
pub fn restart(lines: &[String], line: usize, lang: &str) -> Outcome {
    match crate::restart::plan(lines, line) {
        Ok(plan) => {
            let mut out = plan.program.join("\n");
            out.push('\n');
            let mut err = crate::restart::remarks(&plan, lang).join("\n");
            err.push('\n');
            Outcome { out, err, code: 0 }
        }
        Err(why) => Outcome::failed(why.text(lang)),
    }
}

/// The lines of the problems of a text file of the motion layer, in the language, with the name of the file in front.
fn problems_text(file: &str, problems: &[crate::problem::Problem], lang: &str) -> String {
    problems
        .iter()
        .map(|p| format!("{file}: {}\n", p.text(lang)))
        .collect()
}

/// `--lookahead`: the time of the program with the look-ahead of the planner, next to the time of the same moves stopping at each one.
pub fn lookahead(
    lines: &[String],
    s: &Settings,
    cfg: &crate::motioncfg::MotionCfg,
    lang: &str,
) -> Outcome {
    use crate::notes::Label;
    let change_s = s.tuning.change_seconds(s.rapid_mm_min);
    let mut planner = cfg.planner.clone();
    if s.tuning.accel_mm_s2 > 0.0
        && planner.accel_mm_s2 == crate::motioncfg::PlannerCfg::default().accel_mm_s2
    {
        // the acceleration of the settings is used for the three axes when the motion file says nothing of its own
        planner.accel_mm_s2 = [s.tuning.accel_mm_s2; 3];
    }
    let p = crate::lookahead::plan_lines(lines, rates(s), &planner, change_s);
    let line = |label: Label, v: String| format!("{}: {v}\n", label.text(lang));
    let mut out = String::new();
    out.push_str(&line(
        Label::Time,
        format!("{} ({:.1} s)", crate::sim::hms(p.total_s), p.total_s),
    ));
    out.push_str(&line(
        Label::TimeStopToStop,
        format!(
            "{} ({:.1} s)",
            crate::sim::hms(p.stop_to_stop_s),
            p.stop_to_stop_s
        ),
    ));
    out.push_str(&line(Label::Blocks, p.blocks.len().to_string()));
    out.push_str(&line(Label::Stops, p.stops.to_string()));
    out.push_str(&line(Label::Length, format!("{:.1}", p.length_mm)));
    if planner.arcs {
        out.push_str(&line(Label::ArcsRebuilt, p.arcs_rebuilt.to_string()));
    }
    Outcome {
        out,
        err: String::new(),
        code: 0,
    }
}

/// The post-processor the options name: a file given by its path, or one beside the settings by its name. `None` for no `--post`; an error
/// text when it is not there.
fn post_of(
    o: &Options,
    settings_file: Option<&Path>,
    lang: &str,
) -> Result<Option<(crate::post::Post, String)>, Outcome> {
    let Some(name) = o.post.as_deref() else {
        return Ok(None);
    };
    let direct = Path::new(name);
    let path = if direct.is_file() {
        Some(direct.to_path_buf())
    } else {
        settings_file.and_then(|f| crate::post::post_path(f, name))
    };
    match path.as_deref().and_then(crate::post::read) {
        Some((post, problems)) => Ok(Some((post, problems_text(name, &problems, lang)))),
        None => Err(Outcome::failed(
            crate::notes::Label::NoPost.text(lang).to_string(),
        )),
    }
}

/// What the post-processor does to a program: the lines it gives and what is to be said (its problems and a script that is not run).
fn post_apply(
    post: &(crate::post::Post, String),
    cfg: &crate::motioncfg::MotionCfg,
    lines: &[String],
    lang: &str,
) -> (Vec<String>, String) {
    let mut said = post.1.clone();
    if let Err(label) = post.0.script_state(cfg.import.post_scripts) {
        said.push_str(&format!("{}\n", label.text(lang)));
    }
    (post.0.apply(lines), said)
}

fn joined(lines: &[String]) -> String {
    let mut s = lines.join("\n");
    s.push('\n');
    s
}

/// `--import`: a drawing as the program that cuts along it, moved to the zero of the work; what the readers did not understand is said on the
/// standard error and makes the exit code 1.
pub fn import(
    path: &Path,
    cfg: &crate::motioncfg::MotionCfg,
    post: Option<&(crate::post::Post, String)>,
    lang: &str,
) -> Outcome {
    use crate::notes::Label;
    let mut drawing = match crate::import::read_file(path, &cfg.import) {
        Ok(d) => d,
        Err(why) => return Outcome::failed(why.text(lang).to_string()),
    };
    crate::import::to_origin(&mut drawing);
    let mut lines = crate::import::to_gcode(&drawing, &cfg.import);
    let mut err = String::new();
    for n in &drawing.notes {
        err.push_str(&format!("{}\n", n.text(lang)));
    }
    if let Some(p) = post {
        let (l, said) = post_apply(p, cfg, &lines, lang);
        lines = l;
        err.push_str(&said);
    }
    err.push_str(&format!(
        "{}: {}\n{}: {}\n",
        Label::Paths.text(lang),
        drawing.paths.len(),
        Label::Lines.text(lang),
        lines.len()
    ));
    Outcome {
        out: joined(&lines),
        err,
        code: i32::from(!drawing.notes.is_empty()),
    }
}

/// `--post` on a program: the program as the post-processor writes it.
pub fn post_program(
    post: &(crate::post::Post, String),
    cfg: &crate::motioncfg::MotionCfg,
    lines: &[String],
    lang: &str,
) -> Outcome {
    let (out, said) = post_apply(post, cfg, lines, lang);
    Outcome {
        out: joined(&out),
        err: said,
        code: 0,
    }
}

/// `--template` and `--templates`: the settings of a kind of machine, or the list of them.
pub fn template(name: Option<&str>, lang: &str) -> Outcome {
    match name {
        None => {
            let mut out = crate::templates::list(lang).join("\n");
            out.push('\n');
            Outcome {
                out,
                err: String::new(),
                code: 0,
            }
        }
        Some(n) => match crate::templates::find(n) {
            Some(t) => Outcome {
                out: t.settings_text(),
                err: String::new(),
                code: 0,
            },
            None => Outcome::failed(crate::templates::unknown(n, lang)),
        },
    }
}

/// What the tool printed, written to the file `--out` names instead of the standard output. The file is made new: one that is there is never written over
/// (not even the program that was read), and the tool ends with 2 and says so.
pub fn deliver(o: &Options, r: Outcome, lang: &str) -> Outcome {
    let Some(path) = o.out.as_ref() else {
        return r;
    };
    let shown = path.display().to_string();
    let wrote = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .and_then(|mut f| f.write_all(r.out.as_bytes()));
    let (said, code) = match wrote {
        Ok(()) => (say(lang, Msg::OutWritten, &[shown]), r.code),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            (say(lang, Msg::OutExists, &[shown]), 2)
        }
        Err(e) => (say(lang, Msg::OutError, &[shown, e.to_string()]), 2),
    };
    Outcome {
        out: String::new(),
        err: format!("{}{said}\n", r.err),
        code,
    }
}

/// `--compare`: where two programs differ.
pub fn compare(
    a: &[String],
    b: &[String],
    names: (&str, &str),
    s: &Settings,
    lang: &str,
) -> Outcome {
    let change_s = s.tuning.change_seconds(s.rapid_mm_min);
    let accel = s.tuning.accel_mm_s2;
    let fa = crate::compare::facts(a, rates(s), change_s, accel, &s.tuning);
    let fb = crate::compare::facts(b, rates(s), change_s, accel, &s.tuning);
    let (text, differ) = crate::compare::compare(&fa, &fb, names, lang);
    let mut out = text.join("\n");
    out.push('\n');
    Outcome {
        out,
        err: String::new(),
        code: i32::from(differ > 0),
    }
}

/// Runs the tool the options ask for with these settings (they are read from the settings file by `run`).
pub fn run_in(o: &Options, s: &Settings) -> Outcome {
    run_with(o, s, None)
}

/// As `run_in`, knowing where the settings file is: the profiles of the user named in `--format` are looked for next to it.
pub fn run_with(o: &Options, s: &Settings, settings_file: Option<&Path>) -> Outcome {
    let lang = o.lang.unwrap_or(s.lang).code();
    let r = run_tool(o, s, settings_file, lang);
    deliver(o, r, lang)
}

fn run_tool(o: &Options, s: &Settings, settings_file: Option<&Path>, lang: &str) -> Outcome {
    if o.templates {
        return template(None, lang);
    }
    if let Some(name) = o.template.as_deref() {
        return template(Some(name), lang);
    }
    let Some(path) = o.file.as_ref() else {
        return Outcome::failed("this needs a G-code file".to_string());
    };
    let (cfg, cfg_problems) = settings_file
        .map(crate::motioncfg::motion_path)
        .map(|p| crate::motioncfg::MotionCfg::read(&p))
        .unwrap_or_default();
    let mut r = run_on_file(o, s, settings_file, lang, path, &cfg);
    if !cfg_problems.is_empty() {
        let name = settings_file.map_or_else(String::new, |f| {
            crate::motioncfg::motion_path(f)
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().to_string())
        });
        r.err = format!("{}{}", problems_text(&name, &cfg_problems, lang), r.err);
    }
    r
}

fn run_on_file(
    o: &Options,
    s: &Settings,
    settings_file: Option<&Path>,
    lang: &str,
    path: &Path,
    cfg: &crate::motioncfg::MotionCfg,
) -> Outcome {
    let post = match post_of(o, settings_file, lang) {
        Ok(p) => p,
        Err(e) => return e,
    };
    if o.import {
        return import(path, cfg, post.as_ref(), lang);
    }
    let lines = match read_lines(path) {
        Ok(l) => l,
        Err(why) => return Outcome::failed(why),
    };
    if let Some(unit) = o.convert.as_deref() {
        return convert(&lines, unit == "in", lang);
    }
    if let Some(second) = o.compare.as_ref() {
        let other = match read_lines(second) {
            Ok(l) => l,
            Err(why) => return Outcome::failed(why),
        };
        let name = |p: &Path| {
            p.file_name().map_or_else(
                || p.display().to_string(),
                |n| n.to_string_lossy().to_string(),
            )
        };
        return compare(&lines, &other, (&name(path), &name(second)), s, lang);
    }
    if o.times {
        return times(&lines, s, lang);
    }
    if o.offsets {
        return offsets(&lines, s, lang);
    }
    if o.holder {
        return holder(&lines, s, lang);
    }
    if o.life {
        return life(&lines, s, lang);
    }
    if let Some(line) = o.restart {
        return restart(&lines, line, lang);
    }
    if o.lookahead {
        return lookahead(&lines, s, cfg, lang);
    }
    if let Some(p) = post.as_ref() {
        return post_program(p, cfg, &lines, lang);
    }
    match o.format.as_deref() {
        Some(profile) => format_check_in(&lines, profile, lang, settings_file),
        None => Outcome::failed("nothing to do".to_string()),
    }
}

/// Runs the tool the options ask for with the settings of the settings file, prints what it makes and gives the exit code.
pub fn run(o: &Options) -> i32 {
    let file = std::fs::read_to_string(crate::settings::settings_path()).unwrap_or_default();
    let mut s = Settings::parse(&file);
    if let Some(name) = o.profile.as_ref() {
        if !s.use_profile(name) {
            eprintln!("phoenix-cnc: no machine profile is named '{name}'");
            return 2;
        }
    }
    s.resolve_tool_table(crate::tools::load(&crate::settings::settings_path()));
    let r = run_with(o, &s, Some(&crate::settings::settings_path()));
    print!("{}", r.out);
    eprint!("{}", r.err);
    r.code
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(str::to_string).collect()
    }

    fn options(args: &[&str]) -> Options {
        Options::parse(args.iter().map(|a| a.to_string())).unwrap()
    }

    /// A file of its own in the temporary folder, removed when the test is over.
    struct Temp(PathBuf);

    impl Temp {
        fn new(name: &str, text: &str) -> Temp {
            let path = std::env::temp_dir().join(format!("cnc11_{}_{name}", std::process::id()));
            std::fs::write(&path, text).unwrap();
            Temp(path)
        }

        fn arg(&self) -> String {
            self.0.to_string_lossy().to_string()
        }
    }

    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    #[test]
    fn convert_prints_the_program_and_says_what_it_did_on_the_other_stream() {
        let r = convert(&lines("G21\nG1 X25.4 F254"), true, "en");
        assert_eq!(r.out, "G20\nG1 X1 F10\n");
        assert_eq!(r.err, "2 numbers in 2 lines were converted to G20 (in)\n");
        assert_eq!(r.code, 0);
        // a unit change halfway is a warning and so a 1
        let w = convert(&lines("G21\nG1 X1\nG20\nG1 X1"), false, "es");
        assert_eq!(w.code, 1);
        assert!(w.err.contains("Línea 3: G20 cambia la unidad"), "{}", w.err);
    }

    #[test]
    fn times_prints_the_split_and_never_fails() {
        let s = Settings::default();
        let r = times(&lines("G21 G90\nG0 X30\nG1 X90 F600"), &s, "en");
        assert_eq!(r.code, 0);
        assert!(r.out.starts_with("Time by kind of move\n"));
        assert!(r.out.contains("Rapid moves:") && r.out.contains("Cutting:"));
        assert!(r.out.lines().last().unwrap().starts_with("Total: "));
        assert!(r.err.is_empty());
    }

    #[test]
    fn offsets_reads_the_table_of_the_settings() {
        let mut s = Settings::default();
        let program = lines("T2 M6\nG43 H7\nG1 Z-1 F100");
        let r = offsets(&program, &s, "en");
        assert_eq!(r.code, 1);
        assert!(r.out.contains("Line 2: G43 asks for H7"), "{}", r.out);
        // a program that asks for what the table has is quiet
        s.tuning.tools =
            crate::tools::ToolTable::parse("tool = 2 | diameter=3\ntool = 7 | diameter=3\n");
        let quiet = offsets(&lines("T7 M6\nG43 H7\nG1 Z-1 F100"), &s, "en");
        assert_eq!((quiet.out.as_str(), quiet.code), ("", 0));
    }

    #[test]
    fn format_check_names_the_profile_that_does_not_exist_and_reads_the_others() {
        let r = format_check(&lines("G90\nG41 G1 X5 F100\nM30"), "grbl", "en");
        assert_eq!(r.code, 1);
        assert!(
            r.out.contains("Line 2: G41 is not in the list of grbl"),
            "{}",
            r.out
        );
        let ok = format_check(&lines("G90\nG1 X5 F100\nM30"), "GRBL", "en");
        assert_eq!((ok.out.as_str(), ok.code), ("", 0));
        let bad = format_check(&lines("G90"), "nosuchprofile", "en");
        assert_eq!(bad.code, 2);
        assert!(
            bad.err
                .contains("No format profile is named 'nosuchprofile'"),
            "{}",
            bad.err
        );
        // only information is not something to read
        let info = format_check(&lines("G90\nG1 X5 F100"), "grbl", "en");
        assert!(info.out.contains("does not end with M2 or M30"));
        assert_eq!(info.code, 0);
    }

    #[test]
    fn compare_exits_1_when_the_programs_differ_and_0_when_they_do_not() {
        let s = Settings::default();
        let a = lines("G21 G90\nG1 X10 F600");
        let same = compare(&a, &a, ("a.nc", "b.nc"), &s, "en");
        assert_eq!(same.code, 0);
        assert!(same.out.starts_with("Comparison of a.nc and b.nc\n"));
        let other = compare(
            &a,
            &lines("G21 G90\nG1 X99 F600"),
            ("a.nc", "b.nc"),
            &s,
            "en",
        );
        assert_eq!(other.code, 1);
        assert!(
            other.out.contains("Cutting length: 10.0 mm -> 99.0 mm"),
            "{}",
            other.out
        );
    }

    #[test]
    fn the_options_pick_the_tool_and_the_files_are_read() {
        let a = Temp::new("a.nc", "G21\nG1 X25.4 F254\n");
        let b = Temp::new("b.nc", "G21\nG1 X50.8 F254\n");
        let s = Settings::default();
        let o = options(&["--convert", "in", &a.arg()]);
        assert!(wanted(&o));
        let r = run_in(&o, &s);
        assert_eq!((r.out.as_str(), r.code), ("G20\nG1 X1 F10\n", 0));
        let o = options(&["--times", &a.arg()]);
        assert!(run_in(&o, &s).out.contains("Cutting"));
        let o = options(&["--compare", &b.arg(), &a.arg()]);
        let r = run_in(&o, &s);
        assert_eq!(r.code, 1);
        assert!(r.out.contains("25.4 mm -> 50.8 mm"), "{}", r.out);
        let o = options(&["--format", "full", &a.arg()]);
        assert_eq!(run_in(&o, &s).code, 0);
        // a file that is not there is an error and says which
        let o = options(&["--offsets", "cnc11_no_such_file.nc"]);
        let r = run_in(&o, &s);
        assert_eq!(r.code, 2);
        assert!(r.err.contains("cnc11_no_such_file.nc"), "{}", r.err);
        // no tool asked for
        assert!(!wanted(&options(&[&a.arg()])));
    }

    #[test]
    fn a_file_in_a_code_page_is_read_byte_by_byte() {
        let t = Temp::new("cp.nc", "G21\n(caf\u{e9})\nG1 X1\n");
        let bytes = std::fs::read(&t.0).unwrap();
        // the file holds UTF-8; each of its bytes becomes a character
        let l = read_lines(&t.0).unwrap();
        assert_eq!(l.len(), 3);
        assert_eq!(
            l[1].chars().count(),
            bytes.split(|&b| b == b'\n').nth(1).unwrap().len()
        );
    }

    #[test]
    fn a_missing_file_option_or_name_is_an_error_not_a_panic() {
        let s = Settings::default();
        let o = Options {
            times: true,
            ..Options::default()
        };
        assert_eq!(run_in(&o, &s).code, 2);
        let nothing = Options {
            file: Some(PathBuf::from("cnc11_none.nc")),
            ..Options::default()
        };
        assert_eq!(run_in(&nothing, &s).code, 2);
    }

    /// A path in the temporary folder that nothing is at, removed when the test is over whatever was written there.
    struct Spot(PathBuf);

    impl Spot {
        fn new(name: &str) -> Spot {
            let path = std::env::temp_dir()
                .join(format!("phoenix-cnc-tools-{}-{name}", std::process::id()));
            let _ = std::fs::remove_file(&path);
            Spot(path)
        }

        fn arg(&self) -> String {
            self.0.to_string_lossy().to_string()
        }
    }

    impl Drop for Spot {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    #[test]
    fn what_a_tool_prints_goes_to_a_new_file_and_never_over_one_that_is_there() {
        let input = Temp::new("out-in.nc", "G21\nG1 X25.4 F254\n");
        let out = Spot::new("out.nc");
        let s = Settings::default();
        let o = options(&["--convert", "in", "--out", &out.arg(), &input.arg()]);
        let r = run_in(&o, &s);
        assert_eq!((r.out.as_str(), r.code), ("", 0));
        assert!(
            r.err.contains("2 numbers in 2 lines were converted"),
            "{}",
            r.err
        );
        assert!(
            r.err.ends_with(&format!("{} was written\n", out.arg())),
            "{}",
            r.err
        );
        assert_eq!(std::fs::read_to_string(&out.0).unwrap(), "G20\nG1 X1 F10\n");
        // the same file again is refused and keeps what it had
        let again = run_in(&o, &s);
        assert_eq!(again.code, 2);
        assert!(
            again
                .err
                .contains("is there already and is not written over"),
            "{}",
            again.err
        );
        assert_eq!(std::fs::read_to_string(&out.0).unwrap(), "G20\nG1 X1 F10\n");
        // the program that was read is not written over either
        let same = options(&["--convert", "in", "--out", &input.arg(), &input.arg()]);
        assert_eq!(run_in(&same, &s).code, 2);
        assert_eq!(
            std::fs::read_to_string(&input.0).unwrap(),
            "G21\nG1 X25.4 F254\n"
        );
        // the refusal is told in the language asked for
        let es = run_in(
            &options(&[
                "--convert",
                "in",
                "--out",
                &out.arg(),
                "--lang",
                "es",
                &input.arg(),
            ]),
            &s,
        );
        assert!(
            es.err.contains("ya existe y no se sobrescribe"),
            "{}",
            es.err
        );
        // a place that cannot be written is an error that says so
        let nowhere = std::env::temp_dir()
            .join("phoenix-cnc-no-such-folder")
            .join("x.nc");
        let bad = run_in(
            &options(&["--times", "--out", &nowhere.to_string_lossy(), &input.arg()]),
            &s,
        );
        assert_eq!(bad.code, 2);
        assert!(bad.err.contains("could not be written"), "{}", bad.err);
    }

    #[test]
    fn the_exit_code_of_the_tool_is_kept_when_its_text_goes_to_a_file() {
        let input = Temp::new("out-code.nc", "G21\nG1 X1\nG20\nG1 X1\n");
        let out = Spot::new("out-code.txt");
        let o = options(&["--convert", "mm", "--out", &out.arg(), &input.arg()]);
        assert_eq!(run_in(&o, &Settings::default()).code, 1);
        assert!(out.0.is_file());
    }

    #[test]
    fn a_format_profile_of_the_user_is_found_by_its_path_or_by_its_name_beside_the_settings() {
        let program = Temp::new("fmt.nc", "G90\nG41 G1 X5 F100\nM30\n");
        let mine = Temp::new("mine.txt", "base = grbl\nallow_g = 41\n");
        let s = Settings::default();
        // GRBL does not list G41; the profile of the user does
        let grbl = run_in(&options(&["--format", "grbl", &program.arg()]), &s);
        assert_eq!(grbl.code, 1);
        let user = run_in(&options(&["--format", &mine.arg(), &program.arg()]), &s);
        assert_eq!((user.out.as_str(), user.code), ("", 0), "{}", user.err);
        // by name, next to the settings file
        let settings = std::env::temp_dir().join(format!("cnc-fmt-{}.cfg", std::process::id()));
        let beside = crate::postcheck::profile_path(&settings, "shop").unwrap();
        std::fs::write(&beside, "base = grbl\nallow_g = 41\n").unwrap();
        let named = run_with(
            &options(&["--format", "shop", &program.arg()]),
            &s,
            Some(&settings),
        );
        let _ = std::fs::remove_file(&beside);
        assert_eq!((named.out.as_str(), named.code), ("", 0), "{}", named.err);
        // without the settings file the name is not known
        let lost = run_in(&options(&["--format", "shop", &program.arg()]), &s);
        assert_eq!(lost.code, 2);
        assert!(lost.err.contains("no file has that name"), "{}", lost.err);
        // a file with lines that cannot be used is named line by line
        let broken = Temp::new(
            "broken.txt",
            "base = grbl\nflavour = sweet\nmax_line = lots\n",
        );
        let b = run_in(&options(&["--format", &broken.arg(), &program.arg()]), &s);
        assert_eq!(b.code, 2);
        assert!(
            b.err.contains("Line 2 of the profile: 'flavour'"),
            "{}",
            b.err
        );
        assert!(
            b.err
                .contains("Line 3 of the profile: the value of 'max_line'"),
            "{}",
            b.err
        );
    }

    #[test]
    fn the_holder_the_life_the_restart_and_the_templates_run_from_the_options() {
        let program = Temp::new(
            "tools4.nc",
            "T1 M6\nG21 G90\nM3 S9000\nG0 Z5\nG1 Z-6 F600\nG1 X60\nG0 Z5\nG0 X0\nG1 Z-1 F600\nG1 X60\nM30\n",
        );
        let mut s = Settings::default();
        s.tuning.tools =
            crate::tools::ToolTable::parse("tool = 1 | diameter=3 | reach=5 | life=0.15\n");
        // the holder
        let h = run_in(&options(&["--holder", &program.arg()]), &s);
        assert_eq!(h.code, 1);
        assert!(
            h.out
                .contains("Line 5: tool 1 cuts 6.00 mm below the surface"),
            "{}",
            h.out
        );
        // the life: the tool wears out in the second pass
        let l = run_in(&options(&["--life", &program.arg()]), &s);
        assert_eq!(l.code, 1);
        assert!(
            l.out.starts_with("Wear of the tools by pass\n"),
            "{}",
            l.out
        );
        assert!(l.out.contains("Tool 1 is worn out in pass"), "{}", l.out);
        s.tuning.tools = crate::tools::ToolTable::parse("tool = 1 | diameter=3 | life=100\n");
        let calm = run_in(&options(&["--life", &program.arg()]), &s);
        assert_eq!(calm.code, 0);
        // the restart
        let r = run_in(&options(&["--restart", "8", &program.arg()]), &s);
        assert_eq!(r.code, 0);
        assert!(
            r.out
                .starts_with("(restart at line 8 of the program)\nG21\nG90\n"),
            "{}",
            r.out
        );
        assert!(
            r.out
                .contains("M0 (restart: check the machine, then resume)\nG0 X0\n"),
            "{}",
            r.out
        );
        assert!(
            r.err
                .starts_with("Restart at line 8: the 7 lines before it were read"),
            "{}",
            r.err
        );
        let past = run_in(&options(&["--restart", "99", &program.arg()]), &s);
        assert_eq!(past.code, 2);
        assert!(past.err.contains("there is no line 99"), "{}", past.err);
        // the templates need no program
        let t = run_in(&options(&["--template", "router"]), &s);
        assert_eq!(
            (t.code, t.out.as_str()),
            (
                0,
                crate::templates::find("router")
                    .unwrap()
                    .settings_text()
                    .as_str()
            )
        );
        let list = run_in(&options(&["--templates", "--lang", "de"]), &s);
        assert!(list.out.starts_with("Maschinenvorlagen"), "{}", list.out);
        assert_eq!(list.out.lines().count(), 5);
        // and can go to a file
        let out = Spot::new("template.cfg");
        let w = run_in(
            &options(&["--template", "pcb-mill", "--out", &out.arg()]),
            &s,
        );
        assert_eq!(w.code, 0);
        assert!(std::fs::read_to_string(&out.0)
            .unwrap()
            .starts_with("# phoenix-cnc template pcb-mill\n"));
        assert_eq!(
            run_in(
                &options(&["--template", "pcb-mill", "--out", &out.arg()]),
                &s
            )
            .code,
            2
        );
    }

    #[test]
    fn programs_from_a_fixed_seed_go_through_every_tool_without_a_panic_or_a_hole_in_any_language()
    {
        let pieces = [
            "T1 M6",
            "T2 M6",
            "G20",
            "G21",
            "G90",
            "G91",
            "G17",
            "G18",
            "G54",
            "G55",
            "M3 S9000",
            "M5",
            "M8",
            "G0 X5 Y5",
            "G0 Z5",
            "G1 Z-3 F200",
            "G1 X10 Y12 F400",
            "G2 X20 Y0 R12 F300",
            "G81 X3 Y3 Z-4 R1 F100",
            "G80",
            "G41 D1",
            "G40",
            "G43 H1",
            "G92 X1",
            "G4 P1",
            "M30",
            "(a comment)",
            "",
            "G1 X-2.123456789 F80",
            "G83 X1 Y1 Z-9 R1 Q2 F100",
        ];
        let mut seed = 31u64;
        let mut next = move || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 33) as usize
        };
        let mut s = Settings::default();
        s.tuning.tools = crate::tools::ToolTable::parse(
            "tool = 1 | diameter=3 | reach=4 | life=0.2\ntool = 2 | diameter=6 | reach=8 | life=5 | used=4.9\n",
        );
        let mut texts = 0;
        for _ in 0..250 {
            let n = 1 + next() % 25;
            let p: Vec<String> = (0..n)
                .map(|_| pieces[next() % pieces.len()].to_string())
                .collect();
            let line = 1 + next() % p.len();
            for lang in crate::words::LANGS {
                let all = [
                    holder(&p, &s, lang),
                    life(&p, &s, lang),
                    restart(&p, line, lang),
                    format_check(&p, "full", lang),
                    convert(&p, true, lang),
                    template(Some("engraver"), lang),
                ];
                for r in all {
                    assert!((0..=2).contains(&r.code), "{p:?}");
                    assert!(
                        !r.out.contains('{') && !r.err.contains('{'),
                        "{lang} {p:?}\n{}\n{}",
                        r.out,
                        r.err
                    );
                    texts += 1;
                }
            }
        }
        assert_eq!(texts, 250 * 8 * 6);
    }

    #[test]
    fn lookahead_prints_the_two_times_and_the_motion_file_changes_them() {
        let program = Temp::new("la.nc", "G21 G90\nG1 X10 F600\nG1 X20\nG1 X30\nG1 X40\n");
        let s = Settings::default();
        let r = run_in(&options(&["--lookahead", &program.arg()]), &s);
        assert_eq!(r.code, 0, "{}", r.err);
        let lines: Vec<&str> = r.out.lines().collect();
        assert!(lines[0].starts_with("Time with look-ahead: "), "{}", r.out);
        assert!(lines[1].starts_with("Time stopping at every move: "));
        assert!(r.out.contains("Moves: 4") && r.out.contains("Full stops: 1"));
        // in Portuguese of Portugal
        let pt = run_in(
            &options(&["--lookahead", "--lang", "pt", &program.arg()]),
            &s,
        );
        assert!(pt.out.starts_with("Tempo com antecipação: "), "{}", pt.out);
        // a motion file beside the settings: its lines that are wrong are told in the language and the rest is read
        let dir = std::env::temp_dir().join(format!("cnc_clitools_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let settings = dir.join("phoenix-cnc.txt");
        std::fs::write(
            dir.join("phoenix-cnc-motion.txt"),
            "accel_x = 400\nbogus = 1\njog_stale_ms = 5\n",
        )
        .unwrap();
        let es = run_with(
            &options(&["--lookahead", "--lang", "es", &program.arg()]),
            &s,
            Some(&settings),
        );
        assert_eq!(es.code, 0);
        assert!(
            es.err
                .contains("phoenix-cnc-motion.txt: 2: clave desconocida"),
            "{}",
            es.err
        );
        assert!(es.err.contains("3: fuera de rango"), "{}", es.err);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn import_prints_the_program_of_a_drawing_and_says_what_it_did_not_read() {
        let svg = Temp::new(
            "imp.svg",
            r#"<svg width="20mm" height="20mm" viewBox="0 0 20 20"><rect x="5" y="5" width="10" height="10"/><text>x</text></svg>"#,
        );
        let s = Settings::default();
        let r = run_in(&options(&["--import", &svg.arg()]), &s);
        assert_eq!(r.code, 1, "{}", r.err);
        assert!(
            r.out.starts_with("G21 G90 G17\n") && r.out.contains("G1 Z-0.5 F100"),
            "{}",
            r.out
        );
        assert!(r
            .err
            .contains("Some commands or elements of the file are not read."));
        assert!(r.err.contains("Paths read: 1"));
        // the program can be read back by the path of the program
        let lines: Vec<String> = r.out.lines().map(str::to_string).collect();
        assert!(crate::gcode::toolpath(&lines).len() > 4);
        // a file that is not a drawing is an error in the language, and a missing one too
        let junk = Temp::new("imp.txt", "hello");
        let e = run_in(&options(&["--import", "--lang", "de", &junk.arg()]), &s);
        assert_eq!(e.code, 2);
        assert!(
            e.err.contains("Das Format der Datei ist nicht bekannt."),
            "{}",
            e.err
        );
        assert_eq!(
            run_in(&options(&["--import", "cnc_no_such_drawing.svg"]), &s).code,
            2
        );
    }

    #[test]
    fn post_runs_a_post_processor_on_a_program_or_on_an_import() {
        let program = Temp::new("post.nc", "G0 X1\nT1 M6\nG1 X2 F100 (cut)\n");
        let post = Temp::new(
            "post-rules.txt",
            "header = G21\nreplace = G0 -> G00\ndrop = M6\nscript = tidy\n",
        );
        let s = Settings::default();
        let r = run_in(&options(&["--post", &post.arg(), &program.arg()]), &s);
        assert_eq!(r.code, 0, "{}", r.err);
        assert_eq!(r.out, "G21\nG00 X1\nT1\nG1 X2 F100 (cut)\n");
        // the script is not run and the program says so (the switch is off)
        assert!(r.err.contains("Scripts are switched off"), "{}", r.err);
        // with an import
        let svg = Temp::new(
            "post.svg",
            r#"<svg width="10mm" height="10mm" viewBox="0 0 10 10"><line x1="0" y1="0" x2="10" y2="0"/></svg>"#,
        );
        let i = run_in(
            &options(&["--import", "--post", &post.arg(), &svg.arg()]),
            &s,
        );
        assert!(i.out.starts_with("G21\n"), "{}", i.out);
        // a post-processor that is not there
        let none = run_in(&options(&["--post", "nothere", &program.arg()]), &s);
        assert_eq!(none.code, 2);
        assert!(none
            .err
            .contains("There is no post-processor with that name."));
        // by its name, beside the settings
        let dir = std::env::temp_dir().join(format!("cnc_clipost_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("phoenix-cnc-post-mine.txt"), "footer = M30\n").unwrap();
        let byname = run_with(
            &options(&["--post", "mine", &program.arg()]),
            &s,
            Some(&dir.join("phoenix-cnc.txt")),
        );
        assert!(byname.out.ends_with("M30\n"), "{}", byname.out);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_new_options_need_a_file_and_are_tools_that_print() {
        for args in [["--lookahead"], ["--import"]] {
            assert!(Options::parse(args.iter().map(|a| a.to_string())).is_err());
        }
        assert!(Options::parse(["--post".to_string(), "x".to_string()]).is_err());
        assert!(Options::parse(["--post".to_string()]).is_err());
        assert!(
            Options::parse(["--post".to_string(), " ".to_string(), "a.nc".to_string()]).is_err()
        );
        let o = options(&["--lookahead", "a.nc"]);
        assert!(wanted(&o));
        assert!(wanted(&options(&["--import", "a.svg"])));
        assert!(wanted(&options(&["--post", "x", "a.nc"])));
        let help = crate::cli::help_text();
        for w in ["--lookahead", "--import", "--post NAME"] {
            assert!(help.contains(w), "{w}");
        }
    }
}
