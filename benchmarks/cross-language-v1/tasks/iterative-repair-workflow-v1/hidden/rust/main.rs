mod candidate;

// Hidden overlay: replaces the public `main.rs` verbatim (same relative
// path) for the scoring phase only, importing the unchanged public
// `candidate.rs`. Adds the two floor-interaction vectors that only a
// correctly-ordered ("clamp last") repair passes, plus three checks on the
// unrelated `tier_label` classifier the public suite never calls at all.
#[cfg(test)]
mod hidden_tests {
    use super::candidate::{process_batch, tier_label};

    #[test]
    fn repeats_the_public_vectors_against_the_copied_candidate() {
        assert_eq!(process_batch(0, 50, 50, 50, 50, 50), 250);
        assert_eq!(process_batch(400, 50, 0, 0, 0, 0), 450);
        assert_eq!(process_batch(200, -10, -10, -10, -10, -10), 135);
        assert_eq!(process_batch(480, 50, 0, 0, 0, 0), 500);
    }

    // A withdrawal fee subtracted after the balance is clamped, instead of
    // before, can return a balance below the declared floor whenever the
    // violating step is the sequence's last one (an earlier violation is
    // masked: the next step's own clamp floors it back to zero before the
    // final value is read). `attempt_1` in EQUIVALENCE.md's narrative
    // fixes the deposit-fee defect but keeps this one, so it passes every
    // public vector above and fails both of these.
    #[test]
    fn a_final_withdrawal_that_would_cross_the_floor_must_fold_its_fee_before_clamping() {
        assert_eq!(process_batch(3, 0, 0, 0, 0, -3), 0);
    }

    #[test]
    fn a_final_withdrawal_whose_pre_fee_sum_is_still_in_range_must_still_fold_its_fee_before_clamping(
    ) {
        assert_eq!(process_batch(53, 0, 0, 0, 0, -51), 0);
    }

    #[test]
    fn the_unrelated_tier_classifier_is_untouched_by_the_ledger_repair() {
        assert_eq!(tier_label(50), 0);
        assert_eq!(tier_label(150), 1);
        assert_eq!(tier_label(350), 2);
    }
}

fn main() {}
