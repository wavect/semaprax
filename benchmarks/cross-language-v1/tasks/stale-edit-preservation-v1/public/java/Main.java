// Public test entry: ordinary discounts reduce price. See
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
        check(Candidate.applyDiscount(100, 10), 90, "ten percent off");
        check(Candidate.applyDiscount(200, 25), 150, "twenty five percent off");

        System.exit(failures == 0 ? 0 : 1);
    }
}
