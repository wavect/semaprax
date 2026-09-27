/* Public test entry: public sentinel vectors. See ../../EQUIVALENCE.md for
 * the exact contract.
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
        unsigned char v1[] = {0xff};
        check(sentinel_checksum(v1, 1), 0, "one ff");
    }
    {
        unsigned char v2[] = {0xff, 0x07, 0xff};
        check(sentinel_checksum(v2, 3), 2, "two ff");
    }
    {
        unsigned char v3[] = {0x01, 0x02, 0x7f};
        check(sentinel_checksum(v3, 3), 6, "other bytes");
    }

    return failures == 0 ? 0 : 1;
}
