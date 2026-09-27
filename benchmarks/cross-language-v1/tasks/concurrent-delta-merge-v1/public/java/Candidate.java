// Candidate: merge two independently-arriving deltas against a shared
// [0, 1_000_000]-bounded counter from one common base value, treating both
// deltas as arriving concurrently. Unchanged between the public and hidden
// phases; the hidden overlay replaces only `Main.java`.
public final class Candidate {
    static long mergeConcurrentDeltas(long base, long deltaA, long deltaB) {
        long total = base + deltaA + deltaB;
        if (total < 0) {
            return 0;
        } else if (total > 1_000_000) {
            return 1_000_000;
        } else {
            return total;
        }
    }
}
