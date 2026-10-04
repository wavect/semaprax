def normalize_email(raw: str) -> str:
    e = raw.strip().lower()
    if "@" not in e or e.startswith("@") or e.endswith("@"):
        raise ValueError("invalid email")
    return e
