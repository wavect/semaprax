// Public test entry: merges two deltas well inside the shared bound, and
// negative deltas that never approach the floor. See ../../EQUIVALENCE.md.
public final class Main {
    private static int failures = 0;

    private static void check(long actual, long expected, String label) {
        if (actual != expected) {
            System.err.println(label + ": expected " + expected + ", got " + actual);
            failures++;
        }
    }

    public static void main(String[] args) {
        check(Candidate.mergeConcurrentDeltas(100, 50, -30), 120, "well inside the bound");
        check(Candidate.mergeConcurrentDeltas(500_000, 100, 100), 500_200, "midrange sum");
        check(Candidate.mergeConcurrentDeltas(10, -5, -3), 2, "negative deltas away from the floor");
        check(Candidate.mergeConcurrentDeltas(0, 0, 0), 0, "identity");

        System.exit(failures == 0 ? 0 : 1);
    }
}
