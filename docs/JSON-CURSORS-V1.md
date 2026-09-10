# JSON Cursor Adapters v1

Status: implemented bounded adapters; **HOSTED GREEN** under the
[v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md), including the admitted
standalone, Project v16 and cross-package roundtrip profiles.

Audience: language users, standard-library contributors, and backend
implementers.

This specification adds small, allocation-free adapters between the existing
`std.io` cursors and the existing bounded JSON packages. It extends the
packages without replacing or changing any old API. A `Reader` and `Writer`
remain the ordinary nongeneric records from [IO Cursors v1](IO-CURSORS-V1.md):
each owns a caller-supplied `Bytes` value and carries a `usize` cursor. These
functions do not introduce streams, handles, files, or host authority.

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
