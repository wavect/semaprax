/* Public implementation: a saturating counter (bounded to [0, 100]) that
 * applies five signed 64-bit deltas in sequence, clamping after every
 * individual step rather than only once at the end. See ../../EQUIVALENCE.md
 * for the exact input/output/boundary contract every language implementation
 * of this task must meet.
 *
 * `main` itself is the official test runner, exactly as sequence-digest-v1's
 * C port already establishes for this suite's C lane.
 */
#include <stdio.h>

static long long clamp(long long value) {
    if (value > 100) {
        return 100;
    } else if (value < 0) {
        return 0;
    } else {
        return value;
    }
}

static long long step(long long counter, long long delta) {
    return clamp(counter + delta);
}

static long long apply5(long long c0, long long d1, long long d2, long long d3, long long d4, long long d5) {
    return step(step(step(step(step(c0, d1), d2), d3), d4), d5);
}

static int failures = 0;

static void check(long long actual, long long expected, const char *label) {
    if (actual != expected) {
        fprintf(stderr, "%s: expected %lld, got %lld\n", label, expected, actual);
        failures++;
    }
}

int main(void) {
    check(apply5(0, 10, 10, 10, 10, 10), 50, "apply5(0,10,10,10,10,10)");
    check(apply5(50, 10, -5, 10, -5, 10), 70, "apply5(50,10,-5,10,-5,10)");
    check(apply5(95, 10, 0, 0, 0, 0), 100, "apply5(95,10,0,0,0,0)");
    check(apply5(5, -10, 0, 0, 0, 0), 0, "apply5(5,-10,0,0,0,0)");

    return failures == 0 ? 0 : 1;
}
