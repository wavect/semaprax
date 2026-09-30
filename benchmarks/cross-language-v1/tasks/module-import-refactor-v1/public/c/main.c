/* Public test entry: exact tax and zero-shipping cases. See
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
    check(invoice_total(10, 2, 10, 5), 27, "exact tax with shipping");
    check(invoice_total(40, 2, 25, 0), 100, "exact tax without shipping");
    check(invoice_total(25, 4, 20, 10), 130, "larger exact subtotal");

    return failures == 0 ? 0 : 1;
}
