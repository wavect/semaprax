// Public test entry: ordinary deltas combine by plain addition. See
// ../../EQUIVALENCE.md for the exact contract.
public final class Main {
    private static int failures = 0;

    private static void check(int actual, int expected, String label) {
        if (actual != expected) {
            System.err.println(label + ": expected " + expected + ", got " + actual);
            failures++;
        }
    }

    public static void main(String[] args) {
        check(Candidate.combineTelemetry(10, 20), 30, "ordinary positive deltas");
        check(Candidate.combineTelemetry(-5, 5), 0, "ordinary mixed deltas");

        System.exit(failures == 0 ? 0 : 1);
    }
}
