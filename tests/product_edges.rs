//! The pure parts of the program at their exact edges: the map of heights of the stock, the transformation of a program and the reading of a line by the guard.

use eh3d_phoenix_cnc::guard::{line_motion, line_moves};
use eh3d_phoenix_cnc::heightmap::{num, parse_probe_report, probe_points, HeightMap, MAX_POINTS};
use eh3d_phoenix_cnc::transform::{needs_start, start_from_tool, Refusal, Transform, Why};

fn l(lines: &[&str]) -> Vec<String> {
    lines.iter().map(|s| s.to_string()).collect()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

const AREA: [f64; 4] = [0.0, 0.0, 10.0, 10.0];

/// A map of 3 by 3 points on a tilted plane: the height is x / 5 + 3 y / 5.
fn tilted() -> HeightMap {
    let readings = [1.0, 2.0, 3.0, 6.0, 5.0, 4.0, 7.0, 8.0, 9.0];
    HeightMap::from_readings(AREA, 3, 3, &readings).unwrap()
}

// ---- the numbers ----

#[test]
fn num_writes_up_to_four_decimals_without_trailing_zeros_or_minus_zero() {
    assert_eq!(num(0.0), "0");
    assert_eq!(num(-0.0), "0");
    assert_eq!(num(-0.00004), "0");
    assert_eq!(num(1.5), "1.5");
    assert_eq!(num(-2.25), "-2.25");
    assert_eq!(num(10.0), "10");
    assert_eq!(num(1.00004), "1");
    assert_eq!(num(0.1234), "0.1234");
}

// ---- the grid of the probe ----

#[test]
fn the_probe_goes_along_the_rows_back_and_forth() {
    let pts = probe_points(AREA, 3, 3).unwrap();
    assert_eq!(
        pts,
        [
            (0.0, 0.0),
            (5.0, 0.0),
            (10.0, 0.0),
            (10.0, 5.0),
            (5.0, 5.0),
            (0.0, 5.0),
            (0.0, 10.0),
            (5.0, 10.0),
            (10.0, 10.0)
        ]
    );
}

#[test]
fn a_grid_of_two_by_two_has_the_four_corners() {
    let pts = probe_points(AREA, 2, 2).unwrap();
    assert_eq!(pts, [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]);
}

#[test]
fn a_grid_that_makes_no_sense_is_refused_with_the_reason() {
    let nan = probe_points([0.0, f64::NAN, 1.0, 1.0], 3, 3).unwrap_err();
    assert!(nan.contains("not finite"), "{nan}");
    let flat = probe_points([0.0, 0.0, 0.0, 5.0], 3, 3).unwrap_err();
    assert!(flat.contains("empty"), "{flat}");
    let back = probe_points([5.0, 0.0, 1.0, 5.0], 3, 3).unwrap_err();
    assert!(back.contains("empty"), "{back}");
    for (nx, ny) in [(1, 5), (5, 1), (0, 0)] {
        let e = probe_points(AREA, nx, ny).unwrap_err();
        assert!(e.contains("at least 2"), "{e}");
    }
}

#[test]
fn the_number_of_points_has_an_exact_limit() {
    assert_eq!(MAX_POINTS, 10_000);
    assert_eq!(probe_points(AREA, 100, 100).unwrap().len(), 10_000);
    let e = probe_points(AREA, 101, 100).unwrap_err();
    assert!(e.contains("10000"), "{e}");
    assert!(probe_points(AREA, usize::MAX, 2).is_err());
}

// ---- the report of the probe ----

#[test]
fn the_report_of_the_probe_gives_the_height_or_nothing_for_a_miss() {
    let text = "ok\n[PRB:1.000,2.000,-3.500:1]\n[PRB:1,2,-4:0]\nnoise\n[PRB:0,0,0.25:1]\r\n";
    assert_eq!(parse_probe_report(text), [Some(-3.5), None, Some(0.25)]);
}

#[test]
fn a_report_with_damaged_numbers_is_a_miss_and_never_a_panic() {
    let text =
        "[PRB:1,2:1]\n[PRB:1,2,abc:1]\n[PRB:1,2,NaN:1]\n[PRB:1,2,inf:1]\n[PRB:1,2,3]\nPRB:\n";
    assert_eq!(parse_probe_report(text), [None, None, None, None]);
    assert!(parse_probe_report("").is_empty());
}

// ---- making a map ----

#[test]
fn the_readings_are_put_in_the_grid_with_the_row_that_comes_back_turned_round() {
    let m = tilted();
    let want: Vec<f64> = (0..9).map(f64::from).collect();
    assert_eq!(m.z, want);
    assert_eq!((m.nx, m.ny), (3, 3));
    assert_eq!(m.area(), AREA);
    assert_eq!(m.span(), (0.0, 8.0));
}

#[test]
fn the_map_is_zero_at_the_point_nearest_to_the_zero_of_the_work() {
    let readings = [3.5, 1.0, 2.0, 9.0, 4.25, 7.0, 6.0, 8.0, 5.0];
    let m = HeightMap::from_readings([-5.0, -5.0, 5.0, 5.0], 3, 3, &readings).unwrap();
    assert!(m.offset_at(0.0, 0.0).abs() < 1e-12);
    let (lo, hi) = m.span();
    assert!(lo <= 0.0 && hi >= 0.0);
}

#[test]
fn readings_that_do_not_fit_the_grid_are_refused() {
    let few = HeightMap::from_readings(AREA, 3, 3, &[0.0; 5]).unwrap_err();
    assert!(
        few.contains("5 readings") && few.contains("9 points"),
        "{few}"
    );
    let mut bad = [0.0; 9];
    bad[4] = f64::NAN;
    assert!(HeightMap::from_readings(AREA, 3, 3, &bad).is_err());
    bad[4] = f64::INFINITY;
    assert!(HeightMap::from_readings(AREA, 3, 3, &bad).is_err());
    assert!(HeightMap::from_readings(AREA, 1, 3, &[0.0; 3]).is_err());
}

#[test]
fn a_map_from_a_report_names_the_point_that_did_not_touch() {
    let ok = "[PRB:0,0,-1:1]\n[PRB:10,0,-0.5:1]\n[PRB:10,10,0:1]\n[PRB:0,10,-1.5:1]\n";
    let m = HeightMap::from_report(AREA, 2, 2, ok).unwrap();
    assert_eq!(m.z, [0.0, 0.5, -0.5, 1.0]);
    assert_eq!(m.offset_at(5.0, 5.0), 0.25);
    let miss = ok.replace("-0.5:1", "-0.5:0");
    let e = HeightMap::from_report(AREA, 2, 2, &miss).unwrap_err();
    assert!(e.contains("point 2"), "{e}");
    assert!(HeightMap::from_report(AREA, 2, 2, "").is_err());
}

// ---- the height under a point ----

#[test]
fn the_height_between_the_points_follows_the_tilted_plane_from_a_fixed_seed() {
    let m = tilted();
    let mut s: u64 = 20_241;
    let mut next = || {
        s = (s * 1_103_515_245 + 12_345) % 2_147_483_648;
        s as f64 / 2_147_483_648.0 * 10.0
    };
    for _ in 0..200 {
        let (x, y) = (next(), next());
        assert!(close(m.offset_at(x, y), x / 5.0 + 3.0 * y / 5.0), "{x} {y}");
    }
}

#[test]
fn the_height_outside_the_grid_is_that_of_the_edge() {
    let m = tilted();
    assert_eq!(m.offset_at(-100.0, -100.0), 0.0);
    assert_eq!(m.offset_at(100.0, 100.0), 8.0);
    assert_eq!(m.offset_at(100.0, 0.0), 2.0);
    assert_eq!(m.offset_at(0.0, 100.0), 6.0);
}

#[test]
fn the_map_holds_half_a_millimetre_around_its_area() {
    let m = tilted();
    assert!(m.contains(0.0, 0.0) && m.contains(10.0, 10.0));
    assert!(m.contains(10.5, 5.0) && m.contains(-0.5, 5.0));
    assert!(!m.contains(10.51, 5.0));
    assert!(!m.contains(5.0, -0.51));
    assert!(!m.contains(f64::NAN, 5.0));
}

// ---- saving a map ----

#[test]
fn a_map_is_saved_and_read_back_the_same() {
    let m = tilted();
    let text = m.to_text();
    assert_eq!(text, "heightmap 0 0 10 10 3 3\n0 1 2\n3 4 5\n6 7 8\n");
    assert_eq!(HeightMap::parse(&text).unwrap(), m);
    let with_bom = format!("\u{feff}{text}\n\n");
    assert_eq!(HeightMap::parse(&with_bom).unwrap(), m);
}

#[test]
fn a_map_that_is_damaged_is_refused_and_never_a_panic() {
    for bad in [
        "",
        "heightmap",
        "heightmap 0 0 10 10 3\n",
        "map 0 0 10 10 3 3\n0 0 0 0 0 0 0 0 0\n",
        "heightmap 0 0 10 10 x 3\n",
        "heightmap 0 0 10 10 3 3\n0 1 2\n3 4 5\n",
        "heightmap 0 0 10 10 3 3\n0 1 2\n3 4 5\n6 7 8\n9\n",
        "heightmap 0 0 10 10 3 3\n0 1 2\n3 NaN 5\n6 7 8\n",
        "heightmap 0 0 10 10 3 3\n0 1 2\n3 inf 5\n6 7 8\n",
        "heightmap 0 0 10 10 3 3\n0 1 2\n3 four 5\n6 7 8\n",
        "heightmap 10 0 0 10 3 3\n0 1 2\n3 4 5\n6 7 8\n",
        "heightmap 0 0 10 10 1 3\n0\n1\n2\n",
    ] {
        assert!(HeightMap::parse(bad).is_err(), "{bad:?}");
    }
}

// ---- correcting a program ----

#[test]
fn a_program_follows_the_map_in_short_pieces() {
    let m = tilted();
    let prog = l(&[
        "G21",
        "G90",
        "M3 S1000",
        "G0 Z5",
        "G0 X1 Y1",
        "G1 X0 Y0 Z-1 F100",
        "G1 X10",
    ]);
    let out = m.compensate(&prog, 5.0).unwrap();
    assert_eq!(
        out,
        l(&[
            "G21",
            "G90",
            "M3 S1000",
            "G0 Z5",
            "G0 X1 Y1",
            "G1 X0 Y0 Z-1 F100",
            "G1 X5 Y0 Z0",
            "G1 X10 Y0 Z1"
        ])
    );
}

#[test]
fn a_program_that_the_map_cannot_correct_is_refused_naming_the_line() {
    let m = tilted();
    let cases = [
        ("G2 X1 Y1 I1", "an arc"),
        ("G3 X1 Y1 J1", "an arc"),
        ("G20", "inches"),
        ("G91", "relative"),
        ("G92 X0", "shift of the zero"),
        ("G53 G0 Z0", "shift of the zero"),
    ];
    for (line, why) in cases {
        let e = m.compensate(&l(&["G21", line]), 1.0).unwrap_err();
        assert!(e.starts_with("line 2:") && e.contains(why), "{line}: {e}");
    }
}

#[test]
fn a_move_that_leaves_the_probed_area_is_refused() {
    let m = tilted();
    let e = m.compensate(&l(&["G1 X20"]), 5.0).unwrap_err();
    assert!(e.starts_with("line 1:") && e.contains("probed area"), "{e}");
    let rapid = m.compensate(&l(&["G0 X50 Y50 Z5"]), 5.0).unwrap_err();
    assert!(rapid.contains("probed area"), "{rapid}");
    assert_eq!(
        m.compensate(&l(&["G0 X50 Y50"]), 5.0).unwrap(),
        l(&["G0 X50 Y50"])
    );
}

#[test]
fn the_length_of_a_piece_has_an_exact_minimum() {
    let m = tilted();
    let prog = l(&["G1 X1"]);
    assert!(m.compensate(&prog, 0.01).is_ok());
    for bad in [0.009, 0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(m.compensate(&prog, bad).is_err(), "{bad}");
    }
}

#[test]
fn a_program_with_no_moves_comes_back_as_it_was() {
    let m = tilted();
    let prog = l(&["", "(only a comment)", "; another", "M5", "G54"]);
    assert_eq!(m.compensate(&prog, 1.0).unwrap(), prog);
    assert!(m.compensate(&[], 1.0).unwrap().is_empty());
}

// ---- transforming a program ----

fn moved(shift: [f64; 3]) -> Transform {
    Transform {
        shift,
        ..Transform::default()
    }
}

#[test]
fn doing_nothing_returns_the_program_even_when_it_would_be_refused() {
    let prog = l(&["G91", "G81 X1 Y1 Z-1 R1", "G1 X1"]);
    assert_eq!(Transform::default().apply(&prog).unwrap(), prog);
    let start_only = Transform {
        start: (3.0, 4.0),
        ..Transform::default()
    };
    assert!(start_only.is_identity());
    assert_eq!(start_only.apply(&prog).unwrap(), prog);
    assert!(!moved([0.0, 0.0, 0.001]).is_identity());
}

#[test]
fn a_shift_changes_the_numbers_and_leaves_the_rest_as_it_was() {
    let prog = l(&[
        "G0 X0 Y0",
        "g1 x1 y2 Z-1 F100",
        "G1 X1 (X99 Y99) ; Y77",
        "M3 S1000",
        "",
    ]);
    let out = moved([10.0, 5.0, 1.0]).apply(&prog).unwrap();
    assert_eq!(
        out,
        l(&[
            "G0 X10 Y5",
            "g1 x11 y7 Z0 F100",
            "G1 X11 (X99 Y99) ; Y77",
            "M3 S1000",
            ""
        ])
    );
}

#[test]
fn a_scale_multiplies_the_three_axes_and_not_the_feed() {
    let t = Transform {
        scale: 2.0,
        ..Transform::default()
    };
    let out = t.apply(&l(&["G1 X1 Y2 Z-1 F300"])).unwrap();
    assert_eq!(out, l(&["G1 X2 Y4 Z-2 F300"]));
}

#[test]
fn a_mirror_turns_the_sign_and_an_arc_that_is_mirrored_once_changes_its_direction() {
    let mx = Transform {
        mirror_x: true,
        ..Transform::default()
    };
    assert_eq!(mx.apply(&l(&["G1 X5 Y3"])).unwrap(), l(&["G1 X-5 Y3"]));
    assert_eq!(
        mx.apply(&l(&["G2 X1 Y1 I1 J0"])).unwrap(),
        l(&["G3 X-1 Y1 I-1 J0"])
    );
    assert_eq!(
        mx.apply(&l(&["G3 X1 Y1 I1 J0"])).unwrap(),
        l(&["G2 X-1 Y1 I-1 J0"])
    );
    let both = Transform {
        mirror_x: true,
        mirror_y: true,
        ..Transform::default()
    };
    assert_eq!(
        both.apply(&l(&["G2 X1 Y1 I1 J0"])).unwrap(),
        l(&["G2 X-1 Y-1 I-1 J0"])
    );
}

#[test]
fn a_turn_of_ninety_degrees_writes_both_coordinates() {
    let t = Transform {
        rotate_deg: 90.0,
        ..Transform::default()
    };
    let out = t.apply(&l(&["G1 X1 Y0", "G1 X2"])).unwrap();
    assert_eq!(out, l(&["G1 X0 Y1", "G1 X0 Y2"]));
}

#[test]
fn the_first_move_that_gives_one_coordinate_writes_the_other_when_it_moves() {
    let t = moved([0.0, 5.0, 0.0]);
    assert_eq!(
        t.apply(&l(&["G1 X3", "G1 X4"])).unwrap(),
        l(&["G1 X3 Y5", "G1 X4"])
    );
    let from = Transform {
        start: (0.0, 2.0),
        ..t
    };
    assert_eq!(from.apply(&l(&["G1 X3"])).unwrap(), l(&["G1 X3 Y7"]));
    assert_eq!(
        moved([0.0, 5.0, 0.0]).apply(&l(&["G1 Y3"])).unwrap(),
        l(&["G1 Y8"])
    );
}

#[test]
fn the_points_and_the_vectors_follow_the_turn_around_the_centre() {
    let t = Transform {
        rotate_deg: 180.0,
        centre: (10.0, 10.0),
        shift: [1.0, 0.0, 0.0],
        ..Transform::default()
    };
    let (x, y) = t.point(12.0, 10.0);
    assert!(close(x, 9.0) && close(y, 10.0), "{x} {y}");
    let (vx, vy) = t.vector(1.0, 2.0);
    assert!(close(vx, -1.0) && close(vy, -2.0), "{vx} {vy}");
}

#[test]
fn every_code_that_could_make_the_program_wrong_is_refused_with_its_reason() {
    let t = moved([1.0, 0.0, 0.0]);
    let table = [
        ("G91", Why::Relative),
        ("G18", Why::Plane),
        ("G19", Why::Plane),
        ("G81 X1 Y1 Z-1 R1", Why::Cycle),
        ("G83 X1 Y1 Z-1 R1 Q1", Why::Cycle),
        ("G38.2 Z-5", Why::Cycle),
        ("G10 L2 P1 X0", Why::Zero),
        ("G28", Why::Zero),
        ("G30", Why::Zero),
        ("G53 G0 X0", Why::Zero),
        ("G92 X0", Why::Zero),
        ("G92.1", Why::Zero),
        ("G90.1", Why::AbsoluteCentre),
    ];
    for (line, why) in table {
        let prog = l(&["G0 X1", "G1 X2", line]);
        assert_eq!(t.apply(&prog), Err(Refusal { why, line: 3 }), "{line}");
    }
    for fine in ["G90", "G21", "G17", "G54", "M3 S1000", "G91.1"] {
        assert!(t.apply(&l(&["G0 X1", fine])).is_ok(), "{fine}");
    }
}

#[test]
fn a_change_that_is_not_a_number_the_program_can_take_is_refused_before_any_line() {
    let bad = [
        Transform {
            scale: 0.0,
            ..Transform::default()
        },
        Transform {
            scale: -1.0,
            ..Transform::default()
        },
        Transform {
            scale: f64::NAN,
            ..Transform::default()
        },
        Transform {
            scale: 1_000_001.0,
            ..Transform::default()
        },
        moved([f64::NAN, 0.0, 0.0]),
        moved([0.0, f64::INFINITY, 0.0]),
        Transform {
            rotate_deg: f64::NAN,
            ..Transform::default()
        },
        Transform {
            centre: (f64::NAN, 0.0),
            ..Transform::default()
        },
        Transform {
            start: (0.0, f64::INFINITY),
            ..Transform::default()
        },
    ];
    for t in bad {
        assert_eq!(
            t.apply(&l(&["G1 X1"])),
            Err(Refusal {
                why: Why::BadScale,
                line: 0
            }),
            "{t:?}"
        );
    }
    let top = Transform {
        scale: 1_000_000.0,
        ..Transform::default()
    };
    assert_eq!(top.apply(&l(&["G1 X1"])).unwrap(), l(&["G1 X1000000"]));
}

#[test]
fn a_shift_and_its_opposite_give_back_the_program_from_a_fixed_seed() {
    let mut s: u64 = 987_654;
    let mut next = |m: u64| {
        s = (s * 1_103_515_245 + 12_345) % 2_147_483_648;
        (s >> 8) % m
    };
    let mut prog = Vec::new();
    for _ in 0..60 {
        let (x, y) = (next(1000) as i64 - 500, next(1000) as i64 - 500);
        prog.push(format!("G1 X{x} Y{y}"));
    }
    let (dx, dy) = (next(300) as f64 - 150.0, next(300) as f64 - 150.0);
    let there = moved([dx, dy, 0.0]).apply(&prog).unwrap();
    let back = moved([-dx, -dy, 0.0]).apply(&there).unwrap();
    assert_eq!(back, prog);
}

#[test]
fn a_mirror_done_twice_gives_back_the_program() {
    let prog = l(&["G0 X1 Y2", "G1 X-3.5 Y4 Z-1 F200", "G2 X1 Y1 I1 J0"]);
    for (mx, my) in [(true, false), (false, true), (true, true)] {
        let t = Transform {
            mirror_x: mx,
            mirror_y: my,
            ..Transform::default()
        };
        let once = t.apply(&prog).unwrap();
        assert_ne!(once, prog);
        assert_eq!(t.apply(&once).unwrap(), prog, "{mx} {my}");
    }
}

#[test]
fn a_program_that_needs_the_start_is_found_by_its_first_move() {
    assert_eq!(needs_start(&l(&["G90", "G1 X5 Y5"])), None);
    assert_eq!(needs_start(&l(&["G90", "G1 X5"])), Some(2));
    assert_eq!(needs_start(&l(&["G1 Y5", "G1 X1 Y1"])), Some(1));
    assert_eq!(needs_start(&l(&["G90", "M3", "G1 Z-1"])), None);
    assert_eq!(needs_start(&[]), None);
}

#[test]
fn the_start_taken_from_the_tool_follows_the_units_of_the_program() {
    let mm = start_from_tool(&l(&["G21", "G1 X1"]), [10.0, 20.0, 5.0]);
    assert_eq!(mm, (10.0, 20.0));
    let inch = start_from_tool(&l(&["G20", "G1 X1"]), [25.4, 50.8, 5.0]);
    assert!(close(inch.0, 1.0) && close(inch.1, 2.0), "{inch:?}");
    let none = start_from_tool(&l(&["G1 X1"]), [3.0, 4.0, 0.0]);
    assert_eq!(none, (3.0, 4.0));
    let first = start_from_tool(&l(&["G21", "G20"]), [25.4, 25.4, 0.0]);
    assert_eq!(first, (25.4, 25.4));
}

// ---- the lines the guard reads ----

#[test]
fn the_lines_that_move_the_axes_are_told_by_their_codes() {
    for yes in [
        "G0 X1",
        "g1 x1",
        "G00 X1",
        "G01 X1",
        "G2 X1 I1",
        "G3 X1 I1",
        "G28",
        "G30",
        "G31 Z-5",
        "G38.2 Z-5",
        "G38",
        "G81 Z-1",
        "G89 Z-1",
        "N10 G1 X5",
        "  G1 X1  ",
        "G90 G0 X1",
        "G1 (cut) X5",
    ] {
        assert!(line_motion(yes), "{yes}");
        assert!(line_moves(yes), "{yes}");
    }
}

#[test]
fn the_lines_that_do_not_move_the_axes_are_not_motion() {
    for no in [
        "G4 P1",
        "G10 L2",
        "G17",
        "G20",
        "G21",
        "G32",
        "G37",
        "G39",
        "G54",
        "G80",
        "G90",
        "G90.1",
        "G91",
        "G91.1",
        "M5",
        "",
        "X5 Y5",
        "( G1 ) M5",
        "; G1 X1",
    ] {
        assert!(!line_motion(no), "{no}");
    }
}

#[test]
fn the_lines_that_start_something_that_moves_are_not_motion_but_still_move() {
    for yes in ["M3 S1000", "m4 s500", "M03", "M6 T1", "M7", "M8", "T1 M6"] {
        assert!(!line_motion(yes), "{yes}");
        assert!(line_moves(yes), "{yes}");
    }
    for no in [
        "M5",
        "M9",
        "M2",
        "M30",
        "M300",
        "M0",
        "M1",
        "M5 ; M3 later",
        "M5 (M3)",
    ] {
        assert!(!line_moves(no), "{no}");
    }
}

#[test]
fn the_commands_of_the_controller_that_jog_or_home_move_and_the_others_do_not() {
    for yes in ["$J=G91X5F100", "$j=g90x1f100", "$H", "$HZ", " $H "] {
        assert!(line_motion(yes), "{yes}");
        assert!(line_moves(yes), "{yes}");
    }
    for no in ["$X", "$$", "$G", "$#", "$100=250", "$I", "$N", "$"] {
        assert!(!line_moves(no), "{no}");
    }
}
