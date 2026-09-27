// Public test entry: the starter operation and its new sibling compute
// correctly. See ../../EQUIVALENCE.md for the exact contract.
public final class Main {
    private static int failures = 0;

    private static void check(long actual, long expected, String label) {
        if (actual != expected) {
            System.err.println(label + ": expected " + expected + ", got " + actual);
            failures++;
        }
    }

    public static void main(String[] args) {
        check(Candidate.add(19, 23), 42, "the starter operation still works");
        check(Candidate.subtract(50, 8), 42, "the new sibling operation");

        System.exit(failures == 0 ? 0 : 1);
    }
}
