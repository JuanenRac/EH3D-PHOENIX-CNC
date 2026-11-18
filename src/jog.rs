//! The soft limits of the jog: a step by hand that would take an axis past the travel of the machine is shortened to the limit, and one that
//! starts at the limit is refused. A machine already outside its travel (the zero was lost, the machine was not homed) may still be moved
//! towards the inside. It knows nothing of the controller, so it is tested on its own.

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
}
