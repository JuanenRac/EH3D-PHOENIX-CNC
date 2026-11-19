//! The soft limits of the jog: a step by hand that would take an axis past the travel of the machine is shortened to the limit, and one that
//! starts at the limit is refused. A machine already outside its travel (the zero was lost, the machine was not homed) may still be moved
//! towards the inside. It knows nothing of the controller, so it is tested on its own.

/// When the soft limits of the jog hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// From the start, whatever the machine knows of its position.
    Always,
    /// Only after the machine was homed: before that its coordinates mean nothing.
    AfterHoming,
    /// Never.
    Off,
}

impl Mode {
    /// The word of the settings file.
    pub fn key(self) -> &'static str {
        match self {
            Mode::Always => "always",
            Mode::AfterHoming => "homing",
            Mode::Off => "no",
        }
    }

    /// Reads the word of the settings file (`yes` is `always`, what it meant before there were modes); `None` for anything else.
    pub fn from_key(v: &str) -> Option<Mode> {
        match v.trim().to_ascii_lowercase().as_str() {
            "always" | "yes" | "on" | "true" | "1" => Some(Mode::Always),
            "homing" | "after_homing" | "after-homing" => Some(Mode::AfterHoming),
            "no" | "off" | "false" | "0" | "never" => Some(Mode::Off),
            _ => None,
        }
    }
}

/// What to do with a step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Step {
    /// The whole step is inside the travel.
    Go(f64),
    /// The step is shortened to this distance (mm, with its sign) so that the axis stops at the limit.
    Shorten(f64),
    /// The axis is at the limit already (or the numbers are not usable): nothing moves.
    Refuse,
}

/// The distance an axis may move: `pos` is where it is, `dist` the step asked for (both mm, in the machine coordinates) and `range` the
/// travel of the axis (minimum, maximum).
pub fn limit_step(pos: f64, dist: f64, range: (f64, f64)) -> Step {
    const EPS: f64 = 1e-6;
    let (lo, hi) = range;
    if !(pos.is_finite() && dist.is_finite() && lo.is_finite() && hi.is_finite()) || hi < lo {
        return Step::Refuse;
    }
    if dist == 0.0 {
        return Step::Refuse;
    }
    let target = pos + dist;
    if (lo - EPS..=hi + EPS).contains(&target) {
        return Step::Go(dist);
    }
    if pos > hi + EPS || pos < lo - EPS {
        // outside the travel: a step towards it is allowed as it is, one away from it is not
        let bound = if pos > hi { hi } else { lo };
        return if (target - bound).abs() < (pos - bound).abs() {
            Step::Go(dist)
        } else {
            Step::Refuse
        };
    }
    let room = if dist > 0.0 { hi - pos } else { lo - pos };
    if room.abs() > EPS {
        Step::Shorten(room)
    } else {
        Step::Refuse
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: (f64, f64) = (0.0, 100.0);

    #[test]
    fn the_mode_is_read_from_the_settings_word_and_written_back() {
        for m in [Mode::Always, Mode::AfterHoming, Mode::Off] {
            assert_eq!(Mode::from_key(m.key()), Some(m));
        }
        assert_eq!(Mode::from_key(" YES "), Some(Mode::Always));
        assert_eq!(Mode::from_key("After_Homing"), Some(Mode::AfterHoming));
        assert_eq!(Mode::from_key("off"), Some(Mode::Off));
        assert_eq!(Mode::from_key("sometimes"), None);
    }

    #[test]
    fn a_step_inside_the_travel_goes_whole() {
        assert_eq!(limit_step(50.0, 10.0, R), Step::Go(10.0));
        assert_eq!(limit_step(50.0, -50.0, R), Step::Go(-50.0));
        assert_eq!(limit_step(90.0, 10.0, R), Step::Go(10.0));
    }

    #[test]
    fn a_step_past_the_limit_is_shortened() {
        assert_eq!(limit_step(95.0, 10.0, R), Step::Shorten(5.0));
        assert_eq!(limit_step(3.0, -10.0, R), Step::Shorten(-3.0));
    }

    #[test]
    fn a_step_from_the_limit_outwards_is_refused() {
        assert_eq!(limit_step(100.0, 1.0, R), Step::Refuse);
        assert_eq!(limit_step(0.0, -0.01, R), Step::Refuse);
        // and the other way it goes
        assert_eq!(limit_step(100.0, -1.0, R), Step::Go(-1.0));
    }

    #[test]
    fn a_machine_outside_its_travel_can_only_come_back() {
        assert_eq!(limit_step(120.0, -10.0, R), Step::Go(-10.0));
        assert_eq!(limit_step(120.0, 10.0, R), Step::Refuse);
        assert_eq!(limit_step(-20.0, 10.0, R), Step::Go(10.0));
        assert_eq!(limit_step(-20.0, -10.0, R), Step::Refuse);
    }

    #[test]
    fn numbers_that_cannot_be_used_move_nothing() {
        assert_eq!(limit_step(f64::NAN, 1.0, R), Step::Refuse);
        assert_eq!(limit_step(5.0, f64::INFINITY, R), Step::Refuse);
        assert_eq!(limit_step(5.0, 0.0, R), Step::Refuse);
        assert_eq!(limit_step(5.0, 1.0, (10.0, 0.0)), Step::Refuse);
    }

    #[test]
    fn a_step_that_ends_on_a_limit_or_a_hair_past_it_goes_whole_and_one_further_is_shortened() {
        assert_eq!(limit_step(95.0, 5.0, R), Step::Go(5.0));
        assert_eq!(limit_step(5.0, -5.0, R), Step::Go(-5.0));
        // the rounding of the controller (a millionth of a mm) is not a step past the limit
        assert_eq!(limit_step(95.0, 5.000_000_5, R), Step::Go(5.000_000_5));
        assert_eq!(limit_step(95.0, 5.01, R), Step::Shorten(5.0));
        // the room keeps the sign of the step
        assert_eq!(limit_step(40.0, -100.0, R), Step::Shorten(-40.0));
        assert_eq!(limit_step(40.0, 100.0, R), Step::Shorten(60.0));
    }

    #[test]
    fn an_axis_with_no_room_moves_nowhere() {
        // a travel of a single point: every step leaves it
        assert_eq!(limit_step(5.0, 1.0, (5.0, 5.0)), Step::Refuse);
        assert_eq!(limit_step(5.0, -1.0, (5.0, 5.0)), Step::Refuse);
        // a limit that is not a number refuses, and so does a step of zero anywhere
        assert_eq!(limit_step(5.0, 1.0, (0.0, f64::NAN)), Step::Refuse);
        assert_eq!(
            limit_step(5.0, 1.0, (f64::NEG_INFINITY, 10.0)),
            Step::Refuse
        );
        assert_eq!(limit_step(50.0, 0.0, R), Step::Refuse);
    }

    #[test]
    fn a_machine_outside_its_travel_is_only_allowed_a_step_that_gets_it_nearer() {
        // 20 mm past the top: a step down that leaves it 10 mm past, or just inside, is allowed whole
        assert_eq!(limit_step(120.0, -10.0, R), Step::Go(-10.0));
        assert_eq!(limit_step(120.0, -20.0, R), Step::Go(-20.0));
        assert_eq!(limit_step(120.0, -30.0, R), Step::Go(-30.0));
        // a step that crosses the whole travel and ends farther from it than it started is refused
        assert_eq!(limit_step(120.0, -150.0, R), Step::Refuse);
        // the bottom side is the same
        assert_eq!(limit_step(-20.0, 30.0, R), Step::Go(30.0));
        assert_eq!(limit_step(-20.0, 150.0, R), Step::Refuse);
        assert_eq!(limit_step(-20.0, -1.0, R), Step::Refuse);
    }

    #[test]
    fn the_words_of_the_mode_are_read_in_every_spelling_and_the_keys_are_different() {
        for (word, mode) in [
            ("always", Mode::Always),
            ("TRUE", Mode::Always),
            ("1", Mode::Always),
            ("homing", Mode::AfterHoming),
            ("after-homing", Mode::AfterHoming),
            (" After_Homing ", Mode::AfterHoming),
            ("never", Mode::Off),
            ("False", Mode::Off),
            ("0", Mode::Off),
        ] {
            assert_eq!(Mode::from_key(word), Some(mode), "{word}");
        }
        for bad in ["", " ", "2", "maybe", "yes please"] {
            assert_eq!(Mode::from_key(bad), None, "{bad:?}");
        }
        let keys = [Mode::Always.key(), Mode::AfterHoming.key(), Mode::Off.key()];
        assert_eq!(
            keys.iter().collect::<std::collections::HashSet<_>>().len(),
            3
        );
    }
}
