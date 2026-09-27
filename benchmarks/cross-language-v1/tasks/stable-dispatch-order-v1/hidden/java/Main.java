// Hidden overlay: replaces the public `Main.java` verbatim (same relative
// path) for the scoring phase only; `Candidate.java` is unchanged. Ties
// must preserve arrival order, including a three-way tie.
public final class Main {
    private static int failures = 0;

    private static void check(long actual, long expected, String label) {
        if (actual != expected) {
            System.err.println(label + ": expected " + expected + ", got " + actual);
            failures++;
        }
    }

    public static void main(String[] args) {
        check(Candidate.dispatchOrder(1, 1, 2), 123, "first two arrivals tie");
        check(Candidate.dispatchOrder(1, 2, 1), 132, "first and last arrivals tie");
        check(Candidate.dispatchOrder(2, 1, 1), 231, "last two arrivals tie");
        check(Candidate.dispatchOrder(7, 7, 7), 123, "all arrivals tie");

        System.exit(failures == 0 ? 0 : 1);
    }
}
