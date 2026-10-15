# Review of #830 (source only, nothing executed)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

## Blockers

None confirmed from the pasted text. The fast-path predicate and the work-charge sequence both look correct. The items below are the closest to blockers.

**1. Memo latching differs between the fast path and the general producer (`closure`, fast-path `Err` arm vs. general-path `?`).**
- A `share` failure calls `reject_after_error()`, but every `budget.charge(...)?` and `reserve_*?` in the general path returns without latching.
- If `factored::equivalent` ever reuses a memo after a non-latched error, behavior depends on which branch failed. That includes the cursor charge at the top of the loop.
- Your stated invariant is "all errors latch failed". Either every `closure()` error latches, or the caller must provably drop the memo on any `Err`. Only the second is visible, and only in prose.
- Confirm that `insert`/`authenticate` latch internally, and that the `Err` from `closure()` always makes the caller stop.
- A test of "memo state after error" compared with the old producer would expose the difference.

**2. Allocation frontiers intentionally diverge from the old producer.**
- The old path charges `reserve_btree_node` per new entry, interleaved after each route-addition charge, plus `rc_backing_bytes` for the header. The fast path charges one `size_of::<Closure>() + size_of::<Result<..>>()` fee after all work charges.
- Work-frontier equality (same `Diagnostic` at each work limit) should hold, because the sequence is N×`1` followed by `len+1`, which matches the old order.
- Any allocation-limited input that previously refused with "factored closure node" or "shared header" now succeeds or fails with a different diagnostic.
- You say this is intended. The control that compares against the old producer must therefore restrict equality to work-limited budgets, and an allocation-limited test should document the changed outcome. Otherwise it contradicts the "preserve every existing phase/order" claim.

## Semantic checks that pass

- **Predicate:** `node != sink && moves.is_empty() && epsilon.len()==1` yields exactly the child map. The general path would insert nothing for the node itself and add `0 + routes` once per unique key, so no overflow is possible.
- **Excluded shapes:** duplicate edges (`len==2`) and moves or sink fall through to the general path.
- **Self-loop:** a node whose only epsilon target is itself finds no closure entry and gets the cycle error, same as before.
- **Insert guard:** the `contains_key` check at pop prevents a double insert for diamonds.
- **Borrowing:** `inner` borrows `self.closures`, and `share` returns an owned `Rc` with no tied lifetime. `reject_after_error` after that is fine under NLL. Both `self.automaton` and `self.closures` are borrowed as disjoint fields.
- **Drop depth:** the value type is `BTreeMap<usize,u64>`, which holds no nested `Rc`, so a 64-long alias chain does not cause recursive drop.

## Uncertainties and smaller gaps

- **Misleading comment/label in `share`.** `reserve_bytes(function, 0, "…authentication")` is a budget call. The helper has no memo and cannot authenticate it. Real authentication happens at `closure()` entry via `self.closures.authenticate`. Rename the label or the comment so tests don't assume the memo is checked at this point. The extra `reserve_bytes(0)` is harmless only if it is a no-op for `LegacyUnobserved` and for the foreign/stale-budget cases. If it can fail in those cases, the error text and latching need a test.
- **Fee shape.** Charging `size_of::<Closure>() + size_of::<Result<Closure, Diagnostic>>()` models two slots: the local and the carrier. The code has three Rc-sized moves: the helper's `shared`, the `Ok(shared)` payload, and the caller's `shared` binding. The `Result` size is dominated by `Diagnostic` and may be niche-optimized. Confirm that "exact named receiving Rc + actual Result" is meant as a conceptual fee and not a literal stack measurement. If it is literal, the test should compute it from the same types.
- **Fee timing.** The fee is debited before `Rc::clone` and before `insert`. Whether `insert` separately pays for the chunk or slot allocation is not shown, and that decides whether the first-use chunk is funded. This is unverifiable from the paste.
- **Double counting.** Memo or physical-reduction accounting that sums `rc_backing_bytes` or map bytes per entry would count aliased Rc backing once per alias. Check that the census and "physical reduction" assertions use pointer identity, not per-entry sums.
- **Final return.** `self.closures.get(&root).cloned()` is an unaccounted Rc clone. That is consistent with "no new backing", but it runs after `failed` could have been latched, in which case `get` returns `None`. The later `Ok(None)` is then indistinguishable from route overflow. Check whether the caller treats `None` as an overflow verdict after a latched failure. All error paths here return `Err`, so this is probably safe, but a test should pin it.
- **Misleading error text.** If `failed` is latched, `get(&state.epsilon[0])` returns `None` and the code reports "cleanup CFG contains a cycle". That is a wrong diagnostic, but unreachable if the latch always returns `Err` first.
- **Type alias.** `Closure` in the helper duplicates `Rc<BTreeMap<usize,u64>>` in the caller. It matches today, but nothing enforces it. A type-equality assertion in the tests would help.
- **Not checkable here.** The `tests` module, the `Memo` internals (`authenticate`, `insert`, `reserve_vec` geometric payment), `pays_allocations`, the a33 archive equivalence, and the six controls were not pasted. I make no claim about them.
