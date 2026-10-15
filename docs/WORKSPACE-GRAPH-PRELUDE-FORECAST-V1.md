# Workspace Graph Prelude Forecast Accounting v1

Audience: compiler contributors, tool authors, and reviewers.

Status: source-ready accounting correction; executable qualification pending.

This additive private forecast revision supersedes the fitting-receipt
preservation clauses in Workspace Semantic Graph v1 only for the missing
unconditional compiler-owned Option/Result retained-declaration term. It applies
to all private forecast modes zero through seven. Public graph schemas, source
syntax, identities, allocation authority, the 64 MiB builder ceiling, replay
work ceilings, and physical limits remain unchanged. Historical receipts remain
complete prior-model evidence, not corrected current-model receipts.

## Owning input and formula

`prelude::declarations_for_program` always selects the ordered Option/Result
prefix. Both `DeclarationIndex` construction and the resolver's returned type
chain retain it, including with no source use. Neither filters that prefix by
an authored name. Empty, Option-only, Result-only and both-selected controls
exercise the borrowed schema seam, not four newly admitted production modes.
Parser-only overlapping-name controls do not change source verification rules.

`prelude/base_schema.rs` owns immutable metadata used by the ordinary Option and
Result constructors. Estimation borrows this metadata; it neither initializes
the process OnceLock table nor allocates a duplicate AST. Metadata contains
exact stable IDs, names, parameter names, case order, field order, and each
field's named parameter type. Complete original constructors independently
check declaration equality and physical requested backing through Drop.

Let `C` be the existing uncollapsed declaration source-input cost: each
TypeDeclaration, TypeParameterDeclaration, VariantCaseDeclaration,
FieldDeclaration and field Type uses its actual Rust `size_of`; each string uses
the existing String header plus exact byte length. This is exactly the existing
`ast_type_declaration_cost` with `StructuralCost::new`, checked independently
against held declarations. Let `S=C.string_bytes`, and let `N` count the named
field-type identity occurrences using the existing identity-slot rules. Let
`M` be the maximum of the dependency identity bound and these selected base
declaration/case/field stable-ID lengths. The missing retained term is:

```text
P = (C.total - S) * HIR_STRUCTURE_EXPANSION_FACTOR
  + S * HIR_STRING_COPY_FACTOR
  + N * M * HIR_IDENTITY_COPY_FACTOR
```

All factors are imported from the existing owning forecast (currently 24, 64,
and 16); this revision introduces no independent multiplier, observed-gap
allowance, or cap. Existing compile-time declaration bundle assertions support
the structural factor; ordinary index, resolved-HIR and transient map/string
retention remains conservatively covered by those same expansion rules. The
term is a conservative bound, not an exact allocator measurement. Each sum,
product and final term is checked under the caller's active builder limit;
refusal retains the ordinary complete SPX-G171 diagnostic contract.

## No double counting or discount substitution

The pre-correction raw census traverses only authored `program.types`. Imported
declarations and private-layout/default/main materialization do not append these
compiler-owned base declarations to synthetic AST. `prelude::all_ids` supplies
maximum identity length only; it supplies neither an occurrence count nor a
constructor-shaped cost. There is therefore no selected-base source-input term
in the original raw, runtime or generic-instance cost.

`P` is added once to `retained_hir` after the ordinary fixed expression discount.
It flows once into retained clone/HIR and total clone/HIR bounds. Synthetic AST,
runtime, transient imported clone and authored expression discount fields remain
unchanged. A complete old forecaster independently verifies precisely this
field-by-field delta for every mode and both parsed/semantic retention profiles.
The correction does not reinterpret an expression discount or count a prelude
AST clone. Extended selected Vec/Box/iterator/list/stream/collection accounting
is outside this correction's sufficiency claim.

## Qualification

The complete pre-correction forecaster, source archives, pinned source hashes,
and exact byte inverses preserve historical evidence without normalizing old
results. Existing numeric receipt pins are not guessed or edited in this
source-only lane: an authorized actual producer run must regenerate any moved
current-model pins while retaining old receipts as historical controls.

Required gates include metadata zero-allocation input parity, complete original
producer parity through physical Drop, complete old-forecaster delta, exact,
one-short and zero term/whole-forecast boundaries, live first-sticky refusal
preservation, source inverse/inventory gates, and the unchanged seven-fixture
carrier resolver/replay highwater gate. Existing application controls and full
quality gates remain required. Until those execute on the corrected source,
no current-source fit, exclusive causal attribution, allocation savings, hosted
support or completion status is claimed.
