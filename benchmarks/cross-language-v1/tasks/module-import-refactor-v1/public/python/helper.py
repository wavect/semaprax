"""Imported whole-subtotal tax helper. Unchanged between the public and
hidden phases.
"""


def tax_for_subtotal(subtotal: int, tax_rate: int) -> int:
    return (subtotal * tax_rate) // 100
