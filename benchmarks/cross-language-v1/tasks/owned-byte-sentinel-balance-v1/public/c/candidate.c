/* Candidate: map sentinels in an owned byte buffer and compute the
 * one-based positional checksum of the transformed bytes. Unchanged
 * between the public and hidden phases; the hidden overlay replaces only
 * `main.c`, which #includes this file.
 *
 * `input` is copied into a private, owned buffer before the sentinel
 * mapping is applied in place, mirroring the Rust reference's
 * `input.to_vec()` and the TypeScript reference's `new Uint8Array(input)`.
 */
#include <stdlib.h>

static long long sentinel_checksum(const unsigned char *input, long long length) {
    unsigned char *transformed = (unsigned char *)malloc((size_t)length > 0 ? (size_t)length : 1);
    for (long long i = 0; i < length; i++) {
        unsigned char byte = input[i];
        if (byte == 0xff) {
            transformed[i] = 0x00;
        } else if (byte == 0x00) {
            transformed[i] = 0xff;
        } else {
            transformed[i] = 0x01;
        }
    }
    long long checksum = 0;
    for (long long i = 0; i < length; i++) {
        checksum += (i + 1) * (long long)transformed[i];
    }
    free(transformed);
    return checksum;
}
