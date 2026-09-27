/* Public test entry: the starter operation and its new sibling compute
 * correctly. See ../../EQUIVALENCE.md for the exact contract.
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
    check(add(19, 23), 42, "the starter operation still works");
    check(subtract(50, 8), 42, "the new sibling operation");

    return failures == 0 ? 0 : 1;
}
