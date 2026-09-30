/* Public implementation: four independent scalar digests over five signed
 * 64-bit inputs. See ../../EQUIVALENCE.md for the exact input/output/boundary
 * contract every language implementation of this task must meet.
 *
 * C has no built-in assertion/test convention this suite can rely on without
 * a third-party framework (and this sandbox performs no package installs), so
 * `main` itself is the official test runner: it counts failures and reports
 * them through its own exit code, exactly the same convention the other
 * languages' officially documented invocations already reduce to.
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

    return failures == 0 ? 0 : 1;
}
