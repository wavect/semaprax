# Acyclic nested owned-record exact destructuring v1

Status: internal implementation tranche; promotion evidence is not yet claimed.

Audience: language, HIR, loan, cleanup, interpreter, native, Wasm, and evidence
maintainers.

## Purpose and closed admission

This contract extends the bounded [Acyclic Nested Owned-Byte Records
v1](NESTED-OWNED-BYTE-RECORDS-V1.md) profile with exact recursive
destructuring. It admits one irrefutable record pattern under explicit
`match own` or `match borrow` only when the scrutinee is an exact named place
whose resolved type already satisfies that profile.

The inherited limits remain exact: record depth **64**, transitive owned
`Bytes` leaves **256**, and fields examined **4,096**. Source verification and
HIR validation independently use bounded iterative worklists and resolve every
pattern node to persistent record and field declaration IDs. Display names,
source offsets, target offsets, and generated layout never identify ownership.

Variants, generics, `String`, resources, classes, arrays, stored slices,
recursive types, update/mutation, guards, alternative patterns, constructor or
call temporaries, escaping or mutable loans, task/closure capture, FFI, Project
exports, Components, and public aggregate ABIs remain closed. Arm results are
Copy-only in this tranche.

## Exact recursive pattern

Every record node must name the exact resolved record declaration and list
every declared field exactly once. Reordering authored fields is permitted,
but validation and cleanup use declaration order. A nested record field must
use a nested record pattern. A direct owned `Bytes` leaf may never be hidden by
a wildcard.

For `match own`:

- every transitive owned `Bytes` leaf is bound exactly once;
- Copy leaves may be bound or wildcarded;
- no omitted, duplicate, foreign, wildcard-concealed, or residual owned
  subtree is accepted; and
- the whole scrutinee commits as one ownership transition. A verifier or
  backend may not expose a partially moved source.

For `match borrow`:

- bindings are synchronous immutable aliases scoped to the arm;
- no owner, liveness flag, cleanup slot, or transfer is created;
- the root remains owned and usable after the arm; and
- nested `bytes_as_slice`, range, and direct reborrow operations retain the
  complete root plus stable field-ID path.

An active borrowed descendant conflicts with movement, assignment, owned
matching, or transfer of any ancestor or overlapping descendant. Disjoint
siblings remain independent. Last-use release is path-exact.

## Cleanup and proof schemas

Functions containing an admitted recursive ownership-aware record pattern
select additive `semaprax.cleanup-plan.v8`. The plan retains the existing slot,
place, transition, status, and finalizer vocabulary while authenticating:

- the exact recursive pattern inventory and ownership mode;
- complete stable field-ID paths for every binding and leaf;
- one atomic whole-source transfer for `match own`;
- no transfer or new cleanup action for `match borrow`;
- declaration-order transfer and reverse actual-completion cleanup; and
- sticky failure selection and ordinary result-publication ordering.

Canonical construction and independent replay derive the pattern tree and
paths separately from validated HIR. They compare all byte-significant fields
and reject missing, duplicate, reordered, foreign, reparented, wrong-mode,
wrong-epoch, or partially committed evidence. Downstream code must not sort,
repair, or reinterpret the plan.

Programs requiring this destructuring select additive `semaprax.graph.v28`.
When the same validated function also contains an authenticated nested
projected shared loan, it selects `semaprax.graph.v29`. V28/V29 compose all
facts explicitly owned by the preceding nested-record schemas; they do not
reinterpret CleanupPlan v2-v7, Graph v1-v27, or any Project/public artifact.
Native-import or otherwise unrepresentable schema combinations fail closed.

Plans and graphs remain proof data. They grant no execution, source,
filesystem, process, publication, or ABI authority.

## Runtime lowering

The interpreter, native C11 backend, and Core Wasm backend consume only
validated HIR and independently replayed CleanupPlan v8.

`match own` transfers every live descendant carrier in recursive declaration
order and clears all source flags at the single commit boundary. No owning
clone, shallow aggregate assignment, `memcpy`, or Wasm `memory.copy` is
permitted. A failure before commit retains the complete source; a failure after
commit never reacquires it. Arm failure settles transferred bindings exactly
once in reverse canonical order without replacing the selected status.

`match borrow` aliases existing storage, creates no runtime owner, and must
settle every arm loan before the root can move or finalize. Invalid paths,
liveness disagreements, impossible active aliases, or schema mismatches fail
before payload access, cleanup, or publication.

## Evidence gate

Promotion requires one exact revision to pass:

- canonical source round trips and stable nested record/field identities;
- exact and plus-one depth, leaf, and field-work limits;
- success for reordered authored fields, deep owned bindings, Copy wildcards,
  nested borrowed views/ranges/reborrows, sibling independence, last-use
  release, branch, loop, and repeated entry;
- stable diagnostics for every excluded shape, omitted/duplicate/foreign
  field, wildcarded owned subtree, wrong mode/root, and non-Copy arm result;
- hostile HIR mutations of record, field, binding, type, ownership, root,
  loan parent, and last-use identities;
- hostile CleanupPlan mutations of every path, transfer, epoch, commit,
  finalizer, status source, and publication boundary;
- failure injection before/after the whole-source commit and during arm
  evaluation, with sticky status and exact reverse cleanup;
- identical normalized interpreter, native `-O0`/`-O2`, and Node/Core-Wasm
  behavior and cleanup observations;
- native sanitizer and tight Wasm owner-capacity exact/plus-one evidence; and
- byte-frozen CleanupPlan v2-v7, Graph v1-v27, Project v1-v10, package, ABI,
  and generated-consumer fixtures.

Until that complete corpus passes, every affected completion-matrix row remains
Partial. Compilation, source-layout checks, one backend, or serialized proof
data alone do not satisfy this gate.

## Nonclaims

This contract does not provide nested variants, owned arm results, aggregate
mutation, general pattern exhaustiveness, general lifetime inference, mutable
or escaping borrows, closures/tasks, public borrowed calls, stable layout,
foreign ownership, a new Project profile, a C/Rust/WIT/Component ABI,
concurrency, or production support by itself.
