# Review of the redundant `seen` set removal (source-only, nothing executed)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

## Blockers (conditional on code I can't see)

1. **`Set` and `set_insert` may become dead code in non-test builds.**
   - After this diff, `paid_cfg.rs` no longer calls `cfg_directory::Set::new` or `set_insert`.
   - Only the `#[cfg(test)]` `seen_before` module still uses them.
   - If nothing else in the crate uses them, `cargo build` emits `dead_code` warnings.
   - This becomes a hard failure under `#![deny(warnings)]` or `-D warnings`.
   - Either gate them with `#[cfg(test)]`, or confirm another caller exists.

2. **The final `assert_eq!(current_metrics().allocated, at_stop)` in `observe` is wrong if `allocated` is live bytes.**
   - `at_stop` is sampled while `owner`, `error` and `retry` are all still alive.
   - Those three are dropped before the final assert.
   - If `allocated` is live bytes, dropping `owner` frees its `Rc` and the assert fails.
   - If it is a cumulative counter, the assert passes, but then the message "during Drop" is misleading, because a cumulative counter can't show frees.
   - Check the metric's definition.
   - To show that Drop does not allocate, sample cumulative allocations before the drops and compare after.

## Semantic findings

3. **The duplicate-check path no longer calls `reserve(0)`, so error precedence changes in one case.**
   - Old (`paid_cfg.rs:30`): `set_insert` on a duplicate presumably went through `Map::insert`, which begins with `payer.reserve(0)`.
   - That call authenticates the ACTIVE `Rc` and returns the sticky first reason.
   - New: `contains_key` is a pure `get` with no payer interaction.
   - If the payer were already sticky-failed or inactive at a duplicate, old returned the storage marker and new returns the duplicate-identity diagnostic.
   - On the normal path this is unreachable, because every earlier failure propagates with `?`.
   - It is only observable if `reserve(0)` can fail without a prior propagated error, such as an inactive `Rc`.
   - The intent to "preserve full error order" is therefore true only under that invariant.
   - Add a test, or a comment stating the invariant.

4. **The old and new equivalence argument depends on one invariant.**
   - It requires that `seen` and `root_by_expression` always hold identical key sets at the check.
   - That holds because the `set_insert` and `map_insert` pair either both succeed or return an error.
   - It breaks if any other code path inserts into `root_by_expression`, or removes from it, between iterations.
   - The visible prefix has none, but the elided remainder of `build` (diff lines roughly 30–385) also uses the map.
   - Confirm that nothing there inserts or removes before the loop ends.

5. **The duplicate lookup is now done twice per node.**
   - `contains_key` runs first, then `Map::insert` calls `get` again.
   - This is harmless for work units, since `charge` precedes both lookups.
   - It is a small avoidable cost. A single `insert` returning `Some` would also detect the duplicate, but it would overwrite the value and change behavior, so the current form is reasonable.

## Accounting and test findings

6. **Byte-accounting shifts are intended, but the tests must not compare physical-refusal points between old and new.**
   - Removing `Set::new` saves `size_of::<Set>` bytes of reservation.
   - It also saves `2*size_of::<Node<ExpressionId,()>>` per unique expression.
   - Under any given physical limit, old and new therefore refuse at different nodes.
   - The tests are described as measuring old and new separately, which is correct.
   - Do not assert equal first-refusal indices or equal `work.used` at physical refusal. Only the work-limit boundary tests (0..=old used) can compare them.
   - Also confirm the `old.allocations − current.allocations == unique_count` assertion applies only to successful runs.
   - On partial or refused runs the difference is not the unique count.
   - It also assumes exactly one heap allocation per `Set` node and none for `Set::new`.
   - `Set::new` is inline, per `Map::new`, so this holds, but only if `Set` wraps `Map<K,()>` or something equivalent. I can't see `Set`.

7. **The observer's `.expect("positive fixture only physically refuses")` panics on any non-physical error.**
   - That is fine for the positive fixture.
   - Make sure the malformed `right.id = left.id` fixture does not go through `observe`, because it returns a diagnostic, not a physical refusal.
   - The statement that the "64-byte actual first refusal [is] not causally attributed" is reasonable.
   - Note also that the test's `run(..., old)` retry after refusal relies on `fresh()` calling `reserve(0)` before any `charge`, so `work.used` stays unchanged. That holds for both `build` and `seen_before`.

8. **The `seen_before.rs` full copy of the old function is a drift risk.**
   - It is intentional here, but it will silently diverge from any later change to shared helpers such as `paid_children::push_children`.
   - That would make the equivalence tests compare against a moving baseline.
   - The prefix you pasted is only the discovery prefix, so I could not check that the full copy is byte-faithful.

## Nits

- **Rustfmt churn:** the `const _: () = assert!(...)` reflow is unrelated to this change. Drop it to keep the diff focused.
- **Redundant `#[path]`:** `#[path = "paid_cfg/..."]` resolves to the default location for a non-`mod.rs` module in `src/loan_plan/`. It is harmless, but redundant.

## Not verified

- I did not see the `Set`, `set_insert` or `CfgReservation::reserve` implementations.
- I did not see the new test files.
- Nothing was compiled or run. All findings above are from reading the diff and the snippets only.
