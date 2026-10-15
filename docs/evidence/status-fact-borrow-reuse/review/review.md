# review (review)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

## Findings (source-only, nothing compiled or executed)

### Blockers
None found. The change is sound as pasted, under your stated `Lookup` signature and the assumptions below.

**Typing and borrowing (CURRENT, `let Some(fact) = expressions.fact(...)? else`, then `let Some(call) = fact else`):**
- The returned `&'a Option<CallFact<'a>>` is tied to `&'a self` only, not to `budget`. So holding `fact` across `Directory::new(function, budget, …)` and `cases.insert(…, budget)` in the other arms does not conflict with the `&mut ReplayBudget` borrows. This depends on the trait signature exactly as you quoted it. If any impl or the `Facts → &dyn Lookup` coercion tied the lifetime to `budget`, the arithmetic arm would fail to compile.
- `let Some(call) = fact` against `&Option<CallFact>` works through default binding modes, giving `call: &CallFact`. `call.callee.as_ref() != callee` is unchanged from OLD.
- `size_of::<&Option<CallFact<'_>>>()` compiles because the anonymous lifetime is inferred in expression position.
- The first lookup runs before the `match`, so the `UnknownStatus` error comes at the same point in the sequence as in OLD.

**Semantics:**
- The ordering is preserved: `ids.insert`, then the receiving charge, then the existence lookup, then lane/producer/callee.
- Removing the second lookup is safe only if `fact` is deterministic and the facts are immutable. `ids.insert` and `is_none` don't touch the facts, so this holds from the source shown.
- One behavior difference is intentional: the OLD second lookup could fail on budget exhaustion after the first succeeded. CURRENT cannot.

### Uncertainties (not blockers)

1. **Cost goes up for non-call sources (CURRENT, the two `budget.reserve_bytes`/`budget.charge` lines before the lookup).**
   - Every source now pays 8 bytes and 1 step on top of the first lookup, but only `PropagatedCall` sources save a second lookup.
   - `CheckedArithmetic`, `CheckedAllocation`, and `ContractFalse` sources are strictly more expensive than in OLD.
   - A function that previously fit exactly in budget can now be refused. If this is acceptable it should be stated, and any exact-boundary tests built on OLD numbers need regenerating.
   - Whether `reserve_bytes` is cumulative or peak/released is not visible here. Cumulative is conservative. Peak or released is also fine, but the per-source charge may then double-count against `CONTROL_BYTES`.

2. **The `call` binding is a second 8-byte pointer copy (CURRENT, `let Some(call) = fact`).**
   - The receiving control covers `fact`. `Option<&T>` is niche-optimized to 8 bytes, so the `Option` discriminant needs no extra space.
   - `call` is a distinct local, though it is just a copy of the same pointer.
   - In OLD, the second lookup's paid control and result covered the equivalent binding. In CURRENT, nothing explicitly pays for it.
   - This is real only under a strict "every live pointer is paid" model. Under a model that charges the lookup result and the receiving control, it is covered. Either say the model covers it, or avoid the extra local by matching `fact` in place.

3. **The first budget error can change.**
   - Under tight budgets, the failure can now occur at "retained status fact receiving control" or "…assignment" instead of inside the lookup.
   - Both reserve and charge occur before the lookup, so the label and the failing step can differ from OLD.
   - If the budget error is sticky (first error wins), this is a diagnostic-text change only. Any test that asserts exact messages will need updating.

4. **Rust-version dependence.** The `let … else` forms need Rust 1.65 or later. This is almost certainly already true, since OLD uses them.

### Is there a real gap in the new carrier, authority, first-error, or sticky-budget accounting?
No unpaid carrier is evident beyond item 2.
- The only new authority is the borrowed `&Option<CallFact>`. It is paid by the 8-byte control plus one assignment step.
- The `Result<Option<&Option<…>>, Diagnostic>` overlap is attributed to `ArenaLookup`, as you state. I can't verify that from the pasted code.
- Nothing escapes: `fact` and `call` are dropped at the end of the iteration. The returned `StatusSources` holds only `function`, `budget`, `ledger`, and `ids`.
- The `Facts::Tree` branch is byte-identical to OLD.

### Test gaps to check
Verify these exist, since the pasted source includes no tests:
- **Exact budget boundaries.** For a call source and for a non-call source, run with the budget exactly short by 1 byte and by 1 step at the receiving control, at the assignment, and inside the lookup. Also test exactly zero remaining. Expect the correct label each time and no partial `StatusSources`.
- **Lookup count.** Because `Facts` is concrete, you can't count calls directly. Assert that the old and current budget deltas differ by exactly (second-lookup cost) − (8 bytes + 1 step) on a call source.
- **Controls:** wrong callee, unknown expression, non-call expression with a `PropagatedCall` producer, duplicate id, lane mismatch, and multiple faults. For multiple faults, pin which error wins in order: duplicate, then unknown, then non-call, then wrong callee, then lane.
- **Drop behavior.** Check that early errors release the `Directory` and leave the budget in the expected state.
