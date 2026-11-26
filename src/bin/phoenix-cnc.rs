//! The EH3D machine-control program.
//!
//! Usage: `phoenix-cnc [FILE.NC] [--lang CODE] [--theme NAME]`, or `phoenix-cnc --report FILE.NC` to print what looks wrong in a program
//! For now it drives the simulated controller: jog, zero, home, the commands of the operator and a G-code program with its path.

#![cfg_attr(windows, windows_subsystem = "windows")]

use eh3d_phoenix_cnc::cli::Options;

fn main() -> eframe::Result {
    eh3d_phoenix_cnc::crash::install("phoenix-cnc", env!("CARGO_PKG_VERSION"));
    attach_console();
    let words: Vec<String> = std::env::args().skip(1).collect();
    if words.first().is_some_and(|w| w == "project") {
        let done = eh3d_phoenix_cnc::eh3dcli::run(&words[1..]);
        print!("{}", done.out);
        eprint!("{}", done.err);
        std::process::exit(done.code);
    }
    match Options::parse(words.into_iter()) {
        Ok(o) if o.version => {
            println!("phoenix-cnc {}", env!("CARGO_PKG_VERSION"));
            std::process::exit(0);
        }
        Ok(o) if o.help => {
            print!("{}", eh3d_phoenix_cnc::cli::help_text());
            std::process::exit(0);
        }
        Ok(o) if o.report => std::process::exit(print_report(&o)),
        Ok(o) if o.gcode.is_some() => std::process::exit(print_gcode_help(&o)),
        Ok(o) if o.macros.is_some() => std::process::exit(print_macros_check(&o)),
        Ok(o) if eh3d_phoenix_cnc::cli_tools::wanted(&o) => {
            std::process::exit(eh3d_phoenix_cnc::cli_tools::run(&o))
        }
        Ok(o) => eh3d_phoenix_cnc::gui::run(&o),
        Err(e) => {
            eprintln!("phoenix-cnc: {e}");
            std::process::exit(2);
        }
    }
}

/// `--gcode WORD`: prints the help of a G-code word, or of the words of a line, and returns the exit code (0 when there is help, 2 when
/// no word of the argument has any).
fn print_gcode_help(o: &Options) -> i32 {
    let arg = o.gcode.as_deref().unwrap_or("");
    let lang = o.lang.map_or("en", |l| l.code());
    match eh3d_phoenix_cnc::gcodehelp::answer(arg, lang) {
        Some(text) => {
            print!("{text}");
            0
        }
        None => {
            eprintln!("phoenix-cnc: no help for '{arg}'");
            2
        }
    }
}

/// `--macros FILE`: checks a file of macros and returns the exit code (0 when nothing needs reading, 1 when something does, 2 when the
/// file cannot be read). The settings come from the settings file, as in the window.
fn print_macros_check(o: &Options) -> i32 {
    use eh3d_phoenix_cnc::{macrocheck, settings};
    let Some(path) = o.macros.as_ref() else {
        return 2;
    };
    let bytes = match std::fs::read(path) {
        Ok(b) if b.len() <= 1024 * 1024 => b,
        Ok(_) => {
            eprintln!(
                "phoenix-cnc: {} is too big for a file of macros",
                path.display()
            );
            return 2;
        }
        Err(e) => {
            eprintln!("phoenix-cnc: {}: {e}", path.display());
            return 2;
        }
    };
    let file = std::fs::read_to_string(settings::settings_path()).unwrap_or_default();
    let s = settings::Settings::parse(&file);
    let lang = o.lang.unwrap_or(s.lang).code();
    let (text, problems) =
        macrocheck::validate_text(&String::from_utf8_lossy(&bytes), &s.tuning, lang);
    print!("{text}");
    i32::from(problems > 0)
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
    let mut s = settings::Settings::parse(&file);
    if let Some(name) = o.profile.as_ref() {
        if !s.use_profile(name) {
            eprintln!("phoenix-cnc: no machine profile is named '{name}'");
            return 2;
        }
    }
    // the material chosen in the settings holds the program to its ranges of feed and speed
    let library = eh3d_phoenix_cnc::materials::load(&settings::settings_path());
    s.resolve_material(&library);
    s.resolve_atc_table(eh3d_phoenix_cnc::atc::load_table(&settings::settings_path()));
    s.resolve_tool_table(eh3d_phoenix_cnc::tools::load(&settings::settings_path()));
    let cfg = report::Cfg {
        travel: s.travel,
        offset: [0.0; 3],
        rapid_mm_min: s.rapid_mm_min,
        default_feed: s.feed_mm_min,
        lang: o.lang.unwrap_or(s.lang).code().to_string(),
    };
    let r = report::build(&lines, &s.tuning, &s.lint, &cfg);
    print!("{}", r.text);
    if let Some(out) = o.html.as_ref() {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let page = eh3d_phoenix_cnc::workreport::html(&eh3d_phoenix_cnc::workreport::Input {
            name: &name,
            lines: &lines,
            tuning: &s.tuning,
            lint: &s.lint,
            cfg: &cfg,
            height_map: None,
            version: env!("CARGO_PKG_VERSION"),
        });
        match eh3d_phoenix_cnc::workreport::write_new(out, &page) {
            Ok(()) => eprintln!("phoenix-cnc: work report written to {}", out.display()),
            Err(e) => {
                eprintln!("phoenix-cnc: {}: {e}", out.display());
                return 2;
            }
        }
    }
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
