/* Public test entry: ordinary discounts reduce price. See
 * ../../EQUIVALENCE.md for the exact contract.
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
    check(apply_discount(100, 10), 90, "ten percent off");
    check(apply_discount(200, 25), 150, "twenty five percent off");

    return failures == 0 ? 0 : 1;
}
