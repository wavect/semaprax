// prior-session: shipment tag helper; unrelated to this task's repair, keep
// unchanged. A candidate that clobbers or deletes this while repairing
// `applyDiscount` below has not preserved a stale, already-completed edit.
// Unchanged between the public and hidden phases; the hidden overlay
// replaces only `Main.java`.
public final class Candidate {
    static long staleNote(long tag) {
        return tag * 2 + 7;
    }

    static long applyDiscount(long price, long pct) {
        // A corrupted upstream feed can send `pct` above 100; the result
        // must floor at zero rather than go negative.
        long raw = price - (price * pct) / 100;
        return raw < 0 ? 0 : raw;
    }
}
