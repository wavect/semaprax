# Public Generic Metadata Consumers v1

Status: implemented bounded generator with local evidence. This closes the
*grammar* half of gates PG-5 and PG-6 of the
[Public Generic Ownership milestone](PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md);
both gates stay open for their descriptor half, because no public generic
descriptor, carrier, or calling convention exists. No hosted run is recorded,
and public generic ownership remains unsupported and unpublished.

Audience: generated-consumer integrators, ABI reviewers, and promotion
reviewers.

## Why metadata consumers come first

Before a foreign toolchain can *call* a public generic export, several
toolchains have to agree on what its types are — and refuse everything else.
That agreement is falsifiable on its own, without a calling convention: give
four languages the same canonical description, and require all four to accept
exactly those bytes and to refuse each hostile variant with the same closed
reason.

A generated consumer here therefore receives no SEMAPRAX value, allocates
none, frees none, instantiates no Wasm module, and links against nothing. Its
generated type declarations show the substituted field tree in the target
language; they define no layout, no ownership transfer, and no ABI. Every
generated file carries that statement in its own banner, and the executable
gate asserts the banner is present.

## Identifiers

| Layer | Identifier |
| --- | --- |
| Metadata format | `semaprax.public-generic-consumer-metadata.v1` |
| Magic prefix | `spxpgcm1;` |
| Languages | `rust`, `typescript`, `c`, `cxx` |
| Type spelling | [Public Generic Type Grammar v1](PUBLIC-GENERIC-TYPE-GRAMMAR-V1.md) |
| Described surface | [Public Generic Compatibility v1](PUBLIC-GENERIC-COMPATIBILITY-V1.md) |

## The metadata format

```
document := "spxpgcm1;" record*
record   := "|" <field count> ";" field+
field    := <byte length> ":" <bytes> ";"
```

Counts are decimal without a leading zero. Every field is framed by its byte
length, so a declaration identity may contain the format's own punctuation — or
a newline — without a parser ever guessing where a field ends. The first field
of a record is its kind:

| Kind | Fields |
| --- | --- |
| `E` | export identity, parameter count |
| `P` | export identity, index, position kind, ownership mode, type |
| `R` | export identity, position kind, type |
| `I` | instance term, template declaration identity, declared arity |
| `A` | instance term, argument index, argument term |
| `F` | instance term, field index, field identity, field term |
| `L` | instance term, leaf index, owned-leaf path |
| `D` | surface digest |

Record order is the surface's own byte order — exports by declaration
identity, instances by canonical term, then by index — and the `D` record
closes the document, so a stale description differs from a fresh one even when
every other fact agrees.

## What every consumer does

1. Parse strictly. Exact byte lengths, no leading zero in a count, a `;` after
   every field, a `|` before every record, no NUL inside a field, and nothing
   after the last record.
2. Validate every type field as a canonical term of the type grammar, with the
   grammar's own 64-level and 4,096-node bounds, requiring the term to consume
   its field exactly.
3. Compare the whole document against the metadata the consumer embeds.

Refusals are a closed three-value vocabulary, identical in all four languages:

| Reason | Meaning |
| --- | --- |
| `malformed` | not a well-formed document of the format |
| `term` | a type field is not a canonical grammar term |
| `mismatch` | well formed, but not the expected document |

There is deliberately no fourth "non-canonical" reason. The format admits no
optional whitespace, no alternate spelling, and no leading zero, so a strict
parse that consumes the document already implies the bytes are their own
canonical rendering; a reason no input could produce would be decoration, not
a closed vocabulary.

A consumer run with no argument verifies its embedded bytes; run with a file
it verifies that file. It prints `ok` and exits 0, or prints
`refused:<reason>` and exits 3.

## Generated declarations

Type names are derived from bytes, not from display names: the lowercase hex of
the exact instance term or field identity. A rename therefore changes no
generated identifier, and two distinct instances can never collide. Nested
instances are declared before the instances that hold them, because a struct
member of an incomplete type is an error in C and C++.

The scalar spellings are declaration types for reading metadata, not an ABI
mapping:

| Grammar | Rust | TypeScript | C | C++ |
| --- | --- | --- | --- | --- |
| `i64` | `i64` | `bigint` | `int64_t` | `std::int64_t` |
| `i32` | `i32` | `number` | `int32_t` | `std::int32_t` |
| `u8` | `u8` | `number` | `uint8_t` | `std::uint8_t` |
| `usize` | `u64` | `bigint` | `uint64_t` | `std::uint64_t` |
| `char` | `char` | `string` | `uint32_t` | `char32_t` |
| `f32` | `f32` | `number` | `float` | `float` |
| `f64` | `f64` | `number` | `double` | `double` |
| `bool` | `bool` | `boolean` | `bool` | `bool` |
| `bytes` | `Vec<u8>` | `Uint8Array` | `struct spx_pg_bytes` | `std::vector<std::uint8_t>` |

The TypeScript consumer is two files: the ES module a Wasm host would load and
the ambient declarations a TypeScript author compiles against.

## Bounds and diagnostics

| Bound | Value |
| --- | --- |
| Canonical metadata bytes | 1,048,576 |
| Records per document | 8,192 |
| Fields per record | 8 |

Reaching a bound is a refusal, never a truncated or repaired document.
`SPX-PG401` reports a refusal on the Rust side and `SPX-PG402` a bound.
Generation is deterministic — regenerating produces byte-identical files — and
authority-free: it reads checked facts, returns source text, and touches no
file, process, or network.

## Evidence

The `public_generic_consumers` gate in the projections harness generates all
four consumers, compiles them warning-free (`-Wall -Wextra -Werror` for C and
C++, `-D warnings` for Rust), and runs each one on its embedded metadata, on
the same bytes from a file, and on nine hostile documents: empty, wrong magic,
truncated, a leading-zero count, a field count reaching past the record, a
forged term length, reordered records, an appended record, and a stale surface.
Each language must report the same closed reason as the Rust reference reader
and as every other exercised language, and the gate fails if no toolchain was
available rather than passing silently. A language whose toolchain is absent is
skipped; that is a narrower run, not a passing one.

## Nonclaims

This defines no calling convention, descriptor, carrier, package, layout,
allocation, or ownership transfer, and no value crosses any boundary. It does
not admit a public generic signature: the public projections still reject
generic surfaces, and the milestone's separation gate continues to prove it.
It is not hosted evidence, not a support decision, and not a publication. The
remaining half of PG-5 and PG-6 — consumers that call a public generic export
over a versioned descriptor and carrier, and hostile replay of those descriptor
bytes — is untouched by this work.
