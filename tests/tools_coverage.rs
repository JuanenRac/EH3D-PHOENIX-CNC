//! The tools of the command line (the conversion of units, the time by kind of move, the offsets of the tools, the format for a controller and the comparison
//! of two programs) tried on the programs of `examples/` and on the properties that tie them together: a program in inches is the same path as the one in
//! millimetres, a program compared with itself does not differ, no tool says anything but 0 (clean) or 1 (something to read) of a program that exists, and no
//! text of any of them leaves a hole of a message unfilled in any of the eight languages.

use eh3d_phoenix_cnc::cli::Options;
use eh3d_phoenix_cnc::cli_tools::{run_in, wanted};
use eh3d_phoenix_cnc::compare::{compare, facts};
use eh3d_phoenix_cnc::gcode::toolpath;
use eh3d_phoenix_cnc::postcheck::{check, Profile, NAMES};
use eh3d_phoenix_cnc::settings::Settings;
use eh3d_phoenix_cnc::sim::Rates;
use eh3d_phoenix_cnc::timesplit::split;
use eh3d_phoenix_cnc::tuning::Tuning;
use eh3d_phoenix_cnc::units::convert;
use eh3d_phoenix_cnc::words::LANGS;
use std::path::PathBuf;

const RATES: Rates = Rates {
    rapid: 3000.0,
    default_feed: 600.0,
};

/// The programs of `examples/`: the path and the lines.
fn examples() -> Vec<(PathBuf, Vec<String>)> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples");
    let mut out: Vec<(PathBuf, Vec<String>)> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "nc"))
        .map(|p| {
            let text = std::fs::read_to_string(&p).unwrap();
            (p, text.lines().map(str::to_string).collect())
        })
        .collect();
    out.sort();
    assert!(out.len() >= 3, "the examples are there");
    out
}

fn parse(args: &[&str]) -> Options {
    Options::parse(args.iter().map(|a| a.to_string())).unwrap()
}

#[test]
fn every_tool_gives_0_or_1_on_every_example_in_every_language_with_no_hole_left() {
    let s = Settings::default();
    let tools: [&[&str]; 8] = [
        &["--times"],
        &["--offsets"],
        &["--format", "grbl"],
        &["--format", "full"],
        &["--format", "generic"],
        &["--convert", "in"],
        &["--convert", "mm"],
        &["--compare", "examples/01_square.nc"],
    ];
    for (path, _) in examples() {
        let file = path.to_string_lossy().to_string();
        for tool in tools {
            for lang in LANGS {
                let mut args: Vec<&str> = tool.to_vec();
                let compare_with =
                    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/01_square.nc");
                let compare_with = compare_with.to_string_lossy().to_string();
                if tool[0] == "--compare" {
                    args = vec!["--compare", &compare_with];
                }
                args.extend(["--lang", lang, &file]);
                let o = parse(&args);
                assert!(wanted(&o), "{args:?}");
                let r = run_in(&o, &s);
                assert!(r.code == 0 || r.code == 1, "{args:?}: {} {}", r.code, r.err);
                assert!(!r.out.contains('{') && !r.err.contains('{'), "{args:?}");
                assert!(
                    !r.out.is_empty() || !r.err.is_empty() || r.code == 0,
                    "{args:?}"
                );
            }
        }
    }
}

#[test]
fn a_program_in_inches_is_the_same_path_as_the_one_in_millimetres() {
    for (path, lines) in examples() {
        let inches = convert(&lines, true);
        let before = toolpath(&lines);
        let after = toolpath(&inches.lines);
        assert_eq!(before.len(), after.len(), "{}", path.display());
        for (a, b) in before.iter().zip(after.iter()) {
            for (p, q) in [(a.from, b.from), (a.to, b.to)] {
                assert!(
                    (p.0 - q.0).abs() < 0.002
                        && (p.1 - q.1).abs() < 0.002
                        && (p.2 - q.2).abs() < 0.002,
                    "{}: {p:?} against {q:?}",
                    path.display()
                );
            }
            // a G20 added at the top moves every line down by one
            let shift = inches.lines.len() - lines.len();
            assert_eq!((a.rapid, a.arc, a.line + shift), (b.rapid, b.arc, b.line));
        }
        // and back again, to the letter of the numbers a machine shows
        let back = convert(&inches.lines, false);
        let again = toolpath(&back.lines);
        assert_eq!(again.len(), before.len());
        for (a, b) in before.iter().zip(again.iter()) {
            assert!(
                (a.to.0 - b.to.0).abs() < 0.002 && (a.to.1 - b.to.1).abs() < 0.002,
                "{}",
                path.display()
            );
        }
    }
}

#[test]
fn a_program_and_its_inch_version_have_the_same_facts() {
    let tuning = Tuning::default();
    for (path, lines) in examples() {
        let inches = convert(&lines, true);
        let (a, b) = (
            facts(&lines, RATES, 5.0, 0.0, &tuning),
            facts(&inches.lines, RATES, 5.0, 0.0, &tuning),
        );
        assert!((a.cut_mm - b.cut_mm).abs() < 0.05, "{}", path.display());
        assert!((a.rapid_mm - b.rapid_mm).abs() < 0.05, "{}", path.display());
        assert!((a.seconds - b.seconds).abs() < 0.5, "{}", path.display());
        assert_eq!(
            (&a.tools, a.changes, a.moves),
            (&b.tools, b.changes, b.moves)
        );
        assert_eq!(a.deepest_mm.is_some(), b.deepest_mm.is_some());
    }
}

#[test]
fn a_program_compared_with_itself_does_not_differ_in_any_language() {
    let tuning = Tuning::default();
    for (path, lines) in examples() {
        let f = facts(&lines, RATES, 5.0, 0.0, &tuning);
        for lang in LANGS {
            let (text, n) = compare(&f, &f, ("a.nc", "b.nc"), lang);
            assert_eq!((n, text.len()), (0, 2), "{} {lang}", path.display());
        }
    }
}

#[test]
fn the_parts_of_the_time_add_up_for_every_example_and_every_change_time() {
    for (path, lines) in examples() {
        for change_s in [0.0, 12.5] {
            for accel in [0.0, 200.0] {
                let s = split(&lines, RATES, change_s, accel);
                let stats = eh3d_phoenix_cnc::stats::measure_accel(&lines, RATES, change_s, accel);
                assert!(
                    (s.total_s() - s.dwell_s - stats.seconds).abs() < 1e-6,
                    "{} {change_s} {accel}",
                    path.display()
                );
            }
        }
    }
}

#[test]
fn the_examples_are_in_the_format_of_the_controllers_they_are_for() {
    // the three samples are written for the simulator, which takes what GRBL takes: nothing to read for the profile that lists less
    for (path, lines) in examples() {
        for name in NAMES {
            let p = Profile::named(name).unwrap();
            let found = check(&lines, &p);
            assert!(
                found
                    .iter()
                    .all(|f| f.level < eh3d_phoenix_cnc::progcheck::Level::Warning),
                "{} for {name}: {found:?}",
                path.display()
            );
        }
    }
}
