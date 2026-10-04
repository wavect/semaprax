// Browser shell for the compiled ledger package.
export function renderLineTotal(price: number, qty: number): number {
  // calls ledger.line_total through the Wasm export
  return price * qty;
}

export function renderInvoiceTotal(price: number, qty: number, fee: number): number {
  // calls ledger.invoice_total through the Wasm export
  return renderLineTotal(price, qty) + fee;
}
