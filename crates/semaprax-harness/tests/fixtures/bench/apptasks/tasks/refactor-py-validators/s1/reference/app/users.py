from .validators import normalize_email


def register(email: str, users: dict) -> str:
    e = normalize_email(email)
    if e in users:
        raise ValueError("duplicate")
    users[e] = {"email": e}
    return e
