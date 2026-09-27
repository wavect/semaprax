// Hidden overlay: replaces the public `Main.java` verbatim (same relative
// path) for the scoring phase only; `Candidate.java` is unchanged.
// Subtraction may produce a negative result, unlike a bounded counter
// elsewhere in this corpus.
public final class Main {
    private static int failures = 0;

    private static void check(long actual, long expected, String label) {
        if (actual != expected) {
            System.err.println(label + ": expected " + expected + ", got " + actual);
            failures++;
        }
    }

    public static void main(String[] args) {
        check(Candidate.subtract(8, 50), -42, "subtraction may go negative, unlike a bounded counter");
        check(Candidate.subtract(-5, -5), 0, "subtracting equal negatives");
        check(Candidate.add(19, 23), 42, "the starter operation is unchanged");

        System.exit(failures == 0 ? 0 : 1);
    }
}
