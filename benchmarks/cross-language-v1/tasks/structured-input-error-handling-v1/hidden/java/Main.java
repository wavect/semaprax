// Hidden overlay: replaces the public `Main.java` verbatim (same relative
// path) for the scoring phase only; `Candidate.java` is unchanged.
// Compound-invalid envelopes make the first-error precedence independently
// observable: a candidate that checks version before kind, or accepts an
// out-of-range length, fails here.
public final class Main {
    private static int failures = 0;

    private static void check(long actual, long expected, String label) {
        if (actual != expected) {
            System.err.println(label + ": expected " + expected + ", got " + actual);
            failures++;
        }
    }

    public static void main(String[] args) {
        check(Candidate.validate(6, 2, 0), 1, "kind precedes version and length");
        check(Candidate.validate(7, 2, 0), 2, "version precedes length");
        check(Candidate.validate(7, 1, -1), 3, "negative payload");
        check(Candidate.validate(7, 1, 65), 3, "large payload");
        check(Candidate.validate(7, 1, 64), 0, "maximum valid payload");
        check(Candidate.validate(7, 1, 1), 0, "minimum valid payload");

        System.exit(failures == 0 ? 0 : 1);
    }
}
