//! More checks of the pure parts of the program: how a G-code program is read, what is saved to go on after a crash, the settings file as an operator may edit it, the queue of
//! programs, the movements of the views and the command line.

use eh3d_motioncnc::cli::Options;
use eh3d_motioncnc::gcode::{arc_moves, read_preview, toolpath, words, Modal, PREVIEW_MAX_BYTES};
use eh3d_motioncnc::link::{parse_port, GRBL_BAUD};
use eh3d_motioncnc::queue::{Queue, State};
use eh3d_motioncnc::recovery::{due, program_hash, recovery_path, Recovery};
use eh3d_motioncnc::settings::{Settings, RECENT_MAX};
use eh3d_motioncnc::texts::Lang;
use eh3d_motioncnc::view::{Orbit, Plan2d, ORBIT_HOME, ZOOM_MAX, ZOOM_MIN};
use std::path::{Path, PathBuf};

fn lines(s: &str) -> Vec<String> {
    s.lines().map(str::to_string).collect()
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

/// A folder of its own for a test, empty.
fn dir(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("more-{name}"));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

// ---- the path of a program ----

#[test]
fn a_move_is_read_with_no_spaces_in_lower_case_and_with_any_form_of_number() {
    let p = toolpath(&lines(
        "g1x5y-2.5f300\nG1 X+3 Y.5\nG1 X5. Z-.25\nN100 G1 X1",
    ));
    assert_eq!(p[0].to, (5.0, -2.5, 0.0));
    assert_eq!(p[0].feed, 300.0);
    assert_eq!(p[1].to, (3.0, 0.5, 0.0));
    assert_eq!(p[2].to, (5.0, 0.5, -0.25));
    assert_eq!(
        p[3].to,
        (1.0, 0.5, -0.25),
        "a line number is not a coordinate"
    );
    assert_eq!(p.iter().map(|s| s.line).collect::<Vec<_>>(), [0, 1, 2, 3]);
}

#[test]
fn comments_of_both_kinds_hide_what_they_hold_in_any_script() {
    let p = toolpath(&lines(
        "G1 X1 (X99 y99) Y2 ; Z99\n(ñandú 日本語) G1 X3 ; cota ñ\nG1 X4 ; (unclosed\n(open X50\nG1 X6",
    ));
    let ends: Vec<_> = p.iter().map(|s| s.to).collect();
    assert_eq!(ends[0], (1.0, 2.0, 0.0));
    assert_eq!(ends[1], (3.0, 2.0, 0.0));
    assert_eq!(ends[2], (4.0, 2.0, 0.0));
    // a comment that is never closed hides the rest of its line only, and the next line is read again
    assert_eq!(ends[3], (6.0, 2.0, 0.0));
    assert_eq!(p.len(), 4);
}

#[test]
fn rapid_and_cut_are_modal_and_an_arc_ends_with_the_next_straight_move() {
    let p = toolpath(&lines("G1 X5 F100\nX10\nG0\nX20\nG2 X30 I5\nX40\nG1 X50"));
    assert_eq!(
        p.iter().map(|s| (s.rapid, s.arc)).collect::<Vec<_>>(),
        [
            (false, false),
            (false, false),
            (true, false),
            (false, true),
            (false, true),
            (false, false)
        ]
    );
}

#[test]
fn inches_and_incremental_moves_mix_in_any_order() {
    let p = toolpath(&lines("G91 G20\nG1 X1\nG21\nG1 X1\nG90\nG1 X2\nG20 G1 Y1"));
    assert!(near(p[0].to.0, 25.4));
    assert!(near(p[1].to.0, 26.4));
    assert!(near(p[2].to.0, 2.0));
    assert!(near(p[3].to.1, 25.4));
    // the same program in inches and in millimetres draws the same path
    let a = toolpath(&lines("G20\nG1 X1 Y2 Z-0.5 F10"));
    let b = toolpath(&lines("G21\nG1 X25.4 Y50.8 Z-12.7 F254"));
    assert_eq!((a[0].to, a[0].feed), (b[0].to, b[0].feed));
}

#[test]
fn a_move_that_changes_nothing_is_not_a_segment_and_a_move_in_z_alone_is() {
    assert!(toolpath(&lines("G1 X0\nG91\nG1 X0 Y0 Z0\nG0 X0")).is_empty());
    let p = toolpath(&lines("G0 Z5\nG0 Z5\nG0 Z-1"));
    assert_eq!(p.len(), 2);
    assert_eq!((p[1].from.2, p[1].to.2), (5.0, -1.0));
}

#[test]
fn the_words_of_a_line_drop_what_has_no_number_and_split_a_run_together() {
    assert_eq!(words("G1X5Y-2"), vec![('G', 1.0), ('X', 5.0), ('Y', -2.0)]);
    assert_eq!(words("G X- Y 3"), vec![('Y', 3.0)]);
    assert_eq!(words("M3S12000"), vec![('M', 3.0), ('S', 12000.0)]);
    assert!(words("").is_empty());
    assert!(words("(everything) ; is a comment").is_empty());
    // a repeated letter is kept, in order: the program decides which one counts
    assert_eq!(words("X1 X2"), vec![('X', 1.0), ('X', 2.0)]);
}

#[test]
fn an_arc_counts_for_each_line_that_moves_while_the_mode_is_on() {
    assert_eq!(arc_moves(&lines("G2 X1\nX2\nX3 Y1\nG1 X4\nX5")), 3);
    assert_eq!(
        arc_moves(&lines("G3 F100\nG3 M3\nS1000")),
        0,
        "no coordinate, no move"
    );
    assert_eq!(arc_moves(&lines("G2 R5\nG0 X1\nG3 I1")), 2);
    assert_eq!(arc_moves(&[]), 0);
}

#[test]
fn the_modal_state_ignores_what_makes_no_sense_and_forgets_on_the_end_of_the_program() {
    let mut m = Modal::default();
    m.apply("F0 S-5 F-3");
    assert_eq!((m.feed, m.speed), (None, None));
    m.apply("S0");
    assert_eq!(m.speed, Some(0.0), "a speed of zero is a speed");
    m.apply("F250.5 M4 M7");
    assert_eq!((m.feed, m.spindle, m.coolant), (Some(250.5), 4, 7));
    m.apply("M9 M5");
    assert_eq!((m.spindle, m.coolant), (5, 9));
    m.apply("G1 G20 G91");
    m.apply("M3 M8 M2");
    assert_eq!((m.spindle, m.coolant), (5, 9));
    assert!(
        m.inches && !m.absolute && m.motion == 1,
        "the end of the program does not change the units"
    );
    // 12 G codes and 3 M codes are shown, always in the same order
    assert_eq!(m.g_codes().len(), 12);
    assert_eq!(m.m_codes().len(), 3);
}

#[test]
fn applying_the_same_line_twice_is_the_same_as_once() {
    for line in [
        "G91 G20 F100 S500 M3 M8",
        "G0 G90",
        "M30",
        "(none) ; nothing",
    ] {
        let mut once = Modal::default();
        once.apply(line);
        let mut twice = once.clone();
        twice.apply(line);
        assert_eq!(once, twice, "{line}");
    }
}

#[test]
fn the_preview_of_a_file_reads_any_code_page_and_both_line_endings() {
    let d = dir("preview");
    let lf = d.join("lf.nc");
    let crlf = d.join("crlf.nc");
    std::fs::write(&lf, "G0 X-5 Y-5\nG1 X10 Y20 F100\n").unwrap();
    std::fs::write(&crlf, "G0 X-5 Y-5\r\nG1 X10 Y20 F100\r\n").unwrap();
    let (a, b) = (read_preview(&lf).unwrap(), read_preview(&crlf).unwrap());
    assert_eq!(a, b);
    assert_eq!(a.bounds, Some([-5.0, -5.0, 10.0, 20.0]));
    // bytes that are not UTF-8 in a comment and a NUL in the middle do not stop the reading
    let odd = d.join("odd.nc");
    std::fs::write(&odd, b"G0 X1 (cota \xF1\xFF)\nG1 X2 \x00 Y3\nG1 X4\n").unwrap();
    assert_eq!(read_preview(&odd).unwrap().moves, 3);
    // the biggest file allowed is read and one byte more is not
    let edge = d.join("edge.nc");
    std::fs::write(&edge, vec![b'\n'; PREVIEW_MAX_BYTES as usize]).unwrap();
    assert!(read_preview(&edge).is_some());
    let empty = d.join("empty.nc");
    std::fs::write(&empty, b"").unwrap();
    let pv = read_preview(&empty).unwrap();
    assert_eq!((pv.moves, pv.bounds), (0, None));
}

// ---- going on after a crash ----

fn recovery() -> Recovery {
    Recovery {
        file: "C:\\trabajos\\pieza ñandú=1.nc".into(),
        next: 41,
        total: 100,
        job: "job-7".into(),
        sha256: program_hash(&lines("G0 X1\nG1 X2")),
    }
}

#[test]
fn what_is_saved_is_what_is_read_back_whatever_the_name_of_the_file_is() {
    for file in [
        "a.nc",
        "dir with spaces/日本語 ñ.nc",
        "x=y.nc",
        "  padded  .nc",
    ] {
        let r = Recovery {
            file: file.into(),
            ..recovery()
        };
        assert_eq!(Recovery::parse(&r.to_text()), Some(r), "{file}");
    }
    let d = dir("recovery");
    let path = recovery_path(&d.join("motioncnc.cfg"));
    assert_eq!(path.file_name().unwrap(), "recovery.cfg");
    recovery().save(&path).unwrap();
    assert_eq!(Recovery::load(&path), Some(recovery()));
    // saving again replaces it and leaves no side file
    let later = Recovery {
        next: 77,
        ..recovery()
    };
    later.save(&path).unwrap();
    assert_eq!(Recovery::load(&path).unwrap().next, 77);
    assert_eq!(std::fs::read_dir(&d).unwrap().count(), 1);
    Recovery::clear(&path);
    Recovery::clear(&path);
    assert_eq!(Recovery::load(&path), None);
}

#[test]
fn a_line_break_in_a_name_cannot_add_a_key_to_the_file() {
    let r = Recovery {
        file: "x.nc\nnext=0\njob=evil".into(),
        ..recovery()
    };
    let back = Recovery::parse(&r.to_text()).unwrap();
    assert_eq!(back.next, 41);
    assert_eq!(back.job, "job-7");
    assert!(back.file.starts_with("x.nc"));
}

#[test]
fn a_file_that_is_cut_old_or_from_another_program_is_not_a_recovery() {
    for text in [
        "",
        "hello",
        "file=\nnext=1\ntotal=5\n",
        "next=1\ntotal=5\n",
        "file=a.nc\nnext=5\ntotal=5\n",
        "file=a.nc\nnext=6\ntotal=5\n",
        "file=a.nc\nnext=1\ntotal=0\n",
        "file=a.nc\nnext=x\ntotal=5\n",
        "file=a.nc\nnext=1\ntotal=-5\n",
        "file=a.nc\nnext=-1\ntotal=5\n",
        "file=a.nc\nnext=1\n",
    ] {
        assert_eq!(Recovery::parse(text), None, "{text:?}");
    }
    // unknown keys, blank lines, Windows line ends and a later key that repeats an earlier one
    let ok =
        Recovery::parse("\r\nfile=a.nc\r\nflavour=none\r\nnext=1\r\n\r\ntotal=5\r\nnext=3\r\n")
            .unwrap();
    assert_eq!(
        (
            ok.file.as_str(),
            ok.next,
            ok.total,
            ok.job.as_str(),
            ok.sha256.as_str()
        ),
        ("a.nc", 3, 5, "", "")
    );
}

#[test]
fn the_hash_of_a_program_changes_with_one_character_and_with_the_order() {
    assert_eq!(
        program_hash(&[]),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    let a = program_hash(&lines("G0 X1\nG1 X2"));
    assert_eq!(a, program_hash(&lines("G0 X1\nG1 X2")));
    assert_ne!(a, program_hash(&lines("G0 X1\nG1 X3")));
    assert_ne!(a, program_hash(&lines("G1 X2\nG0 X1")));
    assert_ne!(a, program_hash(&lines("G0 X1\nG1 X2\n\n")));
    assert_eq!(a.len(), 64);
    assert!(a
        .chars()
        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
}

#[test]
fn it_is_time_to_save_after_enough_lines_in_either_direction_and_always_the_first_time() {
    assert!(due(None, 0, 1000));
    assert!(!due(Some(100), 100, 50));
    assert!(!due(Some(100), 149, 50));
    assert!(due(Some(100), 150, 50));
    assert!(due(Some(100), 50, 50), "the program went back");
    assert!(due(Some(7), 7, 0));
}

// ---- the settings file ----

#[test]
fn what_an_operator_edits_by_hand_is_read_with_care() {
    let s = Settings::parse(
        "  FEED = 800 \nstep=0\nSpindle = abc\nunits = INCH\nlang = xx\ntheme = nothing\n# feed = 5\njust text\n=novalue\nrapid = 0\nsummary_tiny = -1\nresume_safe_z = 1e9\n",
    );
    let d = Settings::default();
    assert_eq!(s.feed_mm_min, 800.0);
    assert_eq!(s.step_mm, d.step_mm, "a step of zero is refused");
    assert_eq!(s.spindle_rpm, d.spindle_rpm);
    assert!(s.inches);
    assert_eq!((s.lang, s.theme), (d.lang, d.theme));
    assert_eq!(s.rapid_mm_min, d.rapid_mm_min);
    assert_eq!(s.summary_tiny_mm, d.summary_tiny_mm);
    assert_eq!(s.resume_safe_z_mm, d.resume_safe_z_mm);
}

#[test]
fn every_number_has_its_range_and_the_edges_are_inside() {
    for (key, lo, hi, get) in [
        (
            "step",
            0.001,
            1000.0,
            (|s: &Settings| s.step_mm) as fn(&Settings) -> f64,
        ),
        ("feed", 1.0, 100_000.0, |s| s.feed_mm_min),
        ("spindle", 0.0, 1_000_000.0, |s| s.spindle_rpm),
        ("rapid", 1.0, 1_000_000.0, |s| s.rapid_mm_min),
        ("resume_spindle_wait", 0.0, 120.0, |s| {
            s.resume_spindle_wait_s
        }),
        ("summary_margin", 0.0, 1000.0, |s| s.summary_margin_mm),
    ] {
        assert_eq!(
            get(&Settings::parse(&format!("{key} = {lo}"))),
            lo,
            "{key} low edge"
        );
        assert_eq!(
            get(&Settings::parse(&format!("{key} = {hi}"))),
            hi,
            "{key} high edge"
        );
        let def = get(&Settings::default());
        for bad in [lo - 0.0001 * lo.abs().max(1.0), hi * 1.001 + 1.0] {
            assert_eq!(
                get(&Settings::parse(&format!("{key} = {bad}"))),
                def,
                "{key} = {bad}"
            );
        }
        for bad in ["nan", "inf", "-inf", "1,5", ""] {
            assert_eq!(
                get(&Settings::parse(&format!("{key} = {bad}"))),
                def,
                "{key} = {bad:?}"
            );
        }
    }
}

#[test]
fn the_travel_needs_a_maximum_above_the_minimum_and_two_numbers() {
    let d = Settings::default();
    for bad in [
        "5", "5,5", "9,1", "a,b", "1,", ",5", "nan,5", "0,inf", "1,2,3",
    ] {
        let s = Settings::parse(&format!("travel_y = {bad}"));
        assert_eq!(s.travel[1], d.travel[1], "{bad}");
    }
    let s = Settings::parse("travel_x = -10 , 410.5\ntravel_z=-80,20\ntravel_w = 1,2");
    assert_eq!((s.travel[0], s.travel[2]), ((-10.0, 410.5), (-80.0, 20.0)));
    assert_eq!(s.travel[1], d.travel[1]);
}

#[test]
fn a_switch_is_off_for_no_off_false_and_zero_in_any_case() {
    for off in ["no", "off", "false", "0", " no ", "No", "OFF", "False"] {
        let s = Settings::parse(&format!(
            "show_log = {off}\nconfirm_start = {off}\nreject_out_of_travel = {off}"
        ));
        assert!(
            !s.show_log && !s.confirm_start && !s.reject_out_of_travel,
            "{off:?} must be off"
        );
    }
    for on in ["yes", "on", "true", "1", "anything"] {
        let s = Settings::parse(&format!("show_log = {on}"));
        assert!(s.show_log, "{on:?}");
    }
}

#[test]
fn a_file_saved_by_a_text_editor_with_a_byte_order_mark_is_read_from_its_first_line() {
    let s = Settings::parse("\u{feff}lang = de\ntheme = neon\n");
    assert_eq!(s.lang, Lang::De);
}

#[test]
fn nothing_written_to_the_file_can_add_a_line_to_it() {
    // the keys that the file writes before these values are the ones a line break could overwrite
    let s = Settings {
        last_dir: "C:\\jobs\nshow_log = no\nunits = in\nfeed = 7".into(),
        recent: vec!["a.nc".into(), "b.nc\nshow_program = no".into()],
        server_address: "host:1\nserver_token = stolen".into(),
        server_token: "tok\r\nshow_log = no".into(),
        ..Default::default()
    };
    let back = Settings::parse(&s.to_text());
    assert!(back.show_log && back.show_program && !back.inches);
    assert_eq!(back.feed_mm_min, Settings::default().feed_mm_min);
    assert_eq!(back.server_token, "tok  show_log = no");
    assert_eq!(back.server_address, "host:1 server_token = stolen");
    assert_eq!(back.recent.len(), 2);
    assert!(!back.recent.iter().any(|r| r.contains('\n')));
    assert!(!back.last_dir.contains('\n') && back.last_dir.starts_with("C:\\jobs"));
    assert_eq!(back.server_machine, "");
}

#[test]
fn every_setting_survives_a_round_trip_even_the_odd_ones() {
    let s = Settings {
        lang: Lang::Ja,
        inches: true,
        travel: [(-1.5, 2.5), (-300.0, 0.25), (-1000.0, 1000.0)],
        step_mm: 0.001,
        feed_mm_min: 99_999.5,
        spindle_rpm: 0.0,
        show_buttons: false,
        show_program: false,
        show_log: false,
        show_rapids: false,
        recent: vec!["日本語.nc".into(), "ñandú.nc".into()],
        last_dir: "/home/ana/trabajos ñ".into(),
        server_address: "10.0.0.2:18200".into(),
        server_token: "t".into(),
        server_machine: "bungard".into(),
        server_restore_outputs: true,
        resume_safe_z_mm: -2.5,
        resume_spindle_wait_s: 0.0,
        confirm_start: false,
        reject_out_of_travel: false,
        rapid_mm_min: 1.0,
        summary_tiny_mm: 0.0,
        summary_far_mm: 1_000_000.0,
        summary_margin_mm: 1000.0,
        shortcuts: false,
        show_toolbar: false,
        open_preview: false,
        view_3d: true,
        read_controller_travel: false,
        ..Settings::default()
    };
    assert_eq!(Settings::parse(&s.to_text()), s);
    // and writing what was read is writing the same text
    assert_eq!(Settings::parse(&s.to_text()).to_text(), s.to_text());
    assert_eq!(
        Settings::parse(&Settings::default().to_text()),
        Settings::default()
    );
}

#[test]
fn the_recent_files_keep_the_newest_first_without_repeats_and_no_more_than_the_limit() {
    let mut s = Settings::default();
    for i in 0..(RECENT_MAX + 4) {
        s.remember_recent(&format!("f{i}.nc"));
    }
    assert_eq!(s.recent.len(), RECENT_MAX);
    assert_eq!(s.recent[0], format!("f{}.nc", RECENT_MAX + 3));
    let last = s.recent[RECENT_MAX - 1].clone();
    s.remember_recent(&last);
    assert_eq!(s.recent[0], last);
    assert_eq!(s.recent.len(), RECENT_MAX);
    assert_eq!(s.recent.iter().filter(|p| **p == last).count(), 1);
    // a file written by hand with too many, blank or padded entries is cut and cleaned
    let many: Vec<String> = (0..20).map(|i| format!(" g{i}.nc ")).collect();
    let p = Settings::parse(&format!("recent = {}||  |", many.join("|")));
    assert_eq!(p.recent.len(), RECENT_MAX);
    assert_eq!(p.recent[0], "g0.nc");
}

#[test]
fn a_missing_or_unreadable_file_gives_the_defaults_and_saving_makes_the_folder() {
    let d = dir("settings");
    assert_eq!(Settings::load(&d.join("nothing.cfg")), Settings::default());
    let bad = d.join("bad.cfg");
    std::fs::write(&bad, [0xFFu8, 0xFE, 0x00, 0x80]).unwrap();
    assert_eq!(Settings::load(&bad), Settings::default());
    let nested = d.join("a").join("b").join("motioncnc.cfg");
    let s = Settings {
        feed_mm_min: 1234.0,
        ..Settings::default()
    };
    s.save(&nested).unwrap();
    assert_eq!(Settings::load(&nested).feed_mm_min, 1234.0);
}

// ---- the queue ----

fn queue(n: usize) -> Queue {
    Queue::new((0..n).map(|i| PathBuf::from(format!("dir/p{i}.nc"))))
}

fn states(q: &Queue) -> Vec<State> {
    q.items.iter().map(|i| i.state).collect()
}

#[test]
fn the_programs_run_in_order_and_a_failure_stops_the_queue_until_it_is_resumed() {
    let mut q = queue(3);
    assert!(!q.active && q.current().is_none() && !q.finished() && !q.halted());
    assert_eq!(q.begin_next(), Some(0));
    assert_eq!(q.begin_next(), None, "one is loaded");
    assert_eq!(q.finish(true), Some(0));
    assert!(q.active, "it goes on by itself");
    assert_eq!(q.begin_next(), Some(1));
    assert_eq!(q.finish(false), Some(1));
    assert!(!q.active && q.halted());
    assert_eq!(states(&q), [State::Done, State::Failed, State::Waiting]);
    q.resume();
    assert!(q.active && !q.halted());
    assert_eq!(q.begin_next(), Some(2));
    assert_eq!(q.finish(true), Some(2));
    assert!(q.finished() && q.begin_next().is_none() && q.finish(true).is_none());
}

#[test]
fn nothing_is_resumed_when_nothing_waits_and_an_empty_queue_is_never_finished() {
    let mut q = queue(1);
    q.begin_next();
    q.finish(false);
    q.resume();
    assert!(!q.active);
    assert!(q.finished());
    assert!(!Queue::default().finished() && Queue::default().is_empty());
}

#[test]
fn only_what_waits_can_be_moved_and_what_is_done_or_loaded_keeps_its_place() {
    let mut q = queue(5);
    q.begin_next();
    q.finish(true);
    q.begin_next();
    // p0 done, p1 loaded, p2 p3 p4 wait
    assert!(
        !q.move_up(0) && !q.move_up(1) && !q.move_up(2),
        "p2 would pass the loaded one"
    );
    assert!(!q.move_down(1) && !q.move_down(4) && !q.move_down(9));
    assert!(q.move_up(3) && q.name_of(2) == "p3.nc" && q.name_of(3) == "p2.nc");
    assert!(q.move_down(2) && q.name_of(3) == "p3.nc");
    assert_eq!(states(&q)[..2], [State::Done, State::Running]);
}

#[test]
fn removing_and_skipping_follow_the_state_of_the_program() {
    let mut q = queue(4);
    q.begin_next();
    assert!(!q.remove(0), "the loaded one cannot be taken out");
    assert!(!q.remove(9));
    assert!(q.remove(3));
    assert!(q.skip(2) && q.items[2].state == State::Skipped);
    assert!(!q.skip(2), "a skipped one is skipped");
    assert!(q.skip(0), "the loaded one is given up");
    assert_eq!(q.current(), None);
    assert!(!q.skip(7));
    assert_eq!(q.waiting(), 1);
    q.begin_next();
    q.finish(true);
    assert!(!q.skip(1), "a program that is done is not skipped");
    assert!(q.finished());
}

#[test]
fn clearing_keeps_the_loaded_program_and_turns_the_queue_off_when_nothing_is_left() {
    let mut q = queue(3);
    q.begin_next();
    q.clear();
    assert_eq!(q.items.len(), 1);
    assert!(q.active, "the loaded program goes on");
    q.finish(true);
    q.clear();
    assert!(q.is_empty() && !q.active);
}

#[test]
fn a_name_is_the_file_without_folders_and_a_missing_place_is_empty() {
    let q = Queue::new([PathBuf::from("a/b/ñandú 日本.nc"), PathBuf::from("..")]);
    assert_eq!(q.name_of(0), "ñandú 日本.nc");
    assert_eq!(q.name_of(1), "..");
    assert_eq!(q.name_of(5), "");
}

// ---- the views ----

#[test]
fn zooming_in_at_the_limit_does_not_drag_the_view_and_a_wheel_back_and_forth_returns() {
    let mut v = Plan2d::default();
    let (centre, cursor) = ((400.0, 300.0), (500.0, 250.0));
    for _ in 0..100 {
        v.wheel(1000.0, cursor, centre);
    }
    assert_eq!(v.zoom, ZOOM_MAX);
    let pinned = v;
    v.wheel(500.0, (10.0, 10.0), centre);
    assert_eq!(v.zoom, pinned.zoom);
    assert!(
        (v.pan.0 - pinned.pan.0).abs() < 1e-6 && (v.pan.1 - pinned.pan.1).abs() < 1e-6,
        "a wheel past the limit changes nothing: {v:?} / {pinned:?}"
    );
    let mut w = Plan2d::default();
    w.wheel(240.0, cursor, centre);
    w.wheel(-240.0, cursor, centre);
    assert!(
        near(w.zoom, 1.0) && near(w.pan.0, 0.0) && near(w.pan.1, 0.0),
        "{w:?}"
    );
    // the point under the pointer is the same at every zoom
    let mut z = Plan2d::default();
    let before = z.apply(cursor, centre);
    for lines in [120.0, -400.0, 90.0, 600.0] {
        z.wheel(lines, cursor, centre);
        let after = z.apply(cursor, centre);
        assert!(
            near(before.0, after.0) && near(before.1, after.1),
            "{lines}: {after:?}"
        );
    }
    assert!(z.zoom >= ZOOM_MIN && z.zoom <= ZOOM_MAX);
}

#[test]
fn the_orbit_never_goes_under_the_table_nor_over_the_top_and_keeps_its_turn_in_a_circle() {
    let mut o = Orbit::default();
    assert!(o.is_home() && o == ORBIT_HOME);
    o.drag(0.0, 100_000.0);
    assert_eq!(o.pitch_deg, 89.0);
    o.drag(0.0, -100_000.0);
    assert_eq!(o.pitch_deg, 5.0);
    for dx in [1.0, -1000.0, 123_456.0, -0.5] {
        o.drag(dx, 0.0);
        assert!((0.0..360.0).contains(&o.yaw_deg), "{dx}: {}", o.yaw_deg);
    }
    o.wheel(1e9);
    assert_eq!(o.zoom, ZOOM_MAX);
    o.wheel(-1e9);
    assert_eq!(o.zoom, ZOOM_MIN);
    o.reset();
    assert!(o.is_home());
}

// ---- the command line and the port ----

fn opts(s: &str) -> Result<Options, String> {
    Options::parse(s.split_whitespace().map(String::from))
}

#[test]
fn the_command_line_takes_the_first_file_as_the_program_and_the_rest_as_the_queue() {
    let o = opts("a.nc b.nc --view 3d c.nc").unwrap();
    assert_eq!(o.file, Some(PathBuf::from("a.nc")));
    assert_eq!(o.more, [PathBuf::from("b.nc"), PathBuf::from("c.nc")]);
    assert_eq!(o.view_3d, Some(true));
    assert_eq!(opts("--view plan").unwrap().view_3d, Some(false));
    assert_eq!(opts("--view 2d").unwrap().view_3d, Some(false));
    assert_eq!(opts("").unwrap().wait, 6);
    assert!(opts("--ports").unwrap().ports);
}

#[test]
fn the_command_line_names_what_it_cannot_read() {
    for (args, expect) in [
        ("--lang", "--lang needs a value"),
        ("--lang xx", "unknown language 'xx'"),
        ("--theme", "needs a value"),
        ("--theme blue-ish", "unknown theme"),
        ("--wait many", "needs a number"),
        ("--wait -3", "needs a number"),
        ("--view side", "unknown view 'side'"),
        ("--grbl", "needs a value"),
        ("--frobnicate", "unknown option '--frobnicate'"),
        ("--shot", "--shot needs a value"),
        ("--lang --theme", "unknown language '--theme'"),
    ] {
        let e = opts(args).unwrap_err();
        assert!(e.contains(expect), "{args}: {e}");
    }
}

#[test]
fn a_port_is_a_name_with_an_optional_baud_rate_inside_the_limits() {
    assert_eq!(parse_port("COM3"), Ok(("COM3".into(), GRBL_BAUD)));
    assert_eq!(
        parse_port("/dev/ttyUSB0@250000"),
        Ok(("/dev/ttyUSB0".into(), 250_000))
    );
    assert_eq!(parse_port("a@b@9600"), Ok(("a@b".into(), 9600)));
    assert_eq!(parse_port("COM3@1200").unwrap().1, 1200);
    assert_eq!(parse_port("COM3@2000000").unwrap().1, 2_000_000);
    for (text, expect) in [
        ("", "empty"),
        ("@115200", "empty"),
        ("COM3@", "is not a baud rate"),
        ("COM3@fast", "is not a baud rate"),
        ("COM3@-5", "is not a baud rate"),
        ("COM3@1199", "out of range"),
        ("COM3@2000001", "out of range"),
        ("COM3@0", "out of range"),
    ] {
        let e = parse_port(text).unwrap_err();
        assert!(e.contains(expect), "{text:?}: {e}");
    }
}
