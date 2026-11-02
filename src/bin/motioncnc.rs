//! The EH3D machine-control program.
//!
//! Usage: `motioncnc [FILE.NC] [--lang CODE] [--theme NAME]`
//! For now it drives the simulated controller: jog, zero, home, the commands of the operator and a G-code program with its path.

#![cfg_attr(windows, windows_subsystem = "windows")]

use eh3d_motioncnc::cli::Options;

fn main() -> eframe::Result {
    eh3d_motioncnc::crash::install("motioncnc", env!("CARGO_PKG_VERSION"));
    attach_console();
    match Options::parse(std::env::args().skip(1)) {
        Ok(o) => eh3d_motioncnc::gui::run(&o),
        Err(e) => {
            eprintln!("motioncnc: {e}");
            std::process::exit(2);
        }
    }
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
