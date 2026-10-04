def chunk(xs, n):
    return [xs[i : i + n] for i in range(0, len(xs), n)]


def group_by(xs, key):
    out = {}
    for x in xs:
        out.setdefault(key(x), []).append(x)
    return out
