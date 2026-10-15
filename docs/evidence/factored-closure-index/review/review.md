# Review: lazy chunk memo (pasted source only, nothing executed)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

The paste has no line numbers, so I cite quoted expressions. `physical_budget.rs` is cut off at `budget_refusal`, and `canonical_shared.rs` is cut off after `closure`. Anything depending on those is marked as absent context.

## Blockers

1. **Control and chunk padding is guessed, not derived.**
   - In `authenticate`, `2 * size_of::<Self>() + size_of::<Result<(), Diagnostic>>() + size_of::<Binding>()` has no matching live values. `Binding` is already inside `Self` (`Option<Binding>`), so it is counted twice. `Memo` is a field of `Canonicalizer`, so it is not a new allocation. No `Result<(), Diagnostic>` is built on this path.
   - In `insert`, `2 * size_of::<[Option<T>; WIDTH]>() + size_of::<Box<..>>() + size_of::<Result<Box<..>, Diagnostic>>()` has the same problem. The only heap value is one array. The `from_fn` temporary is on the stack, and no `Result<Box, Diagnostic>` is ever constructed.
   - This probably overpays rather than underpays. But it violates your "actual simultaneous values, no guessed padding" criterion. It also makes exact-cap boundary tests (cap−1 refuses, cap passes) unprovable.
   - Each formula needs a per-term justification or should be replaced by the exact heap size.

2. **The "no automaton-wide allocation" claim is false for sparse keys.**
   - `insert` sizes the directory to `key / WIDTH + 1` slots (8 bytes each). The first insertion of a high node index therefore costs O(max_key/8) bytes. The old map cost O(1) for that key.
   - Closure keys are node indices and are inserted in post-order. Whether low or high indices come first is not visible here.
   - The module doc comment is therefore inaccurate. For tiny automata, the controls charge plus one chunk plus the directory may exceed one old BTree node. That means earlier refusals and changed caps, so any "unchanged caps" claim needs re-baselined tests.

3. **No tests are included.** For a change that alters accounting, these are required:
   - sparse high-key-first insertion, with gap `None` preserved in `get`/`contains_key`
   - directory growth exactly at a capacity boundary
   - exact-cap pass and cap−1 refusal for controls, directory and chunk
   - foreign function, foreign budget and stale ledger, each leaving `failed` sticky
   - the `LegacyUnobserved` payer staying explicit
   - parity of closure results and work debits against the retained `canonical_before` control

## Likely defects and risks

- **Pointer identity is fragile.**
  - `binding.budget != from_ref(budget)` compares addresses. If `ReplayBudget` is ever moved by value between calls, or the same logical function is re-resolved to a different `&ResolvedFunction`, a legitimate memo is rejected as "another receiver".
  - Conversely, an address reused after the original is dropped can pass the pointer check. Only `ledger.current()` guards that case.
  - This is safe only if one `&mut` budget and one function reference span the whole canonicalization. Confirm at the call site.
- **`get` and `contains_key` are unauthenticated.**
  - `closure()` authenticates up front, so its loop is covered. Any other `Canonicalizer` method that reads `self.closures` is not (not visible in the paste).
  - After `failed = true`, the memo still serves entries to those readers.
- **Inner BTrees remain.** Each memo value is still an `Rc<BTreeMap<usize,u64>>`, and `closure()` still charges `reserve_btree_node::<usize,u64>` per inner node. Only the outer per-key tree is removed. If the goal was all per-key closure BTree nodes, that is not achieved.
- **Possible cycle-detection gap (probably pre-existing).**
  - In an epsilon cycle, `(node,false)` is re-pushed because `contains_key(target)` is false. The loop never reaches `(node,true)` with a missing child.
  - It would run until the cursor charge or stack reservation refuses, so the "cleanup CFG contains a cycle" branch looks unreachable for real cycles. Confirm the old code behaved the same way.
- **Dead code.** `index.checked_add(1)` cannot overflow, because `key / 8 < usize::MAX`. The refusal-text branch is unreachable. It is harmless, but it is not a tested path.
- **Inconsistent gating.**
  - `reserve_array`, `reserve_vec` and `reserve_btree_node` return early on `!pays_allocations()`.
  - The memo's direct `reserve_bytes(positive)` calls (controls, chunk) do not, and they run `prepare_builder` for positive requests.
  - `own_weighted` does the same, so this may be the established convention. Verify that `LegacyUnobserved` and non-paying payers see no new side effects such as refusal-String bootstrap.

## Typing

I found no definite compile error in the shown code. I checked these specifically:

- `std::array::from_fn(|_| None)` needs no `T: Clone`.
- The `Box::new` target type is inferred from the slot.
- The disjoint-field write to `self.failed` inside `if let Some(binding) = &self.binding` is accepted under NLL.
- `from_ref(&mut ReplayBudget)` coerces to `&`.
- Indexing `Box<[_; 8]>` through `as_mut().expect(..)` autoderefs, and the right-hand side is evaluated first.
- `&self.automaton.nodes[node]` borrows through a shared `&'a`, so it does not conflict with `self.closures.insert`.

Uncertainties:

- `std::ptr::from_ref` needs Rust 1.76 or later (MSRV unknown).
- `use std::mem::size_of` is redundant on Rust 1.80 or later. This matters only under `deny(warnings)` or `redundant_imports`.
- `.expect("paid chunk")` may violate a no-panic lint.
- `Binding` holds raw pointers, so `Canonicalizer` is `!Send`/`!Sync`. It already is because of `Rc`.

## Absent context (cannot call these bugs)

- Whether `ClassifierLedger::capture()` allocates. `size_of::<Binding>()` covers inline bytes only, so a heap-backed ledger would be underpaid.
- The semantics of `ledger.current()` after a restore or foreign move.
- What `budget_refusal` allocates, since it is cut off. The mismatch path calls it before the sticky zero-reserve consult, which is probably fine if it is bootstrap-aware.
- The old per-key outer-node charge. If the old code paid it before the work charge, the debit ordering, and so the refusal point, may differ. The shown `closure()` retains the cursor and route charges, `closure.len()+1`, and the Rc header, in the same order.
- `replay_growth_capacity` and `try_reserve_exact` behavior. The cumulative sticky payer covers the old and new directory buffers coexisting during growth only because the old buffer was paid earlier. That holds for the shown code.

## Verdict

The shown code looks type-correct. The padding formulas (1), the sparse-key directory cost (2) and the missing tests (3) are the items to resolve before accepting this as parity-preserving. The rest depends on call-site or unpasted context.
