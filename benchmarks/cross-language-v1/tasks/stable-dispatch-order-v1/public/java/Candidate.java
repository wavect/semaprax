// Candidate: order three jobs by increasing priority while preserving
// arrival order for ties. Unchanged between the public and hidden phases;
// the hidden overlay replaces only `Main.java`.
public final class Candidate {
    static long dispatchOrder(long aPriority, long bPriority, long cPriority) {
        if (aPriority <= bPriority && aPriority <= cPriority) {
            return bPriority <= cPriority ? 123 : 132;
        }
        if (bPriority <= aPriority && bPriority <= cPriority) {
            return aPriority <= cPriority ? 213 : 231;
        }
        return aPriority <= bPriority ? 312 : 321;
    }
}
