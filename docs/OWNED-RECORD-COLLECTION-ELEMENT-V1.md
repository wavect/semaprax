# Owned Record Collection Element v1

Audience: compiler contributors and reviewers of SPX-AI-019/020 (issues #118,
#119).

Status: **admission classifier only**. Source and HIR can independently
recognize the one admitted record shape this document defines
(`src/hir/owned_record_collection.rs`,
`src/source_verify/declared_type/owned_record_collection.rs`), with local unit
evidence. No compiler intrinsic accepts this element type yet: `Vec<T>`/`Box<T>`
construction, push, length, capacity, or clear over this shape are **not**
wired to source, HIR, cleanup-plan, or any backend. A program cannot construct
a value of this element's collection type today. This document exists so the
follow-up execution tranche (SPX-AI-020, issue #119) has one audited admission
rule and one recorded backend hazard to design around, rather than
rediscovering both.

## Purpose and non-goals

[SPX-AI-018](CATALOG-NORMALIZER-ORACLE-V1.md) freezes a catalog-normalizer
record shape — `id: Bytes`, `label: Bytes`, `quantity` a bounded nonnegative
integer — as the acceptance application's per-record payload. That document's
own routing note states the application itself does not require a growable
collection: batches are validated and re-emitted from bounded, re-scanned
input, not accumulated into an owned `Vec` of records. This profile is
therefore not on SPX-AI-025's (issue #124) critical path by that document's own
design; it exists because [SPX-AI-020](https://github.com/wavect/semaprax/issues/119)
("Execute owned-record collections on interpreter, C11 and Core Wasm") is a
separate roadmap step that depends on #118 regardless, and because a bounded
internal owned-record-in-a-collection profile is useful general language
composition beyond this one application.

This is **not** a public generic ABI, not a change to
[GENERIC-COMPILER-COLLECTIONS-V1](GENERIC-COMPILER-COLLECTIONS-V1.md)'s frozen
scalar type-parameter profile, and not a change to
[PUBLIC-GENERIC-BOUNDARY-PROFILE-V1](PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md).
It admits exactly one concrete, non-generic, internal record shape as a
collection element; nothing here is reachable from a public signature, and
nothing here widens `Vec<Bytes>`/`Box<Bytes>` (v2) or `Vec<T>`/`Box<T>` scalar
(v1) admission.

## Admitted shape

A declaration is an admitted owned-record collection element iff, after
resolution:

- it is an explicitly authored `record` (never a `class`, `variant`, or
  `resource`, and never a compiler-owned or automatically synthesized
  nominal);
- it has zero type parameters (monomorphic; this profile does not compose
  with [Concrete Generic Owned-Byte Records v1](CONCRETE-GENERIC-OWNED-BYTE-RECORDS-V1.md)
  or any other generic-template admission);
- it declares **exactly three fields**: exactly two of type `Bytes` and
  exactly one of an admitted Copy scalar (`i64`, `i32`, `u8`, `usize`, `char`,
  `f32`, `f64`, or `bool`); and
- it has no other field, in any order — field *names* and declaration order
  are not part of the rule, only the resolved field type multiset.

This matches the exact catalog-normalizer record structurally (two owned
`Bytes` leaves plus one Copy scalar) without hardcoding its field names, so
the classifier is exercised across all eight Copy-scalar substitutions the
way every other owned-record profile in this codebase is
(`docs/GENERIC-MULTI-OWNER-RECORDS-V1.md`, `docs/CONCRETE-GENERIC-OWNED-BYTE-RECORDS-V1.md`).
Any other authored declaration — a fourth field, a `String` field, a nested
record/class/variant/resource field, a generic record instantiated to the
same field shape, or a `class`/`variant`/`resource` sharing the field-name
convention — is refused. This is the AC-1 requirement ("the exact selected
payload is admitted; structurally similar foreign nominal types and
unsupported nested/resource payloads are refused") re-derived independently
in both source (AST declared-type facts) and HIR (`DeclarationIndex` facts),
mirroring the existing dual-validator pattern used by every other admission
rule in this codebase (for example
`type_reachability::is_admitted_concrete_owned_byte_variant`).

Both classifiers are pure, `DeclarationIndex`/`TypeTable`-derived predicates,
not cached flags: they recompute admission from resolved declaration facts on
every call, so a hostile or forged intermediate representation cannot forge
admission by carrying a stale bit.

Reference implementation and unit evidence:

- `src/hir/owned_record_collection::is_admitted_owned_record_collection_element`
  (resolved/HIR side, keyed on `DeclarationIndex`), with 12 unit cases:
  admission across all eight Copy-scalar substitutions, field-order
  independence, an extra field, a missing field, a wrong field type in either
  slot, two Copy fields with only one `Bytes` field, a nested record field, a
  `resource`, a `class`, a generic record instantiated to the exact field
  shape, a `variant` sharing the same field-name convention, and non-nominal
  types.
- `src/source_verify/declared_type/owned_record_collection::is_admitted_owned_record_collection_element`
  (AST side, keyed on `TypeTable`), with 5 unit cases covering the same
  admit/refuse boundary before resolution.

Both are `pub(crate)`/`pub(in crate::source_verify)` and intentionally
`#[cfg_attr(not(test), allow(dead_code))]`: no call site outside their own
tests exists yet (see "What remains" below). They exist as one audited home
for this exact rule so the execution tranche does not reimplement it inline
at each future call site, matching the precedent set by
`src/host_ownership.rs` for a reference model staged ahead of its call sites.

## Cleanup rule (explicit)

An admitted record's cleanup **as a standalone value** is already implemented
and unaffected by this document: it is an ordinary flat owned record under
[Owned Byte Record Algebra v1](OWNED-BYTE-RECORD-ALGEBRA-V1.md) ("flat
monomorphic records with one or more direct `Bytes` fields and zero or more
direct Copy-scalar fields"), which this profile's exact shape already
satisfies. `match own`/`match borrow`, construction, and lexical drop for a
plain `record Item { id: Bytes, label: Bytes, quantity: i64 }` outside any
collection are governed entirely by that existing contract; this document
adds nothing there and step 1 of the SPX-AI-019 implementation plan ("check
whether the current branch already admits the selected record") is answered
**yes** for the standalone record.

The **gap** is placing that record inside a compiler-owned `Vec`/`Box`
carrier. The existing cleanup-plan treatment of any `Vec<T>`/`Box<T>` value is
already element-type-generic at the plan level:
`cleanup_plan::build::bounded_vec::bounded_vec_shape` (and the parallel
`bounded_box` builder) assigns the *whole* vector or box exactly **one**
opaque non-Copy leaf with the shared `core.vec`/`core.box` drop lifecycle,
regardless of what `T` is; it never inspects the element's own field
structure. Concretely: cleanup inventory order for the record's *own* fields
(when constructed or matched standalone) remains the record's authored field
order per AGENTS.md ("cleanup inventory order is structural metadata");
cleanup for the *collection* itself remains exactly one leaf per collection
value, exactly as it is today for `Vec<Bytes>`/`Box<Bytes>`. Extending this
profile to a multi-field record element requires **no change** to
`cleanup_plan::build::bounded_vec`/`bounded_box`'s own shape logic — the
opaque single-leaf treatment already generalizes.

What *does* need to change, and is **not done here**: both
`cleanup::is_owned_bounded_vec_type` and `box_ops::is_type` gate that opaque
recognition on `crate::vec_ops::resolved_vec_element_is_admitted`/
`crate::box_ops::resolved_box_element_is_admitted` — the same scalar-or-Bytes
predicate that native and Wasm backends also consult (see "Backend hazard"
below). Widening those two predicates is exactly the unsafe step this
document explains how to avoid.

## Borrow rule (explicit)

No new loan-plan mechanism is needed. `src/loan_plan.rs` is already fully
element-type generic: its `LoanCause` variants (`SliceView`, `StrView`,
`BorrowedCall`, `MatchBorrow`) know nothing about `Vec`/`Bytes`/scalars
specifically. A `borrow`-mode call parameter — which is exactly how
`vec_len`/`vec_capacity` already borrow their vector argument today,
regardless of element type — always opens a `BorrowedCall` loan on its origin
place and closes it at the ordinary loan-plan boundary; a later `own`-mode
call on the *same* place (staging it as a consuming argument, as
`vec_push`/`vec_reserve_exact`/`vec_set`/`vec_clear` already do) is rejected
while that loan is live, by the same general mechanism that already rejects
it for `Vec<Bytes>` and `Vec<i64>` today. Nothing about this record element
requires a new `LoanCause`, a new liveness rule, or any change to
`src/loan_plan.rs`. This satisfies AC-3 ("a borrow live across reallocating or
consuming mutation is rejected before lowering") **once an operation surface
of this shape exists at all** — the rule that rejects it is already general
and already tested for every other admitted element type; this document
identifies it, it does not have to add it.

## Ownership/transfer rule (explicit)

Per the existing owned-call contract (AGENTS.md: "an owned call stages
arguments left to right and transfers them together at its declared commit
boundary"), a future `vec_push`-shaped operation over this element would stage
the vector and the new record argument left to right and transfer both at the
call's commit boundary, exactly as `vec_push<Bytes>` already does — the
record's own two `Bytes` leaves move as a unit inside that one argument slot,
never individually. A pushed record cannot be reused afterward for the same
reason a pushed `Bytes` value cannot: the source binding's ownership epoch is
consumed at that same commit boundary (AGENTS.md: "ownership errors are
compile-time diagnostics, never backend accidents" — move-after-push would be
rejected the same way an ordinary use-after-move is today, via the existing
place-liveness tracking that already covers every owned type). This satisfies
AC-2 ("push/extract transfer ownership once, and a moved record cannot be
reused") as a *design consequence* of the existing, already-tested owned-call
and move-tracking contract; again, once a real operation exists to exercise
it, no new transfer mechanism is required.

## Backend hazard this profile must not reintroduce

`Vec`/`Box` operation dispatch shares one admission predicate between the
front end and every backend:

- Native: `src/codegen/native_emit/expression/vec_ops.rs::emit_vec_op` checks
  `crate::vec_ops::resolved_operation_element_is_admitted(op, element)`, then
  computes a numeric element tag with
  `match element { ResolvedType::I64 => 1, ..., _ => unreachable!("admitted
  bounded Vec element is scalar") }` — for **every** operation, before the
  per-operation branch. `src/codegen/native_emit/expression/box_ops.rs` has
  the identical pattern (two `unreachable!("admitted bounded Box element is
  scalar")` sites).
- Interpreter: `src/interpreter/owned_vec.rs` has the parallel
  `unreachable!("validated Vec clear carrier")` once dispatch reaches an
  element outside the admitted set.

Each of these `unreachable!()`s is safe **today** only because
`resolved_operation_element_is_admitted`/`resolved_vec_element_is_admitted`
(and the Box equivalents) return `false` for anything but a Copy scalar or
`Bytes`, so source/HIR/cleanup already refuse the call before any backend is
reached. **Widening either shared predicate to admit this record shape — even
only to make `Vec<Item>` a well-formed type for cleanup's opaque-leaf
treatment — reopens this hazard**: the only way a program can ever hold a live
value of that type is through an admitted constructive operation
(`vec_with_capacity`, `box_new`), so admitting construction and reaching
`build`/`run` on that program would hit one of the `unreachable!()`s above
instead of a diagnostic. That is a direct violation of "ownership errors are
compile-time diagnostics, never backend accidents" (AGENTS.md) and of this
issue's own AC-8 ("no backend executes the new shape before its conformance is
delivered") — not as a policy choice but as a compiler crash.

This is why this tranche stops at the classifier: closing this hazard requires
either (a) replacing those `unreachable!()` sites with a clean diagnostic in
`src/codegen/native_emit/**` and `src/interpreter/**` — both outside this
issue's file lease — done in lockstep with admitting the operations, which is
exactly SPX-AI-020/issue #119's stated scope ("Execute owned-record
collections on interpreter, C11 and Core Wasm with failure settlement"); or
(b) introducing a genuinely separate, non-`core.vec`/`core.box`-sharing
compiler-owned nominal type and operation identity set that no existing
backend dispatch table recognizes at all. Option (b) is verified safe by
construction: `src/codegen/native_emit/expression.rs::emit_call_expr` tries
`vec_ops::by_id`, `iterator_ops::by_id`, `box_ops::by_id`, `host_io_ops`,
`str_ops`, `byte_ops`, `string_ops`, and otherwise falls to
`emit_user_call_expr`, which looks the callee up in `self.functions` and
returns a clean `Err(backend_error("resolved callee `…` is not indexed"))`
rather than panicking when it is registered nowhere; `cleanup_plan::build`'s
call-shape dispatch has the identical fallback
(`"unknown cleanup call target"`). A genuinely new, unregistered intrinsic ID
therefore fails closed safely without touching any backend file — but
building that surface (new stable operation identities, HIR resolution,
cleanup-plan build/replay branches, and the source-side call recognition) is
substantial, net-new plumbing this tranche did not attempt, consistent with
the issue's own review-checkpoint note that a bounded design should be
submitted for review before an implementing agent self-approves this much new
compiler surface.

## What remains (explicitly out of scope here)

- An operation surface (`with_capacity`/`push`/`len`/`capacity`/`clear` at
  minimum; `get`/`set`/`reserve_exact` are deliberately excluded from the
  narrow profile the same way `vec_get<Bytes>` is already excluded today, to
  avoid "an ambiguous copy-returning get of an owned value" per this issue's
  own implementation guidance) — via either closure strategy above.
- Widening `cleanup::is_owned_bounded_vec_type` /
  `crate::vec_ops::resolved_vec_element_is_admitted` (and the Box
  equivalents) — only safe once paired with the backend hazard fix.
- Consuming extraction/traversal beyond bulk `clear`. `Vec`'s own v1/v2
  profile already ships with no `pop`/insertion/removal as a stated
  "Nonclaim" (`OWNED-BOUNDED-VEC-V1.md`); this profile follows that precedent
  rather than introducing pop for records first. A user-visible consuming
  traversal, if wanted, should compose with the existing
  [Owning Iterator Payloads v2](OWNING-ITERATOR-PAYLOADS-V2.md) profile rather
  than reinvent it here.
- Any change to `docs/COMPLETION-MATRIX.md`, `CHANGELOG.md`, or the roadmap
  (coordinator-owned).

## Test discovery

`cargo test --locked -p semaprax --lib owned_record_collection` selects the 17
focused unit tests above (12 HIR-side, 5 source-side). They construct real
`.spx` source through `crate::parse`/`crate::hir::resolve` (not hand-built
HIR), the same pattern `type_reachability`'s own classifier tests use, so the
positive cases are genuine parsed-and-resolved programs, not synthetic
fixtures assembled to make the classifier agree with itself.
