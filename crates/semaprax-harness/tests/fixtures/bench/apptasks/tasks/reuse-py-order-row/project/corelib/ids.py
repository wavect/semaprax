import hashlib


def short_id(text: str, n: int = 8) -> str:
    return hashlib.sha1(text.encode()).hexdigest()[:n]
