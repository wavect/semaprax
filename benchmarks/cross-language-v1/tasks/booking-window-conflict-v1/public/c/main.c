/* Public test entry: overlapping and contained bookings conflict; a real
 * gap does not. See ../../EQUIVALENCE.md for the exact contract.
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
    check(conflicts(10, 20, 15, 25), 1, "proper overlap");
    check(conflicts(10, 30, 12, 18), 1, "contained booking");
    check(conflicts(10, 20, 25, 30), 0, "strictly separated bookings");

    return failures == 0 ? 0 : 1;
}
