# Bounded JSON Scanner v1

Audience: language users, tool authors, and standard-library contributors.

Status: partially implemented, across five sibling packages that share one
result encoding. Each is pure, allocation-free, and operates on a borrowed
byte view or Copy scalars:

| Package | Admitted scope |
| --- | --- |
| `std.data.json` | The **JSON string token**: whitespace skipping, escape classification, `\uXXXX` code-unit decoding, strict surrogate-pair rules, control-byte rejection, and a byte offset for the first rejection |
| `std.data.json.token` | **Number and literal tokens**: the RFC 8259 number grammar and exact `i64` decoding, plus `true`, `false`, and `null` |
| `std.data.json.utf8` | **UTF-8 validation** of raw bytes, rejecting malformed, overlong, surrogate, and out-of-range sequences |
| `std.data.json.write` | Deterministic **string encoding**: the exact length and each byte of the quoted JSON encoding of a byte view |
| `std.data.json.digits` | Deterministic **number and literal encoding**: the exact decimal bytes of any `i64` and the literal words |

Structural document validation, decoded string output, an owned document tree,
and an output buffer are Missing.

This document owns the result encoding and rejection policy shared by all
five. [Standard Library v1](STANDARD-LIBRARY-V1.md) owns their status rows and
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

`failure(input, offset)` builds the rejection value, `is_failure(input, r)`
tests it, and `failure_offset(input, r, fallback)` decodes it, returning
`fallback` for a success value so that it is total. The encoding is exact and
allocation-free, and lets one scan carry both the answer and the diagnostic
offset. A rejection offset equal to `n` means the input ended early, so a
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

## What is not implemented

These are absent, not merely undocumented. A program must not infer them:

- **Structural validation.** There is no object, array, nesting-depth,
  trailing-byte, or duplicate-key rule, and therefore no notion of a complete
  JSON document. Nothing here validates that a sequence of tokens is a
  well-formed value.
- **Floating-point numbers.** `i64_or` refuses a fraction or exponent rather
  than converting it, and nothing renders an `f64`.
- **Decoded strings.** Escapes are validated and measured, never expanded;
  expansion needs an output buffer with an explicit capacity.
- **An output buffer**, pretty-printing, and any owned document
  representation. The writer reports bytes; it does not store them.
- **A composed reader/writer round trip** over a whole document.

## The limit that shapes this package

The scanner is smaller than the surrounding design because of a compiler
bound, not a library choice. The Workspace Semantic Graph pre-bound described
in [Workspace Semantic Graph v1](WORKSPACE-SEMANTIC-GRAPH-V1.md#limits-and-budget)
charges an upper estimate of resolver memory for the whole link closure, and
[Standard Library v1](STANDARD-LIBRARY-V1.md) requires every library function
to be imported by the package's conformance module, which charges each
function's tree a second time. The budget is charged against the whole package — library,
examples, and conformance modules together — so no single module can hold this
slice. Measured by padding each of these packages until `SPX-G171` fires, the
admitted total package source is between 13.3 KB and 15.9 KB.

The scope is therefore authored as sibling packages a consumer links, which
became viable when the pre-bound stopped charging an imported function as a
second complete copy of its provider. Each package restates the two- or
three-line byte-inspection helpers it needs rather than depending on a
sibling, because a `[dependencies]` edge spends the whole dependency source
against the consumer's budget while the helper costs a few hundred bytes.
Structural document validation still needs either a further split or that
bound raised.
