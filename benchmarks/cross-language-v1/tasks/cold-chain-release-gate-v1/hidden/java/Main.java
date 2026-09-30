// Hidden overlay: replaces the public `Main.java` verbatim (same relative
// path) for the scoring phase only; `Candidate.java` is unchanged. A good
// reading in one sensor must never override a bad reading in the other,
// and the band edges are inclusive.
public final class Main {
    private static int failures = 0;

    private static void check(long actual, long expected, String label) {
        if (actual != expected) {
            System.err.println(label + ": expected " + expected + ", got " + actual);
            failures++;
        }
    }

    public static void main(String[] args) {
        check(Candidate.releaseAllowed(5, 106), 0, "good temperature cannot override bad pressure");
        check(Candidate.releaseAllowed(1, 100), 0, "good pressure cannot override bad temperature");
        check(Candidate.releaseAllowed(2, 95), 1, "lower inclusive edges");
        check(Candidate.releaseAllowed(8, 105), 1, "upper inclusive edges");

        System.exit(failures == 0 ? 0 : 1);
    }
}
