/* Public test entry: orders distinct priorities from each initial
 * position. See ../../EQUIVALENCE.md for the exact contract.
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
    check(dispatch_order(1, 2, 3), 123, "already ordered distinct priorities");
    check(dispatch_order(3, 1, 2), 231, "middle arrival has smallest priority");
    check(dispatch_order(2, 3, 1), 312, "last arrival has smallest priority");

    return failures == 0 ? 0 : 1;
}
