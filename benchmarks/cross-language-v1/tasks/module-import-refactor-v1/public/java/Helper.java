// Imported whole-subtotal tax helper. Unchanged between the public and
// hidden phases; `javac Main.java` auto-discovers and compiles this file
// (and Candidate.java, which references it) from the same directory.
public final class Helper {
    static long taxForSubtotal(long subtotal, long taxRate) {
        return (subtotal * taxRate) / 100;
    }
}
