//! The report of a program printed by the command line (`--report FILE`): the statistics, the time of the path and everything that looks
//! wrong in it (the checks of the summary, the second checker, the keep-out boxes and the travel), with no window and no controller.
//! It builds a text and counts the problems, so a script can stop on a program that is not right. It only reads.

use crate::boxes::{arc_chords, hits, travel_excess};
use crate::gcode::toolpath;
use crate::lint::{lint, Kind as LintKind, LintCfg};
use crate::progcheck::{check, Level};
use crate::sim::{hms, Rates, Timeline};
use crate::stats::measure;
use crate::tuning::Tuning;
use crate::words::{say, Msg};

/// What the report needs to know about the machine; the command line takes it from the settings file.
#[derive(Debug, Clone, PartialEq)]
pub struct Cfg {
    pub travel: [(f64, f64); 3],
    /// Where the zero of the work is in the machine; a program read from a file does not know it, so it is the zero of the machine.
    pub offset: [f64; 3],
    pub rapid_mm_min: f64,
    pub default_feed: f64,
    pub lang: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub text: String,
    /// The things to read: findings of the checks that warn of a danger or of the summary, boxes entered and moves out of the travel.
    pub problems: usize,
}

/// One thing to read about a program, with the line it is about so that a window can show that line.
#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    /// The line of the program (from 0), or nothing for a remark about the whole program.
    pub line: Option<usize>,
    pub level: Level,
    /// It counts as a problem: the exit code of `--report` and the count of the window read it.
    pub problem: bool,
    pub text: String,
}

/// Everything that looks wrong in the program, in the order: the summary check, the second checker, the keep-out boxes, the arcs that are
/// tested by their chord only and the travel. A program with nothing to say gives an empty list.
pub fn notes(lines: &[String], tuning: &Tuning, lint_cfg: &LintCfg, cfg: &Cfg) -> Vec<Note> {
    let lang = cfg.lang.as_str();
    let path = toolpath(lines);
    let mut out: Vec<Note> = Vec::new();
    for f in lint(lines, lint_cfg) {
        out.push(Note {
            line: Some(f.line.saturating_sub(1)),
            level: if f.kind == LintKind::RapidDown {
                Level::Danger
            } else {
                Level::Warning
            },
            problem: true,
            text: f.message(lang),
        });
    }
    for f in check(lines, tuning) {
        out.push(Note {
            line: f.line,
            level: f.level,
            problem: f.level == Level::Danger,
            text: f.text(lang),
        });
    }
    for h in hits(&path, cfg.offset, &tuning.keepouts, tuning.keepout_margin) {
        out.push(Note {
            line: Some(h.line),
            level: Level::Danger,
            problem: true,
            text: say(
                lang,
                Msg::BoxHit,
                &[(h.line + 1).to_string(), h.name.clone()],
            ),
        });
    }
    let chords = if tuning.keepouts.is_empty() {
        0
    } else {
        arc_chords(&path)
    };
    if chords > 0 {
        out.push(Note {
            line: None,
            level: Level::Info,
            problem: false,
            text: say(lang, Msg::BoxArc, &[chords.to_string()]),
        });
    }
    for e in travel_excess(&path, cfg.offset, cfg.travel) {
        out.push(Note {
            line: Some(e.line),
            level: Level::Danger,
            problem: true,
            text: say(
                lang,
                Msg::TravelOut,
                &[(e.line + 1).to_string(), e.axis.to_string()],
            ),
        });
    }
    out
}

/// Reads the program and writes the report.
pub fn build(lines: &[String], tuning: &Tuning, lint_cfg: &LintCfg, cfg: &Cfg) -> Report {
    let lang = cfg.lang.as_str();
    let rates = Rates {
        rapid: cfg.rapid_mm_min,
        default_feed: cfg.default_feed,
    };
    let stats = measure(lines, rates, tuning.tool_change_s);
    let path = toolpath(lines);
    let timeline = Timeline::build(&path, rates);
    let mut out: Vec<String> = stats.lines(lang);
    out.push(say(
        lang,
        Msg::ReportMoves,
        &[path.len().to_string(), hms(timeline.total_s)],
    ));
    let found = notes(lines, tuning, lint_cfg, cfg);
    let problems = found.iter().filter(|n| n.problem).count();
    if found.is_empty() {
        out.push(format!("- {}", say(lang, Msg::ReportClean, &[] as &[&str])));
    }
    for n in found {
        out.push(format!("- {}", n.text));
    }
    let mut text = out.join("\n");
    text.push('\n');
    Report { text, problems }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(lang: &str) -> Cfg {
        Cfg {
            travel: [(0.0, 300.0), (0.0, 200.0), (-50.0, 0.0)],
            offset: [0.0; 3],
            rapid_mm_min: 6000.0,
            default_feed: 300.0,
            lang: lang.to_string(),
        }
    }

    fn prog(text: &str) -> Vec<String> {
        text.lines().map(str::to_string).collect()
    }

    const GOOD: &str = "G21 G90\nM3 S10000\nG0 X10 Y10\nG1 Z-1 F200\nG1 X20\nG0 Z0\nM5\nM30";

    #[test]
    fn a_good_program_is_clean() {
        let r = build(
            &prog(GOOD),
            &Tuning::default(),
            &LintCfg::default(),
            &cfg("en"),
        );
        assert_eq!(r.problems, 0, "{}", r.text);
        assert!(r.text.starts_with("Program statistics\n"));
        assert!(r.text.contains("moves in the path, estimated time"));
        assert!(r.text.trim_end().ends_with("- Nothing looks wrong"));
    }

    #[test]
    fn the_problems_of_every_check_are_listed_and_counted() {
        let t = Tuning {
            keepouts: vec![crate::boxes::KeepOut::parse("clamp 10 0 -10 20 20 10").unwrap()],
            ..Tuning::default()
        };
        // a rapid below the surface (lint), G10 (danger), a move through the clamp and a move past the travel
        let p = prog("G21\nM3 S1000\nG10 L2 X0\nG0 X0 Y5 Z-3\nG1 X30 F100\nG1 X400\nM30");
        let r = build(&p, &t, &LintCfg::default(), &cfg("en"));
        assert!(
            r.text.contains("rapid moves end below the surface"),
            "{}",
            r.text
        );
        assert!(r.text.contains("G10 writes offsets"));
        assert!(r.text.contains("enters the keep-out box clamp"));
        assert!(r.text.contains("leaves the travel of axis X"));
        // 1 lint + 1 danger + 2 boxes (lines 5 and 6 cross the clamp... the first move starts at X0, the second goes on from X30)
        assert!(r.problems >= 4, "{}", r.problems);
    }

    #[test]
    fn the_report_follows_the_language() {
        let r = build(
            &prog(GOOD),
            &Tuning::default(),
            &LintCfg::default(),
            &cfg("pt"),
        );
        assert!(r.text.starts_with("Estatísticas do programa\n"));
        assert!(r.text.contains("Nada de anormal"));
    }

    #[test]
    fn arcs_are_said_only_when_there_are_boxes() {
        let p = prog("G21\nM3 S1000\nG0 X0 Y0\nG2 X10 Y0 I5 J0 F100\nM30");
        let none = build(&p, &Tuning::default(), &LintCfg::default(), &cfg("en"));
        assert!(!none.text.contains("chord"));
        let t = Tuning {
            keepouts: vec![crate::boxes::KeepOut::parse("far 100 100 -5 110 110 5").unwrap()],
            ..Tuning::default()
        };
        let with = build(&p, &t, &LintCfg::default(), &cfg("en"));
        assert!(with
            .text
            .contains("1 arc moves are checked by their chord only"));
    }

    #[test]
    fn an_empty_program_still_reports() {
        let r = build(&[], &Tuning::default(), &LintCfg::default(), &cfg("en"));
        assert!(r.text.contains("0 moves in the path"));
        assert!(r.problems == 0 || r.text.contains("- "));
    }

    #[test]
    fn the_notes_carry_the_line_and_the_level_of_each_finding() {
        let t = Tuning {
            keepouts: vec![crate::boxes::KeepOut::parse("clamp 10 0 -10 20 20 10").unwrap()],
            ..Tuning::default()
        };
        let p = prog("G21\nM3 S1000\nG10 L2 X0\nG0 X0 Y5 Z-3\nG1 X30 F100\nG1 X400\nM30");
        let n = notes(&p, &t, &LintCfg::default(), &cfg("en"));
        // the rapid below the surface is on line 4 (index 3) and is a danger
        let rapid = n
            .iter()
            .find(|x| x.text.contains("below the surface"))
            .unwrap();
        assert_eq!(
            (rapid.line, rapid.level, rapid.problem),
            (Some(3), Level::Danger, true)
        );
        // G10 is on index 2
        let g10 = n.iter().find(|x| x.text.contains("G10")).unwrap();
        assert_eq!((g10.line, g10.level), (Some(2), Level::Danger));
        assert!(n
            .iter()
            .any(|x| x.line == Some(5) && x.text.contains("travel")));
        // the count of problems of the report is the count of the notes that are one
        let r = build(&p, &t, &LintCfg::default(), &cfg("en"));
        assert_eq!(r.problems, n.iter().filter(|x| x.problem).count());
    }

    #[test]
    fn a_good_program_has_no_notes_and_a_remark_about_the_whole_program_has_no_line() {
        assert!(notes(
            &prog(GOOD),
            &Tuning::default(),
            &LintCfg::default(),
            &cfg("en")
        )
        .is_empty());
        let p = prog("G21\nM3 S1000\nG0 X0 Y0\nG2 X10 Y0 I5 J0 F100\nM30");
        let t = Tuning {
            keepouts: vec![crate::boxes::KeepOut::parse("far 100 100 -5 110 110 5").unwrap()],
            ..Tuning::default()
        };
        let n = notes(&p, &t, &LintCfg::default(), &cfg("en"));
        assert_eq!(n.len(), 1);
        assert_eq!(
            (n[0].line, n[0].level, n[0].problem),
            (None, Level::Info, false)
        );
    }
}
