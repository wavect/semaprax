// Candidate: map sentinels in an owned byte buffer and compute the
// one-based positional checksum of the transformed bytes. Unchanged
// between the public and hidden phases; the hidden overlay replaces only
// `Main.java`.
//
// Java's `byte` is signed, so an unsigned 0..255 byte value is represented
// here as `int` (each element constrained to that range by the caller),
// avoiding sign-extension pitfalls a literal `byte[]` would introduce for
// values at or above 0x80; this is the same buffer contract as the other
// ports, expressed in Java's own idiom rather than transliterated.
public final class Candidate {
    static long sentinelChecksum(int[] input) {
        int[] transformed = input.clone();
        for (int i = 0; i < transformed.length; i++) {
            int b = transformed[i];
            if (b == 0xff) {
                transformed[i] = 0x00;
            } else if (b == 0x00) {
                transformed[i] = 0xff;
            } else {
                transformed[i] = 0x01;
            }
        }
        long checksum = 0;
        for (int i = 0; i < transformed.length; i++) {
            checksum += (long) (i + 1) * transformed[i];
        }
        return checksum;
    }
}
