# Project Literal Constructors v1

Status: authored, unrun. No completion or target-execution promotion.

Audience: agent builders, compiler contributors, and reviewers.

The recursive candidate expression grammar adds two closed literal forms:

```json
{"kind":"string","value":"hello\n世界"}
{"kind":"array_u8","values":[0,127,255]}
```

`string` carries decoded Unicode text, including empty strings, NUL, other
control characters, quotes and backslashes. The value is literal content,
never a source fragment, identifier, escape-language input or interpolation.
The existing canonical source formatter escapes it, and ordinary source
reparsing must recover exactly the same contents without Unicode normalization.
It constructs the existing owned `string` expression, not a borrowed `str`,
an implicit byte conversion, or a new string operation.

`array_u8` carries an ordered array of JSON integers in `0..=255`. Empty arrays
are valid. There are no recursively evaluated elements, integer coercions,
source suffixes, repeat counts, implicit allocations or `Bytes` constructors.
The existing source array literal determines the exact Copy type `[u8; N]`.
Canonical source renders each element with its ordinary `u8` suffix.

Both forms compose with the existing expression constructors in function
bodies, selected expressions, declarations and typed hole fills. Structural
availability does not make either form valid at an arbitrary selected type,
contract, effect budget or target profile. Complete candidate formatting,
reparsing and independent Project admission remain mandatory. Selected
expression replacement must additionally preserve the original resolved type
and ownership; changing an array's length changes its type.

## Ownership and byte views

String literals create owned values under the existing ownership and cleanup
rules. String place reads and transfers retain ordinary compiler semantics,
including existing cloning behavior. This constructor adds no ownership
relaxation and does not prove allocation will succeed.

Byte-array literals retain the existing inline storage and call-path budgets.
To use `core.array-u8.as-slice`, first bind the array with the scoped
[`let` constructor](PROJECT-LEXICAL-BINDING-CONSTRUCTOR-V1.md), then supply that
named place to the [byte builtin constructor](PROJECT-BUILTIN-CALL-CONSTRUCTOR-V1.md).
Bind the resulting view before passing it onward wherever source provenance
requires a named view. No constructor manufactures a borrow root, relaxes a
loan lifetime or permits borrowing an array temporary. The ordinary
[indexed byte data contract](PORTABLE-INDEXED-BYTE-DATA-V1.md) owns those checks.

String-bearing declarations outside an executable entry/test closure remain
source-retention evidence only. This addition does not widen ordinary Wasm,
Project import, aggregate, export or package admission.

## Limits, discovery and diagnostics

A string literal contains at most 16,384 UTF-8 bytes, checked before cloning
its payload into the constructed AST. A byte-array literal contains at most
4,095 elements. Each array element consumes one unit of the shared 4,096-unit
expression construction budget, in addition to the literal's root. Other
enclosing constructors and siblings consume the same budget; 4,095 elements
are therefore only possible when the root has the full remaining allowance.
This is a constructor bound, not a reduction of the source language's array
length limit. Compact repeat-array construction is not included.

The existing 64-depth expression bound, 8,192-node/64-depth JSON input bounds,
1 MiB Semantic Change bound, source/output bounds and transport-specific limits
remain active. These limits do not promise a total memory or execution budget.
Invalid shape, wrong JSON value type and out-of-range byte values use
`SPX-G225`; literal or shared construction capacity excess uses `SPX-G226`.
Ordinary source type, ownership, borrowing and target diagnostics remain
authoritative after construction. Failure leaves the original candidate,
sibling drafts and canonical source unchanged.

[Constructor schemas](CANDIDATE-CONSTRUCTOR-SCHEMAS-V1.md) close both forms.
The string schema permits length zero and records both the character limit
and the stricter UTF-8 byte bound. Array schema metadata records cumulative
element charging, which structural JSON Schema alone cannot enforce.
Change catalogues and full/compact hole contexts advertise the two kinds;
they do not promise a successful fill. Finite checked hole suggestions keep
their existing place/direct-call search and do not automatically invent
literal values.

The additive [scalar literal extension](PROJECT-SCALAR-LITERAL-CONSTRUCTORS-V1.md)
completes the shared expression and signature-default grammar with exact
`char`, `f32` and `f64` encodings. Inert record-field defaults and scalar
diagnostic retagging retain their separate narrower grammars. No method,
permission, publication route, source syntax, Graph schema or backend operation
is added.

## Evidence

[Candidate regressions](../tests/project_candidate_literal_constructors_v1.rs)
and [protocol regressions](../tests/image_literal_constructors_v5.rs) are
authored but unrun. They require exact literal contents and source replay,
ownership/type/provenance checks, failure immutability and discovery alignment.
The existing generated Rust client serialized-size gate remains unchanged and
must still pass for every policy; no client-size cap is raised by this work.
No compiler, interpreter, generated consumer or quality gate was executed for
this addition. The full graph-operational programme remains Partial.
