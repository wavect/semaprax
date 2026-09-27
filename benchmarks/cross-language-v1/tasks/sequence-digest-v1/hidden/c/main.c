/* Hidden overlay: replaces the public `main.c` verbatim (same relative path)
 * for the scoring phase only, adding two hidden vectors a solver never sees.
 * Implementation functions are unchanged from the public file.
 */
#include <stdio.h>

static long long is_even(long long value) { return (value % 2 == 0) ? 1 : 0; }
static long long is_negative(long long value) { return (value < 0) ? 1 : 0; }
static long long max2(long long left, long long right) { return (left > right) ? left : right; }

static long long sum_of(long long a, long long b, long long c, long long d, long long e) {
    return a + b + c + d + e;
}

static long long count_even(long long a, long long b, long long c, long long d, long long e) {
    return is_even(a) + is_even(b) + is_even(c) + is_even(d) + is_even(e);
}

static long long count_negative(long long a, long long b, long long c, long long d, long long e) {
    return is_negative(a) + is_negative(b) + is_negative(c) + is_negative(d) + is_negative(e);
}

static long long max_of(long long a, long long b, long long c, long long d, long long e) {
    return max2(max2(max2(a, b), max2(c, d)), e);
}

static int failures = 0;

static void check(long long actual, long long expected, const char *label) {
    if (actual != expected) {
        fprintf(stderr, "%s: expected %lld, got %lld\n", label, expected, actual);
        failures++;
    }
}

int main(void) {
    check(sum_of(1, 2, 3, 4, 5), 15, "sum(1,2,3,4,5)");
    check(count_even(1, 2, 3, 4, 5), 2, "count_even(1,2,3,4,5)");
    check(count_negative(1, 2, 3, 4, 5), 0, "count_negative(1,2,3,4,5)");
    check(max_of(1, 2, 3, 4, 5), 5, "max_of(1,2,3,4,5)");

    check(sum_of(-1, -2, -3, -4, -5), -15, "sum(-1,-2,-3,-4,-5)");
    check(count_even(-1, -2, -3, -4, -5), 2, "count_even(-1,-2,-3,-4,-5)");
    check(count_negative(-1, -2, -3, -4, -5), 5, "count_negative(-1,-2,-3,-4,-5)");
    check(max_of(-1, -2, -3, -4, -5), -1, "max_of(-1,-2,-3,-4,-5)");

    /* Hidden cases: never shipped in the public directory tree. */
    check(sum_of(0, 0, 0, 0, 0), 0, "sum(0,0,0,0,0)");
    check(count_even(0, 0, 0, 0, 0), 5, "count_even(0,0,0,0,0)");
    check(count_negative(0, 0, 0, 0, 0), 0, "count_negative(0,0,0,0,0)");
    check(max_of(0, 0, 0, 0, 0), 0, "max_of(0,0,0,0,0)");

    check(sum_of(-100, 7, 7, 7, 100), 21, "sum(-100,7,7,7,100)");
    check(count_even(-100, 7, 7, 7, 100), 2, "count_even(-100,7,7,7,100)");
    check(count_negative(-100, 7, 7, 7, 100), 1, "count_negative(-100,7,7,7,100)");
    check(max_of(-100, 7, 7, 7, 100), 100, "max_of(-100,7,7,7,100)");

    return failures == 0 ? 0 : 1;
}
