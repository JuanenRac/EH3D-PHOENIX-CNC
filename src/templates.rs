//! Templates of a machine: the travel, the speeds and the limits of a kind of machine (a small desktop mill, a wood router, a light engraver, a
//! circuit-board mill) as lines of the settings file, to start from instead of from the defaults. A template is printed (`--template NAME`) or applied to
//! settings; it never writes the settings file by itself, and the numbers are the usual ones of the kind of machine, not those of a machine in particular:
//! the operator checks them against the manual of theirs. The values are in this file's table because they are a starting point; the settings file that
//! the operator saves from them is where they live afterwards.

use crate::settings::Settings;
use crate::words::{say, Msg};

/// A kind of machine.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Template {
    pub name: &'static str,
    pub description: Msg,
    /// The travel of X, Y and Z in mm (low, high); Z is below the zero of the work.
    pub travel: [(f64, f64); 3],
    pub spindle_rpm: f64,
    pub step_mm: f64,
    pub feed_mm_min: f64,
    pub rapid_mm_min: f64,
    pub accel_mm_s2: f64,
    /// The feed and the speed the checker warns above.
    pub lint_max_feed: f64,
    pub lint_max_rpm: f64,
}

/// The templates, in the order they are listed.
pub const ALL: [Template; 4] = [
    Template {
        name: "small-mill",
        description: Msg::TplSmallMill,
        travel: [(0.0, 300.0), (0.0, 180.0), (-80.0, 0.0)],
        spindle_rpm: 10_000.0,
        step_mm: 1.0,
        feed_mm_min: 600.0,
        rapid_mm_min: 2500.0,
        accel_mm_s2: 250.0,
        lint_max_feed: 1500.0,
        lint_max_rpm: 12_000.0,
    },
    Template {
        name: "router",
        description: Msg::TplRouter,
        travel: [(0.0, 1200.0), (0.0, 800.0), (-120.0, 0.0)],
        spindle_rpm: 18_000.0,
        step_mm: 10.0,
        feed_mm_min: 3000.0,
        rapid_mm_min: 8000.0,
        accel_mm_s2: 600.0,
        lint_max_feed: 9000.0,
        lint_max_rpm: 24_000.0,
    },
    Template {
        name: "engraver",
        description: Msg::TplEngraver,
        travel: [(0.0, 200.0), (0.0, 200.0), (-40.0, 0.0)],
        spindle_rpm: 12_000.0,
        step_mm: 0.5,
        feed_mm_min: 800.0,
        rapid_mm_min: 3000.0,
        accel_mm_s2: 400.0,
        lint_max_feed: 2000.0,
        lint_max_rpm: 15_000.0,
    },
    Template {
        name: "pcb-mill",
        description: Msg::TplPcb,
        travel: [(0.0, 200.0), (0.0, 150.0), (-40.0, 0.0)],
        spindle_rpm: 30_000.0,
        step_mm: 0.1,
        feed_mm_min: 300.0,
        rapid_mm_min: 1500.0,
        accel_mm_s2: 200.0,
        lint_max_feed: 600.0,
        lint_max_rpm: 35_000.0,
    },
];

/// The names of the templates, in the order they are listed.
pub fn names() -> Vec<&'static str> {
    ALL.iter().map(|t| t.name).collect()
}

/// The template with this name (the case does not matter).
pub fn find(name: &str) -> Option<&'static Template> {
    ALL.iter()
        .find(|t| t.name.eq_ignore_ascii_case(name.trim()))
}

impl Template {
    /// The lines of the settings file for this machine (millimetres, the keys of `Settings`), with a first line that says what they are.
    pub fn settings_text(&self) -> String {
        let t = &self.travel;
        format!(
            "# phoenix-cnc template {}\nunits = mm\nstep = {}\nfeed = {}\nspindle = {}\nrapid = {}\naccel = {}\ntravel_x = {},{}\ntravel_y = {},{}\ntravel_z = {},{}\nlint_max_feed = {}\nlint_max_rpm = {}\n",
            self.name,
            self.step_mm,
            self.feed_mm_min,
            self.spindle_rpm,
            self.rapid_mm_min,
            self.accel_mm_s2,
            t[0].0,
            t[0].1,
            t[1].0,
            t[1].1,
            t[2].0,
            t[2].1,
            self.lint_max_feed,
            self.lint_max_rpm
        )
    }

    /// Sets these values in the settings; the rest of the settings is left as it is.
    pub fn apply(&self, s: &mut Settings) {
        let t = Settings::parse(&self.settings_text());
        s.inches = t.inches;
        s.step_mm = t.step_mm;
        s.feed_mm_min = t.feed_mm_min;
        s.spindle_rpm = t.spindle_rpm;
        s.rapid_mm_min = t.rapid_mm_min;
        s.travel = t.travel;
        s.lint.max_feed_mm_min = t.lint.max_feed_mm_min;
        s.lint.max_rpm = t.lint.max_rpm;
        s.tuning.accel_mm_s2 = t.tuning.accel_mm_s2;
    }
}

/// The list of the templates in a language: the title and a line for each one.
pub fn list(lang: &str) -> Vec<String> {
    let mut out = vec![say::<&str>(lang, Msg::TplList, &[])];
    out.extend(
        ALL.iter()
            .map(|t| format!("  {}: {}", t.name, say::<&str>(lang, t.description, &[]))),
    );
    out
}

/// What `--template` says when the name is not one of them.
pub fn unknown(name: &str, lang: &str) -> String {
    say(lang, Msg::TplUnknown, &[name.trim().to_string()])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_names_are_found_whatever_the_case_and_the_others_are_not() {
        for t in ALL {
            assert_eq!(find(t.name).map(|x| x.name), Some(t.name));
            assert!(find(&t.name.to_uppercase()).is_some());
            assert!(find(&format!("  {} ", t.name)).is_some());
        }
        for no in ["", "mill", "small_mill", "lathe", "pcb"] {
            assert!(find(no).is_none(), "{no}");
        }
        let mut seen = names();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), 4);
    }

    #[test]
    fn what_a_template_prints_is_read_back_by_the_settings_without_changing_a_number() {
        for t in ALL {
            let s = Settings::parse(&t.settings_text());
            assert!(!s.inches, "{}", t.name);
            assert_eq!(s.travel, t.travel, "{}", t.name);
            assert_eq!(
                (s.step_mm, s.feed_mm_min, s.spindle_rpm, s.rapid_mm_min),
                (t.step_mm, t.feed_mm_min, t.spindle_rpm, t.rapid_mm_min),
                "{}",
                t.name
            );
            assert_eq!(s.tuning.accel_mm_s2, t.accel_mm_s2, "{}", t.name);
            assert_eq!(
                (s.lint.max_feed_mm_min, s.lint.max_rpm),
                (t.lint_max_feed, t.lint_max_rpm),
                "{}",
                t.name
            );
        }
    }

    #[test]
    fn the_numbers_are_sensible_and_the_description_says_the_same_as_the_table() {
        for t in ALL {
            // the work is below the zero of Z and the travel is a box
            assert!(t.travel[2].1 == 0.0 && t.travel[2].0 < 0.0, "{}", t.name);
            assert!(t.travel[0].1 > t.travel[0].0 && t.travel[1].1 > t.travel[1].0);
            // the feed of the template is under its own limit, the rapid over it, and the spindle under the limit of the speed
            assert!(t.feed_mm_min < t.lint_max_feed && t.feed_mm_min < t.rapid_mm_min);
            assert!(t.spindle_rpm < t.lint_max_rpm);
            assert!(t.step_mm > 0.0 && t.accel_mm_s2 > 0.0);
            let en = say::<&str>("en", t.description, &[]);
            let size = format!(
                "{} x {} x {} mm, {} rpm",
                t.travel[0].1 - t.travel[0].0,
                t.travel[1].1 - t.travel[1].0,
                t.travel[2].1 - t.travel[2].0,
                t.spindle_rpm
            );
            assert!(en.ends_with(&size), "{en} / {size}");
        }
    }

    #[test]
    fn applying_a_template_changes_its_values_and_leaves_the_rest_of_the_settings() {
        let mut s = Settings {
            inches: true,
            server_address: "192.168.0.180:18200".into(),
            last_dir: "C:/jobs".into(),
            ..Settings::default()
        };
        let before = s.clone();
        let router = find("router").unwrap();
        router.apply(&mut s);
        assert!(!s.inches);
        assert_eq!(s.travel[0], (0.0, 1200.0));
        assert_eq!(s.spindle_rpm, 18_000.0);
        assert_eq!(s.tuning.accel_mm_s2, 600.0);
        assert_eq!(s.server_address, before.server_address);
        assert_eq!(s.last_dir, before.last_dir);
        assert_eq!(s.theme, before.theme);
        // the same template twice is the same settings
        let once = s.clone();
        router.apply(&mut s);
        assert_eq!(s, once);
    }

    #[test]
    fn the_list_and_the_refusal_are_told_in_the_eight_languages() {
        for lang in crate::words::LANGS {
            let l = list(lang);
            assert_eq!(l.len(), 5);
            for line in &l {
                assert!(
                    !line.contains('{') && !line.trim().is_empty(),
                    "{lang}: {line}"
                );
            }
            assert!(unknown("lathe", lang).contains("lathe"), "{lang}");
            assert!(!unknown("lathe", lang).contains('{'));
        }
        assert_eq!(
            list("en")[1],
            "  small-mill: A small desktop mill: 300 x 180 x 80 mm, 10000 rpm"
        );
        assert!(unknown(" lathe ", "en").contains("'lathe'"));
    }
}
