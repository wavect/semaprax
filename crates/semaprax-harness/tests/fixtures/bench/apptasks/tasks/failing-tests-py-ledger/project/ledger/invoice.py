from .money import to_cents, fmt
from .tax import tax_cents


def invoice_total(lines, rate_percent):
    """lines: list of (decimal string, qty). Returns (net, tax, gross) in cents."""
    net = sum(to_cents(p) * q for p, q in lines)
    tax = tax_cents(net, rate_percent)
    return net, tax, net + tax


def invoice_text(lines, rate_percent):
    net, tax, gross = invoice_total(lines, rate_percent)
    return f"net {fmt(net)} tax {fmt(tax)} total {fmt(gross)}"
