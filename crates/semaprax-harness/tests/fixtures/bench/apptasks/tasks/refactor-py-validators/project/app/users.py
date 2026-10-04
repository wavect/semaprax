def register(email: str, users: dict) -> str:
    e = email.strip().lower()
    if "@" not in e or e.startswith("@") or e.endswith("@"):
        raise ValueError("invalid email")
    if e in users:
        raise ValueError("duplicate")
    users[e] = {"email": e}
    return e
