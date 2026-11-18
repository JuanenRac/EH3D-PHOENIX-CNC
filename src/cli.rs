//! The command line of the window.

use std::path::PathBuf;

use eh3d_view::texts::Lang;
use eh3d_view::theme::ThemeId;

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
    /// A window to open at the start (`--window settings|open|save|edit|about|summary|keys|check|keepout|stats`): for the pictures.
    pub window: Option<String>,
    /// How the path is seen at the start (`--view plan|3d`); without it the view the settings remember.
    pub view_3d: Option<bool>,
    /// Drives GRBL on a serial port instead of the simulator (`--grbl PORT[@BAUD]`).
    pub grbl: Option<String>,
    /// Lists the serial ports and leaves (`--ports`).
    pub ports: bool,
    /// Prints the report of the program (statistics, time, what looks wrong) and leaves (`--report FILE`); no window, nothing moves.
    pub report: bool,
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
                "--grbl" => o.grbl = Some(value("--grbl")?),
                "--ports" => o.ports = true,
                "--report" => o.report = true,
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
        if o.report && o.file.is_none() {
            return Err("--report needs a G-code file".to_string());
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
    fn what_is_wrong_is_said() {
        assert!(parse("--lang xx").is_err());
        assert!(parse("--theme pink").is_err());
        assert!(parse("--wait many").is_err());
        assert!(parse("--bogus").is_err());
        assert!(parse("--shot").is_err());
    }
}
