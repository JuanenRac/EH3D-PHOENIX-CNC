//! The ranges of the values of the program tools in the settings file. The settings window offers a number only inside its range, and
//! the settings file keeps a value only inside it too (a value outside is left out and the default stays), so the two must agree: a test
//! walks through this table and checks both ends of every range against the file reader.

/// A key of the settings file with the smallest and the biggest value it takes.
pub struct Range {
    pub key: &'static str,
    pub lo: f64,
    pub hi: f64,
}

pub const RANGES: [Range; 19] = [
    Range {
        key: "lint_rapid_floor",
        lo: -500.0,
        hi: 500.0,
    },
    Range {
        key: "lint_max_feed",
        lo: 0.0,
        hi: 1_000_000.0,
    },
    Range {
        key: "lint_max_rpm",
        lo: 0.0,
        hi: 1_000_000.0,
    },
    Range {
        key: "lint_arc_gap",
        lo: 0.0001,
        hi: 0.5,
    },
    Range {
        key: "keepout_margin",
        lo: 0.0,
        hi: 100.0,
    },
    Range {
        key: "probe_feed",
        lo: 1.0,
        hi: 1000.0,
    },
    Range {
        key: "probe_depth",
        lo: 0.1,
        hi: 200.0,
    },
    Range {
        key: "probe_retract",
        lo: 0.0,
        hi: 100.0,
    },
    Range {
        key: "probe_plate",
        lo: 0.0,
        hi: 100.0,
    },
    Range {
        key: "probe_clearance",
        lo: 0.5,
        hi: 200.0,
    },
    Range {
        key: "probe_grid",
        lo: 1.0,
        hi: 1000.0,
    },
    Range {
        key: "heightmap_segment",
        lo: 0.05,
        hi: 100.0,
    },
    Range {
        key: "queue_repeat_max",
        lo: 1.0,
        hi: 10_000.0,
    },
    Range {
        key: "queue_pause",
        lo: 0.0,
        hi: 86_400.0,
    },
    Range {
        key: "sim_speed",
        lo: 0.1,
        hi: 10_000.0,
    },
    Range {
        key: "log_max_entries",
        lo: 10.0,
        hi: 1_000_000.0,
    },
    Range {
        key: "macros_max_lines",
        lo: 1.0,
        hi: 10_000.0,
    },
    Range {
        key: "tool_change_time",
        lo: 0.0,
        hi: 3600.0,
    },
    Range {
        key: "accel",
        lo: 0.0,
        hi: 100_000.0,
    },
];

/// The smallest and the biggest value of a key (anything for a key that is not in the table).
pub fn range_of(key: &str) -> (f64, f64) {
    RANGES
        .iter()
        .find(|r| r.key == key)
        .map_or((f64::MIN, f64::MAX), |r| (r.lo, r.hi))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;

    fn line_of(text: &str, key: &str, value: f64) -> bool {
        let want = format!("{key} = {value}");
        text.lines().any(|l| l == want)
    }

    #[test]
    fn both_ends_of_every_range_are_taken_by_the_file_reader_and_what_is_outside_is_not() {
        for r in &RANGES {
            for v in [r.lo, r.hi] {
                let s = Settings::parse(&format!("{} = {v}\n", r.key));
                assert!(
                    line_of(&s.to_text(), r.key, v),
                    "{} = {v} was not kept",
                    r.key
                );
            }
            let below = if r.lo > 0.0 { r.lo / 2.0 } else { r.lo - 1.0 };
            let above = r.hi * 2.0 + 1.0;
            for v in [below, above] {
                let s = Settings::parse(&format!("{} = {v}\n", r.key));
                assert!(
                    !line_of(&s.to_text(), r.key, v),
                    "{} = {v} was kept though it is outside",
                    r.key
                );
            }
        }
    }

    #[test]
    fn the_defaults_are_inside_their_ranges() {
        let text = Settings::default().to_text();
        for r in &RANGES {
            let value: f64 = text
                .lines()
                .find_map(|l| l.strip_prefix(&format!("{} = ", r.key)))
                .unwrap_or_else(|| panic!("{} is not written", r.key))
                .parse()
                .unwrap();
            assert!((r.lo..=r.hi).contains(&value), "{} = {value}", r.key);
        }
    }

    #[test]
    fn a_key_is_listed_once_and_an_unknown_one_has_no_limit() {
        let mut seen = std::collections::HashSet::new();
        for r in &RANGES {
            assert!(seen.insert(r.key), "{} twice", r.key);
            assert!(r.lo < r.hi);
        }
        assert_eq!(range_of("probe_feed"), (1.0, 1000.0));
        assert_eq!(range_of("nothing"), (f64::MIN, f64::MAX));
    }

    #[test]
    fn every_range_has_its_row_in_the_settings_window() {
        // the window offers a number only for a key of this table, and every key of the table is offered
        let window = include_str!("gui/prefs.rs");
        for r in &RANGES {
            assert!(
                window.contains(&format!("\"{}\"", r.key)),
                "{} has no row in the settings window",
                r.key
            );
        }
    }

    #[test]
    fn the_new_checks_and_the_acceleration_have_their_ranges() {
        assert_eq!(range_of("lint_arc_gap"), (0.0001, 0.5));
        assert_eq!(range_of("accel"), (0.0, 100_000.0));
    }
}
