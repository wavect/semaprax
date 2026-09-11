# Public Generic Type Grammar v1

Status: implemented bounded projection with local evidence; gates PG-1 and PG-2
of the [Public Generic Ownership milestone](PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md).
No hosted run is recorded for it, it is not selected by any public descriptor,
carrier, package, or consumer, and public generic ownership remains unsupported
and unpublished. A term of this grammar is not a public ABI.

Audience: ABI, package, evidence, and generated-consumer maintainers.

## Scope

`public_generic_type` projects one already-checked `ResolvedType` into a
versioned, target-neutral term plus the explicit template and ordered argument
identities derived from it. It is read-only: it admits no syntax, compiles
nothing, executes nothing, creates no file, and grants no authority. Nothing is
read back from a previously emitted artifact; every fact is re-derived from the
checked program.

| Layer | Identifier |
| --- | --- |
| Grammar schema | `semaprax.public-generic-type-grammar.v1` |
| Term digest domain | `semaprax.public-generic-type-grammar.v1.term\0` |
| Template digest domain | `semaprax.public-generic-type-grammar.v1.template\0` |
| Instance digest domain | `semaprax.public-generic-type-grammar.v1.instance\0` |

The grammar is deliberately not the compiler's internal `identity_key`
spelling. That key is unversioned, carries type parameters and constructs this
vocabulary excludes, and may change with internal work; publishing it would
make an internal detail an interchange format.

## Admitted vocabulary

```
term     := scalar | "bytes" | instance
scalar   := "i64" | "i32" | "u8" | "usize" | "char" | "f32" | "f64" | "bool"
instance := "@" length ":" declaration-id "<" arguments ">"
arguments:= ε | term ("," term)*
length   := the decimal byte length of declaration-id, without a leading zero
```

An admitted instance names an authored `record` declaration, supplies exactly
one term per declared type parameter, and nests only this vocabulary. A record
with no type parameters is admitted and renders an empty argument list
(`@13:grammar.plain<>`), so a concrete record and a zero-argument instance have
one spelling rather than two.

Every other checked type rejects with exactly one closed reason:

| Reason | Rejected type |
| --- | --- |
| `type_parameter` | an unsubstituted type parameter |
| `owned_string` | `string` |
| `borrowed_str` | `str` |
| `borrowed_byte_view` | `Slice<u8>` |
| `unit` | `unit` |
| `inline_byte_array` | `[u8; N]` |
| `function_type` | a function type |
| `compiler_owned_nominal` | `Option`, `Result`, `Vec`, `Box`, `Iter`, and the rest of the compiler-owned inventory |
| `unadmitted_nominal_kind` | an authored class, variant, or resource |
| `missing_declaration` | a nominal whose declaration is absent from the checked program |
| `ambiguous_declaration` | a nominal whose identity appears twice in the checked program |
| `arity_mismatch` | an argument count unequal to the declared arity |

A rejection is closed and total: there is no partial admission, no repair, and
no fallback spelling. Widening the vocabulary — variants, resources, borrowed
views, owned strings, collections — is a new grammar version with its own
gates, never a silent admission inside v1.

## Injectivity

Every declaration identity is length-prefixed in bytes. An identity may
therefore contain `<`, `>`, `,`, `:`, `@`, non-ASCII bytes, or be empty, and
still render and parse back exactly; two distinct types can never render alike.
A delimiter-only grammar would confuse `@1:a<@1:b<>>` with an identity spelled
`a<@1:b<`; the length prefix makes that impossible, and the executable case
pins it.

Parsing is strict. Whitespace, a leading zero in a length prefix, a length that
exceeds or splits the identity bytes, a NUL byte in an identity, a missing or
doubled delimiter, a trailing comma, an unknown token, and any trailing byte
after the term all fail closed with `SPX-PG103`.

## Template and ordered argument identities

A template identity binds the persistent declaration identity, the declared
arity, and the ordered parameter positions as owner-and-index pairs. Display
names of the record, its type parameters, and its fields are carried as
presentation and are never hashed, so a display rename leaves the template
digest, the instance digest, and the term unchanged.

An instance identity binds the template digest, the canonical term bytes, every
ordered argument (position, parameter owner, parameter index, argument digest),
every substituted field (index, field identity, field term digest) in
declaration order, and the ordered transitive owned-leaf paths. Permuting,
duplicating, or substituting an argument therefore changes the instance
identity; omitting one is not a different instance but an `arity_mismatch`
rejection.

Fields are substituted by exact owner and index before descendants are
examined, so a nested instance's field terms are the concrete ones. Owned
leaves are the transitive direct-`Bytes` positions, reported in structural
order as identity-framed paths (`@17:grammar.pair.left/@17:grammar.pair.left`).
An instance with no owned leaf is a valid term but not an ownership surface;
that requirement belongs to a surface admission, not to the grammar.

## Bounds and diagnostics

| Bound | Value |
| --- | --- |
| Canonical term bytes | 65,536 |
| Record nesting depth | 64 |
| Transitive owned leaves | 256 |
| Visited type nodes | 4,096 |
| Declared template arity | 16 |

The nesting, leaf, and node bounds match the existing nested owned-record work
limits, and a recursive call never resets one. Reaching a bound is a refusal
(`SPX-PG102`), never a truncated or repaired term.

| Code | Meaning |
| --- | --- |
| `SPX-PG101` | the type is outside the admitted vocabulary; the message carries the closed reason |
| `SPX-PG102` | a grammar bound was reached |
| `SPX-PG103` | submitted bytes are not a canonical term |
| `SPX-PG104` | submitted bytes parse but do not equal the independently recomputed term |

Replay compares bytes. `verify_term` parses the submitted bytes, requires them
to be their own canonical rendering, independently recomputes the term from the
checked program, and requires byte equality. Submitted bytes are never treated
as source, HIR, identity, or authority.

## Nonclaims

This grammar defines no descriptor, carrier, package, calling convention,
layout, or memory representation; it makes no compatibility decision, emits no
consumer, allocates nothing, settles no failure, and observes no runtime. It
does not widen any language admission profile: a type this grammar can spell is
not thereby admitted in a public signature, and the milestone's separation gate
continues to pin that the public projections reject generic surfaces. It is not
hosted evidence, not a support decision, and not a publication.
