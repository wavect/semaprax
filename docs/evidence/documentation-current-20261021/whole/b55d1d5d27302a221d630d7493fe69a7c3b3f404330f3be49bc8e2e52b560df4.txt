No blockers are provable from the pasted code alone, and I ran nothing. Quoted snippets stand in for line numbers, since compaction makes numbering unreliable.

## Likely blocker (depends on module layout)

**1. Re-export visibility chain (census seams).** "factored `pub(super) use child Scope as EpsilonAppCensus`" followed by "replay `pub(crate)` reexports factored alias" is E0364 if `factored` is a direct child of `replay`. A `pub(crate) use` cannot widen an import whose binding is only `pub(super)`, so `pub(super)` there means `pub(in replay)`. This compiles only if the alias is `pub(crate)` at `factored`, or `factored` sits higher in the tree. Check this first. A related check is whether `app_census` (`pub(super)` in `canonical_shared`) is visible from `factored`.

## Uncertainties that could become blockers

**2. Borrow conflict in `case_certified.rs`.** `StatusInsertionOwner::new(function, &ids, budget)` is followed by `ids.insert_certified(...)` (which looks like `&mut self`) while `prefix_owner` is still live. If the owner type carries a lifetime tied to `&ids`, this is E0502. If it is a token or pointer, it is fine. The explicit `drop(prefix_owner)` before `Some(ids)` is consistent with either reading.

**3. `Debug` bounds in tests.** `StatusCases` and `Attempt` have no `Debug`, and `State` derives only `Clone, Copy, Eq, PartialEq`. Any test calling `unwrap_err()` or `expect_err()` on `Result<StatusCases, _>`, or using `assert_eq!` on a state, will not compile. `tests.rs` was not pasted, so I cannot assess the tests at all.

**4. `Option::is_none_or` in `member`.** This needs Rust 1.82 or later. Confirm the crate's MSRV.

**5. `use std::mem::size_of` in `case_proof.rs`.** It is redundant on toolchains where `size_of` is in the prelude (1.80 or later). This only matters under `deny(warnings)` or `deny(unused_imports)`.

**6. `Diagnostic::io("SPX-H006", String::new())` in `reject`.** I cannot verify the signature. It also gives every proof-state failure an empty-message IO diagnostic. If tests or callers distinguish diagnostic categories, that matters.

## Semantic observations

**7. `member` is tautological.** In `validate_status_sources_certified`, `proof.member(function, source, ordinal, budget)` is called with `source` taken from `function.cleanup_plan.status_sources`, and `rows` is that same slice. `authenticate` has already pinned `function` by pointer. So `ptr::eq(held, source)` cannot fail on this path. `op` is never inspected (`..`), and `normalized_cases` is discarded (`let _ = normalized_cases;`).
- Any ordered row/op/cases certification therefore rests entirely on `validate_required_status_sources`, which is not shown.
- The Attempt/member machinery only enforces state, budget identity and ledger currency.
- If a reviewer or test expects `member` to prove content, it does not.
- Only the `CheckedArithmetic` arm calls `member`. Allocation, contract-false and propagated-call rows are never routed through the proof, so the row coverage is asymmetric.

**8. Allocation arm gating.** The `CheckedAllocation` arm accepts only when `schema == CLEANUP_PLAN_SCHEMA_V20` and the cases equal exactly `[PayloadRefused, SizeOverflow]`. Any other schema or ordering falls to `_` and returns "lane does not match its producer".
- Verify this matches the old full body, in both the error chosen and its priority relative to the fact-lookup errors.
- Its comparison is also uncharged. This is probably negligible.

**9. Error priority.** `StatusCases::validate` runs `validate_required_status_sources` eagerly. The certified loop's duplicate-id errors (`RepeatedStatus`), `UnknownStatus` and fact-lookup errors come afterwards. You say late-HIR priority is preserved inside the old validator. That claim does not cover ordering between the early validator and these later errors. It holds only if the old caller also ran the validator before them, and the caller is not pasted.

**10. Reuse after `Finished`.** After `complete`, `authenticate` rejects `Finished`, and `reject()` flips the state to `Failed`. If any "laterchecks" code calls `proof.authenticate` or `member` after `validate_status_sources_certified` returns, it will fail and poison the proof. This is fine if the proof is single-use. Confirm that.

## Accounting uncertainties

**11. Repeated reservations.**
- `authenticate` reserves `size_of::<Result<(), Diagnostic>>()` on every call.
- `member` makes its own reservation (`size_of::<Option<&StatusSource>>() + size_of::<Result<(), Diagnostic>>()`) on top of that.
- `complete` reserves the Result size again after `authenticate`.

If `reserve_bytes` is cumulative or tracks a high-water mark, per-row cost grows with each arithmetic row, and exact-threshold budget tests could shift. I cannot see its semantics. This sits oddly with "no gain/cap/credit change" and the weighted-exact-capacity framing. Confirm against the old body's per-row reserve and charge totals.

**12. Reservation before error paths.** `authenticate` sets `Failed` on reserve or charge failure, as intended. In `begin` and `complete`, a failed reserve or charge relies on `Attempt::drop` (or an explicit `Failed` set) for poisoning. That is correct for `complete`, since the drop sees `complete == false`. Just confirm that no path both returns `Err` and leaves the state `Active` with the attempt forgotten.

**13. Raw-pointer identity.** `budget: *const ReplayBudget` is only compared, never dereferenced, so there is no soundness issue. But the pointer is not lifetime-tied to the budget. If a budget is dropped and another is allocated at the same address while the proof lives, `ptr::eq` would pass. This is acceptable given "stable borrowed mutable budget" and is worth a doc note.

## Items that look fine

- `&mut ReplayBudget` coerces to `*const ReplayBudget` at the struct-field and `ptr::eq` argument sites.
- `&Vec<StatusSource>` deref-coerces to `&'hir [StatusSource]`.
- The four-`super` path to `replay::validate_status_sources` resolves correctly from `retained`.
- Visibility of `begin`, `complete`, `member` and `authenticate` as `pub(super)` is right for use from sibling `case_certified`. `StatusCases` is `pub(in replay)`, matching the certified function's signature.
- Closures in `authenticate` and `begin` capture `budget` after the prior `Result` has been produced, so there are no overlapping borrows, provided `reserve_bytes` and `charge` return owned results.
- `mod semantic_refusal { pub(super) use super::Receipt; }` inside `prior_layout` is a valid alias of a private parent item. It stays within that module's scope.

## Not assessable from the paste

- `tests.rs` and the `cfg(test)` old retained body.
- The TLS/`Cell` Drop ordering claim.
- The `nested/check` imports.
- The workspace_graph alias use.
- `ClassifierLedger::current()` poisoning.
- The `validate_required_status_sources` contents.
