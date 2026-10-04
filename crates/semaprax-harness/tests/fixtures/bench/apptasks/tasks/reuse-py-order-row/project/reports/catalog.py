from corelib.text import slugify, truncate


def catalog_key(title: str) -> str:
    return truncate(slugify(title), 24)
