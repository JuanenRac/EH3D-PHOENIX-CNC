//! The command line of the window.

use std::path::PathBuf;

use eh3d_view::texts::Lang;
use eh3d_view::theme::ThemeId;

use crate::view::Preset;
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
    /// The machine profile whose values the settings take at the start (`--profile NAME`); the name must be one of the profiles saved.
    pub profile: Option<String>,
    /// A window to open at the start (`--window NAME`, one of `WINDOWS`): for the pictures.
    pub window: Option<String>,
    /// How the path is seen at the start (`--view plan|3d`); without it the view the settings remember.
    pub view_3d: Option<bool>,
    /// A fixed view the 3D view starts in (`--view top|front|side|iso`): `top` is the plan.
    pub view_preset: Option<Preset>,
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
    /// Prints the help of a G-code word, or of every word of a line, and leaves (`--gcode WORD`); no window, nothing moves.
    pub gcode: Option<String>,
    /// Checks a file of macros and leaves (`--macros FILE`): what a macro leaves on, an empty one, one that ends the program (exit code 0
    /// clean, 1 something to read, 2 error); no window, nothing moves.
    pub macros: Option<PathBuf>,
    /// Prints the program in the other unit and leaves (`--convert in|mm`, with the file): `in` or `mm`, as `units::convert` takes it.
    pub convert: Option<String>,
    /// Compares the program with another one and leaves (`--compare FILE2`, with the first file); exit code 0 equal, 1 different, 2 error.
    pub compare: Option<PathBuf>,
    /// Prints the time of the program by kind of move and leaves (`--times`, with the file).
    pub times: bool,
    /// Checks the `H` and `D` words and the length offsets against the table of tools and leaves (`--offsets`, with the file).
    pub offsets: bool,
    /// Checks the format of the program for a kind of controller and leaves (`--format grbl|full|generic`, or the name or the path of a profile file of the
    /// user, with the file).
    pub format: Option<String>,
    /// Checks the tool holder against the material (the `reach` of the tools in the table) and leaves (`--holder`, with the file).
    pub holder: bool,
    /// Prints the wear of the tools pass by pass and leaves (`--life`, with the file).
    pub life: bool,
    /// Prints the program as it would restart from this line (from 1), with the state rebuilt, and leaves (`--restart LINE`, with the file).
    pub restart: Option<usize>,
    /// Prints the settings of a kind of machine and leaves (`--template NAME`), or lists them (`--templates`); no file.
    pub template: Option<String>,
    pub templates: bool,
    /// Prints the time of the program with the look-ahead of the planner and leaves (`--lookahead`, with the file).
    pub lookahead: bool,
    /// The file is a drawing (HPGL, DXF or SVG) to turn into a program that is printed, and leaves (`--import`, with the file).
    pub import: bool,
    /// The post-processor (its name beside the settings, or the path of its file) the program printed by `--import`, or the program of the
    /// file, goes through, and leaves (`--post NAME`, with the file).
    pub post: Option<String>,
    /// Writes what a tool prints to this file instead of the standard output, and never over a file that is there (`--out FILE`).
    pub out: Option<PathBuf>,
    /// Prints the version and leaves (`--version`, `-V`), or the help and leaves (`--help`, `-h`); no window, nothing moves.
    pub version: bool,
    pub help: bool,
}

/// The windows that `--window` opens, in the order of the help.
pub const WINDOWS: [&str; 22] = [
    "settings",
    "settings-checks",
    "settings-probe",
    "settings-queue",
    "settings-atc",
    "settings-materials",
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
    "transform",
    "bookmarks",
    "probe",
    "probe-grid",
    "resume",
    "machine",
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
         \x20 --gcode WORD       print the help of a G-code word (G83, M6, F) or of the words of a line, in the language of --lang, and leave\n\
         \x20 --macros FILE      check a file of macros (what a macro leaves on, an empty one, one that ends the program), and leave (exit code 0 clean, 1 something to read, 2 error)\n\
         \x20 --convert UNIT     print the program in the other unit (in or mm) and say what was converted, and leave (exit code 0 clean, 1 something to read, 2 error)\n\
         \x20 --compare FILE2    compare the program with another one (lengths, time, tools, reach, depth, findings), and leave (exit code 0 equal, 1 different, 2 error)\n\
         \x20 --times            print the time of the program by kind of move (rapid, cutting, plunges, arcs, dwells, tool changes), and leave\n\
         \x20 --offsets          check the H and D words and the length offsets against the table of tools, and leave (exit code 0 clean, 1 something to read, 2 error)\n\
         \x20 --format PROFILE   check the format of the program for a kind of controller (grbl, full, generic, or the name or the path of a profile file of your own), and leave (exit code 0 clean, 1 something to read, 2 error)\n\
         \x20 --holder           check the tool holder against the material with the reach of each tool in the table of tools, and leave (exit code 0 clean, 1 something to read, 2 error)\n\
         \x20 --life             print the wear of the tools pass by pass against the life in the table of tools, and leave (exit code 0 none worn out, 1 one is, 2 error)\n\
         \x20 --restart LINE     print the program as it restarts from that line (from 1), with the state before it rebuilt and a pause before the first move, and leave\n\
         \x20 --template NAME    print the settings of a kind of machine (small-mill, router, engraver, pcb-mill) to start from, and leave\n\
         \x20 --templates        list the machine templates, and leave\n\
         \x20 --lookahead        print the time of the program with the look-ahead of the planner (corners, acceleration of each axis), and leave\n\
         \x20 --import           read the file as a drawing (HPGL, DXF or SVG) and print the G-code that cuts along it, and leave (exit code 0 clean, 1 something to read, 2 error)\n\
         \x20 --post NAME        pass the program (or the one --import makes) through the post-processor NAME (a file beside the settings, or its path), and leave\n\
         \x20 --out FILE         write what the tool prints to FILE and never over a file that is there (exit code 2 if it is)\n\
         \x20 --lang CODE        the language (en, es, de, fr, it, ja, zh, pt and the others of the window)\n\
         \x20 --theme NAME       the colours\n\
         \x20 --view VIEW        how the path is seen at the start (plan, 3d, top, front, side or iso)\n\
         \x20 --color MODE       how the path is coloured at the start ({})\n\
         \x20 --select LINE      select the move of that line of the program (from 1) at the start\n\
         \x20 --profile NAME     start with the values of a machine profile saved in the settings (travel, spindle, feeds, probe)\n\
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
                "--profile" => {
                    let v = value("--profile")?;
                    if crate::profiles::clean_name(&v).is_none() {
                        return Err("--profile needs the name of a machine profile".to_string());
                    }
                    o.profile = Some(v);
                }
                "--view" => {
                    let v = value("--view")?;
                    o.view_3d = Some(match v.as_str() {
                        "plan" | "2d" => false,
                        "3d" => true,
                        other => match Preset::from_key(other) {
                            Some(p) => {
                                o.view_preset = Some(p);
                                p != Preset::Top
                            }
                            None => {
                                return Err(format!(
                                    "unknown view '{v}' (plan, 3d, top, front, side or iso)"
                                ))
                            }
                        },
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
                "--gcode" => {
                    let v = value("--gcode")?;
                    if v.trim().is_empty() {
                        return Err("--gcode needs a G-code word such as G83".to_string());
                    }
                    o.gcode = Some(v);
                }
                "--macros" => o.macros = Some(PathBuf::from(value("--macros")?)),
                "--convert" => {
                    let v = value("--convert")?;
                    o.convert = Some(
                        match v.trim().to_ascii_lowercase().as_str() {
                            "in" | "inch" | "inches" | "g20" => "in",
                            "mm" | "metric" | "millimetres" | "millimeters" | "g21" => "mm",
                            _ => return Err(format!("--convert needs in or mm, not '{v}'")),
                        }
                        .to_string(),
                    );
                }
                "--compare" => o.compare = Some(PathBuf::from(value("--compare")?)),
                "--times" => o.times = true,
                "--offsets" => o.offsets = true,
                "--format" => {
                    let v = value("--format")?;
                    if v.trim().is_empty() {
                        return Err(format!(
                            "--format needs a profile ({}) or the name or the path of a profile file",
                            crate::postcheck::NAMES.join(", ")
                        ));
                    }
                    // one of the three of the program is kept in lower case; anything else is the name or the path of a file of the user, read when it is used
                    o.format = Some(if crate::postcheck::Profile::named(&v).is_some() {
                        v.trim().to_ascii_lowercase()
                    } else {
                        v.trim().to_string()
                    });
                }
                "--lookahead" => o.lookahead = true,
                "--import" => o.import = true,
                "--post" => {
                    let v = value("--post")?;
                    if v.trim().is_empty() {
                        return Err("--post needs the name of a post-processor".to_string());
                    }
                    o.post = Some(v.trim().to_string());
                }
                "--holder" => o.holder = true,
                "--life" => o.life = true,
                "--restart" => {
                    let v = value("--restart")?;
                    o.restart = match v.trim().parse::<usize>() {
                        Ok(n) if n >= 1 => Some(n),
                        _ => {
                            return Err(format!("--restart needs a line number from 1, not '{v}'"))
                        }
                    };
                }
                "--template" => {
                    let v = value("--template")?;
                    if crate::templates::find(&v).is_none() {
                        return Err(format!(
                            "unknown template '{v}' ({})",
                            crate::templates::names().join(", ")
                        ));
                    }
                    o.template = Some(v.trim().to_ascii_lowercase());
                }
                "--templates" => o.templates = true,
                "--out" => o.out = Some(PathBuf::from(value("--out")?)),
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
        for (asked, name) in [
            (o.convert.is_some(), "--convert"),
            (o.compare.is_some(), "--compare"),
            (o.times, "--times"),
            (o.offsets, "--offsets"),
            (o.format.is_some(), "--format"),
            (o.holder, "--holder"),
            (o.life, "--life"),
            (o.restart.is_some(), "--restart"),
            (o.lookahead, "--lookahead"),
            (o.import, "--import"),
            (o.post.is_some(), "--post"),
        ] {
            if asked && o.file.is_none() {
                return Err(format!("{name} needs a G-code file"));
            }
        }
        if o.out.is_some() && !crate::cli_tools::wanted(&o) {
            return Err(
                "--out only goes with a tool that prints (--convert, --compare, --times, --offsets, --format, --holder, --life, --restart, --lookahead, --import, --post, --template, --templates)"
                    .to_string(),
            );
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
        assert!(parse("--view sideways").is_err());
        // the fixed views: the one from above is the plan, the others are 3D
        let f = parse("--view front").unwrap();
        assert_eq!(
            (f.view_3d, f.view_preset),
            (Some(true), Some(Preset::Front))
        );
        let t = parse("--view Top").unwrap();
        assert_eq!((t.view_3d, t.view_preset), (Some(false), Some(Preset::Top)));
        assert_eq!(parse("--view iso").unwrap().view_preset, Some(Preset::Iso));
        assert_eq!(parse("--view 3d").unwrap().view_preset, None);
        assert!(help_text().contains("front"));
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
            "--gcode",
            "--macros",
            "--convert",
            "--compare",
            "--times",
            "--offsets",
            "--format",
            "--holder",
            "--life",
            "--restart",
            "--template",
            "--templates",
            "--out",
            "--lang",
            "--theme",
            "--view",
            "--window",
            "--profile",
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
    fn a_machine_profile_is_asked_for_by_name() {
        assert_eq!(
            parse("--profile Mill").unwrap().profile.as_deref(),
            Some("Mill")
        );
        assert_eq!(parse("job.nc").unwrap().profile, None);
        // it goes with the report as well as with the window
        let o = parse("--report job.nc --profile Mill").unwrap();
        assert!(o.report && o.profile.is_some());
        // a name with nothing in it, or none, is refused
        assert!(parse("--profile").is_err());
        assert!(parse("--profile |").is_err());
        assert!(help_text().contains("--profile"));
    }

    #[test]
    fn the_conversion_the_comparison_the_times_the_offsets_and_the_format_are_asked_for_with_their_own_options(
    ) {
        let c = parse("--convert IN job.nc").unwrap();
        assert_eq!(c.convert.as_deref(), Some("in"));
        assert_eq!(
            parse("--convert g21 job.nc").unwrap().convert.as_deref(),
            Some("mm")
        );
        assert_eq!(
            parse("--convert inches job.nc").unwrap().convert.as_deref(),
            Some("in")
        );
        assert!(parse("--convert cm job.nc").is_err());
        assert!(parse("--convert").is_err());
        let k = parse("--compare other.nc job.nc").unwrap();
        assert_eq!(k.compare, Some(PathBuf::from("other.nc")));
        assert_eq!(k.file, Some(PathBuf::from("job.nc")));
        assert!(parse("--times job.nc").unwrap().times);
        assert!(parse("--offsets job.nc").unwrap().offsets);
        let f = parse("--format GRBL job.nc").unwrap();
        assert_eq!(f.format.as_deref(), Some("grbl"));
        // a name that is none of the three is the name or the path of a profile file of the user, looked for when it is used
        assert_eq!(
            parse("--format nosuchprofile job.nc")
                .unwrap()
                .format
                .as_deref(),
            Some("nosuchprofile")
        );
        // each one needs the program to read
        for args in [
            "--convert in",
            "--compare other.nc",
            "--times",
            "--offsets",
            "--format grbl",
        ] {
            let why = parse(args).unwrap_err();
            assert!(why.ends_with("needs a G-code file"), "{args}: {why}");
        }
        // none of them is asked for by a plain file
        let plain = parse("job.nc").unwrap();
        assert!(plain.convert.is_none() && plain.compare.is_none() && plain.format.is_none());
        assert!(!plain.times && !plain.offsets);
        let h = help_text();
        for word in [
            "--convert UNIT",
            "--compare FILE2",
            "--times",
            "--offsets",
            "--format PROFILE",
        ] {
            assert!(h.contains(word), "{word}");
        }
    }

    #[test]
    fn the_help_of_a_word_and_the_check_of_the_macros_are_asked_for_with_their_own_options() {
        let g = parse("--gcode G83 --lang es").unwrap();
        assert_eq!(g.gcode.as_deref(), Some("G83"));
        assert_eq!(g.lang, Some(Lang::Es));
        assert_eq!(parse("job.nc").unwrap().gcode, None);
        assert!(parse("--gcode").is_err());
        let m = parse("--macros my.txt").unwrap();
        assert_eq!(m.macros, Some(PathBuf::from("my.txt")));
        assert_eq!(parse("job.nc").unwrap().macros, None);
        assert!(parse("--macros").is_err());
        // the help answers first
        assert!(parse("--gcode G1 --help").unwrap().help);
        let h = help_text();
        assert!(h.contains("--gcode WORD") && h.contains("--macros FILE"));
    }

    #[test]
    fn what_is_wrong_is_said() {
        assert!(parse("--lang xx").is_err());
        assert!(parse("--theme pink").is_err());
        assert!(parse("--wait many").is_err());
        assert!(parse("--bogus").is_err());
        assert!(parse("--shot").is_err());
    }

    #[test]
    fn the_holder_the_life_the_restart_and_the_templates_are_asked_for_with_their_own_options() {
        assert!(parse("--holder job.nc").unwrap().holder);
        assert!(parse("--life job.nc").unwrap().life);
        assert_eq!(parse("--restart 12 job.nc").unwrap().restart, Some(12));
        assert_eq!(parse("--restart  1 job.nc").unwrap().restart, Some(1));
        for bad in [
            "--restart 0 job.nc",
            "--restart -3 job.nc",
            "--restart two job.nc",
            "--restart",
        ] {
            assert!(parse(bad).is_err(), "{bad}");
        }
        // the first three need the program, the templates do not
        for args in ["--holder", "--life", "--restart 4"] {
            assert!(
                parse(args).unwrap_err().ends_with("needs a G-code file"),
                "{args}"
            );
        }
        assert_eq!(
            parse("--template ROUTER").unwrap().template.as_deref(),
            Some("router")
        );
        let why = parse("--template lathe").unwrap_err();
        assert!(
            why.contains("small-mill, router, engraver, pcb-mill"),
            "{why}"
        );
        assert!(parse("--templates").unwrap().templates);
        let plain = parse("job.nc").unwrap();
        assert!(!plain.holder && !plain.life && plain.restart.is_none());
        assert!(plain.template.is_none() && !plain.templates && plain.out.is_none());
    }

    #[test]
    fn a_format_profile_of_the_user_is_given_by_name_or_by_path() {
        let by_path = parse("--format shop/my-profile.txt job.nc").unwrap();
        assert_eq!(by_path.format.as_deref(), Some("shop/my-profile.txt"));
        // the case of a name that is not one of the three is kept
        assert_eq!(
            parse("--format Shop job.nc").unwrap().format.as_deref(),
            Some("Shop")
        );
        assert_eq!(
            parse("--format FULL job.nc").unwrap().format.as_deref(),
            Some("full")
        );
        assert!(Options::parse([
            "--format".to_string(),
            "  ".to_string(),
            "job.nc".to_string()
        ])
        .is_err());
    }

    #[test]
    fn the_output_file_goes_with_a_tool_that_prints_and_with_nothing_else() {
        assert_eq!(
            parse("--convert in --out new.nc job.nc").unwrap().out,
            Some(PathBuf::from("new.nc"))
        );
        assert!(parse("--times --out t.txt job.nc").unwrap().out.is_some());
        assert!(parse("--templates --out t.txt").unwrap().out.is_some());
        assert!(parse("--restart 3 --out r.nc job.nc")
            .unwrap()
            .out
            .is_some());
        for bad in [
            "--out new.nc job.nc",
            "--out new.nc",
            "--report job.nc --out x.txt",
        ] {
            let why = parse(bad).unwrap_err();
            assert!(
                why.starts_with("--out only goes with a tool"),
                "{bad}: {why}"
            );
        }
        assert!(parse("--convert in --out").is_err());
        let h = help_text();
        for word in [
            "--holder",
            "--life",
            "--restart LINE",
            "--template NAME",
            "--templates",
            "--out FILE",
        ] {
            assert!(h.contains(word), "{word}");
        }
        // the help lines still start where they did
        assert!(h.lines().filter(|l| l.starts_with("  --")).count() >= 28);
    }
}
