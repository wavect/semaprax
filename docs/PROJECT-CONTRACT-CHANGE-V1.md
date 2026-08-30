# Project Contract Change v1

Audience: agent builders, compiler contributors, and reviewers.

Status: authored, unrun. The full graph-operational programme remains Partial.

The additive `add_contract` intention appends exactly one typed predicate to
an explicit, monomorphic, top-level non-main function in an immutable
[Project Candidate](PROJECT-CANDIDATES-V1.md). It grants no source publication
or execution authority and introduces no new source-language syntax.

## Intention and context

The object has exactly `kind`, `target`, `phase`, and `predicate`:

```json
{
  "kind": "add_contract",
  "target": "calculator.divide",
  "phase": "requires",
  "predicate": {
    "kind": "binary",
    "op": ">=",
    "left": {"kind": "place", "name": "left"},
    "right": {"kind": "i64", "value": 0}
  }
}
```

`phase` is `requires` or `ensures`. Predicates use the existing bounded typed
expression constructors. Function parameters are in scope; the compiler's
`result` binding is additionally available only in `ensures`. Local body
bindings are unavailable. Calls must resolve through existing local/import
stable-ID bindings; this operation cannot add imports, effects or capabilities.

The ordinary verifier decides boolean typing, contract purity, allowed call
semantics, ownership, loans and cleanup. A syntactically valid JSON object is
not a validation receipt. Adding a precondition may restrict valid inputs;
adding a postcondition may introduce runtime contract failure. Full candidate
admission does not prove all executions satisfy the new predicate or establish
external API compatibility.

## Preservation and replay

The transformation retains every existing predicate in its original order
and appends one new predicate to the selected phase. Candidate invariant
comparison permits exactly the corresponding count increment for that target;
all other contract inventories, declared effects and module permits remain
unchanged. The existing explicit identity and manifest/export guards still
apply. Neither removing nor replacing an old predicate is supported here.

A separate [Contract Expression Change](PROJECT-CANDIDATE-CONTRACT-HOLES-V1.md)
intention can replace a selected authored subtree in an existing predicate,
with exact source reconstruction and expected-type/ownership checks. It does
not change the append-only behavior of `add_contract`.

Canonical source is generated in memory and reparsed. Complete Project
rebuilding links callers and revalidates contracts, ownership, cleanup and
the selected profile. Independent source replay must reproduce the exact
Project revision and graph. Previously admitted native C11 and structurally
validated Core-Wasm projection lanes must remain admitted. No target or test
execution occurs during candidate admission.

Changes, graph digests, structural impact and source diffs use the existing
candidate evidence envelope. The exact typed predicate remains in the
ordered change history for independent replay. Source files and sibling
candidates stay unchanged on success, stale input or failure.

## Bounds and evidence

The function's combined pre/postcondition count must be below 1024 before an
addition. Existing change bytes, expression depth/node, 32-intention history,
canonical source, Project and target bounds remain in force. Bad phases,
unknown fields or illegal constructor scope reject with `SPX-G225`; contract
inventory capacity rejects with `SPX-G226`. Type/purity/ownership failures use
the ordinary compiler diagnostic. Exact candidate and Project digest checks
continue to reject stale changes before transformation.

Authored, unrun cases in `tests/project_candidates_v1.rs` cover ordered
preservation of an existing precondition, additive postconditions using
`result`, exact replay, no writes, invalid phase/scope, and non-boolean
rejection. These cases have not been executed; no completion gate is promoted.
