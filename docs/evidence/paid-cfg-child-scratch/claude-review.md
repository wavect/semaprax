# claude review (paid-cfg-child-scratch)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

## Verdict

I see no semantic, Drop, ordering or first-error blocker in the proposed edit, and retaining one child backing preserves old behavior except for the intended debit change. Efficiency is plausible from the source, but its magnitude is unproven, and the 11-byte margin cannot be read as evidence of it.

## Preserved (from source)

- **Child order.** `children.iter().copied()` yields the same sequence as the old `into_iter()`, so `pending` receives identical pushes. `push_children` is unchanged and only appends.
- **Types and borrows.** `Vec<&'a ResolvedExpr>` clears and refills without extending any lifetime beyond `'a`. Elements are `Copy` references with a trivial Drop. Nothing escapes, because `children` is dropped before the point allocation and never enters `Cfg`.
- **Error placement.** If you keep the scratch use after `push(expressions)`, the duplicate-identity error and the `charge` error still precede any child work. Every error path `?`-returns, so a half-filled scratch is never reused. The `clear()` calls are therefore redundant but harmless, and they enforce the invariant that scratch is empty at iteration start.
- **Capacity behavior.** `Vec::clear` keeps capacity. `storage::push` grows only when `len == capacity`, so the retained backing causes growth only when a child count exceeds the previous maximum. Failure paths (`array`, `try_reserve_exact`, capacity mismatch) are identical, just reached less often.
- **Drop and release.** Nothing in the shown code releases debit on free. `storage::push`, `vector` and `Map::insert` only debit. So "drop before points" has no payer effect and matters only for physical RSS. Early-return Drop of the scratch is plain `Vec` dealloc.

## Concrete issues to control

1. **Debit is not rewritten, so first-failure position moves.** `classify` gives physical failure strict precedence over semantic whenever `payer.first()` is set. Per-iteration `fresh()` (24 bytes via `size_of::<Vec<T>>()`) plus growth from 0 (min cap 4, then doubling) disappears, so the new cumulative debit is at every step ≤ the old. Consequences:
   - A budget that old failed physically may now succeed, or reach a later semantic error.
   - A budget that new fails physically must also fail in old.
   
   Do not compare error kinds under short budgets for equality. Assert monotonicity instead: new success ⇐ old success, and new physical ⇒ old physical.

2. **One-time scratch allocation moves a 24-byte reserve ahead of the first `charge`.** Old order in iteration 1 was `charge`, then set/map/push, then `fresh`. New order is `fresh` (before the loop), then `charge`.
   - With remaining < 24, new fails physically at the scratch where old might have returned a semantic work-limit error first.
   - Old would still fail physically at `set_insert` (≥ 2×node_upper) in the same iteration, so the difference only appears when the work counter is exhausted at iteration 1.
   - You need a boundary test for this. If you want zero ordering drift, make the first scratch creation lazy at the old `fresh` position. The cost is one `Option` check.

3. **`reserve(0)` replaces `fresh`'s sticky check.** It gives equal sticky authentication, but not equal exhaustion detection. Old `fresh` could fail on remaining < 24, whereas `reserve(0)` cannot. That is the intended change, but it means any existing test pinning the "remaining < 24" failure shape must be rewritten.

4. **Physical peak.** The retained backing stays live while `pending`, `expressions`, `seen` and `root_by_expression` keep growing. Old freed it each iteration. Peak live heap can rise by up to max-child-capacity × 8 bytes versus old at the `pending` growth peaks. Cumulative debit falls, but if any physical or peak-live limit exists (distinct from the cumulative reservation), check it. Your test allocator should record peak live as well as total.

5. **Forecast and prebound constants.** If Catalog95/prebound (51,697,600) or any exact/short/sticky test derives from the old per-iteration charge model, the forecast remains a sound upper bound by monotonicity but is no longer exact. Any test asserting equality of actual and forecast, or exact legacy consumption, will break. Update the model, not the caps.

6. **Pass/fail at the margin.** Remaining 11 against a 64-byte reserve means any debit reduction flips this case. That tells you the edit can pass this fixture, not that it is the principal cost. See the justification below.

## Minimal required controls

- **CFG equality.** Compare `points`, `expressions` (order and identity), `edges`, `nodes` lookups and adjacency against the old `paid_cfg` and the saved `build_cfg`. Use ample payer for equality, and compare the saved `Cfg` through its fields (it has `successors`/`predecessors`, not `Adjacency`).
- **Ample-budget first errors.** Cover duplicate expression IDs (at the first, middle and last duplicate), `charge` failure at every k within a small function, node_count > endpoint bound, and edge bound. Error text and kind must be equal.
- **Debit accounting.** Using the actual allocator, assert a per-iteration identity: old debit − new debit = Σ(24 + old growth bytes) − (24 + new max growth bytes). Run it over exact, short (budget = exact − 1 at every debit boundary within a small function) and sticky cases, through Drop. The allocator must confirm all heap is returned on every error path, including the retained scratch.
- **Shape coverage.**
  - Wide: arity ≫ 4, so growth happens once at 4, 8, 16, …
  - Deep chains: child count ≤ 1.
  - Alternating wide/leaf/narrow: capacity retained, no re-growth.
  - Forced spill in `pending`, with the scratch growth reserve interleaved.
- **Sticky behavior.** After a physical failure at each distinct site (scratch creation, scratch growth, `pending` growth, `set_insert`, `map_insert`), assert that `classify` returns `Physical` and that no later semantic error masks it.
- **Boundary at scratch creation.** Remaining = 23, 24, 25 before the loop, with the work counter at limit.

## Overlooked scopes

- `lexical_exit::destination(expression, &expressions, work)` is called inside the edge loop. Its allocation behavior is not shown; confirm it does not allocate per call from `payer`.
- `evaluation_children` in `paid_cfg` is used as an iterator (the saved version returns a `Vec`), so the paid version is not the saved one. Confirm it allocates nothing, since an allocation there would be an unpaid heap acquisition.
- `paid_cursor::Cursor` already retains its spill across iterations, so the proposal makes `paid_cfg` consistent with it. Check that both use the same child order, which your whole-CFG equality test will cover.
- Edge-phase `edge_set.into_vec`, `nodes` and `adjacency` costs are separate. Do not fold them into this change.
- The J115/G171 attribution loss is independent of this edit. It does not change correctness here, but it remains an unresolved defect.

## Is the efficiency justified from source?

Only in principle. The code shows that debits are cumulative and unrefunded: `storage::push` never releases. So per-iteration `fresh` costs 24 bytes plus a first growth of at least 4 × 8 = 32 bytes per expression with children, and more for wide expressions. Retention removes that to a one-time cost. That is a genuine, source-visible reduction in debit.

Three things are not shown:
- Expression count per function in the 10.9M and 3.57M phases. Without it the saving, at roughly 56 bytes (more for wide expressions) × N, cannot be compared to the 51.7M forecast.
- The `CfgReservation` internals, to confirm "cumulative, no refund".
- Whether the discovery loop bound is below the expression limit, since the 4,096-endpoint check happens after discovery.

So the edit is justified as a monotone debit reduction with no semantic change. Whether it is material to this catalog is not established by source and should not be claimed.
