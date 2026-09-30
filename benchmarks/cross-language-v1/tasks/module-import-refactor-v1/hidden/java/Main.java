// Hidden overlay: replaces the public `Main.java` verbatim (same relative
// path) for the scoring phase only; `Candidate.java`/`Helper.java` are
// unchanged. Vectors where the whole-subtotal tax must precede shipping and
// round (truncate) exactly once.
public final class Main {
    private static int failures = 0;

    private static void check(long actual, long expected, String label) {
        if (actual != expected) {
            System.err.println(label + ": expected " + expected + ", got " + actual);
            failures++;
        }
    }

    public static void main(String[] args) {
        check(Candidate.invoiceTotal(19, 3, 8, 10), 71, "whole subtotal tax before shipping");
        check(Candidate.invoiceTotal(7, 5, 13, 9), 48, "tax rounds once");
        check(Candidate.invoiceTotal(1, 64, 17, 3), 77, "bounded quantity");

        System.exit(failures == 0 ? 0 : 1);
    }
}
