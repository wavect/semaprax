# Review of `canonical_shared.rs` at 93fdc103 (source only, nothing executed)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Line numbers are approximate, counted from the pasted excerpts.

## Blockers

I found no proven semantic or work regression against `canonical_before.rs`.

- **Work debits** are identical. `share_weighted` charges `len+1` before its zero-byte authentication, the same as `clone_weighted`. Every other `charge` call is byte-for-byte the same.
- **Debit order** is correct. `own_weighted` reserves the header before `Rc::new`, and the vector is reserved before `collect`.
- **Identity and ordering** are value-based. `Rc<Vec<(usize,u64)>>` delegates `Ord` and `Eq` to the vector.
- **Cycle detection** compares equal values held in different `Rc` allocations, so it does not depend on pointer identity.
- **Drops**: error returns drop frames, `open` and the memos by ordinary ownership. I see no leak, double-free or cycle in the `Rc` graph, because the `Rc`s are acyclic and immutable.

## Probable accounting defect (depends on std behavior, unverified)

**`successors`, ≈L180–186.** The charge is `reserve_array::<(usize,u64)>(targets.len())`, followed by `targets.into_iter().collect()`.

- `BTreeMap::IntoIter` is not `TrustedLen`. As I recall std, the generic `Vec::from_iter` allocates `max(MIN_NON_ZERO_CAP, lower+1)` elements, which is 4 for 16-byte elements.
- A target set of 1–3 entries would then allocate 64 bytes while charging only 16–48.
- The comment above `own_weighted` ("caller has already paid the moved vector's complete allocated backing") would then be false. The `Rc` header is paid on top, but the vector's capacity slack is not.
- The old receiver had the same under-charge, so this is not a regression. It does contradict the stated goal of full-capacity backing.
- **Fix:** build the vector with `Vec::with_capacity(targets.len())` plus `extend`, or charge `capacity()` after the fact. Charging after the fact breaks the debit-before-allocation rule, so the first option is better.

## Uncertainties (the pasted source cannot settle these)

1. **Zero-byte reserve bypasses `pays_allocations()`.**
   - `share_weighted` calls `reserve_bytes(0)` unconditionally (≈L38). Every `reserve_*` helper, including the old `reserve_clone`, returns `Ok` early when `pays_allocations()` is false.
   - If any payer state is non-paying but has `SelectedReservation` with `physical == None`, the `.expect("selected reservation payer")` would panic. The old path would not have.
   - `own_weighted` calls `reserve_bytes` with positive bytes without the gate in the same way.
   - I can't see `pays_allocations`, so I can't prove this is reachable.
2. **Physical charge parity is dropped silently.** The old path ran `prepare_clone_cursor` and `clone_payload_bytes` through `reserve_clone`. The new path does neither. Under the `ActiveBuilder` payer the total charge therefore differs from the old one: a missing cursor reservation, a different pattern of refusal points, and an Oracle-lease work charge that is no longer taken. That may be intended. Oracle routes to `borrowed_canonical` in `compare_with_index_storage`, but unit tests call `Canonicalizer` directly. Nothing asserts physical-charge parity.
3. **`RcBacking<T>` layout.** It mirrors std's `RcInner` only by assumption. The `repr(C)` and two-`Cell<usize>` layout is reasonable, but no test pins it other than the loose inequality below.
4. **Unseen helpers.** I can't see `replay_error`'s code, `with_physical`'s limit semantics, or what `reject()` sets.

## Test weaknesses (omissions, not defects)

1. **The physical assertions are inequalities.**
   - `shared_full_backing_*` asserts `allocated <= charged` and `highwater <= charged`.
   - `reserve_btree_node` uses `btree_node_upper`, which charges per insert and is pessimistic. That slack can hide the vector-capacity under-charge above, or a missing 40-byte `Rc` header.
   - No component-level equality exists, such as a single `own_weighted` call measured against `rc_backing_bytes + capacity*16 + 24`. This is the biggest gap for the "full backing" claim.
2. **Cycle probe assertion (likely test bug).**
   - `probe` does `assert_eq!(error.code, "SPX-H006")` for every `Err`, but the cycle case (`roots[6..]`) returns `replay_error("cleanup CFG contains a cycle")`.
   - If that error's code is not SPX-H006, the cycle probe panics inside the test. If it is, the test cannot tell cycle from budget exhaustion.
   - Either way the check is mis-targeted; separate the codes.
3. **Refusal is swept only at exact−1.**
   - Only `admission` and `admission+charged-1` are tried. Failures between the vector allocation and the header debit, mid-`successors`, mid-frame push and mid-exit are not swept.
   - A limit sweep over `[admission, admission+charged)` asserting `live==0` and `allocated==freed` would cover the error-drop paths.
4. **Overflow coverage is only the `checked_mul` path** (`roots[3]` with `u64::MAX`).
   - Not covered: the `closed` addition overflow, the successor-target `slot.checked_add(*weight)` overflow, and closure route-count overflow.
   - The `None` return after partial `open` and memo state is untested for those paths.
   - The test also does not check that `states` and `ids` remain consistent after `None` or a cycle error and the next `id` call. Memos persist across calls and ids use `self.ids.len()`.
5. **Parity tests use `ReplayBudget::new()` only.** Work and remaining are compared under the unobserved payer. Parity under `ActiveBuilder` or `SelectedReservation` is untested, which is where sticky refusal matters. Only the unit test `immutable_shares_*` touches `SelectedReservation`, and it does so without `canonical_before`.
6. **No dedup and sharing check.** Nothing asserts that equal-valued but pointer-distinct children collapse to one memo entry. It also doesn't assert that the `Rc` strong counts return to 1 after the run. `Rc::ptr_eq` is asserted only on a trivial share.
7. **`states()` comparison is one-directional.** It looks each old key up in `current`, with equal lengths. That is adequate, but a differing state ordering is not checked.

## Minor

- Eager `Rc` header payment for children that are later skipped (already memoized) is extra debit relative to the old receiver. This is acceptable under the "no refunds" premise but is a charge change that the work-only parity tests don't show.
- `own_weighted` keeps the vector payer-independent of `Rc::new` success. Allocation failure aborts, so that is fine.

## Summary

The shared implementation preserves algebra, work units and value-based identity relative to the old receiver. The one concrete accounting hole is likely collected-vector capacity slack, which neither implementation charges. The tests' loose upper-bound physical assertions would not expose it.
