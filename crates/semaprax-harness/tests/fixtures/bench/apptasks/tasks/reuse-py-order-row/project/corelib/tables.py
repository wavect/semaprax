def pad_right(text: str, width: int) -> str:
    return text + " " * max(0, width - len(text))


def render_row(cells, widths):
    return " | ".join(pad_right(str(c), w) for c, w in zip(cells, widths))
