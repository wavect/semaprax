# Project Builtin Call Constructor v1

Status: authored, unrun implementation and regression coverage. No completion
promotion or target execution evidence.

Audience: compiler contributors, agent builders, and reviewers.

Typed candidate expressions can select an existing compiler-owned byte or string
operation without inventing a source function or import:

```json
{
  "kind": "builtin_call",
  "target": "core.bytes.len",
  "arguments": [{ "kind": "place", "name": "input" }]
}
```

The object is closed to these three fields. `target` is a stable operation
identity, never a source spelling. Arguments use the ordinary recursive typed
expression grammar, retain request order, and consume its existing node,
depth, and input budgets. There are no type arguments or implicit conversions.
The existing `call` constructor retains its local/imported function lookup.

## Compiler ownership and source replay

`src/byte_ops.rs` and `src/string_ops.rs` own the closed operation inventories,
source spellings, arities, and semantic signatures. This constructor projects
those inventories; it introduces no new language semantics, evaluator, or
backend operation.

| Stable identity | Source operation | Arguments | Result |
| --- | --- | --- | --- |
| `core.bytes.len` | `byte_len` | borrowed byte slice | `usize` |
| `core.bytes.get` | `byte_get` | borrowed byte slice, `usize` index | `Option<u8>` |
| `core.bytes.range` | `byte_range` | borrowed byte slice, `usize` start, `usize` end | byte slice |
| `core.bytes.copy` | `bytes_copy` | borrowed byte slice | owned `Bytes` |
| `core.bytes.as-slice` | `bytes_as_slice` | borrowed `Bytes` place | byte slice |
| `core.array-u8.as-slice` | `array_as_slice` | borrowed fixed byte-array place | byte slice |
| `core.str.as-bytes` | `str_as_bytes` | borrowed `str` | byte slice |
| `core.string.as-str` | `string_as_str` | unprojected owning `string` place | borrowed `str` |
| `core.string.len` | `string_len` | borrowed String read | `i64` |
| `core.string.concat` | `string_concat` | two consumed Strings | owned `string` |
| `core.string.is_empty` | `string_is_empty` | borrowed String read | `bool` |
| `core.string.starts_with` | `string_starts_with` | two borrowed String reads | `bool` |
| `core.string.contains` | `string_contains` | two borrowed String reads | `bool` |
| `core.string.len_chars` | `string_len_chars` | borrowed String read | `i64` |
| `core.string.from_char` | `string_from_char` | copied `char` | owned `string` |

String operations retain their [ordinary source contract](STRING-OPS-V1.md).
`len` counts UTF-8 bytes; `len_chars` counts Unicode scalar values. Borrowed
reads leave their named owners available, while concatenation consumes both
arguments using the existing ownership machinery. Compiler-generated staging
and String place reads retain their ordinary cloning/allocation behavior;
`borrow` metadata is not a claim of allocation-free execution. `from_char`
accepts an existing checked character place or an ordinary character-valued
expression. The separate [scalar literal constructor](PROJECT-SCALAR-LITERAL-CONSTRUCTORS-V1.md)
can construct that checked character; this operation does not coerce an
integer/string into a character. String and byte operations remain distinct:
there is no implicit String-to-`str`, byte-slice or `Bytes` conversion.

An authored Project identity that collides with the selected compiler identity
fails closed. Reserved spelling collisions and active lexical bindings must
not redirect a requested operation. Constructor lowering emits the ordinary
source call, then the complete candidate is canonically rendered, reparsed,
and independently admitted through the existing Project path. Wrong argument
types, unsupported profiles, escaping views, live-loan conflicts, cleanup,
capacity, and contract restrictions remain compiler obligations. A structurally
valid request is not evidence that a candidate can be admitted.

Before source materialization, the disposable edited AST inventory is also
checked against builtin selectors in the retained intention history and new
request. This catches a colliding declaration introduced by the same request
or a later candidate edit, rather than relying only on the prior revision's
identity inventory. Both checks use the complete source inventory, including
protocol and implementation IDs omitted by runtime graph lookup. The prior
inventory is parsed lazily once per expression constructor; it is not a new
persistent index. Invalid selectors, arity, scope, or namespace collisions use
the existing `SPX-G225` constructor diagnostic. Semantic rebase conflicts
retain the existing `SPX-G235` diagnostic; source admission keeps its own
type, provenance, and ownership diagnostics.
The history check is conservative: overwriting an earlier expression does not
release its builtin selector for reuse as an authored declaration identity.

View operations retain their existing source-place constraints. Bind an owned
value before borrowing it; do not treat a nested temporary as an authenticated
owner. A slice produced by a range operation retains its original provenance.
See [Portable Indexed Byte Data](PORTABLE-INDEXED-BYTE-DATA-V1.md),
[Shared Loan Plan](SHARED-LOAN-PLAN-V1.md), and
[Projected Owned Byte Field Borrow](PROJECTED-OWNED-BYTE-FIELD-BORROW-V1.md).
The constructor does not widen those language profiles.

The separate [Field Place Constructor](PROJECT-FIELD-PLACE-CONSTRUCTOR-V1.md)
can supply a direct authenticated record field to `core.bytes.as-slice` without
the value-staging temporary used by `project`. Its root-type and field-identity
checks do not waive the source borrow profile or live-loan checks.

`bytes_copy` allocates an owned value and participates in capacity and cleanup
analysis. An empty declared effect list grants no ambient authority and does
not mean allocation-free execution, unrestricted contract admission, or
guaranteed success.

Semantic rebase binds compiler-owned operation descriptors separately from
ordinary source function dependencies. It rechecks identity and spelling
availability in the destination revision and retains nested ordinary-call
dependencies. Source function signature, effect, and contract guards remain
in force; a matching source identity is not compiler provenance.

## Discovery

Recursive constructor schemas derive fourteen closed alternatives from the
two compiler operation inventories. Each alternative fixes the target and exact argument
count while recursively referring to the complete expression grammar.

Target-specific change catalogues and full typed-hole contexts can expose
`builtin_calls` separately from their existing ordinary `accessible_calls`.
Rows describe the constructor kind, target, source name, arity, ordered
parameters, return type identity, effects, evidence owner, and the requirement
for full candidate validation. These are available operation descriptors,
not a result-type-filtered list of proven valid replacements.

The eight borrowed byte/view descriptors retain their existing contents and order; the seven
string descriptors follow them. String parameter types and ownership come from
the owner's resolved signatures, with `type_family: null` and
`evidence_owner: compiler_string_operations`. Byte descriptors keep
`evidence_owner: compiler_byte_operations`. Closed response schemas distinguish
the two inventories. Namespace filtering can remove unavailable descriptors;
absence does not authorize a fallback to an authored function with a similar name.

Each parameter carries its index, name, ownership mode, and either a concrete
`type_id` or a `type_family`. The array parameter describes the fixed byte-array
family; the operation owner's internal `ArrayU8(0)` sentinel must not be
presented as requiring a zero-length array. Actual array length and aggregate
capacity remain checked from the source argument.

The compact hole navigation interface continues to expose constructor choices
and a link to the full context. No new transport authority or publication
route is added. Candidate exploration and failed fills leave canonical source
unchanged; committing source still requires the existing separate authority.

## Limits and evidence

This operation does not add arbitrary intrinsics, external calls or raw source
fragments. Separate [literal constructors](PROJECT-LITERAL-CONSTRUCTORS-V1.md)
can supply string or byte-array values, and the existing field-place extension
authenticates both the root's type and selected field. Record value projection
still stages an authenticated typed value; it cannot be silently substituted
for a borrow of the original owned field.

Library and transport regression cases are authored but unrun. No compiler,
interpreter, backend, generated client, or quality gate was executed for this
change. The graph-operational programme and completion matrix remain partial.

The string extension has separate
[candidate regressions](../tests/project_candidate/string_builtin_calls.rs)
and [protocol regressions](../tests/image_v5/string_builtin_constructors.rs).
String fixtures retained outside the selected entry/test closure establish no
active Project or Wasm String execution support. Existing target profiles,
generated-client response limits, and the serialized Rust client size gate are
unchanged; their executable validation remains required.
