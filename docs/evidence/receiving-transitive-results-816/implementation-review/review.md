# Review of the new caller-paid place-result helpers (source only, nothing executed)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

The paste has no line numbers, so I cite code fragments instead. I found no definite blocker. I did not compile anything, and several conclusions depend on types and helpers that are not shown.

## Semantics (checked against old `under`)

- **Ordering and matching are equivalent.** The new code walks `by_storage[storage]` in stored order and indexes `by_flag[&flag]` as the old code did. The `place.projections.len() > projections.len() { continue; }` check followed by the zip-and-compare loop matches `starts_with`.
- **Empty prefix:** the zip is empty, so every leaf matches, as in the old code.
- **Missing storage key:** the result is an empty vector, as before.
- **Error priority in the facade** keeps the old order: unknown storage, then empty result. Budget refusals can now come first, which you called acceptable. The `?` on `under_with_budget` also sits before the emptiness check, so a refusal takes priority over "no droppable leaf". That is the same natural change.

## Rust typing

- **`by_storage` value type (uncertain).** `map_or(&[] as &[LivenessFlagId], Vec::as_slice)` requires `by_storage: Map<StorageId, Vec<LivenessFlagId>>`. The old `.into_iter().flatten()` compiled for any `IntoIterator` value. If the value is a `SmallVec`, `BTreeSet` or boxed slice, this fails. Use `.map(|v| v.as_slice()).unwrap_or(&[])` or `Deref` to be robust.
- **`by_flag[&flag]`** is consistent with the old `self.by_flag[*flag]` (which indexed by `&LivenessFlagId`), so it compiles if that is a map.
- **`left.as_str()` (uncertain).** I cannot see the projection type. If it is not string-like, this fails. If equality does more than a string compare, the `min(len)+1` fee may understate it.
- **Borrows are fine.** The `ok_or_else(|| budget.budget_refusal(..))?` closures borrow `budget` immutably and end before the next `budget.charge(..)`. `state.flags` borrows `self`, not `budget`.

## Accounting

- **Carriers are plausible.** `Under`, the return `Result`, the receiving `Vec`, and the `bool` are reserved before the first allocation. The facade reserves `Result + Vec`. The callee's `Result` is the same slot as the facade's temporary, so this is conservative double counting, not undercounting.
- **Relocation fee matches `reserve_vec`.** `len == capacity` is exactly `required > capacity` for `additional = 1`. The `len+1` charge precedes `reserve_vec`, and a refusal after the charge only over-charges, which is safe. Growth rounding in `replay_growth_capacity` can exceed doubling, but the copy cost is still `len`. A fee of 1 for the first allocation at length 0 is fine.
- **Backing bytes versus work (uncertain).** `reserve_array::<T>(target)` presumably charges the new backing. I cannot tell from the paste whether it also accounts for the old backing, which is live simultaneously during `try_reserve_exact` relocation, or whether it holds the bytes until the caller drops the result. If the semantics are "check peak, not cumulative", the callee's reserves do not include the facade's frame. If you intend overlapping live bytes, confirm that `reserve_bytes` is cumulative or held-based. This is the main place I would look for undercounting.
- **Uncounted stack locals (uncertain).** Not reserved: the `zip` iterator, `projections` and `left`/`right` references, `flag`, and `units`. They are not result carriers, so they may be outside the parent's "initialized/return/receiving" requirement. State that explicitly if so.
- **`under_with_budget` does not check `pays_allocations`.** `reserve_vec` silently skips growth when the budget does not pay, but the relocation charge above it still fires based on std growth capacity. If anything outside the facade calls this on a non-paying budget, work charges are applied without byte reservations. The facade gates this correctly. Making the helper private to the facade, or asserting the invariant, would avoid misuse.
- **`reserve_vec` capacity check (existing, not new).** `try_reserve_exact` only guarantees capacity ≥ the request. Equality holds in practice for non-ZST `Vec`, but the `capacity != target` refusal depends on that, so it is a latent allocator-dependent refusal.
- **Unpaid inherited costs:** `BTreeSet::contains` in the facade, the `by_storage` and `by_flag` lookups, and `replay_error` allocation. You stated these are inherited. The `by_flag` lookup is now once per flag inside a loop, but the old code did the same.

## Work fees

- **Per-flag fee:** 3 plus 1, plus `min+1` per compared pair. A mismatch is charged for the failing element too, which is correct. A prefix longer than the leaf's projections is charged only the 4 base units, which is fine since no comparison runs.
- **No negative credit and no overflow path** worth worrying about. The `checked_add` calls are effectively unreachable but harmless.

## Tests

No tests are included in the paste. I would want these before approval:

- Differential tests of old `under` against the new helper over random places: empty prefix, longer-than-leaf prefix, shared prefix, missing storage.
- Exact-refusal boundary tests at the byte limit and at the work limit, covering the relocation-charge boundary at capacities 0, 4, 8 and so on.
- A test that sticky and active state is unchanged when the non-paying branch is taken.
- An assertion that the facade's error branches (unknown storage, empty result) drop the owned `Vec` inside the observer.

## Verdict

Blockers: none visible. Items to confirm before sign-off:

1. The `by_storage` value type.
2. The projection type's `as_str` and equality cost.
3. Whether `reserve_array` and `reserve_bytes` are held-cumulative or peak, which decides whether old-plus-new relocation backing is accounted.
4. Whether the unreserved stack locals fall outside the parent's carrier requirement.
