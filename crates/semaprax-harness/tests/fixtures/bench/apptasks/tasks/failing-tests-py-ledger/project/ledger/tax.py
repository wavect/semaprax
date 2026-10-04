def tax_cents(net_cents: int, rate_percent: int) -> int:
    """Tax rounded half up to whole cents."""
    return net_cents * rate_percent // 100
