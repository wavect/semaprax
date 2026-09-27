// Hidden overlay: replaces the public `Main.java` verbatim (same relative
// path) for the scoring phase only; `Candidate.java` is unchanged. A
// corrupted percentage above full price must floor at zero, and the
// preexisting `staleNote` helper must be unchanged.
public final class Main {
    private static int failures = 0;

    private static void check(long actual, long expected, String label) {
        if (actual != expected) {
            System.err.println(label + ": expected " + expected + ", got " + actual);
            failures++;
        }
    }

    public static void main(String[] args) {
        check(Candidate.applyDiscount(100, 150), 0, "corrupted percentage floors at zero");
        check(Candidate.applyDiscount(40, 130), 0, "corrupted percentage floors at zero (2)");
        check(Candidate.staleNote(5), 17, "preexisting stale helper unchanged");
        check(Candidate.staleNote(0), 7, "preexisting stale helper unchanged at zero");

        System.exit(failures == 0 ? 0 : 1);
    }
}
