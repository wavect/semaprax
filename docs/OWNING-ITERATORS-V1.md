# Owning Iterators v1

Status: focused cross-engine runtime, projection, and ProgramRoot replay corpora
pass locally; exact-head hosted promotion remains pending.

Audience: language users, compiler contributors, backend implementers, and
workspace-service authors.

This LANG-07 profile introduces a first-class consuming iterator protocol over
bounded scalar vectors. It is the ownership foundation for subsequent iterator
loops and adapters, not an eager vector transformation.
Owning `Bytes` payload traversal is the additive
[Owning Iterator Payloads v2](OWNING-ITERATOR-PAYLOADS-V2.md) profile.

## Checked types and operations

`Iter<T>` has compiler identity `core.iter` and owns one existing `Vec<T>` plus
a `usize` cursor. `T` is one of the eight admitted Copy scalars. The iterator
is non-Copy even though its yielded items are Copy. Its representation is
opaque to source construction and field projection.

`IterStep<T>` has compiler identity `core.iter-step` and these ordered cases:

| Tag | Case identity | Fields in canonical order |
| --- | --- | --- |
| 0 | `core.iter-step.done` | None |
| 1 | `core.iter-step.yield` | `item: T`, `rest: Iter<T>` |

The field identities are `core.iter-step.yield.item` and
`core.iter-step.yield.rest`. A consuming match on `Yield` transfers its sole
iterator owner into the `rest` binding. `Done` carries no owner. The checked
tag determines which payload is live before any payload authority is granted.
Private functions can consume and return the exact `IterStep<T>` type, including
reconstructing `Done` or `Yield` through an owning match. The item is copied and
the rest is transferred; reconstruction never copies an iterator owner.
A local or directly matched `Done` constructor selects the same iterator
prelude and graph without requiring an iterator intrinsic call. Cleanup
construction and independent replay retain the closed guarded case domain;
only the constructed case initializes runtime ownership flags.

| Source operation | Stable identity | Signature |
| --- | --- | --- |
| `vec_into_iter<T>` | `core.vec.into-iter` | `(own Vec<T>) -> Iter<T>` |
| `iter_next<T>` | `core.iter.next` | `(own Iter<T>) -> IterStep<T>` |

Construction transfers the vector's existing allocation without cloning its
elements or allocating iterator backing storage. The cursor starts at zero.
Each `next` consumes its input once. If the cursor is below the vector length,
it reads that item, advances the cursor once, and publishes `Yield` with the
successor iterator. Otherwise it settles the vector once and publishes `Done`.
An empty vector produces `Done` on the first call. No source handle can name
the prior iterator generation after a successful transfer.

Discarding an unfinished iterator or an unmatched `Yield` settles the same
underlying vector exactly once. Arguments stage left to right. Failures retain
the selected status and leave every staged or committed owner governed by the
canonical call boundary; failed operations do not publish a result. `iter_next`
defers its owner commit until its item read succeeds. On read failure the
staged caller owner remains live for canonical cleanup.

## Independent semantic projections

Prelude v7 binds the reserved types, cases, fields, operations, ownership modes,
and element restrictions. Earlier prelude contracts remain byte-identical.
Source and HIR validate those identities before interpretation or lowering.

CleanupPlan v10 adds the iterator lifecycle `core.iter.drop` and the conditional
`Yield/rest` owner path. The earlier Bytes-only variant profile is not widened
by reinterpreting its schema. Inventory construction and independent cleanup
replay derive the same new profile from retained checked types. Plans remain
canonical runtime order; backends cannot infer ownership from carrier layout.
v10 is selected only when retained inventory contains the iterator lifecycle or
the authenticated conditional `core.iter-step.yield.rest` owner path; it remains
additive over v2-v9 and a supplied v9 plan cannot stand in for that meaning.
The existing Inventory v2 slot and structural projection paths remain unchanged;
v10 adds lifecycle meaning and does not reinterpret those retained paths.

Graph v38 and ProgramRoot retain exact iterator type arguments, consuming call
edges, selected cleanup schema, conditional owner paths, and Prelude v7 binding.
Externally supplied graph, layout, cache, or cleanup bytes confer no authority.

Native storage contains the existing vector carrier and cursor (`Iter`: 48 bytes,
`IterStep`: 64 bytes, both aligned to 8). The step reserves an eight-byte scalar
item slot. Core Wasm uses 16-byte iterators and 32-byte steps, aligned to 8, and
keeps the existing vector handle and cursor in compiler-owned frame storage;
it reuses explicit vector host operations. Neither lane gains a new ambient
allocator, transport, or host capability. Public Project, C, C++, Rust, WIT,
Component, and package descriptors remain independently closed.

## Evidence required

The executable corpus must cover every scalar type, empty/exhausted iteration,
ordered yields, dropping before exhaustion, consuming matches, private helper
composition, result/argument failure, and repeated exact settlement on the
interpreter, C11 O0/O2, and Core Wasm. Hostility includes forged type arguments,
case/tag identities, omitted or duplicated conditional owners, stale source
binding, changed cleanup schema, reordered transfers, and shallow owner copies.

Owning payloads, generic authored iterator implementations, associated types,
lazy closure adapters, public iterator ABI, and hosted promotion remain
separate work within the full language goal. Consuming loop syntax is specified
and implemented separately by [Owning Iterator Loops v1](OWNING-ITERATOR-LOOPS-V1.md);
that profile does not widen this protocol's frozen v1 contract. This initial
protocol must not be described as completion of that broader iterator goal.
