//! Tests that throw many made-up programs at every analysis of a program, from a fixed seed so a failure comes back the same, and a test
//! that every message of the newer checks is raised by some program (so none is written and never reached). The programs are random
//! mixes of the codes, the words and the numbers of G-code, including the mistakes: a number with no word, a word with no number, a huge
//! number, a comment that never closes, text of other alphabets and the lines meant for the controller.

use std::collections::HashSet;

use crate::lint::LintCfg;
use crate::progcheck::Finding;
use crate::report::{build, notes, Cfg};
use crate::tools::ToolTable;
use crate::tuning::Tuning;
use crate::words::{Msg, LANGS};

/// The generator: xorshift64*, so the same seed gives the same programs on every computer.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[self.below(items.len())]
    }
}

const CODES: &[&str] = &[
    "G0",
    "G1",
    "G2",
    "G3",
    "G4",
    "G10 L2 P1",
    "G10 L20 P0",
    "G10 L2 P9",
    "G17",
    "G18",
    "G19",
    "G20",
    "G21",
    "G28",
    "G38.2",
    "G40",
    "G41",
    "G42",
    "G43.1",
    "G49",
    "G53",
    "G54",
    "G55",
    "G56",
    "G59",
    "G61",
    "G73",
    "G80",
    "G81",
    "G82",
    "G83",
    "G84",
    "G85",
    "G86",
    "G87",
    "G89",
    "G90",
    "G91",
    "G90.1",
    "G92",
    "G98",
    "G99",
    "M0",
    "M2",
    "M3",
    "M4",
    "M5",
    "M6",
    "M7",
    "M8",
    "M9",
    "M30",
    "M112",
    "T1",
    "T2",
    "T9",
    "D1",
    "D2",
    "(a comment)",
    "; a note",
    "(never closed",
    "$H",
    "%",
    "G",
    "X",
    "10",
    "Ж 5",
    "木 3",
    "g1 x5",
];

const LETTERS: &[&str] = &[
    "X", "Y", "Z", "I", "J", "K", "R", "Q", "P", "F", "S", "L", "H",
];

fn number(r: &mut Rng) -> String {
    match r.below(12) {
        0 => "0".to_string(),
        1 => "-0".to_string(),
        2 => format!("{}", r.below(50)),
        3 => format!("-{}", r.below(50)),
        4 => format!("{}.{}", r.below(100), r.below(1000)),
        5 => format!("-{}.{}", r.below(100), r.below(1000)),
        6 => format!("{}", r.below(100_000)),
        7 => "0.0001".to_string(),
        8 => "9".repeat(1 + r.below(30)),
        // longer than any f64 holds: parses as infinity
        9 if r.below(8) == 0 => "9".repeat(400),
        10 => ".5".to_string(),
        _ => format!("{}", r.below(1000) as f64 / 8.0),
    }
}

fn line(r: &mut Rng) -> String {
    let mut parts: Vec<String> = Vec::new();
    for _ in 0..1 + r.below(5) {
        if r.below(3) == 0 {
            parts.push(r.pick(CODES).to_string());
        } else if r.below(10) == 0 {
            parts.push(number(r));
        } else {
            parts.push(format!("{}{}", r.pick(LETTERS), number(r)));
        }
    }
    let sep = if r.below(4) == 0 { "" } else { " " };
    parts.join(sep)
}

fn program(r: &mut Rng) -> Vec<String> {
    (0..r.below(40)).map(|_| line(r)).collect()
}

fn cfg(lang: &str) -> Cfg {
    Cfg {
        travel: [(0.0, 300.0), (0.0, 200.0), (-50.0, 0.0)],
        offset: [0.0; 3],
        rapid_mm_min: 6000.0,
        default_feed: 300.0,
        lang: lang.to_string(),
    }
}

fn tuning() -> Tuning {
    Tuning {
        tools: ToolTable::parse(
            "tool = 1 | diameter=2 | life=60 | used=10\ntool = 2 | diameter=6 | life=120",
        ),
        material: "aluminium".to_string(),
        ..Tuning::default()
    }
}

fn all_lines_are_inside(f: &[Finding], len: usize) -> bool {
    f.iter().all(|f| f.line.is_none_or(|l| l < len.max(1)))
}

/// Runs every analysis on a program and checks what must hold of any program. Returns the report, so two runs can be compared.
fn run(lines: &[String], lang: &str) -> String {
    let t = tuning();
    let l = LintCfg::default();
    let c = cfg(lang);
    let r = build(lines, &t, &l, &c);
    assert!(r.text.ends_with('\n') && !r.text.trim().is_empty());
    assert!(r.problems <= r.text.lines().count());
    for n in notes(lines, &t, &l, &c) {
        assert!(n.line.is_none_or(|x| x < lines.len().max(1)), "{lines:?}");
        assert!(!n.text.contains('{'), "{lang}: {}", n.text);
    }
    // the canned cycles
    let e = crate::cycles::expand(lines);
    assert_eq!(e.origin.len(), e.lines.len());
    assert!(e.origin.windows(2).all(|w| w[0] <= w[1]), "{lines:?}");
    assert!(e.origin.iter().all(|&o| o < lines.len().max(1)));
    assert!(all_lines_are_inside(&e.findings, lines.len()));
    if !crate::cycles::has_cycles(lines) {
        assert_eq!(e.lines, lines, "a program with no cycle is not changed");
        assert_eq!(e.holes, 0);
    }
    // the cutter compensation
    assert!(all_lines_are_inside(
        &crate::cutcomp::findings(lines, &t.tools),
        lines.len()
    ));
    let path = crate::gcode::toolpath(lines);
    let finite = path.iter().all(|s| {
        [s.from, s.to]
            .iter()
            .all(|p| p.0.is_finite() && p.1.is_finite() && p.2.is_finite())
    });
    if finite {
        let comp = crate::cutcomp::compensated(lines, &t.tools);
        for s in comp {
            // a very sharp corner may send a joint far away, never to a number that is not one
            assert!(s.from.0.is_finite() && s.to.1.is_finite(), "{lines:?}");
        }
    }
    // the work coordinate systems
    let w = crate::wcs::scan(lines);
    assert!(all_lines_are_inside(&w.findings, lines.len()));
    assert!(w.used.windows(2).all(|p| p[0].system < p[1].system));
    assert!(w.used.iter().all(|u| u.moves > 0));
    // the help of the words of every line
    for text in lines {
        for h in crate::gcodehelp::for_line(text, lang) {
            assert!(!h.trim().is_empty());
        }
    }
    // the same lines as a macro
    let mut lib = String::from("[fuzz]\n");
    for text in lines {
        lib.push_str(text);
        lib.push('\n');
    }
    let (said, _) = crate::macrocheck::validate_text(&lib, &t, lang);
    assert!(!said.contains('{'), "{lang}: {said}");
    r.text
}

#[test]
fn a_thousand_made_up_programs_pass_through_every_analysis_without_a_panic() {
    for seed in [
        0x5EED_0001u64,
        0x5EED_0002,
        0x5EED_0003,
        0x1234_5678_9ABC_DEF1,
    ] {
        let mut r = Rng(seed);
        for i in 0..250 {
            let p = program(&mut r);
            run(&p, LANGS[i % LANGS.len()]);
        }
    }
}

#[test]
fn the_same_seed_gives_the_same_programs_and_the_same_reports() {
    let make = || {
        let mut r = Rng(0xC0FF_EE01);
        (0..60)
            .map(|i| {
                let p = program(&mut r);
                (p.clone(), run(&p, LANGS[i % LANGS.len()]))
            })
            .collect::<Vec<_>>()
    };
    let (a, b) = (make(), make());
    assert_eq!(a, b);
    // and the generator really makes different programs
    let distinct: HashSet<&Vec<String>> = a.iter().map(|(p, _)| p).collect();
    assert!(distinct.len() > 50, "{}", distinct.len());
    assert!(a.iter().any(|(p, _)| p.is_empty()) || a.iter().any(|(p, _)| p.len() > 20));
}

#[test]
fn the_generator_keeps_its_first_numbers() {
    // a fixed seed is only worth something if the numbers never change under it
    let mut r = Rng(1);
    let first: Vec<u64> = (0..3).map(|_| r.next()).collect();
    assert_eq!(
        first,
        [
            5_180_492_295_206_395_165,
            12_380_297_144_915_551_517,
            13_389_498_078_930_870_103
        ]
    );
}

#[test]
fn programs_of_pure_junk_are_measured_and_reported() {
    let junk = [
        "",
        "G",
        "((((",
        "))))",
        "X",
        "XYZ",
        "G999 M999",
        "G81 G81 G81 X Y Z R Q P F",
        "G41 G42 G40 G41",
        "G10 L2 P1 L20 P2",
        "木 Ж ß",
        "G1 X99999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999 F100",
    ];
    for l in junk {
        let p = vec![l.to_string(); 3];
        for lang in LANGS {
            run(&p, lang);
        }
        run(&[], "en");
    }
}

/// The messages of the newer checks, each with a program that raises it.
fn raised(lines: &[&str], t: &Tuning) -> HashSet<Msg> {
    let p: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
    let mut seen: HashSet<Msg> = HashSet::new();
    if crate::cycles::has_cycles(&p) {
        seen.extend(crate::cycles::expand(&p).findings.iter().map(|f| f.msg));
    }
    seen.extend(crate::cutcomp::findings(&p, &t.tools).iter().map(|f| f.msg));
    seen.extend(crate::wcs::findings(&p).iter().map(|f| f.msg));
    seen
}

#[test]
fn every_message_of_the_newer_checks_is_raised_by_some_program() {
    let t = Tuning {
        tools: ToolTable::parse("tool = 1 | diameter=2\ntool = 2 | diameter=8"),
        ..Tuning::default()
    };
    let programs: [&[&str]; 14] = [
        &["G81 X1 Y1 R2 F10", "G80"],
        &["G81 X1 Y1 Z-1 F10", "G80"],
        &["G81 X1 Y1 Z-1 R-3 F10", "G80"],
        &["G83 X1 Y1 Z-5 R1 F10", "G80"],
        &["G91", "G81 X1 Y1 Z-1 R1 F10", "G80"],
        &["G87 X1 Y1 Z-1 R2 F10", "G80"],
        &["G0 Z5", "G81 X1 Y1 Z-1 R2 F10", "M30"],
        &["T1 M6", "G0 X0 Y0", "G41 D1", "G0 X5", "G40", "M30"],
        &[
            "T1 M6",
            "G1 F5",
            "G41",
            "G1 X5",
            "G3 X10 Y5 I0 J5",
            "G40 G2 X15 Y0 I5 J-5",
            "M30",
        ],
        &["T1 M6", "G1 F5", "G41", "G1 X5", "M30"],
        &[
            "T2 M6",
            "G1 F5",
            "G41",
            "G1 X5",
            "G3 X6 Y1 I0 J1",
            "G40",
            "M30",
        ],
        &[
            "T1 M6", "G1 F5", "G41", "G1 X10", "G1 Y0.5", "G1 X0", "G40", "M30",
        ],
        &["T7 M6", "G1 F5", "G41", "G1 X5", "G40"],
        &[
            "G10 L2 P2 X1",
            "G10 L20 P0 X0",
            "G10 L2 P9 X1",
            "G0 X1",
            "G1 X2 F5",
            "G10 L2 P1 X5",
            "G56",
            "G0 X1",
            "G55",
            "G1 X9 F5",
            "G54",
            "G0 X0",
        ],
    ];
    let mut seen: HashSet<Msg> = HashSet::new();
    for p in programs {
        seen.extend(raised(p, &t));
    }
    for m in [
        Msg::CycleNoZ,
        Msg::CycleNoR,
        Msg::CycleRBelowZ,
        Msg::CycleNoQ,
        Msg::CycleRelative,
        Msg::CycleUnsupported,
        Msg::CycleOpen,
        Msg::CompOnNotLinear,
        Msg::CompOffNotLinear,
        Msg::CompStillOn,
        Msg::CompArcSmall,
        Msg::CompGouge,
        Msg::CompNoDiameter,
        Msg::WcsWrite,
        Msg::WcsWriteHere,
        Msg::WcsBadP,
        Msg::WcsWriteAfterUse,
        Msg::WcsUnset,
        Msg::WcsSwitchCut,
        Msg::WcsUsed,
    ] {
        assert!(seen.contains(&m), "{m:?} is never raised");
    }
}

#[test]
fn the_other_new_messages_come_from_the_report_and_the_macro_checks() {
    let t = Tuning {
        tools: ToolTable::parse("tool = 1 | diameter=2 | life=100"),
        material: "steel".to_string(),
        ..Tuning::default()
    };
    let p: Vec<String> = [
        "G21 G90",
        "T1 M6",
        "M3 S1000",
        "G0 Z5",
        "G81 X1 Y1 Z-3 R1 F600",
        "G80",
        "G1 X0 Y0 F100",
        "G41",
        "G1 X600 F600",
        "G40",
        "G10 L2 P2 X0",
        "G55",
        "G0 X1",
        "M30",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let r = build(&p, &t, &LintCfg::default(), &cfg("en"));
    for part in [
        "Canned cycles: 1 holes",
        "Cutter compensation:",
        "Work coordinate systems in use",
        "use up",
    ] {
        assert!(r.text.contains(part), "{part}: {}", r.text);
    }
    let lib = "[empty]\n\n[leaky]\nG91\nG0 X1\nM30\n[moves]\nG0 X1\n";
    let (s, n) = crate::macrocheck::validate_text(lib, &Tuning::default(), "en");
    for part in [
        "no line of G-code",
        "is still in force",
        "ends the program",
        "without choosing G90 or G91",
    ] {
        assert!(s.contains(part), "{part}: {s}");
    }
    assert!(n >= 4);
}
