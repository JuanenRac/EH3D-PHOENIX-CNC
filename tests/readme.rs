//! What the README and the scripts run.sh / run.bat promise can be checked without opening the window: the three show the same sample, the sample is a file that exists,
//! the arguments of the command are accepted by the command line of the program and every program of examples/ is read into a path.

use eh3d_phoenix_cnc::cli::Options;
use eh3d_phoenix_cnc::gcode::toolpath;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(name: &str) -> String {
    std::fs::read_to_string(root().join(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn words(s: &str) -> Vec<String> {
    s.split_whitespace().map(str::to_string).collect()
}

/// The arguments after `--` of every `cargo run` line of the README.
fn of_readme() -> Vec<Vec<String>> {
    read("README.md")
        .lines()
        .filter_map(|l| {
            l.trim()
                .strip_prefix("cargo run --release --features gui -- ")
                .map(words)
        })
        .collect()
}

fn of_run_sh() -> Vec<String> {
    let t = read("run.sh");
    let line = t
        .lines()
        .find(|l| l.contains("cargo run") && l.contains(":-"))
        .expect("run.sh has the sample");
    let sample = line.split(":-").nth(1).unwrap().split('}').next().unwrap();
    words(sample)
}

fn of_run_bat() -> Vec<String> {
    let t = read("run.bat");
    let line = t
        .lines()
        .find(|l| l.contains("cargo run") && !l.contains("%*"))
        .expect("run.bat has the sample");
    words(line.split(" -- ").nth(1).unwrap())
        .iter()
        .map(|w| w.replace('\\', "/"))
        .collect()
}

#[test]
fn the_readme_and_both_scripts_show_the_same_sample() {
    let readme = of_readme();
    assert!(!readme.is_empty(), "the README shows no command");
    assert!(
        readme.contains(&of_run_sh()),
        "run.sh differs from the README"
    );
    assert!(
        readme.contains(&of_run_bat()),
        "run.bat differs from the README"
    );
}

#[test]
fn the_arguments_of_the_readme_are_accepted_and_name_a_file_that_exists() {
    for args in of_readme() {
        let o = Options::parse(args.clone()).unwrap_or_else(|e| panic!("{args:?}: {e}"));
        let file = o.file.unwrap_or_else(|| panic!("{args:?} opens no file"));
        assert!(
            root().join(&file).is_file(),
            "{} is not there",
            file.display()
        );
    }
}

#[test]
fn every_example_is_read_into_a_path() {
    let mut seen = 0;
    for entry in std::fs::read_dir(root().join("examples")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "nc") {
            let text = std::fs::read_to_string(&path).unwrap();
            let lines: Vec<String> = text.lines().map(str::to_string).collect();
            assert!(
                !toolpath(&lines).is_empty(),
                "{} has no moves",
                path.display()
            );
            seen += 1;
        }
    }
    assert!(seen >= 3, "the examples are fewer than the README says");
}

#[test]
fn the_scripts_wait_for_a_key_unless_nopause_is_set() {
    assert!(read("run.sh").contains("NOPAUSE"));
    assert!(read("run.bat").contains("NOPAUSE"));
}
