/* Hidden overlay: replaces the public `main.c` verbatim (same relative
 * path) for the scoring phase only, importing the unchanged public
 * `candidate.c`. Vectors mixing zero and 0xff sentinels at several
 * positions.
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
    {
        unsigned char v1[] = {0x00, 0xff, 0x00, 0x09};
        check(sentinel_checksum(v1, 4), 1024, "zero and ff");
    }
    {
        unsigned char v2[] = {0x09, 0x00, 0xff, 0x00, 0x09};
        check(sentinel_checksum(v2, 5), 1536, "two zeroes");
    }

    return failures == 0 ? 0 : 1;
}
