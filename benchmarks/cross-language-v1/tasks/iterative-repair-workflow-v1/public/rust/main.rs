mod candidate;

// Public tests: four vectors that never let a withdrawal's fee interact
// with the balance floor. A candidate that still charges the withdrawal
// fee on deposits too (the first, surface-level defect this task's
// narrative attempt_0 has) fails `deposit_only_sequence_never_charges_a_fee`
// immediately; a candidate that only fixed that (attempt_1: fee correctly
// skipped on deposits, but still subtracted *after* the balance is
// clamped) passes every vector here, because none of them push a
// withdrawal step's pre-fee sum down to the floor. See
// ../../EQUIVALENCE.md's iterative-repair narrative for the log excerpt
// that motivates why a solver must keep going past attempt_1.
#[cfg(test)]
mod public_tests {
    use super::candidate::process_batch;

    #[test]
    fn deposit_only_sequence_never_charges_a_fee() {
        assert_eq!(process_batch(0, 50, 50, 50, 50, 50), 250);
        assert_eq!(process_batch(400, 50, 0, 0, 0, 0), 450);
    }

    #[test]
    fn withdrawals_mid_range_never_approach_the_floor() {
        assert_eq!(process_batch(200, -10, -10, -10, -10, -10), 135);
    }

    #[test]
    fn a_deposit_clamps_at_the_ceiling_with_no_fee_involved() {
        assert_eq!(process_batch(480, 50, 0, 0, 0, 0), 500);
    }
}

fn main() {}
