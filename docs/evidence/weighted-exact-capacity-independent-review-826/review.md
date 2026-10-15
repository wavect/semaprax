# Review of OPT826 `97480a13`: weighted successor capacity

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Source-only review. Nothing was executed. I found **no blocker** in production.

## Production

- **Capacity (`canonical_shared.rs:32-43`).** `Vec::with_capacity(n)` for `(usize,u64)` allocates exactly `n` slots, and `extend(targets)` cannot reallocate. `Vec::extend_desugared` only reserves when `next()` returns `Some` while `len == capacity`. The `BTreeMap` iterator yields exactly `n` items, so that branch never fires. This fixes the inherited `max(4, len+1)` collect growth, which the old receiver keeps on purpose.
- **Debit before allocation.** `reserve_array(targets.len())` runs before `with_capacity`, and the Rc header is reserved before `Rc::new`. On refusal at the header, the funded `state` Vec drops through `?`. That matches the test's `allocated == freed == vector` expectation.
- **Width 0.** `reserve_array(0)` goes through the zero-byte path in `reserve_bytes`, which still consults sticky refusal. `with_capacity(0)` allocates nothing, so the Rc is the only allocation.
- **Header size.** `RcBacking<Weighted>` is `repr(C)` with two `usize` counters and a 24-byte `Vec`, so 40 bytes. That is consistent with `Rc`'s inner layout. The Vec header is correctly counted inside the Rc payload, not separately.
- **Algebra.** The iteration order and the `Rc<Weighted>` `Ord` comparison are both by inner value, so state identity and signatures should be unchanged. Work debits are unchanged: `weighted_successor` adds no `charge`, and `share_weighted` keeps `len+1`.
- **Physical-only difference.** Physical bytes now differ from the old receiver in two ways. The old receiver paid per-clone `reserve_clone`, while current pays an Rc header per successor. Tests compare only work and ids against the old receiver, which is the intended scope.

## Test findings

None of these is a blocker. Items 2 and 3 are the ones I would act on.

1. **Unit control is strong (`tests.rs:217-296`).** It checks `capacity()`, `observed.allocated == vector + header`, `reallocations == 0`, and the zero/short/exact/Rc-one-short frontiers at widths 0..=4. This is the real evidence for the fix. The assertions that depend on helpers I cannot see are `observed.reallocations`, `error.code == "SPX-H006"`, and how `with_physical` treats the limit as a total. These are consistent with other uses in the file, but I cannot verify them.

2. **The "full old/current" control probably does not reach the claimed widths (`tests.rs:299-337`).**
   - **No width assertion.** The test never asserts the produced target-map width, so it is unverified. It builds `width` targets via `automaton.end(sink, Failure)`. If `end` or `movement` dedups or memoizes per `(sink, terminal)` (cannot tell from the supplied source), every width collapses to 1.
   - **Width 0.** The root has no moves and is not the sink. `closed` is empty, so `successors()` never calls `weighted_successor`. Width 0 is effectively a no-successor automaton.
   - **Overflow claim.** With one `end` target per node and a single label, each target key is reached once. The `u64::MAX` weight therefore never hits `checked_add` overflow in the subset construction. The handoff's "repeated weights and checked overflow" is not exercised here, though `roots[3]` with `u64::MAX` in the first test does cover `routes.checked_mul`.
   - **Fix.** Assert the successor width, for example via `old.signatures()` key lengths or a direct helper. Add a duplicate-child edge (two moves with the same label to the same child) to hit `checked_add` overflow.

3. **No physical integration control for widths 2..3 through `id()`.** `probe` uses `ReplayBudget::with_physical`, but its sequence only produces width-1 target maps. The two-label `roots[4]/[5]` successors have one target each. Widths 2..4 run through the work-only `ReplayBudget::new` test, which has no allocation observer. Both unit and integration claims therefore hold only jointly. If `successors()` stopped using `weighted_successor`, nothing physical would catch it. The probe's `allocated <= charged` is also a loose upper bound, so btree-node slack could mask a 16→64 byte regression at width 1. Adding a width-3 target automaton to `probe` would close this.

4. **Observer scope is fine but subtly asymmetric.** `targets` and the refusal text are built outside `measure`. Their frees inside `measure` are untracked pointers, which the observer ignores (`record_free` returns early on an unknown pointer), so `allocated == freed` still holds. `error` is dropped inside `measure`, as intended. The same argument applies to the `BTreeMap::new()` second call.

5. **Sticky zero refusal.** Zero-byte refusal is tested only through `budget.reserve_bytes` and the `reject()` test. For the `SelectedReservation` payer, the second `weighted_successor(BTreeMap::new())` relies on `PhysicalReservation::reserve(0)` being sticky after refusal, and that code is not in the supplied source. The `ActiveBuilder`/`Oracle` branches check `refusal_spent` directly. I assume the `reject()` test reflects reality, but this is an omitted-context uncertainty.

6. **Restored cfg(test) root `CanonicalFrame` (`factored.rs:802-806`).** It is shadowed by the local enums in `canonical_before.rs` and `canonical_shared.rs`, and the glob import makes that legal. The handoff says the borrowed receiver still uses it. If that receiver is absent in some cfg, a `dead_code` warning is possible. Not a correctness issue.

## Omitted context (not verifiable here)

- Callers' prepayment of the root vector (`own_weighted` comment, `canonical_shared.rs:15-17`); production call sites are not supplied.
- `Automaton::end` and `movement` dedup behavior, which decides item 2.
- `reserve_vec` growth policy for `frames` and `stack`.
- `Label::clone` heap use in `successors` (`label.clone()`), which is unpaid in both old and new code and so is inherited, not a regression.

## Verdict

- **Fix correctness.** The fix looks correct for `Vec` capacity and for debit-before-allocation ordering at widths 0..4. The Drop, refusal, and sticky frontiers are well covered by the direct unit control.
- **Weak spot.** The "full old/current" control overclaims. It likely never reaches widths 0 or 2..4 as live target maps, and it does not exercise the `checked_add` overflow in successor accumulation. Tighten it before treating it as evidence of width coverage.
