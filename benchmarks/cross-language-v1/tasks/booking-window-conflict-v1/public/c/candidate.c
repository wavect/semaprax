/* Candidate: two half-open booking windows conflict exactly when they share
 * at least one instant. Unchanged between the public and hidden phases; the
 * hidden overlay replaces only `main.c`, which #includes this file.
 */
static long long conflicts(long long a_start, long long a_end, long long b_start, long long b_end) {
    if (a_start < b_end && b_start < a_end) {
        return 1;
    }
    return 0;
}
