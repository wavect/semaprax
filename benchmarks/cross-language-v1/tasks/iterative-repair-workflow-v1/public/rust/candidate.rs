// Candidate: a five-step account-ledger repair with a withdrawal fee, plus
// a second, already-correct `tier_label` classifier left by a prior
// session (out of scope for this repair). See ../../EQUIVALENCE.md for the
// full iterative-repair narrative this task's public/hidden split is built
// around: the correct implementation below is the destination of a
// two-step debugging session, not a one-shot draft. Unchanged between the
// public and hidden phases; the hidden overlay replaces only `main.rs`.

fn clamp(value: i64) -> i64 {
    if value < 0 {
        0
    } else if value > 500 {
        500
    } else {
        value
    }
}

// A withdrawal (a negative adjustment) is charged a flat handling fee; a
// deposit (zero or positive) is not.
fn fee(adjustment: i64) -> i64 {
    if adjustment < 0 {
        3
    } else {
        0
    }
}

// One step: fold the adjustment and its fee into the balance, THEN clamp
// the whole result. Subtracting the fee after clamping (`clamp(balance +
// adjustment) - fee(adjustment)`) is the second, masked defect this task
// exists to catch: it can push the returned balance outside [0, 500]
// whenever the pre-fee sum is already at or near the floor.
fn apply_step(balance: i64, adjustment: i64) -> i64 {
    clamp(balance + adjustment - fee(adjustment))
}

pub fn process_batch(b0: i64, a1: i64, a2: i64, a3: i64, a4: i64, a5: i64) -> i64 {
    apply_step(
        apply_step(apply_step(apply_step(apply_step(b0, a1), a2), a3), a4),
        a5,
    )
}

// prior-session: account tier classifier, unrelated to the ledger repair
// above; keep unchanged. A repair that clobbers or deletes this while
// fixing `process_batch` has not preserved a stale, already-completed edit.
pub fn tier_label(balance: i64) -> i64 {
    if balance < 100 {
        0
    } else if balance < 300 {
        1
    } else {
        2
    }
}
