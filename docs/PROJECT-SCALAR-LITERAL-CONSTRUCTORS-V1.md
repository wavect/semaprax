# Project Scalar Literal Constructors v1

Status: authored, unrun. No completion or target-execution promotion.

Audience: agent builders, compiler contributors, and reviewers.

The candidate expression grammar and exact literal defaults for signature
evolution expose the complete built-in Copy-scalar vocabulary. The existing
five forms retain their wire representation:

```json
{"kind":"i64","value":-1}
{"kind":"i32","value":-1}
{"kind":"u8","value":255}
{"kind":"usize","value":42}
{"kind":"bool","value":true}
```

Three additive forms carry characters and floats without relying on JSON
number precision, host float parsing, source escapes or surrogate handling:

```json
{"kind":"char","scalar":"0001f600"}
{"kind":"f32","bits":"3dcccccd"}
{"kind":"f64","bits":"3fb999999999999a"}
```

`scalar` is exactly eight lowercase hexadecimal digits. Its decoded `u32` must
be one Unicode scalar value, so surrogate code points and values above
`0010ffff` reject. The spelling is a numeric transport encoding, not source
text or a JSON character. Canonical source formatting owns escaping NUL,
controls, quotes, backslashes and non-ASCII values; reparsing must recover the
same scalar without normalization.

`f32.bits` and `f64.bits` are exactly eight and sixteen lowercase hexadecimal
digits respectively and carry the complete IEEE-754 representation. Only
finite values are constructible. Positive and negative zero, subnormals,
ordinary finite values and the maximum finite magnitudes are admitted; every
infinity and NaN encoding rejects. Source syntax cannot represent nonfinite
values as literals, so admitting them would violate canonical source replay.

Source parses a leading minus as unary negation rather than part of a float
literal. A sign-bit-set finite request therefore lowers to the existing unary
negation node over a positive-magnitude float literal. This includes negative
zero. The generated unary node consumes the shared expression-node budget and
the ordinary depth bound. Positive encodings lower directly. Canonical
formatting and reparsing must recover this structure and its exact magnitude
bits; checked evaluation recovers the originally requested signed bits. This
route does not change the lexer, parser, formatter, arithmetic or IEEE rules.

## Composition and scope

All eight scalar forms compose through recursive function-body, selected-body,
contract and typed-hole expressions. The legacy append-signature form and the
ordered signature mapping form may add any of the eight by-value types with an
exact matching literal default. Caller staging, argument order, failure
selection, ownership checks and complete candidate replay remain unchanged.

This shared literal grammar does not widen every feature that happens to use a
scalar. Record-field defaults remain the explicit five-kind
`i64`/`i32`/`u8`/`usize`/`bool` surface. Diagnostic literal retagging remains
the explicit four integer kinds. Computed signature argument expressions keep
their separately admitted type selectors. Function/data declaration schemas,
generic arguments, extraction, movement and target profiles are unchanged.

The forms construct existing AST and HIR values. They do not add source syntax,
Graph schemas, coercions, imports, capabilities, runtime operations or
publication authority. Structural discovery is not proof that a literal fits
the selected expression type, contract phase, effect inventory, caller or
target profile. Formatting, source reparsing and independent full-Project
admission remain authoritative.

## Schemas, limits and failure

Constructor schemas close every object to the fields shown above. Fixed-width
lowercase patterns make request validation deterministic in generated
TypeScript, Python and Rust clients. Catalogues and full and compact hole
contexts advertise `char`, `f32` and `f64` alongside the prior scalar kinds.
No generated-client payload cap is raised.

Each wire literal consumes its ordinary root node. A negative float consumes
one additional generated unary node. The existing 4,096-node/64-depth
constructor bounds, enclosing JSON and Semantic Change limits, Project source
limits and transport limits remain active. These are structural bounds, not a
total memory or execution guarantee.

Malformed object shape, wrong field type, wrong width, uppercase or nonhex
digits, invalid Unicode scalars and nonfinite float encodings use `SPX-G225`.
Generated-node or other construction capacity excess uses `SPX-G226`. Ordinary
source type, ownership, contract and target diagnostics follow construction.
Every failure leaves live source, existing candidates and sibling drafts
unchanged.

## Evidence

[Candidate regressions](../tests/project_candidate/scalar_literal_constructors.rs)
and [protocol regressions](../tests/image_v5/literal_constructors.rs) are
authored but unrun. They cover exact Unicode and IEEE boundaries, signed-float
lowering, canonical source and Graph/HIR replay, expression and hole
composition, both signature-evolution forms, malformed input, recovery and the
unchanged narrower record/diagnostic grammars. The existing generated Rust
client serialized-size gate remains unchanged and unrun.

No compiler, client, interpreter, backend, test or quality gate was executed
for this addition. The graph-operational programme remains Partial.
