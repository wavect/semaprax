# Project Candidate Contract Expression Holes v1

Status: Partial; implementation and regression evidence authored, unrun.

Audience: agent builders, embedding hosts and compiler contributors.

Agents can select and replace an existing precondition or postcondition subtree
through its checked HIR identity, directly or through an ephemeral typed hole.
Canonical `.spx` remains authoritative. No source text, spans, AST paths,
unchecked HIR or placeholder expressions are accepted as program meaning.

## Discovery and intention

`ProjectCandidate::contract_expression_catalog(target)` returns
`semaprax.project-contract-expression-catalog.v1`. It discovers uniquely authored
`requires` and `ensures` expressions in an explicit monomorphic top-level
function. Each entry binds its actual revision-scoped HIR identity, phase,
expected type, ownership, lexical scope and canonical source provenance.
Unjoinable compiler-generated expressions remain unavailable.

The additive intention has exactly four fields:

```json
{
  "kind": "replace_contract_expression",
  "target": "payments.calculate",
  "expression_id": "<current contract catalogue identity>",
  "replacement": {"kind": "bool", "value": true}
}
```

The example fits a boolean selection; scalar leaves retain their own expected
type. Existing typed constructors resolve calls and names through authenticated
bindings. No imports, effects, capabilities or declarations are added. Function
parameters are in scope; `result` is available only in postconditions. Body-local
bindings are unavailable in either phase. Predicate-local lexical bindings, when
admitted by the compiler, follow the existing expression scope traversal.

The old `expression_catalog`, `replace_expression`, extraction and body-hole
routes retain their body-only behavior. Contract selection uses separate APIs;
it does not turn an old read-only catalogue entry into mutation authority.

## Source replay and constraints

The compiler derives phase, predicate ordinal and child path from the exact
HIR-to-canonical-AST join. Callers cannot supply those structural coordinates.
The constructor replaces only that subtree and preserves required block forms.
Complete Project admission formats and reparses source, rebuilds HIR/graph,
checks identity, export, effect, ownership, loan, cleanup and profile invariants,
and preserves previously admitted native-C11 and structural Core-Wasm lanes.

An additional check independently reconstructs the requested canonical source
from the original revision and compares the complete resulting source inventory.
The new expression must independently rejoin at the same phase and structural
path with exactly the original resolved type and ownership. Predicate counts
and order are unchanged; unrelated predicates, bodies and declarations cannot
change as a side effect of this intention.

Replacing a predicate may change valid inputs or introduce runtime contract
failures. Admission is not logical implication, satisfaction, behavioral
equivalence, external compatibility or runtime execution evidence. The explicit
contract intention permits its selected predicate change; it does not claim to
preserve that predicate's meaning.

Semantic rebase rejects concurrent changes to the selected function's signature,
contracts or effects. An independently changed body or unrelated display name
may be replayed only after the ordinary dependency/conflict checks. Selection
remapping then authenticates the new source origin and type; it is not itself a
merge-safety proof. Direct calls in the containing predicate and replacement
also retain authenticated signature/effect/contract facts across the original
and rebased intermediate revisions. Changed or unavailable dependencies reject.
Candidate test planning treats this as a callable-target change using the
existing static test-root dependency closure; static relevance is not coverage.

## Drafts, context and recovery

```rust
with_contract_expression_hole(
    &self, expected_draft: &str, target: &str,
    expression_id: &str, hole_id: &str,
) -> Result<ProjectCandidateDraft, Vec<Diagnostic>>
```

The existing `hole_context`, `fill_hole`, `complete`, `summary` and recovery APIs
operate on these immutable drafts. Pending rows use `kind: "contract_expression"`
and include the derived phase. Body, body-expression and contract-expression
holes share sixteen entries and globally unique hole IDs. Body and contract
regions may coexist for the same target. Contract siblings must be disjoint by
phase, predicate ordinal and child path; equal or ancestor/descendant selections
reject. Equal paths in separate predicates or phases do not overlap.

Context uses `semaprax.project-candidate-contract-expression-hole-context.v1`.
It includes exact expected type/ownership, checked lexical bindings, selected
source facts and accessible calls. The contract effect budget is empty even if
the enclosing function declares effects; call availability is descriptive and
still requires fill-time validation. Prior contracts, loan plans and cleanup
plans describe the last valid candidate, never the unresolved expression.

Filling constructs `replace_contract_expression` and performs complete admission.
Every surviving selection is remapped to the new checked revision with its
expected type/ownership preserved. Any failure leaves the original draft and
siblings unchanged. `complete` rejects until all three hole inventories are
empty. Drafts never expose incomplete source or publication authority.

[Draft Recovery v1](PROJECT-CANDIDATE-DRAFT-RECOVERY-V1.md) adds the closed row
`{kind:"contract_expression",hole_id,target,expression_id}`. Phase and paths are
rederived from source, not imported. Restore replays the valid history, recreates
all holes through ordinary APIs and checks exact draft/capsule identity. Legacy
body/expression-only report and capsule bytes remain unchanged.

## Protocol, limits and evidence

V5 candidate-enabled sessions add `candidate/contract-expression-catalog` and
`hole/open-contract-expression`. The former returns a bounded structured catalogue;
the latter takes current image/candidate bindings, target, expression ID, hole
ID and an optional current draft binding. Existing query/fill/complete/discard
and recovery methods operate on the resulting handle. Live input authentication
and registry/response capacity checks still precede installing any draft.

V1–v4 method sets stay unchanged. V5 discovery and generated TypeScript, Python
and Rust helpers describe the new requests; heterogeneous catalogue/context
interiors remain explicitly unbundled. Neither method grants build, test,
filesystem or publication authority, or participates in parallel image reads.

The existing expression bounds remain: 4,096 expressions, depth 256, 16,384
cumulative scope facts and 1 MiB catalogue/context output. Draft selectors use
the existing 4,096-byte identity limit; direct expression intentions retain
their existing expression-ID bound. History, source, target, recovery and
constructor limits remain active. Invalid selectors/type preservation use
`SPX-G225`, expression capacity `SPX-G226`, draft grammar/overlap `SPX-G230`,
draft capacity `SPX-G231` and stale/unresolved drafts `SPX-G232`. Compiler and
rebase diagnostics propagate from their existing owners.

Authored, unrun evidence lives in
`tests/project_candidate_contract_holes_v1.rs` and
`tests/image_contract_holes_transport_v5.rs`. No compiler, interpreter, tests,
application executable or long local gate was run for this batch. Recursive
incomplete declarations, general contract inference/proof and executed evidence
remain outstanding; no completion-matrix row is promoted.
