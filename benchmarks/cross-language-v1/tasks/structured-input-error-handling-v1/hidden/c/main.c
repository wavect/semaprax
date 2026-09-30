/* Hidden overlay: replaces the public `main.c` verbatim (same relative
 * path) for the scoring phase only, importing the unchanged public
 * `candidate.c`. Compound-invalid envelopes make the first-error precedence
 * independently observable: a candidate that checks version before kind, or
 * accepts an out-of-range length, fails here.
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
    check(validate(6, 2, 0), 1, "kind precedes version and length");
    check(validate(7, 2, 0), 2, "version precedes length");
    check(validate(7, 1, -1), 3, "negative payload");
    check(validate(7, 1, 65), 3, "large payload");
    check(validate(7, 1, 64), 0, "maximum valid payload");
    check(validate(7, 1, 1), 0, "minimum valid payload");

    return failures == 0 ? 0 : 1;
}
