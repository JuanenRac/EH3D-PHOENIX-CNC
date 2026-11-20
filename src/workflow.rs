//! What the panels of the window do that has no window in it: the time of a program with and without the look-ahead and when that time is out
//! of date, the reading of a drawing with its post-processor, the rule that lets the machine go to the tool setter, and the writing of the
//! length of a tool into the table of tools. The panels (`gui`) show these and ask the operator; nothing here moves a machine.

use std::path::{Path, PathBuf};

use crate::lookahead::{plan_lines, Plan};
use crate::motioncfg::{MotionCfg, PlannerCfg, SetterCfg};
use crate::post::Post;
use crate::settings::Settings;
use crate::sim::Rates;

/// The speeds of the estimate from the settings.
pub fn rates(s: &Settings) -> Rates {
    Rates {
        rapid: s.rapid_mm_min,
        default_feed: s.feed_mm_min,
    }
}

/// The planner of the motion file; the acceleration of the settings is used for the three axes when the motion file says nothing of its own.
pub fn planner_for(s: &Settings, cfg: &MotionCfg) -> PlannerCfg {
    let mut planner = cfg.planner.clone();
    if s.tuning.accel_mm_s2 > 0.0 && planner.accel_mm_s2 == PlannerCfg::default().accel_mm_s2 {
        planner.accel_mm_s2 = [s.tuning.accel_mm_s2; 3];
    }
    planner
}

/// A number that tells a program from another one cheaply (the length and a hash of the text of every line): the time that was calculated
/// for a program is out of date when the number of the program is not the one it was calculated for.
pub fn program_key(lines: &[String]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |b: u8| {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    };
    for l in lines {
        for b in l.bytes() {
            eat(b);
        }
        eat(b'\n');
    }
    h ^ (lines.len() as u64).rotate_left(32)
}

/// The time of a program with the look-ahead of the planner next to the time of the same moves stopping at each one.
#[derive(Debug, Clone, PartialEq)]
pub struct TimeCompare {
    /// With the look-ahead, and stopping at each move, in seconds.
    pub with_s: f64,
    pub without_s: f64,
    pub blocks: usize,
    pub stops: usize,
    pub length_mm: f64,
    pub arcs_rebuilt: usize,
    /// The number of the program it was calculated for (see [`program_key`]).
    pub key: u64,
}

impl TimeCompare {
    fn from_plan(p: &Plan, key: u64) -> TimeCompare {
        TimeCompare {
            with_s: p.total_s,
            without_s: p.stop_to_stop_s,
            blocks: p.blocks.len(),
            stops: p.stops,
            length_mm: p.length_mm,
            arcs_rebuilt: p.arcs_rebuilt,
            key,
        }
    }

    /// The seconds the look-ahead saves (never below zero).
    pub fn saved_s(&self) -> f64 {
        (self.without_s - self.with_s).max(0.0)
    }

    /// The share of the time that is saved, in percent (0 when there is no time).
    pub fn saved_percent(&self) -> f64 {
        if self.without_s > 0.0 && self.without_s.is_finite() {
            (100.0 * self.saved_s() / self.without_s).clamp(0.0, 100.0)
        } else {
            0.0
        }
    }

    /// The program is not the one this was calculated for.
    pub fn is_stale(&self, lines: &[String]) -> bool {
        self.key != program_key(lines)
    }
}

/// Calculates the time of a program with and without the look-ahead.
pub fn time_compare(lines: &[String], s: &Settings, cfg: &MotionCfg) -> TimeCompare {
    let change_s = s.tuning.change_seconds(s.rapid_mm_min);
    let planner = planner_for(s, cfg);
    let p = plan_lines(lines, rates(s), &planner, change_s);
    TimeCompare::from_plan(&p, program_key(lines))
}

/// Why a post-processor could not be taken, or what it said when it was.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedPost {
    pub post: Post,
    /// What the file said that could not be used, one line each, with its number and the reason (in the language).
    pub problems: Vec<String>,
}

/// The post-processor a name or a path asks for: a file given by its path, or one beside the settings by its name.
pub fn load_post(name: &str, settings_file: &Path, lang: &str) -> Result<LoadedPost, String> {
    let name = name.trim();
    let direct = Path::new(name);
    let path: Option<PathBuf> = if !name.is_empty() && direct.is_file() {
        Some(direct.to_path_buf())
    } else {
        crate::post::post_path(settings_file, name)
    };
    match path.as_deref().and_then(crate::post::read) {
        Some((post, problems)) => Ok(LoadedPost {
            post,
            problems: problems.iter().map(|p| p.text(lang)).collect(),
        }),
        None => Err(crate::notes::Label::NoPost.text(lang).to_string()),
    }
}

/// What a board file was cut as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoardInfo {
    pub kind: crate::pcb::Kind,
    /// The paths the tool cuts along, or the holes and slots it drills.
    pub cuts: usize,
    /// The different drills (0 when it is not a drill file).
    pub tools: usize,
}

/// A drawing or a board layer read and turned into a program.
#[derive(Debug, Clone, PartialEq)]
pub struct Imported {
    /// The G-code that cuts along the paths (moved to the zero of the work, and written by the post-processor when there is one).
    pub lines: Vec<String>,
    pub paths: usize,
    /// The size of the drawing in mm.
    pub size_mm: (f64, f64),
    /// What the reader did not understand or assumed, and what the post-processor had to say (a script that is not run), in the language.
    pub notes: Vec<String>,
    /// What the file was cut as, when it is a board layer (Gerber or Excellon).
    pub board: Option<BoardInfo>,
}

/// Reads a drawing (HPGL, DXF or SVG) or a board layer (Gerber or Excellon) within the limits of the motion file and turns it into the program
/// that cuts along it. The post-processor, when one is given, writes the lines last; a script it asks for is not run unless `post_scripts` is on
/// (and there is no engine yet).
pub fn import_file(
    path: &Path,
    cfg: &MotionCfg,
    post: Option<&LoadedPost>,
    lang: &str,
) -> Result<Imported, String> {
    import_file_with(path, cfg, post, lang, &crate::pcb::Ask::default())
}

/// As [`import_file`], asked which layer a board file is cut as and whether it is mirrored.
pub fn import_file_with(
    path: &Path,
    cfg: &MotionCfg,
    post: Option<&LoadedPost>,
    lang: &str,
    ask: &crate::pcb::Ask,
) -> Result<Imported, String> {
    if crate::pcb::detect_path(path).is_some() {
        let job = crate::pcb::import_file(path, &cfg.import, &cfg.pcb, ask)
            .map_err(|e| e.text(lang).to_string())?;
        let mut notes: Vec<String> = job.notes.iter().map(|n| n.text(lang).to_string()).collect();
        let mut lines = job.lines;
        if let Some(p) = post {
            notes.extend(p.problems.iter().cloned());
            if let Err(label) = p.post.script_state(cfg.import.post_scripts) {
                notes.push(label.text(lang).to_string());
            }
            lines = p.post.apply(&lines);
        }
        return Ok(Imported {
            lines,
            paths: job.cuts,
            size_mm: job.size_mm,
            notes,
            board: Some(BoardInfo {
                kind: job.kind,
                cuts: job.cuts,
                tools: job.tools,
            }),
        });
    }
    let mut drawing =
        crate::import::read_file(path, &cfg.import).map_err(|e| e.text(lang).to_string())?;
    let size_mm =
        crate::import::bounds(&drawing).map_or((0.0, 0.0), |(x0, y0, x1, y1)| (x1 - x0, y1 - y0));
    crate::import::to_origin(&mut drawing);
    let mut lines = crate::import::to_gcode(&drawing, &cfg.import);
    let mut notes: Vec<String> = drawing
        .notes
        .iter()
        .map(|n| n.text(lang).to_string())
        .collect();
    if let Some(p) = post {
        notes.extend(p.problems.iter().cloned());
        if let Err(label) = p.post.script_state(cfg.import.post_scripts) {
            notes.push(label.text(lang).to_string());
        }
        lines = p.post.apply(&lines);
    }
    Ok(Imported {
        lines,
        paths: drawing.paths.len(),
        size_mm,
        notes,
        board: None,
    })
}

/// Whether the machine may be sent to the tool setter now: the machine is free, it has a probe input, the numbers of the setter are sound, and
/// the operator confirmed that the setter is there and clear when the motion file asks for the confirmation (it does by default).
pub fn can_measure(cfg: &SetterCfg, confirmed: bool, idle: bool, has_probe: bool) -> bool {
    idle && has_probe && crate::toolsetter::check(cfg).is_ok() && (confirmed || !cfg.confirm)
}

/// Why the length of a tool could not be written into the table.
#[derive(Debug, Clone, PartialEq)]
pub enum WriteError {
    /// There is no table of tools.
    NoTable,
    /// The table has no tool with that number.
    NoTool,
    /// The table could not be read or written, or the length is not one the table takes.
    Other(String),
}

/// Writes the length of tool `number` into the table of tools beside the settings: only the `length=` part of its line changes. The table is
/// written to a file of its own and put in place, so a failure in the middle never leaves half a table.
pub fn write_tool_length(
    settings_file: &Path,
    number: u32,
    length_mm: f64,
) -> Result<(), WriteError> {
    let path = crate::tools::table_path(settings_file);
    let bytes = std::fs::read(&path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => WriteError::NoTable,
        _ => WriteError::Other(format!("{}: {e}", path.display())),
    })?;
    let text = String::from_utf8_lossy(&bytes).to_string();
    if crate::tools::ToolTable::parse(&text).find(number).is_none() {
        return Err(WriteError::NoTool);
    }
    let new =
        crate::toolsetter::with_length(&text, number, length_mm).map_err(WriteError::Other)?;
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, new).map_err(|e| WriteError::Other(format!("{}: {e}", tmp.display())))?;
    std::fs::rename(&tmp, &path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        WriteError::Other(format!("{}: {e}", path.display()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(t: &str) -> Vec<String> {
        t.lines().map(str::to_string).collect()
    }

    /// A folder of its own for each test (the tests run side by side).
    fn dir(tag: &str) -> PathBuf {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let d = std::env::temp_dir().join(format!("cnc_workflow_{tag}_{}_{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    struct Seed(u64);
    impl Seed {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
    }

    const SQUARE_SVG: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"30mm\" height=\"20mm\" viewBox=\"0 0 30 20\"><rect x=\"5\" y=\"5\" width=\"20\" height=\"10\"/></svg>";

    #[test]
    fn the_key_of_a_program_changes_with_any_line_and_with_the_order() {
        let a = lines("G0 X1\nG1 X2 F100\n");
        assert_eq!(program_key(&a), program_key(&a.clone()));
        assert_ne!(program_key(&a), program_key(&lines("G0 X1\nG1 X3 F100\n")));
        assert_ne!(program_key(&a), program_key(&lines("G1 X2 F100\nG0 X1\n")));
        assert_ne!(
            program_key(&a),
            program_key(&lines("G0 X1\nG1 X2 F100\n\n"))
        );
        assert_ne!(program_key(&[]), program_key(&[String::new()]));
    }

    #[test]
    fn the_look_ahead_never_takes_longer_than_stopping_at_every_move_and_the_numbers_are_sound() {
        let s = Settings::default();
        let cfg = MotionCfg::default();
        // a program with many short corners: the look-ahead saves time
        let mut prog = vec!["G21 G90".to_string(), "G1 F1200".to_string()];
        for i in 0..40 {
            prog.push(format!("G1 X{} Y{}", i * 2, (i % 2) * 2));
        }
        let t = time_compare(&prog, &s, &cfg);
        assert!(t.with_s > 0.0 && t.without_s > 0.0, "{t:?}");
        assert!(t.with_s <= t.without_s + 1e-6, "{t:?}");
        assert!(t.saved_s() >= 0.0 && (0.0..=100.0).contains(&t.saved_percent()));
        assert!(!t.is_stale(&prog));
        let mut changed = prog.clone();
        changed.push("G1 X0".into());
        assert!(t.is_stale(&changed));
        // an empty program has no time to compare
        let e = time_compare(&[], &s, &cfg);
        assert_eq!((e.with_s, e.without_s, e.saved_percent()), (0.0, 0.0, 0.0));
        // programs made from a fixed seed: the numbers are finite, the percent is in range, and the look-ahead does not lose time
        let mut rng = Seed(0xD0C5_0000_0000_0007);
        for round in 0..60 {
            let mut p = vec!["G21 G90".to_string()];
            for _ in 0..(1 + rng.below(30)) {
                let code = ["G0", "G1"][rng.below(2) as usize];
                p.push(format!(
                    "{code} X{} Y{} Z{} F{}",
                    rng.below(200),
                    rng.below(200),
                    rng.below(10),
                    100 + rng.below(1500)
                ));
            }
            let t = time_compare(&p, &s, &cfg);
            assert!(
                t.with_s.is_finite() && t.without_s.is_finite(),
                "round {round}: {t:?}"
            );
            assert!(t.with_s <= t.without_s + 1e-6, "round {round}: {t:?}");
            assert!((0.0..=100.0).contains(&t.saved_percent()), "round {round}");
        }
    }

    #[test]
    fn the_acceleration_of_the_settings_is_used_only_when_the_motion_file_has_none_of_its_own() {
        let mut s = Settings::default();
        s.tuning.accel_mm_s2 = 120.0;
        let plain = MotionCfg::default();
        assert_eq!(planner_for(&s, &plain).accel_mm_s2, [120.0; 3]);
        let mut own = MotionCfg::default();
        own.planner.accel_mm_s2 = [10.0, 20.0, 30.0];
        assert_eq!(planner_for(&s, &own).accel_mm_s2, [10.0, 20.0, 30.0]);
        // and the rest of the planner is the one of the file
        own.planner.lookahead = 7;
        assert_eq!(planner_for(&s, &own).lookahead, 7);
    }

    #[test]
    fn a_drawing_is_read_moved_to_the_zero_and_cut_pass_by_pass_and_the_post_processor_writes_it() {
        let d = dir("import");
        let svg = d.join("part.svg");
        std::fs::write(&svg, SQUARE_SVG).unwrap();
        let cfg = MotionCfg::default();
        let got = import_file(&svg, &cfg, None, "en").unwrap();
        assert_eq!(got.paths, 1);
        assert!(
            (got.size_mm.0 - 20.0).abs() < 0.01 && (got.size_mm.1 - 10.0).abs() < 0.01,
            "{:?}",
            got.size_mm
        );
        assert!(got.lines[0].starts_with("G21"));
        // moved to the zero: no cut goes below zero in X or Y
        for l in &got.lines {
            for w in l.split_whitespace() {
                if let Some(v) = w.strip_prefix('X').or_else(|| w.strip_prefix('Y')) {
                    if let Ok(n) = v.parse::<f64>() {
                        assert!(n >= -0.0005, "{l}");
                    }
                }
            }
        }
        // two passes of 0.5 mm down to 1 mm
        let plunges = got.lines.iter().filter(|l| l.starts_with("G1 Z-")).count();
        assert_eq!(plunges, 2, "{:?}", got.lines);
        // a post-processor beside the settings writes the lines: a header, the words of its rules
        let settings = d.join("phoenix-cnc.cfg");
        std::fs::write(
            crate::post::post_path(&settings, "shop").unwrap(),
            "header = (job for the shop)\nreplace = G0 -> G00\n",
        )
        .unwrap();
        let post = load_post("shop", &settings, "en").unwrap();
        let with = import_file(&svg, &cfg, Some(&post), "en").unwrap();
        assert_eq!(with.lines[0], "(job for the shop)");
        assert!(
            with.lines.iter().any(|l| l.starts_with("G00")),
            "{:?}",
            with.lines
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_drawing_that_is_wrong_is_refused_with_the_reason_in_the_language_and_never_a_panic() {
        let d = dir("wrong");
        let cfg = MotionCfg::default();
        // a file that is not there, one that is not a drawing, one over the limit
        assert!(import_file(&d.join("none.svg"), &cfg, None, "en").is_err());
        let junk = d.join("junk.bin");
        std::fs::write(&junk, [0u8, 1, 2, 3, 255, 254]).unwrap();
        let e = import_file(&junk, &cfg, None, "es").unwrap_err();
        assert!(!e.is_empty());
        let big = d.join("big.svg");
        std::fs::write(&big, SQUARE_SVG).unwrap();
        let mut small = MotionCfg::default();
        small.import.max_bytes = 16;
        assert!(import_file(&big, &small, None, "en").is_err());
        // bytes made from a fixed seed, with the name of each format: an answer every time and no panic
        let mut rng = Seed(0xFEED_0000_0000_0003);
        for (n, name) in ["a.svg", "a.dxf", "a.plt", "a.hpgl", "a.txt"]
            .iter()
            .cycle()
            .take(100)
            .enumerate()
        {
            let len = 1 + rng.below(300) as usize;
            let bytes: Vec<u8> = (0..len).map(|_| rng.below(256) as u8).collect();
            let f = d.join(format!("{n}_{name}"));
            std::fs::write(&f, bytes).unwrap();
            let _ = import_file(&f, &cfg, None, "en");
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_post_processor_is_found_by_name_or_by_path_and_one_that_is_not_there_is_said() {
        let d = dir("post");
        let settings = d.join("phoenix-cnc.cfg");
        let path = d.join("mine.txt");
        std::fs::write(&path, "case = upper\nnothing = 1\n").unwrap();
        let by_path = load_post(&path.to_string_lossy(), &settings, "en").unwrap();
        assert!(by_path.post.uppercase);
        assert_eq!(
            by_path.problems.len(),
            1,
            "the unknown key is told: {:?}",
            by_path.problems
        );
        std::fs::write(
            crate::post::post_path(&settings, "named").unwrap(),
            "case = upper\n",
        )
        .unwrap();
        assert!(load_post("named", &settings, "en").is_ok());
        // a name that would leave the folder, an empty name and a name that is not there
        assert!(load_post("../x", &settings, "en").is_err());
        assert!(load_post("", &settings, "en").is_err());
        let e = load_post("ghost", &settings, "pt").unwrap_err();
        assert_eq!(e, crate::notes::Label::NoPost.text("pt"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_post_that_asks_for_a_script_says_it_is_not_run() {
        let d = dir("script");
        let svg = d.join("p.svg");
        std::fs::write(&svg, SQUARE_SVG).unwrap();
        let settings = d.join("phoenix-cnc.cfg");
        std::fs::write(
            crate::post::post_path(&settings, "lua").unwrap(),
            "script = tidy.lua\n",
        )
        .unwrap();
        let post = load_post("lua", &settings, "en").unwrap();
        let cfg = MotionCfg::default();
        assert!(!cfg.import.post_scripts, "scripts start off");
        let got = import_file(&svg, &cfg, Some(&post), "en").unwrap();
        assert!(!got.notes.is_empty(), "the script that is not run is said");
        assert!(got.notes.iter().all(|n| !n.is_empty()));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_machine_goes_to_the_setter_only_when_it_is_free_has_a_probe_and_the_operator_confirmed()
    {
        let cfg = SetterCfg::default();
        assert!(cfg.confirm, "the confirmation starts on");
        assert!(can_measure(&cfg, true, true, true));
        assert!(!can_measure(&cfg, false, true, true), "no confirmation");
        assert!(!can_measure(&cfg, true, false, true), "busy");
        assert!(!can_measure(&cfg, true, true, false), "no probe");
        let bad = SetterCfg {
            max_travel_mm: 0.0,
            ..SetterCfg::default()
        };
        assert!(!can_measure(&bad, true, true, true), "unsound numbers");
        // with the confirmation turned off in the motion file the other rules still hold
        let free = SetterCfg {
            confirm: false,
            ..SetterCfg::default()
        };
        assert!(can_measure(&free, false, true, true));
        assert!(!can_measure(&free, false, false, true));
        assert!(!can_measure(&free, false, true, false));
    }

    #[test]
    fn the_length_goes_into_the_table_and_only_into_the_line_of_that_tool() {
        let d = dir("table");
        let settings = d.join("phoenix-cnc.cfg");
        let table = crate::tools::table_path(&settings);
        // no table yet
        assert_eq!(
            write_tool_length(&settings, 1, 20.0),
            Err(WriteError::NoTable)
        );
        let before = "# my tools\ntool = 1 | name=End mill 3 mm | diameter=3 \ntool = 2 | name=Drill | diameter=1 | length=10 \n; end\n";
        std::fs::write(&table, before).unwrap();
        assert_eq!(
            write_tool_length(&settings, 9, 20.0),
            Err(WriteError::NoTool)
        );
        assert_eq!(
            std::fs::read_to_string(&table).unwrap(),
            before,
            "a refusal writes nothing"
        );
        write_tool_length(&settings, 1, 21.5).unwrap();
        let after = std::fs::read_to_string(&table).unwrap();
        assert!(after.contains("length=21.5"), "{after}");
        assert!(
            after.contains("tool = 2 | name=Drill | diameter=1 | length=10 "),
            "{after}"
        );
        assert!(after.starts_with("# my tools\n") && after.ends_with("; end\n"));
        // a length that is not a number the table takes is refused and the table stays
        let kept = std::fs::read_to_string(&table).unwrap();
        assert!(matches!(
            write_tool_length(&settings, 1, f64::NAN),
            Err(WriteError::Other(_))
        ));
        assert_eq!(std::fs::read_to_string(&table).unwrap(), kept);
        assert!(
            !table.with_extension("tmp").exists(),
            "no half table is left"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    const EDGE: &str = "%FSLAX34Y34*%\n%MOMM*%\n%TF.FileFunction,Profile,NP*%\n%ADD10C,0.100*%\nD10*\nX0Y0D02*\nX400000Y0D01*\nX400000Y300000D01*\nX0Y300000D01*\nX0Y0D01*\nM02*\n";
    const HOLES: &str =
        "M48\nMETRIC,TZ\nT1C0.800\nT2C1.000\n%\nT1\nX5.0Y5.0\nX10.0Y5.0\nT2\nX20.0Y10.0\nM30\n";

    #[test]
    fn a_board_layer_is_read_into_a_program_with_what_it_was_cut_as_and_a_drawing_is_not_a_board() {
        use crate::pcb::{Ask, Kind};
        let d = dir("board");
        let cfg = MotionCfg::default();
        let edge = d.join("edge.gko");
        std::fs::write(&edge, EDGE).unwrap();
        let got = import_file(&edge, &cfg, None, "en").unwrap();
        let b = got.board.unwrap();
        assert_eq!((b.kind, b.cuts, b.tools), (Kind::Profile, 1, 0));
        assert!(got.lines.iter().any(|l| l.starts_with("G1 Z-")));
        assert!((got.size_mm.0 - 41.0).abs() < 0.1, "{:?}", got.size_mm);
        // asked for something else, and mirrored: the words are those of the language
        let holes = d.join("holes.drl");
        std::fs::write(&holes, HOLES).unwrap();
        let ask = Ask {
            kind: Some(Kind::Drill),
            mirror: true,
        };
        let got = import_file_with(&holes, &cfg, None, "es", &ask).unwrap();
        let b = got.board.unwrap();
        assert_eq!((b.kind, b.cuts, b.tools), (Kind::Drill, 3, 2));
        assert!(
            got.notes.iter().any(|n| n.contains("reflej")),
            "{:?}",
            got.notes
        );
        assert!(
            got.lines.iter().any(|l| l.starts_with("G0 X-5 Y5")),
            "{:?}",
            got.lines
        );
        // a post-processor writes the lines of a board last, as it does for a drawing
        let settings = d.join("phoenix-cnc.cfg");
        std::fs::write(
            crate::post::post_path(&settings, "shop").unwrap(),
            "header = (job for the shop)\n",
        )
        .unwrap();
        let post = load_post("shop", &settings, "en").unwrap();
        let with = import_file(&holes, &cfg, Some(&post), "en").unwrap();
        assert_eq!(with.lines[0], "(job for the shop)");
        // a drawing is not a board
        let svg = d.join("a.svg");
        std::fs::write(&svg, SQUARE_SVG).unwrap();
        assert_eq!(import_file(&svg, &cfg, None, "en").unwrap().board, None);
        // the limits of the motion file hold for a board, and a layer that is not cut is refused in words
        let mut small = MotionCfg::default();
        small.import.max_bytes = 32;
        assert!(import_file(&edge, &small, None, "en").is_err());
        let mask = d.join("mask.gts");
        std::fs::write(&mask, EDGE.replace("Profile,NP", "Soldermask,Top")).unwrap();
        let e = import_file(&mask, &cfg, None, "pt").unwrap_err();
        assert!(e.contains("não é uma camada"), "{e}");
        let _ = std::fs::remove_dir_all(&d);
    }
}
