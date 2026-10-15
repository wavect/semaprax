# f8 fixture review result (held-generic-call-profiles-812)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

**Findings (from the code shown; I can't see `HirValidator::new`, `check`, or the ownership pass, so those points are marked as such):**

1. **The `HirValidator::new(&changed).unwrap()` in the changed-argument loop will probably panic for the new `Bag<Vec<i64>>` case.**
   - The loop bypasses `check` by mutating `params[0].ty` on a resolved clone, but `new` still builds `executable_profiles` and flags for that program.
   - The `Bag<Vec<i64>>` case is the one `check` rejected with T223 and O002. If `new` re-validates signatures or runs the O002 borrow-value rule, the unwrap panics before any parity assertion runs.
   - The same risk applies to the existing `[]`, `[I64, I64]` and missing-nominal variants, because their `new().unwrap()` has never been observed to run (the fixture aborted first).
   - Drive the hostile cases through `HirValidator::new(&changed)` as a `Result`, and assert parity between `Ok` and `Err` outcomes. Don't unwrap.

2. **The second negative can't compare against `verify` alone.**
   - T223 comes from `source_verify::declared_type`, but O002 is a borrow/ownership diagnostic. `crate::check` will return more than `verify(&parsed)` does.
   - Reusing the first negative's `assert_eq!(actual, expected)` shape will fail on the diagnostic list. The expected value must come from the full pipeline stage that emits O002, or assert the exact `[T223, O002]` set and order explicitly.
   - `actual[0].code == "SPX-T223"` also depends on ordering. Pin it, and don't assume it.

3. **`.replace` silently no-ops on a miss.**
   - Adding `rows: borrow Vec<i64>` to `take` and `other` by string replacement can leave one function unchanged if the pattern doesn't match. `take` ends `other: Boxed<i64>)` and `other` ends `value: borrow Bag<i64>)`, so the patterns differ.
   - The tests would still pass on the unchanged function, while the hostile case's `rows` lookup fails or never covers it.
   - Assert after the transform that `rows: borrow Vec<i64>` occurs exactly twice and `items: T` exactly once. Look up the `rows` param by name in `held.take` and fail loudly if it's absent.

4. **Admission of `borrow Vec<i64>` is unverified.**
   - T223 is satisfied (`admitted_vec` covers `Vec<i64>`). However, O002 "borrow value type" is the very rule that rejected the old param, and nothing here shows it admits `borrow Vec<i64>`.
   - If it doesn't, the fixture still aborts, with a different error. Confirm `check` accepts the new source before building tests on it.

5. **Building the hostile type from `rows.ty`.**
   - Construct `Nominal { declaration: "held.bag", arguments: vec![rows.ty.clone()] }` and write it into `params[0].ty` only.
   - Don't also mutate `rows`. `held_tests` assumes `call.parameter(0)` is the changed one, and the `held_entries() == 1` assertions depend on only index 0 being queried.

6. **Direction of the hostile coverage.**
   - The fixture now proves only the admitted `Bag<i64>` path. The nested-`Vec` refusal control lives solely in the changed-program loop and the second negative.
   - If item 1 forces that loop to tolerate a `new` error, the full-receiver assertions (`copy_for_param` vs `borrowed_ownership_copy`, saved vs current argument ownership) never execute for that case. Assert explicitly that they ran.

7. **Two things this change doesn't break.**
   - The `held_tests` loops over all params of all functions, so they now include the two `Vec<i64>` params. They compare saved against current, so equality holds.
   - The budget tests measure `paid` dynamically, so the larger program doesn't invalidate them.
