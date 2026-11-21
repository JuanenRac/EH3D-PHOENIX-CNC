//! The materials of the shared database (EH3D-COMMON) as this program uses them: the starting point of a material for an operation and a cutter of a
//! diameter, the list of the materials, the chip load and the cutting speed, the material of the settings read from the database, and the formats of
//! Marlin and Klipper. Every draw comes from the seed written here, so a run is the same on every machine (the arithmetic is on 64 bits, nothing
//! depends on the size of the pointer), and nothing here touches a machine, a network or a file outside the test.

use eh3d_materials::machining::{
    CncOp, CncPoint, Family, MachiningDatabase, StartNote, StartWarning, Tool,
};
use eh3d_phoenix_cnc::cli::Options;
use eh3d_phoenix_cnc::cli_tools::{run_in, wanted};
use eh3d_phoenix_cnc::materials::Material;
use eh3d_phoenix_cnc::postcheck::{check, Profile, NAMES};
use eh3d_phoenix_cnc::progcheck::Level;
use eh3d_phoenix_cnc::settings::Settings;
use eh3d_phoenix_cnc::sharedmat::{
    chip_load, chip_verdict, database, feeds, find, materials, parse_request, shared_material,
    surface_speed, ChipVerdict, CHIP_HIGH_PCT, CHIP_LOW_PCT,
};
use eh3d_phoenix_cnc::words::{Msg, LANGS};

/// The seed of every draw of this file.
const SEED: u64 = 20_261_121;

/// A small generator of numbers from a seed (a linear congruential one on 64 bits): the same seed gives the same row on every system.
struct Draw(u64);

impl Draw {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 11
    }

    /// A number from 0 up to but not including `n`.
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    /// A number from 0 to 1.
    fn unit(&mut self) -> f64 {
        (self.next() % 1_000_001) as f64 / 1_000_000.0
    }
}

fn parse(args: &[&str]) -> Options {
    Options::parse(args.iter().map(|a| a.to_string())).unwrap()
}

fn db() -> &'static MachiningDatabase {
    database().unwrap()
}

/// The materials that have points for a router.
fn router_materials() -> Vec<&'static eh3d_materials::machining::Workpiece> {
    db().materials()
        .iter()
        .filter(|w| !w.cnc.is_empty())
        .collect()
}

fn group(w: &eh3d_materials::machining::Workpiece, op: CncOp, tool: Tool) -> Vec<&CncPoint> {
    w.cnc
        .iter()
        .filter(|p| p.op == op && p.tool == tool)
        .collect()
}

fn span(points: &[&CncPoint], f: impl Fn(&CncPoint) -> f64) -> (f64, f64) {
    let v: Vec<f64> = points.iter().map(|p| f(p)).collect();
    (
        v.iter().cloned().fold(f64::INFINITY, f64::min),
        v.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
    )
}

#[test]
fn a_starting_point_from_a_seed_stays_between_the_points_it_comes_from_and_never_goes_outside_them()
{
    let mut d = Draw(SEED);
    let all = router_materials();
    assert!(
        all.len() >= 30,
        "the database has the materials of a router"
    );
    let mut interpolated = 0;
    for _ in 0..400 {
        let w = all[d.below(all.len())];
        let p = &w.cnc[d.below(w.cnc.len())];
        let g = group(w, p.op, p.tool);
        let (lo, hi) = span(&g, |p| p.diameter_mm);
        let diameter = lo + (hi - lo) * d.unit();
        let s = w.cnc_start(p.op, p.tool, diameter).unwrap();
        let (rlo, rhi) = span(&g, |p| p.rpm);
        let (flo, fhi) = span(&g, |p| p.feed_mm_min);
        assert!(s.rpm >= rlo - 1e-6 && s.rpm <= rhi + 1e-6, "{} {s:?}", w.id);
        assert!(
            s.feed_mm_min >= flo - 1e-6 && s.feed_mm_min <= fhi + 1e-6,
            "{} {s:?}",
            w.id
        );
        interpolated += usize::from(s.interpolated);
        // nothing is extrapolated: a diameter outside what is known is a warning, never a number
        for outside in [lo - 0.5 - d.unit(), hi + 0.5 + d.unit() * 10.0] {
            if outside <= 0.0 {
                continue;
            }
            assert!(
                matches!(
                    w.cnc_start(p.op, p.tool, outside),
                    Err(StartWarning::OutOfRange { .. })
                ),
                "{} {outside}",
                w.id
            );
        }
    }
    assert!(
        interpolated > 50,
        "the draws reach the points between the known ones: {interpolated}"
    );
}

#[test]
fn the_same_seed_gives_the_same_answers_in_every_language() {
    for lang in LANGS {
        let run = |seed: u64| -> Vec<String> {
            let mut d = Draw(seed);
            let all = router_materials();
            (0..40)
                .map(|_| {
                    let w = all[d.below(all.len())];
                    let p = &w.cnc[d.below(w.cnc.len())];
                    let spec = format!(
                        "{},{},{:.2},{}",
                        w.id,
                        p.op.as_str(),
                        p.diameter_mm,
                        p.tool.as_str()
                    );
                    let r = feeds(&spec, lang);
                    assert_eq!(r.code, 0, "{spec} {lang}: {}", r.err);
                    assert!(
                        !r.out.contains('{') && !r.out.contains('}'),
                        "{spec} {lang}"
                    );
                    r.out
                })
                .collect()
        };
        assert_eq!(run(SEED), run(SEED), "{lang}");
        assert_ne!(run(SEED), run(SEED + 1), "{lang}");
    }
}

#[test]
fn a_material_is_found_by_its_identifier_its_short_name_or_its_name_in_any_language() {
    let mdf = find(db(), "wood/mdf").unwrap();
    assert_eq!(find(db(), " WOOD/MDF ").unwrap().id, mdf.id);
    assert_eq!(find(db(), "mdf").unwrap().id, mdf.id);
    for l in eh3d_materials::i18n::Lang::ALL {
        assert_eq!(find(db(), mdf.names.get(l)).unwrap().id, mdf.id, "{l:?}");
    }
    assert!(find(db(), "").is_none() && find(db(), "unobtainium").is_none());
    // every material of the database is found by its own identifier and by its own name in English
    for w in db().materials() {
        assert_eq!(find(db(), &w.id).unwrap().id, w.id);
        assert!(find(db(), &w.names.en).is_some(), "{}", w.id);
    }
}

#[test]
fn a_request_is_read_part_by_part_and_the_wrong_part_is_named() {
    let r = parse_request(" wood/mdf , Pocket , 3.175 , compression ").unwrap();
    assert_eq!(r.material, "wood/mdf");
    assert_eq!(r.op, Some(CncOp::Pocket));
    assert_eq!(r.diameter_mm, Some(3.175));
    assert_eq!(r.tool, Some(Tool::Compression));
    let only = parse_request("wood/mdf").unwrap();
    assert!(only.op.is_none() && only.diameter_mm.is_none() && only.tool.is_none());
    for (bad, part) in [
        ("", ""),
        ("wood/mdf,carve", "carve"),
        ("wood/mdf,contour,zero", "zero"),
        ("wood/mdf,contour,-3", "-3"),
        ("wood/mdf,contour,nan", "nan"),
        ("wood/mdf,contour,3,saw", "saw"),
        (
            "wood/mdf,contour,3,end_mill,x",
            "wood/mdf,contour,3,end_mill,x",
        ),
    ] {
        assert_eq!(parse_request(bad).unwrap_err(), part, "{bad}");
    }
    // the options say it too, before anything runs
    assert!(Options::parse(["--feeds".to_string(), "wood/mdf,carve".to_string()]).is_err());
    assert!(Options::parse(["--feeds".to_string()]).is_err());
    assert!(Options::parse(["--materials".to_string(), " ".to_string()]).is_err());
}

#[test]
fn feeds_tells_the_point_with_its_chip_load_and_what_a_person_should_know_in_eight_languages() {
    let s = Settings::default();
    let known = db().get("wood/mdf").unwrap();
    let three = known.cnc_start(CncOp::Contour, Tool::EndMill, 3.0).unwrap();
    assert!(
        !three.verified,
        "the seed of the database was never tried on a machine"
    );
    let mut texts = Vec::new();
    for lang in LANGS {
        let o = parse(&["--feeds", "wood/mdf,contour,3", "--lang", lang]);
        assert!(wanted(&o));
        let r = run_in(&o, &s);
        assert_eq!(r.code, 0, "{lang}: {}", r.err);
        assert!(r.err.is_empty() && !r.out.contains('{'), "{lang}");
        assert_eq!(r.out.lines().count(), 4, "{lang}: {}", r.out);
        assert!(r.out.contains("19000") && r.out.contains("1100"), "{lang}");
        // the note of the database about a point never tried is the one of its own language
        let l = eh3d_materials::i18n::Lang::from_code(lang).unwrap();
        assert!(r.out.contains(StartNote::NotVerified.text(l)), "{lang}");
        texts.push(r.out);
    }
    texts.sort();
    texts.dedup();
    assert_eq!(texts.len(), 8, "each language says it in its own words");
    // between two known diameters the line says so, and the chip load follows the numbers of the point it gives
    let r = feeds("wood/mdf,contour,4.5", "en");
    assert_eq!(r.code, 0, "{}", r.err);
    assert!(r
        .out
        .contains(StartNote::Interpolated.text(eh3d_materials::i18n::Lang::En)));
    let between = known.cnc_start(CncOp::Contour, Tool::EndMill, 4.5).unwrap();
    let chip = chip_load(between.feed_mm_min, between.rpm, between.flutes.unwrap()).unwrap();
    assert!(r.out.contains(&format!("{chip:.4}")), "{}", r.out);
}

#[test]
fn feeds_refuses_with_the_exit_code_2_and_says_why() {
    for lang in LANGS {
        for spec in ["unobtainium", "wood/mdf,contour,300"] {
            let r = feeds(spec, lang);
            assert_eq!(r.code, 2, "{spec} {lang}");
            assert!(
                r.out.is_empty() && r.err.starts_with("phoenix-cnc: "),
                "{spec} {lang}"
            );
            assert!(!r.err.contains('{'), "{spec} {lang}: {}", r.err);
        }
        assert!(
            feeds("unobtainium", lang).err.contains("unobtainium"),
            "{lang}"
        );
        let bad = feeds("wood/mdf,carve", lang);
        assert_eq!(bad.code, 2);
        assert!(bad.err.contains("carve"), "{lang}: {}", bad.err);
        let none = feeds("wood/mdf,drill", lang);
        assert_eq!(
            none.code, 2,
            "{lang}: a material without that process is refused"
        );
    }
    // the words of the database for a diameter outside what it knows
    let l = eh3d_materials::i18n::Lang::Es;
    let w = StartWarning::OutOfRange {
        quantity: eh3d_materials::machining::Quantity::Diameter,
        requested: 300.0,
        lowest: 3.0,
        highest: 6.0,
    };
    assert!(feeds("wood/mdf,contour,300", "es").err.contains(&w.text(l)));
}

#[test]
fn feeds_with_no_diameter_lists_the_points_of_the_material_and_of_the_operation() {
    for lang in LANGS {
        let all = feeds("wood/mdf", lang);
        assert_eq!(all.code, 0, "{lang}");
        let rows = all.out.lines().count() - 1;
        assert_eq!(rows, db().get("wood/mdf").unwrap().cnc.len(), "{lang}");
        assert!(!all.out.contains('{'), "{lang}");
        let pocket = feeds("wood/mdf,pocket", lang);
        assert!(
            pocket.out.lines().skip(1).all(|l| l.starts_with("pocket ")),
            "{lang}"
        );
        assert_eq!(pocket.out.lines().count() - 1, 2, "{lang}");
    }
}

#[test]
fn materials_lists_what_the_database_has_for_a_router_and_each_family_on_its_own() {
    let all = router_materials();
    for lang in LANGS {
        let r = materials("all", lang);
        assert_eq!(r.code, 0, "{lang}");
        assert_eq!(r.out.lines().count(), all.len() + 1, "{lang}");
        assert!(!r.out.contains('{'), "{lang}");
        let ids: Vec<&str> = r
            .out
            .lines()
            .skip(1)
            .map(|l| l.split_whitespace().next().unwrap())
            .collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(
            ids, sorted,
            "{lang}: the rows are in the order of the identifiers"
        );
    }
    let mut total = 0;
    for f in Family::ALL {
        let r = materials(f.as_str(), "en");
        let want = all.iter().filter(|w| w.family == f).count();
        if want == 0 {
            assert_eq!(r.code, 2, "{f:?}");
        } else {
            assert_eq!(r.out.lines().count(), want + 1, "{f:?}");
            total += want;
        }
    }
    assert_eq!(total, all.len(), "every material is in one family");
    let wrong = materials("metals", "de");
    assert_eq!(wrong.code, 2);
    assert!(
        wrong.err.contains("metals") && wrong.err.contains("wood"),
        "{}",
        wrong.err
    );
    // the options carry it through the tool
    let o = parse(&["--materials", "Wood", "--lang", "fr"]);
    assert!(wanted(&o));
    let r = run_in(&o, &Settings::default());
    assert_eq!(r.code, 0);
    assert!(r.out.contains("wood/mdf"));
}

#[test]
fn the_chip_load_follows_the_feed_the_speed_and_the_teeth_and_the_verdict_only_goes_up() {
    let mut d = Draw(SEED ^ 0x5eed);
    for _ in 0..500 {
        let feed = 50.0 + d.unit() * 3000.0;
        let rpm = 3000.0 + d.unit() * 30000.0;
        let flutes = 1 + d.below(4) as u8;
        let chip = chip_load(feed, rpm, flutes).unwrap();
        assert!((chip_load(feed * 2.0, rpm, flutes).unwrap() - 2.0 * chip).abs() < 1e-12);
        assert!((chip_load(feed, rpm * 2.0, flutes).unwrap() - chip / 2.0).abs() < 1e-12);
        // a verdict never goes back as the chip gets thicker: low, then fine, then high
        let diameter = 0.5 + d.unit() * 10.0;
        let rank = |c: f64| match chip_verdict(c, diameter) {
            ChipVerdict::Low => 0,
            ChipVerdict::Fine => 1,
            ChipVerdict::High => 2,
        };
        let mut last = 0;
        for k in 0..60 {
            let r = rank(chip * f64::from(k) / 6.0 + 1e-9);
            assert!(r >= last, "{chip} {diameter} {k}");
            last = r;
        }
    }
    // the edges: a bit below, at and above each limit, for a cutter of 4 mm
    assert_eq!(
        chip_verdict(4.0 * CHIP_LOW_PCT / 100.0 * 0.99, 4.0),
        ChipVerdict::Low
    );
    assert_eq!(
        chip_verdict(4.0 * CHIP_LOW_PCT / 100.0 * 1.01, 4.0),
        ChipVerdict::Fine
    );
    assert_eq!(
        chip_verdict(4.0 * CHIP_HIGH_PCT / 100.0 * 0.99, 4.0),
        ChipVerdict::Fine
    );
    assert_eq!(
        chip_verdict(4.0 * CHIP_HIGH_PCT / 100.0 * 1.01, 4.0),
        ChipVerdict::High
    );
    // what is not a number gives nothing
    assert!(chip_load(0.0, 10000.0, 2).is_none());
    assert!(chip_load(600.0, 0.0, 2).is_none());
    assert!(chip_load(600.0, 10000.0, 0).is_none());
    assert!(chip_load(f64::NAN, 10000.0, 2).is_none());
    assert!(chip_load(f64::INFINITY, 10000.0, 2).is_none());
    // 6 mm at 10000 rpm is 188.5 m/min
    assert!((surface_speed(6.0, 10_000.0) - 188.495_559).abs() < 1e-3);
}

#[test]
fn a_chip_that_is_too_thin_or_too_thick_is_said_in_the_line_of_the_point() {
    // 1000 mm/min at 20000 rpm with two teeth is 0.025 mm of chip: 0.8 % of 3 mm
    assert_eq!(
        chip_verdict(1000.0 / (20000.0 * 2.0), 3.0),
        ChipVerdict::Fine
    );
    // every point of the database is judged, and the verdicts agree with the numbers in the line of the point
    let (mut low, mut high, mut fine) = (0, 0, 0);
    for w in router_materials() {
        for p in &w.cnc {
            let Some(f) = p.flutes else { continue };
            let spec = format!(
                "{},{},{},{}",
                w.id,
                p.op.as_str(),
                p.diameter_mm,
                p.tool.as_str()
            );
            let r = feeds(&spec, "en");
            assert_eq!(r.code, 0, "{spec}: {}", r.err);
            let chip = chip_load(p.feed_mm_min, p.rpm, f).unwrap();
            let says_low = r.out.contains("The chip load is low");
            let says_high = r.out.contains("The chip load is high");
            match chip_verdict(chip, p.diameter_mm) {
                ChipVerdict::Low => assert!(says_low && !says_high, "{spec}"),
                ChipVerdict::High => assert!(says_high && !says_low, "{spec}"),
                ChipVerdict::Fine => assert!(!says_low && !says_high, "{spec}"),
            }
            match chip_verdict(chip, p.diameter_mm) {
                ChipVerdict::Low => low += 1,
                ChipVerdict::High => high += 1,
                ChipVerdict::Fine => fine += 1,
            }
        }
    }
    assert!(
        fine > 100,
        "most of the points of the database are in the middle: {fine} {low} {high}"
    );
}

#[test]
fn the_material_of_the_settings_can_be_one_of_the_shared_database() {
    let mut s = Settings {
        material: "wood/mdf".to_string(),
        ..Settings::default()
    };
    s.resolve_material(&[]);
    let (flo, fhi) = s.lint.material_feed.expect("the feed range of MDF");
    let (rlo, rhi) = s.lint.material_rpm.expect("the speed range of MDF");
    assert!(flo < fhi && rlo < rhi);
    // the range holds every point of the material
    for p in &db().get("wood/mdf").unwrap().cnc {
        assert!(p.feed_mm_min >= flo && p.feed_mm_min <= fhi);
        assert!(p.rpm >= rlo && p.rpm <= rhi);
    }
    // the name in another language and the short name give the same
    for name in ["mdf", "MDF", "Wood/MDF", "MDF "] {
        let mut t = Settings {
            material: name.to_string(),
            ..Settings::default()
        };
        t.resolve_material(&[]);
        assert_eq!(t.lint.material_feed, s.lint.material_feed, "{name}");
    }
    // the library of the operator comes first when it has the name
    let own = Material {
        name: "wood/mdf".to_string(),
        feed: Some((10.0, 20.0)),
        rpm: None,
    };
    s.resolve_material(&[own]);
    assert_eq!(s.lint.material_feed, Some((10.0, 20.0)));
    assert_eq!(s.lint.material_rpm, None);
    // no material of that name anywhere: no range, as before
    s.material = "unobtainium".to_string();
    s.resolve_material(&[]);
    assert!(s.lint.material_feed.is_none() && s.lint.material_rpm.is_none());
    s.material.clear();
    s.resolve_material(&[]);
    assert!(s.lint.material_feed.is_none());
}

#[test]
fn every_material_of_the_database_becomes_a_material_of_the_library_that_holds_its_points() {
    let mut d = Draw(SEED);
    let all = router_materials();
    let mut found = 0;
    for _ in 0..200 {
        let w = all[d.below(all.len())];
        let Some(m) = shared_material(&w.id) else {
            continue;
        };
        found += 1;
        assert!(m.check().is_ok(), "{}: {m:?}", w.id);
        for p in &w.cnc {
            if let Some((lo, hi)) = m.feed {
                assert!(
                    p.feed_mm_min >= lo - 1e-9 && p.feed_mm_min <= hi + 1e-9,
                    "{}",
                    w.id
                );
            }
            if let Some((lo, hi)) = m.rpm {
                assert!(p.rpm >= lo - 1e-9 && p.rpm <= hi + 1e-9, "{}", w.id);
            }
        }
        // the line of the file of the library reads back as the same material
        assert_eq!(
            Material::parse(&m.to_line().replacen("material = ", "", 1))
                .as_ref()
                .map(|r| r.feed),
            Some(m.feed),
            "{}",
            w.id
        );
    }
    assert!(found > 150, "nearly every draw has ranges: {found}");
}

#[test]
fn the_formats_of_marlin_and_klipper_say_what_a_milling_program_takes_from_them() {
    assert_eq!(NAMES, ["grbl", "full", "generic", "marlin", "klipper"]);
    let lines = |t: &str| -> Vec<String> { t.lines().map(str::to_string).collect() };
    let marlin = Profile::named("Marlin").unwrap();
    let klipper = Profile::named(" klipper ").unwrap();
    // what a printer controller takes: moves, the home, the frame, the temperatures
    let printer = lines("G21\nG90\nG28\nG1 X10 Y10 F3000\nG92 E0\nM104 S200\nM84");
    assert!(check(&printer, &marlin).is_empty());
    assert!(check(&printer, &klipper).is_empty());
    // a milling program: M30 is the deletion of a file in Marlin, a spindle is nothing in Klipper
    let mill = lines("G21 G90\nM3 S12000\nG1 X5 F600\nM5\nM30");
    let on_marlin = check(&mill, &marlin);
    assert!(on_marlin
        .iter()
        .all(|f| f.msg == Msg::FmtCode && f.level == Level::Warning));
    assert_eq!(
        on_marlin
            .iter()
            .map(|f| f.args[1].as_str())
            .collect::<Vec<_>>(),
        ["M30"]
    );
    assert!(on_marlin.iter().all(|f| f.args[0] == "marlin"));
    let on_klipper: Vec<String> = check(&mill, &klipper)
        .iter()
        .map(|f| f.args[1].clone())
        .collect();
    assert_eq!(on_klipper, ["M3", "M5", "M30"]);
    // inches and the dwell: Klipper takes millimetres only
    let inches = lines("G20\nG4 P100");
    assert_eq!(check(&inches, &klipper).len(), 1);
    assert_eq!(check(&inches, &klipper)[0].args[1], "G20");
    assert!(check(&inches, &marlin).is_empty());
    // a line of Marlin is held to the buffer of 96 characters, Klipper to none
    let long = vec![
        format!("G1 X1 Y2 Z3 ; {}", "a".repeat(10)),
        format!("G1 {}", "X1 ".repeat(60)),
    ];
    let found = check(&long, &marlin);
    assert!(found
        .iter()
        .any(|f| f.msg == Msg::FmtLong && f.line == Some(1)));
    assert!(!found.iter().any(|f| f.line == Some(0)));
    assert!(check(&long, &klipper).iter().all(|f| f.msg != Msg::FmtLong));
    // neither wants the % line or an end by M2/M30
    assert!(check(&lines("G90\nG1 X1 F100"), &marlin).is_empty());
    assert!(check(&lines("G90\nG1 X1 F100"), &klipper).is_empty());
    assert!(!check(&lines("%\nG90"), &marlin).is_empty());
    // the profiles of the user can start from them
    let mine = Profile::from_text("base = marlin\nallow_m = 30\n", "mine").unwrap();
    assert!(check(&mill, &mine).is_empty());
    assert!(Profile::from_text("base = marlin\ndeny_g = 28\n", "mine").is_ok());
}

#[test]
fn the_format_of_marlin_and_klipper_runs_through_the_tool_in_every_language_with_the_exit_codes() {
    let s = Settings::default();
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples");
    let file = dir.join("01_square.nc").to_string_lossy().to_string();
    for name in ["marlin", "klipper"] {
        for lang in LANGS {
            let o = parse(&["--format", name, "--lang", lang, &file]);
            assert!(wanted(&o));
            let r = run_in(&o, &s);
            // the square ends with M30, which neither lists: something to read, never an error
            assert_eq!(r.code, 1, "{name} {lang}: {}", r.err);
            assert!(r.out.contains("M30"), "{name} {lang}: {}", r.out);
            assert!(
                !r.out.contains('{') && !r.err.contains('{'),
                "{name} {lang}"
            );
        }
    }
    let unknown = run_in(&parse(&["--format", "mach3", &file]), &s);
    assert_eq!(unknown.code, 2);
    assert!(
        unknown.err.contains("marlin") && unknown.err.contains("klipper"),
        "{}",
        unknown.err
    );
}

#[test]
fn the_help_names_the_new_tools_and_out_writes_what_they_print_to_a_new_file_only() {
    let h = eh3d_phoenix_cnc::cli::help_text();
    for word in ["--feeds SPEC", "--materials FAMILY", "marlin", "klipper"] {
        assert!(h.contains(word), "{word}");
    }
    // the tools need no file of G-code, and nothing else goes with them but the language and --out
    let s = Settings::default();
    let dir = std::env::temp_dir().join(format!("phoenix-cnc-shared-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("feeds.txt");
    let out_arg = out.to_string_lossy().to_string();
    let o = parse(&[
        "--feeds",
        "wood/mdf,pocket,6",
        "--out",
        &out_arg,
        "--lang",
        "de",
    ]);
    let first = run_in(&o, &s);
    assert_eq!(first.code, 0, "{}", first.err);
    let written = std::fs::read_to_string(&out).unwrap();
    assert!(
        written.contains("19000") || written.contains("17000"),
        "{written}"
    );
    // never over a file that is there
    let second = run_in(&o, &s);
    assert_eq!(second.code, 2);
    assert_eq!(std::fs::read_to_string(&out).unwrap(), written);
    std::fs::remove_dir_all(&dir).unwrap();
}
