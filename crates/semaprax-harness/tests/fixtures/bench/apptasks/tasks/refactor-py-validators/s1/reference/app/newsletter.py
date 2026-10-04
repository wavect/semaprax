from .validators import normalize_email


def subscribe(email: str, subs: list) -> str:
    e = normalize_email(email)
    if e not in subs:
        subs.append(e)
    return e
