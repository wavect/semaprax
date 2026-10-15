# review (followup)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

**Scope:** source-only review of the pasted code. Nothing was executed, and `reserve_vec`, `reserve_bytes`, `ClassifierLedger`, the closure loop and the tests are not pasted, so findings that depend on them are marked conditional.

## Blocker (conditional)

**B1. Directory growth may exceed the charge (`insert_bound`, `reserve_vec` then `resize_with`).**
- The code calls `reserve_vec(..., additional, ...)` and then `self.chunks.resize_with(required, ...)`.
- `resize_with` calls `Vec::reserve`, which grows amortized: roughly `max(2*cap, required)`, with a minimum capacity of 4.
- That is unbacked unless `reserve_vec` itself performs `try_reserve_exact` on this same `Vec`, so the later `resize_with` never reallocates.
- If `reserve_vec` only computes and charges a "target", the real backing can be up to about 2x the paid target. That is exactly the unbacked-term class you asked about.
- A sparse high key makes the effect large.
- Fix: have `reserve_vec` reserve exactly, or `try_reserve_exact` right here after the charge. Test that `capacity()` after the insert is at most the charged target, including the second insert, where `len < cap`.

## Uncertainties and unbacked terms

**U1. Chunk formula (`2*size_of::<[Option<T>;8]>() + size_of::<Box<..>>()`).**
- The live copies are:
  - the `from_fn` result,
  - `values`,
  - the `Box::new` argument,
  - the heap array.
- Those are up to 3 stack copies plus the heap.
- Optimization usually elides most of them, but the comment claims an exact named-initializer model.
- Either document "2 arrays" as a deliberate policy or charge consistently.
- The frame charge in `id` (`reserve_array::<CanonicalFrame>(1)` before `vec![Enter(root_copy)]`) charges heap only, with no initializer. That is inconsistent with the chunk policy.

**U2. Activation formula (`size_of::<Self>() + size_of::<Binding>() + size_of::<Result<(),Diagnostic>>()`).**
- `Self` already contains `Option<Binding>`, and the named `Binding` is separate. Good.
- The `Some(binding)` temporary is `Option<Binding>`, which is likely larger than `Binding`. `Binding` has no spare niche: its raw pointers have none, and `Option<Rc>` already spends its niche on `None`.
- Those extra bytes are uncharged.
- `Memo::new()`'s return initializer and the `Canonicalizer` struct-literal temporary are also not charged. Only one `Self` is counted, although you say new "initializer carriers" are covered.
- The sizes are small, but they are inconsistent with the stated policy.

**U3. One-shot activation charge and `pays_allocations`.**
- The fee is paid only when `binding.is_none()` and `pays_allocations()` is true at that moment.
- If `pays_allocations` can differ between the first authenticate and later chunk or directory charges on the same budget, the receiver is permanently unpaid. Confirm that it is constant per `ReplayBudget`.
- The `Result` carrier is charged only on first activation, although every later `authenticate` returns one too.

**U4. Legacy mode.**
- With `pays_allocations == false`, `reserve_vec` skips growth control and `resize_with` uses ordinary growth.
- A key of roughly `usize::MAX/8` makes `index*8` bytes exceed `isize::MAX`, so `resize_with` panics with capacity overflow instead of returning `Err`.
- This is a non-issue only if keys are bounded by the automaton state count. Put that bound in a `debug_assert` or a test.

**U5. Ledger capture ordering.**
- Capture happens after the positive `reserve_bytes` in the paying path. If a ledger is bootstrapped at that point, the capture sees it.
- In the non-paying path there is no bootstrap, so capture may record `None`.
- Later positive reserves, such as `reserve_array` in `id` or `closure` right after `authenticate`, could bootstrap a ledger after capture. That could make `current()` fail spuriously, or hold a stale ledger.
- This depends on `capture()` and `current()` semantics for `None`, which are not shown. Test non-paying capture followed by a positive reserve on the same budget.

**U6. Authority is identity-by-address only (`Binding` pointers).**
- No dereference occurs, so this is memory-safe.
- An equal address is accepted even if a different `ResolvedFunction` or `ReplayBudget` was later placed at the same slot. This holds only if the `Memo` cannot outlive the borrows.
- For your actual caller (one `Canonicalizer` local, two `id` calls on the same borrows, dropped together) it is sound.
- `Memo` and `Canonicalizer` are not lifetime-tied. Nothing enforces this for any other caller.
- If `Canonicalizer` stays `pub(super)`, add a doc invariant or a `PhantomData<&'a ()>` tie.
- A moved `ReplayBudget` fails closed, which is fine.

**U7. Reads after sticky failure.**
- `get` and `contains_key` return `None`, which is indistinguishable from "absent".
- Today the closure loop only reads between its upfront `authenticate` and the return, and every `insert` re-authenticates. So a failed state surfaces as `Err` on the next insert or call.
- If a closure path can read `None` for a root and return `Ok(None)` without another authenticate or insert, a failure could be read as "no closure". That would be a semantic error rather than a diagnostic.
- Verify the final `root.get` cannot turn a sticky failure into `Ok(None)`.
- In `id`, `authenticate` precedes the `states` lookup, so the cached-root path is covered.

## Checked and fine (from the pasted text)

- **Typing:**
  - `ptr::from_ref(budget)` with `&mut` coerces to `&`.
  - The `NLL` borrow of `&self.binding` ends before `self.failed = true`.
  - `std::array::from_fn(|_| None)` needs no `Copy`.
  - `required = index + 1` cannot overflow because `WIDTH > 1`.
- **Failure handling:** every error path in `authenticate` and `insert` sets `failed`.
- **Authentication order:** pointer and ledger checks run before any `budget` read. `reserve_bytes(0)` authenticates without a bootstrap.
- **Error text:** the empty `String::new()` error text does not allocate.
- **Chunk receiver:** the directory's `Option<Box>` slot is not double-charged.
- **Tests:** none were pasted, so the new sparse-first, wrong-function, wrong-budget, stale-ledger and legacy tests cannot be judged. The missing capacity-versus-charge assertion in B1 is the most valuable addition.
