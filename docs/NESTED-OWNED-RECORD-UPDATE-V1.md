# Acyclic nested owned-record immutable update v1

Status: internal implementation tranche; promotion evidence is not yet claimed.

Audience: language, HIR, ownership, cleanup, interpreter, native, Wasm, and
evidence maintainers.

## Purpose and closed admission

This contract extends the bounded [Acyclic Nested Owned-Byte Records
v1](NESTED-OWNED-BYTE-RECORDS-V1.md) profile with immutable top-level record
reconstruction:

```semaprax
value with { marker: 7, leaf: replacement }
```

For a nested non-Copy record, `value` must be one exact unprojected named
`own` place. The record remains monomorphic, sized, acyclic, resource-free,
and composed only of checked Copy scalars and transitive owned `Bytes`. The
inherited limits remain exact: record depth **64**, transitive owned `Bytes`
leaves **256**, and fields examined **4,096**.

An authored replacement names one direct field of the outer record. It may
replace a checked Copy scalar, one direct `Bytes` leaf, or one whole nested
record subtree of exactly the declared type. Replacement paths are never
implicit: dotted-path update, in-place mutation, and partial nested
reconstruction remain closed. Variants, generics, `String`, resources,
classes, arrays, slices, recursive types, contracts, loops, public aggregate
ABIs, FFI, tasks, and mutable or escaping loans remain closed.

## Evaluation and ownership

The base is evaluated exactly once before any replacement. Replacement
expressions are evaluated exactly once from left to right in authored order.
Every field identity is resolved to its persistent declaration ID; display
names and target offsets never identify ownership.

A successful reconstruction has one atomic ownership outcome:

- every unchanged owned descendant transfers from the base to the result
  exactly once;
- every replaced old owned descendant is settled exactly once;
- every completed owned replacement transfers into the result exactly once;
- the source becomes unavailable only at the authenticated commit boundary;
  and
- no shallow aggregate copy, `memcpy`, or Wasm `memory.copy` may duplicate an
  owner.

Before commit, failure retains or settles exactly the portions that remain
owned according to the canonical plan. During replacement evaluation, only
the actually completed replacement prefix can require cleanup, in reverse
actual-completion order. After commit, the source cannot be reacquired.
Failure selection is sticky, non-result cleanup precedes result publication,
and cleanup cannot replace the selected status.

An active loan of the base, a replaced subtree, or any overlapping descendant
blocks reconstruction. Disjoint paths do not grant permission to reconstruct
their ancestor. This tranche adds no runtime loan object or authority.

## Cleanup and Graph proof

An admitted nested owned-record reconstruction selects additive
`semaprax.cleanup-plan.v9`. Canonical construction and independent replay
derive and compare:

- the exact base, result, record, and top-level replacement identities;
- authored replacement evaluation order and declaration-order descendant
  inventories;
- unchanged descendant transfers;
- replaced-old-subtree settlement;
- completed replacement initialization and transfer;
- preflight liveness/capacity checks and one commit boundary; and
- reverse actual-completion cleanup, sticky failure, postconditions, and
  result publication.

Programs requiring this profile select additive `semaprax.graph.v30`, or
`semaprax.graph.v31` when composed with universally authenticated nested
projected shared loans. These versions include the preceding nested-record and
exact-destructuring facts without reinterpreting CleanupPlan v2-v8 or Graph
v1-v29. Mixed or unauthenticated evidence fails closed.

Plans and graphs are proof data only. They grant no source, filesystem,
process, network, build, publication, ABI, or finalizer authority.

## Runtime lowering

The interpreter validates the complete recursive carrier and replacement
inventory before mutation. Native C11 uses checked fieldwise recursive moves
and distinct liveness flags. Core Wasm checks all pointer, offset, range,
owner-capacity, and liveness requirements before consuming the base. All three
backends consume only validated HIR plus independently replayed CleanupPlan v9
and must expose equivalent result, status, ownership, and cleanup traces.

## Evidence gate

Promotion requires one exact revision to pass:

- canonical source round trips and persistent base/record/field identities;
- exact and plus-one depth, owned-leaf, field-work, owner-capacity, and output
  bounds;
- Copy, direct-`Bytes`, whole-subtree, reordered, empty, and multiple
  top-level replacement cases, with an explicit decision for each;
- base failure and failure before, during, and after every replacement and
  commit boundary;
- active-loan overlap and source-reuse rejection;
- hostile HIR and CleanupPlan mutations of base, result, record, field,
  subtree, type, ordering, liveness, epoch, transition, and publication data;
- repeated interpreter, native `-O0`/`-O2`, sanitizer, and Node/Core-Wasm
  equivalence with no owner growth;
- deterministic double-build and Graph/cache/workspace replay; and
- byte-frozen CleanupPlan v2-v8, Graph v1-v29, flat update, Project, ABI, and
  generated-consumer fixtures.

Until that corpus passes, affected completion-matrix rows remain Partial.
Focused local execution, one backend, serialized proof data, or implementation
presence alone does not establish promotion or production support.

## Nonclaims

This contract does not provide nested-path update, mutation, general
aggregate expressions, nested variants, owned match-arm results, resources,
general generics, mutable borrowing, closures/tasks, a stable layout, a public
aggregate ABI, Component publication, concurrency, or production support by
itself.
