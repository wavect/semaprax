/* Hidden overlay: replaces the public `main.c` verbatim (same relative
 * path) for the scoring phase only, importing the unchanged public
 * `candidate.c`. Forward and reverse adjacency (a shared boundary instant,
 * not a shared interior instant) must not conflict.
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
    check(conflicts(10, 20, 20, 25), 0, "forward adjacency");
    check(conflicts(20, 25, 10, 20), 0, "reverse adjacency");

    return failures == 0 ? 0 : 1;
}
