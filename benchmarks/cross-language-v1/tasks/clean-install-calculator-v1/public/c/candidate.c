/* The starter calculator module a fresh scaffold ships with one operation
 * (`add`); this port's baseline mirrors that same minimal two-function
 * calculator shape a fresh project would start from before any of its own
 * logic exists. Unchanged between the public and hidden phases; the hidden
 * overlay replaces only `main.c`, which #includes this file.
 */
static long long add(long long left, long long right) {
    return left + right;
}

static long long subtract(long long left, long long right) {
    return left - right;
}
