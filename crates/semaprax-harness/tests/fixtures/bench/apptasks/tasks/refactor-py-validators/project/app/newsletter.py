def subscribe(email: str, subs: list) -> str:
    e = email.strip().lower()
    if "@" not in e or e.startswith("@") or e.endswith("@"):
        raise ValueError("invalid email")
    if e not in subs:
        subs.append(e)
    return e
