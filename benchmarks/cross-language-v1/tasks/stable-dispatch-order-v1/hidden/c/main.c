/* Hidden overlay: replaces the public `main.c` verbatim (same relative
 * path) for the scoring phase only, importing the unchanged public
 * `candidate.c`. Ties must preserve arrival order, including a three-way
 * tie.
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
    check(dispatch_order(1, 1, 2), 123, "first two arrivals tie");
    check(dispatch_order(1, 2, 1), 132, "first and last arrivals tie");
    check(dispatch_order(2, 1, 1), 231, "last two arrivals tie");
    check(dispatch_order(7, 7, 7), 123, "all arrivals tie");

    return failures == 0 ? 0 : 1;
}
