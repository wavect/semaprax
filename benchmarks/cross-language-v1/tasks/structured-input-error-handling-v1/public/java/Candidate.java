// Candidate: classify a bounded versioned record envelope, first-error
// precedence. Unchanged between the public and hidden phases; `javac
// Main.java` auto-discovers and compiles this file from the same directory
// (its default sourcepath), so the hidden overlay can replace only
// `Main.java`, mirroring the Rust port's `mod candidate;` / TypeScript
// port's `import { validate }` separation.
public final class Candidate {
    static long validate(long kind, long version, long payloadLen) {
        if (kind != 7) {
            return 1;
        }
        if (version != 1) {
            return 2;
        }
        if (payloadLen < 1 || payloadLen > 64) {
            return 3;
        }
        return 0;
    }
}
