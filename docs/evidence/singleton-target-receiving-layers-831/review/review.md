# Review of the successor body (source only, not executed)

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

The paste has no line numbers, so I cite each point by the quoted statement or string. I can't see `Budget`, `Weighted`/`SharedWeighted`, `Label`, `weighted_successor` or `successor_targets`, and no tests were included. Everything below is conditional on those.

## Blockers

I found none provable from the pasted text alone. The shown control flow, borrows and exhaustiveness look sound.

- **Match borrows.** `*targets = Targets::Many(promoted)` follows the last use of `first` and `total` (`promoted.insert(*first, *total)`). Matching on a `&mut Targets` place with default binding modes doesn't move `targets`. I expect this to compile under NLL, so it is not a blocker.
- **Guard arm.** The guarded `One { .. } if *first == *child` followed by the unguarded `One` arm is exhaustive.

## Must-verify before merge (type-level)

1. **`Rc::new(state)` vs the `Many` arm.** In the `One` arm, `state` is `Vec<(usize, u64)>`, and `Rc::new(state)` yields `Rc<Vec<(usize, u64)>>`. The `Many` arm yields whatever `weighted_successor` returns. The `match` compiles only if `SharedWeighted == Rc<Weighted>` and `Weighted == Vec<(usize, u64)>` (a sized alias, not `[(usize, u64)]`). If `Weighted` is a slice, both `Rc::new(Vec)` and `rc_backing_bytes::<Weighted>()` fail. If the charge was meant for `Rc<Vec<_>>` but the alias differs, the fee is wrong silently. Confirm the alias.
2. **`rc_backing_bytes::<Weighted>()` must match `Rc::new(state)`.** It has to be the `RcBox` size, counting strong and weak counts plus the Vec header, with the element buffer charged separately. The `reserve_array::<(usize, u64)>(1)` before `with_capacity(1)` covers the buffer. If `own_weighted` charged the header differently, the `One` path diverges from `Many`.

## Accounting concerns (uncertain, depend on the old fee model)

- **Unconditional "finish receiving" charge.** `"factored inline successor finish receiving"` runs before the `match`, so `Many` also pays for a `Vec<(usize, u64)>` and a `(usize, u64)` carrier it never builds. Those exist only in the `One` arm. This could overcharge `Many` relative to the old path. It is also a behavior change if the old helper charged those only on the singleton route. Check whether the carriers should move into the `One` arm.
- **`Many` double-counts `SharedWeighted`.** The pre-match charge includes `2 * size_of::<SharedWeighted>()`, and the `Many` arm then charges `size_of::<Result<SharedWeighted, Diagnostic>>()`. This is probably intentional (receiving slot plus result slot). Confirm it is not a duplicated fee.
- **Promotion charges two nodes.** `reserve_btree_node::<usize, u64>` is called twice, though a two-entry `BTreeMap` allocates one leaf. This is consistent with the `Many` arm's per-new-child charge and with `One` charging no target node, so total node fees equal the distinct-child count. It is correct only if `reserve_btree_node` is a per-entry model. If it models real node allocation (about 11 entries per leaf), the promotion overcharges.
- **`label.clone()` is uncharged.** It runs after the reservations, so any allocating `Label` would be unaccounted. This is fine if `Label` is a cheap or interned type. Verify.
- **Empty-route comment.** "Zero-byte vector authentication above still runs" relies on `reserve_array(…, 0, …)` authenticating. Confirm in `Budget`, since the empty route now skips every other carrier charge.
- **Ordering around real allocation.** `Vec::with_capacity(1)` happens before the Rc backing charge. A failure there frees a Vec that was already paid for, so it is safe. It does differ from a strict "charge, then allocate" order for the Rc, so confirm the old path ordered these the same way.

## Semantics checked and looking right

- **Overflow ordering.** The repeated-child `checked_add` returns `Ok(None)` before any later acquisition or error, matching the comment. In `Many`, `contains_key` then `reserve` then `entry().or_insert(0)` means a failed reserve never leaves a half-inserted entry. For a new key, `0 + weight` cannot overflow.
- **Promotion arithmetic.** The old total is carried over unchanged. The new child's sum is `0 + weight`, which cannot overflow, so the `expect` is unreachable.
- **`assert_ne!` and `expect`.** They are guarded by the `if` arm and the `contains_key` check, so they are unreachable.
- **Ordering.** `BTreeMap` iteration preserves label order, and the `Many` inner map preserves child order. Both match the old behavior if the old code also used `BTreeMap`.
- **Early returns.** Early `?` returns drop `result` and the remaining iterator. Nothing is leaked or double-charged.

## Test gaps to cover

The paste has no tests, so I can't assess them. The following should exist:

- Exact-budget boundary tests that fail at each named reservation: node, initializer, authentication, promotion carriers, the two promotion nodes, iterator receiving, finish receiving, `One` vector, `One` Rc header, and `Many` producer receiving. Each should check the budget passes at N and fails at N−1.
- Overflow in the repeated-`One` path and in `Many` (`u64::MAX + 1`) returns `Ok(None)`, and does so before any later charge.
- Empty-route test: no iterator fee, and the zero-byte vector authentication still fires.
- A differential test of `One`, promotion and `Many` against the old helper, including a mixed multi-label case, with `pays_allocations()` both true and false.
- The seven controls that are said to be unchanged should still pass.

The claim that this beats the old 831 behavior is unmeasured. Do not treat it as established.
