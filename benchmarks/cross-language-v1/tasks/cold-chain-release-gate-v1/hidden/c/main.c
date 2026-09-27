/* Hidden overlay: replaces the public `main.c` verbatim (same relative
 * path) for the scoring phase only, importing the unchanged public
 * `candidate.c`. A good reading in one sensor must never override a bad
 * reading in the other, and the band edges are inclusive.
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
    check(release_allowed(5, 106), 0, "good temperature cannot override bad pressure");
    check(release_allowed(1, 100), 0, "good pressure cannot override bad temperature");
    check(release_allowed(2, 95), 1, "lower inclusive edges");
    check(release_allowed(8, 105), 1, "upper inclusive edges");

    return failures == 0 ? 0 : 1;
}
