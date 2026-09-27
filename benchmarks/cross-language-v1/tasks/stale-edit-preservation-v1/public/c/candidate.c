/* prior-session: shipment tag helper; unrelated to this task's repair, keep
 * unchanged. A candidate that clobbers or deletes this while repairing
 * `apply_discount` below has not preserved a stale, already-completed edit.
 * Unchanged between the public and hidden phases; the hidden overlay
 * replaces only `main.c`, which #includes this file.
 */
static long long stale_note(long long tag) {
    return tag * 2 + 7;
}

static long long apply_discount(long long price, long long pct) {
    /* A corrupted upstream feed can send `pct` above 100; the result must
     * floor at zero rather than go negative. */
    long long raw = price - (price * pct) / 100;
    if (raw < 0) {
        return 0;
    }
    return raw;
}
