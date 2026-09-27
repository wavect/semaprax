// Hidden overlay: replaces the public `Main.java` verbatim (same relative
// path) for the scoring phase only; `Candidate.java` is unchanged. A
// ceiling- or floor-side delta must not clamp before the other concurrent
// delta lands.
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

        // Hidden boundary vectors: never shipped in the public directory
        // tree. A candidate that clamps deltaA against base before adding
        // deltaB -- treating the two concurrent deltas as a sequential
        // edit -- diverges from the correct concurrent merge exactly here.
        check(Candidate.mergeConcurrentDeltas(999_990, 20, -50), 999_960,
                "a ceiling-side delta must not clamp before the other concurrent delta lands");
        check(Candidate.mergeConcurrentDeltas(10, -20, 15), 5,
                "a floor-side delta must not clamp before the other concurrent delta lands");

        System.exit(failures == 0 ? 0 : 1);
    }
}
