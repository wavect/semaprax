// Candidate: two half-open booking windows conflict exactly when they
// share at least one instant. Unchanged between the public and hidden
// phases; the hidden overlay replaces only `Main.java`.
public final class Candidate {
    static long conflicts(long aStart, long aEnd, long bStart, long bEnd) {
        return (aStart < bEnd && bStart < aEnd) ? 1 : 0;
    }
}
