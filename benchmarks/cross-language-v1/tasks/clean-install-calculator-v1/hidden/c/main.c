/* Hidden overlay: replaces the public `main.c` verbatim (same relative
 * path) for the scoring phase only, importing the unchanged public
 * `candidate.c`. Subtraction may produce a negative result, unlike a
 * bounded counter elsewhere in this corpus.
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
    check(subtract(8, 50), -42, "subtraction may go negative, unlike a bounded counter");
    check(subtract(-5, -5), 0, "subtracting equal negatives");
    check(add(19, 23), 42, "the starter operation is unchanged");

    return failures == 0 ? 0 : 1;
}
