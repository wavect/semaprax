/* Candidate: merge two independently-arriving deltas against a shared
 * [0, 1_000_000]-bounded counter from one common base value, treating both
 * deltas as arriving concurrently (summed against the same base, then
 * clamped once) rather than sequentially. Unchanged between the public and
 * hidden phases; the hidden overlay replaces only `main.c`, which
 * #includes this file.
 */
static long long merge_concurrent_deltas(long long base, long long delta_a, long long delta_b) {
    long long total = base + delta_a + delta_b;
    if (total < 0) {
        return 0;
    } else if (total > 1000000) {
        return 1000000;
    } else {
        return total;
    }
}
