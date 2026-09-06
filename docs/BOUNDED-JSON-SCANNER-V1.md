# Bounded JSON Scanner v1

Audience: language users, tool authors, and standard-library contributors.

Status: partially implemented, across six sibling packages that share one
result encoding. Each is pure, allocation-free, and operates on a borrowed
byte view or Copy scalars:

| Package | Admitted scope |
| --- | --- |
| `std.data.json` | The **JSON string token**: whitespace skipping, escape classification, `\uXXXX` code-unit decoding, strict surrogate-pair rules, control-byte rejection, and a byte offset for the first rejection |
| `std.data.json.token` | **Number and literal tokens**: the RFC 8259 number grammar and exact `i64` decoding, plus `true`, `false`, and `null` |
| `std.data.json.utf8` | **UTF-8 validation** of raw bytes, rejecting malformed, overlong, surrogate, and out-of-range sequences |
| `std.data.json.write` | Deterministic **string encoding**: the exact length and each byte of the quoted JSON encoding of a byte view |
| `std.data.json.digits` | Deterministic **number and literal encoding**: the exact decimal bytes of any `i64` and the literal words |
| `std.data.json.doc` | **Structural documents**: the object and array grammar, a bounded nesting depth, trailing-byte rejection over a whole document, and a duplicate-key rule over byte-identical member names |

Decoded string output, an owned document tree, and an output buffer are
Missing.

This document owns the result encoding and rejection policy shared by all
six. [Standard Library v1](STANDARD-LIBRARY-V1.md) owns their status rows and
the admission limits that shape them.

## Why a scanner and not a document

SEMAPRAX admits no growable collection today: `Bytes` is uniquely owned and
immutable, `[u8; N]` is fixed and Copy, and `Slice<u8>` is a non-escaping
borrowed view ([Portable indexed byte data](PORTABLE-INDEXED-BYTE-DATA-V1.md)).
A JSON *document* in the usual sense is a tree of owned nodes, so v1 does not
build one. It instead answers questions about the caller's own bytes:

- where does the JSON string token that starts here end?
- where is the first byte that cannot be part of it?
- is this whole byte range exactly one JSON string?
- what code point does this `\uXXXX` escape denote?

Every function takes `borrow Slice<u8>` and returns a Copy scalar. No value
the scanner produces can outlive the source, because the scanner never
produces a view: offsets are meaningful only against the exact slice that was
passed in, and the language already prevents that slice from escaping.

## Result encoding

Locating functions return `usize`. Let `n` be `byte_len(input)`.

| Result | Meaning |
| --- | --- |
| `r <= n` | success; `r` is the exclusive end offset |
| `r > n` | rejection; the first offending byte is at `r - n - 1` |

`std.data.json.failure(input, offset)` builds the rejection value,
`is_failure(input, r)` tests it, and `failure_offset(input, r, fallback)`
decodes it, returning
`fallback` for a success value so that it is total. The encoding is exact and
allocation-free, and lets one scan carry both the answer and the diagnostic
offset. Every package here produces and consumes exactly these values;
`std.data.json` is the one that exports the three helpers, and a package that
does not depend on it writes the same arithmetic inline. A rejection offset equal to `n` means the input ended early, so a
truncated string is never reported as a complete one.

## Lexical rules

`skip_whitespace` steps over the four JSON whitespace bytes `0x20`, `0x09`,
`0x0A`, and `0x0D`, and is total: a `start` past the end yields the length.

`string_end(input, start)` requires a `"` at `start` and returns the offset
one past the closing `"`. Raw bytes `0x00`-`0x1F` are rejected inside a
string, as RFC 8259 requires, at their own offset. A string that is never
closed is rejected at the end of the input.

`escape_kind(input, start)` classifies the escape that starts at a `\`:

| Result | Meaning |
| --- | --- |
| `0` | not an escape, or a rejected one |
| `1` | one of `\"`, `\\`, `\/`, `\b`, `\f`, `\n`, `\r`, `\t`, two bytes |
| `2` | `\uXXXX` denoting a non-surrogate scalar, six bytes |
| `3` | `\uXXXX` denoting a high surrogate, the first half of a pair |

`escape_end(input, start)` turns that into an end offset. Surrogates follow
the strict rule:

- a code unit outside `D800`-`DFFF` is one complete six-byte escape;
- a high surrogate `D800`-`DBFF` must be followed immediately by `\u` and a
  low surrogate `DC00`-`DFFF`; the pair is one escape of twelve bytes;
- a lone high surrogate and any low surrogate that is not the second half of
  a pair are rejected at the offset of the backslash that opened them.

`code_unit(input, start)` decodes exactly four hexadecimal digits, in either
case, to `0`-`65535`, and returns `-1` when any of the four is absent or not
a hexadecimal digit. `hex_at` is the single-digit form.

`is_string(input)` is `string_end(input, 0)` reaching exactly
`byte_len(input)`: the whole input is one complete JSON string and nothing
else.

## Number and literal tokens

`std.data.json.token` scans the RFC 8259 number grammar in three parts that
compose through the same encoding: `integer_end` takes an optional `-` and
either a single `0` or a nonzero leading digit followed by digits, so `01`
ends its token after the `0`; `fraction_end` takes an optional `.` that must
be followed by at least one digit; `exponent_end` takes an optional `e` or
`E`, an optional sign, and at least one digit. `number_end` composes the
three and stops at the first rejection; `is_number` is `number_end` reaching
exactly `byte_len(input)`.

`literal_kind` returns `1` for `true`, `2` for `false`, `3` for `null`, and
`0` otherwise, and `literal_end` turns that into an end offset or a rejection
at `start`.

`i64_or(input, start, fallback)` decodes an integer token exactly. It returns
`fallback` — never a rounded or truncated value — when the token is absent,
when the token is immediately followed by `.`, `e`, or `E` (so a value that is
not an integer is refused rather than silently truncated), or when the
magnitude leaves the `i64` range. It accumulates negatively, so
`-9223372036854775808` decodes exactly and no intermediate overflows.

## UTF-8 validation

`std.data.json.utf8` validates the caller's raw bytes, which the string
scanner deliberately does not. `sequence_kind` classifies a lead byte as a 1-,
2-, 3-, or 4-byte sequence or `0` for anything that cannot lead one, so
`0xC0`, `0xC1`, and `0xF5`-`0xFF` are rejected at the lead. `scalar_at`
decodes one sequence to its scalar value or `-1`, rejecting missing or
malformed continuations, overlong encodings, the surrogate range
`D800`-`DFFF`, and anything above `U+10FFFF`. `sequence_end` and `utf8_end`
lift that to the shared offset encoding, and `is_utf8` is `utf8_end` reaching
exactly `byte_len(input)`.

## Structural documents

`std.data.json.doc` is the only package here that reads a *document* rather
than a token. It is still allocation-free and still returns only scalars: the
container stack is a single `i64` used as a base-2 stack with a sentinel bit,
so an object push is `stack * 2 + 1`, an array push is `stack * 2`, a pop is
`stack / 2`, and `stack == 1` is the top level. Nesting is therefore bounded by
an explicit counted limit rather than by a call stack; the language admits no
recursion here and none is used.

`document_end(input, start, depth_limit)` scans exactly one JSON value,
skipping leading whitespace, and returns the offset one past it.
`whole_end(input, depth_limit)` additionally requires that only JSON whitespace
follows, so a **trailing byte is rejected at its own offset**.
`is_document(input)` is `whole_end` with the maximum depth, reaching exactly
`byte_len(input)`.

- **Depth is explicit and bounded.** `depth_limit` counts open containers; a
  container opened beyond it is rejected at the offset of its `{` or `[`. The
  parameter is clamped to **32**, which is the depth `is_document` uses, so no
  caller can ask for an unbounded scan.
- **Grammar.** Six modes drive the scan: a value is required, a value or the
  closing `]`, a key, a key or the closing `}`, the `:`, and then `,` or the
  matching closer. A closer that does not match the innermost container is
  rejected at its own offset, as is a `,` that is not followed by a member.
- **Tokens.** `number_end` is the RFC 8259 number grammar, so `01`, `1.`, and
  `1e` are rejected; `literal_end` accepts only the exact words `true`,
  `false`, and `null`; `string_end` frames a string, rejecting raw `0x00`-`0x1F`
  and an unterminated string at end of input.
- **State machine as data.** `step_action(input, index, mode, stack)` returns
  `action + class * 16`, `next_state(action, mode, stack)` returns
  `mode + stack * 8`, and `advance(input, index, step)` turns a packed action
  into the next offset. They are exported so a caller can drive the same
  machine over its own buffer.

### What the document layer delegates

The layer owns *structure*. Two token-level rules stay with their owning
sibling and are **not** re-checked here, because the workspace-graph pre-bound
described below does not admit both in one package:

- **Escape characters.** `std.data.json.doc.string_end` treats a `\` and the
  byte after it as two bytes. That finds the correct end of every string,
  including `\uXXXX`, but it does not check that the escape is one of the
  admitted forms or that surrogates pair. `std.data.json.escape_kind`,
  `escape_end`, and `is_string` own that rule.
- **UTF-8.** Raw bytes are not decoded. `std.data.json.utf8.is_utf8` owns that
  rule, and applies to the whole document as one byte range.

A caller that needs full RFC 8259 conformance composes the three:
`is_document(view) && is_utf8(view)`, plus `is_string` over each string span.

### Duplicate keys

`std.data.json.doc.is_unique(input)` is `is_document` plus member-name
uniqueness, and `unique_end(input, depth_limit)` is its offset form under the
family result encoding. `is_document` itself is unchanged: it still **accepts**
a repeated name, so a caller that wants the RFC 8259 recommendation asks for it
by name.

The rule is stated exactly, because it is narrower than "the same string twice":

- Two members of **the same object** are duplicates when their **name spans are
  byte-identical**, quotes included. Names in different objects never collide,
  and a `{` inside a string is not an object.
- **Escape forms are compared as written.** `"a"` and `"\u0061"` denote the
  same name in RFC 8259 but are *not* duplicates here, because nothing in this
  package expands an escape. Decoded comparison is Missing and needs the same
  output buffer decoded strings need.
- Rejection is reported at the offset of the **`{` of the object that repeats a
  name**, not at the repeated member, so a nested violation names the inner
  object.

The implementation retains nothing, which is what makes it admissible: there is
no growable collection to hold the earlier keys in. `unique_end` first validates
the document, then walks the bytes once, skipping each string span through
`string_end` so that only structural `{` bytes are seen. For each such object,
`object_keys` walks its direct members through `next_key` — which skips the
member's value with `document_end` — and asks `key_before` whether an earlier
member of the same object carries the same name span, comparing bytes through
`span_same`. The cost is quadratic in the members of one object and grows with
name length; the benefit is that the scan is still allocation-free and still
returns only scalars.

The owned bounded byte buffer of
[Owned Bounded Byte Buffer v1](OWNED-BOUNDED-BYTE-BUFFER-V1.md) is *not* a
usable key record here, for two independent reasons: `bytes_set` admits only a
`usize` **literal** index (`SPX-T272`) while a key record is written at offsets
a scan discovers, and neither operation is admitted in a `while` body at all
(`SPX-T252`, `SPX-T267`). Core Wasm is no longer one of those reasons: both
operations now execute there through the `env.spx_bytes_zeroed` and
`env.spx_bytes_set` host-arena imports, so a future key record would have to
clear only the two limits above.

## Writing

The writer is *pull-based*: it computes the exact output length and then the
byte at each output index, so it needs no output buffer and stays inside the
language's allocation-free profile. The caller supplies the destination.

`std.data.json.write.quoted_len(input)` is the exact byte length of the
quoted JSON encoding of `input`, and `quoted_byte(input, index)` is its byte
at `index`, or `-1` past the end. `escape_len` and `escape_byte` are the
per-input-byte form: `"` and `\` and the five named control escapes become two
bytes, every other byte below `0x20` becomes six bytes as `\u00XX` with
lowercase hexadecimal, and every other byte — including raw UTF-8 — passes
through unchanged. `usize_len` and `usize_byte` render a count exactly.

`std.data.json.digits.i64_len(value)` and `i64_byte(value, index)` render any
`i64` in canonical decimal. The rendering never passes a value through an
`f64` and never negates the minimum: it accumulates toward zero so that
`-9223372036854775808` renders as its exact twenty bytes. `literal_len` and
`literal_byte` give the bytes of `true`, `false`, and `null`.

## Consuming the slice

`examples/agent-response-project` is the worked consumer: a four-module
Package Manifest v1 project under `useful-data.v1` whose only dependency is
`std.data.json.doc`. It validates an agent-style response - one whole document
with a bounded depth, no trailing bytes, and no repeated member name in any
object - finds its top-level members by walking `next_key` and comparing name
spans, and renders a pull-based structured verdict. It has no allocation, no
network, and no filesystem authority, and
`tests/useful_data.rs::agent_response_project` executes it on the interpreter,
on native C11 at `-O0` and `-O2`, and on Core Wasm under Node.

Its shape is governed by the same pre-bound as the packages. A consumer that
vendors the 10.3 KB document layer keeps about 8.8 KB of its own source, and a
consumer of two JSON packages keeps about 5.8 KB, so that project restates a
two-digit decimal helper locally instead of taking a second dependency on
`std.data.json.digits`. The directory's own README records the exact
measurement.

## What is not implemented

These are absent, not merely undocumented. A program must not infer them:

- **Decoded duplicate keys.** `is_unique` compares name spans byte for byte,
  so two spellings of one name, such as `"a"` and `"\u0061"`, are not reported;
  see above.
- **Floating-point numbers.** `i64_or` refuses a fraction or exponent rather
  than converting it, and nothing renders an `f64`.
- **Decoded strings.** Escapes are validated and measured, never expanded;
  expansion needs an output buffer with an explicit capacity.
- **An output buffer**, pretty-printing, and any owned document
  representation. The writer reports bytes; it does not store them.
- **A composed reader/writer round trip** over a whole document.

## The limit that shapes this package

The scanner and the document layer are smaller than the surrounding design
because of a compiler bound, not a library choice. The Workspace Semantic Graph pre-bound described
in [Workspace Semantic Graph v1](WORKSPACE-SEMANTIC-GRAPH-V1.md#limits-and-budget)
charges an upper estimate of resolver memory for the whole link closure, and
[Standard Library v1](STANDARD-LIBRARY-V1.md) requires every library function
to be imported by the package's conformance module, which charges each
function's tree a second time. The budget is charged against the whole package — library,
examples, and conformance modules together — so no single module can hold this
slice. Measured by padding each of these packages until `SPX-G171` fires, the
admitted total package source is between 19.7 KB and 22.1 KB for the token
packages, after the identity term of the pre-bound was re-derived; the same
measurement before that re-derivation gave 13.3 KB to 15.9 KB. The bound is not a byte count: it is charged against declarations,
expression structure, and the length of the longest stable identity in the
package, so a package with many small declarations and dense expressions is
admitted at far less source. `std.data.json.doc` was measured at **12,216 B
admitted and 12,292 B rejected** before the identity term was re-derived — its
own declarations cost roughly twice per byte what the token packages' do, which
is why its helper set is merged down and its module segment is `doc` rather
than `document`. After the re-derivation it was measured again by appending
`at_in`-shaped probe functions to the library and their imports to the
conformance module: **19,250 B of package source admitted, 19,524 B rejected**.
The duplicate-key layer spends most of that headroom; the package is
**18,154 B**, so about **1.1 KB** remains.

The scope is therefore authored as sibling packages a consumer links, which
became viable when the pre-bound stopped charging an imported function as a
second complete copy of its provider. Each package restates the two- or
three-line byte-inspection helpers it needs rather than depending on a
sibling, because a `[dependencies]` edge spends the whole dependency source
against the consumer's budget while the helper costs a few hundred bytes.
Two consequences of that measurement shape `std.data.json.doc`:

- **It depends on nothing.** A `[dependencies]` edge on `std.data.json` alone
  (4,769 B) or on `std.data.json` and `std.data.json.token` together (9,333 B)
  puts the document layer over the bound even though its own source is only
  8.5 KB, because the vendored dependency source is charged in full against the
  consumer. Restating the few byte probes it needs costs a few hundred bytes.
- **It delegates escape and UTF-8 validity.** Restating `hex_at`, `code_unit`,
  `escape_kind`, and `escape_end` costs about 2.2 KB before the conformance
  coverage those exports require, and the UTF-8 rules cost more again. The
  1.1 KB left after the duplicate-key layer admits neither.

Decoded strings, an output buffer, an owned document tree, and re-checking
escape and UTF-8 validity inside the document layer still need either a further
split or that bound raised.
