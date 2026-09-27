// Candidate: a five-step account-ledger repair with a withdrawal fee, plus
// a second, already-correct tierLabel classifier left by a prior session
// (out of scope for this repair). See ../../EQUIVALENCE.md for the full
// iterative-repair narrative this task's public/hidden split is built
// around. Unchanged between the public and hidden phases; the hidden
// overlay replaces only Main.java.
public final class Candidate {
    private static long clampBalance(long value) {
        if (value < 0) {
            return 0;
        } else if (value > 500) {
            return 500;
        } else {
            return value;
        }
    }

    // A withdrawal (a negative adjustment) is charged a flat handling fee;
    // a deposit (zero or positive) is not.
    private static long fee(long adjustment) {
        return adjustment < 0 ? 3 : 0;
    }

    // One step: fold the adjustment and its fee into the balance, THEN
    // clamp the whole result. Subtracting the fee after clamping is the
    // second, masked defect this task exists to catch.
    private static long applyStep(long balance, long adjustment) {
        return clampBalance(balance + adjustment - fee(adjustment));
    }

    static long processBatch(long b0, long a1, long a2, long a3, long a4, long a5) {
        return applyStep(applyStep(applyStep(applyStep(applyStep(b0, a1), a2), a3), a4), a5);
    }

    // prior-session: account tier classifier, unrelated to the ledger
    // repair above; keep unchanged.
    static long tierLabel(long balance) {
        if (balance < 100) {
            return 0;
        } else if (balance < 300) {
            return 1;
        } else {
            return 2;
        }
    }
}
