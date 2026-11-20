//! The check of a dialect (`--dialect-check NAME`): a controller of the suite, GRBL, grblHAL or Marlin, is put against its simulator and
//! made to do what the program asks of every controller, and the answer of each step is judged. The steps are the status, a move, a jog,
//! the homing, a refusal by number (told in the language), a silent controller under the guard (it must be stopped and the machine
//! refused afterwards), and the same steps on cables that cut the answers in pieces and add noise, from a row of seeds. A step that does not
//! pass makes the check not pass: the exit code is 2. The evidence is the simulator; nothing here has met a real controller.

use eh3d_controller::{Command, Error, State};

use crate::dialtext;
use crate::guard::Trip;
use crate::link::{self, Dialect};
use crate::motioncfg::{GuardCfg, WireCfg};
use crate::notes::Label;
use crate::session::BoxedController;
use crate::wire::Impair;

/// The seeds of the noisy cables the steps are repeated on.
pub const NOISY_SEEDS: std::ops::RangeInclusive<u64> = 1..=8;

/// One step and how it ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub name: &'static str,
    pub passed: bool,
    /// What went wrong (empty when it passed), in English: it is the detail for the person who reads the check.
    pub detail: String,
}

/// The result of the check of one dialect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub dialect: Dialect,
    pub steps: Vec<Step>,
}

impl Report {
    pub fn passed(&self) -> bool {
        self.steps.iter().all(|s| s.passed)
    }

    /// The text of the check in the language: a line a step and the verdict.
    pub fn text(&self, lang: &str) -> String {
        let mut t = String::new();
        for s in &self.steps {
            let mark = if s.passed { "ok  " } else { "FAIL" };
            t.push_str(&format!("{mark} {}", s.name));
            if !s.detail.is_empty() {
                t.push_str(&format!(": {}", s.detail));
            }
            t.push('\n');
        }
        let verdict = if self.passed() {
            Label::CheckPassed
        } else {
            Label::CheckNotPassed
        };
        t.push_str(&format!(
            "{} ({}): {}\n",
            verdict.text(lang),
            self.dialect.family().name(),
            self.steps.len()
        ));
        t
    }
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

fn fail<T>(why: impl Into<String>) -> Result<T, String> {
    Err(why.into())
}

/// The status, a move, a jog and the homing on a controller; the position is checked at each step.
fn basic(c: &mut BoxedController) -> Result<(), String> {
    let st = c.status().map_err(|e| format!("status: {e}"))?;
    if !matches!(st.state, State::Idle | State::Run) {
        return fail(format!("the state at the start is {:?}", st.state));
    }
    c.send_line("G90").map_err(|e| format!("G90: {e}"))?;
    c.send_line("G1 X10 Y5 F600")
        .map_err(|e| format!("a move: {e}"))?;
    let st = c
        .status()
        .map_err(|e| format!("status after the move: {e}"))?;
    if !(near(st.machine_mm[0], 10.0) && near(st.machine_mm[1], 5.0)) {
        return fail(format!(
            "after G1 X10 Y5 the position is ({}, {})",
            st.machine_mm[0], st.machine_mm[1]
        ));
    }
    c.command(Command::Jog {
        axis: 0,
        distance_mm: 2.0,
        feed_mm_min: 300.0,
    })
    .map_err(|e| format!("a jog: {e}"))?;
    let st = c
        .status()
        .map_err(|e| format!("status after the jog: {e}"))?;
    if !near(st.machine_mm[0], 12.0) {
        return fail(format!("after the jog X is {}", st.machine_mm[0]));
    }
    c.command(Command::Home(vec![]))
        .map_err(|e| format!("homing: {e}"))?;
    let st = c
        .status()
        .map_err(|e| format!("status after homing: {e}"))?;
    if !(near(st.machine_mm[0], 0.0) && near(st.machine_mm[1], 0.0)) {
        return fail("the machine is not at the origin after homing");
    }
    Ok(())
}

/// A line that the controller refuses, with the number of its dialect, and the words for it in all eight languages.
fn refusal(d: Dialect) -> Result<(), String> {
    let mut c = link::simulated_with(d, Impair::clean(), false, Some(1));
    let e = c
        .send_line("G1 X1 F100")
        .err()
        .ok_or("the line that must fail was taken")?;
    if !matches!(e, Error::Refused(_)) {
        return fail(format!("the failure is {e:?}, not a refusal"));
    }
    if e.code() != Some(d.failing_code()) {
        return fail(format!("the number of the refusal is {:?}", e.code()));
    }
    let given = e.to_string();
    let mut seen = Vec::new();
    for lang in crate::ctltext::LANGS {
        let said = dialtext::explain(d.family(), "error", e.code(), &given, lang);
        if said == given {
            return fail(format!("no words for the refusal in {lang}"));
        }
        seen.push(said);
    }
    seen.sort_unstable();
    seen.dedup();
    if seen.len() != crate::ctltext::LANGS.len() {
        return fail("two languages say the refusal the same");
    }
    Ok(())
}

/// A controller that falls silent while a move is on its way: the guard stops the machine and refuses what moves after it.
fn silence(d: Dialect) -> Result<(), String> {
    let impair = Impair {
        mute_from: Some(1),
        mute_lines: 10_000,
        ..Impair::clean()
    };
    let c = link::simulated_with(d, impair, false, None);
    let mut l = link::wrap(c, false, &GuardCfg::default());
    let e = l
        .controller
        .send_line("G1 X5 F100")
        .err()
        .ok_or("a line that got no answer was taken for done")?;
    if e != Error::Timeout {
        return fail(format!("the silence is {e:?}, not a timeout"));
    }
    let trip = l.guard.as_ref().and_then(|g| g.tripped());
    if trip != Some(Trip::Silent) {
        return fail(format!("the guard did not stop the machine: {trip:?}"));
    }
    match l.controller.send_line("G1 X6 F100") {
        Err(Error::Alarm(_)) => Ok(()),
        other => fail(format!("a move after the stop was not refused: {other:?}")),
    }
}

/// The basic steps on cables that cut the answers and add noise, from each seed.
fn noisy(d: Dialect, wire: &WireCfg) -> Result<(), String> {
    for seed in NOISY_SEEDS {
        let impair = Impair {
            seed,
            chunk_max: 3,
            noise_per_mille: 300,
            ..link::impair_of(wire)
        };
        let mut c = link::simulated_with(d, impair, wire.marlin_numbering, None);
        basic(&mut c).map_err(|e| format!("seed {seed}: {e}"))?;
    }
    Ok(())
}

fn step(name: &'static str, r: Result<(), String>) -> Step {
    Step {
        name,
        passed: r.is_ok(),
        detail: r.err().unwrap_or_default(),
    }
}

/// Puts the dialect through the steps. `wire` is the cable of the motion file (and says whether Marlin is sent numbered lines).
pub fn run(d: Dialect, wire: &WireCfg) -> Report {
    let mut plain = link::simulated(d, wire);
    let steps = vec![
        step("status-move-jog-home", basic(&mut plain)),
        step("refusal-by-number", refusal(d)),
        step("silence-under-the-guard", silence(d)),
        step("noisy-cables", noisy(d, wire)),
    ];
    Report { dialect: d, steps }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_dialect_passes_its_check_with_the_cable_of_the_defaults() {
        for d in Dialect::ALL {
            let r = run(d, &WireCfg::default());
            assert!(r.passed(), "{d:?}: {}", r.text("en"));
            assert_eq!(r.steps.len(), 4);
        }
    }

    #[test]
    fn the_check_passes_on_a_cable_that_cuts_and_adds_noise_and_marlin_numbered() {
        for d in Dialect::ALL {
            let wire = WireCfg {
                seed: 42,
                chunk_max: 2,
                noise_per_mille: 500,
                flip_per_mille: 0,
                marlin_numbering: d == Dialect::Marlin,
            };
            let r = run(d, &wire);
            assert!(r.passed(), "{d:?}: {}", r.text("en"));
        }
    }

    #[test]
    fn a_cable_that_spoils_the_lines_fails_the_check_of_marlin_without_numbers_and_not_with_them() {
        let spoiled = |numbering: bool| WireCfg {
            seed: 3,
            chunk_max: 0,
            noise_per_mille: 0,
            flip_per_mille: 700,
            marlin_numbering: numbering,
        };
        let numbered = run(Dialect::Marlin, &spoiled(true));
        // with the numbers a line that arrives spoiled is asked for again, so the check can still pass or end in the error of the
        // checksum, but never in a position that was not asked for
        for s in &numbered.steps {
            assert!(
                s.passed
                    || s.detail.contains("103")
                    || s.detail.contains("checksum")
                    || s.detail.contains("refused"),
                "{s:?}"
            );
        }
        let plain = run(Dialect::Marlin, &spoiled(false));
        assert!(
            !plain.passed(),
            "a cable that spoils 70 lines in 100 cannot pass without the numbers: {}",
            plain.text("en")
        );
    }

    #[test]
    fn the_report_is_a_line_a_step_and_a_verdict_in_the_language() {
        let r = run(Dialect::Grbl, &WireCfg::default());
        let en = r.text("en");
        assert!(en.starts_with("ok   status-move-jog-home\n"), "{en}");
        assert!(en.ends_with("Dialect check passed (GRBL): 4\n"), "{en}");
        assert!(r.text("de").contains("Dialektprüfung bestanden"));
        let bad = Report {
            dialect: Dialect::Marlin,
            steps: vec![Step {
                name: "x",
                passed: false,
                detail: "why".into(),
            }],
        };
        assert!(!bad.passed());
        assert!(bad.text("en").starts_with("FAIL x: why\n"));
        assert!(bad.text("es").contains("no superada"));
    }

    #[test]
    fn the_refusal_step_names_the_number_of_each_dialect() {
        for d in Dialect::ALL {
            assert_eq!(refusal(d), Ok(()), "{d:?}");
        }
        assert_eq!(Dialect::Grbl.failing_code(), 22);
        assert_eq!(Dialect::GrblHal.failing_code(), 46);
    }

    #[test]
    fn the_silent_controller_is_stopped_for_every_dialect() {
        for d in Dialect::ALL {
            assert_eq!(silence(d), Ok(()), "{d:?}");
        }
    }
}
