/* Public test entry: merges two deltas well inside the shared bound, and
 * negative deltas that never approach the floor. See ../../EQUIVALENCE.md.
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

    return failures == 0 ? 0 : 1;
}
