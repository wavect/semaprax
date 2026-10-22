# Review of the partial825 receiver (pasted source only, not executed)

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

## Blockers to settle before wiring

**B1. Setup failures are reported as `builder_bytes` limit errors, and only `Budget` sets sticky state.**
- `into_legacy_unfunded`, `Self::Setup(_)` arm: every `Setup` variant becomes `limit_error("builder_bytes", active_builder_limit())`. That covers `NoOwner`, `Arithmetic`, `Allocation`, `Capacity` and `Detached`.
- Only `acquire`'s `Budget` returns go through `reserve_active_required`, which poisons the ledger.
- `NoOwner`, `Arithmetic` and `Allocation` leave no sticky refusal. `Capacity` sets none unless the excess reserve fails, and `Detached` poisons with 0 only inside `current()`.
- The compatibility output therefore claims a builder-byte refusal that the ledger may not record. The retry loop's `live_builder_refusal` and the emitted diagnostic can then disagree.
- Also, `active_builder_limit()` is read after the child restored, so it is probably the parent's limit. I can't confirm that from the paste.
- `Unreceived` discards `refusal: _`, so `Detached` is never surfaced.

**B2. The setup charge is paid on every attempt, including those where the constructor succeeds (lines in `acquire`).**
- Because the Receiver is acquired at function start, `setup_bytes` plus the one-slot Vec is debited whether or not `receive` is ever called.
- This shifts exact-limit thresholds for previously successful builds, and a build that used to fit can now fail with `Setup::Budget` before the constructor runs.
- That is not necessarily wrong, since there is no refund by design. But the change should be explicit and covered by a boundary test, not implicit.
- **Uncertain:** if the retry loop reuses one child ledger, each attempt pays again. I can't see whether `with_limit` creates a fresh ledger per attempt.

**B3. `ClassifierLedgerDebug` holds the original `Rc` past `with_limit` restore.**
- This is fine only if restore does not depend on strong-count uniqueness (`try_unwrap`, `get_mut`, or a `strong_count` assertion).
- It is also fine only if dropping `ClassifierLedger` has no side effects.
- Neither is visible in the paste, so I treat this as unverified. The `Received` ledger lives in the returned value until `into_legacy_unfunded` drops it, which is after restore by design.

**B4. Compile dependencies are not shown.**
- `#[derive(Debug)]` on `CoreFailure` requires `Failure: Debug` and `Diagnostic: Debug`.
- `primary.into_legacy_unfunded()` assumes `Failure` has a method of that name. Your description only says the conversion is `One => vec![d]` and `Many => existing`.
- `super::super::limit_error` and `active_builder_limit` paths are unverified.

## Non-blocking mistakes

**N1. The sticky "observation" is dead code (`receive`).**
- `let _selected_budget_refusal = active_refused();` discards the value.
- The doc comment says the function "observes sticky state", but `Received` is returned identically whether or not sticky is set.
- Either drop the call and the claim, or record the result. As written, no sticky information reaches `CoreFailure`. The downstream render must supply it, and that dependency should be stated.
- `current()` doesn't check overflow either, so an overflowed ledger still yields `Received`.

**N2. `Capacity` is rejected after being fully paid.**
- `acquire` pays `(cap−1)*size_of::<Diagnostic>()` and then returns `Err(Capacity)`, dropping a valid, paid buffer.
- It is conservative and not unsafe, but wasteful, and it adds a failure mode with no sticky state (see B1).

**N3. `setup_bytes` overcounts stack-only types.**
- It sums `Self`, `Option<Self>`, `Result<Self,Setup>` and `CoreFailure`, none of which is heap.
- The actual heap allocation is `1 * size_of::<Diagnostic>()`, which is covered. That is conservative, so it is not underfunding.
- The sum is not an exact-capacity claim, and `size_of::<CoreFailure>()` couples the charge to `Failure`'s layout. Document it as an upper bound.

## Pressure points that hold up from reading

- **Affinity:** `current()` runs before any move, and `Detached` returns `primary` and `owner` intact. A foreign ledger poisons the held original with 0 and creates no budget.
- **One:** `capacity == 1` is guaranteed by the `acquire` check and the fields are private. `push` onto len 0 cannot reallocate, and `receive` performs no reservation or formatting.
- **Many:** the order is preserved by moving the Vec as-is. It becomes `Legacy` on the attached path and is retained inside `Unreceived` on the detached path, so there is no reordering.
- **Sticky:** `receive` never acquires, so a sticky ledger still moves the slot.
- **Partial moves:** moving `self.report` and `self.ledger` out of `self` compiles only because `Receiver` has no `Drop`. Adding a `Drop` impl later would break the `Received` construction.
- **`Many` arm:** it drops a paid but unused slot. That is accounted, not a leak.

## Residual scope (not new bugs)

- A `Received` report is a `Vec` with len 1 and capacity 1. If any compatibility consumer appends to it (help or final diagnostics), the reallocation is unfunded. That is the disclosed OPEN item, but nothing in the types stops it. A comment or a typed wrapper would help.
- `Legacy` from existing helpers, `Setup` and `Unreceived` allocation in `into_legacy_unfunded`, and dynamic help are all disclosed as unfunded.
- Per-diagnostic heap (the strings inside `Diagnostic`) is charged by the constructor, not here.

## Missing tests (none are in the paste)

1. `One` moves into the slot without a changed buffer pointer and without ledger debit growth.
2. `Many` order and length preserved on both the attached and detached paths.
3. Foreign or restored ledger yields `Unreceived` and poisons the held original.
4. Sticky ledger: `receive` still moves the slot with zero new reserve.
5. `acquire` boundary at exactly `bytes`, `bytes−1`, and with `capacity > 1` (excess payment).
6. Each `Setup` variant against sticky state and the emitted diagnostic (B1).
7. Receiver drop on the success path leaves the debit unchanged (B2).
8. The `Rc` strong-count and restore interaction (B3).

This review does not approve the whole of 825, and the seam should not be wired until B1 to B3 are resolved.
