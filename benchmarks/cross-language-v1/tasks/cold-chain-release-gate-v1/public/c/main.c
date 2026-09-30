/* Public test entry: accepts readings inside both operating bands; rejects
 * when both readings are outside their bands. See ../../EQUIVALENCE.md.
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
    check(release_allowed(5, 100), 1, "both readings inside operating bands");
    check(release_allowed(1, 94), 0, "both readings below operating bands");
    check(release_allowed(9, 106), 0, "both readings above operating bands");

    return failures == 0 ? 0 : 1;
}
