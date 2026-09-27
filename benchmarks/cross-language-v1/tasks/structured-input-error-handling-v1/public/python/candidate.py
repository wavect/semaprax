"""Candidate: classify a bounded versioned record envelope, first-error
precedence. Unchanged between the public and hidden phases; the hidden
overlay replaces only `digest.py`, which imports this module (the script's
own directory is always on `sys.path`, so this import needs no package
installation), mirroring the Rust port's `mod candidate;` / TypeScript
port's `import { validate }` separation.
"""


def validate(kind: int, version: int, payload_len: int) -> int:
    if kind != 7:
        return 1
    if version != 1:
        return 2
    if payload_len < 1 or payload_len > 64:
        return 3
    return 0
