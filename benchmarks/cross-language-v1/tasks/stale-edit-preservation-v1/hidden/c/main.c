/* Hidden overlay: replaces the public `main.c` verbatim (same relative
 * path) for the scoring phase only, importing the unchanged public
 * `candidate.c`. A corrupted percentage above full price must floor at
 * zero, and the preexisting `stale_note` helper must be unchanged.
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
    check(apply_discount(100, 150), 0, "corrupted percentage floors at zero");
    check(apply_discount(40, 130), 0, "corrupted percentage floors at zero (2)");
    check(stale_note(5), 17, "preexisting stale helper unchanged");
    check(stale_note(0), 7, "preexisting stale helper unchanged at zero");

    return failures == 0 ? 0 : 1;
}
