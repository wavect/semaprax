# Review: DISTINCT removal of `referenced_exits`

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

I reviewed only the pasted text, so I am not claiming any execution or compilation. The snippets have no line numbers, so I cite them by label and quoted expression.

## Verdict

I found no proven blocker in the error-priority logic. The unknown-or-repeated vs wrong-owner ordering looks preserved for the cases you enumerated. Budget-exhaustion priority is not preserved, and a few Rust bounds and placement facts are not visible in the paste.

## Semantic check: priority preserved

Taking your stated caller invariants as given (contiguous checked prefix, ids equal to indices, every earlier transition and terminator already accepted):

- **Out-of-range exit.** The range check is first in both versions, so it returns unknown-or-repeated.
- **`owner == current`.** An earlier insertion of `x` would have required `exits[x].from == earlier_id`. That ID is below current and `from` is current, so it cannot have been inserted. The old insert succeeded and the owner check passed. The new code falls through, which matches.
- **`owner < current`, `blocks[owner]` is `Exit(x)`.** The old set contained `x`, giving repeated. The new `probe.repeated` is true, giving the same message.
- **`owner < current`, other terminator.** The old insert succeeded, then wrong-owner. The new `repeated` is false, giving wrong-owner. This is correct.
- **`owner > current`.** The new code skips the lookup and gives wrong-owner, as the old one did.
- **`previous.id == probe.owner`.** This is redundant given the prefix invariant but harmless.
- **`None` branch.** It is unreachable, since `owner < current < len`.
- **Forged earlier blocks.** An earlier block with `Exit(x)` and a wrong `from` would already have returned wrong-owner, so the new code is not fooled by it.

## Concerns (not proven blockers)

1. **Budget-error priority is not preserved on the cold path.** The old wrong-owner and repeated routes paid real AVL work (comparisons, node visits, a possible Vec replacement and rebalance). Each charge could fail with a budget error before the semantic error. The new cold route pays `CONTROL_BYTES` and 20 units under different labels (`"cleanup prior exit reference controls"` and `"cleanup prior exit reference proof"`). Under a tight budget, a plan can fail with a different error, label or point than before.
   - This is acceptable only if your requirement covers non-budget errors. It conflicts with "byte-exact". Decide explicitly which one holds.
   - The same applies to the valid route, where charges simply shrink. A plan that used to exhaust the budget now succeeds, and that is probably intended.

2. **Placement of the `Directory::new` replacement is not shown.**
   - The two zero checks must sit exactly where `Directory::new` ran, relative to the edge inventory loop and to the other pre-loop charges and reservations. Otherwise first-error order changes for a zero or sticky budget.
   - Per-exit zero checks run after the range check and before the owner read. That matches the old insert position, assuming the old insert used the same label string for its reserve and charge. That assumption is not visible.

3. **Rust bounds are not visible in the paste.**
   - `Probe { owner: …from, current: block.id, exit: *exit }` needs `BlockId: Copy`. `*exit` already implies `ExitTargetId: Copy`.
   - `previous.id == probe.owner` needs `BlockId: PartialEq`. The old `from != block.id` already requires it.
   - `seen == probe.exit` needs `ExitTargetId: PartialEq`. The old code used an ordering key and may never have used `==`, so this bound is new and unconfirmed. Alternatively compare `seen.0 == probe.exit.0`.
   - The `matches!` with an `if` guard binds `seen` by copy out of a `&CleanupBlock`. That compiles only if `ExitTargetId` is `Copy`.
   - `Probe<'static>` and the `Option<&CleanupBlock>` sizes inside a `const` are fine. Elided lifetimes in type arguments are allowed there.
   - The `BlockId.0` and `ExitTargetId.0` fields need visibility from the `exit_prefix` child module. The caller already reads `block.id.0`, so this is probably fine.

4. **"Write-only" is an assertion, not something the paste shows.** If any code after the loop reads `referenced_exits`, removing it is wrong. For example, an "unreferenced exit" check would read it. Confirm that no other use exists in the 440-line function.

5. **The accounting is a model, not a measurement.**
   - `CONTROL_BYTES` is a sum of `size_of` values, and 20 is a hand-counted step ceiling. The count does add up (5+2+3+2+1+3+3 = 19, plus 1). Nothing ties it to the compiled code, so a later edit to the probe could silently desync it.
   - All cold paths return `Err` immediately, so the permanent-reservation semantics of `reserve_bytes` do not matter.
   - Dropping the old Directory removes its `Drop` and release behavior. Verify that nothing consumed the post-state, such as peak or used counters, on success or error.

6. **The `Probe` struct is redundant.** The cold path could use plain locals with the same prepayment. The struct exists mainly to justify the byte figure. Fields such as `owner`, `current` and `exit` are written and read once. Check for warnings if the crate uses `deny(warnings)`.

## Uncertainties I cannot resolve from the paste

- The old `insert` internals (whether the duplicate path charged before returning false), and whether `reserve0` and `charge0` used the same label as `Directory::new`.
- Whether `ExitTarget.id` can disagree with its index. You say it is not validated, so both versions ignore it, which is consistent.
- No test code was pasted, so I cannot assess test mistakes. "Independently compiled verbatim" and the pending parity checks are unverifiable here.

## Tests I would require

- A differential old-vs-new harness over small generated plans. Vary the owner (`<`, `==`, `>` current and out of range), the owner's terminator (same `Exit`, different `Exit`, non-`Exit`), the exit id (out of range and `u32::MAX`), and combined faults across multiple blocks. Compare the exact error string.
- Budget sweeps from 0 up to the point where both versions pass. Compare the error label and the first failing step. Expect and document deliberate differences on cold routes.
- Sticky-failure and zero-budget cases that hit the constructor replacement before the first block, with and without any `Exit` terminator.
- A check that valid-route charges drop by exactly the old AVL cost and nothing else.
