/* Hidden overlay: replaces the public `main.c` verbatim (same relative
 * path) for the scoring phase only, importing the unchanged public
 * `candidate.c`/`helper.c`. Vectors where the whole-subtotal tax must
 * precede shipping and round (truncate) exactly once.
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
    check(invoice_total(19, 3, 8, 10), 71, "whole subtotal tax before shipping");
    check(invoice_total(7, 5, 13, 9), 48, "tax rounds once");
    check(invoice_total(1, 64, 17, 3), 77, "bounded quantity");

    return failures == 0 ? 0 : 1;
}
