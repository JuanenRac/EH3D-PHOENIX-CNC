//! The tools of the command line that read a program and print a text, with no window: `--convert` (millimetres and inches), `--times` (the time by kind
//! of move), `--offsets` (the `H` and the `D` against the table of tools), `--format` (the format a controller takes) and `--compare` (two programs). The
//! work is in `units`, `timesplit`, `offsets`, `postcheck` and `compare`; this module reads the files, picks the language and the settings, and makes
//! the text and the exit code (0 clean, 1 something to read, 2 error), so the binary only prints what it is given. Nothing is sent to a machine.

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
    o.convert.is_some() || o.compare.is_some() || o.times || o.offsets || o.format.is_some()
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
    let Some(p) = crate::postcheck::Profile::named(profile) else {
        return Outcome::failed(say(lang, Msg::FmtUnknownProfile, &[profile]));
    };
    let found = crate::postcheck::check(lines, &p);
    Outcome {
        out: texts(&found, lang),
        err: String::new(),
        code: i32::from(to_read(&found)),
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
    let Some(path) = o.file.as_ref() else {
        return Outcome::failed("this needs a G-code file".to_string());
    };
    let lines = match read_lines(path) {
        Ok(l) => l,
        Err(why) => return Outcome::failed(why),
    };
    let lang = o.lang.unwrap_or(s.lang).code();
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
    match o.format.as_deref() {
        Some(profile) => format_check(&lines, profile, lang),
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
    let r = run_in(o, &s);
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
}
