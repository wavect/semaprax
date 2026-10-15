# Review of the storage-only linker diff

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Nothing was executed. Findings come only from the pasted diff and helper snippets.

## Blockers (contingent on code not shown)

**B1. Disposal of shared case owners (new lines ~170-180, `CaseFields::drain`).**
- `CaseFields::drain` does nothing for `CaseStorage::Case`. Each shared case entry now holds one extra strong reference to the same `cases` owner that `variant_cases` also holds.
- If shared-payload draining uses uniqueness (`Arc::try_unwrap` or `get_mut`) to dispose of types iteratively, the order of draining matters.
  - When `variant_cases` is drained first, its refcount is greater than 1 because the `case_fields` entries still hold clones. Iterative disposal would be skipped.
  - The last reference would then drop when `case_fields` is dropped, using native recursive `Drop`. That is a stack-overflow risk for deeply nested hostile types.
- This is a blocker unless the index's drain path either drains `case_fields` before `variant_cases`, or has `Case` entries release their reference before the owner is drained.
- Your authored "complete input/index disposal lifetime" control only covers this if it uses deeply nested types and checks the disposal callback count. Please confirm that it does.
- The same concern applies to the caller's input. If the input `cases` Arc is still alive when the index is drained, uniqueness fails. Check that the old and new paths behave identically in that case.

**B2. `.into()` conversion (new line `case.fields.clone().into()`).**
- The diff does not show the type of `case.fields` or the `case_fields` map value type. The old code inserted `case.fields.clone()` directly.
- The new code requires `From<typeof case.fields> for CaseFields`. Confirm that this impl exists and that it produces `CaseStorage::Owned` without extra allocation.
- If `case.fields` is `NominalPayload<ResolvedFieldDeclaration>`, the clone for an `Owned` payload deep-copies the field Vec. That matches the old behavior.
- If the map previously stored `NominalPayload`, the `case_fields` type changed outside the diff. The statement "unchanged except diff" would then be false.
- Also confirm that `nominal_payload::map::CaseFields` resolves through `super::super::*`. If that path is private to a sibling module, the build fails.

## Uncertainties

**U1. Copy-on-write aliasing.**
- `CaseFields::Case` reads fields via `owner[ordinal]`, and the owner is a shared Arc.
- If any index mutation path uses `Arc::make_mut` on `variant_cases` or a similar payload, a clone would diverge from the `case_fields` view. This is safe only if shared payloads are strictly immutable.
- The code comment asserts immutability, but nothing enforces it.

**U2. Consistency between `ordinal` and `case.id`.**
- Ordinal comes from `cases.iter().enumerate()`, and `variant_cases` stores the same `cases.clone()`. This is consistent as long as `iter()` order matches indexing order.
- Confirm that `NominalPayload::iter` and `Index<usize>` or `len()` agree, particularly for `Shared` storage.
- `canonical` asserts `is_shared` and `ordinal < len`. Those asserts are unreachable here, so panics are not a concern.

**U3. Method availability.** `cases.iter()` and `cases.is_shared()` must be callable on the binding type. The old loop was `for case in cases`, which suggests `cases` is `&NominalPayload`, so `.iter()` is likely fine. This is only a compile-time risk.

**U4. Shadowing.** `let fields` inside the `Variant` arm shadows nothing live, since the `Record` arm's `fields` is out of scope. It still reads confusingly next to `record_fields`, so consider renaming it to `case_fields_value`.

## Verified from the snippet (no issue found)

- **Error precedence and order:** The `require_fact` and `insert_*_view` sequence is the same as before. The `case_fields` insert still happens after the field loop, so partial-map state on error matches the old path. The new code adds nothing that can fail before the insert.
- **Allocation:** For shared owners, the new path adds one Arc increment per case and no Vec or Arc allocation. This matches the stated claim.
- **Record path:** The record branch still does `fields.clone()`. That is a refcount bump for shared payloads and a full clone for owned ones. It is unchanged and out of scope, but the "no per-case alloc" claim should not be read as covering records.
- **Owned path:** `variant_cases.insert(..., cases.clone())` clones the full Vec when `cases` is owned. That is existing behavior.
- **Module paths:** `#[cfg(test)] mod original;` in a non-`mod.rs` file resolves to `linked_owned_data/original.rs`. Confirm the files are placed there.

## Test gaps to check (the controls are unexecuted)

- **`CaseFields` equality:** Comparing "complete maps" between old and new routes needs a `PartialEq` or an accessor-based comparison. A `Case` view and an `Owned` view must compare equal by content, not by representation. If the tests compare via accessors, make sure they cover every ordinal and not just ordinal 0.
- **Allocation counter:** It must be scoped to the link call only, excluding test fixture construction. Also assert strictly fewer allocations only on the shared input and equal allocations on the owned input.
- **Missing cases:**
  - A shared variant with zero cases.
  - A shared owner whose inner `case.fields` are themselves `Owned` or `Shared`, to confirm they are not double-counted.
  - A failure midway through a variant after earlier `Case` views were inserted. This checks that partial maps drop cleanly and that disposal is not skipped for the already-retained owner references (ties to B1).
- **Error-precedence tests:** The listed missing / wrong-kind / wrong-owner / unrelated-fact / duplicate-type cases are good. Add a mixed case where two different errors are possible, to pin which one wins. This matters because the old function is the oracle only if both are run on the same input and compared by error value.
- **Original functions:** "Verbatim apart from 3 names" is a claim a reviewer cannot verify from this diff. The old implementation is the oracle, so a diff of `original.rs` against the prior function body should be attached.

## Summary

The logic is correct for the order of effects and allocation. B1 (disposal order and refcount uniqueness) and B2 (the `From` conversion and map type) need confirmation from code outside the diff before merge.
