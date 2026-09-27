/* Candidate: order three jobs by increasing priority while preserving
 * arrival order for ties. Unchanged between the public and hidden phases;
 * the hidden overlay replaces only `main.c`, which #includes this file.
 */
static long long dispatch_order(long long a_priority, long long b_priority, long long c_priority) {
    if (a_priority <= b_priority && a_priority <= c_priority) {
        if (b_priority <= c_priority) {
            return 123;
        } else {
            return 132;
        }
    } else if (b_priority <= a_priority && b_priority <= c_priority) {
        if (a_priority <= c_priority) {
            return 213;
        } else {
            return 231;
        }
    } else if (a_priority <= b_priority) {
        return 312;
    } else {
        return 321;
    }
}
