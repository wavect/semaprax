# Review of commit 5d9e6b125 (source-only, nothing executed)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

I found no confirmed blocker in the pasted lines. Everything below the first section depends on code I can't see, and I've labeled it as uncertain.

## Checked and consistent

- **Raw-E stickiness (20–76, 204–212):**
  - `receive` stores the first `Identity` error and never overwrites it (56–58).
  - `substitute` short-circuits once `first` is set (70–72).
  - `populate` takes `raw` before `diagnostic`, and does so regardless of `valid` (204–212). A swallowed sentinel therefore can't mask E.
  - Because storage stops after the first `Diagnostic`, a real semantic error and a raw E can't both occur.
- **Frame order (381–392, 448–459):** `Finish` is pushed beneath the `.rev()` children, so children pop in source order. `Callable` pushes `Enter(result)` below the reversed parameters, so `values` ends up `[p0..pn, result]`.
- **Callable assembly (416–427):** `split` is computed before `pop`. After the pop, `drain(split..)` yields exactly the `count` parameters.
- **Reserve accounting for pushes:**
  - Nominal with `count == 0` gets an explicit `grow` (403–405). Otherwise the drain frees at least one slot.
  - Callable always pops one value, so its push fits.
  - Frame capacity is pre-grown for `n+1` and `n+2` pushes (376–380, 385–389), so the later `extend` calls should not allocate.
- **`into_vec` and `mem::take`:** `children.len()` is read (452) before `into_vec()` (456). The `.map` closure only constructs a `TypeGuard`, so no callback can fail while the types sit unguarded.
- **Early-exit paths:** `?` at 131–137 and 442–447 drops `children` through its guard. A failed push at 248 drops the root `TypeGuard` inside the frame.

## Uncertainties (need code outside the paste)

1. **Unpaid clones (397, 411, 450).**
   - `leaf.clone()` is paid only `size_of::<ResolvedType>()` (396).
   - `declaration.clone()` at 411 and 450 is not paid at all.
   - If `DeclarationId` or a leaf variant owns heap data (`String`, `Vec`, non-`Arc`), these are unpaid allocations. That would contradict the "same byte owner" claim.
   - If they are `Arc` or interned, this is a non-issue.
   - **Please confirm the types.**

2. **Possible double-charge or mismatched charge (244, 442–447).**
   - Line 442 charges `len * size_of::<TypeGuard>()` and line 447 then calls `grow(&mut frames, len+1)`.
   - The frames are `Frame`, not `TypeGuard`, and `Frame::Finish` is larger.
   - If `grow` already charges per slot, the 442 charge is redundant. That would exhaust the cap earlier than the old constructors, against "no cap change".
   - If it pays for something else, such as guard bookkeeping, it needs a comment.
   - Line 244 is the same question for the root.

3. **Per-child `grow(&mut children, 1)` (134).**
   - If `grow` is exact-reserve and re-pays capacity on each reallocation, total charges are O(n²) in child count.
   - The iterator's size is known up front, so one reservation would be cheaper.
   - This depends on `Storage::grow` semantics.

4. **Unguarded window in `storage.receive(result)?` (136).**
   - `substitute` returns a plain deep `ResolvedType` (432).
   - If `Storage::receive` can return `None` on an `Ok(child)` because storage has already stopped, it would drop that type recursively.
   - Line 70 checks only `payer.first`, not storage's stopped state. Line 134's `grow` mostly covers this, but I can't verify it.
   - The same applies to `substituted(...)?` at 370 if its error path drops an owned partial.

5. **`TypeGuard::drop` (36–41).**
   - It relies on `workspace.resolved_type(ty)` being an iterative guard that doesn't pay or fail.
   - If it can panic or allocate, a drop during unwinding could abort.
   - Likewise, `expect`/`assert_eq!` at 33, 425 and 431 become panics that run these drops mid-substitution.

6. **Tests and oracle coverage (120–123, 251–252).**
   - `oracle_work` ticks only in the classifier compute loop (252).
   - The substitute loop (367–430) and `children` (124–140) have no tick.
   - Tests under `cfg(test)` therefore can't bound or observe work in the new substitution path.
   - The comment at 121–123 says no new account is granted, but I can't tell whether that is deliberate or a gap.
   - No tests were included, so I can't assess assertions for raw-E-after-sentinel, first-error source order, or partial-drop on mid-tree failure.

## Not verifiable here

- Whether `substituted()` (370) validates templates in source order.
- Whether `active` is `Copy`. The closures at 382 and 392 capture it by reference and would otherwise fail to compile or alias.
- Whether the key/builtin/text callbacks route Identity errors through `ConstructorPayer::receive`. If any of them bypasses it, raw E from those paths would not be sticky, because `first` is only set in `receive`.
