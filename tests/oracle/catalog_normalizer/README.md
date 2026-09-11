# catalog-normalizer oracle — FROZEN, protected location

This directory is the independent acceptance oracle for the
**catalog-normalizer** application defined in
[`docs/CATALOG-NORMALIZER-ORACLE-V1.md`](../../../docs/CATALOG-NORMALIZER-ORACLE-V1.md)
(GitHub issue #117, SPX-AI-018). Read that document first; it is the
normative prose, this directory is its executable and data form.

## Protected location — read this before touching anything here

**Everything under this directory is frozen.** It was authored by the
oracle-author worker for issue #117, specifically so that a later,
independent implementation worker (issue #124, SPX-AI-025, and the
composition work in issues #118–#123 it depends on) has one fixed target it
cannot quietly bend to fit its own implementation.

- **Implementation agents must not edit anything under this directory** —
  not `oracle.py`, not `fixtures/`, not any file under `cases/`. If your
  implementation fails against this oracle, that is evidence about your
  implementation, not authorization to change the oracle. File a genuine
  language-gap finding against SPX-AI-019..025 instead (see the "Change
  procedure" section of the spec) rather than editing this directory.
- **`cases/published/**` is for you to read and build against.** Use it the
  way any known-answer test corpus is used during development.
- **`cases/hidden/**` is off limits during implementation.** Do not open,
  read, `cat`, grep the contents of, or copy any value out of
  `cases/hidden/**` while implementing or testing the catalog-normalizer
  application. This is a policy boundary enforced by review, the same way
  this repository already keeps other held-back solutions out of
  candidate-visible files (see `docs/AGENT-QUICK-REFERENCE.md`'s own
  convention, and issue #124's own instruction to "keep their solutions
  outside candidate-visible files" for its later agent-trial exercises).
  Nothing here stops you at the filesystem level from reading it; consulting
  it anyway defeats the entire point of having it and is a contract
  violation regardless of whether your implementation's tests stay green.
  The Rust harness (`tests/useful_data/catalog_normalizer_oracle.rs`) is the
  only thing that is expected to exercise `cases/hidden/**` mechanically,
  and it does so only to prove the oracle itself is self-consistent — it
  never feeds hidden inputs to a candidate implementation.

## Layout

```
oracle.py                        the independent Python reference oracle
README.md                        this file
fixtures/enrichment.json         frozen deterministic enrichment fixture table
cases/published/*.json           known-answer cases implementers may read
cases/hidden/*.json              known-answer cases implementers may NOT read
cases/negative_controls.json     pairs a BUGGY_MODE with the case that exposes it
```

Each `cases/{published,hidden}/*.json` file is one manifest:
`{"schema": "...", "cases": [{"name", "input" | "input_hex", "enrich",
"expected_output"}, ...]}`. `input`/`expected_output` are UTF-8 text (JSON
strings can hold the request's real embedded `\n` line breaks directly);
the one case whose whole point is invalid UTF-8 bytes uses `input_hex`
instead, since invalid UTF-8 cannot be represented as a JSON string.

## Running the oracle

```sh
# One request, over the documented CLI (stdin -> stdout):
python3 oracle.py < request.jsonl
python3 oracle.py --enrich --fixture fixtures/enrichment.json < request.jsonl

# The full frozen corpus plus every negative control, in one shot:
python3 oracle.py --self-test
```

`--self-test` is what `tests/useful_data/catalog_normalizer_oracle.rs`
invokes. It checks every `published/` and `hidden/` case against the oracle
byte for byte, then runs every entry in `cases/negative_controls.json`:
each names one of `oracle.py`'s built-in `--buggy MODE` implementations (a
real, runnable implementation of one of the five wrong-implementation
patterns GitHub issue #117 explicitly lists, plus a sixth
"hard-codes-the-visible-example" pattern) and asserts that mode's output
differs from the correct oracle's output on its designated case. This is
the negative-control evidence the worker contract requires: proof that a
deliberately wrong candidate is rejected by the byte-exact acceptance
comparison, not merely that the oracle runs.

## Regenerating the frozen corpus after a deliberate, reviewed spec change

Do not hand-edit `expected_output` fields. Load `oracle.py` as a module,
call `oracle.normalize(body, enrich, fixture, None)` for each case's input,
and write the result back as that case's `expected_output`. Then rerun
`python3 oracle.py --self-test` and confirm it reports `OK` with the same or
a larger case count than before, and update the requirement ids in
`docs/CATALOG-NORMALIZER-ORACLE-V1.md` that the change touched. A change
that is only a bug fix to the oracle's own implementation of an unchanged
rule follows the same regeneration step but touches no `CNORM-NNN` id.
