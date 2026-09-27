// A telemetry combiner register must saturate at the 32-bit signed
// boundary rather than wrap or trap. Java's `int` is already exactly
// 32-bit, but ordinary `+` silently wraps on overflow rather than
// signaling it, so the guard below (not `Math.addExact`, which throws
// instead of saturating) is the required implementation. Unchanged between
// the public and hidden phases; the hidden overlay replaces only
// `Main.java`.
public final class Candidate {
    static int combineTelemetry(int deltaA, int deltaB) {
        if (deltaB > 0 && deltaA > Integer.MAX_VALUE - deltaB) {
            return Integer.MAX_VALUE;
        } else if (deltaB < 0 && deltaA < Integer.MIN_VALUE - deltaB) {
            return Integer.MIN_VALUE;
        } else {
            return deltaA + deltaB;
        }
    }
}
