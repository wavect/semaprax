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

## Addendum: the gap is not the `std.data.json.dec` dependency itself

A later SPX-AI-025 session (this one) re-attempted the full CNORM-001..043
pipeline as a **fully self-contained project with zero `[dependencies]`**,
to test whether removing the package import (the prior session's prime
suspect) would clear `SPX-G171`. It does not, and the same session found a
second, independent ceiling underneath it. Both are recorded here so a
future attempt does not have to re-discover them from scratch.

**The probes below used a scratch project outside this repository, never
committed** (built from copies of this project's own ported
`std.data.json.doc`/`std.data.json.dec`-equivalent logic, re-namespaced
under `cn.*` instead of a package import). No source under
`examples/catalog-normalizer-project/` was changed by this session; the
directory is byte-identical to the prior session's commit.

**Finding 1 — `SPX-G171` fires on the project's own source alone.** A
hand-ported, dependency-free equivalent of `std.data.json.dec` (escape
decode, surrogate pairs, cursor comparisons — no package import, ~11 KB)
checks fine by itself. Adding a hand-ported, dependency-free structural
JSON scanner (`std.data.json.doc`-equivalent object/array/number/string
grammar, ~8 KB) and the record-level schema/type/duplicate/overflow checks
this issue's full scope calls for (~14 KB) — still zero dependencies —
reproducibly re-triggers `SPX-G171` once the combined project reaches
roughly 35-40 KB of source, well under `apex-supply-chain`'s own admitted
~10.5 KB multi-module business-logic example but also well past what a
`std.*` PACKAGE alone tolerates per the note above (~20-22 KB). Shortening
every `@id` stable identity in the project (the longest dropped from 73 to
34 characters) and removing every `ensures`/`requires` contract measurably
reduced, but did not eliminate, this ceiling.

**Finding 2 — a second, independent ceiling, `SPX-H006` ("cleanup replay
path bound exceeds the global path budget" / "cleanup replay program-wide
skeleton-work preflight exceeds the global budget"), fires per function and
per whole-program before `SPX-G171` does, on structurally ordinary code
with no owned resources at all:**

- A depth-bounded *recursive* JSON value/object/array grammar (mutually
  recursive `value_end` <-> `object_end`/`array_end`, admitted depth 8,
  the compiler's own runtime recursion bound is 256 frames) hits
  `SPX-H006` on the whole program from the recursive call CYCLE itself,
  independent of source size — replaced with an iterative, non-recursive
  port of `std.data.json.doc.document_end`'s own bit-packed mode/stack
  state machine (no recursion, no call cycle), which cleared this specific
  diagnostic.
- A single function combining roughly ten sequential `if`/`else`
  classification branches (a per-record status dispatcher) hits
  `SPX-H006` on itself alone; splitting it into ten small single-purpose
  functions chained by a trivial `next_stage(previous, next)` helper did
  not reliably clear the combined project's `SPX-G171`, and in one
  isolated case (`sequence_len`, a UTF-8 lead-byte-width classifier) even a
  *fully flattened, non-nested* boolean formula (four independent
  `&&`-chained range checks summed arithmetically, no cascading `if`)
  still hit `SPX-H006` on that function alone. A `match`-based rewrite of
  the same classifier (enumerating every lead-byte value as a discrete
  arm, the same style `std.data.json.doc.step_action` already uses and
  this project's own ported iterative scanner reuses successfully)
  produced a *different* diagnostic ("cleanup plan: wildcard match arm
  must be the final exhaustive arm", against a `match` whose wildcard arm
  was in fact last), which reads as the verifier's own synthesized
  representation of a moderately large `match` hitting an edge case rather
  than a source defect.
- With every one of the above mitigations applied, `record_status`'s own
  file combined with its four dependency modules (structural scanner,
  decode cursor, UTF-8 validator, shared limits) still exceeded `SPX-G171`
  — i.e., fixing the specific functions this session could identify by
  bisection did not, in the time available, add up to fixing the whole
  combination.

**Net conclusion.** This is not a dependency-import cost, a contract-usage
cost, or a single identifiable "too clever" function; it is a combination
of at least two independent internal capacity limits (`SPX-G171`'s
byte/identity/expression-slot pre-bound and `SPX-H006`'s per-function and
whole-program cleanup-replay path/work budgets) that, taken together,
appear to cap how much genuinely branch-heavy validation logic (as opposed
to the mostly straight-line arithmetic `apex-supply-chain` exercises) a
single admitted project can contain today, independent of package
dependencies. Closing GitHub issue #124's full scope needs either a
compiler-side change to one or both of these budgets for legitimate
non-trivial single-project logic, or a fundamentally different
implementation strategy this session did not find (bisection reproduction
commands are not preserved, since the scratch project was never
committed; the exact sequence of `semaprax check <dir> --json` probes
against incrementally larger `[modules] sources` lists, with and without
each mitigation above, is reproducible from this description alone).
