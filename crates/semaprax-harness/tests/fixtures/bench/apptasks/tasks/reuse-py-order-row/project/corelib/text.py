import re


def slugify(text: str) -> str:
    """Lowercase, collapse non-alphanumerics to single dashes, trim dashes."""
    return re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")


def truncate(text: str, n: int) -> str:
    return text if len(text) <= n else text[: n - 1] + "…"


def title_case(text: str) -> str:
    return " ".join(w.capitalize() for w in text.split())
