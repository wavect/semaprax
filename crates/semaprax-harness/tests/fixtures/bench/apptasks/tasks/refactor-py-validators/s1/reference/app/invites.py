from .validators import normalize_email


def invite(email: str, invited: set) -> str:
    e = normalize_email(email)
    invited.add(e)
    return e
