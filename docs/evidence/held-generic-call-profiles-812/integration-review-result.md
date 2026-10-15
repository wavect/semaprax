# integration review result (held-generic-call-profiles-812)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

**No definite blocker found in the supplied source.** Nothing I checked fails type, visibility, lifetime or module resolution on its own text. Several checks depend on items that were not provided, so I list those as uncertainty and not as defects.

## Checked and consistent

**Test module privacy and associated items**
- `held_profiles.rs:239` declares `pub(super) mod tests`, and `fixture()` is `pub(in crate::hir::validation)`. `call_parameters/held_tests.rs` reaches it through `super::super::held_profiles::tests::fixture()`, and `call_parameters` and `held_profiles` are both under `validation`.
- `tests.rs` is a child of `held_profiles`. It can therefore construct `Key { slot, parameter }`, name the private `Slot`, read `bound.held`, and call the `#[cfg(test)]` methods `len` and `owned_capacity`.
- `use super::*` also brings in the private imports `paid`, `DeclarationId` and `ResolvedProgram`. A local `paid` binding in the tests shadows only in the value namespace, so the `paid::fund` path is unaffected.
- `before_812::validate_argument_ownership_view_saved_812` is `pub(super)`. Its `super` is `call_parameters`, which contains `held_tests`.
- `saved_observations_812` and `before_812::Lookup` methods are `pub(super)` and used only from `executable_profiles`. The local `Lookup`, `Entry`, `State` and `Observation` in `before_812.rs` shadow the glob imports.

**Sealed `Call` / `Parameter` ownership and lifetimes**
- `Call<'a>` and `Parameter<'a>` hold only `&'a ResolvedProgram`, a `Copy` slot and an index. Every derived `Copy` is valid.
- `Call::function` and `Parameter::param` return `&'a` through a copied `&'a` reference, so nothing is tied to the `&self` borrow.
- `Bound::call(&self) -> Option<Call<'a>>` is explicitly `'a`. So `Bound::new(&foreign).call(..).unwrap().parameter(i)` in `tests.rs` does not borrow the dropped temporary `Bound`.
- `ParameterView::held_view` takes `&parameter.ty` and `&parameter.id` from `carrier.param()` (`&'a`) before moving `carrier` into `held`. That is fine.
- The `detached.ty = &temporary` assignment in `held_tests` relies on `ParameterView` being covariant. All its fields are covariant, so it compiles.
- `CallParameters::Held(Call<'a>)` and `map_or(CallParameters::Borrowed(..), CallParameters::Held)` unify under covariance.

**Callbacks and the long-lived lookup**
- In `runtime_admitted_param` and `copy_for_param`, the `RefCell` borrow is confined to a block whose result is a plain `Option<(Key, Option<bool>)>`. Its tail temporaries (`Option<&Bound>`) have no drop glue, so there is no E0597 and no borrow is held across `compute`.
- `compute: impl FnOnce` is moved in mutually exclusive diverging branches, then once at the end. That passes move checking.
- The test closures capture `&validator` and `&Cell`. They do not touch `lookup`'s `RefCell` during the callback, so there is no re-entrant `borrow_mut` hazard at compile level.
- `drop(lookup)` in `receive` is legal while `param` and `call` are live, because those carry the program lifetime and not the lookup's.

**Const-fn visibility for the closed operations**
- `MapOp::id`, `ListOp::id` and `IteratorOp::id` are all `pub(crate) const fn` with literal or const-only match arms. `CompilerDeclaration::text` calls them from a `const fn`, so those const calls are valid.
- The three new variants hold types that derive `Clone, Copy, Debug, Eq, PartialEq`, so the enum's derives still hold.
- `DeclarationId::compiler` is `const fn` and has no cycle.

**Fixture and constructor assumptions**
- The raw-string literal has no `"#` inside it. Both `.replace` patterns exist in the source, and the replacement produces `Bag<T>{items:T}` with `borrow Bag<Vec<i64>>`.
- `ResolvedExpr` literals in `fixed_operation_tests.rs` use the same five fields as `map.rs`, `list.rs` and `iterator.rs`. `ResolvedParam` literals use the same five fields as `map_ops.rs`.
- The `view.ty` fix is in place. The only remaining `.ty` assignments are on `argument`, which is in scope.
- `include_str!` relative paths resolve correctly from every file shown (`held_profiles/`, `ids/`, `resolve_box_call/*/`).

## Uncertainty that could still stop compilation (not shown, so not asserted)

1. **`src/hir/validation/executable_profiles/held.rs:~108`**: `super::super::ownership_flags::Payer::authenticate()?`. I could not see `Payer`'s visibility or return type. It must be reachable from `validation` descendants and yield `Result<_, Diagnostic>`.
2. **`validation.rs` `held_parameters` binding**: the span assigns `held_parameters` inside the `else` branch and reads it later through `borrowed_parameters`. This needs an earlier `let mut held_parameters: Option<Call<'_>> = None;` outside the shown span. It is probably already integrated, but I could not confirm it.
3. **`held_tests.rs` `let (validator, _) = with_limit(usize::MAX, || ..)`**: this assumes `with_limit` returns a pair. Other uses ignore the return value, so they are consistent with either shape. If `with_limit` returns a bare `T`, this one line fails.
4. **`crate::bounded_output::active_refused`, `active_remaining`, `reserve_active_preserving`, and `test_allocation_observer::current_metrics`**: I could not see these signatures. `held_profiles.rs` and `held_tests.rs` use them.
5. **`crate::parse`, `crate::verify::verify` and `crate::check` error types**: the T223 test needs `actual == expected`, so the `Err` type of `check` must equal `verify`'s return type, with `PartialEq + Debug`.
6. **`HirValidator: Clone`**, **`require_type(&ResolvedType, &ResolvedType, &str)`**, and **`HirValidator::with_profiles`** argument order. These match the visible call shapes but are not shown.
7. **Existing test files that could break on the new fields**:
   - `executable_profiles/tests.rs` would break if it builds `State { entries }` literally, since `State` gained `bound`.
   - `call_parameters/tests.rs` or `copy_*` tests would break if they construct `ParameterView { .. }` literally, since it gained `held`. Neither file was provided, so I can only flag this as an assumption I could not check.
8. **`validation.rs` registration**: `mod held_profiles;` and its `Bound`/`Call` visibility are used throughout. I did not see the declaration line (outside the supplied spans). Other files use it through `use super::*`, so it is presumably present.
9. **Dependencies**: `serde` (with `derive`), `serde_json` and `sha2` must be available to the test profile for `source_audit.rs` and `fixed_operation_tests.rs`.
10. **Runtime (not compile) assumptions**:
    - The changed-arguments loop calls `HirValidator::new(&changed).unwrap()` on a program whose `held.take` parameter 0 was rewritten to a wrong-arity or unknown `Nominal`. I could not tell whether `ConstructorSeed::authenticate` rejects that. If it does, the `unwrap` panics, which is a test failure and not a build error.
    - `ExpressionId::new(owner, &str)` and `ResolvedExprKind::Int(i64)` are inferred from usage, not seen.

## Non-hazards, for the record

- `Slot`, `Key`, `Call` and `Parameter` have no `Debug`, and no assertion in the shown tests formats them. All `assert_eq!` operands are `Option<bool>`, `Result<bool/(), Diagnostic>` or `Option<bool>`.
- The old-receiver comparison keeps full old diagnostics: unbound, foreign, detached or mismatched carriers all fall to the complete `runtime_admitted` / `compute` route. I saw no path that relaxes receiver, effect, type or ownership checks.
- I made no claim of generic admission expansion or actual gain.
