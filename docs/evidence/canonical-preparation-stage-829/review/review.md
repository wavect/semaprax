# Source-only review of OPT829 enclosing-stage attribution (commit 5dc34d3)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Nothing was executed. I found no confirmed blocker in the pasted source. Several claims in the packet rest on code that isn't shown, and some test gaps are noted below.

## Production delta

**Typing and privacy (looks sound).**
- The new enum variant and its `label()` arm are exhaustive (`bounded_output.rs`, enum and `label`).
- `ReservationStageGuard` is `pub(crate)` with a private field, and the call in `caller.rs` `prepare_sealed` uses the `crate::bounded_output::` path correctly.
- The guard only clones the existing `Rc<Budget>` (`reservation_stage_guard`), so it allocates nothing and has no quota effect.
- Drop order in `prepare_sealed` is fine: `captured` (which holds an `Rc`) drops before `_stage`, and `owner.diagnostic()` runs before the stage restores.

**Placement.**
- The guard is the first statement, before the program-pointer check and before `CfgReservation::acquire()`, so it matches the intended "entry before acquisition".
- The guard also covers the `hir_error(...)` path, which labels any budgeted allocation there as canonical. That is harmless and consistent with the doc text.

**Label shadowing (uncertainty, not a blocker).**
- The new guard replaces any narrower outer stage, such as `HirValidation` or `FreshResolution`, for the whole preparation scope. The stage restore is unconditional (`ReservationStageGuard::drop`), so it has no check that the budget's stage is still the one it set.
- That is correct for lexically scoped production use.
- It is not safe for out-of-order same-budget drops. Test 3 only exercises cross-budget drops.
- I could not see whether inner `build_plan_with_reservation` code sets its own stages. If it does, they restore to `CanonicalLoanStorage` correctly.

## Test and guard findings

1. **Source guard does not pin placement** (`reuse.rs`, the `_stage` inverse).
   - The inverse removes the exact `_stage` text with `matches(...).count() == 1`, wherever it appears.
   - If the guard moved after `captured` acquisition, the equality to `old` would still pass.
   - Placement is pinned only behaviorally, in `stage_tests.rs` at limit 0: the acquisition refusal must carry `CanonicalLoanStorage`. That compensates, but the "source guard inverse" itself is weaker than the packet implies.

2. **`owned_before.rs` is not actually independent or source-bound.**
   - It is `cfg(test)` and unhashed in the shown guard.
   - It calls the *current* `append`, `Payer`, `ensure*`, `refused`, and `build_plan_with_reservation`.
   - It is therefore independent only for the outer `prepare_sealed` shell. A regression in shared inner code would move old and current identically and pass the old-versus-current comparison.
   - Unless another guard binds `owned_before.rs` to the saved text, its body can drift silently. I can't see one in this packet. Treat this as a likely test gap, not a blocker.

3. **`prepare_shared` and `shared::seal` have no stage coverage.**
   - `stage_tests.rs` only drives `prepare_with_comparison`, whose seal is the trivial `|storage, _| Ok(storage)`.
   - The production seal path is not exercised for numeric or stage parity.
   - The seal inverse in `reuse.rs` is just a text replacement of `Ok((seal(storage, payer)?, deferred))`. It does not show that `shared::seal` adds no reservation or work.
   - "No new quota/work" is therefore unproven for that path from this packet.

4. **Layout test is weaker than "mirrors/niche checks" suggests** (`canonical_stage_tests.rs`, test 1).
   - It compares only `size_of` and `align_of`, and `repr(Rust)` doesn't guarantee that structurally identical types share field offsets or niche placement.
   - `size_of::<Option<…>>` equalities are a reasonable proxy for the niche claim.
   - The `Before*` types are hand-written mirrors, not frozen compiler output, so this is a regression tripwire, not proof.
   - `BeforeBudget`, `BeforeGuard`, and `BeforePaidCarrier` are never constructed, only named in `size_of`. Depending on the toolchain this may raise a `dead_code` warning. It matters only if warnings are denied or clippy is gated. This is a low-confidence uncertainty.
   - Privacy and imports are fine: `Budget`, `Cell`, and `Rc` come through `use super::*` in a child module.

5. **Final-attempt test is largely vacuous** (`refusal_tests.rs`, `final_attempt_help_never_imports…`).
   - It feeds `live_builder_refusal` a receipt from a fresh `with_limit_usage_refusal`, whose budget starts with `stage: None` by construction.
   - It never runs the real retry or final-attempt orchestration, so it can't catch an import from an earlier attempt in the actual caller.
   - It does confirm that the help text takes its label only from the receipt.

6. **Tests lean on `bounded_output` code not in the packet.**
   - The following behaviors are not visible, so the tests could fail for reasons I can't evaluate:
     - sticky refusal on `reserve_active_required(0)` (stage_tests and test 3)
     - the floor arithmetic behind `remaining: 4`
     - `used == 1` after sticky refusals
     - `reserve_active_required` itself
   - Test 3's expected receipt (requested 3, remaining 4, floor 2) is consistent with the child's charge of 1 against a limit of 5.

7. **Expected-message branch in `stage_tests.rs`.**
   - The test expects an empty message at limit 0 and `MESSAGE` otherwise.
   - This holds only if `exact - 1` always exceeds acquisition cost and refuses after acquisition, and if the retry's `acquire` always fails on a poisoned budget. Both are plausible from `cfg_reservation.rs` but depend on the unseen `reserve`.
   - The `Payer::Captured` paths return empty `refused()` and rely on `owner.diagnostic()` to supply the text, which is consistent.

8. **`metrics` comparisons.**
   - `assert_eq!(current.0, old.0)` requires the metrics type to be `PartialEq + Debug`.
   - `let metrics = current.0 .1;` is a partial move, which is fine unless the type has `Drop`.
   - Unverified, but not a likely compile error.

## Verdict

- **Blockers:** none from the pasted source.
- **Should fix or confirm:**
  - Bind `owned_before.rs` to the saved old text.
  - Add stage and numeric coverage for `prepare_shared` and `shared::seal`.
  - Strengthen the placement inverse in `reuse.rs`, for example by asserting the guard immediately precedes the `_operation` hook.
- **Uncertain:** the exhaustive-match sites outside this packet, the dead-code lint on the mirror structs, and the unseen reserve and sticky semantics.
