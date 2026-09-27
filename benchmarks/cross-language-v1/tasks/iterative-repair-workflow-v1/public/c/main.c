/* Public test entry: four vectors that never let a withdrawal's fee
 * interact with the balance floor. See ../../EQUIVALENCE.md's
 * iterative-repair narrative for why a candidate that passes every one of
 * these is not yet done.
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
    check(process_batch(0, 50, 50, 50, 50, 50), 250, "deposit-only sequence never charges a fee");
    check(process_batch(400, 50, 0, 0, 0, 0), 450, "single deposit baseline");
    check(process_batch(200, -10, -10, -10, -10, -10), 135, "withdrawals mid-range, never approach the floor");
    check(process_batch(480, 50, 0, 0, 0, 0), 500, "a deposit clamps at the ceiling with no fee involved");

    return failures == 0 ? 0 : 1;
}
