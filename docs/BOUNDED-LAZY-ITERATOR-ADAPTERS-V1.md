# Bounded Lazy Iterator Adapters v1

Status: implemented bounded profile for the existing scalar/Bytes consuming
iterator element set; **not hosted**, focused local evidence only.
Composition over the owned-record collection element (issue #119's
`Vec<Item>` profile) is explicitly not implemented and is blocked on work
outside this profile's scope; see "What remains" below.

Audience: compiler contributors, reviewers, and agent authors working on
SPX-AI-022 (issue #121).

This profile answers issue #121 ("Compose bounded lazy owned-data iterator
adapters") entirely by composing already-`HOSTED GREEN` profiles — [Owning
Iterators v1](OWNING-ITERATORS-V1.md), [Owning Iterator Loops
v1](OWNING-ITERATOR-LOOPS-V1.md), [Generic Compiler Collections
v1](GENERIC-COMPILER-COLLECTIONS-V1.md), [Generic Iterator Operations
v1](GENERIC-ITERATOR-OPERATIONS-V1.md), and [Scalar Snapshot Closures
v2](CLOSURES-V2.md) — into new authored `.spx` helpers. It introduces no new
compiler intrinsic, carrier type, cleanup schema, or admission predicate:
every helper below is ordinary source over the existing 1-or-2-type-parameter
generic function profile, exactly as `map`/`filter`/`fold` themselves already
are (`examples/iterator-operations.spx`).

## What this profile adds

Five helpers, in `examples/lazy-iterator-adapters.spx`:

```text
first_if_step<T>(own IterStep<T>, keep: fn(T) -> bool) -> bool
first_if<T>(own Iter<T>, keep: fn(T) -> bool) -> bool
map_filter<T, U>(own Iter<T>, capacity: usize, transform: fn(T) -> U, keep: fn(U) -> bool) -> Vec<U>
filter_fold<T, A>(own Iter<T>, keep: fn(T) -> bool, initial: A, combine: fn(A, T) -> A) -> A
map_fold<T, U>(own Iter<T>, transform: fn(T) -> U, initial: U, combine: fn(U, U) -> U) -> U
```

`first_if`/`first_if_step` is a one-pull adapter: it advances the source
iterator exactly once (`iter_next`) and lets the caller drop the untouched
remainder — which may still hold unconsumed elements — without ever reaching
exhaustion or pulling a second element. `first_if_step` takes `own
IterStep<T>` (the already-pulled step) rather than `own Iter<T>` so that its
body's `match own step { IterStep::Done{} => …, IterStep::Yield{…} => … }` is
admitted by the existing narrow rule in
`src/source_verify/declared_type/generic_variant.rs::profile`, which requires
an owned `IterStep<T>` parameter or return type before a generic function
body may pattern-match a `Yield`/`Done` case at all (see "What this profile
does **not** add" below for why a version with `own Iter<T>` and an internal
`iter_next` call inside the `match` is refused). `first_if` is the thin,
ergonomic wrapper callers actually use; it calls `iter_next<T>` and delegates
to `first_if_step`, staying inside the simpler direct-scalar call-composition
shape that needs no `match` admission at all.

`map_filter`, `filter_fold`, and `map_fold` are single-pass fused pipelines:
each performs exactly one `for own item in input` traversal and calls each
stage's callback at most once per element, in source order, without building
any intermediate `Vec`. Composing today's `map(...)` then `filter(...)` then
`fold(...)` (as `examples/iterator-operations.spx`'s `main` does) allocates
two intermediate `Vec`s; these fused helpers allocate at most one (`map_filter`'s
output) or zero (`filter_fold`/`map_fold`, which have no `Vec` in their body
at all — grep `examples/lazy-iterator-adapters.spx` for `vec_with_capacity`
inside those two bodies and find none).

Both `map_fold` and `map_filter` stay within the existing hard
`(1..=2)`-type-parameter generic-function admission ceiling
(`src/source_verify/declaration/functions.rs`,
`src/hir/generic_collection.rs`) by pinning the accumulator's type to the
mapped output type `U` rather than adding a third independent type parameter;
a `map_fold<T, U, A>` with an independently-typed accumulator is refused by
that ceiling today and is out of this profile's scope.

## What this profile does **not** add, and the exact diagnostics why

Two designs this session tried and could not land, kept here so a future
session does not re-derive them by trial and error:

- **A multi-pull, caller-invisible `filter_next<T>(own Iter<T>, keep) ->
  IterStep<T>` that internally skips more than one rejected element inside
  one call.** Two independent implementations were attempted and both are
  refused by the current tree:
  - A `while` loop with an internal `match own step { … }` re-pulling
    `iter_next` on rejection: refused with `SPX-T252`, `"match expressions
    are not yet admitted in while bodies"`.
  - A version calling itself on rejection (`filter_next<T>(rest, keep)`)
    instead of looping: refused with `SPX-T226`, `"generic function
    `filter_next` participates in a recursive call cycle"` — generic
    self-recursion is refused outright, independent of the `while`/`match`
    restriction above.

  Both diagnostics were reproduced directly against this tree (not assumed
  from documentation) while designing `first_if`; see this document's git
  history for the exact source that triggered each. `first_if` above is the
  profile's answer to what *is* constructible: a single pull, not a
  multi-pull skip-forward.
- **A version of `first_if` that takes `own Iter<T>` and calls `iter_next`
  and matches its result in one function body**, rather than splitting into
  `first_if`/`first_if_step`. Refused with `SPX-T226`, `"generic function
  `first_if` uses an expression outside the direct-scalar slice"`, because
  `src/source_verify/declared_type/generic_variant.rs::profile` only admits a
  `match` over `IterStep::Done`/`Yield` inside a generic function whose own
  *signature* (a parameter or the return type) names `IterStep<T>` directly
  — a plain `Iter<T>` parameter does not qualify, even though the function's
  first statement produces an `IterStep<T>` value internally. This mirrors
  exactly how the pre-existing `advance`/`rebuild`/`project` helpers in
  `tests/owned_data/generic_owned_function_runtime/generic_iterators.rs` are
  shaped (each takes or returns `IterStep<T>` in its own signature); this
  profile's `first_if_step` follows that same precedent rather than being a
  new pattern.

## Boundedness

`map_filter`'s output remains the existing bounded `Vec<U>` with an explicit
`capacity`; under-provisioning it fails with the existing
`semaprax.vec.v1` code 1 diagnostic instead of silently dropping the
overflowing element
(`lazy_iterator_adapters_bounded_capacity_exhaustion_is_reported`). The
fold-producing helpers have no growable output at all — the accumulator is a
fixed-size Copy scalar — so there is nothing to exhaust.

## Laziness

Construction and per-call cost are checked, not assumed:

- Empty and exhausted iterators invoke no callback at all. Each fused helper
  and `first_if` is exercised with a `requires false`-guarded "poison"
  callback over a zero-element `Iter<T>`; the program succeeds only because
  the guarded callback is never reached
  (`lazy_iterator_adapters_empty_and_exhausted_invoke_no_callback`).
- `first_if` pulls exactly one element per call and returns; the caller may
  drop the remaining, still-unconsumed elements (held by the pulled step's
  `rest: Iter<T>`) without calling anything again
  (`lazy_iterator_adapters_partial_consumption_then_drop_settles_once`).
- Rejected items never reach `combine`/the output `Vec`, and accepted items
  preserve source order, proven by exact expected values rather than counts
  alone (`lazy_iterator_adapters_rejected_items_dropped_once_accepted_preserve_order`).

## Ownership, sticky failure, and cleanup order

A callback (or its own `requires`) failing partway through a fused traversal
retains the selected status regardless of where in the traversal it occurred,
and no result is published
(`lazy_iterator_adapters_callback_failure_is_sticky_and_settles_owners`,
mirroring the existing `iterator_operations_callback_and_capacity_failure_settle_all_owners`
pattern this profile composes rather than reimplements).

Reusing the iterator argument after it has transferred into a fused call is
refused (`SPX-O101`) exactly like any other owned-call reuse-after-move
(`lazy_iterator_adapters_reuse_of_transferred_iterator_is_refused`,
`src/iterator_ops/lazy_adapter_tests.rs::fused_filter_fold_rejects_reuse_of_the_transferred_iterator`).
This is the closest achievable analogue to the issue's "repeated calls
through a one-shot closure are refused" requirement; see "One-shot closures:
inapplicable, not unmet" below for why the literal closure case cannot be
constructed today.

Cleanup order is checked two ways, per AGENTS.md ("cleanup inventory order is
structural metadata... must never be sorted or repaired downstream"):

1. **Determinism.** Two independent compiles of the identical fused-adapter
   source produce byte-identical graph JSON, cleanup plan included
   (`fused_filter_fold_runs_at_the_existing_iterator_and_cleanup_schema`).
2. **Hostile reorder rejection.** The fused function's own `for own item in
   input` loop lowers to a cleanup block whose `transitions` stage the
   hidden `IterStep<T>` slot and the accumulator's call argument, in the
   exact order `[CallArgumentTransfer, InitializeVariant, TransferVariant]`
   (observed directly from this function's own resolved cleanup plan, not
   assumed). A cloned, resolved program with two of that block's transitions
   swapped is rejected by ordinary `hir::validate` with `SPX-H006` — the
   same code and mechanism
   `src/cleanup_plan/replay_tests.rs::assert_independent_replay_rejects`
   already exercises for other shapes, reached here through the public
   `hir::validate` entry point rather than `cleanup_plan::replay`'s private
   internals, which this profile's file lease does not include
   (`fused_filter_fold_rejects_a_hostile_transition_order_reorder`).

The fused helpers use the existing `CLEANUP_PLAN_SCHEMA_V11` Owning Iterator
Loops v1 already introduced; no new schema version is added.

## One-shot closures: inapplicable, not unmet

Issue #121's required-test list includes "repeated calls through a one-shot
closure are refused; explicit state-threading variants pass their ownership
checks." The state-threading half is met: `filter_fold`/`map_fold`'s
accumulator *is* explicit owned state threaded through each call. The
one-shot-closure half cannot be constructed against the current tree: [Scalar
Snapshot Closures v2](CLOSURES-V2.md) states plainly that its profile "does
not admit owning captures," so every closure value in this language today is
a Copy scalar snapshot that can be invoked any number of times — there is no
one-shot (consuming) closure construct to attempt reuse of. Introducing one is
exactly [SPX-AI-021 (issue #120)](https://github.com/wavect/semaprax/issues/120)'s
scope ("bounded owning-capture closure slice"), which issue #121 explicitly
does **not** list as a prerequisite (only #119 is listed) and whose own
bounded-scope section directs implementers *away* from owning captures in
this profile ("do not smuggle mutable captures into the one-shot closure
profile... use a checked reusable scalar callback or explicit owned state
threading"). This profile follows that direction. Once #120 lands, an
owning-capture variant of these adapters (and a source-level reuse-refusal
test against it) becomes constructible and should be added as an additive
extension of this document, not a revision of it.

## #120 assessment: not a real blocker for this issue

[Issue #120](https://github.com/wavect/semaprax/issues/120) is a separate,
independent feature (owning-capture closures) that #121's own issue text does
not list as an implementation prerequisite — only [issue
#119](https://github.com/wavect/semaprax/issues/119) is listed. #121's own
bounded-scope text is written to be satisfiable *without* #120 (state
threading or reusable scalar callbacks), which this profile does. #120 is
therefore an **assumed**, not a real, blocker for #121; the real blocker this
session found is documented next.

## What remains (the real blocker, and why it is not #120)

Composing these adapters over the owned-record collection element from
[SPX-AI-020/issue #119](OWNED-RECORD-COLLECTION-ELEMENT-V1.md) — the
"owned-data pipeline" framing in #121's own outcome statement — is **not
implemented and is blocked**, but not by #120. `OWNED-RECORD-COLLECTION-ELEMENT-V1.md`'s
own "What remains" section states plainly that consuming
traversal beyond bulk `clear` is not admitted for that element at all:
`vec_into_iter` is refused for `Vec<Item>` because
`iterator_ops::resolved_element_is_admitted` (this file) delegates to
`crate::vec_ops::resolved_vec_element_is_admitted`, which is deliberately
**not** widened to recognize the owned-record carrier
(`hir::owned_record_collection::is_owned_record_vec_type` is a separate,
front-end-only predicate by design, to avoid the backend-accident hazard that
document explains in detail). Without `vec_into_iter<Item>`, there is no
`Iter<Item>` value for any adapter in this document to consume; building one
requires widening the shared element-admission predicate consistently across
the ~25-40 HIR/cleanup/native/Wasm call sites that document's "SPX-AI-020
execution decision and status" section enumerates — every one of which is
outside this session's file lease (`src/hir/**`, `src/cleanup_plan/**`,
`src/interpreter*`, `src/codegen/**`, `src/wasm/**`). This is a real,
structural gap, not a policy choice this session made.

A second, independent structural finding, confirmed empirically this session
(see "What this profile does **not** add" above): even once `Iter<Item>`
exists, a genuinely step-wise, multi-pull, **element-type-changing** lazy
adapter (a `map_next` returning `IterStep<U>` from an `IterStep<T>` while
preserving a live, still-lazy remainder, or even a same-element-type
`filter_next` that skips more than one rejected element per call) is not
constructible against today's `Iter`/`IterStep` prelude and generic-function
admission rules at all, for any element type, scalar or record. Two
independent restrictions each rule it out on their own:

- `iterator_ops::step_shape` (this file) requires a `Yield` case's `rest`
  field to have type `Iter<T>` with the *same* type-parameter index as
  `item` — the prelude's `IterStep<T>` cannot be reconstructed with a
  `U`-typed item and a `T`-typed remainder, ruling out element-type-changing
  step adapters specifically.
- Independent of element-type-changing: a generic function cannot loop
  internally with a `match` in its own `while` body (`SPX-T252`) and cannot
  recurse into itself (`SPX-T226`, "participates in a recursive call
  cycle"), so *no* generic function — element-type-changing or not — can
  perform more than one internal `iter_next` pull per external call. This
  rules out even a same-element-type `filter_next` that skips multiple
  rejected items in one call; `first_if` above is deliberately a
  single-pull adapter for exactly this reason.

This is why `map_filter`/`filter_fold`/`map_fold` above are fused,
fully-draining single-pass functions rather than step-wise adapter objects: a
composable, resumable, multi-pull lazy adapter requires either a new carrier,
a relaxed `step_shape` rule, or admitting `match`-in-`while`/generic
self-recursion — all HIR/parser/backend surface outside this profile's
authored-source-only scope and file lease.

Neither gap is a "maintainer review" question this profile defers on its own
authority — issue #121's review checkpoint asks the *implementing* agent to
submit a bounded design and negative tests before broader work, which is what
this document and its tests are.

## Test discovery

`cargo test --locked -p semaprax --lib iterator_ops` selects, among the
existing `iterator_ops` suite, the three new
`src/iterator_ops/lazy_adapter_tests.rs` cases:
`fused_filter_fold_runs_at_the_existing_iterator_and_cleanup_schema`,
`fused_filter_fold_rejects_a_hostile_transition_order_reorder`, and
`fused_filter_fold_rejects_reuse_of_the_transferred_iterator`.

`cargo test --locked --test owned_data lazy_iterator_adapters` selects the
seven `tests/owned_data/generic_owned_function_runtime/lazy_iterator_adapters.rs`
cases, which execute the canonical example
(`examples/lazy-iterator-adapters.spx`) and each required scenario across the
interpreter, native C11 O0/O2, and Core Wasm via the existing
`generic_owned_function_runtime::collections` harness (the same
interpreter/native/Wasm/owner-settlement machinery
`iterator_operations.rs` already uses; no new backend host or runtime
assertion is added).
