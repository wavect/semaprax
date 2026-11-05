# Complete formatting preservation in historical source controls

Status: Local repair in progress; hosted CI remains authoritative.
Audience: Compiler maintainers and CI reviewers.

Ten historical source assertions compared newly formatted retained control bodies
with their complete earlier spellings. Each assertion still compares the entire
original body. Its named inverse now first authenticates every current byte and
an independent current archive, applies the declared exact edits, and compares
every reconstructed byte with the independent original archive. Empty, truncated,
duplicated, same-length changed and foreign inputs are refused.

The executable verifier runs the same formatter over both complete versions.
It requires identical output, including string literal contents, in addition to
the full byte, digest, length and inverse checks. These ten records contain
formatting changes only. They do not alter a compiler algorithm, allocation
invoice, source ownership boundary, test fixture, budget or refusal control.

Run with a Rust formatter on PATH:

```sh
python3 docs/evidence/ci-formatting-preservation-20261105/verify.py
```
