// Public test entry: valid boundaries and single-field errors. See
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
        check(Candidate.validate(7, 1, 1), 0, "minimum valid envelope");
        check(Candidate.validate(7, 1, 64), 0, "maximum valid envelope");
        check(Candidate.validate(6, 1, 10), 1, "unknown kind");
        check(Candidate.validate(7, 2, 10), 2, "unsupported version");
        check(Candidate.validate(7, 1, 0), 3, "short payload");
        check(Candidate.validate(7, 1, 65), 3, "long payload");

        System.exit(failures == 0 ? 0 : 1);
    }
}
