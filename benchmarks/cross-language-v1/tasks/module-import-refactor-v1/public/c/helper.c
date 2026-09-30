/* Imported whole-subtotal tax helper. Unchanged between the public and
 * hidden phases; #included (transitively, via candidate.c) rather than
 * separately compiled, since this suite's C build compiles only `main.c`.
 */
static long long tax_for_subtotal(long long subtotal, long long tax_rate) {
    return (subtotal * tax_rate) / 100;
}
