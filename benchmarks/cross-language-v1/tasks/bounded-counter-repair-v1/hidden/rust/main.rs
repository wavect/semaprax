// Hidden overlay: replaces the public `main.rs` verbatim (same relative
// path) for the scoring phase only, adding a hidden test module that
// exercises the classic off-by-one repair bug this task is about: clamping
// only the final summed delta instead of clamping the running counter after
// every individual step. Implementation functions are unchanged from the
// public file.

fn clamp(value: i64) -> i64 {
    if value > 100 {
        100
    } else if value < 0 {
        0
    } else {
        value
    }
}

fn step(counter: i64, delta: i64) -> i64 {
    clamp(counter + delta)
}

fn apply5(c0: i64, d1: i64, d2: i64, d3: i64, d4: i64, d5: i64) -> i64 {
    step(step(step(step(step(c0, d1), d2), d3), d4), d5)
}

fn main() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn never_leaves_bounds_upward() {
        assert_eq!(apply5(0, 10, 10, 10, 10, 10), 50);
        assert_eq!(apply5(95, 10, 0, 0, 0, 0), 100);
    }

    #[test]
    fn never_leaves_bounds_downward_or_mixed() {
        assert_eq!(apply5(50, 10, -5, 10, -5, 10), 70);
        assert_eq!(apply5(5, -10, 0, 0, 0, 0), 0);
    }
}

#[cfg(test)]
mod hidden_tests {
    use super::*;

    // A single end-of-sequence clamp instead of a per-step clamp sums
    // 90+50-30=110 and clamps once to 100; the correct stepwise counter
    // clamps 90+50 to 100 first, then applies -30 to reach 70.
    #[test]
    fn saturates_upward_then_recovers_downward() {
        assert_eq!(apply5(90, 50, -30, 0, 0, 0), 70);
    }

    // A single end-of-sequence clamp instead of a per-step clamp sums
    // 5-20+50=35 and never needs to clamp at all; the correct stepwise
    // counter clamps 5-20 to 0 first, then applies +50 to reach 50.
    #[test]
    fn floors_downward_then_recovers_upward() {
        assert_eq!(apply5(5, -20, 50, 0, 0, 0), 50);
    }
}
