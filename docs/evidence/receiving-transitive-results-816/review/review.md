# Review of the issue816 proposal (source-only, nothing executed)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

## Blockers (must be specified before approval)

1. **Growth contract is undefined.** Per-push `reserve_vec` on an already-growing Vec is ambiguous. `reserve_vec` prepays a full replacement array while the old one stays live, so doubling growth charges old plus new at every step. Also, `Vec::push` at `len == capacity` reallocates silently and unmetered. The helper must (a) compute the new capacity with `checked_mul`/`checked_add`, (b) reserve before push, and (c) return an error rather than push if `len == capacity`.

2. **`try_reserve_exact` capacity can exceed the request.** The std docs don't guarantee exact capacity. Decide the rule: either `capacity > prepaid` is a sticky error, or the excess is charged. Charge by capacity, never by `len`. Test both the exact and the oversized case.

3. **Don't call old helpers from the new path.** The old `owned_continue_leaves` does `chain[..index-1].to_vec()` and `StorageId::Value(remainder.clone())`. The old `region_chain_to_root` grows by unpaid `push`. The new owning-continue must be a reimplementation returning `Option<usize>` (the prefix length). Truncate only on the `Some` path. On `None`, `source_chain` must stay intact for the `loop_region` fallback.

4. **Error-order preservation points:**
   - Source block lookup, then full source chain, then the match.
   - LoopLatch: owned-continue errors first, then `loop_region` unknown, then the not-contained error. A single `position()` scan can serve both the contains check and the truncation point, but the `loop_region` failure must stay before it.
   - Continue/Break/Return: target block lookup, then the full target chain (errors included), then membership.
   - Within owned-continue, the second-owner and `index == 0` errors share one message, so their relative order is not observable. The `index - 1` underflow is already guarded by `index == 0`.
   - The `plan.regions[body]` indexing panics must stay identical, so don't add fallible conversions that change panic into `Err`.

5. **Budget error versus diagnostic precedence in `under_with_budget`/`validate_place`.** Old code never produced a budget error, so you must define the rule. For example: charge points may preempt later diagnostics, and the unknown-storage check precedes any reserve. An unknown storage must not reserve anything, and zero matches must not reserve. Use `reserve_bytes(0)` for authentication only. Without this rule, "preserves unknown-storage before empty-leaf" is untestable under exhaustion.

6. **Cyclic parent chains.** Old `region_chain_to_root` has no cycle guard, so it grows forever. The new path terminates only if work is charged per step, before the step. The charge must precede the push or reserve, and the failure must be sticky. Add a cycle test asserting a budget error. Don't claim semantic equality there, because old simply hangs or OOMs.

## Uncertainties

- **Borrowed remainder matching.** `region.slots.contains(&storage)` depends on the type of `slots`. If it is a `BTreeSet<StorageId>`, a borrowed lookup without constructing a `StorageId` needs a `Borrow` impl, so the fallback is an `iter().any(matches!(.. Value(v) if v == remainder))` scan. That is linear, must be charged, and is equivalent only if `StorageId` equality is derived. Also unknown: whether `remainder.clone()` allocates at all. If it is a Copy id, the no-clone rewrite buys nothing.
- **Unpaid allocations on error paths.** `replay_error` builds a `Diagnostic` (likely String-backed). `loop_region` and `exits.iter().find` are unmetered scans, and `loop_region` may allocate. Confirm the parent's "caller-paid" scope covers these.
- **Truncation keeps capacity.** This is fine under no-refund accounting, but the returned Vec's capacity is larger than its length. Check that nothing downstream calls `shrink_to_fit` or clones it with unpaid capacity.
- **Duplicated logic can drift from the old code.** Mitigate with differential tests at ample budget over generated plans.

## Minimal safe shape

- `reserve_exact_checked(&mut Vec<T>, additional)` is the only allocation primitive. It charges, reserves, verifies capacity, and returns `Err` on any failure. All pushes go through a `push_paid` that refuses `len == capacity`.
- `region_chain_paid`: walk, charging per step. Pushes use the growth helper above or the two-pass shape below.
- Target membership uses the complete target Vec, only after the full target traversal succeeds. Charge `contains` scans by elements examined.
- Truncate source to `position(|r| target.contains(r)).unwrap_or(len)`.

## Lower-allocation alternative without an unmetered first pass: yes

A **metered** counting pass works. Walk the chain, charging each step and producing the same unknown-region errors at the same point. Then call `reserve` once for the exact count, then fill by a second metered walk, which cannot fail on unknown regions. This removes doubling and the old-plus-new peak. The same pattern applies to `under_with_budget`: count matches while charging each examined flag, reserve exactly, then fill. The cost is double the charged walk work, with no discount.

Going further, the target Vec can be eliminated by re-walking the target chain per source candidate. This is safe only if the target chain is fully validated first, and the quadratic work is charged. I would not take it unless the budget units tolerate that. Don't claim runtime gains.

## Tests required

- Unknown source block; unknown region in the source chain versus in the target chain, with both present (source error wins).
- Shared ancestor at the first position, in the middle, or absent.
- LoopLatch: owned-continue `None`, unknown loop, not-contained, and `Some` with prefix 0 and prefix > 0. Owned-continue errors: no owner, two owners, `index == 0`, retained chain differs.
- Exhaustion at each reserve and charge boundary: the error is sticky, no refund occurs, and later `reserve_bytes(0)` fails.
- Charge equals capacity, including the oversized-capacity case.
- Full `Diagnostic` equality with old only at ample budget. Under tight budgets, assert only the error kind, not the boundary position.
