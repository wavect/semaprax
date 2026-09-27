// Hidden overlay: replaces the public `Main.java` verbatim (same relative
// path) for the scoring phase only; `Candidate.java` is unchanged. A delta
// that would carry the total past the 32-bit signed boundary must
// saturate instead of wrapping or trapping.
public final class Main {
    private static int failures = 0;

    private static void check(int actual, int expected, String label) {
        if (actual != expected) {
            System.err.println(label + ": expected " + expected + ", got " + actual);
            failures++;
        }
    }

    public static void main(String[] args) {
        check(Candidate.combineTelemetry(Integer.MAX_VALUE, 1), Integer.MAX_VALUE, "saturates at the positive boundary");
        check(Candidate.combineTelemetry(Integer.MIN_VALUE, -1), Integer.MIN_VALUE, "saturates at the negative boundary");

        System.exit(failures == 0 ? 0 : 1);
    }
}
