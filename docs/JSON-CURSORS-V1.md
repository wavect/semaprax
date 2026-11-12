# JSON Cursor Adapters v1

Status: implemented bounded adapters; **HOSTED GREEN** under the
[v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md), including the admitted
standalone, Project v16 and cross-package roundtrip profiles.

Audience: language users, standard-library contributors, and backend
implementers.

These allocation-free adapters connect existing `std.io` cursors to the
bounded JSON packages without changing older APIs. `Reader` and `Writer`
remain the nongeneric [IO Cursors v1](IO-CURSORS-V1.md) records, each with a
caller-supplied owned `Bytes` value and `usize` cursor. The adapters add no
stream, handle, file, or host authority.

## API surface

The additions are source functions with these signatures:

```text
std.data.json.dec.decode_into(
    input: borrow std.io.Reader,
    output: own std.io.Writer
) -> std.io.Writer

std.data.json.write.quoted_into(
    input: borrow std.io.Reader,
    output: own std.io.Writer
) -> std.io.Writer

std.data.json.write.count_into(
    value: usize,
    output: own std.io.Writer
) -> std.io.Writer
```

`decode_into` consumes the output writer and returns its advanced replacement.
The input is borrowed and remains available, with its cursor and owned buffer
unchanged. `quoted_into` has the same ownership and cursor behavior; `count_into`
consumes a scalar count and the output Writer. As with `writer_write_u8`, a successful operation advances the output
cursor once per emitted byte; `writer_finish` returns the original caller
buffer, including its unwritten suffix.

These are additive package APIs. Existing scanner, decoder, quoted-byte,
decimal-byte, Reader, and Writer functions retain their signatures, IDs, and
meaning. The adapters do not add compiler operations, a hidden allocation, a
public nominal ABI, or an owned JSON tree.

## Reader position and preflight

The string adapters read bytes beginning at the borrowed reader's
current `position`. The position must be within the reader buffer, as required
by the existing cursor observers. A borrowed input is never advanced: callers
that need a new cursor can use the existing owned `reader_advance` transition
after the operation.

Every byte count and every input span is computed before the first output byte
is written. The output writer's position must likewise be within its buffer,
and its remaining capacity must be at least the complete result size. A
capacity failure occurs before any buffer write or cursor update; there is no
partial JSON output. The consumed writer follows ordinary failure cleanup and
is not returned to the caller. The same all-or-nothing preflight applies to
malformed or incomplete input. The ordinary checked contract and failure
selection rules decide how a rejected call is reported; this specification
does not introduce a new result encoding.

The preflight is exact and uses the caller's actual buffer length. The old
`std.data.json.dec.capacity()` value of `256` remains an API for the bounded
comparison helper; it is not a limit on these cursor adapters. Caller output
buffers larger than 256 bytes are valid, and an adapter may fill any capacity
that its exact preflight proves sufficient.

## `decode_into`

`std.data.json.dec.decode_into` decodes one complete JSON string token at the
reader cursor. It applies the existing `std.data.json.dec` rules: the opening
and closing quotes are required, simple escapes and `\uXXXX` escapes are
expanded, strict surrogate pairing is enforced, raw control bytes are
rejected, and raw bytes that are admitted by the existing decoder pass through
unchanged. The token's decoded length is determined with `decoded_len` before
the writer is touched, then the exact decoded bytes are emitted in source
order.

The token must be complete in the reader's remaining range. Bytes after its
closing quote remain unread and are not consumed or interpreted by this
operation; callers that require a whole input can first apply the existing
full-string scanner policy. A string that is truncated, malformed, or whose
decoded result does not fit the writer is rejected during preflight, before
any output mutation. An empty JSON string is valid and leaves the output cursor
unchanged after the successful zero-byte emission.

The operation has the same bounded meaning as `decoded_size` and the same
UTF-8 policy: it expands escapes to UTF-8 for escaped scalar values, while
preserving admitted raw input bytes. It does not validate a raw UTF-8 sequence
that the existing decoder does not validate. It does not retain a decoded
string object, and no decoded view escapes the call.

## `decoded_token_eq`

`std.data.json.dec.decoded_token_eq(input, left, right)` compares two decoded
JSON string tokens in `input` for equality. It walks both tokens through the
package's existing pull surface (`emit_at`, `emit_len`, `token_end`), the same
surface `decode_into` and the caller-streamed token loop use, so escapes
decode to their scalar value before either side is compared: a simple escape
and a `\uXXXX` escape for the same character compare equal, and `"ab"` equals
a separately positioned `"ab"`. Comparison rejects up front when either
token's `decoded_len` is a failure or when the two tokens' decoded lengths
differ, and otherwise compares decoded bytes one emitted position at a time.
No intermediate buffer is allocated; unlike `decoded_eq`, this comparison is
not bound by the package's 256-byte comparison capacity.

This is a token-level comparison only. Duplicate-key *detection over a JSON
document* — walking an object's members and reporting a repeated name — is
still Missing here; that object walk lives in `std.data.json.doc`, which
compares member name spans byte for byte and does not yet consult decoded
values either.

## Decoded-key fingerprints

Additive scalar helpers let a caller index decoded keys without an owned map:

```text
std.data.json.dec.decoded_byte_at(input, index, part) -> usize
std.data.json.dec.decoded_fingerprint(input, start, limit) -> usize
std.data.json.dec.fingerprint_entry(opening, fingerprint) -> usize
std.data.json.dec.entry_opening(entry) -> usize
std.data.json.dec.entry_fingerprint(entry) -> usize
std.data.json.dec.fingerprint_match(input, entry, current, fingerprint) -> bool
```

`decoded_byte_at` returns the canonical decoded byte at one pull cursor, or
`256` when that cursor emits none. An admitted raw byte is its own value; an
escape uses `emit_at`.

The caller must validate the input's raw UTF-8 before using these token
helpers; a fingerprint does not establish UTF-8 validity. This is the same
prerequisite as the pull decoder and exact decoded-token comparison.

`decoded_fingerprint` requires `1 <= limit <= 256`. It walks the token at
`start` once through `emit_len`, `decoded_byte_at` and `token_end`, folding each
decoded byte `b` as `hash = (hash * 257 + b) % 65521`. A complete string with
`1..=limit` decoded bytes returns `length * 65536 + hash`, below `16842752`.
Any other token, including an empty string, malformed escape, unterminated
token, or a token whose decoded size exceeds `limit`, returns `0`. The walk
stops at the first decoded byte past `limit`, so its work is bounded by `limit`
rather than by the token's source span. Escaped, surrogate-pair and raw
spellings of the same bytes produce the same fingerprint.

Raw bytes in `32..=255`, excluding the quote and escape markers, use the
same one-byte pull meaning directly. This includes the constituent bytes of
UTF-8 scalars; it changes neither raw UTF-8 validation nor escape expansion.
The package conformance retains the previous ASCII-only shortcut as an
independent reference, checks all 256 scalar byte inputs, and compares valid
raw/escaped Unicode at exact decoded-length and one-short bounds. Invalid raw
UTF-8 constituent tests establish the old helper's total byte behavior only;
they grant no application admission or UTF-8 validity.

`fingerprint_entry` packs an opening offset of at most 131,072 and a
fingerprint into one Copy `usize`; `entry_opening` and `entry_fingerprint`
recover them. `fingerprint_match` returns `false` when the stored fingerprint
differs from `fingerprint`. Otherwise it returns `decoded_token_eq(input,
entry_opening(entry), current)`.

A fingerprint is a rejection filter only, never equality authority. Every equal
fingerprint, including every hash collision, is decided by exact decoded-byte
comparison. Callers keep their own input order, first-match selection and
error precedence. The helpers allocate nothing, add no compiler operation, and
grant no capability.

At `7268e5fbe`, the Catalog Normalizer passed all 36 published oracle cases,
the fourteen named source tests, and the 21 focused decoded-index cases. The
focused cases use an independent decoded-byte oracle and cover deliberate
unequal UTF-8 collisions, raw and escaped equivalence including surrogate
pairs, NUL, common prefixes, the 64/65-byte ID boundary, 256 records, and
first-error order. They ran on the interpreter, native C11 at `-O0` and `-O2`,
and Core Wasm. The independently executable pairwise reference passed the
same application matrix. The maximal direct-response cases retained their
160,000,000-step interpreter limit.
[OPT #771](https://github.com/wavect/semaprax/issues/771)

## `quoted_into`

`std.data.json.write.quoted_into` emits a quoted JSON string for every byte in
the reader's remaining range. Its size is the exact sum of the opening and
closing quotes plus `quoted_len`'s per-byte widths. Its bytes are exactly the
ones defined by `quoted_byte` and `escape_byte`:

- `"` and `\\` use their two-byte named escapes;
- backspace, tab, line feed, form feed, and carriage return use the existing
  short escapes;
- other bytes below `0x20` use lowercase `\u00hh` escapes;
- all other bytes pass through unchanged.

The full quoted size is preflighted before writing the opening quote, so a
short writer cannot receive a prefix. The input remains borrowed and its
remaining bytes are copied in order into the JSON representation. Bytes after
the reader cursor are all included; there is no implicit terminator or
extra whitespace.

## `count_into`

`std.data.json.write.count_into` writes the decimal ASCII representation of its
`value: usize` argument into the owned writer and returns the advanced writer.
It uses the existing `usize_len` and `usize_byte` policy, with no sign and no
leading zeroes except for the value zero. The decimal width is preflighted
before the first digit is written, and the writer advances by exactly the
number of decimal digits. The function does not read or modify a `Reader`.

## Evaluation, ownership, and backends

Arguments and all helper calls evaluate left to right. Input validation,
decoded or encoded length calculation, and output-capacity validation complete
before output mutation. A successful call transfers the output `Writer` as one
ordinary owned value and publishes its advanced cursor. A rejected call does
not publish a partially written writer or fabricated input state. Existing
cleanup and sticky failure selection remain authoritative.

The adapters are source-level compositions of existing cursor, JSON scanner,
decoder, and writer operations. They require no new compiler primitive and
grant no ambient filesystem, process, network, home, secret, key, wallet, or
signing capability. Interpreter, native C11, and Core Wasm implementations
must consume the same checked HIR meaning. No backend may infer validity from
the `Reader` or `Writer` carrier layout.

This document makes no public hosted-I/O, physical-device, production, nominal
ABI, growable-buffer, or owned-document-tree claim. The implemented adapter
corpus has hosted-green release evidence; the broader Everyday profile remains
partial.

## Project composition

The decoder retains `owned-data-api.v1`. The writer uses the additive
`useful-data.v2` (Project v16) profile: private cursor calls use checked owned
data, while its existing `quoted_len` web export retains the frozen Useful
Data v1 public projection. A public owning or nominal cursor export is not
admitted. Both packages declare the exact bundled `std.io` dependency.

`quoted_remaining_len` observes the encoded size of the Reader remainder
without consuming either buffer. Independent cursor conformance programs use
a two-entry byte arena; legacy decoder conformance keeps its one-entry arena.
The new cases include a 300-byte decoded string and sentinel bytes outside the
output prefix. These tests do not add an owned document tree or a streaming
parser.

## Local verification record

The original local standalone decoder and writer cursor corpus passed on the
interpreter, native C11 at `-O0` and `-O2`, and Core Wasm. It included a 300-byte
decoded string, demonstrating that the adapters are not limited by the decoder's
old 256-byte comparison buffer. Six malformed-input, insufficient-capacity,
and forged-cursor contract-rejection cases also passed with no partial output
or cursor publication.

Project v16's
`profile_admission::project_v16_json_cursor_public_facade_replays_and_executes`
gate covers deterministic npm reconstruction, envelope replay, and repeated
Node execution. Cross-package decode/requote roundtrip uses the
`private_json_cursor_roundtrip_executes_across_project_backends` gate on the
interpreter entry and repeated test, native C11 at `-O0` and `-O2`, and repeated
Core Wasm with a strict two-entry arena. The unchanged 16 MiB workspace budget
fits this gate. The historical local observations retain their original scope;
the implemented release corpus is hosted green without changing the public
export or nominal-ABI boundary.

The JSON Query byte conversion and UTF-8 offset calculation passed their
package conformance gates at `114d7434f` on the interpreter, native C11 at
`-O0` and `-O2`, and Core Wasm. Query conformance checked all 256 byte values.
UTF-8 conformance checked all 256 byte values at eight base/span boundaries
and verified the absent-offset result. [OPT #753](https://github.com/wavect/semaprax/issues/753)

The generated identifier decoder passed its collision, escaped-equivalence
and domain-refusal cases at the default 1,000,000 interpreter steps. The
256-record collision fixture preserved the late grammar error and its original
offset. Maximum UTF-8 requests with 264 values of 64 decoded bytes passed for
raw and escaped ASCII and for raw, escaped and mixed Unicode. The maximum-shape
checks retained their 160,000,000-step and 16 MiB interpreter limits and ran
through the same native and Wasm backends. The catalog producer regenerated
identical human, agent and authoring catalog bytes.
