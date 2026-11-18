//! The command line of the window.

use std::path::PathBuf;

use eh3d_view::texts::Lang;
use eh3d_view::theme::ThemeId;

use crate::viewer::ColorBy;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Options {
    pub lang: Option<Lang>,
    pub theme: Option<ThemeId>,
    /// The G-code file to open (the first one given without an option).
    pub file: Option<PathBuf>,
    /// More files given after the first: with it they make a queue of programs that run one after the other.
    pub more: Vec<PathBuf>,
    /// A picture of the window to save and leave (`--shot FILE`, after `--wait` frames).
    pub shot: Option<PathBuf>,
    pub wait: u32,
    /// Starts the program as soon as the window is open (`--start`): for the pictures of a program in progress. With a real controller
    /// (`--grbl`) it also needs `--start-real`, because the machine would move as the window opens.
    pub start: bool,
    /// Says that `--start` with `--grbl` is meant: a real machine starts the program as soon as the window is open.
    pub start_real: bool,
    /// A window to open at the start (`--window NAME`, one of `WINDOWS`): for the pictures.
    pub window: Option<String>,
    /// How the path is seen at the start (`--view plan|3d`); without it the view the settings remember.
    pub view_3d: Option<bool>,
    /// How the path is coloured at the start (`--color state|kind|progress|tool|feed`): for the pictures of the viewer.
    pub color: Option<ColorBy>,
    /// The line of the program (from 1, as the program panel counts) whose move is selected at the start (`--select LINE`): for the pictures.
    pub select: Option<usize>,
    /// Drives GRBL on a serial port instead of the simulator (`--grbl PORT[@BAUD]`).
    pub grbl: Option<String>,
    /// Lists the serial ports and leaves (`--ports`).
    pub ports: bool,
    /// Prints the report of the program (statistics, time, what looks wrong) and leaves (`--report FILE`); no window, nothing moves.
    pub report: bool,
    /// With `--report`: also writes the work report as one page of HTML (`--html FILE`); a file that is there is not written over.
    pub html: Option<PathBuf>,
    /// Prints the version and leaves (`--version`, `-V`), or the help and leaves (`--help`, `-h`); no window, nothing moves.
    pub version: bool,
    pub help: bool,
}

/// The windows that `--window` opens, in the order of the help.
pub const WINDOWS: [&str; 17] = [
    "settings",
    "settings-checks",
    "settings-probe",
    "settings-queue",
    "open",
    "save",
    "edit",
    "about",
    "summary",
    "keys",
    "check",
    "keepout",
    "stats",
    "macros",
    "probe",
    "probe-grid",
    "resume",
];

/// The text of `--help`: every option the command line reads.
pub fn help_text() -> String {
    format!(
        "phoenix-cnc {}\n\
         \n\
         Usage: phoenix-cnc [OPTIONS] [FILE.NC ...]\n\
         \n\
         A G-code file is opened as the program; more files make a queue that runs one after the other.\n\
         \n\
         Options:\n\
         \x20 --report FILE      print the statistics, the time and what looks wrong in a program, and leave (exit code 0 clean, 1 something to read, 2 error)\n\
         \x20 --html FILE        with --report, also write the work report as one page of HTML (a file that is there is not written over)\n\
         \x20 --lang CODE        the language (en, es, de, fr, it, ja, zh, pt and the others of the window)\n\
         \x20 --theme NAME       the colours\n\
         \x20 --view plan|3d     how the path is seen at the start\n\
         \x20 --color MODE       how the path is coloured at the start ({})\n\
         \x20 --select LINE      select the move of that line of the program (from 1) at the start\n\
         \x20 --window NAME      open a window at the start ({})\n\
         \x20 --shot FILE        save a picture of the window and leave (after --wait frames)\n\
         \x20 --wait FRAMES      the frames --shot waits for\n\
         \x20 --start            start the program as soon as the window is open\n\
         \x20 --start-real       also needed to --start with a real controller (--grbl)\n\
         \x20 --grbl PORT[@BAUD] drive a GRBL controller on a serial port (a build with --features serial)\n\
         \x20 --ports            list the serial ports and leave\n\
         \x20 --version, -V      print the version and leave\n\
         \x20 --help, -h         print this text and leave\n",
        env!("CARGO_PKG_VERSION"),
        ColorBy::ALL.map(ColorBy::key).join(", "),
        WINDOWS.join(", ")
    )
}

impl Options {
    pub fn parse<I: IntoIterator<Item = String>>(args: I) -> Result<Options, String> {
        let mut o = Options {
            wait: 6,
            ..Default::default()
        };
        let mut it = args.into_iter();
        while let Some(a) = it.next() {
            let mut value = |name: &str| it.next().ok_or(format!("{name} needs a value"));
            match a.as_str() {
                "--lang" => {
                    let v = value("--lang")?;
                    o.lang = Some(Lang::from_code(&v).ok_or(format!("unknown language '{v}'"))?);
                }
                "--theme" => {
                    let v = value("--theme")?;
                    o.theme = Some(ThemeId::from_key(&v).ok_or(format!("unknown theme '{v}'"))?);
                }
                "--shot" => o.shot = Some(PathBuf::from(value("--shot")?)),
                "--wait" => {
                    let v = value("--wait")?;
                    o.wait = v
                        .parse()
                        .map_err(|_| format!("--wait needs a number, not '{v}'"))?;
                }
                "--start" => o.start = true,
                "--start-real" => o.start_real = true,
                "--window" => o.window = Some(value("--window")?),
                "--view" => {
                    let v = value("--view")?;
                    o.view_3d = Some(match v.as_str() {
                        "plan" | "2d" => false,
                        "3d" => true,
                        _ => return Err(format!("unknown view '{v}' (plan or 3d)")),
                    });
                }
                "--color" => {
                    let v = value("--color")?;
                    o.color = Some(ColorBy::from_key(&v).ok_or(format!(
                        "unknown colouring '{v}' ({})",
                        ColorBy::ALL.map(ColorBy::key).join(", ")
                    ))?);
                }
                "--select" => {
                    let v = value("--select")?;
                    o.select = match v.parse::<usize>() {
                        Ok(n) if n >= 1 => Some(n),
                        _ => return Err(format!("--select needs a line number from 1, not '{v}'")),
                    };
                }
                "--grbl" => o.grbl = Some(value("--grbl")?),
                "--ports" => o.ports = true,
                "--report" => o.report = true,
                "--html" => o.html = Some(PathBuf::from(value("--html")?)),
                "--version" | "-V" => o.version = true,
                "--help" | "-h" => o.help = true,
                other if !other.starts_with("--") => {
                    if o.file.is_none() {
                        o.file = Some(PathBuf::from(other));
                    } else {
                        o.more.push(PathBuf::from(other));
                    }
                }
                other => return Err(format!("unknown option '{other}'")),
            }
        }
        // the version and the help are answered whatever else was given
        if o.version || o.help {
            return Ok(o);
        }
        if o.report && o.file.is_none() {
            return Err("--report needs a G-code file".to_string());
        }
        if o.html.is_some() && !o.report {
            return Err("--html only goes with --report".to_string());
        }
        if o.select.is_some() && o.file.is_none() {
            return Err("--select needs a G-code file".to_string());
        }
        if o.start_real && !o.start {
            return Err("--start-real only goes with --start".to_string());
        }
        if o.start && o.grbl.is_some() && !o.start_real {
            return Err(
                "--start with --grbl would move a real machine as soon as the window opens: add --start-real if that is what is wanted"
                    .to_string(),
            );
        }
        Ok(o)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Result<Options, String> {
        Options::parse(s.split_whitespace().map(String::from))
    }

    #[test]
    fn the_options_are_read() {
        let o = parse("--lang de --theme neon --shot a.png --wait 9 --start job.nc").unwrap();
        assert_eq!(o.lang, Some(Lang::De));
        assert_eq!(o.theme, Some(ThemeId::Neon));
        assert_eq!((o.wait, o.start), (9, true));
        assert_eq!(
            parse("--window about").unwrap().window.as_deref(),
            Some("about")
        );
        assert_eq!(o.file, Some(PathBuf::from("job.nc")));
        assert_eq!(o.shot, Some(PathBuf::from("a.png")));
        assert_eq!(parse("").unwrap().wait, 6);
        let g = parse("--grbl COM3@250000 --ports").unwrap();
        assert_eq!((g.grbl.as_deref(), g.ports), (Some("COM3@250000"), true));
        assert!(parse("--grbl").is_err());
        assert_eq!(parse("--view 3d").unwrap().view_3d, Some(true));
        assert_eq!(parse("--view plan").unwrap().view_3d, Some(false));
        assert_eq!(parse("").unwrap().view_3d, None);
        assert!(parse("--view side").is_err());
    }

    #[test]
    fn a_real_controller_does_not_start_a_program_by_itself_unless_it_is_said_twice() {
        // the simulator starts as before
        assert!(parse("--start job.nc").unwrap().start);
        // a real machine needs the second word
        assert!(parse("--grbl COM3 --start job.nc").is_err());
        assert!(parse("--start --grbl COM3").is_err());
        let o = parse("--grbl COM3 --start --start-real job.nc").unwrap();
        assert!(o.start && o.start_real && o.grbl.is_some());
        // the second word alone says nothing, and without a controller it changes nothing
        assert!(parse("--start-real").is_err());
        assert!(!parse("--grbl COM3 job.nc").unwrap().start);
        assert!(parse("--start --start-real").unwrap().start_real);
    }

    #[test]
    fn several_files_make_a_queue_in_the_order_given() {
        let o = parse("a.nc b.nc c.nc").unwrap();
        assert_eq!(o.file, Some(PathBuf::from("a.nc")));
        assert_eq!(o.more, vec![PathBuf::from("b.nc"), PathBuf::from("c.nc")]);
        assert!(parse("a.nc").unwrap().more.is_empty());
    }

    #[test]
    fn the_report_needs_a_file() {
        assert!(parse("--report").is_err());
        let o = parse("--report job.nc --lang es").unwrap();
        assert!(o.report);
        assert_eq!(o.file, Some(PathBuf::from("job.nc")));
        assert!(!parse("job.nc").unwrap().report);
    }

    #[test]
    fn the_version_and_the_help_are_asked_for_and_win_over_the_rest() {
        assert!(parse("--version").unwrap().version);
        assert!(parse("-V").unwrap().version);
        assert!(parse("--help").unwrap().help);
        assert!(parse("-h").unwrap().help);
        // nothing else is needed or checked
        assert!(parse("--report --version").unwrap().version);
        assert!(parse("--grbl COM3 --start --help").unwrap().help);
        assert!(!parse("job.nc").unwrap().version && !parse("job.nc").unwrap().help);
    }

    #[test]
    fn the_help_names_every_option_the_command_line_reads() {
        let h = help_text();
        assert!(h.starts_with(&format!("phoenix-cnc {}", env!("CARGO_PKG_VERSION"))));
        for option in [
            "--report",
            "--html",
            "--lang",
            "--theme",
            "--view",
            "--window",
            "--color",
            "--select",
            "--shot",
            "--wait",
            "--start",
            "--start-real",
            "--grbl",
            "--ports",
            "--version",
            "--help",
        ] {
            assert!(h.contains(option), "{option} is not in the help");
        }
        // every window that --window opens is listed
        for name in WINDOWS {
            assert!(h.contains(name), "{name} is not in the help");
        }
    }

    #[test]
    fn no_window_is_named_twice() {
        let mut seen = std::collections::HashSet::new();
        for name in WINDOWS {
            assert!(seen.insert(name), "{name} twice");
        }
    }

    #[test]
    fn the_html_report_goes_with_the_report() {
        let o = parse("--report --html out.html job.nc").unwrap();
        assert_eq!(o.html, Some(PathBuf::from("out.html")));
        assert!(o.report);
        assert!(parse("--html out.html job.nc").is_err());
        assert!(parse("--report job.nc --html").is_err());
        assert_eq!(parse("--report job.nc").unwrap().html, None);
    }

    #[test]
    fn the_colouring_and_the_selected_line_are_read_for_the_pictures() {
        let o = parse("--color Tool --select 4 job.nc --shot a.png").unwrap();
        assert_eq!((o.color, o.select), (Some(ColorBy::Tool), Some(4)));
        assert_eq!(parse("job.nc").unwrap().color, None);
        assert_eq!(parse("job.nc").unwrap().select, None);
        for c in ColorBy::ALL {
            let o = parse(&format!("--color {}", c.key())).unwrap();
            assert_eq!(o.color, Some(c));
        }
        // a colouring that does not exist, a line that is not a number from 1, and a line with no program to select in
        assert!(parse("--color rainbow").is_err());
        assert!(parse("--color").is_err());
        assert!(parse("--select 0 job.nc").is_err());
        assert!(parse("--select two job.nc").is_err());
        assert!(parse("--select -3 job.nc").is_err());
        assert!(parse("--select 4").is_err());
        // every colouring is in the help
        let h = help_text();
        for c in ColorBy::ALL {
            assert!(h.contains(c.key()), "{} is not in the help", c.key());
        }
    }

    #[test]
    fn what_is_wrong_is_said() {
        assert!(parse("--lang xx").is_err());
        assert!(parse("--theme pink").is_err());
        assert!(parse("--wait many").is_err());
        assert!(parse("--bogus").is_err());
        assert!(parse("--shot").is_err());
    }
}
