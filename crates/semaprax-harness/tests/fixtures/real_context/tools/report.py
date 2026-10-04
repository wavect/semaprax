"""Nightly report helper for the ledger web shell."""


def parse_amount(text):
    whole, _, frac = text.partition(".")
    return int(whole) * 100 + int((frac + "00")[:2])


def build_report(lines):
    total = 0
    for line in lines:
        total += parse_amount(line)
    return summarize(total)


def summarize(total):
    return f"total={total}"
