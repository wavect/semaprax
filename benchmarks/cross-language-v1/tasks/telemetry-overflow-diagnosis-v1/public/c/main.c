/* Public test entry: ordinary deltas combine by plain addition. See
 * ../../EQUIVALENCE.md for the exact contract.
 */
#include <stdio.h>
#include "candidate.c"

static int failures = 0;

static void check(int32_t actual, int32_t expected, const char *label) {
    if (actual != expected) {
        fprintf(stderr, "%s: expected %d, got %d\n", label, expected, actual);
        failures++;
    }
}

int main(void) {
    check(combine_telemetry(10, 20), 30, "ordinary positive deltas");
    check(combine_telemetry(-5, 5), 0, "ordinary mixed deltas");

    return failures == 0 ? 0 : 1;
}
