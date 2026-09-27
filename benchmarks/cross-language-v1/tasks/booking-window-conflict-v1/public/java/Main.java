// Public test entry: overlapping and contained bookings conflict; a real
// gap does not. See ../../EQUIVALENCE.md for the exact contract.
public final class Main {
    private static int failures = 0;

    private static void check(long actual, long expected, String label) {
        if (actual != expected) {
            System.err.println(label + ": expected " + expected + ", got " + actual);
            failures++;
        }
    }

    public static void main(String[] args) {
        check(Candidate.conflicts(10, 20, 15, 25), 1, "proper overlap");
        check(Candidate.conflicts(10, 30, 12, 18), 1, "contained booking");
        check(Candidate.conflicts(10, 20, 25, 30), 0, "strictly separated bookings");

        System.exit(failures == 0 ? 0 : 1);
    }
}
