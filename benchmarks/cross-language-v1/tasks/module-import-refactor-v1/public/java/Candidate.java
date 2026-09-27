// Candidate: a bounded invoice total computed through the imported
// whole-subtotal tax helper. Unchanged between the public and hidden
// phases; the hidden overlay replaces only `Main.java`, mirroring the Rust
// port's `mod candidate; mod helper;` / TypeScript port's
// `import { taxForSubtotal } from "./helper"` cross-file structure.
public final class Candidate {
    static long invoiceTotal(long price, long quantity, long taxRate, long shipping) {
        long subtotal = price * quantity;
        return subtotal + Helper.taxForSubtotal(subtotal, taxRate) + shipping;
    }
}
