// Public implementation: a saturating counter (bounded to [0, 100]) that
// applies five signed 64-bit deltas in sequence, clamping after every
// individual step rather than only once at the end. See
// ../../EQUIVALENCE.md for the exact input/output/boundary contract every
// language implementation of this task must meet.
//
// Compiled with `javac` and run directly with `java`, exactly as
// sequence-digest-v1's own Java port already establishes for this suite's
// Java lane.
public final class Main {
    private static int failures = 0;

    private static long clamp(long value) {
        if (value > 100) {
            return 100;
        } else if (value < 0) {
            return 0;
        } else {
            return value;
        }
    }

    private static long step(long counter, long delta) {
        return clamp(counter + delta);
    }

    private static long apply5(long c0, long d1, long d2, long d3, long d4, long d5) {
        return step(step(step(step(step(c0, d1), d2), d3), d4), d5);
    }

    private static void check(long actual, long expected, String label) {
        if (actual != expected) {
            System.err.println(label + ": expected " + expected + ", got " + actual);
            failures++;
        }
    }

    public static void main(String[] args) {
        check(apply5(0, 10, 10, 10, 10, 10), 50, "apply5(0,10,10,10,10,10)");
        check(apply5(50, 10, -5, 10, -5, 10), 70, "apply5(50,10,-5,10,-5,10)");
        check(apply5(95, 10, 0, 0, 0, 0), 100, "apply5(95,10,0,0,0,0)");
        check(apply5(5, -10, 0, 0, 0, 0), 0, "apply5(5,-10,0,0,0,0)");

        System.exit(failures == 0 ? 0 : 1);
    }
}
