# Owned Record Collection Element v1

Audience: compiler contributors and reviewers of SPX-AI-019/020 (issues #118,
#119).

Status: **the reference interpreter and native C11 execute it; Core Wasm
still refuses it**. Source and HIR independently recognize the one admitted
record shape this document defines (`src/hir/owned_record_collection.rs`,
`src/source_verify/declared_type/owned_record_collection.rs`), and a bounded
`Vec<T>` operation surface over it — `vec_with_capacity`, `vec_push`,
`vec_len`, `vec_capacity`, `vec_clear` — is wired through the source verifier,
the resolver, HIR validation, the loan plan, the cleanup inventory, the
cleanup plan and its replay. `vec_get`, `vec_set` and `vec_reserve_exact`
stay refused, as does `Box<T>` of this element. The reference interpreter
stores one authored record per element and executes the whole surface
(`SPX-F112` retired), and the native C11 backend lowers the same surface to
real per-element storage and drop (`SPX-B115` retired); WebAssembly
(`SPX-W125`) does not implement the carrier yet and refuses the same source
up front rather than emitting a carrier it cannot lower. See "Interpreter
conformance tranche (2026-09-12)" and "Native C11 conformance tranche
(2026-09-12)" at the end of this document for what that cost and for the
measured seam the remaining backend sits behind. Earlier sections
record how the profile reached this state: the classifier tranche, the
backend panic-hardening, the 2026-09-11 acceptance-criteria audit, and the
front-end admission tranche. Statements there about "no call site exists",
"no program can construct this type" and "nothing here is executed" describe
the state before the tranche recorded at the end of this document, not the
current tree.

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
  (resolved/HIR side, keyed on `DeclarationIndex`), with 13 unit cases:
  admission across all eight Copy-scalar substitutions, field-order
  independence, an extra field, a missing field, a wrong-but-Copy-scalar
  field type in either slot, a `String` field (neither `Bytes` nor a Copy
  scalar, exercising the classifier's other refusal path), two Copy fields
  with only one `Bytes` field, a nested record field, a `resource`, a
  `class`, a generic record instantiated to the exact field shape, a
  `variant` sharing the same field-name convention, and non-nominal types.
- `src/source_verify/declared_type/owned_record_collection::is_admitted_owned_record_collection_element`
  (AST side, keyed on `TypeTable`), with 6 unit cases covering the same
  admit/refuse boundary before resolution, including the same dedicated
  `String`-field case.

Both are `pub(crate)`/`pub(in crate::source_verify)`, and each is now the one
audited home its own projection's call sites consult, rather than the rule
being reimplemented inline at each site. They were staged ahead of those call
sites, matching the precedent set by `src/host_ownership.rs`; the tranche
recorded at the end of this document wired them up.

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

What needed to change: both `cleanup::is_owned_bounded_vec_type` and
`box_ops::is_type` gate that opaque recognition on
`crate::vec_ops::resolved_vec_element_is_admitted`/
`crate::box_ops::resolved_box_element_is_admitted` — the same scalar-or-Bytes
predicate that native and Wasm backends also consult (see "Backend hazard"
below). Widening those two predicates is exactly the unsafe step this
document explains how to avoid, and the tranche recorded at the end of this
document did not take it: it added a separate carrier predicate consulted
only by the front end, and refused the profile at every backend instead. The
`Vec` carrier therefore still gets exactly one opaque `core.vec.drop` leaf,
and the record still gets its ordinary per-field leaves; `box_ops::is_type`
is untouched because `Box<T>` of this element is not admitted.

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

## SPX-AI-020 (issue #119) execution decision and status

This section is the record of the choice this tranche made between the two
closure strategies above, what it verified before choosing, and exactly what
it landed and did not land. It supersedes nothing above; it narrows the two
options to one and reports the concrete blast radius the prior section could
only estimate.

**Decision: closure strategy (a)** — reuse the existing `vec_ops`/`box_ops`
dispatch, cleanup-plan single-leaf shape, and loan-plan borrow rules by fixing
the shared admission predicates and their backend call sites together, rather
than building a second, `core.vec`/`core.box`-independent intrinsic identity
set (strategy (b)). Both `src/codegen/native_emit/expression/vec_ops.rs` and
`src/interpreter/owned_vec.rs` (plus their `box_ops.rs`/`owned_box.rs`
Box analogues in the same tree) were read directly, not taken on the prior
summary's word, before this choice was made.

Reasoning, from what reading the actual call graph showed:

- The issue's own implementation guidance (step 1) asks to "reuse existing
  collection operation dispatch and carrier allocation owners... without
  teaching each backend a different language rule." Strategy (b) does the
  opposite: it would stand up a second call-ID namespace, a second HIR call
  resolver, and a second cleanup-plan call-shape branch that duplicate logic
  this document's own "Cleanup rule" and "Borrow rule" sections already prove
  is element-type-generic and needs no change for this shape. Strategy (a)
  spends no new plumbing on state that already generalizes.
- Strategy (b)'s central safety claim — an unregistered intrinsic ID fails
  closed through `emit_user_call_expr`'s "resolved callee is not indexed"
  fallback — is real, but it is a safety property of *doing nothing*
  (nothing ever recognizes the new ID), not a step toward execution. Since
  this issue's outcome is execution, not a second safely-inert surface,
  strategy (a) is the only one of the two that is actually going somewhere.
- Strategy (b) does not avoid the deeper shared-infrastructure problem
  documented below (TypeFacts and `is_owned_bounded_vec_type` consistency):
  a `RecordVec<Item>`-shaped alternative type would need its own admission
  entry in `src/hir/declaration_index/owned_builtin.rs`-equivalent TypeFacts
  wiring and its own cleanup-plan shape gate anyway, so it does not shrink
  the real blast radius — it only relocates it into new files while forgoing
  the reuse the issue asks for.

**What reading the code showed that the prior tranche's summary did not
state:** the two documented `unreachable!()` sites are not the only place
that gates on the shared admission predicates
(`crate::vec_ops::resolved_vec_element_is_admitted` /
`resolved_operation_element_is_admitted`, and the `box_ops` equivalents).
`crate::cleanup::is_owned_bounded_vec_type` (defined in terms of the same
`vec_ops` predicate) is independently consulted at roughly two dozen sites
across native codegen (`src/codegen/native_cleanup.rs`,
`src/codegen/native_emit/mod.rs`, `src/codegen/native_emit/expression.rs`,
`src/codegen/native_vec.rs`) and Wasm codegen (`src/wasm/vec_ops.rs`,
`src/wasm/aggregate.rs` alone has upward of a dozen call sites, plus
`src/wasm/aggregate/post_transitions.rs`) for value-type mapping, alignment,
parameter/return handling, and cleanup replay — not just the one dispatch
function each backend uses for the five intrinsic operations. Separately,
`src/hir/declaration_index/owned_builtin.rs::owned_builtin_facts` is the
*only* thing that makes `Vec<Item>` a well-formed `sized`/`needs_drop` type
at all; cleanup-inventory construction calls `type_facts()` on every local
binding's type, so no `let` binding of that type can exist, in any function,
on any backend, until `owned_builtin_facts` admits it.

This means TypeFacts admission and `is_owned_bounded_vec_type` cannot be
widened independently of each other or of the two documented backend
dispatch sites: if `owned_builtin_facts` alone were widened, `Vec<Item>`
would become a legal, sized, needs-drop type *everywhere a type can appear*
(function parameters, return types, struct fields), while `is_owned_bounded_vec_type`
would still say no at every one of its ~25 other call sites — an
inconsistency between "HIR says this type is a legal owned type" and "every
backend layout/ABI function says this nominal is not the compiler-owned Vec
profile," which is exactly the shape of hazard this document exists to avoid,
just relocated to codegen's type-layout functions instead of the two
documented op-dispatch sites. Closing the two named `unreachable!()`s alone,
without this consistency, would not be sufficient. The honest scope of
strategy (a)'s "lockstep" fix is therefore a signature change threading
`&DeclarationIndex`/`&ResolvedProgram` context through
`resolved_vec_element_is_admitted`/`resolved_operation_element_is_admitted`
(or `is_owned_bounded_vec_type` directly), consistently, across on the order
of 25-40 call sites spanning `src/hir/resolve_vec_call.rs`,
`src/hir/validation/vec_intrinsic.rs`,
`src/hir/declaration_index/owned_builtin.rs`, `src/cleanup.rs`,
`src/cleanup_plan/build/bounded_vec.rs`,
`src/cleanup_plan/replay/nested_shape.rs`, `src/codegen/native_cleanup.rs`,
`src/codegen/native_emit/mod.rs`, `src/codegen/native_emit/expression.rs`,
`src/codegen/native_vec.rs`,
`src/codegen/native_emit/expression/vec_ops.rs`, `src/wasm/vec_ops.rs`,
`src/wasm/aggregate.rs`, `src/wasm/aggregate/box_ops.rs`,
`src/wasm/aggregate/post_transitions.rs`, `src/interpreter/owned_vec.rs`, and
`src/hir/validation.rs` (plus the `box_ops` analogues throughout), verified
consistently at every site in one pass. That is a large, single-sitting,
every-backend-simultaneously change; landing it partially or with one missed
call site would risk creating a *new*, unaudited backend-accident hazard
elsewhere in codegen's type-layout functions, which would be worse than the
status quo this document already describes. It was not attempted in this
session for that reason, consistent with the repository's own
review-checkpoint guidance against one implementing agent self-approving
this much new compiler-wide surface in one pass.

**What this session landed instead (SPX-AI-020, this commit):** hardening of
the two documented `unreachable!()` sites plus their `Box` analogues, ahead
of and independent of the predicate-widening above, so that *if* a future
change ever reaches this dispatch with an unadmitted element — whether by the
lockstep widening above, an unrelated future refactor, or a forged/hostile
intermediate representation — it fails closed with a diagnostic today, not a
compiler panic:

- `src/codegen/native_emit/expression/vec_ops.rs`: `emit_vec_op`'s inline
  scalar-tag `match` and `vec_bits_to_scalar`'s fallback both used
  `unreachable!("admitted bounded Vec element is scalar")`. Both are now the
  extracted, directly unit-tested `vec_element_tag`/`vec_bits_to_scalar`
  functions, returning `Err(backend_error(..))` for anything outside the
  admitted scalar profile.
- `src/codegen/native_emit/expression/box_ops.rs`: the identical pattern
  (`box_element_tag`/`box_bits_to_scalar`), same treatment.
- `src/interpreter/owned_vec.rs`: the `VecOp::Clear` arm's
  `unreachable!("validated Vec clear carrier")` is now
  `Err(Flow::Guard(..))`. (This one is additionally provably unreachable
  today by the immediately preceding `as_slice()` match on the same `values`
  binding, independent of element admission — the hardening is still applied
  for defense in depth and consistency with the other four sites.)
- Wasm's own dispatch (`src/wasm/aggregate.rs::vec_element_tag`) was checked
  and already returns `Err(..)` rather than panicking for an unadmitted
  element; it needed no change. `src/interpreter/owned_box.rs` was checked
  and has no analogous `unreachable!()`.

None of `resolved_vec_element_is_admitted`, `resolved_operation_element_is_admitted`,
the `box_ops` equivalents, `is_owned_bounded_vec_type`, or `owned_builtin_facts`
were touched. Every one of the five hardened branches remains exactly as
unreachable through its normal entry point as it was before this change — no
program can construct a value of the admitted record-collection element type
today, so **no backend executes this shape**, exactly as before this
tranche. What changed is that "unreachable" is now enforced by a typed
`Result`/`Flow::Guard` return with a direct regression test proving it,
instead of by an `unreachable!()` whose safety depended entirely on every
caller of the shared predicate staying in sync by convention.

Four new unit tests were added, one per hardened native-codegen function
(`vec_element_tag_refuses_an_unadmitted_element_without_panicking`,
`vec_bits_to_scalar_refuses_an_unadmitted_element_without_panicking`,
`box_element_tag_refuses_an_unadmitted_element_without_panicking`,
`box_bits_to_scalar_refuses_an_unadmitted_element_without_panicking`),
exercising each function directly with an element outside the admitted
profile and asserting `Err`, not a panic. The interpreter's `owned_vec.rs`
site has no analogous direct test because it is structurally unreachable
through any input already (see above); adding one would require fabricating
an internal state the surrounding code proves cannot occur, which would not
be genuine regression coverage.

**Consequently, this tranche does not claim interpreter, native, or Wasm
execution of the owned-record collection profile.** The acceptance criteria
that require genuine execution, exact settlement proof, or ordered-traversal
value comparisons are not met by this tranche; see the issue report for the
criterion-by-criterion accounting. The next tranche's critical path is the
predicate-widening inventory above, done consistently in one pass across
every listed call site, backend by backend in the priority order the issue
states (interpreter, then native, then Wasm) — starting with interpreter
execution requires *all* of the HIR/TypeFacts/cleanup-plan sites above plus
`src/interpreter/owned_vec.rs`, not the interpreter file alone, because a
real `.spx` program must resolve before the interpreter ever sees it.

## Issue #118 acceptance-criteria audit (2026-09-11 re-audit)

This section records a fresh criterion-by-criterion re-check of issue #118
against the tree at the time of this audit (starting commit `20f0c24c`, the
tip of `origin/main`), performed independently of the summaries above rather
than trusting them on their word. The prior sections' claims were re-verified
by reading the cited source directly, not re-derived from the doc text.

- **AC-1** ("the exact selected payload is admitted; structurally similar
  foreign nominal types and unsupported nested/resource payloads are
  refused"): **met**. Confirmed by reading
  `src/hir/owned_record_collection.rs` and
  `src/source_verify/declared_type/owned_record_collection.rs` directly: both
  classifiers are present, `pub(crate)`/`pub(in crate::source_verify)`, and
  structurally implement exactly the rule this document states. This session
  added one more regression per classifier (`string_field_is_refused`) that
  the prior 12+5 did not isolate: every existing "wrong field type" fixture
  substituted another *admitted Copy scalar* (`bool`) for a `Bytes` field, so
  it is refused only by the bytes/copy-count check; no fixture exercised a
  field type that is neither `Bytes` nor a Copy scalar against a *record*
  shape specifically — only the pre-existing nested-record-field fixture did,
  which conflates two different reasons for refusal (see below). Landing this
  fixture surfaced a real, previously-undocumented boundary in the HIR case:
  a record mixing owned `Bytes` fields with a `string` field does not reach
  `admits_field_shape`'s own `return false` branch at all — it is refused
  earlier, during resolution itself, by the pre-existing owned-Bytes record
  shape rule (`SPX-T268`, "must be a monomorphic acyclic record tree with
  only `Bytes` or direct Copy scalar leaves"), the same upstream-refusal
  pattern the pre-existing `class_declaration_is_refused` and
  `generic_record_is_refused_even_when_instantiated_to_the_exact_field_shape`
  fixtures already rely on via `resolve_if_admitted`. The HIR-side
  `string_field_is_refused` fixture was written and initially failed with a
  resolution panic before this was found and the fixture was corrected to
  tolerate that upstream refusal the same way its two siblings do; the
  source-side fixture (AST/`TypeTable`-only, no resolution) does reach and
  exercise the classifier's own `return false` branch directly, so between
  the two, both the earlier-refusal and classifier-own-refusal paths are now
  each pinned by at least one regression. The new tests pin the doc's own
  explicit claim ("a `String` field... is refused") with a dedicated case.
  Test count is now 19 (13 HIR-side, 6 source-side).
- **AC-2** ("push/extract transfer ownership once, and a moved record cannot
  be reused"): **not met**. Re-confirmed: `crate::vec_ops::PUSH_NAME`/
  `PUSH_ID` and the parallel Get/Set/Clear intrinsics resolve calls through
  `src/vec_ops.rs`, `src/hir/resolve_vec_call.rs` and
  `src/hir/validation/vec_intrinsic.rs`, all of which still gate on
  `resolved_operation_element_is_admitted`, unchanged from before this
  session (`rg -n resolved_vec_element_is_admitted\|resolved_operation_element_is_admitted
  src/vec_ops.rs src/box_ops.rs` shows only the same two definitions and their
  pre-existing call sites; no new call site was added anywhere in the tree).
  No operation surface exists for this element shape, so there is no push or
  extract to test transfer against.
- **AC-3** ("a borrow live across reallocating or consuming mutation is
  rejected before lowering"): **not met, and not independently testable yet**.
  `src/loan_plan.rs` is confirmed unmodified and already element-type
  generic, so the rule *would* apply automatically once an operation surface
  existed (as the "Borrow rule (explicit)" section above already argued), but
  with no admitted operation there is no borrow-mode call parameter of this
  element type to construct a positive or negative fixture around. Asserting
  this criterion today would require fabricating call sites this profile does
  not have, which is not genuine coverage.
- **AC-4** ("failure paths derive canonical per-leaf cleanup and retain sticky
  status"): **not met**. `cleanup_plan::build::bounded_vec`/`bounded_box`
  remain the single-opaque-leaf treatment described above; no per-leaf
  cleanup for a record living inside a collection element exists because no
  collection of this element type can be constructed. The record's own
  *standalone* cleanup (outside any collection) already works today under
  the pre-existing Owned Byte Record Algebra v1 contract, as this document
  already stated; that is unrelated to and does not satisfy this
  collection-specific criterion.
- **AC-5** (generic substitution / source-HIR replay / old public-profile
  rejection stability): **met, for the surface that exists**. This session
  re-ran the classifier's own generic-substitution refusal case plus the full
  existing `vec_ops`/`box_ops` unit suites (see verification commands and
  counts in the coordinator's final report) with no regression; the
  classifier's own tests already assert generic-record refusal
  independently in both source and HIR. No source/HIR/graph replay test
  targets this element specifically beyond the classifier unit tests, because
  no source construct can produce a value of this type yet for replay to
  observe.
- **Definition-of-done, "no backend executes the new shape before its
  conformance is delivered"**: **met, trivially and by construction, not by
  policy**. Re-confirmed directly: `owned_builtin_facts`
  (`src/hir/declaration_index/owned_builtin.rs`) still gates
  `Vec`/`Box` TypeFacts admission on
  `resolved_vec_element_is_admitted`/`resolved_box_element_is_admitted`,
  unchanged, so `Vec<Item>`/`Box<Item>` is not a legal, sized, needs-drop type
  anywhere a type can appear (parameter, return, field, or local binding).
  No `.spx` program can construct, hold, or pass a collection of this
  element, so no backend can be reached with one, independent of whether the
  two previously-hazardous `unreachable!()` sites remain hardened.
- **Panic-hazard finding (the central design constraint for this audit)**:
  **re-confirmed independently, and still closed on `origin/main`**. The two
  originally-cited sites
  (`src/codegen/native_emit/expression/vec_ops.rs`'s inline scalar-tag match
  and `vec_bits_to_scalar`'s fallback, and the identical pattern in
  `src/codegen/native_emit/expression/box_ops.rs`) were re-read at this
  session's starting commit: neither contains `unreachable!()` any longer.
  `rg -n "unreachable!" src/codegen/native_emit/expression/vec_ops.rs
  src/codegen/native_emit/expression/box_ops.rs src/interpreter/owned_vec.rs`
  returns only doc-comment prose mentioning the historical `unreachable!()`,
  not a live panic site; the functions now return
  `Err(backend_error(...))`/`Err(Flow::Guard(...))` and are covered by four
  dedicated unit tests
  (`vec_element_tag_refuses_an_unadmitted_element_without_panicking` and its
  three siblings) that call the extracted helper functions directly with an
  unadmitted element and assert `Err`, not a panic. This closure landed on
  `origin/main` at commit `fab82012` (`git merge-base --is-ancestor fab82012
  origin/main` succeeds), prior to and independent of this session's work, as
  part of issue #119 rather than #118. This session did not touch, and did
  not need to touch, any of those files (all four are outside this session's
  file lease: `src/codegen/native_emit/**`, `src/interpreter/**`). No
  predicate that feeds either hardened site was widened by this session or by
  any commit reachable from `origin/main`
  (`resolved_vec_element_is_admitted`/`resolved_operation_element_is_admitted`
  and the `box_ops` equivalents are byte-for-byte the same scalar-or-`Bytes`
  check as before), so the hardening remains exercised only by its direct
  unit tests, not by any live, reachable program path — consistent with "no
  backend executes the new shape" above.

**Conclusion of this audit:** the only criterion this session found room to
close, within its file lease (this document plus the two classification
modules and their own tests, excluding `src/public_generic_abi/**`,
`src/public_generic_consumer/**`, `src/live_invocation/**`, `src/wasm/**`,
`src/cleanup_plan/**`, native codegen and the interpreter), was strengthening
AC-1's regression coverage with the `String`-field case. AC-2 through AC-4 and
the operation-level part of AC-5 require the predicate-widening-in-lockstep
work this document already scopes under "SPX-AI-020 execution decision and
status" and "What remains" below, which touches exactly the files this
session's lease excludes. Widening the classifier's *admitted shape itself*
(e.g., admitting additional field counts or types) was considered and
rejected: issue #118's own bounded-scope section fixes the admitted shape to
the one application record deliberately ("additional payload trees require
separately enumerated admission and tests"), so widening it here would be
scope creep against the issue's own text, not a gap closure — the narrowness
is a design decision already made, not an oversight this session found.

## Front-end admission tranche (2026-09-12)

This section records the tranche that closed issue #118's AC-2, AC-3 and AC-4,
and supersedes the "not met" rows of the 2026-09-11 audit above.

**What it chose.** Strategy (a)'s reuse of the existing `vec_ops` dispatch,
cleanup-plan shape and loan-plan rules, but only through the front end, with
every ordinary execution target refusing the profile up front. That is issue
#118's own implementation step 6 ("keep ordinary target execution closed for
this new profile until SPX-AI-020 supplies all selected backend
implementations; add an explicit stable unsupported-target diagnostic if
intermediate source admission is reachable, rather than emitting a broken
carrier"), and it is what makes AC-2 through AC-4 testable at all: they are
statements about ownership, borrowing and cleanup, all of which are decided
before lowering.

**What it did not widen, and why.** `crate::cleanup::is_owned_bounded_vec_type`
and `crate::vec_ops::resolved_vec_element_is_admitted` keep their existing
narrow meaning. The blast-radius inventory above is accurate: roughly forty
native and Wasm call sites consult that predicate to map the carrier onto a
concrete machine representation, and this profile has no such representation
yet. Widening it would have relocated the hazard into codegen's type-layout
functions, exactly as that inventory warns. Instead the profile has its own
carrier predicate,
`hir::owned_record_collection::is_owned_record_vec_type`, consulted only by
the front-end sites that decide meaning:
`hir::resolve_vec_call`, `hir::resolve_program`'s two generic-argument rules,
`hir::declaration_index::owned_builtin::owned_builtin_facts`,
`hir::validation`'s intrinsic signature, callable-type and borrow-argument
rules, `cleanup::type_needs_resource_cleanup` and the cleanup inventory,
`cleanup_plan::build::bounded_vec` and `cleanup_plan::replay::nested_shape`,
plus their source-side companions in `source_verify::declared_type`,
`source_verify::type_table`, `source_verify::iterative::enter` and
`source_verify::oracle::calls`.

**The target refusal.** `hir::owned_record_collection::program_uses_profile`
scans signatures and resolved compiler-owned `Vec` call type arguments — the
only two ways a value of the carrier type can exist — and
`reject_for_target` turns a hit into one stable diagnostic. It is applied at
each backend's single emission choke point:
`codegen::native_emit::emit_hir_c_with_labels` (via `validate_for_native`),
`wasm::aggregate::target_gates::reject_unsupported_profiles` (both aggregate
module profiles), and every `src/interpreter.rs` entry point (via
`validate_for_interpreter`/`resolve_for_interpreter`). None of these files
grew: each edit replaces an existing validation line, and `wasm/aggregate.rs`
shrank because its duplicated resource gate moved into the new
`target_gates` helper.

**Ownership, borrow and cleanup, as observed rather than argued.** The
"Ownership/transfer rule" and "Borrow rule" sections above predicted that no
new mechanism would be required once an operation surface existed. That
prediction held, and is now pinned by regressions in
`src/hir/owned_record_collection/operation_tests.rs`:

- `vec_push` stages its vector and its record left to right and transfers
  both at one `CallCommit` boundary; the record's two owned `Bytes` leaves
  move as a unit inside the single argument slot. Reusing a pushed record is
  `SPX-O101` in both projections.
- A borrowing `vec_len` of the carrier after a consuming `vec_clear` is
  refused, and so is one *live across* a consuming `vec_push` — writing
  `vec_len<Item>(items)` inside the second argument of
  `vec_push<Item>(items, …)`, where left-to-right staging has already
  transferred `items`. Both refuse before lowering, in both projections.
- Cleanup expands the record per leaf: one `core.bytes.drop` leaf for each
  owned field, in authored declaration order with its declared field index,
  `NoDrop` for the Copy field, and exactly one `core.vec.drop` leaf for the
  carrier. Every fallible owned call publishes a `SelectFailure` edge, so
  failure selection is sticky and later cleanup cannot replace it.

**Element ownership mode.** `VecOp::param_ownership_for` now treats a
`ResolvedType::Nominal` element as owned, like `Bytes`. That is exact rather
than approximate: Copy scalars are primitive `ResolvedType` variants and a
generic collection's element is a `ResolvedType::TypeParameter`, so the
admitted record is the only `Nominal` any admission path lets reach that
function. The AST-side companion cannot make the same argument — a generic
wrapper's type parameter and this record are both `Type::Named` with no
arguments — so `vec_ops::ast_params_with_owned_element` takes the
classifier's answer from its caller instead of guessing.

**Known limits of this tranche.**

- `Vec<Item>` is still refused in a *declared signature* by the pre-existing
  `SPX-T223` generic-copy-type rule, so the profile is confined to function
  bodies. `program_uses_profile` scans parameters and return types anyway, so
  the refusal stays correct if that rule is ever widened.
- Record construction inside a `while` body remains `SPX-T252`, so the
  loop-carried accumulate shape is not available to this profile yet.
- `Box<T>` of this element, and therefore a consuming extraction such as
  `box_into_inner`, is not admitted. AC-2's "extract" half is not covered;
  the profile has bulk `clear` only, following `OWNED-BOUNDED-VEC-V1.md`'s
  own "Nonclaim" precedent.
- Nothing in *that* tranche was executed: there was no interpreter, native or
  Wasm evidence of the profile running, by construction. The interpreter half
  of that limit is closed by the tranche recorded below; native and Wasm are
  still refusals.

## Interpreter conformance tranche (2026-09-12)

This section records SPX-AI-020's (issue #119) first backend, and supersedes
the "Nothing here is executed" limit of the front-end tranche above.

**What lifted.** `SPX-F112` is retired and the reference interpreter executes
the profile. It is retired by working lowering, not by deleting a refusal:
`interpreter::owned_vec::OwnedVecValue` already stores one `Value` per
element, so the change is that `evaluate_vec_op` admits the record element
through this profile's own `admits_vec_operation_element` predicate, and
`vec_push` stores one authored `Value::Record` per element. Every interpreter
entry point is back on plain `hir::validate`/`hir::resolve`.

**What deliberately did not widen.** `cleanup::is_owned_bounded_vec_type` and
`crate::vec_ops::resolved_vec_element_is_admitted` keep their narrow meaning,
unchanged. The interpreter reaches the profile through the separate carrier
predicate the front-end tranche introduced, so none of the native and Wasm
layout, ABI and cleanup-replay sites that map the shared predicate onto a
machine representation is touched, and the hazard this document exists to
avoid is not relocated into codegen's type-layout functions.

**Element authenticity.** Push re-derives the element's admission from
`DeclarationIndex` facts on every call rather than trusting the static type:
the runtime record must name the same authored declaration, carry exactly the
declared field identities, and hold a value of the declared type in each. A
forged or mis-typed carrier is a `Flow::Guard` diagnostic. With the new
admission disabled, the profile's execution fixtures fail with
`GuardError("invalid compiler-owned bounded Vec type")` — a diagnostic, never
a panic — which is both the negative control for the tranche and evidence that
the hardening recorded above still holds.

**Owned-payload bound.** One admitted record owns exactly two `Bytes` leaves,
so it is charged twice the single-leaf rate `Vec<Bytes>` already uses, through
one shared constant
(`hir::owned_record_collection::OWNED_PAYLOAD_BYTES_PER_RECORD_ELEMENT`, 32
bytes per element) so every target that later implements this carrier bounds
it identically. Against the existing `MAX_OWNED_PAYLOAD_BYTES` of 131072 that
admits 4096 elements, within the shared `MAX_CAPACITY` of 8192; a dynamic
request past it selects the existing `semaprax.vec.v1` code 3 rather than
over-committing.

**Executed evidence.** `tests/owned_data/owned_record_vec_runtime.rs` drives
committed `.spx` source through `semaprax::check` and
`interpreter::interpret`: an accumulate-clear-reuse application fragment, the
empty and exactly-full-capacity boundaries, push past capacity
(`semaprax.vec.v1` code 1), the owned-payload bound (code 3), and precondition
and postcondition failure (`semaprax.contract.v1` codes 1 and 2). Each fixture
runs four times and must publish the same outcome, and the sticky-failure case
is paired with a widened twin that does publish a value, so the failure is the
push itself and not an unrelated refusal. Two further lib regressions in
`src/hir/owned_record_collection/operation_tests.rs` exercise the same surface
through `evaluate_resolved_zero_arg_i64`.

**Backend agreement, honestly.** At the time of this tranche native C11
(`SPX-B115`) and Core Wasm (`SPX-W125`) both still refused, asserted against
the exact same source the interpreter executes. Native has since lifted; see
the native tranche below. Core Wasm still refuses, and that assertion is still
carried by both the lib and the integration harness.

**Why native and Wasm did not lift in the same sitting.** The seam is not the
predicate count; it is that neither backend's carrier is element-type generic
the way the interpreter's `Vec<Value>` already was. (The native bullet below
is the measurement as it stood at this tranche. It over-estimated: see "Native
C11 conformance tranche (2026-09-12)" for what the native lift actually cost
and why. The Wasm bullet still stands.)

- Native selects exactly one of two whole-program `Vec` runtimes in
  `codegen::native_vec::emit_runtime`: the scalar runtime, whose slot is
  `uint64_t`, or the owned-payload runtime
  (`codegen::native_vec::owned_payload`), whose slot is the fixed
  `spx_bytes_v1`. The record element is neither. Its slot type is a
  per-declaration authored struct, so it needs a third runtime variant whose
  slot type, push, clear and drop are *generated per record declaration*, a
  new `type_tag` beyond the existing 1..=9 and its dispatch in `spx_vec_drop`,
  and a three-way (not two-way) whole-program runtime selection — on top of
  widening the eight native `is_owned_bounded_vec_type` sites
  (`native_emit/mod.rs` ×3, `native_cleanup.rs` ×2,
  `native_emit/expression.rs` ×1, `native_vec.rs` ×2) and the element tag in
  `native_emit/expression/vec_ops.rs`.
- Wasm does not hold the vector in linear memory at all: the carrier is an
  `i64` host handle (`wasm::aggregate` maps the type to `I64` with an 8/8
  layout) and the module imports a fixed nine-function host boundary, chosen
  between `spx_vec_*` and `spx_vec_*_v2` by the same whole-program
  owned-payload question (`wasm::aggregate::vec_owned_payload::import_names`).
  A record element needs a third, `_v3` import set carrying record handles,
  implemented in every test host that links these imports, plus the fifteen
  `is_owned_bounded_vec_type` sites in `wasm/aggregate.rs`, `wasm/vec_ops.rs`
  and `wasm/aggregate/post_transitions.rs`.

Neither is blocked; both are larger than one sitting, and landing either
partially would create exactly the unaudited backend-accident hazard in
codegen's type-layout functions that this document exists to prevent. The
interpreter is now the semantic oracle both must be compared against.

## Native C11 conformance tranche (2026-09-12)

This section records SPX-AI-020's (issue #119) second backend, and supersedes
the native half of the seam analysis in the interpreter tranche above.

**What lifted.** `SPX-B115` is retired by working lowering. The native lane's
single emission choke point is back on plain `hir::validate`, and
`codegen::emit_c` emits, compiles and runs the same committed fixtures the
reference interpreter executes.

**The seam was cheaper than measured, and for a stated reason.** The
interpreter tranche predicted a *per-declaration* slot struct, a third
whole-program runtime variant and a three-way runtime selection. None of the
three was necessary. The admitted element shape is exactly two owned `Bytes`
fields and one admitted Copy scalar, so **one fixed slot layout covers every
admitted declaration**:

```c
typedef struct { spx_bytes_v1 spx_owned[2]; uint64_t spx_scalar; } spx_vec_record_v1;
```

The emitter places a declaration's fields into that slot in declaration order
— the admission rule is a field-type multiset, so declaration order is the one
canonical order available — and the profile admits no `get`, so nothing ever
reads an element back out and the placement is write-only bookkeeping the
runtime owns. That collapses the predicted third runtime into a `type_tag` of
10 inside the existing owned-payload runtime, and the predicted three-way
selection into one extra disjunct on the existing two-way choice.

**What deliberately did not widen.** `cleanup::is_owned_bounded_vec_type` and
`crate::vec_ops::resolved_vec_element_is_admitted` keep their narrow meaning,
unchanged. The native lane asks a new union predicate,
`codegen::native_emit::owned_carrier::is_native_owned_vec_type`, at the sites
that map a carrier onto a machine representation. Widening the shared
predicate instead would silently have changed the fifteen Wasm sites, which
still refuse this profile. No file under `src/wasm/` is touched.

**Element authenticity and staging.** The record lowering re-derives the
element's admission from `DeclarationIndex` facts at the emission boundary,
so a forged or widened HIR reaching `emit_vec_record_op` gets a backend
diagnostic rather than an `unreachable!()` arm. Staging reuses the canonical
cleanup plan's own `materialize_record_carrier` transitions — the same ones an
owned-record argument to a user call already uses — and emits them verbatim;
nothing here sorts, repairs or reinterprets a cleanup vector. A refused push
settles the staged element inside the runtime, because the caller's projected
liveness flags were already cleared to move the leaves into it, and the
carrier itself stays live for the epilogue. That is exactly one drop of each
leaf on both paths.

**Executed evidence.** `tests/owned_data/owned_record_vec_runtime.rs` now
compiles the emitted C with `clang -std=c11 -Wall -Wextra -Werror` at `-O0`
and `-O2` and runs it, for the same seven fixtures the interpreter executes:
accumulate-clear-reuse → 29, the empty and exactly-full-capacity boundaries →
29, push past capacity → `semaprax.vec.v1` code 1, the owned-payload bound →
code 3, and `requires false`/`ensures false` → `semaprax.contract.v1` 1 and 2.
Each binary runs its entry four times in one process.

**Physical settlement.** Every native case additionally proves, after each of
those four invocations, that the interposed allocator reports **zero live
allocations** and that **no `vec_authority` entry is live** — on success and
after injected failure alike. A carrier allocation refused by an injected
failing allocator selects `semaprax.vec.v1` code 3, publishes no value and
leaves nothing live. This is the physical probe the interpreter tranche
explicitly could not provide. A refused owned-`Bytes` leaf allocation is not
a case: `byte_ops` keeps physical allocation failure invariant fail-stop
rather than a selected status, and the staged-element settlement it would
probe is already covered by the refused push, which has a fully constructed
two-leaf element live at the moment of failure.

**Negative control.** An element one field away from the admitted shape
(three `Bytes` fields) is refused with a stable `SPX-` diagnostic by the
source verifier, and — were it ever to resolve — by the native emitter, rather
than reaching a lowering with no layout for it. Independently, disabling the
element admission makes the native execution fixtures fail with a diagnostic,
not a panic, which is this issue's governing invariant checked rather than
assumed.

**Owned-payload bound, shared not reinvented.** The emitted runtime's
`SPX_VEC_RECORD_MAX_CAPACITY` is pinned by a `const` assertion to
`MAX_OWNED_PAYLOAD_BYTES / OWNED_PAYLOAD_BYTES_PER_RECORD_ELEMENT`, so a
change to either constant fails the build rather than letting the native
ceiling drift away from the interpreter's.

## What remains (explicitly out of scope here)

- Core Wasm conformance. The seam is unchanged from the interpreter tranche's
  measurement: the carrier is an `i64` host handle, the module imports a fixed
  nine-function host boundary chosen `_v*` by a whole-program question, and a
  record element needs a third import set implemented in every linking test
  host, plus the fifteen `is_owned_bounded_vec_type` sites in
  `wasm/aggregate.rs`, `wasm/vec_ops.rs` and `wasm/aggregate/post_transitions.rs`.
  The native tranche's fixed-slot result suggests the per-declaration part of
  that estimate may also be avoidable, but the host-boundary part is not.
  Lifting `SPX-W125` is the rest of this issue's acceptance boundary.
- Physical allocation-count probes on Core Wasm. Native now provides them (see
  the native tranche above); the interpreter's byte accounting remains a
  cumulative budget rather than a liveness counter, so its evidence stays
  values, ordering and sticky failure selection.
- `Box<T>` of this element, and any consuming extraction. `vec_get`,
  `vec_set` and `vec_reserve_exact` also stay refused: `get` would be "an
  ambiguous copy-returning get of an owned value" per this issue's own
  implementation guidance, and the other two are outside the enumerated
  surface.
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

`cargo test --locked -p semaprax --lib owned_record_collection` selects 33
focused unit tests: the 19 classifier cases above (13 HIR-side, 6
source-side) plus the 14 operation-surface and interpreter-execution
regressions in `src/hir/owned_record_collection/operation_tests.rs`.
`cargo test --locked --test owned_data owned_record_vec_runtime` selects the
3 committed-source runtime cases. They construct real
`.spx` source through `crate::parse`/`crate::hir::resolve` (not hand-built
HIR), the same pattern `type_reachability`'s own classifier tests use, so the
positive cases are genuine parsed-and-resolved programs, not synthetic
fixtures assembled to make the classifier agree with itself.
