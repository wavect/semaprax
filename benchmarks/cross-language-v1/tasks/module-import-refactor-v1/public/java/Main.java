// Public test entry: exact tax and zero-shipping cases. See
// ../../EQUIVALENCE.md for the exact contract.
public final class Main {
    private static int failures = 0;

    private static void check(long actual, long expected, String label) {
        if (actual != expected) {
            System.err.println(label + ": expected " + expected + ", got " + actual);
            failures++;
        }
    }

    public static void main(String[] args) {
        check(Candidate.invoiceTotal(10, 2, 10, 5), 27, "exact tax with shipping");
        check(Candidate.invoiceTotal(40, 2, 25, 0), 100, "exact tax without shipping");
        check(Candidate.invoiceTotal(25, 4, 20, 10), 130, "larger exact subtotal");

        System.exit(failures == 0 ? 0 : 1);
    }
}
