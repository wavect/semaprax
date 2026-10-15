# Review of the materializer and `ordered_union` (source only, nothing executed)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

I found no definite blocker in the pasted code. The risks are uncertainties that depend on APIs and tests I can't see.

## Likely correct
- **Heap logic:** `fill` and `next` look sound. The sift-up/down comparisons are consistent, `swap_remove(0)` followed by sift is correct on a 1-element heap, and `heap[0] = &first[1..]` keeps the `'a` lifetime.
- **No growth after prepayment:** pass 2 reuses the drained heap, which has capacity `count` and holds at most `count` entries, so it never reallocates. `output` is pushed at most `unique` times.
- **Non-strict rows:** the malformed test uses `>`, so rows with equal adjacent IDs take the fast path. The k-way dedup handles them, and the `<=` debug assertion matches.
- **Malformed rows:** these fall back to the paid-AVL path in the same order as the old code. The old `live_before`-then-`live_after` ordering is preserved, and `into_vec` matches the old `set_vector`.
- **Borrow and move:** `edge_live` is only borrowed until `zip(edge_live)` consumes it. In `merge`, `rows` is moved only inside the returning `if` branch, so the later uses compile.
- **Scaling:** the cost is O(total·log k) per pass, which is not worse than the old O(total·log total) AVL path.

## Uncertainties to confirm (lines refer to the new code)
1. **`live_after` iterator path.**
   - The old code used `cfg.successors(node)`, which yields `to`. The new code uses `cfg.successor_edges(node)`, then `cfg.edges[edge as usize].1`, then `binary_search` again.
   - This is only equivalent if `successor_edges` yields indices into `cfg.edges` (`u16` or `usize`, since `edge as usize` compiles either way). If it yields something else, the result is silently wrong.
   - The second binary search is redundant if it does yield edge indices; you could use `edge` directly.
   - The `merge` bound `impl Iterator + Clone` requires `successor_edges`'s return type to be `Clone`. If it is a non-`Clone` adapter, this fails to compile.
2. **Sticky-error identity.**
   - `payer.reserve(0).map_err(|_| storage::marker())?` (at the start of `merge`, in both drain loops, and at the top of `materialize`) discards the reservation's error.
   - The old `Set::new(payer)?` and `vector(..)?` calls propagated the `Diagnostic` directly.
   - If `storage::marker()` is not identical to the first sticky error, the "first sticky error" requirement is violated. Confirm that `marker()` is what a sticky failure maps to elsewhere.
3. **Scratch accounting.**
   - The heap is prepaid at `count × size_of::<&[LoanId]>()`, but it is dropped at the end of every `merge` call (twice per node).
   - If `CfgReservation` never refunds, that charge accumulates over nodes and can trip refusal earlier than the old path in dense CFGs. That would be an accounting regression, not a correctness bug.
   - Whether the old AVL node charges were also cumulative isn't visible, so I make no claim either way.
4. **Unpaid clones.** `point.clone()` (endpoints), `cfg.points[..].clone()` (`ends`), `draft.site.clone()` and `draft.cause.clone()` are not routed through the payer. If any of these types owns heap data, they are unpaid allocations. The old block had the same `point.clone()`, so this looks pre-existing. I can't tell whether the `Loan` field clones are new.
5. **Empty-row fast path.**
   - `vector(0, payer)` is said to pay the Vec header. The old path went through `Set::new` and `into_vec`.
   - Confirm both charge identically for the empty case, since `from_vec_paid` follows.
6. **Generic bound on `vector`.** `cfg_directory::vector` is now instantiated for `Vec<&[LoanId]>`. If it is bounded (for example by a `Paid` trait or `T: Copy`), this may not compile.
7. **Error order at refusal.**
   - Both merges now run before the `starts` and `kills` clones. The old code interleaved `live_before` into-vec, `live_after` inserts and the clones.
   - A refusal can now land at a different step. This is fine if only the Err category matters, but exact charge-boundary tests will shift.
8. **Pre-existing truncation.** `LoanId(index as u16)` truncates silently when `drafts.len() > 65536`. This is unchanged, but the "at most u16 domain" comment on `unique` depends on it being enforced upstream.

## Tests
- `before` (the old implementation, `cfg(test)`) and `tests` weren't pasted, so I can't verify coverage.
- A good suite would cover, in both `live_before` and `live_after`:
  - an empty rows iterator and all-empty rows;
  - duplicates across rows and within a row;
  - a single row;
  - a malformed unordered row, compared against `before` with identical output;
  - refusal at each payment point, checking that no allocation happens after the refusal.
- Release builds skip the `debug_assert`s, so the malformed-row tests are the only protection there.
