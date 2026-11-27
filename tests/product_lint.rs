//! The warnings about a program at their exact edges: each limit on both sides, the line and the count of every finding, and a program of nonsense.

use eh3d_phoenix_cnc::lint::{lint, Finding, Kind, LintCfg};

fn l(lines: &[&str]) -> Vec<String> {
    lines.iter().map(|s| s.to_string()).collect()
}

fn run(lines: &[&str]) -> Vec<Finding> {
    lint(&l(lines), &LintCfg::default())
}

fn kinds(found: &[Finding]) -> Vec<Kind> {
    found.iter().map(|f| f.kind).collect()
}

#[test]
fn a_program_with_no_moves_says_nothing_not_even_that_it_has_no_end() {
    assert!(run(&[]).is_empty());
    assert!(run(&["M3 S1000"]).is_empty());
    assert!(run(&["(a comment)", "", "G21", "G90"]).is_empty());
}

#[test]
fn the_end_of_the_program_is_m2_or_m30_and_m5_is_not() {
    for end in ["M30", "M2", "m30"] {
        assert!(run(&["M3 S1000", "G1 X5 F100", end]).is_empty(), "{end}");
    }
    let found = run(&["M3 S1000", "G1 X5 F100", "M5"]);
    assert_eq!(kinds(&found), [Kind::NoEnd]);
    assert_eq!(found[0].line, 3);
    assert_eq!(found[0].count, 1);
    assert_eq!(found[0].limit, 0.0);
}

#[test]
fn cuts_with_the_spindle_off_are_one_finding_with_the_first_line_and_the_count() {
    let found = run(&["G1 X1 F100", "G0 X5", "G1 X2", "G1 X3", "M30"]);
    assert_eq!(kinds(&found), [Kind::CutWithoutSpindle]);
    assert_eq!((found[0].line, found[0].count), (1, 3));
}

#[test]
fn a_rapid_is_below_the_surface_only_under_the_floor() {
    let found = run(&["M3 S1000", "G0 Z-1", "G0 Z0", "G0 Z-0.5", "M30"]);
    assert_eq!(kinds(&found), [Kind::RapidDown]);
    assert_eq!((found[0].line, found[0].count), (2, 2));
    let cfg = LintCfg {
        rapid_floor_mm: -2.0,
        ..LintCfg::default()
    };
    let prog = l(&["M3 S1000", "G0 Z-1", "M30"]);
    assert!(lint(&prog, &cfg).is_empty());
}

#[test]
fn the_feed_limit_is_exclusive_and_inches_are_turned_into_millimetres() {
    let found = run(&[
        "M3 S1000",
        "G1 X1 F5000",
        "G1 X2 F5001",
        "G1 X3 F9000",
        "M30",
    ]);
    assert_eq!(kinds(&found), [Kind::FeedTooHigh]);
    assert_eq!((found[0].line, found[0].count), (3, 2));
    assert_eq!(found[0].limit, 5000.0);
    let inch = run(&["G20", "M3 S1000", "G1 X1 F200", "G1 X2 F196", "M30"]);
    assert_eq!(kinds(&inch), [Kind::FeedTooHigh]);
    assert_eq!(inch[0].line, 3);
}

#[test]
fn the_speed_limit_is_exclusive_and_reports_the_limit() {
    let found = run(&["M3 S24000", "M3 S24001", "M30"]);
    assert_eq!(kinds(&found), [Kind::SpeedTooHigh]);
    assert_eq!((found[0].line, found[0].count), (2, 1));
    assert_eq!(found[0].limit, 24_000.0);
}

#[test]
fn a_limit_of_zero_switches_that_limit_off() {
    let cfg = LintCfg {
        max_feed_mm_min: 0.0,
        max_rpm: 0.0,
        ..LintCfg::default()
    };
    let prog = l(&["M3 S999999", "G1 X1 F999999", "M30"]);
    assert!(lint(&prog, &cfg).is_empty());
}

#[test]
fn a_spindle_that_turns_the_other_way_without_stopping_is_found_once_per_change() {
    let prog = ["M3 S1000", "G1 X1 F100", "M4", "G1 X2", "M3", "M30"];
    let found = run(&prog);
    assert_eq!(kinds(&found), [Kind::SpindleReversal]);
    assert_eq!((found[0].line, found[0].count), (3, 2));
    let off = LintCfg {
        reversal: false,
        ..LintCfg::default()
    };
    assert!(lint(&l(&prog), &off).is_empty());
    assert!(run(&["M3 S1000", "M3", "M5", "M4", "G1 X1 F100", "M30"]).is_empty());
}

#[test]
fn a_feed_of_zero_or_less_is_found_and_the_switch_turns_it_off() {
    let prog = ["M3 S1000", "G1 X1 F0", "G1 X2 F-5", "M30"];
    let found = run(&prog);
    assert_eq!(kinds(&found), [Kind::FeedZero]);
    assert_eq!((found[0].line, found[0].count), (2, 2));
    let off = LintCfg {
        zero_feed: false,
        ..LintCfg::default()
    };
    assert!(lint(&l(&prog), &off).is_empty());
}

#[test]
fn a_spindle_with_no_speed_is_found_but_a_speed_of_zero_counts_as_given() {
    let found = run(&["M3", "G1 X1 F100", "M30"]);
    assert_eq!(kinds(&found), [Kind::SpindleWithoutSpeed]);
    assert!(run(&["S0", "M3", "G1 X1 F100", "M30"]).is_empty());
    assert!(run(&["M3 S500", "M5", "M3", "G1 X1 F100", "M30"]).is_empty());
}

#[test]
fn a_tool_change_is_found_at_its_line() {
    let found = run(&["M3 S1000", "T1 M6", "G1 X1 F100", "M30"]);
    assert_eq!(kinds(&found), [Kind::ToolChange]);
    assert_eq!(found[0].line, 2);
}

#[test]
fn the_feed_range_of_the_material_holds_the_cuts_in_x_or_y_and_not_a_plunge() {
    let cfg = LintCfg {
        material_feed: Some((100.0, 500.0)),
        ..LintCfg::default()
    };
    let prog = l(&[
        "M3 S1000",
        "G1 X1 F99",
        "G1 X2 F100",
        "G1 X3 F500",
        "G1 X4 F501",
        "G1 Z-1 F50",
        "M30",
    ]);
    let found = lint(&prog, &cfg);
    assert_eq!(
        kinds(&found),
        [Kind::FeedBelowMaterial, Kind::FeedAboveMaterial]
    );
    assert_eq!((found[0].line, found[0].limit), (2, 100.0));
    assert_eq!((found[1].line, found[1].limit), (5, 500.0));
}

#[test]
fn the_speed_range_of_the_material_has_exact_edges_and_a_stopped_spindle_is_fine() {
    let cfg = LintCfg {
        material_rpm: Some((8000.0, 20_000.0)),
        ..LintCfg::default()
    };
    let prog = l(&[
        "M3 S1000",
        "M3 S8000",
        "M3 S20000",
        "M3 S20001",
        "M5",
        "S0",
        "M30",
    ]);
    let found = lint(&prog, &cfg);
    assert_eq!(
        kinds(&found),
        [Kind::SpeedBelowMaterial, Kind::SpeedAboveMaterial]
    );
    assert_eq!((found[0].line, found[0].limit), (1, 8000.0));
    assert_eq!((found[1].line, found[1].limit), (4, 20_000.0));
}

#[test]
fn a_switched_off_check_gives_nothing_for_any_program() {
    let cfg = LintCfg {
        enabled: false,
        ..LintCfg::default()
    };
    let prog = l(&["G1 X1 F0", "M6", "G0 Z-9", "M3"]);
    assert!(lint(&prog, &cfg).is_empty());
    assert!(!lint(&prog, &LintCfg::default()).is_empty());
}

#[test]
fn the_default_settings_are_the_ones_the_documents_state() {
    let c = LintCfg::default();
    assert!(c.enabled && c.arcs && c.reversal && c.zero_feed);
    assert_eq!(c.rapid_floor_mm, 0.0);
    assert_eq!(c.max_feed_mm_min, 5000.0);
    assert_eq!(c.max_rpm, 24_000.0);
    assert!(c.material_feed.is_none() && c.material_rpm.is_none());
}

#[test]
fn programs_of_nonsense_from_a_fixed_seed_give_findings_that_point_inside_the_program() {
    let words = [
        "G0", "G1", "G2", "G3", "G20", "G21", "G90", "G91", "M3", "M4", "M5", "M6", "M30", "S100",
        "S0", "F0", "F300", "X1", "Y-2", "Z5", "Z-3", "I1", "J1", "R2", "(c)", "$", "G1X", "",
    ];
    let mut s: u64 = 55_555;
    let mut next = |m: usize| {
        s = (s * 1_103_515_245 + 12_345) % 2_147_483_648;
        ((s >> 8) as usize) % m
    };
    for _ in 0..60 {
        let n = next(25);
        let prog: Vec<String> = (0..n)
            .map(|_| {
                let k = next(5);
                (0..k)
                    .map(|_| words[next(words.len())])
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect();
        for f in lint(&prog, &LintCfg::default()) {
            assert!(f.count >= 1);
            assert!(
                (1..=prog.len()).contains(&f.line),
                "{f:?} of {}",
                prog.len()
            );
        }
    }
}

// ---- a line is read as the controller reads it ----

#[test]
fn a_semicolon_inside_a_comment_hides_nothing() {
    let words = eh3d_phoenix_cnc::gcode::words("G0 (a;b) X999");
    assert_eq!(words, vec![('G', 0.0), ('X', 999.0)]);
    let path = eh3d_phoenix_cnc::gcode::toolpath(&l(&["G0 (a;b) X999"]));
    assert_eq!(path.len(), 1);
    assert_eq!(path[0].to.0, 999.0);
}

#[test]
fn the_characters_a_controller_takes_out_do_not_drop_the_word() {
    let words = eh3d_phoenix_cnc::gcode::words("G0 X9!99 Y1~0 Z?5");
    assert_eq!(
        words,
        vec![('G', 0.0), ('X', 999.0), ('Y', 10.0), ('Z', 5.0)]
    );
}

#[test]
fn the_guard_sees_a_move_behind_a_comment_and_with_blanks_in_it() {
    use eh3d_phoenix_cnc::guard::{line_motion, line_moves};
    for line in [
        "(note) G0 X10",
        "M5 (x) G0 X10",
        "G 0 X10",
        "X10",
        "z-5 f100",
        "$ J=G91 X1 F100",
        "(a) $H",
        "G0(;)X1",
    ] {
        assert!(line_motion(line), "{line:?}");
    }
    for line in ["(go) M3 S1000", "M 3 S1000", "M0 3"] {
        assert!(line_moves(line), "{line:?}");
    }
    assert!(line_moves("QUAD_GANTRY_LEVEL"));
    for line in [
        "G92 X0",
        "G10 L20 P1 X0",
        "G4 P1",
        "M5",
        "$G",
        "$X",
        "(G0 X10)",
        "; X10",
        "F100",
        "M117 X marks it",
    ] {
        assert!(!line_moves(line), "{line:?}");
    }
}

#[test]
fn a_command_of_the_controller_behind_a_comment_stops_the_program() {
    use eh3d_controller::sim::SimController;
    use eh3d_phoenix_cnc::session::Session;
    for line in ["(set) $130=9999", " ( a ) ( b ) $RST=*"] {
        let mut s = Session::new(Box::new(SimController::new()));
        s.load(&format!("G21\n{line}\nG0 X1\n"));
        s.start();
        s.pump(10);
        assert!(
            s.log.iter().any(|m| m.starts_with("line 2 ")),
            "{line:?}: {:?}",
            s.log
        );
    }
}

#[test]
fn what_cannot_be_read_or_judged_is_said() {
    for (program, line) in [
        (["G0 X1", "G1 X5.5.5 F100", "M30"], 2),
        (["G0 X1", "G1 #5 X2 F100", "M30"], 2),
        (["G93", "G1 X5 F10", "M30"], 1),
        (["M3 S1000", "M0", "M30"], 2),
    ] {
        let found = run(&program);
        assert!(
            found.iter().any(|f| f.line == line),
            "{program:?}: {found:?}"
        );
    }
    assert!(run(&["M3 S1000", "G0 X1", "M5", "M0", "M30"]).is_empty());
}
