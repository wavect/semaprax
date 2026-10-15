# review (review)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

I did not execute or compile anything, and the four test controls and the helper types (`ClassifierLedger`, `ReplayBudget`, `compare_with_index_storage`) were not pasted. The pasted files have no line numbers, so I cite function and statement instead.

## Likely blockers

1. **Payment escapes the active payer (`continuation_receiver.rs::cross_check`).**
   - The `Some(storage)` comparison runs against `let mut factored = ReplayBudget::with_skeleton_limit(0)`, not the hook's `budget`.
   - Any `fresh()`, nested carrier, or directory reservation made by `Storage::acquire` during that compare is charged to this throwaway budget.
   - The caller's `budget` only pays the zero-byte `authenticate` reservation. That breaks the claim that every `reserve_bytes` uses the same active payer and sticky refusal.
   - Nothing pasted shows `with_skeleton_limit` producing `AllocationPayer::ActiveBuilder`. If it doesn't, reservations on `factored` may be unpaid or refused, and the fix would be to pass the caller's budget through.

2. **Refusals become oracle-disagreement panics (same function).**
   - `compare_with_index_storage` can now return `Err` from `acquire`/`authenticate` (Program/Detached refusal, payment refusal on `factored`).
   - `assert!(matches!(verdict, Ok(Some(true))), ...)` turns every such `Err` into a panic that reads "disagrees with enumeration".
   - This is a new failure mode compared with the original fresh-storage call, which had no owner and no authentication. A hostile or foreign storage can panic the process instead of returning a diagnostic.
   - The hook authenticates just before, which narrows the window but does not close it. A payment refusal on `factored` is still reachable (see item 1).

3. **The program-identity check in `acquire` is vacuous (`continuation_storage.rs::acquire`).**
   - It calls `self.authenticate(self.program, function, budget)`, so `matches` compares `self.program` to itself, assuming `ResolvedProgram::full_program()` returns `Some(self)`.
   - `Failure::Program` can therefore only be set from the hook, which supplies the real context. Any other acquire path gets only the Detached ledger check.
   - If the claim is that acquire receives authority from the caller's program, it does not, because `acquire` has no program parameter.

4. **`current()` may not compile (`paid_receiving.rs::authenticate`).**
   - It uses `self.receiving_owner.as_ref().is_some_and(|owner| !owner.current())`.
   - The spec says `current` poisons the held original on mismatch. If that needs `&mut self` rather than interior mutability, `as_ref()` fails to compile; it would need `as_mut()`.
   - I can't see `ClassifierLedger`, so I'm flagging this as a compile risk, not a certainty.

## Semantic and independence concerns

- **The old oracle is no longer independent.**
  - The original final compare used fresh storage (`None`). The new `Ok(None)` branch of the hook compares through the ordinary `Storage`, sharing frame backing, nested directory and sticky state with the code under test.
  - This contradicts "old-oracle independence" for owner-bearing storage. Only the opted branch avoids ordinary storage.

- **"Byte-identical" is not what is pasted.**
  - `continuation_receiver::cross_check` is a duplicated copy of the original body (enumerate, sort, compare, assert), not the original function.
  - The original body is called only in the fallback arms. Drift between the two copies is possible, and the four controls are the only guard.
  - The `enumerating` budget is a separate `with_skeleton_limit(MAX_REPLAY_WORK_UNITS)` budget and is unpaid by the active cap. That is the same as the original, but worth confirming it is intended for an ActiveBuilder root.

- **Sticky `receiving_first` poisons the storage permanently.**
  - One foreign-program probe through the hook makes later legitimate use of that storage fail.
  - A Detached failure also stays sticky after the ledger is restored, while a pre-check success is re-evaluated on every call.
  - This is plausible as intended, but "restored/cached" test expectations must match: Detached before the first check is sticky, and later detachment is checked each time.

- **Ledger check ordering.**
  - The ledger is only consulted if the program matches, so a program mismatch never poisons the ledger.
  - The hook's opted arm skips authentication entirely. That is fine only if the opted oracle truly ignores `storage`.

- **Coverage gaps.**
  - `for_program` captures the ledger only when `budget.payer == ActiveBuilder` and the program has at least one function. Empty programs and other payers get no owner, so no authentication.
  - `capture()` behaviour when no active ledger exists is not shown.

## Uncertainties (cannot confirm from the paste)

- **Zero-byte reservations:** whether `reserve_bytes(.., 0, ..)` honours sticky refusal and the payer check. `authenticate` plus the `acquire` Top path call it twice per acquire, which is harmless only if it is idempotent.
- **Glob imports:** `use super::*` in the child module must bring `AllocationPayer`, `ReplayBudget`, `Diagnostic` and `ResolvedFunction` into scope, and `budget.payer` must be visible there.
- **Carrier size:** `2*size_of::<Self>() + size_of::<Result<Self, Diagnostic>>()` charges stack bytes for a heap-free carrier. The justification is not derivable here. It is not a bug if it matches the spec.
- **Rc clone lifetime:** the held `Rc` clone keeps the original ledger alive past the active scope. That makes `Rc::ptr_eq` address-reuse safe, but if the ledger's `Drop` does accounting or assertions, running it on whole-drop of `Storage` outside the active scope is untested here.
- **Panic unwind:** `SKELETON_MATERIALIZATIONS` is not restored on assert panic (`cross_check`). This is probably inherited from the original and only matters if tests catch the panic.
- **Borrow rules:** `self.authenticate(self.program, ..)` relies on two-phase borrows and `&'hir ResolvedProgram` coercing to `&dyn ReplayContext`. That is likely fine, assuming `ResolvedProgram: ReplayContext`.
- **Hook shape:** a by-move binding `Some(storage) if guard` followed by `_ =>` is valid Rust. `Err(refusal)` in the `panic!` needs `Debug`.

## Not reviewable

The four source controls (paired comparisons in the same observer/ledger, automata/work/diagnostics, exact/short/zero budgets, foreign/restored/cached, whole-drop) were not pasted, so I make no claims about them. Given blocker 1, the exact/short/zero controls should specifically assert that `factored`-side allocation is charged to the root budget.
