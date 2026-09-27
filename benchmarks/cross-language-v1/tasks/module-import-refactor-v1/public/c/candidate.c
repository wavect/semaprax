/* Candidate: a bounded invoice total computed through the imported
 * whole-subtotal tax helper. Unchanged between the public and hidden
 * phases; the hidden overlay replaces only `main.c`, which #includes this
 * file (this file itself #includes the unchanged `helper.c`), mirroring
 * the Rust port's `mod candidate; mod helper;` / TypeScript port's
 * `import { taxForSubtotal } from "./helper"` cross-file structure as
 * closely as this suite's single-entry-file C build allows.
 */
#include "helper.c"

static long long invoice_total(long long price, long long quantity, long long tax_rate, long long shipping) {
    long long subtotal = price * quantity;
    return subtotal + tax_for_subtotal(subtotal, tax_rate) + shipping;
}
