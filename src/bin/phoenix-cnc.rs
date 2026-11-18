//! The EH3D machine-control program.
//!
//! Usage: `phoenix-cnc [FILE.NC] [--lang CODE] [--theme NAME]`, or `phoenix-cnc --report FILE.NC` to print what looks wrong in a program
//! For now it drives the simulated controller: jog, zero, home, the commands of the operator and a G-code program with its path.

#![cfg_attr(windows, windows_subsystem = "windows")]

use eh3d_phoenix_cnc::cli::Options;

fn main() -> eframe::Result {
    eh3d_phoenix_cnc::crash::install("phoenix-cnc", env!("CARGO_PKG_VERSION"));
    attach_console();
    match Options::parse(std::env::args().skip(1)) {
        Ok(o) if o.report => std::process::exit(print_report(&o)),
        Ok(o) => eh3d_phoenix_cnc::gui::run(&o),
        Err(e) => {
            eprintln!("phoenix-cnc: {e}");
            std::process::exit(2);
        }
    }
}

/// `--report FILE`: prints the report of the program and returns the exit code (0 when nothing needs reading, 1 when something does, 2 when
/// the file cannot be read). The settings come from the settings file, as in the window.
fn print_report(o: &Options) -> i32 {
    use eh3d_phoenix_cnc::{report, settings};
    let Some(path) = o.file.as_ref() else {
        return 2;
    };
    let bytes = match std::fs::read(path) {
        Ok(b) if b.len() <= 64 * 1024 * 1024 => b,
        Ok(_) => {
            eprintln!("phoenix-cnc: {} is too big for a report", path.display());
            return 2;
        }
        Err(e) => {
            eprintln!("phoenix-cnc: {}: {e}", path.display());
            return 2;
        }
    };
    // the same reading as the program: every byte is a character, so a file in any code page is read
    let text: String = bytes.iter().map(|&b| b as char).collect();
    let lines: Vec<String> = text.lines().map(str::to_string).collect();
    let file = std::fs::read_to_string(settings::settings_path()).unwrap_or_default();
    let s = settings::Settings::parse(&file);
    let cfg = report::Cfg {
        travel: s.travel,
        offset: [0.0; 3],
        rapid_mm_min: s.rapid_mm_min,
        default_feed: s.feed_mm_min,
        lang: o.lang.unwrap_or(s.lang).code().to_string(),
    };
    let r = report::build(&lines, &s.tuning, &s.lint, &cfg);
    print!("{}", r.text);
    i32::from(r.problems > 0)
}

/// A program with a window has no console of its own on Windows (no black window); when it was started from a terminal, what it prints (the help, a mistake in the options) goes to that terminal.
#[cfg(windows)]
fn attach_console() {
    #[link(name = "kernel32")]
    extern "system" {
        fn AttachConsole(process_id: u32) -> i32;
    }
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    // SAFETY: a call of the system with a constant; if there is no parent console it does nothing.
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

#[cfg(not(windows))]
fn attach_console() {}
