// Public implementation: a saturating counter (bounded to [0, 100]) that
// applies five signed 64-bit deltas in sequence, clamping after every
// individual step rather than only once at the end. See
// ../../EQUIVALENCE.md for the exact input/output/boundary contract every
// language implementation of this task must meet.
//
// Compiled and tested with bare `rustc --test` (no Cargo project), matching
// sequence-digest-v1's own fixture: this task has no dependencies, so
// Cargo's build graph and lockfile add nothing this snapshot needs.

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
