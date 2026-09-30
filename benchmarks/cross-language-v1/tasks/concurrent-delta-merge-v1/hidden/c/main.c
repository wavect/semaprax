/* Hidden overlay: replaces the public `main.c` verbatim (same relative
 * path) for the scoring phase only, importing the unchanged public
 * `candidate.c`. A ceiling- or floor-side delta must not clamp before the
 * other concurrent delta lands.
 */
#include <stdio.h>
#include "candidate.c"

static int failures = 0;

static void check(long long actual, long long expected, const char *label) {
    if (actual != expected) {
        fprintf(stderr, "%s: expected %lld, got %lld\n", label, expected, actual);
        failures++;
    }
}

int main(void) {
    check(merge_concurrent_deltas(100, 50, -30), 120, "well inside the bound");
    check(merge_concurrent_deltas(500000, 100, 100), 500200, "midrange sum");
    check(merge_concurrent_deltas(10, -5, -3), 2, "negative deltas away from the floor");
    check(merge_concurrent_deltas(0, 0, 0), 0, "identity");

    /* Hidden boundary vectors: never shipped in the public directory tree.
     * A candidate that clamps `delta_a` against `base` before adding
     * `delta_b` -- treating the two concurrent deltas as a sequential
     * edit -- diverges from the correct concurrent merge exactly here. */
    check(merge_concurrent_deltas(999990, 20, -50), 999960,
          "a ceiling-side delta must not clamp before the other concurrent delta lands");
    check(merge_concurrent_deltas(10, -20, 15), 5,
          "a floor-side delta must not clamp before the other concurrent delta lands");

    return failures == 0 ? 0 : 1;
}
