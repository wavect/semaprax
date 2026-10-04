from datetime import date


def parse_iso(text: str) -> date:
    """Parse YYYY-MM-DD into a date; raises ValueError on bad input."""
    y, m, d = text.split("-")
    return date(int(y), int(m), int(d))


def days_between(a: date, b: date) -> int:
    return (b - a).days


def short_date(d: date) -> str:
    """Render as DD Mon YYYY, e.g. 05 Mar 2024."""
    months = "Jan Feb Mar Apr May Jun Jul Aug Sep Oct Nov Dec".split()
    return f"{d.day:02d} {months[d.month - 1]} {d.year}"
