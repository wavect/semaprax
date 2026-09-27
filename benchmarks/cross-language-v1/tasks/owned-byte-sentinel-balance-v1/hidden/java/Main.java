// Hidden overlay: replaces the public `Main.java` verbatim (same relative
// path) for the scoring phase only; `Candidate.java` is unchanged. Vectors
// mixing zero and 0xff sentinels at several positions.
public final class Main {
    private static int failures = 0;

    private static void check(long actual, long expected, String label) {
        if (actual != expected) {
            System.err.println(label + ": expected " + expected + ", got " + actual);
            failures++;
        }
    }

    public static void main(String[] args) {
        check(Candidate.sentinelChecksum(new int[] {0x00, 0xff, 0x00, 0x09}), 1024, "zero and ff");
        check(Candidate.sentinelChecksum(new int[] {0x09, 0x00, 0xff, 0x00, 0x09}), 1536, "two zeroes");

        System.exit(failures == 0 ? 0 : 1);
    }
}
