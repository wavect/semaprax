# review (small-case-review)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

## Blockers

1. **`exact_accounting` does not check that the runs were valid.** It never asserts `old.0 && new.0`, `!old.3 && !new.3`, or `old.4.is_none() && new.4.is_none()`. The identities only make sense for full-limit successful runs. On a refused or failed run, `used`/`debit` are partial. A mistaken call on a hostile input would either fail confusingly or hide the problem. Add these preconditions, or call it only from the full-limit branch.

2. **The strict win on the representative is not asserted.** The `_ => {}` arm proves nothing, and the identity only proves `new = old − removed + borrow`. The claim is only honest if the test asserts, for the function that carries the statuses:
   - `status_sources.len() >= 2` (or `== 3`);
   - `new.1 < old.1`, `new.2 < old.2`, and `new.5 < old.5` (or, equivalently, `removed.0 > BORROW_UNITS` and `removed.1 > BORROW_BYTES`).

   The status count is currently only printed with `eprintln!`. Nothing shows that `main` actually produces three status sources. Nothing shows that `read` (one status, probably +3 units) is excluded from the "win" assertion. Nothing shows that bytes win rather than only units.

3. **The one-status byte and heap behavior is unproven.** For `len == 1` only the units are pinned (`removed.0 == 4`, `+3`). `removed.1` and `removed.2` are then fitted by differencing. By your own description, the first grow prepays a full replacement capacity of node-sized elements. That may be below or above 49 bytes. A byte regression for one status is therefore possible, and the test neither derives nor bounds it. Either derive `removed.1` from `size_of::<Node>() * cap + len+1`-style charges, or assert its sign and say the byte equation is observational.

4. **The zero-status arm is only half asserted.** It pins units but not `removed.1 == 0`, `removed.2 == 0`, or `new.5 == old.5`. The "guard allocates no heap" claim, and "`Directory::new` authenticates 0" in bytes, are exactly what this arm should prove.

## Assessment of the equations

- The unit equation for one status is derived rather than fitted. Grow charge 1, plus `VISIT3`, gives 4. `7 − 4 = +3` follows from literal constants, and `BORROW_UNITS == 7` is asserted separately. This part is fine.
- The general identity is sound as an additivity check. If the old and new receivers differ by anything other than the Directory work and the borrow bookkeeping, it fails. It is still tautological for any quantity measured only by isolation. It validates `BORROW_*` against the measurement but does not independently derive the multi-status cost. That is acceptable only if you describe it as "removed work measured in isolation," not "proved."
- The identity uses the production `BORROW_UNITS` and `BORROW_BYTES` on both sides. A wrong constant would make the test fail rather than pass silently, which is good. However, `49` is described as 64-bit-specific. If `BORROW_BYTES` is not computed from `size_of`, the test breaks on 32-bit targets. Likewise, `assert_eq!(BORROW_UNITS, 7)` hardcodes the value in a test that otherwise tracks the constant.

## Does the independent removed Directory exercise the same operations?

Mostly yes. The same producer runs, the token is held until after the Directory drops, and the measurement is taken before the drops. That matches the old receiver's retention window. Remaining concerns:

- **Source order and filtering.** The helper iterates `function.cleanup_plan.status_sources` directly. This is the same as production only if the old receiver does not filter, deduplicate, or reorder. Order matters for rotations. Nothing in the test ties the two sequences together.
- **Duplicated label.** The label `"cleanup exit status directory"` is copied into the test. If the production label differs, it only matters when labels are charged or allocated. The copy can still drift silently.
- **Metrics isolation (uncertainty).** `removed.2` uses `current_metrics()` deltas, and the outer `measure` asserts `live == 0`. Whether these metrics are thread-local or serialized under `with_limit_usage_refusal` is not visible. If they are global and tests run in parallel, `removed.2` and `old.5` can be contaminated.
- **Single-run determinism (uncertainty).** `removed.0` and `removed.1` are measured once. That is fine if the charges are deterministic. It also assumes the old receiver's Directory was never operating under a different `budget.remaining` or a different prepaid state than a fresh budget.

## Drop and authority

- Drop order in the helper is `directory`, `held`, `budget`, and the closure's final `allocation.live == 0` check covers leaks. This is adequate.
- The helper discards `first`/`refused` only after asserting they are clean. Fine.
- Missing: nothing asserts that the *new* path's borrowed Directory is not dropped or freed by the receiver (no extra allocation). Item 4's `new.5` relation would cover this.

## Uncertainties (not blockers)

- Whether the strict win holds at exactly 2 statuses. I have no values and am not inventing any. The representative needs a threshold derived from the grow, comparison, and `VISIT3` costs, or the test should assert on the actual three-status function only.
- Whether the "false universal" wording is fully retracted elsewhere. The correction should say "cost increases by 3 units for exactly one status and decreases for ≥N statuses," not just "no regression."

**Recommendation:** do not send until blockers 1–4 are addressed. The two additions with the most value are the preconditions in `exact_accounting` and explicit `<` assertions on the representative's status-carrying function, with the status count asserted.
