# Review of the status-prefix certified insertion proposal

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

I reviewed only the pasted text. I ran nothing, and none of this is an approval claim.

## Answers to the questions

**Is p valid for every stored key? Yes, conditionally.**

- Let q be the query expression, lo the min expression and hi the max expression. p = min(LCP(q, lo), LCP(q, hi)). Both lo and hi start with q[0..p].
- The set of byte strings with a fixed prefix is a contiguous interval in bytewise lexicographic order. So any expression s with lo ≤ s ≤ hi also starts with q[0..p].
- This holds for any bytes, including NUL, empty and prefix cases. It also holds when the query lies outside [lo, hi]; p is then possibly 0, which is still correct.
- Lane ties are fine. Order is (expression, lane) with expression primary, so the expression sequence in AVL order is non-decreasing. Multiple lanes on one expression do not break the bound.
- For UTF-8, `String`/`str` `Ord` is byte-lexicographic, so the byte loop agrees with it.

**Preconditions the pasted source does not show (uncertainties):**

- `StatusSourceId`'s `Ord` is derived with `expression` declared before `lane`. Only `KeyWork` is pasted. If the field order differs, or there are more fields, the equivalence claim fails.
- `ExpressionId`'s `Ord` is the plain `String` order. A custom or wrapped `Ord` would break the byte-loop equivalence.
- The directory really is sorted. Min is `nodes[first]` and max is the right-walk from the root, so both depend on the AVL and linked-list invariants. Those hold only if every prior insertion used the same order. The specialized path never verifies the invariant. If it were corrupted, you would get silently wrong orderings, not a refusal. The `checked_sub` in the comparator only catches `prefix > end`.

**Does the same root path with explicit suffix comparison preserve duplicate and semantic priority? Yes, provided p is valid.** The comparison result at each node equals the old `Ord::cmp`, so the path, `parent`/`before`/`after`/`side` and the found-duplicate exit are identical. This does not trust earlier facts. It trusts only the structural invariant above, so state that explicitly in the design.

## Likely blockers

1. **Double charging or mismatched charging if the pasted `compare` is reused per node.**
   - `compare` itself calls `reserve_bytes(COMPARISON_BYTES)`, charges `BOUND_UNITS`, and charges the suffix units. The proposal also charges "VISIT_UNITS3 + certificate/control bookkeeping + suffix" before each node.
   - Either you charge twice, or you reserve `COMPARISON_BYTES` once per node. If that reservation is not released, it consumes byte budget linearly along the path.
   - The proposal also says it prepays layouts once. Pick one owner for each charge. Use a non-reserving suffix primitive, or prove the reservation is released.

2. **Charge-before-read ordering.** `compare` receives `candidate` already dereferenced, so the node and key reads happen before its charges. The proposal's VISIT charge must be paid before `&self.nodes[at]` and before the key's `as_str()`. That is not satisfiable if the comparator is called as pasted. Write a dedicated suffix routine.

3. **Diagnostic phase strings.** The comparator uses its own phases ("expression-fact certified suffix lookup", "expression-fact prefix control bookkeeping"). Old probe charges used `self.phase` ("cleanup status identity directory"). A refusal at a231 would then report a different phase or label, which breaks the "all 6 fields" diagnostic identity unless the new routine takes `self.phase`.

4. **`pub(super)` in the child module.** In `directory/status_prefix.rs`, `pub(super)` means visible only inside `directory`, not `receiving`. The retained caller will not compile. Use `pub(in super::super)` or re-export from the parent.

## Accounting and semantic risks

- **The change is not monotonically cheaper.**
  - Up front it pays prepayment, two full prefix-0 comparisons (about 2·len), a 3-unit-per-level max walk, and bookkeeping. The savings are about depth·p.
  - For small directories, short keys or shallow paths, the new algorithm can cost more than the old one. A budget that the old code finished within can now refuse. A duplicate at the root, which cost the old code one visit, now pays all the prepay first and may refuse first.
  - Tests must include a "new refuses where old succeeds" case. The claim that the "first duplicate before unknown expression before callee" priority holds should be qualified as holding only where budget suffices.

- **Lane charge differs.** Old `KeyWork` always adds 1 for the lane. The new code pays it only when expressions are equal. This is acceptable if documented, but it means success and refusal frontiers legitimately move. Tests must not hard-code the old a231 frontier for the new path.

- **Overflow.** `VISIT_UNITS*3`, `min_len - p + 1`, and all sums need `checked_*` with the existing overflow diagnostic. A `prefix > end` condition must be a refusal, never a panic or a wrapped value.

- **Insert tail.** Reusing grow, the VISIT append, node initialization, link updates and rebalance unchanged is sound. Whether a failed rebalance leaves a partial mutation identically to the old code is something to test with a Drop-count check. It cannot be confirmed from what was pasted.

## Lifetime and typing

- Use `impl<'a> Directory<&'a StatusSourceId>` with `key: &'a StatusSourceId`.
  - Nothing in that signature ties `'a` to `function.status_sources`. The claim "borrowed from the immutable function" is a convention, not a type guarantee. The `authenticate(function, budget)` call is the only link, and it is a runtime check.
  - Say so, or thread `function: &'a ResolvedFunction` and take the key from it.
- Min and max references must not escape the method. They are borrows of `self.nodes`, so holding them across `grow` or `push` would fail the borrow checker. Copy the lengths and p, and re-index, as the old probe does.
- An inherent impl in the child module sees `Directory`'s private fields and methods (`nodes`, `grow`, `rebalance`, `Probe`) only if the module is declared inside `directory`. Confirm that.

## Test gaps to add

- A malformed-invariant check: a debug-only structural assertion costing nothing in release, or an explicit statement that the invariant is trusted.
- Multi-byte UTF-8 where bytewise order differs from naive `char` assumptions, and strings that differ only at the last byte.
- A query below min and above max, with p = 0 and with p equal to a full key.
- A length-2 directory, where min and max are adjacent.
- The same expression with lanes 0, 1 and `MAX`, checking that lane compare happens exactly once.
- Equivalence of the old and new AVL graph and `next` chain over randomized insert sequences.
- A check that no allocation occurs after a failed charge, including at the min/max prepay stage.
