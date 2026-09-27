// Public test entry: orders distinct priorities from each initial
// position. See ../../EQUIVALENCE.md for the exact contract.
public final class Main {
    private static int failures = 0;

    private static void check(long actual, long expected, String label) {
        if (actual != expected) {
            System.err.println(label + ": expected " + expected + ", got " + actual);
            failures++;
        }
    }

    public static void main(String[] args) {
        check(Candidate.dispatchOrder(1, 2, 3), 123, "already ordered distinct priorities");
        check(Candidate.dispatchOrder(3, 1, 2), 231, "middle arrival has smallest priority");
        check(Candidate.dispatchOrder(2, 3, 1), 312, "last arrival has smallest priority");

        System.exit(failures == 0 ? 0 : 1);
    }
}
