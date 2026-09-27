/* Hidden overlay: replaces the public `main.c` verbatim (same relative path)
 * for the scoring phase only, adding hidden vectors that exercise the classic
 * off-by-one repair bug this task is about: clamping only the final summed
 * delta instead of clamping the running counter after every individual step.
 * Implementation functions are unchanged from the public file.
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

    /* Hidden cases: never shipped in the public directory tree. A single
     * end-of-sequence clamp instead of a per-step clamp gets both of these
     * wrong (100 and 35, respectively); the correct stepwise counter clamps
     * after every delta and gets 70 and 50.
     */
    check(apply5(90, 50, -30, 0, 0, 0), 70, "apply5(90,50,-30,0,0,0) saturates upward then recovers downward");
    check(apply5(5, -20, 50, 0, 0, 0), 50, "apply5(5,-20,50,0,0,0) floors downward then recovers upward");

    return failures == 0 ? 0 : 1;
}
