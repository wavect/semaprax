// Hidden overlay: replaces the public `Main.java` verbatim (same relative
// path) for the scoring phase only, adding two hidden vectors a solver never
// sees. Implementation functions are unchanged from the public file.
public final class Main {
    private static int failures = 0;

    private static long isEven(long value) { return value % 2 == 0 ? 1 : 0; }
    private static long isNegative(long value) { return value < 0 ? 1 : 0; }
    private static long max2(long left, long right) { return left > right ? left : right; }

    private static long total(long a, long b, long c, long d, long e) { return a + b + c + d + e; }

    private static long countEven(long a, long b, long c, long d, long e) {
        return isEven(a) + isEven(b) + isEven(c) + isEven(d) + isEven(e);
    }

    private static long countNegative(long a, long b, long c, long d, long e) {
        return isNegative(a) + isNegative(b) + isNegative(c) + isNegative(d) + isNegative(e);
    }

    private static long maxOf(long a, long b, long c, long d, long e) {
        return max2(max2(max2(a, b), max2(c, d)), e);
    }

    private static void check(long actual, long expected, String label) {
        if (actual != expected) {
            System.err.println(label + ": expected " + expected + ", got " + actual);
            failures++;
        }
    }

    public static void main(String[] args) {
        check(total(1, 2, 3, 4, 5), 15, "sum(1,2,3,4,5)");
        check(countEven(1, 2, 3, 4, 5), 2, "countEven(1,2,3,4,5)");
        check(countNegative(1, 2, 3, 4, 5), 0, "countNegative(1,2,3,4,5)");
        check(maxOf(1, 2, 3, 4, 5), 5, "maxOf(1,2,3,4,5)");

        check(total(-1, -2, -3, -4, -5), -15, "sum(-1,-2,-3,-4,-5)");
        check(countEven(-1, -2, -3, -4, -5), 2, "countEven(-1,-2,-3,-4,-5)");
        check(countNegative(-1, -2, -3, -4, -5), 5, "countNegative(-1,-2,-3,-4,-5)");
        check(maxOf(-1, -2, -3, -4, -5), -1, "maxOf(-1,-2,-3,-4,-5)");

        // Hidden cases: never shipped in the public directory tree.
        check(total(0, 0, 0, 0, 0), 0, "sum(0,0,0,0,0)");
        check(countEven(0, 0, 0, 0, 0), 5, "countEven(0,0,0,0,0)");
        check(countNegative(0, 0, 0, 0, 0), 0, "countNegative(0,0,0,0,0)");
        check(maxOf(0, 0, 0, 0, 0), 0, "maxOf(0,0,0,0,0)");

        check(total(-100, 7, 7, 7, 100), 21, "sum(-100,7,7,7,100)");
        check(countEven(-100, 7, 7, 7, 100), 2, "countEven(-100,7,7,7,100)");
        check(countNegative(-100, 7, 7, 7, 100), 1, "countNegative(-100,7,7,7,100)");
        check(maxOf(-100, 7, 7, 7, 100), 100, "maxOf(-100,7,7,7,100)");

        System.exit(failures == 0 ? 0 : 1);
    }
}
