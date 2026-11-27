//! The reading of G-code and the programs built on it at their exact edges: the words of a line, the modal state, starting a program from a line, the change of unit,
//! the work coordinate systems, the cutter compensation, the canned cycles, the formats of the controllers, the settings ranges and the touch-off of the probe.
//! Nothing here opens a window or talks to a machine; the files it writes are in a folder of its own that is removed at the end.

use eh3d_phoenix_cnc::cutcomp::{offset_chain, offset_length, Offset, Side, MITRE_LIMIT};
use eh3d_phoenix_cnc::cycles::{expand, expand_with, has_cycles, MAX_PECKS, PECK_CLEARANCE_MM};
use eh3d_phoenix_cnc::gcode::{words, Modal};
use eh3d_phoenix_cnc::postcheck::{
    check, profile_path, resolve, Found, Profile, MAX_PROFILE_BYTES, NAMES,
};
use eh3d_phoenix_cnc::probe::{z_touch_off_lines, ProbeCfg};
use eh3d_phoenix_cnc::progcheck::{Finding, Level};
use eh3d_phoenix_cnc::ranges::{range_of, RANGES};
use eh3d_phoenix_cnc::restart::{plan, Why};
use eh3d_phoenix_cnc::startline::{self, Note, Refusal};
use eh3d_phoenix_cnc::units::{convert, MM_PER_INCH};
use eh3d_phoenix_cnc::wcs;
use eh3d_phoenix_cnc::words::Msg;

/// A small seeded generator, so every run sees the same numbers.
fn lcg(seed: u32, n: usize, alphabet: u32) -> Vec<u8> {
    let mut s = seed;
    (0..n)
        .map(|_| {
            s = s.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            ((s >> 16) % alphabet) as u8
        })
        .collect()
}

fn lines(text: &str) -> Vec<String> {
    text.lines().map(str::to_string).collect()
}

fn scratch(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("eh3d-cnc-r5-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn msgs(found: &[Finding]) -> Vec<(Option<usize>, Msg)> {
    found.iter().map(|f| (f.line, f.msg)).collect()
}

// ---------------------------------------------------------------- the words of a line

#[test]
fn a_line_with_nothing_in_it_has_no_words() {
    assert!(words("").is_empty());
    assert!(words("   \t ").is_empty());
    assert!(words("; only a comment").is_empty());
    assert!(words("(only a comment)").is_empty());
}

#[test]
fn the_words_are_a_letter_and_a_number_in_the_order_they_come() {
    assert_eq!(
        words("G1 X10 Y-2.5 F300"),
        [('G', 1.0), ('X', 10.0), ('Y', -2.5), ('F', 300.0)]
    );
}

#[test]
fn the_letters_are_read_in_upper_case_and_the_spaces_do_not_matter() {
    assert_eq!(words("g1x5y6"), [('G', 1.0), ('X', 5.0), ('Y', 6.0)]);
    assert_eq!(words("G 1  X 5"), [('G', 1.0), ('X', 5.0)]);
}

#[test]
fn spaces_inside_a_number_are_taken_out() {
    assert_eq!(words("X 1 0"), [('X', 10.0)]);
}

#[test]
fn the_forms_of_a_number_that_a_controller_takes_are_read() {
    assert_eq!(words("X+5 Y.5 Z5."), [('X', 5.0), ('Y', 0.5), ('Z', 5.0)]);
    assert_eq!(words("X-0"), [('X', 0.0)]);
}

#[test]
fn a_letter_with_no_number_and_a_number_that_does_not_read_are_left_out() {
    assert_eq!(words("G X5"), [('X', 5.0)]);
    assert_eq!(words("X1.2.3 Y4"), [('Y', 4.0)]);
    assert_eq!(words("X- Y4"), [('Y', 4.0)]);
}

#[test]
fn a_comment_after_a_semicolon_is_not_read() {
    assert_eq!(words("X1 ; Y2 Z3"), [('X', 1.0)]);
}

#[test]
fn a_comment_in_parentheses_is_not_read_and_the_line_goes_on_after_it() {
    assert_eq!(words("X1 (Y2) Z3"), [('X', 1.0), ('Z', 3.0)]);
    assert_eq!(words("(a) (b) X1"), [('X', 1.0)]);
}

#[test]
fn a_comment_in_parentheses_that_is_not_closed_takes_the_rest_of_the_line() {
    assert_eq!(words("X1 (Y2 Z3"), [('X', 1.0)]);
}

#[test]
fn the_same_letter_twice_is_kept_twice_in_order() {
    assert_eq!(words("X1 X2"), [('X', 1.0), ('X', 2.0)]);
}

#[test]
fn a_very_long_line_is_read_without_trouble() {
    let long = "X1 ".repeat(5_000);
    assert_eq!(words(&long).len(), 5_000);
}

#[test]
fn any_line_made_of_a_small_alphabet_is_read_without_a_panic() {
    let alphabet = b"GXYZ01 .-+;()\t";
    for seed in 1..300u32 {
        let raw = lcg(seed, (seed as usize * 3) % 70, alphabet.len() as u32);
        let text: String = raw.iter().map(|b| alphabet[*b as usize] as char).collect();
        let _ = words(&text);
    }
}

// ---------------------------------------------------------------- the modal state

#[test]
fn a_new_state_is_the_one_a_controller_starts_in() {
    let m = Modal::default();
    assert_eq!((m.motion, m.inches, m.absolute), (0, false, true));
    assert_eq!((m.spindle, m.coolant, m.wcs, m.plane), (5, 9, 54, 17));
    assert_eq!((m.feed, m.speed, m.tool_length), (None, None, None));
}

#[test]
fn the_motion_the_units_and_the_distances_follow_their_codes() {
    let mut m = Modal::default();
    m.apply("G1");
    assert_eq!(m.motion, 1);
    m.apply("G00");
    assert_eq!(m.motion, 0);
    m.apply("G20 G91");
    assert!(m.inches && !m.absolute);
    m.apply("G21 G90");
    assert!(!m.inches && m.absolute);
}

#[test]
fn a_code_with_a_decimal_is_not_the_code_without_it() {
    let mut m = Modal::default();
    m.apply("G91.1");
    assert!(
        m.absolute,
        "G91.1 is the way the arcs are measured, not relative distances"
    );
    m.apply("G59.1");
    assert_eq!(m.wcs, 54);
    m.apply("G17.1");
    assert_eq!(m.plane, 17);
    m.apply("G53");
    assert_eq!(m.wcs, 54);
}

#[test]
fn the_work_systems_and_the_planes_follow_their_codes() {
    let mut m = Modal::default();
    for (code, want) in [
        ("G55", 55),
        ("G56", 56),
        ("G57", 57),
        ("G58", 58),
        ("G59", 59),
        ("G54", 54),
    ] {
        m.apply(code);
        assert_eq!(m.wcs, want, "{code}");
    }
    for (code, want) in [("G18", 18), ("G19", 19), ("G17", 17)] {
        m.apply(code);
        assert_eq!(m.plane, want, "{code}");
    }
}

#[test]
fn the_tool_length_offset_takes_the_z_of_its_line_and_goes_with_g49() {
    let mut m = Modal::default();
    m.apply("G43.1 Z-5");
    assert_eq!(m.tool_length, Some(-5.0));
    m.apply("G43.1");
    assert_eq!(m.tool_length, Some(0.0));
    m.apply("G49");
    assert_eq!(m.tool_length, None);
}

#[test]
fn a_feed_of_zero_or_less_is_not_taken_and_the_last_good_one_stays() {
    let mut m = Modal::default();
    m.apply("F200");
    assert_eq!(m.feed, Some(200.0));
    m.apply("F0");
    m.apply("F-5");
    assert_eq!(m.feed, Some(200.0));
}

#[test]
fn a_speed_of_zero_is_a_speed_and_a_negative_one_is_not() {
    let mut m = Modal::default();
    m.apply("S0");
    assert_eq!(m.speed, Some(0.0));
    m.apply("S12000");
    m.apply("S-1");
    assert_eq!(m.speed, Some(12_000.0));
}

#[test]
fn the_spindle_and_the_coolant_follow_their_codes_and_the_end_of_the_program_stops_both() {
    let mut m = Modal::default();
    m.apply("M3 M8");
    assert_eq!((m.spindle, m.coolant), (3, 8));
    m.apply("M4 M7");
    assert_eq!((m.spindle, m.coolant), (4, 7));
    m.apply("M6");
    assert_eq!((m.spindle, m.coolant), (4, 7));
    m.apply("M5 M9");
    assert_eq!((m.spindle, m.coolant), (5, 9));
    m.apply("M3 M8");
    m.apply("M30");
    assert_eq!((m.spindle, m.coolant), (5, 9));
    m.apply("M3 M8");
    m.apply("M2");
    assert_eq!((m.spindle, m.coolant), (5, 9));
}

#[test]
fn the_codes_are_read_in_lower_case_and_comments_change_nothing() {
    let mut m = Modal::default();
    m.apply("g1 m3 s100 f50");
    assert_eq!(
        (m.motion, m.spindle, m.speed, m.feed),
        (1, 3, Some(100.0), Some(50.0))
    );
    let mut c = Modal::default();
    c.apply("(G1 M3) ; G91");
    assert_eq!(c, Modal::default());
}

#[test]
fn several_codes_on_a_line_all_count() {
    let mut m = Modal::default();
    m.apply("G1 G20 G91 G55 G18 F10 S500 M3 M8");
    assert_eq!(
        (m.motion, m.inches, m.absolute, m.wcs, m.plane),
        (1, true, false, 55, 18)
    );
    assert_eq!(
        (m.feed, m.speed, m.spindle, m.coolant),
        (Some(10.0), Some(500.0), 3, 8)
    );
}

#[test]
fn the_list_of_codes_in_force_has_the_order_of_the_panel() {
    let m = Modal::default();
    assert_eq!(
        m.g_codes(),
        ["G0", "G17", "G21", "G40", "G49", "G54", "G64", "G80", "G90", "G94", "G97", "G99"]
    );
    assert_eq!(m.m_codes(), ["M5", "M9", "M48"]);
    let mut n = Modal::default();
    n.apply("G1 G20 G91 G55 G18 G43.1 Z1 M3 M8");
    assert_eq!(
        n.g_codes(),
        ["G1", "G18", "G20", "G40", "G43.1", "G55", "G64", "G80", "G91", "G94", "G97", "G99"]
    );
    assert_eq!(n.m_codes(), ["M3", "M8", "M48"]);
}

// ---------------------------------------------------------------- starting a program from a line

#[test]
fn a_program_with_no_lines_cannot_be_started() {
    let c = startline::check(&[], 0);
    assert_eq!((c.line, c.text.as_str()), (0, ""));
    assert_eq!(c.refusal, Some(Refusal::Empty));
    assert!(c.notes.is_empty() && c.at.is_none());
}

#[test]
fn a_line_past_the_end_is_the_last_line() {
    let p = lines("G0 X1\nG0 X2");
    let c = startline::check(&p, 99);
    assert_eq!((c.line, c.text.as_str()), (2, "G0 X2"));
    assert_eq!(c.at.map(|a| a.0), Some(1.0));
    let huge = startline::check(&p, usize::MAX);
    assert_eq!(huge.line, 2);
}

#[test]
fn relative_distances_in_force_refuse_the_start() {
    let p = lines("G91\nG0 X1\nM30");
    assert_eq!(startline::check(&p, 2).refusal, Some(Refusal::Relative));
    assert_eq!(startline::check(&p, 0).refusal, None);
    let back = lines("G91\nG90\nG0 X1\nM30");
    assert_eq!(startline::check(&back, 3).refusal, None);
}

#[test]
fn a_canned_cycle_that_is_on_refuses_the_start_with_the_line_that_began_it() {
    let p = lines("G81 X1 Y1 Z-1 R1\nG0 X2");
    assert_eq!(startline::check(&p, 1).refusal, Some(Refusal::Cycle(1)));
    let closed = lines("G81 X1 Y1 Z-1 R1\nG80\nG0 X2");
    assert_eq!(startline::check(&closed, 2).refusal, None);
    let later = lines("G0 X0\nG0 X1\nG83 X1 Z-1 R1 Q1\nG0 X5");
    assert_eq!(startline::check(&later, 3).refusal, Some(Refusal::Cycle(3)));
}

#[test]
fn a_cutter_compensation_that_is_on_refuses_the_start() {
    let left = lines("G41 D1\nG1 X1 F100");
    assert_eq!(
        startline::check(&left, 1).refusal,
        Some(Refusal::Compensation(1))
    );
    let right = lines("G0 X0\nG42 D1\nG1 X1 F100");
    assert_eq!(
        startline::check(&right, 2).refusal,
        Some(Refusal::Compensation(2))
    );
    let off = lines("G41 D1\nG1 X1 F100\nG40\nG0 X5");
    assert_eq!(startline::check(&off, 3).refusal, None);
}

#[test]
fn the_tool_the_spindle_and_the_coolant_in_force_are_told() {
    let p = lines("T3 M6\nM3 S1000\nM8\nG0 X1");
    let c = startline::check(&p, 3);
    assert_eq!(
        c.notes,
        [
            Note::Tool(3),
            Note::Spindle {
                code: 3,
                rpm: Some(1000.0)
            },
            Note::Coolant(8)
        ]
    );
    let stopped = lines("M3 S1000\nM8\nM5\nM9\nG0 X1");
    assert!(startline::check(&stopped, 4).notes.is_empty());
}

#[test]
fn a_spindle_with_no_speed_is_told_without_one() {
    let p = lines("M4\nG0 X1");
    let c = startline::check(&p, 1);
    assert_eq!(c.notes, [Note::Spindle { code: 4, rpm: None }]);
}

#[test]
fn a_tool_with_a_negative_number_is_not_a_tool() {
    let p = lines("T-1\nG0 X1");
    assert!(startline::check(&p, 1).notes.is_empty());
}

#[test]
fn the_units_the_plane_the_system_and_the_length_offset_are_told() {
    let p = lines("G20 G18 G55 G43.1 Z2\nG0 X1");
    let c = startline::check(&p, 1);
    assert_eq!(
        c.notes,
        [
            Note::Inches,
            Note::Plane(18),
            Note::Wcs(55),
            Note::ToolLength
        ]
    );
}

#[test]
fn a_shifted_zero_is_told_with_its_line_and_g92_1_takes_it_back() {
    let shifted = lines("G0 X0\nG92 X0\nG0 X5");
    assert_eq!(startline::check(&shifted, 2).notes, [Note::Offset(2)]);
    let g10 = lines("G10 L2 P1 X1\nG0 X5");
    assert_eq!(startline::check(&g10, 1).notes, [Note::Offset(1)]);
    let g923 = lines("G92.3\nG0 X5");
    assert_eq!(startline::check(&g923, 1).notes, [Note::Offset(1)]);
    let undone = lines("G92 X0\nG92.1\nG0 X5");
    assert!(startline::check(&undone, 2).notes.is_empty());
    let undone2 = lines("G92 X0\nG92.2\nG0 X5");
    assert!(startline::check(&undone2, 2).notes.is_empty());
}

#[test]
fn a_cut_before_any_feed_is_told_and_a_feed_before_the_cut_is_not() {
    let nofeed = lines("G0 X1\nG1 X5");
    assert!(startline::check(&nofeed, 1).notes.contains(&Note::NoFeed));
    let fed = lines("G0 X1\nG1 F200 X5");
    assert!(!startline::check(&fed, 1).notes.contains(&Note::NoFeed));
    let before = lines("F100\nG0 X1\nG1 X5");
    assert!(!startline::check(&before, 1).notes.contains(&Note::NoFeed));
    let arc = lines("G0 X1\nG2 X5 Y5 I1 J1");
    assert!(startline::check(&arc, 1).notes.contains(&Note::NoFeed));
    let rapids = lines("G0 X1\nG0 X5");
    assert!(!startline::check(&rapids, 1).notes.contains(&Note::NoFeed));
}

#[test]
fn every_note_comes_in_the_same_order() {
    let p = lines("G20 G18 G55 G43.1 Z2 T4 M3 S900 M8 G92 X0\nG1 X5");
    let c = startline::check(&p, 1);
    assert_eq!(
        c.notes,
        [
            Note::Inches,
            Note::Plane(18),
            Note::Wcs(55),
            Note::ToolLength,
            Note::Tool(4),
            Note::Spindle {
                code: 3,
                rpm: Some(900.0)
            },
            Note::Coolant(8),
            Note::NoFeed,
            Note::Offset(1)
        ]
    );
}

#[test]
fn blank_lines_and_comment_lines_before_the_start_change_nothing() {
    let p = lines("; G91\n\n(G91)\n  \nG0 X1\nG0 X2");
    let c = startline::check(&p, 5);
    assert_eq!(c.refusal, None);
    assert_eq!(c.at.map(|a| a.0), Some(1.0));
}

// ---------------------------------------------------------------- starting a program in the middle

#[test]
fn a_line_that_is_not_in_the_program_has_no_plan() {
    let p = lines("G0 X1\nG0 X2");
    assert_eq!(
        plan(&p, 0).unwrap_err(),
        Why::PastEnd { lines: 2, asked: 0 }
    );
    assert_eq!(
        plan(&p, 3).unwrap_err(),
        Why::PastEnd { lines: 2, asked: 3 }
    );
    assert_eq!(
        plan(&[], 1).unwrap_err(),
        Why::PastEnd { lines: 0, asked: 1 }
    );
    assert_eq!(
        plan(&p, usize::MAX).unwrap_err(),
        Why::PastEnd {
            lines: 2,
            asked: usize::MAX
        }
    );
}

#[test]
fn a_state_that_cannot_be_rebuilt_is_refused_by_the_reason() {
    let relative = lines("G91\nG0 X1\nG0 X2");
    assert_eq!(
        plan(&relative, 3).unwrap_err(),
        Why::Refused(Refusal::Relative)
    );
    let cycle = lines("G81 X1 Z-1 R1\nG0 X2");
    assert_eq!(
        plan(&cycle, 2).unwrap_err(),
        Why::Refused(Refusal::Cycle(1))
    );
    let comp = lines("G41 D1\nG0 X2");
    assert_eq!(
        plan(&comp, 2).unwrap_err(),
        Why::Refused(Refusal::Compensation(1))
    );
}

#[test]
fn the_new_program_puts_the_controller_back_and_goes_on_from_the_line() {
    let p = lines("G21 G90\nT2 M6\nM3 S12000\nG0 X10 Y5\nG1 X20 F200\nG1 X30");
    let r = plan(&p, 6).unwrap();
    assert_eq!((r.line, r.before), (6, 5));
    assert_eq!(
        r.program,
        [
            "(restart at line 6 of the program)",
            "G21",
            "G90",
            "G17",
            "G54",
            "F200",
            "S12000",
            "(restart: the tool in use was T2)",
            "(restart: the spindle was on, M3 S12000, start it first)",
            "(restart: the tool was at X20.000 Y5.000 mm before this line, move there at a safe height)",
            "M0 (restart: check the machine, then resume)",
            "G1",
            "G1 X30"
        ]
    );
    assert_eq!(r.preamble, 12);
}

#[test]
fn a_program_in_inches_restarts_in_inches() {
    let p = lines("G20\nG0 X1\nG0 X2");
    let r = plan(&p, 3).unwrap();
    assert_eq!(r.program[1], "G20");
}

#[test]
fn the_lines_of_the_original_after_the_start_are_all_kept_in_order() {
    let p = lines("G21\nG0 X1\nG0 X2\nG0 X3\nM30");
    for line in 1..=p.len() {
        let r = plan(&p, line).unwrap();
        assert_eq!(
            r.program.len(),
            r.preamble + (p.len() - (line - 1)),
            "{line}"
        );
        assert_eq!(&r.program[r.preamble..], &p[line - 1..], "{line}");
        assert_eq!(r.before, line - 1);
        assert!(r.preamble >= 5);
    }
}

#[test]
fn the_new_program_always_ends_its_head_with_a_stop() {
    let p = lines("G0 X1\nG0 X2");
    let r = plan(&p, 1).unwrap();
    // the stop, and after it only the way of moving, which moves nothing
    assert_eq!(
        r.program[r.preamble - 2],
        "M0 (restart: check the machine, then resume)"
    );
    assert_eq!(r.program[r.preamble - 1], "G0");
    assert_eq!(r.program[0], "(restart at line 1 of the program)");
}

// ---------------------------------------------------------------- the change of unit

#[test]
fn an_empty_program_is_converted_to_an_empty_program() {
    let c = convert(&[], true);
    assert!(c.lines.is_empty() && c.findings.is_empty());
    assert_eq!((c.numbers, c.changed_lines), (0, 0));
}

#[test]
fn lengths_in_millimetres_become_inches_with_the_zeros_at_the_end_taken_out() {
    let c = convert(&lines("G21\nG0 X127 Y-25.4 Z2.54"), true);
    assert_eq!(c.lines, ["G20", "G0 X5 Y-1 Z0.1"]);
    assert_eq!((c.numbers, c.changed_lines), (3, 2));
    assert!(c.findings.is_empty());
}

#[test]
fn lengths_in_inches_become_millimetres() {
    let c = convert(&lines("G20\nG1 X1 F10"), false);
    assert_eq!(c.lines, ["G21", "G1 X25.4 F254"]);
}

#[test]
fn inches_have_five_decimals_and_millimetres_four() {
    assert_eq!(convert(&lines("G21\nG1 X1"), true).lines[1], "G1 X0.03937");
    assert_eq!(
        convert(&lines("G20\nG1 X0.12345"), false).lines[1],
        "G1 X3.1356"
    );
    assert!((MM_PER_INCH - 25.4).abs() < f64::EPSILON);
}

#[test]
fn a_program_already_in_the_unit_asked_for_comes_back_the_same() {
    let text = lines("G20\nG1 X1.234 F10");
    let c = convert(&text, true);
    assert_eq!(c.lines, text);
    assert_eq!((c.numbers, c.changed_lines), (0, 0));
    let again = convert(&convert(&lines("G21\nG1 X25.4 F254"), true).lines, true);
    assert_eq!(again.lines, ["G20", "G1 X1 F10"]);
    assert_eq!(again.numbers, 0);
}

#[test]
fn the_angles_the_speed_the_tool_and_the_numbers_of_lines_are_not_lengths() {
    let c = convert(&lines("G21\nA90 B180 C45 S12000 T3 P1 H3 D3 L2 N10"), true);
    assert_eq!(c.lines, ["G20", "A90 B180 C45 S12000 T3 P1 H3 D3 L2 N10"]);
    assert_eq!((c.numbers, c.changed_lines), (0, 1));
}

#[test]
fn the_centres_of_arcs_the_radius_and_the_depth_of_a_peck_are_lengths() {
    let c = convert(
        &lines("G21\nG83 Z-25.4 R2.54 Q5.08 P1\nG2 X25.4 Y0 I12.7 J0"),
        true,
    );
    assert_eq!(c.lines[1], "G83 Z-1 R0.1 Q0.2 P1");
    assert_eq!(c.lines[2], "G2 X1 Y0 I0.5 J0");
}

#[test]
fn comments_in_parentheses_and_after_a_semicolon_are_left_as_they_are() {
    let c = convert(&lines("G21\nG1 X25.4 (X25.4) ; X25.4"), true);
    assert_eq!(c.lines[1], "G1 X1 (X25.4) ; X25.4");
    assert_eq!(c.numbers, 1);
}

#[test]
fn a_value_given_by_a_parameter_or_an_expression_is_left_and_said() {
    let c = convert(&lines("G21\nG1 X#1 Y[#2+1]"), true);
    assert_eq!(c.lines[1], "G1 X#1 Y[#2+1]");
    assert_eq!(
        msgs(&c.findings),
        [
            (Some(1), Msg::UnitsExpression),
            (Some(1), Msg::UnitsExpression)
        ]
    );
    assert_eq!(c.findings[0].args, ["X"]);
    assert_eq!(c.findings[1].args, ["Y"]);
}

#[test]
fn a_feed_in_inverse_time_is_not_converted_and_is_said() {
    let c = convert(&lines("G21\nG93\nG1 X25.4 F2"), true);
    assert_eq!(c.lines[2], "G1 X1 F2");
    assert_eq!(msgs(&c.findings), [(Some(2), Msg::UnitsInverseFeed)]);
    assert_eq!(c.findings[0].level, Level::Warning);
}

#[test]
fn a_feed_that_is_back_in_units_per_minute_is_converted_again() {
    let c = convert(&lines("G21\nG93\nG1 X25.4 F2\nG94\nG1 X25.4 F254"), true);
    assert_eq!(c.lines[4], "G1 X1 F10");
}

#[test]
fn a_change_of_unit_in_the_middle_of_the_program_is_said_and_both_halves_follow_their_unit() {
    let c = convert(&lines("G21\nG1 X25.4\nG20\nG1 X1"), false);
    assert_eq!(c.lines, ["G21", "G1 X25.4", "G21", "G1 X25.4"]);
    assert_eq!(msgs(&c.findings), [(Some(2), Msg::UnitsSwitched)]);
    assert_eq!(c.findings[0].args, ["G20"]);
}

#[test]
fn a_program_that_never_says_its_unit_is_read_as_millimetres_and_says_so_once() {
    let c = convert(&lines("G1 X10\nG1 X20"), true);
    assert_eq!(c.lines, ["G20", "G1 X0.3937", "G1 X0.7874"]);
    assert_eq!(msgs(&c.findings), [(None, Msg::UnitsAssumed)]);
    assert_eq!((c.numbers, c.changed_lines), (2, 2));
    let to_mm = convert(&lines("G1 X10"), false);
    assert_eq!(to_mm.lines, ["G1 X10"]);
    assert_eq!(msgs(&to_mm.findings), [(None, Msg::UnitsAssumed)]);
}

#[test]
fn the_unit_said_late_is_the_unit_of_the_lines_after_it() {
    let c = convert(&lines("G1 X10\nG20\nG1 X1"), false);
    assert_eq!(c.lines, ["G1 X10", "G21", "G1 X25.4"]);
}

#[test]
fn the_letters_in_lower_case_and_a_space_before_the_number_are_kept_as_written() {
    let c = convert(&lines("g21\ng1 x25.4 y 50.8"), true);
    assert_eq!(c.lines, ["g20", "g1 x1 y 2"]);
}

#[test]
fn a_number_that_rounds_to_minus_zero_is_written_as_zero() {
    let c = convert(&lines("G20\nG1 X-0.0000001"), false);
    assert_eq!(c.lines[1], "G1 X0");
}

#[test]
fn big_and_negative_numbers_keep_their_digits() {
    let c = convert(&lines("G21\nG1 X1000000 Z-127"), true);
    assert_eq!(c.lines[1], "G1 X39370.07874 Z-5");
}

#[test]
fn the_lines_that_change_are_counted_one_by_one() {
    let c = convert(&lines("G21\n(comment)\nG1 X25.4\n\nG1 Y25.4"), true);
    assert_eq!(c.changed_lines, 3);
    assert_eq!(c.lines.len(), 5);
}

#[test]
fn any_text_of_a_small_alphabet_is_converted_without_a_panic_and_keeps_its_lines() {
    let alphabet = b"GXYZF0123456789.-+ ;()#[]";
    for seed in 1..200u32 {
        let raw = lcg(seed, 60, alphabet.len() as u32);
        let text: String = raw.iter().map(|b| alphabet[*b as usize] as char).collect();
        let program = lines(&text.replace(';', ";\n"));
        for to_inches in [false, true] {
            let c = convert(&program, to_inches);
            assert!(c.lines.len() == program.len() || c.lines.len() == program.len() + 1);
        }
    }
}

// ---------------------------------------------------------------- the work coordinate systems

#[test]
fn a_program_with_no_move_uses_no_system() {
    let s = wcs::scan(&[]);
    assert!(s.used.is_empty() && s.findings.is_empty());
    assert!(wcs::scan(&lines("G21\nM3 S1000")).used.is_empty());
}

#[test]
fn the_moves_of_a_program_are_counted_in_the_system_in_force() {
    let s = wcs::scan(&lines("G0 X1\nG1 X2 Y3"));
    assert_eq!(
        s.used,
        [wcs::Use {
            system: 54,
            first_line: 0,
            moves: 2
        }]
    );
    assert!(s.findings.is_empty());
}

#[test]
fn a_system_that_nobody_wrote_is_told_once_with_its_first_line() {
    let s = wcs::scan(&lines("G55\nG0 X1"));
    assert_eq!(msgs(&s.findings), [(Some(1), Msg::WcsUnset)]);
    assert_eq!(s.findings[0].args, ["G55"]);
    assert_eq!(s.findings[0].level, Level::Info);
}

#[test]
fn writing_the_zero_of_a_system_is_a_warning_with_the_numbers_it_writes() {
    let s = wcs::scan(&lines("G10 L2 P2 X1 Y2.5\nG55\nG0 X3"));
    assert_eq!(msgs(&s.findings), [(Some(0), Msg::WcsWrite)]);
    assert_eq!(s.findings[0].args, ["G55", "1", "2.5", "-"]);
    assert_eq!(s.findings[0].level, Level::Warning);
    assert_eq!(s.used.len(), 1);
}

#[test]
fn l20_writes_here_and_other_values_of_l_are_not_a_write() {
    let here = wcs::scan(&lines("G10 L20 P1 Z0"));
    assert_eq!(msgs(&here.findings), [(Some(0), Msg::WcsWriteHere)]);
    let other = wcs::scan(&lines("G10 L1 P1 X1"));
    assert!(other.findings.is_empty() && other.used.is_empty());
}

#[test]
fn a_p_that_names_no_system_is_said_with_its_value() {
    for (text, shown) in [
        ("G10 L2 P7 X1", "7"),
        ("G10 L2 P-1 X1", "-1"),
        ("G10 L2 P1.5 X1", "1.5"),
    ] {
        let s = wcs::scan(&lines(text));
        assert_eq!(msgs(&s.findings), [(Some(0), Msg::WcsBadP)], "{text}");
        assert_eq!(s.findings[0].args, [shown]);
    }
}

#[test]
fn a_write_with_no_p_goes_to_the_system_in_force() {
    let s = wcs::scan(&lines("G56\nG10 L2 X1"));
    assert_eq!(s.findings[0].msg, Msg::WcsWrite);
    assert_eq!(s.findings[0].args[0], "G56");
}

#[test]
fn a_system_written_after_it_was_used_is_told_with_the_line_of_its_first_move() {
    let s = wcs::scan(&lines("G55\nG1 X1\nG10 L2 P2 X5"));
    assert_eq!(
        msgs(&s.findings),
        [
            (Some(1), Msg::WcsSwitchCut),
            (Some(2), Msg::WcsWrite),
            (Some(2), Msg::WcsWriteAfterUse)
        ]
    );
    assert_eq!(s.findings[2].args, ["G55", "2"]);
}

#[test]
fn a_cut_right_after_a_change_of_system_is_told_and_a_rapid_is_not() {
    let cut = wcs::scan(&lines("G54\nG1 X1\nG55\nG1 X2"));
    assert_eq!(
        msgs(&cut.findings),
        [
            (Some(3), Msg::WcsSwitchCut),
            (Some(3), Msg::WcsUnset),
            (None, Msg::WcsUsed)
        ]
    );
    assert_eq!(cut.findings[0].args, ["G55", "3"]);
    let rapid = wcs::scan(&lines("G54\nG1 X1\nG55\nG0 X2"));
    assert!(!msgs(&rapid.findings).contains(&(Some(3), Msg::WcsSwitchCut)));
}

#[test]
fn only_the_first_move_after_a_change_counts_for_the_warning() {
    let s = wcs::scan(&lines("G55\nG0 X1\nG1 X2"));
    assert!(!s.findings.iter().any(|f| f.msg == Msg::WcsSwitchCut));
}

#[test]
fn the_systems_a_program_moves_in_are_listed_in_the_order_of_their_number() {
    let s = wcs::scan(&lines("G55\nG0 X1\nG54\nG0 X2\nG0 X3"));
    assert_eq!(
        s.used,
        [
            wcs::Use {
                system: 54,
                first_line: 3,
                moves: 2
            },
            wcs::Use {
                system: 55,
                first_line: 1,
                moves: 1
            }
        ]
    );
    let used = s.findings.iter().find(|f| f.msg == Msg::WcsUsed).unwrap();
    assert_eq!(used.args, ["G54 (2), G55 (1)"]);
    assert_eq!(used.line, None);
}

#[test]
fn a_move_in_machine_coordinates_or_a_shift_of_the_zero_is_not_a_move_in_a_system() {
    assert!(wcs::scan(&lines("G53 G0 Z0")).used.is_empty());
    assert!(wcs::scan(&lines("G92 X0 Y0")).used.is_empty());
    assert!(wcs::scan(&lines("G0 A5")).used.is_empty());
    assert_eq!(wcs::scan(&lines("G0 Z5")).used.len(), 1);
}

#[test]
fn lines_of_the_file_format_are_skipped_and_the_letters_may_be_lower_case() {
    let s = wcs::scan(&lines("%\n$H\ng55\ng1 x1"));
    assert_eq!(s.used.len(), 1);
    assert_eq!(s.used[0].system, 55);
}

#[test]
fn the_last_system_named_on_a_line_is_the_one_in_force() {
    let s = wcs::scan(&lines("G54 G55 X1"));
    assert_eq!(s.used[0].system, 55);
}

#[test]
fn the_sentence_of_the_systems_is_given_only_when_there_are_two_or_more() {
    assert!(wcs::report_lines(&lines("G0 X1"), "en").is_empty());
    assert!(wcs::report_lines(&[], "en").is_empty());
    assert_eq!(
        wcs::report_lines(&lines("G54\nG0 X1\nG55\nG0 X2"), "en").len(),
        1
    );
    assert_eq!(wcs::findings(&lines("G55\nG0 X1")).len(), 1);
}

// ---------------------------------------------------------------- the cutter compensation

fn len(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

#[test]
fn the_sides_of_the_path_have_the_codes_of_the_controller() {
    assert_eq!(Side::Left.code(), "G41");
    assert_eq!(Side::Right.code(), "G42");
    assert!((MITRE_LIMIT - 4.0).abs() < f64::EPSILON);
}

#[test]
fn a_chain_with_fewer_than_two_different_points_has_no_pieces() {
    for chain in [
        vec![],
        vec![(1.0, 1.0)],
        vec![(1.0, 1.0), (1.0, 1.0)],
        vec![(1.0, 1.0), (1.0, 1.0 + 1e-12), (1.0, 1.0)],
    ] {
        let o = offset_chain(&chain, true, 1.0);
        assert!(o.pieces.is_empty() && o.gouged.is_empty(), "{chain:?}");
    }
}

#[test]
fn a_straight_move_is_shifted_to_the_left_or_to_the_right_of_its_direction() {
    let east = [(0.0, 0.0), (10.0, 0.0)];
    assert_eq!(
        offset_chain(&east, true, 1.0).pieces,
        [((0.0, 1.0), (10.0, 1.0))]
    );
    assert_eq!(
        offset_chain(&east, false, 1.0).pieces,
        [((0.0, -1.0), (10.0, -1.0))]
    );
    let north = [(0.0, 0.0), (0.0, 10.0)];
    assert_eq!(
        offset_chain(&north, true, 1.0).pieces,
        [((-1.0, 0.0), (-1.0, 10.0))]
    );
    assert_eq!(
        offset_chain(&north, false, 1.0).pieces,
        [((1.0, 0.0), (1.0, 10.0))]
    );
    assert_eq!(offset_chain(&east, true, 1.0).gouged, [false]);
}

#[test]
fn a_radius_of_zero_leaves_the_points_and_a_negative_one_changes_the_side() {
    let east = [(0.0, 0.0), (10.0, 0.0)];
    assert_eq!(
        offset_chain(&east, true, 0.0).pieces,
        [((0.0, 0.0), (10.0, 0.0))]
    );
    assert_eq!(
        offset_chain(&east, true, -1.0).pieces,
        offset_chain(&east, false, 1.0).pieces
    );
}

#[test]
fn a_point_that_comes_twice_in_a_row_is_taken_once() {
    let a = offset_chain(&[(0.0, 0.0), (0.0, 0.0), (10.0, 0.0)], true, 1.0);
    let b = offset_chain(&[(0.0, 0.0), (10.0, 0.0)], true, 1.0);
    assert_eq!(a, b);
}

#[test]
fn an_inside_corner_is_joined_where_the_two_shifted_moves_meet() {
    let l = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)];
    let o = offset_chain(&l, true, 1.0);
    assert_eq!(
        o.pieces,
        [((0.0, 1.0), (9.0, 1.0)), ((9.0, 1.0), (9.0, 10.0))]
    );
    assert_eq!(o.gouged, [false, false]);
    assert!((offset_length(&o) - 18.0).abs() < 1e-9);
}

#[test]
fn an_outside_corner_is_joined_at_the_mitre_when_it_is_not_too_sharp() {
    let l = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)];
    let o = offset_chain(&l, false, 1.0);
    assert_eq!(
        o.pieces,
        [((0.0, -1.0), (11.0, -1.0)), ((11.0, -1.0), (11.0, 10.0))]
    );
}

#[test]
fn a_very_sharp_outside_corner_is_cut_off_and_the_length_counts_the_line_that_joins() {
    let sharp = [(0.0, 0.0), (10.0, 0.0), (0.0, 1.0)];
    let o = offset_chain(&sharp, false, 1.0);
    assert_eq!(o.pieces[0].1, (10.0, -1.0));
    assert_ne!(o.pieces[0].1, o.pieces[1].0);
    let sum = len(o.pieces[0].0, o.pieces[0].1) + len(o.pieces[1].0, o.pieces[1].1);
    assert!(offset_length(&o) > sum + 1.0);
}

#[test]
fn moves_that_go_on_in_the_same_direction_are_joined_at_one_point() {
    let o = offset_chain(&[(0.0, 0.0), (5.0, 0.0), (10.0, 0.0)], true, 1.0);
    assert_eq!(
        o.pieces,
        [((0.0, 1.0), (5.0, 1.0)), ((5.0, 1.0), (10.0, 1.0))]
    );
}

#[test]
fn a_move_straight_back_keeps_its_two_ends_apart() {
    let o = offset_chain(&[(0.0, 0.0), (10.0, 0.0), (0.0, 0.0)], true, 1.0);
    assert_eq!(
        o.pieces,
        [((0.0, 1.0), (10.0, 1.0)), ((10.0, -1.0), (0.0, -1.0))]
    );
    assert_eq!(o.gouged, [false, false]);
}

#[test]
fn a_corner_tighter_than_the_tool_turns_the_moves_round() {
    let o = offset_chain(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)], true, 5.0);
    assert_eq!(o.gouged, [true, true]);
}

#[test]
fn the_length_of_nothing_is_zero_and_of_a_straight_move_is_its_length() {
    assert_eq!(
        offset_length(&Offset {
            pieces: Vec::new(),
            gouged: Vec::new()
        }),
        0.0
    );
    let o = offset_chain(&[(0.0, 0.0), (6.0, 8.0)], true, 2.0);
    assert!((offset_length(&o) - 10.0).abs() < 1e-9);
}

#[test]
fn a_point_that_is_not_a_number_is_left_out_of_the_chain() {
    let o = offset_chain(&[(0.0, 0.0), (f64::NAN, 0.0), (10.0, 0.0)], true, 1.0);
    assert_eq!(o.pieces.len(), 1);
    let first = offset_chain(&[(f64::NAN, 0.0), (1.0, 0.0)], true, 1.0);
    assert!(first.pieces.is_empty());
}

#[test]
fn points_at_infinity_and_a_huge_radius_do_not_stop_the_work() {
    let _ = offset_chain(&[(0.0, 0.0), (f64::INFINITY, 0.0), (1.0, 1.0)], true, 1.0);
    let o = offset_chain(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)], true, 1.0e300);
    assert_eq!(o.pieces.len(), 2);
    let _ = offset_length(&o);
}

#[test]
fn a_chain_shifted_by_nothing_goes_through_its_own_points() {
    for seed in 1..40u32 {
        let raw = lcg(seed, 16, 20);
        let chain: Vec<(f64, f64)> = raw
            .chunks(2)
            .map(|c| (f64::from(c[0]) * 3.0, f64::from(c[1]) * 3.0))
            .collect();
        let mut dedup: Vec<(f64, f64)> = Vec::new();
        for p in &chain {
            if dedup.last().is_none_or(|l| len(*l, *p) > 1e-9) {
                dedup.push(*p);
            }
        }
        let o = offset_chain(&chain, true, 0.0);
        assert_eq!(o.pieces.len(), dedup.len().saturating_sub(1), "{seed}");
        assert_eq!(o.gouged.len(), o.pieces.len());
        for (i, (s, e)) in o.pieces.iter().enumerate() {
            assert!(
                len(*s, dedup[i]) < 1e-6 && len(*e, dedup[i + 1]) < 1e-6,
                "{seed} {i}"
            );
        }
    }
}

// ---------------------------------------------------------------- the canned cycles

#[test]
fn the_codes_of_the_canned_cycles_are_told_from_the_ones_that_are_not() {
    for yes in [
        "G81 X1", "G73 Z-1", "G82", "G83", "G84", "G85", "G86", "G87", "G88", "G89", "g83 x1",
    ] {
        assert!(has_cycles(&lines(yes)), "{yes}");
    }
    for no in [
        "G80", "G90", "G8", "G81.1", "G72", "(G81)", "; G81", "", "M3 S1000", "G0 X81",
    ] {
        assert!(!has_cycles(&lines(no)), "{no}");
    }
    assert!(!has_cycles(&[]));
}

#[test]
fn a_program_without_a_cycle_comes_back_line_for_line() {
    let p = lines("G21 G90\nG0 X1 Y1\nG1 Z-1 F100\nM30");
    let e = expand(&p);
    assert_eq!(e.lines, p);
    assert_eq!(e.origin, [0, 1, 2, 3]);
    assert_eq!((e.holes, e.findings.len()), (0, 0));
}

#[test]
fn the_lines_of_the_file_format_pass_through_untouched() {
    let p = lines("%\n$H\nG0 X1");
    assert_eq!(expand(&p).lines, p);
}

#[test]
fn a_drilling_cycle_becomes_its_moves() {
    let p = lines("G0 Z5\nG81 X10 Y20 Z-2 R1 F100\nG80");
    let e = expand(&p);
    assert_eq!(
        e.lines,
        [
            "G0 Z5",
            "G0 X10 Y20",
            "G0 Z1",
            "G1 Z-2 F100",
            "G0 Z5",
            "G80"
        ]
    );
    assert_eq!(e.origin, [0, 1, 1, 1, 1, 2]);
    assert_eq!(e.holes, 1);
    assert!(e.findings.is_empty());
}

#[test]
fn a_cycle_that_is_not_closed_is_told_with_the_line_that_began_it() {
    let e = expand(&lines("G82 X1 Y1 Z-1 R2 P0.5 F50"));
    assert_eq!(
        e.lines,
        ["G0 Z2", "G0 X1 Y1", "G1 Z-1 F50", "G4 P0.5", "G0 Z2"]
    );
    assert_eq!(msgs(&e.findings), [(Some(0), Msg::CycleOpen)]);
    assert_eq!(e.findings[0].args, ["G82"]);
}

#[test]
fn a_hole_with_pecks_goes_down_by_the_step_and_comes_up_each_time() {
    let e = expand(&lines("G83 X0 Y0 Z-3 R1 Q1 F10\nG80"));
    assert_eq!(e.holes, 1);
    let cuts: Vec<&String> = e.lines.iter().filter(|l| l.starts_with("G1 ")).collect();
    assert_eq!(cuts, ["G1 Z0 F10", "G1 Z-1", "G1 Z-2", "G1 Z-3"]);
    assert_eq!(e.lines.last().map(String::as_str), Some("G80"));
    assert_eq!(e.lines[e.lines.len() - 2], "G0 Z1");
}

#[test]
fn a_peck_cycle_with_no_step_cuts_the_hole_in_one_go_and_says_so() {
    let e = expand(&lines("G83 X0 Y0 Z-3 R1 F10\nG80"));
    let cuts: Vec<&String> = e.lines.iter().filter(|l| l.starts_with("G1 ")).collect();
    assert_eq!(cuts, ["G1 Z-3 F10"]);
    assert_eq!(msgs(&e.findings), [(Some(0), Msg::CycleNoQ)]);
}

#[test]
fn a_cycle_that_lacks_what_it_needs_is_kept_as_written_and_said_once() {
    let no_z = expand(&lines("G81 X1 Y1 R1\nX2 Y2\nG80"));
    assert_eq!(no_z.lines, ["G81 X1 Y1 R1", "X2 Y2", "G80"]);
    assert_eq!(msgs(&no_z.findings), [(Some(0), Msg::CycleNoZ)]);
    let no_r = expand(&lines("G81 X1 Y1 Z-1\nG80"));
    assert_eq!(no_r.findings[0].msg, Msg::CycleNoR);
    let low_r = expand(&lines("G81 X1 Z1 R0.5\nG80"));
    assert_eq!(low_r.findings[0].msg, Msg::CycleRBelowZ);
    let unsupported = expand(&lines("G87 X1 Y1 Z-1 R1\nG80"));
    assert_eq!(unsupported.findings[0].msg, Msg::CycleUnsupported);
    let relative = expand(&lines("G91\nG81 X1 Y1 Z-1 R1\nG80"));
    assert_eq!(relative.findings[0].msg, Msg::CycleRelative);
    assert_eq!(relative.findings[0].line, Some(1));
}

#[test]
fn a_clearance_that_is_not_a_number_or_is_below_zero_is_the_usual_one() {
    let p = lines("G83 X0 Y0 Z-3 R1 Q1 F10\nG80");
    let usual = expand(&p);
    assert_eq!(expand_with(&p, f64::NAN), usual);
    assert_eq!(expand_with(&p, f64::INFINITY), usual);
    assert_eq!(expand_with(&p, PECK_CLEARANCE_MM), usual);
    assert_eq!(expand_with(&p, -3.0), expand_with(&p, 0.0));
}

#[test]
fn a_hole_is_never_cut_in_more_pecks_than_the_limit() {
    let e = expand(&lines("G83 X0 Y0 Z-1000000 R1 Q0.0001 F10\nG80"));
    let cuts = e.lines.iter().filter(|l| l.starts_with("G1 ")).count();
    assert!(cuts <= MAX_PECKS + 1, "{cuts}");
    assert!(cuts > 100);
}

#[test]
fn any_program_made_of_cycle_words_is_expanded_without_a_panic() {
    let alphabet = b"G8 3012XYZRQPF.-\n";
    for seed in 1..150u32 {
        let raw = lcg(seed, 90, alphabet.len() as u32);
        let text: String = raw.iter().map(|b| alphabet[*b as usize] as char).collect();
        let program = lines(&text);
        let e = expand(&program);
        assert_eq!(e.lines.len(), e.origin.len(), "{seed}");
        assert!(e.origin.iter().all(|o| *o < program.len().max(1)), "{seed}");
    }
}

// ---------------------------------------------------------------- the settings ranges

#[test]
fn the_table_of_ranges_has_nineteen_keys_that_are_all_different() {
    assert_eq!(RANGES.len(), 19);
    let mut seen = std::collections::HashSet::new();
    for r in &RANGES {
        assert!(seen.insert(r.key), "{} twice", r.key);
        assert!(!r.key.is_empty());
        assert!(
            r.key
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b == b'_' || b.is_ascii_digit()),
            "{}",
            r.key
        );
    }
}

#[test]
fn every_range_has_finite_ends_in_order() {
    for r in &RANGES {
        assert!(r.lo.is_finite() && r.hi.is_finite(), "{}", r.key);
        assert!(r.lo < r.hi, "{}", r.key);
    }
}

#[test]
fn the_range_of_a_key_is_the_one_of_the_table() {
    for r in &RANGES {
        assert_eq!(range_of(r.key), (r.lo, r.hi), "{}", r.key);
    }
    assert_eq!(range_of("lint_rapid_floor"), (-500.0, 500.0));
    assert_eq!(range_of("accel"), (0.0, 100_000.0));
}

#[test]
fn a_key_that_is_not_in_the_table_has_the_widest_range_and_the_letter_case_counts() {
    for key in ["", "nothing", "ACCEL", " accel", "accel "] {
        assert_eq!(range_of(key), (f64::MIN, f64::MAX), "{key:?}");
    }
}

// ---------------------------------------------------------------- the probe

fn cfg() -> ProbeCfg {
    ProbeCfg::default()
}

#[test]
fn the_starting_values_of_the_probe_can_be_used() {
    let c = cfg();
    assert_eq!(
        (
            c.feed_mm_min,
            c.depth_mm,
            c.retract_mm,
            c.plate_mm,
            c.clearance_mm
        ),
        (20.0, 10.0, 2.0, 0.0, 5.0)
    );
    assert_eq!(c.check(), Ok(()));
}

#[test]
fn each_value_of_the_probe_is_good_at_both_ends_and_bad_just_outside() {
    let with = |f: &dyn Fn(&mut ProbeCfg)| {
        let mut c = cfg();
        f(&mut c);
        c.check()
    };
    assert!(with(&|c| c.feed_mm_min = 1.0).is_ok());
    assert!(with(&|c| c.feed_mm_min = 1000.0).is_ok());
    assert!(with(&|c| c.feed_mm_min = 0.999).is_err());
    assert!(with(&|c| c.feed_mm_min = 1000.001).is_err());
    assert!(with(&|c| c.depth_mm = 0.1).is_ok());
    assert!(with(&|c| c.depth_mm = 200.0).is_ok());
    assert!(with(&|c| c.depth_mm = 0.09).is_err());
    assert!(with(&|c| c.depth_mm = 200.1).is_err());
    assert!(with(&|c| c.retract_mm = 0.0).is_ok());
    assert!(with(&|c| c.retract_mm = 100.0).is_ok());
    assert!(with(&|c| c.retract_mm = -0.1).is_err());
    assert!(with(&|c| c.retract_mm = 100.1).is_err());
    assert!(with(&|c| c.plate_mm = 0.0).is_ok());
    assert!(with(&|c| c.plate_mm = 100.0).is_ok());
    assert!(with(&|c| c.plate_mm = -0.1).is_err());
    assert!(with(&|c| c.clearance_mm = 0.5).is_ok());
    assert!(with(&|c| c.clearance_mm = 200.0).is_ok());
    assert!(with(&|c| c.clearance_mm = 0.49).is_err());
    assert!(with(&|c| c.clearance_mm = 200.5).is_err());
}

#[test]
fn a_value_that_is_not_a_number_or_is_infinite_is_refused_in_every_field() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for which in 0..5 {
            let mut c = cfg();
            match which {
                0 => c.feed_mm_min = bad,
                1 => c.depth_mm = bad,
                2 => c.retract_mm = bad,
                3 => c.plate_mm = bad,
                _ => c.clearance_mm = bad,
            }
            assert!(c.check().is_err(), "{bad} in field {which}");
        }
    }
}

#[test]
fn the_reason_names_the_value_and_its_range() {
    let mut c = cfg();
    c.feed_mm_min = 0.0;
    assert_eq!(
        c.check().unwrap_err(),
        "the probe feed must be between 1 and 1000 mm/min"
    );
    let mut c = cfg();
    c.depth_mm = 0.0;
    assert_eq!(
        c.check().unwrap_err(),
        "the probe depth must be between 0.1 and 200 mm"
    );
    let mut c = cfg();
    c.clearance_mm = 0.0;
    assert_eq!(
        c.check().unwrap_err(),
        "the clearance must be between 0.5 and 200 mm"
    );
}

#[test]
fn the_first_value_that_is_wrong_is_the_one_that_is_said() {
    let c = ProbeCfg {
        feed_mm_min: 0.0,
        depth_mm: 0.0,
        ..cfg()
    };
    assert!(c.check().unwrap_err().contains("feed"));
}

#[test]
fn the_touch_off_of_z_is_seven_lines_with_the_values_of_the_probe() {
    assert_eq!(
        z_touch_off_lines(&cfg()).unwrap(),
        [
            "G91",
            "G38.2 Z-10 F20",
            "G90",
            "G10 L20 P0 Z0",
            "G91",
            "G0 Z2",
            "G90"
        ]
    );
    let plate = ProbeCfg {
        plate_mm: 1.5,
        retract_mm: 3.0,
        ..cfg()
    };
    let l = z_touch_off_lines(&plate).unwrap();
    assert_eq!(l[3], "G10 L20 P0 Z1.5");
    assert_eq!(l[5], "G0 Z3");
}

#[test]
fn a_touch_off_with_values_that_cannot_be_used_gives_the_reason_and_no_line() {
    let c = ProbeCfg {
        depth_mm: f64::NAN,
        ..cfg()
    };
    assert!(z_touch_off_lines(&c).is_err());
}

// ---------------------------------------------------------------- the formats of the controllers

#[test]
fn the_names_of_the_profiles_are_five_in_this_order() {
    assert_eq!(NAMES, ["grbl", "full", "generic", "marlin", "klipper"]);
}

#[test]
fn each_profile_has_its_own_limits() {
    let want = [
        ("grbl", 79, 4, false, true),
        ("full", 255, 6, true, true),
        ("generic", 0, 0, true, false),
        ("marlin", 96, 0, false, false),
        ("klipper", 0, 0, false, false),
    ];
    for (name, line, decimals, percent, end) in want {
        let p = Profile::named(name).unwrap();
        assert_eq!(
            (p.max_line, p.max_decimals, p.percent, p.needs_end),
            (line, decimals, percent, end),
            "{name}"
        );
    }
}

fn custom(text: &str) -> Profile {
    Profile::from_text(text, "mine").unwrap_or_else(|e| panic!("{text:?}: {e:?}"))
}

fn refused_lines(text: &str) -> Vec<(Option<usize>, Msg, Vec<String>)> {
    Profile::from_text(text, "mine")
        .unwrap_err()
        .into_iter()
        .map(|f| (f.line, f.msg, f.args))
        .collect()
}

#[test]
fn a_profile_with_no_lines_is_the_generic_one_with_the_name_it_was_given() {
    let p = custom("");
    let mut generic = Profile::named("generic").unwrap();
    generic.name = "mine".to_string();
    assert_eq!(p, generic);
}

#[test]
fn the_base_of_a_profile_is_read_in_any_letter_case_and_the_last_base_wins() {
    assert_eq!(custom("BASE = Full").max_line, 255);
    assert_eq!(custom("base = grbl\nbase = marlin").max_line, 96);
    assert_eq!(custom("base = grbl").max_decimals, 4);
}

#[test]
fn a_base_that_does_not_exist_is_an_error_on_its_own_line() {
    assert_eq!(
        refused_lines("base = nothing"),
        [(Some(0), Msg::FmtProfileValue, vec!["base".to_string()])]
    );
}

#[test]
fn the_numbers_of_a_profile_have_their_limits() {
    assert_eq!(custom("max_line = 0").max_line, 0);
    assert_eq!(custom("max_line = 100000").max_line, 100_000);
    assert_eq!(custom("max_decimals = 15").max_decimals, 15);
    for bad in [
        "max_line = 100001",
        "max_line = -1",
        "max_line = abc",
        "max_line = 1.5",
        "max_line =",
        "max_decimals = 16",
    ] {
        let e = refused_lines(bad);
        assert_eq!(e.len(), 1, "{bad}");
        assert_eq!(e[0].1, Msg::FmtProfileValue, "{bad}");
    }
}

#[test]
fn the_switches_of_a_profile_take_the_words_of_yes_and_of_no() {
    for yes in ["yes", "on", "true", "1", "YES", " On "] {
        assert!(
            custom(&format!("base = grbl\npercent = {yes}")).percent,
            "{yes}"
        );
    }
    for no in ["no", "off", "false", "0", "NO"] {
        assert!(
            !custom(&format!("base = full\npercent = {no}")).percent,
            "{no}"
        );
        assert!(
            !custom(&format!("base = grbl\nneeds_end = {no}")).needs_end,
            "{no}"
        );
    }
    let e = refused_lines("percent = maybe");
    assert_eq!(
        e,
        [(Some(0), Msg::FmtProfileValue, vec!["percent".to_string()])]
    );
}

#[test]
fn a_key_that_is_not_known_and_a_line_without_an_equal_sign_are_errors() {
    assert_eq!(
        refused_lines("color = red"),
        [(Some(0), Msg::FmtProfileKey, vec!["color".to_string()])]
    );
    assert_eq!(
        refused_lines("no equal sign"),
        [(Some(0), Msg::FmtProfileSyntax, Vec::new())]
    );
    let e = Profile::from_text("color = red", "x").unwrap_err();
    assert_eq!(e[0].level, Level::Danger);
}

#[test]
fn the_comments_and_the_blank_lines_count_for_the_numbers_of_the_lines() {
    let e = refused_lines("# a comment\n\n; another\ncolor = red\n   \nmax_line = x");
    assert_eq!(
        e.iter().map(|x| x.0).collect::<Vec<_>>(),
        [Some(3), Some(5)]
    );
}

#[test]
fn several_errors_are_given_in_the_order_of_their_lines() {
    let e = refused_lines("max_line = x\nbase = nothing\ncolor = 1");
    assert_eq!(
        e.iter().map(|x| x.0).collect::<Vec<_>>(),
        [Some(0), Some(1), Some(2)]
    );
}

#[test]
fn a_byte_order_mark_at_the_start_of_a_profile_is_ignored() {
    assert_eq!(custom("\u{feff}base = grbl").max_line, 79);
}

#[test]
fn the_name_of_a_profile_is_cleaned_and_a_name_that_is_empty_is_an_error() {
    assert_eq!(custom("name = My|Prof").name, "My Prof");
    assert_eq!(custom("name = a=b").name, "a b");
    assert_eq!(
        refused_lines("name = |||"),
        [(Some(0), Msg::FmtProfileValue, vec!["name".to_string()])]
    );
    assert_eq!(
        Profile::from_text("", "  weird|name ").unwrap().name,
        "weird name"
    );
    assert_eq!(Profile::from_text("", "").unwrap().name, "custom");
}

#[test]
fn the_lists_of_codes_take_a_letter_in_front_and_refuse_what_is_not_a_code() {
    let p = custom("base = grbl\nallow_g = G5.1, 64, g10\nallow_m = M6, 7");
    let program = lines("G5.1\nG64\nM6\nM30");
    assert!(check(&program, &p).is_empty());
    for bad in [
        "allow_g = G1.55",
        "allow_g = xyz",
        "allow_g = 1000",
        "allow_g = -1",
        "allow_m = 1000",
        "allow_m = M5.5",
        "allow_m = M-1",
        "deny_g = 1,,2",
    ] {
        let e = refused_lines(bad);
        assert_eq!(e.len(), 1, "{bad}");
        assert_eq!(e[0].1, Msg::FmtProfileCode, "{bad}");
    }
    assert_eq!(refused_lines("allow_g = G1.55")[0].2, ["G1.55"]);
    assert!(Profile::from_text("allow_g =", "x").is_ok());
}

#[test]
fn a_code_that_is_denied_is_refused_even_when_the_base_lists_it() {
    let p = custom("deny_m = 3");
    let f = check(&lines("M3 S1000"), &p);
    assert_eq!(msgs(&f), [(Some(0), Msg::FmtCode)]);
    assert_eq!(f[0].args, ["mine", "M3"]);
    let g = custom("deny_g = G1");
    assert_eq!(msgs(&check(&lines("G1 X1"), &g)), [(Some(0), Msg::FmtCode)]);
}

#[test]
fn the_place_of_a_profile_of_the_user_is_next_to_the_settings_file() {
    let p = profile_path(std::path::Path::new("conf/phoenix-cnc.cfg"), "mine").unwrap();
    assert_eq!(
        p.file_name().unwrap().to_string_lossy(),
        "phoenix-cnc-format-mine.txt"
    );
    assert_eq!(p.parent().unwrap(), std::path::Path::new("conf"));
    let bare = profile_path(std::path::Path::new(""), "x").unwrap();
    assert_eq!(
        bare.file_name().unwrap().to_string_lossy(),
        "phoenix-cnc-format-x.txt"
    );
}

#[test]
fn only_a_plain_name_has_a_place() {
    let s = std::path::Path::new("conf/phoenix-cnc.cfg");
    let max = "x".repeat(64);
    let over = "x".repeat(65);
    for good in ["a", "a-b_c9", "A", max.as_str()] {
        assert!(profile_path(s, good).is_some(), "{good}");
    }
    for bad in ["", "a b", "a/b", "a\\b", "..", "a.b", "é", over.as_str()] {
        assert!(profile_path(s, bad).is_none(), "{bad:?}");
    }
}

#[test]
fn a_name_that_is_one_of_the_profiles_is_found_before_any_file() {
    assert!(matches!(resolve("GRBL", None), Found::Profile(p) if p.name == "grbl"));
    assert!(matches!(resolve(" marlin ", None), Found::Profile(p) if p.name == "marlin"));
}

#[test]
fn a_name_that_is_nothing_is_not_found() {
    assert_eq!(resolve("", None), Found::NotFound);
    assert_eq!(resolve("   ", None), Found::NotFound);
    assert_eq!(
        resolve("no-such-profile-anywhere-r5", None),
        Found::NotFound
    );
}

#[test]
fn a_profile_of_the_user_is_found_next_to_the_settings_and_by_its_path() {
    let dir = scratch("profile-resolve");
    let settings = dir.join("phoenix-cnc.cfg");
    let file = dir.join("phoenix-cnc-format-mine.txt");
    std::fs::write(&file, "base = grbl\nmax_line = 50\n").unwrap();
    match resolve("mine", Some(settings.as_path())) {
        Found::Profile(p) => assert_eq!((p.name.as_str(), p.max_line), ("mine", 50)),
        other => panic!("{other:?}"),
    }
    match resolve(&file.to_string_lossy(), None) {
        Found::Profile(p) => assert_eq!(p.max_line, 50),
        other => panic!("{other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_profile_file_that_is_wrong_or_too_big_is_said_and_not_used() {
    let dir = scratch("profile-bad");
    let settings = dir.join("phoenix-cnc.cfg");
    std::fs::write(dir.join("phoenix-cnc-format-bad.txt"), "color = red\n").unwrap();
    match resolve("bad", Some(settings.as_path())) {
        Found::Invalid(f) => assert_eq!(f[0].msg, Msg::FmtProfileKey),
        other => panic!("{other:?}"),
    }
    let big = vec![b'#'; MAX_PROFILE_BYTES as usize + 1];
    std::fs::write(dir.join("phoenix-cnc-format-big.txt"), big).unwrap();
    match resolve("big", Some(settings.as_path())) {
        Found::Invalid(f) => assert_eq!((f[0].msg, f[0].line), (Msg::FmtProfileUnreadable, None)),
        other => panic!("{other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

fn grbl() -> Profile {
    Profile::named("grbl").unwrap()
}

fn generic() -> Profile {
    Profile::named("generic").unwrap()
}

#[test]
fn a_program_with_nothing_in_it_has_nothing_to_say_to_any_profile() {
    for name in NAMES {
        let p = Profile::named(name).unwrap();
        assert!(check(&[], &p).is_empty(), "{name}");
        assert!(
            check(&lines("\n(a comment)\n; another"), &p).is_empty(),
            "{name}"
        );
    }
}

#[test]
fn a_word_that_comes_twice_is_said_once_even_when_it_comes_three_times() {
    let f = check(&lines("G1 X1 X2 X3\nM30"), &grbl());
    assert_eq!(msgs(&f), [(Some(0), Msg::FmtDupWord)]);
    assert_eq!(f[0].args, ["X"]);
    let g = check(&lines("G1 X1 X2 Y1 Y2"), &generic());
    assert_eq!(g.len(), 2);
}

#[test]
fn two_codes_of_the_same_group_on_a_line_are_said_with_both_and_the_same_code_twice_is_not() {
    let f = check(&lines("G0 G1 X1"), &generic());
    assert_eq!(msgs(&f), [(Some(0), Msg::FmtGroup)]);
    assert_eq!(f[0].args, ["G0", "G1"]);
    assert!(check(&lines("G1 G1 X1"), &generic()).is_empty());
    for pair in [
        "G90 G91", "G20 G21", "G17 G18", "G54 G55", "G40 G41", "G93 G94",
    ] {
        assert_eq!(check(&lines(pair), &generic()).len(), 1, "{pair}");
    }
}

#[test]
fn two_spindle_codes_on_a_line_are_said_and_the_same_one_twice_is_not() {
    let f = check(&lines("M3 M4"), &generic());
    assert_eq!(f[0].args, ["M3", "M4"]);
    assert!(check(&lines("M3 M3"), &generic()).is_empty());
}

#[test]
fn a_code_the_controller_does_not_list_is_said_with_the_profile_and_the_code() {
    let f = check(&lines("G64\nG5.1\nM6\nM30"), &grbl());
    assert_eq!(
        f.iter().map(|x| x.args.clone()).collect::<Vec<_>>(),
        [["grbl", "G64"], ["grbl", "G5.1"], ["grbl", "M6"]]
    );
}

#[test]
fn a_line_with_only_a_percent_is_told_to_the_profiles_that_do_not_take_it() {
    let f = check(&lines("%"), &grbl());
    assert_eq!(msgs(&f), [(Some(0), Msg::FmtPercent)]);
    assert_eq!(f[0].level, Level::Info);
    assert!(check(&lines("%"), &Profile::named("full").unwrap()).is_empty());
}

#[test]
fn a_line_is_long_when_it_has_more_characters_than_the_profile_takes_without_blanks_and_comments() {
    let marlin = Profile::named("marlin").unwrap();
    let at_limit = format!("G1 X{}", "1".repeat(93));
    assert!(check(&lines(&at_limit), &marlin).is_empty());
    let over = format!("G1 X{}", "1".repeat(94));
    let f = check(&lines(&over), &marlin);
    assert_eq!(msgs(&f), [(Some(0), Msg::FmtLong)]);
    assert_eq!(f[0].args, ["97", "96"]);
}

#[test]
fn the_decimals_are_counted_from_the_longest_number_and_the_limit_itself_is_fine() {
    let f = check(&lines("G1 X1.23456 Y1.5\nM30"), &grbl());
    assert_eq!(msgs(&f), [(Some(0), Msg::FmtDecimals)]);
    assert_eq!(f[0].args, ["X1.23456", "4"]);
    assert!(check(&lines("G1 X1.2345\nM30"), &grbl()).is_empty());
    assert!(check(&lines("G1 X1.23456789"), &generic()).is_empty());
}

#[test]
fn a_program_that_does_not_end_is_told_to_the_profiles_that_want_an_end() {
    let f = check(&lines("G1 X1"), &grbl());
    assert_eq!(msgs(&f), [(None, Msg::FmtNoEnd)]);
    assert_eq!(f[0].args, ["grbl"]);
    assert!(check(&lines("G1 X1\nM2"), &grbl()).is_empty());
    assert!(check(&lines("G1 X1\nM30\n; the end"), &grbl()).is_empty());
    assert!(check(&lines("G1 X1"), &generic()).is_empty());
}

#[test]
fn the_codes_inside_a_comment_are_not_read() {
    assert!(check(&lines("G1 X1 (M6 G64 X2 X3)"), &generic()).is_empty());
    assert!(check(&lines("G1 X1 ; M6 G64 X2 X3"), &generic()).is_empty());
}

#[test]
fn a_profile_made_by_the_user_changes_what_is_said_about_a_program() {
    let p = custom("base = grbl\nallow_g = 64\nallow_m = 6");
    assert!(check(&lines("G64 M6\nM30"), &p).is_empty());
    assert_eq!(check(&lines("G64 M6\nM30"), &grbl()).len(), 2);
}

#[test]
fn any_program_made_of_a_small_alphabet_is_checked_by_every_profile_without_a_panic() {
    let alphabet = b"GM X1234.-\n;()%";
    for seed in 1..120u32 {
        let raw = lcg(seed, 80, alphabet.len() as u32);
        let text: String = raw.iter().map(|b| alphabet[*b as usize] as char).collect();
        let program = lines(&text);
        for name in NAMES {
            let _ = check(&program, &Profile::named(name).unwrap());
        }
    }
}
