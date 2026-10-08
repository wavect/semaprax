# Strict JSON Scan v1

Audience: language users and compiler contributors.

Status: additive source package; focused local interpreter, native C11, Core Wasm,
ordinary-project bundling, and catalog checks pass. See the
[OPT batch receipt](../benchmarks/opt-batch-verification-v1/opt680-682-verification.json).

`std.data.json.scan` is a borrowed-byte, allocation-free facade for applications
that need to consume a complete JSON document. It composes the existing
structural scanner, UTF-8 validator, and decoded string-token query. The older
`std.data.json.doc.string_end` continues to frame strings without validating
escapes or raw UTF-8; `std.data.json.doc.is_unique` continues to compare encoded
member names. Their stable identities and behavior are unchanged.

## Validation and result

The package has the `useful-data.v1` profile and depends on bundled
`std.data.json.doc`, `std.data.json.query`, and `std.data.json.utf8`, including
the query package's transitive `std.data.json` dependency. It requires no
capability and stores no owned JSON tree, token vector, key buffer, or hidden
state. Its inputs are borrowed `Slice<u8>` values; its outputs are Copy scalars.

`strict_end(input, depth_limit, duplicate_policy)` validates exactly one whole
document. On success it returns `byte_len(input)`. A result greater than that
length encodes a selected rejection offset as `result - length - 1`, using the
existing scanner family encoding. An unsupported duplicate policy selects
offset zero. Otherwise structural grammar and depth failure takes priority,
with the offset chosen by `std.data.json.doc.whole_end`. Only a structurally
valid document proceeds to raw UTF-8, strict string, and decoded-duplicate
checks. For these checks the facade chooses the earlier of the raw UTF-8
failure and the first rejection found by its string/object scan. An object
duplicate check can inspect keys ahead of the outer scan cursor. Therefore the
selected offset is deterministic, but it need not be the earliest malformed
byte across different validation stages. `is_document` and `is_unique` select a
32-container maximum and return a boolean. Depth counts open objects and
arrays and is clamped to 32 by the structural scanner. A complete value may be
a scalar, array, or object; whitespace is limited to space, tab, LF, and CR.

Validation covers object and array grammar, exact literals, RFC 8259 number
syntax, no trailing non-whitespace data, raw UTF-8 scalar validity, raw control
byte rejection inside strings, the eight admitted simple escapes, `\u` hex
digits, and correctly paired UTF-16 surrogates. Escaped code points are decoded
as UTF-8 by the existing query package. Number lexemes remain exact byte spans;
the facade never rounds or parses them as `f64`.

The duplicate policy is explicit:

| Policy | Meaning |
| --- | --- |
| `0` | Accept duplicate member names, retaining JSON's permissive baseline. |
| `1` | Reject two names within the same object when their decoded UTF-8 bytes match. |
| any other value | Reject at offset zero. |

Thus `"a"` and `"\u0061"` are duplicates under policy `1`. Names in sibling
objects are independent. The comparison uses repeated source scans and scalar
cursors; it needs no key allocation. It may take quadratic time in the number
of members of one object. The duplicate diagnostic identifies the second key's
opening quote. UTF-8 and escape validity are independently checked for every
quoted token, including both member names and string values.

## Navigation

Call `strict_end` once, check its result against `byte_len(input)`, and then
retain the same immutable byte view. The following functions use absolute
offsets in that view. A navigation offset is meaningful only when obtained from
`root` or another navigator after successful validation. The functions do not
repeat whole-document validation or carry a proof token.

| Function | Result |
| --- | --- |
| `root(input)` | First value after leading whitespace. |
| `kind(input, start)` | `1` object, `2` array, `3` string, `4` number, `5` true, `6` false, `7` null; `0` for no value class. |
| `value_end(input, start, depth_limit)` | Exclusive end of one validated value. |
| `first_member(input, object)` | First key's opening quote, or `byte_len(input)` for none. |
| `member_value(input, key)` | Start of that key's value after the colon and whitespace. |
| `next_member(input, key, depth_limit)` | Next direct key, or `byte_len(input)` for none. |
| `first_element(input, array)` | First element's start, or `byte_len(input)` for none. |
| `next_element(input, element, depth_limit)` | Next element's start, or `byte_len(input)` for none. |
| `key_eq(input, key, expected)` | Whether a quoted key decodes to the caller's expected UTF-8 byte view. |
| `decimal_end(input, start)` | Exclusive end of an exact JSON number lexeme. |

The `depth_limit` argument in navigation should equal the successful validation
limit. The `byte_len(input)` sentinel cannot be mistaken for a valid item start.
For a number starting at `start`, the exact decimal source is
`[start, decimal_end(input, start))`; callers can use `byte_range` after checking
the bounds. No conversion to an integer or floating-point value is implied.

The source package's conformance cases cover escaped-key equality, duplicate
policy, nested and sibling objects, hostile escapes, surrogate errors, invalid
UTF-8, malformed numbers, incomplete containers, trailing input, bounded
depth, scalar navigation, and exact decimal offsets. The package execution
matrix owns interpreter, native C11 `-O0`/`-O2`, and Core Wasm admission.
