/* A telemetry combiner register must saturate at the 32-bit signed boundary
 * rather than wrap or trap: two independent delta readings are summed into
 * one running total, and a reading that would carry the total past
 * `INT32_MAX`/`INT32_MIN` must clamp there instead of silently doing
 * whatever the underlying arithmetic happens to do at that magnitude.
 * Unchanged between the public and hidden phases; the hidden overlay
 * replaces only `main.c`, which #includes this file.
 *
 * The guard below is evaluated before any addition that could overflow, so
 * this never invokes C's signed-overflow undefined behavior.
 */
#include <stdint.h>

static int32_t combine_telemetry(int32_t delta_a, int32_t delta_b) {
    if (delta_b > 0 && delta_a > INT32_MAX - delta_b) {
        return INT32_MAX;
    } else if (delta_b < 0 && delta_a < INT32_MIN - delta_b) {
        return INT32_MIN;
    } else {
        return delta_a + delta_b;
    }
}
