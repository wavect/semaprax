# catalog-normalizer (CNORM-012 slice)

Issue: SPX-AI-025 / GitHub #124, against the frozen oracle from SPX-AI-018 /
GitHub #117 (`docs/CATALOG-NORMALIZER-ORACLE-V1.md`,
`tests/oracle/catalog_normalizer/`).

## What this project implements

One JSON string token (`borrow Slice<u8>` starting at its opening `"`),
composed from `std.data.json.dec` (decode) and a locally ported
`std.bytes`-equivalent ASCII trim:

- **CNORM-010** (decode validity): `token_valid` rejects a bad escape, a
  lone surrogate, or an unterminated string, via
  `std.data.json.dec.is_failure`/`decoded_len`.
- **CNORM-011** (length bound): `token_valid` also rejects a decoded value
  over `max_label_bytes()` (256).
- **CNORM-012** (the trim): `trimmed_bytes`/`trimmed_matches` strip only the
  four ASCII whitespace bytes (space, tab, CR, LF) from both boundaries of
  the *decoded* value, leaving interior whitespace, non-ASCII bytes, and
  everything else untouched.

Every non-rejection test case in `src/tests.spx` reproduces one label value
from `tests/oracle/catalog_normalizer/cases/published/basics.json` byte for
byte (`basic-single-record`, `label-trim-boundary-whitespace`,
`label-internal-whitespace-preserved`,
`label-multibyte-unicode-no-normalization`): both the raw JSON token and the
expected decoded/trimmed bytes are copied from that frozen case's own
`input`/`expected_output`, not invented.

## What this project does NOT implement

- **CNORM-001..005** (whole-body/record-count/line-length limits): no batch
  input at all is parsed; this project operates on one already-isolated
  string token.
- **CNORM-006..009** (structural JSON-object shape, exact 3-key schema,
  value type/range checks): no `{...}` record is parsed; no key lookup.
- **CNORM-013..016** (record order, duplicate-id, checked-overflow running
  total, all-or-nothing across a multi-record batch).
- **CNORM-040..043** (the canonical success/error JSON envelope).

## Why: a measured, real toolchain constraint

Every one of the layers above was implemented in an earlier iteration of
this same project (object/schema scanning, duplicate-id detection via
`std.data.json.dec.decoded_eq`, a checked running total, and full canonical
JSON output rendering with escape-table encoding) and worked in isolation,
but the *combination* of any of them together with `std.data.json.dec`
exceeds `SPX-G171` (the Workspace Semantic Graph builder's byte
pre-bound, `18874368` bytes) — a real, reproducible ceiling, not a bug in
this project's own code:

- A trivial project depending on nothing but `std.data.json.doc` +
  `std.data.json.dec` together (one function each, `is_document` and
  `decoded_size`) already exceeds the pre-bound.
- `std.data.json.dec` alone tolerates a moderate amount of first-party code
  (this project's own ~170-line `src/app.spx`), but not the schema/render
  layers this issue's full scope calls for.
- A second, unrelated interpreter capacity ceiling
  (`MAX_BYTES_COPY_SITES` / `SPX-F105`, "owned byte allocation count
  exceeds verified capacity") was also hit and is the reason
  `src/tests.spx` carries 5 named cases rather than 6: a 6th case (the
  astral-plane surrogate-pair label, also a genuine oracle case) pushed the
  program's total reachable `bytes_copy` call count over that ceiling.

`docs/CATALOG-NORMALIZER-ORACLE-V1.md` itself names this class of outcome
("the same way `std.data.json.doc`'s own design document records what it
had to cut for its package budget") and instructs filing it as a documented
gap rather than weakening the frozen contract. See the implementing
session's final report for the exact probe commands and their output.
