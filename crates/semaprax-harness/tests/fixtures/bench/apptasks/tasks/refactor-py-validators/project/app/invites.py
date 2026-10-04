def invite(email: str, invited: set) -> str:
    e = email.strip().lower()
    if "@" not in e or e.startswith("@") or e.endswith("@"):
        raise ValueError("invalid email")
    invited.add(e)
    return e
