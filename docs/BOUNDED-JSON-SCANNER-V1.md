# Bounded JSON Scanner v1

Audience: language users, tool authors, and standard-library contributors.

Status: partially implemented. `std.data.json` is a pure, allocation-free
scanner over a borrowed byte view. Its admitted scope in this version is the
**JSON string token**: whitespace skipping, escape classification, `\uXXXX`
code-unit decoding, strict surrogate-pair rules, control-byte rejection, and a
byte offset for the first rejection. Number tokens, literal tokens, structural
document validation, decoded string output, an owned document tree, and a
writer are Missing.

This document owns the scanner's result encoding and rejection policy.
[Standard Library v1](STANDARD-LIBRARY-V1.md) owns the package's status row
and the admission limits that shape it.

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

## What v1 does not do

These are absent, not merely undocumented. A program must not infer them:

- **Number tokens.** There is no number grammar, no exact `i64` decoding, and
  no floating-point conversion. Nothing in v1 rounds an integer through an
  `f64`, because nothing in v1 reads a number at all.
- **Literal tokens.** `true`, `false`, and `null` are not scanned.
- **Structural validation.** There is no object, array, nesting-depth,
  trailing-byte, or duplicate-key rule, and therefore no notion of a complete
  JSON document.
- **UTF-8 validation.** `string_end` accepts every byte at or above `0x20`,
  including bytes that are not valid UTF-8. Callers that need UTF-8 validity
  must check it separately.
- **Decoded strings.** Escapes are validated and measured, never expanded;
  expansion needs an output buffer with an explicit capacity.
- **A writer**, and any owned document representation.

## The limit that shapes this package

The scanner is smaller than the surrounding design because of a compiler
bound, not a library choice. The Workspace Semantic Graph pre-bound described
in [Workspace Semantic Graph v1](WORKSPACE-SEMANTIC-GRAPH-V1.md#limits-and-budget)
charges an upper estimate of resolver memory for the whole link closure, and
[Standard Library v1](STANDARD-LIBRARY-V1.md) requires every library function
to be imported by the package's conformance module, which charges each
function's tree a second time. Measured on this package, a library module of
roughly 4.7 KiB of this code's density is admitted and one of roughly 5.6 KiB
fails with `SPX-G171`, and a consumer that links two such packages fails as
well, so the missing scope cannot be recovered by splitting it across sibling
packages. Growing the admitted scope needs that bound raised, not more
library code.
