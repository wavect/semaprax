// Public test entry: accepts readings inside both operating bands; rejects
// when both readings are outside their bands. See ../../EQUIVALENCE.md.
public final class Main {
    private static int failures = 0;

    private static void check(long actual, long expected, String label) {
        if (actual != expected) {
            System.err.println(label + ": expected " + expected + ", got " + actual);
            failures++;
        }
    }

    public static void main(String[] args) {
        check(Candidate.releaseAllowed(5, 100), 1, "both readings inside operating bands");
        check(Candidate.releaseAllowed(1, 94), 0, "both readings below operating bands");
        check(Candidate.releaseAllowed(9, 106), 0, "both readings above operating bands");

        System.exit(failures == 0 ? 0 : 1);
    }
}
