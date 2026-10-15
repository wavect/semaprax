# Review: removal of the write-only `referenced_edges` AVL set

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

I reviewed only the pasted source and did not execute anything.

## Blockers

I found none in the pasted code. I looked for a counterexample to the proof and could not construct one.

- **Cross-block duplicates.** The old set could only reject an edge when `item.id == edge`, `item.from == owner`, and the edge was already inserted. The new helper accepts an edge only if `from == owner`. The block guard rejects any block whose `id != index` before dispatch, so owners are distinct across the loop. An edge accepted for block A therefore has `from == A`, and block B ≠ A fails the `from` check. The error text is identical to the old `from` failure, so the diagnostic and its order are unchanged.
- **In-block duplicates.** Goto has exactly one edge. Branch rejects `edges.len() != 2 || edges[0] == edges[1]` before either helper call, and that precheck runs before any edge is looked up, as in the old code. Since the helper requires `item.id == edge`, two distinct IDs resolve to two distinct edges. Duplicates cannot reach the old `insert == false` path.
- **Error order.** The unknown-edge and id/from failures happen at the same point and with the same message as before. The only old-only failures were `insert(...)?` errors (budget refusals or authentication) and `insert == false`. The first is the intended removal. The second is unreachable by the argument above.
- **Typing.** `edges` is iterated by reference, so `*edge` is a copy of `EdgeId`. This requires `EdgeId: Copy + PartialEq`, which the unchanged `edges[0] == edges[1]` already needs. Direct indexing `plan.edges[edge.0 as usize]` after the helper is bounds-safe because the helper used `.get` on the same index. `Directory<EdgeId>` is no longer instantiated here, but `referenced_exits` still uses `Directory`, so the type stays in use.

## Uncertainties (not provable from the paste)

1. **Code between the guard and the `match`.** The paste shows the guard and the dispatch separately. The proof assumes nothing between them accepts, or skips the guard for, a block. A `continue` there is harmless. An early terminator-handling path that bypasses the guard would break the argument.
2. **Other callers of the old helper.** If `validate_owned_edge_paid` is still referenced elsewhere, for example in the Tree-fallback `validate_blocks_and_edges`, the deletion breaks compilation. If it is kept but unused, it triggers `dead_code`, which is fatal under `deny(warnings)`. If it is used elsewhere, that path keeps the old AVL cost, which is fine but worth confirming.
3. **Phase label in budget errors.** The old `Directory::new(..., "cleanup referenced edge directory")` ran `reserve_bytes(0)` and `charge(0)` with that phase string. The same authentication still runs at function entry, and `referenced_exits` retains it. A budget that is poisoned, foreign, or refuses at zero units would now produce a refusal naming a different phase string, or fail one call later. Whether `budget_refusal` embeds the phase text, and whether a zero-unit charge can fail at an exact-exhausted budget, depends on code I can't see.
4. **Unit accounting for tests.** Removing the probe, insert, growth, and rotation charges changes the total units for every plan that has a Goto or Branch. Any test pinning exact or one-short budgets for edge-bearing plans must be re-baselined. Zero-budget and Drop tests are likely to be affected only where the first refusal used to land inside the edge set. Your pending control comparison is the right check. Expect old/current differences in budget-refusal tests and none in non-budget diagnostics.
5. **Equal-count plans.** Plans where `edges.len() == blocks.len()` or similar cannot break the proof, because it depends on block-ID uniqueness and `from == owner`, not on counts. This depends on the edge loop's contiguity check (`edge.id == index`) running first, which it does in the shown order.

## Non-blocking observations

- **Misleading name.** `validate_owned_edge_unique` does not check uniqueness. It checks ownership, and uniqueness follows from caller-side invariants. A future caller that omits the block guard or the Branch precheck silently loses duplicate detection. Rename it to `validate_owned_edge`, or assert the invariants in the comment as preconditions. The comment already states them.
- **Message wording.** "…or is referenced repeatedly" is now only reachable as a wrong-owner symptom. It is still accurate, and keeping it preserves compatibility with tests that match the string.
- **Exit directory.** I agree it must stay. The exit check inserts before the owner check, so a repeated exit owned by another block reports "unknown or repeated exit". The same argument (`exits[x].from == block.id`) could eliminate it only if the diagnostic ordering were allowed to change. Here it is not, because a foreign-owned repeated exit would change message.
- **Test coverage to confirm.** Cases that should produce identical old/current errors:
  - Forged `id` on an edge.
  - Forged `from` on an edge.
  - Branch with equal edge IDs.
  - Branch with a wrong-length edge list.
  - Unknown edge ID.
  - The same edge used by a Goto in a later block (wrong owner).
  - A foreign block ID whose `id != index`, which must fail at the guard before any helper call.

## Verdict

No concrete semantic, error-order, typing, or accounting counterexample in the pasted source. Resolve uncertainties 1–3 against the full file before sign-off. Uncertainty 4 is an expected, intentional budget change and should be reflected in updated test baselines.
