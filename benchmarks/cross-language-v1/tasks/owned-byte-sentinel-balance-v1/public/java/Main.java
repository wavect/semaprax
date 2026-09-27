// Public test entry: public sentinel vectors. See ../../EQUIVALENCE.md for
// the exact contract.
public final class Main {
    private static int failures = 0;

    private static void check(long actual, long expected, String label) {
        if (actual != expected) {
            System.err.println(label + ": expected " + expected + ", got " + actual);
            failures++;
        }
    }

    public static void main(String[] args) {
        check(Candidate.sentinelChecksum(new int[] {0xff}), 0, "one ff");
        check(Candidate.sentinelChecksum(new int[] {0xff, 0x07, 0xff}), 2, "two ff");
        check(Candidate.sentinelChecksum(new int[] {0x01, 0x02, 0x7f}), 6, "other bytes");

        System.exit(failures == 0 ? 0 : 1);
    }
}
