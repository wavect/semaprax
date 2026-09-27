/* Candidate: a five-step account-ledger repair with a withdrawal fee, plus
 * a second, already-correct tier_label classifier left by a prior session
 * (out of scope for this repair). See ../../EQUIVALENCE.md for the full
 * iterative-repair narrative this task's public/hidden split is built
 * around. Unchanged between the public and hidden phases; the hidden
 * overlay replaces only main.c, which #includes this file.
 */
static long long clamp_balance(long long value) {
    if (value < 0) {
        return 0;
    } else if (value > 500) {
        return 500;
    } else {
        return value;
    }
}

/* A withdrawal (a negative adjustment) is charged a flat handling fee; a
 * deposit (zero or positive) is not. */
static long long fee(long long adjustment) {
    return adjustment < 0 ? 3 : 0;
}

/* One step: fold the adjustment and its fee into the balance, THEN clamp
 * the whole result. Subtracting the fee after clamping is the second,
 * masked defect this task exists to catch. */
static long long apply_step(long long balance, long long adjustment) {
    return clamp_balance(balance + adjustment - fee(adjustment));
}

static long long process_batch(long long b0, long long a1, long long a2, long long a3, long long a4, long long a5) {
    return apply_step(apply_step(apply_step(apply_step(apply_step(b0, a1), a2), a3), a4), a5);
}

/* prior-session: account tier classifier, unrelated to the ledger repair
 * above; keep unchanged. */
static long long tier_label(long long balance) {
    if (balance < 100) {
        return 0;
    } else if (balance < 300) {
        return 1;
    } else {
        return 2;
    }
}
