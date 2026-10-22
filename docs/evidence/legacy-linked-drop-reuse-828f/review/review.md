# Source-only review (nothing executed)

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

## Blockers (verify first; each depends on code not pasted)

1. **The new `Drop for DeclarationIndex` is not in the paste.** This is the code under review. I can't confirm any of these claims:
   - the paid workspace arm is byte-identical;
   - `dispose` replaces only the old no-workspace fallback;
   - the `CaseFields::Owned` path, with its `Unit` replacement, is routed correctly.

   `tests.rs` pins only the *old* body, as text. Nothing pasted exercises "paid arm unchanged" at runtime.

2. **`tests.rs:~40-45`: `index.case_fields.insert(id, fields)` with `fields: Vec<ResolvedFieldDeclaration>`.** You describe a `CaseFields::Owned` enum. If the map value type is `CaseFields`, this doesn't compile, and if it does compile, the test isn't building the `Owned` shape you describe. Confirm the value type. If it is `CaseFields`, the tests don't exercise the claimed Owned/drain-to-`Unit` path.

3. **`legacy_drop.rs` `match current { Nominal { mut arguments, .. } … }` and `Function { mut parameters, result }` move fields out of `ResolvedType`.** This is E0509 if `ResolvedType` (or a field type) implements `Drop`. The pasted code doesn't show that it doesn't. The `let/ownership` structure is otherwise sound: `current` is reassigned on every path before the next loop-top `match`, including the partially-moved `Nominal` with empty `arguments`.

4. **`_ => {}` leaves other recursive variants to ordinary recursive `Drop`.** If `ResolvedType` has any other variant holding child types (tuple, reference, array, optional, and so on), a deep chain through it overflows the stack. The old `before.rs` handled only `Nominal`, so this isn't a regression, but "iterative stack safety" holds only for `Nominal` and `Function`. The variant list isn't shown.

## Semantics and accounting (reasoned from source)

- **Unary and deep path.** The reasoning holds. After `pop`, `arguments` has `len` 0 and the old capacity of 1. `pending.is_empty()` is true, so `pending = arguments` drops the previous backing and adopts the new one. Net: zero new allocations and one free per level. Multiple roots work because `pending` is empty again when each root ends.
- **Function roots with empty `parameters`.** `pending = parameters` swaps in a zero-capacity `Vec`, which is harmless. This is a behavior *improvement* over `before.rs`, which let a `Function` root drop recursively through the closure's by-value `ty`. The `functions=true` test therefore compares against different old behavior than the "Nominal-only" old path. This is fine, but the test names don't say so.
- **Capacity discard.** `pending = arguments` when `pending` is empty throws away a possibly larger backing for a smaller one, so a later wide frontier can reallocate. You disclose this. It makes the zero-allocation claim true only for unary or empty-parameter shapes.
- **Branching test arithmetic.** 17 collected arguments give capacity 17. After one pop, `pending` has 16 elements. The next `Function` appends 9 parameters, which needs 25 and forces a realloc. The test name "keeps every backing" overclaims. It asserts only the ledger and `allocated == freed`, so it can't catch a regression in allocation behavior. It guards only against leaks and double-free.
- **Old-path measurement (test 1).**
  - `drop_before_argument_reuse(mut self)` takes `self` by value, so the **new** `Drop` runs at the end of that function on the already-drained index. Its cost is probably 0 if the drain leaves `Unit` fields, but that is unverified. It's `deltas[0]`'s window, so a non-zero cost there would distort the `4*size_of` assertion.
  - The `4×` figure matches `Vec::extend` from a `Vec` `IntoIter` with `size_hint` 1. The 192-byte difference you observed equals 4×48, so the constant is consistent with the source but not proven as the cause.
  - `deltas` is inferred from the assignment of `current_metrics().allocated - stopped`. If `allocated` is `u64`, `assert_eq!(deltas[0], 4 * size_of::<_>())` is a `usize` vs `u64` mismatch.

## Test validity

- **Test 4 trailing newline.** `before.rs` as pasted ends with extra blank lines. If the file ends in `"}\n\n"`, `strip_suffix('\n')` leaves `"}\n"`, and the equality against the evidence file may fail. This depends on the exact file bytes, which I can't see.
- **Test 4 scope.** It proves text equality of the old body and of `legacy.rs` with evidence copies. It does **not** run the "existing mixed-case 512 owner" scenario you cite (828f/42 vs 512), so the test name overclaims.
- **Evidence paths.** The relative `include_str!` paths resolve correctly from `legacy_drop/` (root at `../../../..`, sibling at `../linked_owned_data/tests/legacy.rs`), assuming those files exist.
- **Test 2 coverage gap.** The `Function` spine is built with empty `parameters` only. The non-empty-parameter-with-pending path (`append`) is exercised only in the branching test, which asserts no allocation bounds.
- **Test 3 (`with_limit_usage_refusal(0, …)`).** The assertions after `drop` do check that the ledger isn't spent. They don't check the drop's own allocations are zero, which is consistent with your disclosure but means the "unfunded" claim is untested beyond not-spending.

## Uncertainties (not blockers)

- `drain_legacy_type_roots` and `drain_recursive_types_for_private_contract` behavior (does draining leave `Unit` so the trailing `Drop` is free?).
- Whether `before` and `tests` unused-import or dead-code warnings under `#![deny(warnings)]` apply, since `dispose` is `pub(super)` and its caller isn't shown.
- The stack-safety argument for `CaseFields::Owned` rests on `drain_recursive_types` replacing every field `ty` with `Unit` *before* the original drop. That is described, not shown.
