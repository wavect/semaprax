/* Hidden overlay: replaces the public `main.c` verbatim (same relative
 * path) for the scoring phase only, importing the unchanged public
 * `candidate.c`. A delta that would carry the total past the 32-bit signed
 * boundary must saturate instead of wrapping or trapping.
 */
#include <stdio.h>
#include "candidate.c"

static int failures = 0;

static void check(int32_t actual, int32_t expected, const char *label) {
    if (actual != expected) {
        fprintf(stderr, "%s: expected %d, got %d\n", label, expected, actual);
        failures++;
    }
}

int main(void) {
    check(combine_telemetry(INT32_MAX, 1), INT32_MAX, "saturates at the positive boundary");
    check(combine_telemetry(INT32_MIN, -1), INT32_MIN, "saturates at the negative boundary");

    return failures == 0 ? 0 : 1;
}
