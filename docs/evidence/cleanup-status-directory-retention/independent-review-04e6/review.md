# Source-only review (nothing executed)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

## Blockers

**B1. The token does not bind the ACTIVE ledger, so the nested foreign/restored identity requirement is unmet (retained.rs:5-9, 115-122).**
- `StatusSources` holds only the function address, the budget address and the directory.
- On the production payer, `ReplayBudget::new()` stores the marker `AllocationPayer::ActiveBuilder` (allocation.rs:88-89). It captures no ledger handle.
- The 0-byte `reserve_bytes` (physical_budget.rs:41-49) checks only whether the current thread's ACTIVE ledger is refused. It never checks that this is the ledger that paid for `ids`.
- If the same `ReplayBudget` address runs under a nested or foreign scope, the check passes. The directory's bytes were charged to a different ledger.
- A scope restored to the original ledger is indistinguishable from the original, and the token cannot detect either case.
- `ClassifierLedger::capture/current` (classifier_string.rs:192-218) is the existing binding mechanism, and it is not used here.
- The straight-line caller (replay.rs:621-635) does not trigger this. The stated contract ("nested foreign/restored active ledger") is still not enforced, so this blocks the contract rather than today's call path.
- The Oracle and SelectedReservation payers are not identity-bound either, though they are test-only.

**B2. Test coverage does not cover the claimed contract (tests.rs:95-123).**
- The sticky-refusal test uses `ReplayBudget::unobserved()`, which is `LegacyUnobserved`. That path has no ACTIVE ledger and no builder refusal.
- With `remaining = 0`, the auth call `charge(...,0,...)` succeeds, because `checked_sub(0)` passes. The final `result.is_err()` can therefore come from later nonzero work (a Directory insert or edge step), not from the authentication point.
- It does not check the message or that the first refusal was sticky. It does not exercise live Drop or any nested ACTIVE case.
- Test 1 does use `ReplayBudget::new()` under a limit. Its `expected` comes from an unobserved budget, and it only covers ample budgets.
- No exhaustion-order test is visible for the retained route. The handoff says existing "exact/short/zero" controls remain, but I cannot see them.
- The mismatch test uses `function.clone()`, which proves distinct addresses only. It cannot show that an ABA reuse of a budget address is rejected.

**B3. The reduction in work charged is a contract change that needs sign-off (tests.rs:62-64).**
- The old second pass built the directory and paid per-insert work. The retained pass pays `charge(0)` instead. The test asserts `new.1 < old.1`, and `new.1` is the work-unit count (`MAX - remaining`), even though the comment says "allocation".
- This is a real reduction in ordinary work charged. The user's "no discounts" wording could be read as forbidding it, though it can also be read as redundant work simply not being done.
- Handoff line 5 says the 32M ceiling's admission of the fixture is unproven. If that ceiling is the reason for this change, the source change itself is the discount.
- I cannot confirm from the pasted source that every remaining membership probe (`validate_edge_condition`, `require_status_paid`) still charges actual work. Those bodies are not shown.

## Concerns, not proven blockers

**C1. Refusal text changes.**
- Later membership-exhaustion refusals now name "cleanup status identity directory" instead of "cleanup status source directory" (the handoff acknowledges this).
- Refusal text is one of the six compared fields. The tests only compare ample-budget outcomes, so equivalence under refusal is unverified.

**C2. Identity mismatch is reported as a semantic diagnostic (retained.rs:115-119).**
- A foreign function or budget returns `replay_error`, not `budget_refusal`. It does not mark the ledger as refused or poison it.
- A caller could read it as an ordinary cleanup-plan verdict.
- `replay_error` also builds an owned message with no payer debit. That pattern is inherited, not new.

**C3. Facts variant is not bound.**
- Status with Arena facts followed by blocks with Tree facts silently takes the old Tree route (line 123). The reverse returns an `ids == None` diagnostic (line 130).
- This is only reachable if a caller mixes variants.

**C4. The Tree route gains new pre-work.**
- `reserve_bytes(0)` and `charge(0)` (121-122) now run before the Tree delegation. `charge` calls `prepare_builder`, which funds a positive refusal buffer if none exists yet.
- In the Tree path no retained Directory has run before this point, so it may be the first bootstrap, unlike the old Tree path.
- I cannot tell whether the production driver ever selects Tree.

**C5. Pointer identity can be defeated.**
- The `'hir` borrow pins the function address.
- The raw `*const ReplayBudget` has no lifetime tie. A budget dropped and recreated at the same address while the token is alive would pass. It compiles because the pointer is raw.
- A moved budget would be falsely rejected, which is harmless.

**C6. Drop accounting is unverified.**
- The observer shows heap `live == 0`, which shows the memory is freed.
- It does not show that the ACTIVE reservation is released or retained as intended.

## Checked and no concrete problem found
- Status validation order in retained.rs:20-103 matches receiving.rs:35-118 apart from the first-pass directory retention.
- Block, edge and transition bodies after the new preamble look unchanged in the visible portions. The pasted receiving.rs ends at line 160, so the tail is not compared.
- The `super::` paths and the `ptr::eq` coercions from `&mut` to `*const` look type-correct. The raw pointer avoids a borrow conflict in `and_then`.
- The carrier itself allocates nothing beyond the already-paid directory, so there is no extra carrier funding to account for. The directory now lives longer than before, which only extends its live range.
- The old Tree route is preserved for status validation.

## Summary
B1 is the unresolved identity gap. B2 means the sticky-refusal and live-Drop controls are not demonstrated with an ACTIVE ledger. B3 is the question of whether reduced work charging is acceptable. No runtime or approval claim is made.
