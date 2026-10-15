# claude review response (paid-receiving-directories)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Source-only review. I couldn't see `receiving_before.rs`, `ReplayBudget`, `Lookup`, or the surrounding types, so anything depending on them is listed as an unknown API. I found no definite compile error, but the items in section 1 would each break the build if the assumption fails.

## 1. Type, lifetime and Clippy

**Probable blockers, each depending on an API I can't see:**

- **Glob-chain names in `directory.rs`.** It names `ExitTargetId`, `LivenessFlagId`, `StatusCase`, `EdgeId`, `StatusSourceId`, `ExpressionId`, `Diagnostic`, `ReplayBudget` and `ResolvedFunction` through `use super::*`. `receiving.rs` never names `ExitTargetId`, `LivenessFlagId` or `StatusCase`, so nothing here proves they reach `expression_facts` scope. Unresolved names would give E0412 or E0405.
- **`numeric_key!` coverage.**
  - `CleanupTerminator::Exit(exit)` must hold exactly `ExitTargetId`, or `referenced_exits.insert(..., *exit, ...)` fails E0277.
  - `normalized_cases` elements must be `StatusCase` and `Copy` (`*case`).
  - If any listed type is an alias of `u32`, it collides with `impl KeyWork for u32` (E0119).
  - The element type yielded by `validate_place(..)` must be `KeyWork + Copy`, or a reference to one. Both the `Transfer` and `consumed` directories rely on this.
- **`entries_paid` element shape.** `expected_calls.insert(function, expression, ...)` and `committed_calls.same_keys(&expected_calls, ...)` require identical `K`. `committed_calls` is `Directory<&ExpressionId>`, so `entries_paid` must yield `&ExpressionId` and not an owned `ExpressionId`. `Lookup` is used as `dyn`, so it can't return `impl Iterator`. The element must be an allocated `Vec` or a boxed iterator, and that allocation is outside this review. Elision with `&self` ties the borrow to `facts` rather than `budget`, so there is no E0499 against the later `budget` uses.
- **`E: From<Diagnostic>`.** `Directory::new(..)?` and `insert(..)?` inside `validate_status_sources_core<E>` convert `Diagnostic` into `E`. `replay_error(..).into()` already depends on this, but confirm the `semantic_refusal::Error` bound guarantees it.
- **`ExpressionId::as_str()` and `budget.budget_refusal(function, detail, "")`.** I can't verify either signature or what the empty third argument means.
- **Test-only unknowns:**
  - Direct access to `ReplayBudget.remaining` from `receiving::tests` fails if the field isn't visible there.
  - In `complete()`, the `PhysicalReservation::new(..)` temporary sits inside the `let facts = ...?;` statement. It is a build error (E0716) if `arena::build`'s result borrows from the payer.

**Checked and fine:**

- `Directory<&StatusSourceId>` passed as `&Directory<&StatusSourceId>` with independent elided lifetimes compiles, because `Directory<K>` is covariant (`Vec<Node<K>>`, `usize`, `&'static str`). Don't add `Cell`, `PhantomData<fn..>` or similar to it.
- `contains(function, &source, budget)` with `source: &StatusSourceId` is a required `&&`, not a `needless_borrow`.
- `&T` blanket impl: `(**self).comparison_units(*other)` calls the inner impl, not itself.
- Disjoint field borrows in `reserve_vec(function, &mut self.nodes, 1, self.phase)` are fine.
- The `ok_or_else(|| overflow(.., budget, ..))` closures end their `&mut budget` borrow before the next `budget.charge`.
- `validate_blocks_and_edges` has exactly 7 parameters, so `too_many_arguments` doesn't fire.
- The `(|| { ..? })()` idiom in tests is exempt from `redundant_closure_call`.
- Style nit: `let source_flags = source_directory;` and the matching `destination_flags` rebind are redundant.

## 2. AVL and failed-mutation behavior

- **Ordering and successor links are correct.** `before` is the last node where the walk went right, and `after` is the last where it went left. Rotations don't touch `next`, so iteration order is preserved.
- **Rotations and rebalance are correct.**
  - `attach`, parent fixups, and the `update(at)` then `update(top)` order are right.
  - The double-rotation case and `top.parent` climbing are right.
  - `left` is stale after the inner rotation but isn't reused.
- **Failure leaves the structure inconsistent.**
  - `grow`, the `VISIT_UNITS` charge and `probe` all fail before mutating, so those failures are clean.
  - A failure inside `rebalance` (budget charge, or `update` overflow) occurs after the node is linked. The BST order and `next` chain stay valid, but heights and balance are stale and `balanced()` would be false.
  - Every caller `?`-propagates and drops the directory, so this is safe only if the budget is sticky and refuses reuse.
- **Stickiness is entirely external.** `authenticate` and `new` rely on `reserve_bytes(.., 0, ..)` and `charge(.., 0, ..)` failing after any earlier refusal. The tests assert exactly this (`remaining` unchanged and no allocations on the second `directory_run`).
- **Overflow paths may not latch.** If `budget_refusal` only builds a `Diagnostic` and doesn't latch the budget, the overflow paths in `probe`, `grow`, `update` and `charge_pair` break that guarantee. They are practically unreachable, but they differ from charge-driven refusals.
- **No test covers refusal mid-`rebalance`.** Refusal at the exact `required - 1` limit is exercised, but nothing reuses the directory afterward.

## 3. Retained backing funding

- `grow` charges `len + 1` relocation units only when `len == capacity`, then calls `reserve_vec(.., 1, ..)`.
- Whether the retained bytes are funded depends on `reserve_vec`. Check these three things:
  - It must pay the capacity actually allocated: if `Vec::reserve` is amortized, the capacity can exceed the bytes paid for.
  - It must pay the new array while the old one is still live.
  - If it uses `reserve_exact`, every push relocates (still charged), and the `capacity == directory.capacity()` assertions still hold.
- The test asserts `allocated <= required` and `highwater <= required`. This only works if `reserve_vec` funds the real capacity, so it is a good tripwire.

## 4. Key, lane and duplicate ordering

- Every directory uses `Ord::cmp` on the real key, so order matches the old `BTreeSet` as long as `StatusSourceId`'s derived field order is unchanged. `comparison_units` is independent of that order and bounds the cost either way.
- Verify that `ExpressionId: Ord` really is byte order over `as_str()`. If it has a custom `Ord` or extra fields, `min(len) + 1` undercounts.
- Duplicate priority is preserved in this file:
  - `RepeatedStatus` fires before `UnknownStatus`.
  - `committed_calls.insert` runs before the lexical-control exemption and before the params and arity checks.
  - `selected.replace` is unchanged.
- Under budget exhaustion, the first diagnostic can now be a budget refusal during the status-directory build, before edge errors. The old `BTreeSet` couldn't refuse, so this is expected, but the first-diagnostic comparison test only runs unrefused.
- `expected_calls` is built (and paid) even when `lexical_control.is_some()`, where the comparison is skipped. If the old code didn't build it in that profile, this is a seam difference.

## 5. Old and current seams

I couldn't diff against `receiving_before.rs`. These are the places to compare textually:

- **Cases:** the `cases.is_empty() || cases.len() != normalized_cases.len()` logic.
- **Transfer:**
  - Both flag sets are now directories.
  - `validate_place` itself may still return an unpriced `Vec` or `BTreeSet`, and `TransferVariant` still uses its raw `.len()`. That is the preserved-unpriced residual you described.
- **Consumed flags:** the `consumed.insert` overlap check.
- **Call commits:** the lexical-profile exemption.
- **Unchanged branches:** `Facts::Tree` early returns are unchanged.
- **Test limits:**
  - The comparison only covers the existing functions plus four attack shapes (status duplicate, unknown status, bad edge id, repeated commit).
  - The transfer, consumed and cases branches have no hostile cases.
  - The full-boundary test does not assert that old and current agree on the raw cost. It only checks that each is internally exact, short and zero.

## 6. Control assumptions in the tests

- `required - 1` is safe only because `required > 0` is asserted first.
- The "exact" test expects `remaining == 0` after the exact run. That holds only if `unobserved()` routes `reserve_vec` and `reserve_bytes` through `remaining` the same way as `charge`.
- The short-run test asserts a specific message substring, "cleanup replay work budget exhausted". It would fail if byte refusals use different text.
