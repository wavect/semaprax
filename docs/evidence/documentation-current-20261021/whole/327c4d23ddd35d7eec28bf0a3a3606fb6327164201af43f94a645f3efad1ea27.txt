# Review of #780 early loan-candidate dispatch (source-only, nothing executed)

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

The paste has its blank lines stripped, so I cite code anchors instead of line numbers. Several findings depend on signatures that are not in the packet, and I mark those as uncertain.

## Blockers (fix or justify before merge)

**B1. `ptr::eq` on `&dyn ReplayContext` in `Slot::region`.**
- Anchor: `!std::ptr::eq(context, self.context)`.
- `ptr::eq` on trait-object pointers compares the vtable pointer as well as the data address. Rust documents that vtable addresses are not guaranteed unique per type, so the same object can compare unequal.
- `Slot::new` and `Slot::region` coerce `&ResolvedProgram` to `&dyn` at different sites. In the adopted callers, `ByteTraversalSlot::new(program, function)` and `byte_census.region(program, function, expression)` are separate coercions.
- A vtable mismatch would turn a valid call into the "census function differs from its current receiver" semantic error.
- The check should compare data addresses only, with `std::ptr::addr_eq` or a cast to `*const ()`.
- Current rustc may also warn through `ambiguous_wide_pointer_comparisons`, which would fail a `-D warnings` build.
- This is a latent, non-deterministic hazard rather than proven breakage.

**B2. Foreign and sticky semantics are untested.**
- The nested-slot tests never exercise the foreign context/function rejection in `Slot::region`.
- They also never test reuse after `self.proof` is `Some`, where every later expression, including non-nested ones, bypasses `super::super::region` and goes through `Census::region`.
- Nothing shows that census answers match strict v1 for non-nested expressions once the slot is sticky.
- The fixture only demonstrates that plans are equal. The test also never checks that functions with no nested traversal leave `proof` as `None`.

**B3. "Unchanged" claims are only partly tested.**
- Only `saved_planner::build_plan` is pinned to the legacy diagnostic.
- Nothing pins `paid_candidate_before` or the legacy path.
- Nothing pins the strict v1 `region` result for a nested expression. For example, a direct `crate::hir::byte_traversal_region` call on the nested expression should still equal the six-field diagnostic.

## Uncertainties (need signatures or code outside the packet)

**U1. Lifetime of `Region<'a>`.**
- Anchor: the fallback arm of `region`, `let region = proof.region(...)?; self.proof = Some(proof); Ok(region)`.
- This compiles only if `Census::region` returns `Region<'a>` tied to the `expression` and `function` lifetime, not to `&self`.
- If `Region` borrows from `&proof`, then moving `proof` into `self.proof` is a borrow error, and so is returning from the cached `&self.proof` branch.
- The same bound is needed at `region_for`, which passes `self.function`; that is fine under two-phase borrows.
- Related: `Slot<'a>` is invariant behind `&mut`, and `region` requires `expression: &'a ResolvedExpr`. Both callers pass expressions drawn from `function`, but if `expressions` is a collected `Vec<&ResolvedExpr>` with a shorter lifetime than the one `new` was given, inference can fail.

**U2. Budget refusal is reported as a `Diagnostic`.**
- Anchor: `Failure::Storage(Refused::Budget).diagnostic()`, propagated via `?` from `new` and `region`.
- `PlannerError` distinguishes `Semantic` from `Physical`, and the test helper `semantic` panics on `Physical`.
- If `?` on a `Diagnostic` becomes `PlannerError::Semantic`, a physical or budget refusal inside `Slot` is indistinguishable from a semantic rejection.
- A caller could then treat budget exhaustion as an invalid program.
- The short-limit loop only checks `is_ok()`, so it would not catch a wrong error class.
- This is the highest-value thing to confirm, since the `Physical` classification is otherwise the point of the paid carriers.

**U3. Which payer is charged.**
- `Slot` uses `Storage::acquire()` and the active-output limit, not the candidate's `payer` (`CfgReservation`).
- The failure branch of test 2 asserts `owner.reserve(0).is_err()` and `owner.reserve(1).is_err()` after any failed build.
- That holds only if a `Slot` or `Census` refusal poisons `owner`. If the refusal comes from the global limit, the owner may stay usable and the assertion would fail.
- The `exact - 1` iteration is where this would show.
- The `0` iteration returns before this check, because `CfgReservation::acquire()` fails first.

**U4. Eager rather than lazy cost.**
- `Slot::new` calls `Storage::acquire()` and reserves `size_of::<Self>() + size_of::<Result<Self, Diagnostic>>()` for every function, including those with no nested traversal. This contradicts the module doc's "lazily".
- The `Result<Self, _>` term includes `Self` again, so the debit is roughly double counted. That is conservative, not unsound.
- The debit increase is accepted, but other exact-capacity or exact-debit tests outside the packet may now be off.
- It is unclear whether `Storage::acquire()` succeeds when no limit is active, which matters for the ordinary `has_own_root_candidate` path. It is also unclear whether it can fail inside an already-held storage context.

**U5. Caller coercion and error types.**
- `ByteTraversalSlot` is not defined in the packet. It is presumably an alias or re-export of `Slot`.
- The callers need `ResolvedProgram: ReplayContext` for `new(program, function)` to coerce.
- Both `?` sites previously propagated a `Diagnostic`, so the conversion is probably fine.

## Test-quality notes

- **Test 1:** `assert_eq!(expected, function.loan_plan)` is weak if resolve populated `loan_plan` via the same `build_plan`. It is also only the ordinary path. No limit-driven test covers `has_own_root_candidate`, so the physical coverage is funded-only.
- **Test 2:**
  - `exact` is whatever the unlimited run consumed, and the test assumes it is the minimum that succeeds.
  - It asserts `refused == (limit != exact)`, which depends on `Slot::controls` setting the refused flag on every denial. It does so only through `reserve_active_required`.
  - It does not assert which error class occurs below `exact`.
  - `exact - 1` is safe only because `exact > 0` is asserted earlier.
- **`nested_refusal`:** it matches only on `code` and `message`. The six-field equality is tested only in `saved_planner`, so a diagnostic with the same message but other differing fields would trigger the census fallback.
- **Mechanics:** the `include_str!` path (four `..` from `retained/`) and the `mod nested_tests;` placement look right. The imports all appear used.

## Verdict

B1 is a concrete fix, and B2 and B3 are test gaps. U1 and U2 decide whether the change compiles and classifies refusals correctly. I'd treat U2 and U3 as likely blockers until the `Diagnostic`-to-`PlannerError` mapping and the payer poisoning are confirmed.
