# Catalog-normalizer acceptance application and oracle v1

Status: **frozen requirements and independent oracle**. The oracle is
implemented and passing its own corpus; the application itself is **not**
implemented in Semaprax by this document or by anything under
`tests/oracle/catalog_normalizer/`. Implementing it in Semaprax is a
separate, later issue: **SPX-AI-025 / GitHub issue #124**. This document and
the oracle exist to let that implementation, and the composition work in
SPX-AI-020..024 (issues #119, #121, #122, #123) that precedes it, target one
fixed, testable contract instead of an evolving one.

Audience: the SPX-AI-025 implementing agent, reviewers of that
implementation, and anyone extending this corpus later.

## Why this exists, and what "done" means for this issue

SPX-AI-018 (GitHub issue #117) asks for one finite application that "drives
language composition" — real JSON parsing, real text handling, real bounded
I/O and an optional provider effect — frozen *before* any compiler or
standard-library change is made to pass it, plus an oracle independent
enough that a Semaprax implementation cannot pass by accident or by copying
the oracle's own logic. This issue is scoped to exactly that: the contract
and the oracle. It is explicitly **not** the Semaprax implementation
(non-goal, stated in the issue body), and it is explicitly not an expansion
of the standard library's public support surface — it is an acceptance
fixture.

"Done" for this document means: every rule below is exact enough that two
independent readers (a human and the Python oracle in this same directory)
produce identical output for identical input, with no case left to
implementer judgement. Where a rule reuses an existing documented Semaprax
primitive, this document says so explicitly and cites it, so the future
implementation is built *of* existing packages rather than a rediscovery of
them.

## What this application is

**catalog-normalizer** ingests a bounded batch of JSON Lines records
describing catalog entries, validates and normalizes them, optionally
enriches each one through a bounded fixture provider, and emits one
deterministic structured JSON response: either a success envelope carrying
every normalized record, or an error envelope carrying one exact failure
category and position. There is no partial success: a batch is accepted in
full or rejected in full.

This exercises, deliberately:

- the **structural JSON document family** (`std.data.json.doc`) for the
  object/member grammar, depth bound, and duplicate/unknown-key policy —
  [Bounded JSON Scanner v1](BOUNDED-JSON-SCANNER-V1.md);
- the **number/literal token family** (`std.data.json.token`) for exact
  `i64` decoding that never rounds through `f64` — same reference;
- the **decoded-string family** (`std.data.json.dec`), including the
  cursor-adapter **`decoded_token_eq`** for comparing two decoded string
  *values* for equality without an owned map — [JSON Cursor Adapters v1](JSON-CURSORS-V1.md);
- the **writer family** (`std.data.json.write`, `std.data.json.digits`) for
  the canonical output encoding;
- bounded **Reader/Writer** I/O and a **fixture provider** seam, the subject
  of SPX-AI-024 (issue #123).

Per the coordinator's routing note for this issue, this specification
**reuses** those existing packages (the shared "JSON-APPLICATION" surface
also used by issue #63's `std.data.json.*` family, issue #123, and issue
#124) and does **not** require a general owned JSON tree, a growable
collection, or a hash map. Every rule below is written to be buildable from
exactly the primitives SEMAPRAX admits today: `Bytes` (uniquely owned,
immutable), `[u8; N]` (fixed, Copy), and `Slice<u8>` / `str` borrowed,
non-escaping views ([Portable Indexed Byte Data v1](PORTABLE-INDEXED-BYTE-DATA-V1.md)).
A requirement below that the language cannot plausibly express is a defect
in this document, not a license for the implementer to weaken it — file it
as a gap against SPX-AI-019..025 instead (issue step 7 of #117), the same
way the existing `std.data.json.doc` design document records what it had to
cut for its package budget.

## Change procedure (read this before touching anything under this heading)

This document and everything under `tests/oracle/catalog_normalizer/` are
**frozen**. Every requirement below carries a stable `CNORM-NNN` id.

- An **implementation agent** (SPX-AI-019 through SPX-AI-025, issues
  #118–#124) may **read** this document and the `published/` corpus freely,
  and **must not** edit this file or anything under
  `tests/oracle/catalog_normalizer/`. **Do not read, open, or copy
  `tests/oracle/catalog_normalizer/cases/hidden/**` into implementation or
  test sources.** That directory exists to catch an implementation that only
  matches the visible published examples; consulting it while implementing
  defeats its purpose even though nothing enforces that boundary at the
  filesystem level. Treat it the same way the repository already treats
  other held-back solutions (compare `docs/AGENT-QUICK-REFERENCE.md`'s
  existing convention for hidden diagnostic fixtures): a policy boundary
  backed by review, not a secret.
- An implementation that fails against this oracle does **not** authorize
  changing this document or the oracle to make that implementation pass.
  Genuinely new evidence that a rule is unbuildable in admitted SEMAPRAX (not
  merely inconvenient) goes to SPX-AI-019..025 as a documented gap first;
  changing the frozen contract to route around a real language gap is
  exactly the outcome this freeze exists to prevent.
- A deliberate, reviewed change to this contract (not a bug fix to the
  oracle's own implementation of an already-frozen rule) bumps this
  document's version number, updates every `CNORM-NNN` requirement it
  touches, regenerates the corpus's `expected_output` fields by re-running
  the oracle (see `tests/oracle/catalog_normalizer/README.md`), and states
  in the commit message exactly which requirement ids changed and why.
- A genuine bug in the oracle's implementation *of* an unchanged rule (the
  oracle's Python contradicts this document) is fixed in the oracle, not in
  this document, with the corpus regenerated the same way.

## Input grammar

**CNORM-001.** The request body is zero or more records, one per line,
delimited by a single `0x0A` (`\n`) byte. A body ending in exactly one
trailing `\n` is equivalent to the same body without it (the trailing
newline is optional and does not create an extra, empty record). Any other
line that is zero bytes long (for example two consecutive `\n` bytes, or a
`\n` that is not the single trailing one) is a malformed record: see
CNORM-021. `\r` is not a line terminator; a stray `0x0D` byte is ordinary
line content and is almost always rejected downstream by the JSON grammar or
the string-content rule in CNORM-011.

**CNORM-002.** The whole request body must be at most **65536 bytes**
(`MAX_TOTAL_INPUT_BYTES`). A larger body is rejected before any line is
read; see CNORM-020.

**CNORM-003.** A request body of at most **256 records** (`MAX_RECORDS`) is
admitted. A 257th record is rejected without reading its content; see
CNORM-022. Zero records (an empty body, or a body that is exactly one `\n`)
is valid and produces the empty success envelope (CNORM-041).

**CNORM-004.** Each record line, not counting its `\n` terminator, must be
at most **512 bytes** (`MAX_LINE_BYTES`); see CNORM-023.

**CNORM-005.** Each record line must be valid UTF-8 as a whole (not only
inside string literals); see CNORM-024. This mirrors the documented
composition `is_document(view) && is_utf8(view)` from
[Bounded JSON Scanner v1 § Structural documents](BOUNDED-JSON-SCANNER-V1.md#structural-documents):
UTF-8 validity is checked over the whole line as one byte range, the same
way `std.data.json.utf8.is_utf8` is documented to compose with
`std.data.json.doc.is_document`.

**CNORM-006.** Each valid-UTF-8 line must be exactly one JSON **object**
(RFC 8259 grammar), optionally preceded and followed only by JSON
whitespace (`0x20`, `0x09`, `0x0D`; `0x0A` cannot occur mid-line by
construction). A line that is syntactically valid JSON but is not an object
at the top level (a bare string, number, array, boolean, or `null`) is
`malformed_json`, not a schema mismatch: this is this application's own line
grammar, narrower than the general `is_document` grammar
`std.data.json.doc` admits. Any other structural defect — unterminated
string, bad escape, invalid number grammar, unbalanced braces, a control
byte `0x00`-`0x1F` occurring raw (unescaped) inside a string, trailing bytes
after the object other than permitted trailing whitespace — is
`malformed_json`, at the exact byte offset given in CNORM-030.

**CNORM-007.** Nested JSON containers (an object or array value nested
inside a member's value) are scanned to a bounded depth of **8**
(`MAX_NESTED_DEPTH`); a container opened past that depth is `malformed_json`
at the offset of the opening `{`/`[`. A conforming record never needs this
depth: `id`, `label`, and `quantity` are always flat scalar values (CNORM-008),
so this bound only fires on a malformed or adversarial record's misplaced
value type.

**CNORM-008.** A record object's member set must be **exactly** the three
keys `"id"`, `"label"`, `"quantity"`, spelled literally (no `\uXXXX` or other
escape in the key), each present exactly once, in any order. This is the
"unknown or duplicate object keys are rejected rather than silently
overwritten" rule from the issue body: every deviation — a missing required
key, an extra key, a required key repeated, or a required key spelled with
an escape — is a `schema` rejection (CNORM-025..028), never a silent
overwrite. Key comparison is a **raw byte-identical span** comparison
(quotes included, escapes not expanded), the same rule
`std.data.json.doc.is_unique` documents for its own duplicate-key check.

**CNORM-009.** `id`'s value must be a JSON string; `label`'s value must be a
JSON string; `quantity`'s value must be a JSON number token that is a bare
RFC 8259 integer (no `.`, `e`, or `E`) and whose exact value satisfies
`0 <= quantity <= 9223372036854775807` (`i64::MAX`). Any deviation —
wrong JSON type, a fractional or exponent number, a negative value, or a
magnitude outside `i64` range — is `schema` (CNORM-025, CNORM-027,
CNORM-028). The magnitude check is exact on the decimal digits of the
token; it is never computed via a rounding pass through `f64` (the same
exactness `std.data.json.token.i64_or` is documented to guarantee, and the
same reason RFC 8259's own float default is refused here rather than
adopted).

**CNORM-010.** `id`'s and `label`'s string values are decoded exactly as
`std.data.json.dec` documents its own decoding: the eight simple escapes,
strict `\uXXXX` and surrogate-pair handling, raw bytes passed through
unchanged, and every rejection (lone surrogate, unknown escape, raw control
byte, unterminated string) reported as `malformed_json` at the offset of the
backslash or offending byte that caused it (mirrors `escape_end` /
`string_end`'s reporting rule; see [Bounded JSON Scanner v1 § Decoding](BOUNDED-JSON-SCANNER-V1.md#decoding)).

**CNORM-011.** `id`'s decoded value must be **1 to 64 bytes** UTF-8
(`MIN_ID_BYTES`..`MAX_ID_BYTES`), measured as **encoded bytes, not Unicode
scalar count**. A decoded value of zero bytes is `schema` (CNORM-025); one
exceeding 64 bytes is `oversized_input` (CNORM-023). `label`'s decoded value
(before trimming — see CNORM-012) must be **at most 256 bytes**
(`MAX_LABEL_BYTES`), likewise measured in encoded bytes; an empty label is
valid. Both ceilings are deliberately measured in bytes, not characters,
because "counting Unicode characters as bytes" is one of the five explicitly
named wrong-implementation patterns this oracle's negative controls check
for (see below).

**CNORM-012 (normalization).** `label`'s normalized form is its decoded
value (CNORM-010) with only the four ASCII whitespace bytes — `0x20`
(space), `0x09` (tab), `0x0A` (line feed), `0x0D` (carriage return) —
stripped from **both boundaries**, and nothing else: no interior whitespace
is touched, no other Unicode whitespace is touched, and no Unicode
normalization (case folding, NFC/NFD, width folding) is applied anywhere.
`0x09`/`0x0A`/`0x0D` can only occur in a decoded label via an escape
(`\t`/`\n`/`\r`), since a raw control byte is rejected by CNORM-010; a raw
`0x20` may occur directly. `id` is never trimmed or otherwise transformed —
its decoded value is used exactly as decoded.

**CNORM-013 (record order).** The output preserves the exact input record
order. Records are never sorted, grouped, or deduplicated by any key other
than the exact rejection in CNORM-014.

**CNORM-014 (duplicate ids).** Two records in one batch are duplicates when
their **decoded** `id` values are byte-for-byte equal, scanning records in
order; the **second** (later) occurrence is rejected as `duplicate_id`
(CNORM-029). This is decoded-value equality, not raw-span equality: `"id":
"café"` and `"id": "café"` denote the same id and are duplicates, the
same distinction [JSON Cursor Adapters v1](JSON-CURSORS-V1.md#decoded_token_eq)
draws for `std.data.json.dec.decoded_token_eq` (a token-level pull
comparison, not the byte-span comparison `std.data.json.doc.is_unique` uses
for *keys*). An implementation with at most 256 records can compare each new
id against every earlier one with `decoded_token_eq` in a bounded `while`
loop, the same quadratic-but-allocation-free shape
`std.data.json.doc.object_keys` already uses for member-name uniqueness (see
[Bounded JSON Scanner v1 § Duplicate keys](BOUNDED-JSON-SCANNER-V1.md#duplicate-keys)).

**CNORM-015 (checked total).** `total_quantity` is the **checked** `i64` sum
of every accepted record's `quantity`, accumulated in input order. An
addition that would exceed `i64::MAX` is rejected as `overflow`
(CNORM-029b) at the record whose addition overflowed; the sum is never
computed via wraparound, saturation, or an unchecked wider type that is
later truncated. (`quantity` itself is already checked nonnegative by
CNORM-009, so overflow is the only failure mode for the running total.)

**CNORM-016 (all-or-nothing).** A batch is validated as a whole before any
output is produced. The **first** rejection encountered, scanning records
strictly in input order and applying the per-record checks in the fixed
order CNORM-020..029b list them, is the **only** one reported; no earlier
partial output (a "valid prefix") is ever published alongside or instead of
the error envelope, matching the language's own sticky-failure and
transactional publication invariants extended to this application's own
contract.

## Failure categories and exact positions

**CNORM-020 (result shape).** Every response is exactly one line: either a
success envelope (CNORM-041) or an error envelope (CNORM-042), UTF-8, ending
in exactly one `0x0A`. An error envelope carries exactly three fields beyond
`status`:

- `category` — one of the nine names below, always present;
- `record_index` — a zero-based index into the input's records (per
  CNORM-001's line splitting), or `-1` for a whole-batch failure not
  attributable to one record (only CNORM-020's total-size case);
- `byte_offset` — a zero-based byte offset **within the line named by
  `record_index`** (or, for the `record_index == -1` case, within the whole
  request body), per the exact rule for that category below.

The nine frozen categories, and the exact position rule for each:

| Category | Meaning | `record_index` | `byte_offset` |
| --- | --- | --- | --- |
| `oversized_input` (whole-batch) | Request body > 65536 bytes (CNORM-002) | `-1` | `65536` |
| `oversized_input` (record count) | A 257th line exists (CNORM-003) | `256` | `0` |
| `oversized_input` (line length) | A line exceeds 512 bytes (CNORM-004) | that line's index | `512` |
| `oversized_input` (id/label length) | Decoded `id` > 64 bytes, or decoded `label` > 256 bytes (CNORM-011) | that record's index | offset of the value's opening `"` |
| `invalid_utf8` | The line is not valid UTF-8 (CNORM-005) | that line's index | first invalid byte, per Unicode's own decode-error convention (`str::from_utf8`'s / Python's `UnicodeDecodeError.start`) |
| `malformed_json` | Not valid JSON, not an object, bad escape, unterminated token, disallowed nesting depth, or trailing bytes (CNORM-006, CNORM-007, CNORM-010) | that line's index | the exact offending byte, or the line's byte length for an unterminated token that runs off the end of the line |
| `schema` | Missing/extra/duplicate required key, wrong value type, empty id, negative or fractional/out-of-range quantity (CNORM-008, CNORM-009, CNORM-011's empty-id case) | that record's index | for a bad key: that key's opening `"`; for a missing key: the record object's opening `{`; for a bad value: that value's start |
| `duplicate_id` | A later record repeats an earlier record's decoded id (CNORM-014) | the later (rejected) record's index | the `id` value's opening `"` |
| `overflow` | The checked running total would exceed `i64::MAX` (CNORM-015) | the record whose addition overflowed | that record's `id` value's opening `"` |
| `provider_denied` / `provider_timeout` / `provider_malformed` | Enrichment outcome for this record's id, when enrichment is enabled (CNORM-062) | that record's index | the `id` value's opening `"` |

There is no tenth, catch-all category. Every rejection this document
describes maps to exactly one of these nine names.

**CNORM-021.** A zero-length line (CNORM-001) is `malformed_json` at offset
`0` of that (empty) line.

## Canonical output

**CNORM-040 (encoding).** Every field name and every fixed literal below is
plain ASCII; the response carries no insignificant whitespace (no spaces
after `:` or `,`) and exactly one line. Numbers are rendered in canonical
decimal exactly as [Bounded JSON Scanner v1 § Writing](BOUNDED-JSON-SCANNER-V1.md#writing)
specifies for `std.data.json.digits`: no leading zeroes (except the literal
`0`), a leading `-` only for a true negative value, and never computed via
`f64`. String values are rendered exactly as
[Bounded JSON Scanner v1 § Writing](BOUNDED-JSON-SCANNER-V1.md#writing)
specifies for `std.data.json.write`: `"` and `\` as their two-byte named
escapes, the five named short control escapes (`\b`, `\t`, `\n`, `\f`,
`\r`), every other byte below `0x20` as lowercase `\u00hh`, and every other
byte — including raw multi-byte UTF-8 — passed through unchanged.

**CNORM-041 (success envelope).** Field order is fixed:

```
{"status":"ok","count":<N>,"total_quantity":<sum>,"records":[<record>,...]}
```

`count` is the number of accepted records (always equal to the number of
input lines, since CNORM-016 means a batch that reaches the success envelope
had every record accepted). Each `<record>` has fixed field order:

```
{"id":<string>,"label":<string>,"quantity":<int>}
```

or, when enrichment is enabled (CNORM-060), with one more field appended
after `quantity`:

```
{"id":<string>,"label":<string>,"quantity":<int>,"category":<int-or-null>}
```

`id` is the record's decoded, un-normalized id (CNORM-010). `label` is its
**normalized** value (CNORM-012). `quantity` is the exact validated integer.
`category` is present **only when enrichment was requested for this run**
(CNORM-060); it is the provider's `category_code` on a `found` outcome, or
JSON `null` on a `missing` outcome (CNORM-061). An empty batch (zero
records) renders as `{"status":"ok","count":0,"total_quantity":0,"records":[]}`.

**CNORM-042 (error envelope).** Field order is fixed:

```
{"status":"error","category":"<name>","record_index":<i>,"byte_offset":<j>}
```

per the table in CNORM-020.

**CNORM-043 (determinism).** For fixed input bytes and a fixed enrichment
flag and fixture table, the output bytes are always identical: no locale,
wall-clock, random, environment-variable, hash-iteration-order, or
floating-point-formatting dependence anywhere in this pipeline. Enrichment
outcomes come only from the frozen fixture table (CNORM-060); nothing here
performs, or depends on, a live network call.

## Optional enrichment (a bounded, typed provider effect)

**CNORM-060.** Enrichment is **disabled by default**; network access is
disabled by default (this application never performs one — even "enabled"
enrichment resolves only against the frozen fixture table in
`tests/oracle/catalog_normalizer/fixtures/enrichment.json`, never a live
service). A run either requests enrichment for every record in the batch or
requests it for none; there is no per-record opt-in. This is the effect seam
issue #123 (SPX-AI-024) is scoped to compose from existing Reader/Writer and
provider pieces; this document only fixes its exact contract.

**CNORM-061.** When enrichment is enabled, each accepted record's decoded
`id` (CNORM-010, exactly as validated — not the label) is looked up in the
fixture table by exact byte-string match. The outcome is exactly one of:

- `found` with a `category_code` (a nonnegative integer), rendered as that
  record's `"category"` field;
- `missing` (an id absent from the table, or explicitly marked `missing`),
  rendered as `"category":null`; **not an error**;
- `denied`, `timeout`, or `malformed` — a provider-level failure. Each is
  terminal and **non-retryable**: encountering one at record *i* rejects the
  whole batch as `provider_denied` / `provider_timeout` / `provider_malformed`
  at record index *i* (CNORM-020's table), with no partial output
  (CNORM-016), and it is never retried, downgraded to `missing`, or treated
  as recoverable.

**CNORM-062.** Enrichment lookups happen **after** a record has otherwise
passed every check in CNORM-008..CNORM-015 for that record specifically, but
**before** that record is appended to the output and before the next
record's checks begin — so a provider failure on record *i* still reports
`record_index = i` and never depends on whether later records would also
have failed. This keeps provider, parse, validation, and capacity failures
distinct per record, matching issue #123's requirement to keep those
categories separate.

## Worked examples

These reproduce two of the frozen published cases byte for byte; see
`tests/oracle/catalog_normalizer/cases/published/` for the full corpus.

Input (`basic-single-record`):

```
{"id":"a1","label":"hello","quantity":5}
```

Output:

```
{"status":"ok","count":1,"total_quantity":5,"records":[{"id":"a1","label":"hello","quantity":5}]}
```

Input (`overflow-total-rejected`, two lines):

```
{"id":"a1","label":"x","quantity":9223372036854775807}
{"id":"a2","label":"y","quantity":1}
```

Output:

```
{"status":"error","category":"overflow","record_index":1,"byte_offset":7}
```

(`7` is the offset of `id`'s opening `"` on the second line,
`{"id":"a2",...`.)

## The oracle

`tests/oracle/catalog_normalizer/oracle.py` is the executable form of every
rule above, written independently in Python — a different language and
toolchain than SEMAPRAX, sharing no parser, decoder, or library with any
Semaprax implementation. See that directory's own `README.md` for its exact
protected-location policy, its CLI, its `--self-test` mode, and how the
frozen corpus and negative controls are organized. See
`tests/useful_data/catalog_normalizer_oracle.rs` (registered in
`tests/useful_data.rs`) for the Rust-side proof that the oracle runs, passes
its corpus, and rejects deliberately wrong candidate output.

## Requirement index

For traceability: `CNORM-001`..`CNORM-016` (input grammar and semantics),
`CNORM-020`..`CNORM-029b` (failure categories and positions, via the
CNORM-020 table plus CNORM-021), `CNORM-040`..`CNORM-043` (canonical
output), `CNORM-060`..`CNORM-062` (enrichment). All are frozen as of this
document's first version; see "Change procedure" above for what changing any
of them requires.
