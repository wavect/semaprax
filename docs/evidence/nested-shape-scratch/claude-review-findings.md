# claude review findings (nested-shape-scratch)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Source-only review of the pasted code. Nothing was compiled or run. I found no hard type or lifetime blocker, and typed derivation is not skipped. I did find two control gaps and a few items I can't verify from the paste.

## Confirmed from the source

- **Typed derivation still runs per root.** `derive_reusing` walks the full type every call. There is no memo, no cache key, and no result reuse. Only the `frames` and `shapes` backing is retained.
- **Payer.** `PaidShapeReceiver` holds the single `&'a mut ReplayBudget` for its whole life. `derive` and `budget()` both reborrow that same field, so there is one payer and no second source of authority. The `let budget = shape_receiver.budget();` shadow reborrows the same payer, and NLL accepts it because `shape_receiver` is only dropped after the last use.
- **Per-root state.** `Budget::default()` is fresh per root, and `next_flag` is shared across roots.
- **Error order.** `reserve_clone` of the root happens before the lease exists. `reserve_vec(frames, 1)` and the first `Enter` come after it.
- **Drop order.**
  - `Lease::drop` clears `shapes` then `frames`, and the `Scratch` field order is `shapes` then `frames`.
  - Per-iteration locals (`meta`, `entries`, `frame`) drop before the lease, as they did before.
  - The clear also runs on unwind and on every `?` path, so the next root starts from empty vectors.
- **Types and lifetimes.**
  - The `Lease` borrows end before the `Lease` drop, so no conflict arises.
  - The `pub(super)` and `pub(in …replay)` re-exports are narrower than or equal to the originals.
  - The `use super::*` globs resolve through ancestor privacy.
  - `(0, &mut Budget::default())` in the tail expression borrows nothing that outlives it.

## Control gaps

1. **Retained backing outlives the shape phase.** `shape_receiver` stays alive until the end of `validate_inventory_coverage`, because `budget()` is borrowed from it. The `frames`/`shapes` capacity therefore stays live through the whole supplemental walk and `receiving::Scratch::new`. Before, it was freed at the end of each derive.
   - If `ReplayBudget` or the physical oracle tracks live or peak bytes, this changes the peak and the refusal point.
   - If accounting is cumulative-only, it is harmless.
   - **Fix:** scope the receiver in a block, and take `budget` back from the parameter after the block, so it drops before the supplemental phase.

2. **Boundary parity is not shown.**
   - Reuse means roots after the first pay nothing for frame/shape growth already held. Growth that does occur pays the full new target with no credit for the old capacity.
   - For a budget set near the threshold, the point and phase label of the first refusal can therefore differ from the old per-root allocation.
   - **Needed:** a test against `before` that sweeps the budget across the exact refusal boundary, covering success versus refusal and the refusal text for multi-root inventories. Otherwise "full error priority" is only argued, not demonstrated.

## Unverifiable from the paste

- **Dead code.** If `paid::derive` (the non-reusing wrapper) has no non-test caller now, `dead_code` becomes an error under `-D warnings`. Check that, and that `PaidShapeReceiver::budget` is used in non-test builds. It is, via the `let budget` line.
- **`storage::push` and `storage::array`.** I can't see how `push` grows `shapes`. If it grows differently from `reserve_vec`, retained capacity may not match the charged `target`. Also, `reserve_vec`'s check that `capacity() == target` after `try_reserve_exact` assumes exact allocation. That behavior predates this change, but retained vectors make a mismatch easier to hit.
- **Clippy.** `derive_reusing` has exactly 7 parameters, which is at the `too_many_arguments` limit and passes. `large_enum_variant` on `Frame` depends on the size of `ResolvedType`, which I can't see.
- **Detached payer.** `Scratch` carries no payer identity, so the pairing of scratch and budget is enforced only by `PaidShapeReceiver` encapsulation. `Scratch` is `pub(in …replay)` with `Default`, and `derive_reusing` is `pub(super)`. That is acceptable while only the receiver calls it.
- **Depth.** The receiver hardcodes depth `0`. Confirm that matches the original caller.

## Scope of the saving

Only the two stack backings are reused. Per-node `meta`, `entries`, `children`, `expected`, substituted type clones, and the lifecycle-identity bytes are still allocated and paid per node. This removes two repeated allocations per root and nothing more.
