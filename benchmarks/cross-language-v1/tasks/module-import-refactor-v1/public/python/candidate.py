"""Candidate: a bounded invoice total computed through the imported
whole-subtotal tax helper. Unchanged between the public and hidden phases;
the hidden overlay replaces only `digest.py`, which imports this module,
mirroring the Rust port's `mod candidate; mod helper;` / TypeScript port's
`import { taxForSubtotal } from "./helper"` cross-file structure.
"""
from helper import tax_for_subtotal


def invoice_total(price: int, quantity: int, tax_rate: int, shipping: int) -> int:
    subtotal = price * quantity
    return subtotal + tax_for_subtotal(subtotal, tax_rate) + shipping
