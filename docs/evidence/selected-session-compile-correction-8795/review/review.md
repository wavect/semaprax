# review (review)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

## Verdict
I see no definite semantic or typing blocker in the pasted hunks. Compile success rests on three unseen items, listed under uncertainties.

## What checks out
- **Tuple types:** `with_limit_usage<T>` returns `(T, bool, usize)`.
  - The first closure returns `owner.usage().1`, a `usize`, so `(base, overflow, _active_used)` types correctly.
  - The `((), overflow, _active_used)` patterns fix the reported `()`-vs-`usize` mismatch, provided those closures really end in `()`.
  - The tail expression `owner.usage().1` holds no borrow, because the tuple is `Copy`, so the drop order of `owner` is not a problem.
- **Baseline field:** `usage()` is `(remaining, physical.charged())`, so `.1` is the selected physical owner usage.
  - The old `(_, overflow, base)` pattern took the wrong field, the unrelated active consumed bytes from `with_limit_usage`'s third element.
  - The new `base` is the right quantity for `base + extra` limits, and `extra = bytes()` is a sound exact boundary.
- **Pairing with `bytes()`:** the `bytes()` tuple matches `Controls<T>` in `paid::acquire` field for field, and `Context<'static>` and `Context<'_>` have the same size. Both sides are an honest independent census with no fitted slack.
- **Ordering:** in the third test, `refusal(...)` runs first and takes its own full controls through the production `acquire`. The test then pays `reserve_bytes(bytes())` followed by `charge(6)`, and only then builds the structural atom. That matches the production order of reserve, then 6 work units, then atom.

## Uncertainties (cannot be settled from the paste)
1. **Visibility and signature of `reserve_bytes` and `charge`.** The replacement calls `owner.budget.reserve_bytes(function, bytes(), label)` and `.charge(function, 6, label)` from the sibling test module.
   - The producer uses `reserve_array::<T>(function, 1, label)` and `charge`, but only from inside `selected_context::paid`.
   - If either method is private to a module the test cannot see, or `reserve_bytes` has a different argument order or return type, this is a new compile error of the same kind you are fixing.
   - Confirm that both methods are reachable from the test module, that `reserve_bytes` returns `Result<_, EngineError>`, and that `EngineError: Debug` (this holds if the old `.unwrap()` compiled).
2. **`reserve_array` and `reserve_bytes` equivalence.** The swap assumes `reserve_array::<T>(f, 1, l)` is exactly `reserve_bytes(f, size_of::<T>(), l)`.
   - If `reserve_array` adds alignment or element-count checks, a different error kind, or different ledger accounting such as a separate array-count charge, the test pays a different amount than production.
   - Error priority could also differ: an overflow check on count × size versus a raw byte limit.
   - The test would then pass without validating the real carrier cost in that path. The label string is also the same, so a label-keyed assertion would not distinguish the two.
3. **Coverage in the third test.** Production `paid::acquire` is no longer exercised directly there, only through `refusal`.
   - A regression in the second `acquire` call, for example a changed `Controls` tuple, would be caught only by the first test's `bytes()` comparison.
   - That is acceptable if intended, but the comment "Independent full Controls<Context> census" overstates what this test checks.
4. **Exact-limit boundary.** `(bytes(), true)` at limit `base + bytes()` assumes that, after the controls are reserved, the structural receipt atom consumes only work units and no further physical bytes.
   - The same assumption applies to `bytes() - 1` being false.
   - If the atom or report backing charges bytes beyond the controls, the exact case would fail.
   - It also assumes `acquire(&context, function, limit)` applies `limit` only to the physical ledger and that its owner cost is deterministic across the `usize::MAX` baseline and the limited runs.
   - I cannot see the `acquire` helper or the asserts in the elided lines, so this is unverified.
5. **Closure return types.** The `()` patterns in the second and third loops assume the closure bodies, which I cannot fully see, end in a statement rather than a value-bearing expression. The `measure(...)` inner closure is also elided.

## Minor
- `_active_used` is deliberately unused, so there is no warning. Under `-D warnings` the three identical shadowing names would also be fine.
- `base + extra` cannot overflow here, since `base` is a small charged count and `extra` is at most `bytes()`.
- I cannot confirm the removed `super` import is truly unused, or that the six unrelated warnings stay unchanged. Both need a compile.

## Execution
Nothing was executed, and this is a static read only. Before relying on it, a `cargo test --no-run` is needed to confirm items 1 and 2.
