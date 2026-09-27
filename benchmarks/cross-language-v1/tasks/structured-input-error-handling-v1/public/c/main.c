/* Public test entry: valid boundaries and single-field errors. See
 * ../../EQUIVALENCE.md for the exact contract. `main` is the official test
 * runner, exactly as sequence-digest-v1's own C port establishes for this
 * suite's C lane.
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
    check(validate(7, 1, 1), 0, "minimum valid envelope");
    check(validate(7, 1, 64), 0, "maximum valid envelope");
    check(validate(6, 1, 10), 1, "unknown kind");
    check(validate(7, 2, 10), 2, "unsupported version");
    check(validate(7, 1, 0), 3, "short payload");
    check(validate(7, 1, 65), 3, "long payload");

    return failures == 0 ? 0 : 1;
}
