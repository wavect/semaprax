// Rust host glue for the compiled ledger package.
pub fn host_line_total(price: i64, qty: i64) -> i64 {
    // ledger.line_total through the native export
    price * qty
}

pub fn host_invoice_total(price: i64, qty: i64, fee: i64) -> i64 {
    // ledger.invoice_total through the native export
    host_line_total(price, qty) + fee
}
