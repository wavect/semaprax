// Hidden overlay: replaces the public Main.java verbatim (same relative
// path) for the scoring phase only; Candidate.java is unchanged. Adds the
// two floor-interaction vectors that only a correctly-ordered ("clamp
// last") repair passes, plus three checks on the unrelated tierLabel
// classifier the public suite never calls at all.
public final class Main {
    private static int failures = 0;

    private static void check(long actual, long expected, String label) {
        if (actual != expected) {
            System.err.println(label + ": expected " + expected + ", got " + actual);
            failures++;
        }
    }

    public static void main(String[] args) {
        check(Candidate.processBatch(0, 50, 50, 50, 50, 50), 250, "deposit-only sequence never charges a fee");
        check(Candidate.processBatch(400, 50, 0, 0, 0, 0), 450, "single deposit baseline");
        check(Candidate.processBatch(200, -10, -10, -10, -10, -10), 135, "withdrawals mid-range, never approach the floor");
        check(Candidate.processBatch(480, 50, 0, 0, 0, 0), 500, "a deposit clamps at the ceiling with no fee involved");

        // A withdrawal fee subtracted after the balance is clamped,
        // instead of before, can return a balance below the declared
        // floor whenever the violating step is the sequence's last one.
        // attempt_1 in EQUIVALENCE.md's narrative fixes the deposit-fee
        // defect but keeps this one, so it passes every public vector
        // above and fails both of these.
        check(Candidate.processBatch(3, 0, 0, 0, 0, -3), 0,
                "a final withdrawal that would cross the floor must fold its fee before clamping");
        check(Candidate.processBatch(53, 0, 0, 0, 0, -51), 0,
                "a final withdrawal whose pre-fee sum is still in range must still fold its fee before clamping");

        check(Candidate.tierLabel(50), 0, "tier classifier: low balance");
        check(Candidate.tierLabel(150), 1, "tier classifier: mid balance");
        check(Candidate.tierLabel(350), 2, "tier classifier: high balance");

        System.exit(failures == 0 ? 0 : 1);
    }
}
