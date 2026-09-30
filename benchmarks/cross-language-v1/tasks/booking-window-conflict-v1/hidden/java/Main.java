// Hidden overlay: replaces the public `Main.java` verbatim (same relative
// path) for the scoring phase only; `Candidate.java` is unchanged. Forward
// and reverse adjacency (a shared boundary instant, not a shared interior
// instant) must not conflict.
public final class Main {
    private static int failures = 0;

    private static void check(long actual, long expected, String label) {
        if (actual != expected) {
            System.err.println(label + ": expected " + expected + ", got " + actual);
            failures++;
        }
    }

    public static void main(String[] args) {
        check(Candidate.conflicts(10, 20, 20, 25), 0, "forward adjacency");
        check(Candidate.conflicts(20, 25, 10, 20), 0, "reverse adjacency");

        System.exit(failures == 0 ? 0 : 1);
    }
}
