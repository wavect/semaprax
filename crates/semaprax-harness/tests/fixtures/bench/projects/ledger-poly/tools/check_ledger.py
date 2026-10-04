"""Release check: compares the Wasm export of ledger.line_total with the contract."""


def expected_line_total(price, qty):
    return price * qty


def check(export):
    return all(export(p, q) == expected_line_total(p, q) for p in range(0, 5) for q in range(0, 5))
