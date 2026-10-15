# review (independent-review)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

**Scope:** source-only review of the pasted code. Nothing was executed, and I did not verify the unseen `undo`, `pays_builder`, `reserve_*` or `oracle` code. I found no definite compile or semantic blocker in the pasted lines. The items below are insufficient controls and uncertainties.

## Likely insufficient controls

**1. `receive()` is a tautological caller control (tests.rs 240–279).**
- `receive` is a local re-implementation of the `?` shape. Asserting `(calls, old_calls) == (1, 1)` and `actual == expected` only proves that this helper short-circuits on `Err`. It would still pass if factored.rs 218–224 were changed.
- The only link to production is the text scan (226–236). It is rustfmt-sensitive and string-based. The `.or_else(` / `.unwrap_or` negatives are a weak guard, and `matches("canonical.id(").count() == 2` constrains nothing beyond call count.
- The helper omits the two `reserve_array` calls at factored.rs 217 and 221 that sit around the real `id()` calls. For `LegacyUnobserved` they are probably no-ops, but I cannot confirm that. The "actual caller boundary" is therefore approximated, not exercised.
- The `roots[5]` branch is never evaluated, which is fine for the Err-only assertion. It also means the success path of the two-root boundary has no coverage.
- **Classification:** non-blocking if the text scan counts as the binding. It is a blocker only if a real exercised caller is required.

**2. "Detached LegacyUnobserved" is asserted by comment, not by construction (tests.rs 185, 193, 209–220).**
- The test uses `ReplayBudget::new()`. allocation.rs 88–92 picks `ActiveBuilder` whenever `bounded_output::active_limit().is_some()`.
- A `cfg(test)` `ReplayBudget::unobserved()` exists (allocation.rs 78) but is not used. There is also no `assert_eq!(budget.payer, AllocationPayer::LegacyUnobserved)`.
- If ambient state ever gives an active limit, these ledgers become `ActiveBuilder`. Line 203 `prepare_builder`, line 204 and physical_budget.rs 310–312 would then make the probe at 216/220 fail, or make the two budgets interact through the shared builder.
- **Classification:** uncertainty about test isolation, but a cheap hardening.

**3. Post-refusal parity compares the budget with itself (tests.rs 212–220).**
- Old and new run the same `ReplayBudget::charge`. Equal `remaining` and `refusal_spent` after a failure only show that both receivers charged the same amount before the first Err.
- That is useful for the work-use parity claim. It is not evidence about receiver behaviour after refusal.
- The probe is only a 0-unit charge. After a failed `checked_sub` (replay.rs 211), `remaining` is left unchanged and may be nonzero for the `spent - 1` case, so a 1-unit follow-up would succeed on legacy. A 1-unit probe would pin the non-sticky legacy semantics more strongly, and a 0-unit probe cannot distinguish "unchanged" from "reset".
- Receiver internal state is not compared. The old wrapper exposes `states()` and `signatures()` (canonical_before.rs 290–295), but the test never compares them with the shared receiver's state after the failure. Whether the shared `Canonicalizer` exposes equivalents is unknown.

**4. The test now pins the non-sticky legacy quirk, so a deliberate policy change would fail it.**
- physical_budget.rs 301–309 matches the assertion (`Legacy` returns early, so `refusal_spent` stays false).
- The sticky selected/ActiveBuilder/Oracle behaviour is not exercised anywhere in the pasted lines. The handoff asserts those controls remain, but I cannot verify it.
- A selected-path assertion beside the legacy one would show the distinction is intentional.

**5. Self-attested evidence is not a control.**
- `source-custody-349-proof.json` and HANDOFF.md say "static byte reconstruction only". The JSON carries no checkable content beyond an archive hash and booleans, so it does not count as verification.

## Source inverse (a.rs 270–679)

Logic appears sound as shown:
- The prefix and suffix stripped from `canonical_before.rs` are consistent with its lines 1–3 and 276–296. `original` ends in `}\n\n`, so `original + marker` matches the literal ending at 666–667.
- The single `#[rustfmt::skip]\n` is asserted and removed (317–318). It is absent from the older literal, which is correct.
- `joined` is asserted to occur exactly once (670), and the final whole-file equality with the archive is retained (672–678). This binds the compiled `canonical_before.rs` bytes to the restored text, which addresses the double-graft from selector348.
- I spot-compared the interior of `canonical_before.rs` with the older literal and found no differences.

**Uncertainties:**
- The older `undo(...)` call at 322 is truncated, and `undo` semantics are unseen. If `undo` does not assert exactly one match and failure, a missed pattern would show up only at the final equality (which does catch it).
- Equality with the archive is checked after the old receiver is restored. The `joined` assertion is partly redundant with that equality. Its real value is binding `canonical_before.rs`, and it does that.
- Relative paths `../../factored.rs` and `../../factored/canonical_before.rs` resolve correctly from `source_custody/` and `canonical_shared/`.
- Visibility and imports of `canonical_before::Receiver` from `canonical_shared::tests` are unseen.

## Minor typing and compile notes (low risk)

- `current.unwrap()` after `assert_eq!(current, old)` compiles, since `assert_eq!` borrows.
- The `receive` closures capture counters and receivers by `&mut` and are moved into the call, so the later reads of `calls` and `old_calls` are fine.
- `Result<Option<bool>, Diagnostic>` equality needs `Diagnostic: PartialEq + Debug`. The earlier `assert_eq!(current, old)` already requires this.
- `spent - 1` underflows only if `spent == 0`. The fixture presumably charges work, but there is no guard.

## Summary

There are no definite blockers. The main weaknesses are:
- The caller boundary is not exercised against the real caller.
- The detached ledger relies on ambient `active_limit`.
- The post-refusal check compares the budget with itself.
- The selected, `ActiveBuilder` and `Oracle` sticky controls are not demonstrated in the pasted lines.
