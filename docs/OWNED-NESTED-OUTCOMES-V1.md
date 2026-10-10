# Owned Nested Outcomes v1

This is the compiler prerequisite tracked by #729 for finite application JSON
construction in #724. Source implementation is staged; the executable gates
below have not yet qualified it. No token, cost, or acceptance advantage is
claimed by this specification.

## Authenticated shape

An ordinary authored monomorphic, invariant-free variant has exactly two cases,
in either declaration order. Every variant, case, field, and transitively used
record declaration has an explicit stable identity. One case has exactly one
field whose type is an acyclic owning record. The other case has exactly three
fields with types `i64`, `usize`, `i64`, in that order. Names carry no authority.
The scalar error case contains no default record and creates no payload owners.

The record closure admits the existing eight Copy scalars, `string`, `Bytes`,
further monomorphic invariant-free records, and the exact existing bounded Vec
carriers admitted by Owned Collection Records v1. No Vec element shape or
capacity changes. String/Bytes runtime caps remain unchanged. Maps, resources,
classes, generic records, nested variants, optional fields, and recursive trees
are outside this tranche. At least one owned leaf must be reachable from the
success record.

Bounds cover the complete variant: depth at most 64, expanded visited fields at
most 4096, and at most 256 owned leaves. The variant occupies the first depth;
the success record starts at depth two. The four case fields count toward the
field budget. A Vec carrier counts as one owner; its existing independent
capacity and content bounds still apply. Repeated record occurrences count
separately. No generated-name, cache, or graph marker grants admission.

## Ownership and checked behavior

Ordinary source constructors, `own` arguments/results, borrowed inspection, and
owning/borrowed matches carry this shape. Argument and field evaluation stay
left to right. A success payload transfers only after its construction succeeds;
partial record owners settle through the canonical cleanup plan. Owned calls
stage all arguments and commit them together. Later argument failure leaves
staged owners with the caller. Callee/postcondition failure cannot publish an
owned result and cannot replace the first selected status during cleanup.

The cleanup inventory and independent cleanup plan retain the complete
`case / payload field / record field ... / owned leaf` identity path. Borrowed
payload bindings alias the active case's storage and cannot move or clone its
owners. Their existing record projection loans retain the payload binding's
lifetime. Moves before those loans end remain compile-time errors.

Native and Wasm layouts use the ordinary target-specific AggregateLayout for the
success record, not a Vec-width placeholder. Native moves the inert Copy shell
and commits each physical owner through the existing canonical leaf slots.
Wasm authenticates the tag before selecting the payload, recursively moves the
active record, and clears the moved storage. Interpreter values use the same
independently authenticated type closure. Inactive payload bytes grant no owner.

## Explicit command profile and replay

The additive private command profile is
`language-command-io.nested-outcome.v1` (Project schema v32). It retains all v31
private helpers and adds the authenticated owning outcome and its record helper
signatures. Selected entry/command roots remain `fn() -> i64`; process effects,
module permits, manifest grants, provider quotas, and public ABI stay unchanged.
Older profiles reject the new carrier, including unused functions before source
closure cropping and body-local constructions behind scalar signatures. Direct
native adapters perform the same negative and independent positive validation.

Graph v74 projects `semaprax.owned-nested-outcomes.v1` with `authority:false`.
Source revision, case/field ordering, explicit identity provenance, exact types,
cleanup, and loans are independently replayed after cache decode. Older graph
and evidence routes cannot mask this behavior. The cache gains no new authority
or opaque executable type: it stores ordinary existing record/variant HIR.

## Owning gates

- `--lib hir::collection_outcome::nested::tests::` (6): source/canonical/graph
  round-trip, exact source drift, cache replay, provenance and case/type forgery,
  full cleanup paths, target-sized record payload, frozen profiles/unused
  helpers, variant-inclusive depth and owned-leaf boundaries.
- `--test owned_data owned_nested_outcome::` (3): identical source on interpreter,
  C11 at O0/O2 with allocation and vector-authority settlement, and strict Wasm;
  success/error, borrow/forward/move, partial construction, grouped argument
  failure, callee/postcondition failure, and no allocation for an error-only
  outcome. Repetition must preserve status and settlement.
- Project v32 routing must separately qualify real selected commands, exact
  grants, source/retained replay, and old-profile refusal before publication.

Full arbitrary nested JSON, optional/nullable schemas, user-defined variants,
and new collection element layouts remain separate obligations of #724.
