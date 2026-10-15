# review (empty-postcondition-scope)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

**No concrete blocker found.** I read the source only; nothing was executed and no test results are claimed.

**Does the omitted local carrier have a consumer? No.**
- The only reader of the ensures-side scope is `validate_contracts_persistent(..., &ensures_scope, "ensures")` at `function_scope.rs`.
- With empty `ensures`, that function returns `Ok(())` at `contract_owners.rs` (`if contracts.is_empty()`) before it touches `outer`. `fresh_baseline_persistent` is never reached.
- `ensures_scope` is a local that is dropped at function exit. It is never passed to `Publication`, stored on `self` or returned.
- The validator-level `self.insert_value(&function.result_id)` still runs before the new guard, so duplicate-id and result-identity authority is unchanged.
- The body is validated against `scope`, which never contained the result binding, so body validation is unaffected.

**Ordering and typing**
- The guard sits after the result-identity check, the `Str`/`SliceU8` return rejections, `require_type`, and the `returned` ownership check. It is also after `expected_ownership`, so every prior error still precedes success.
- `return charge::reserve(Some(0))` has type `Result<(), Diagnostic>` and is visible at that path (`pub(in crate::hir::validation)`).
- `scope` is moved only on the non-empty path, so there is no borrow or move conflict.
- `reserve_active_required(0)` fails under a sticky refusal, so success cannot be reported after a refused payer. This is the intended difference from `before.rs`: the old `prepare_insert` carrier charge and `clone_type` charge are gone from the empty-ensures path.
- The non-empty path is textually unchanged, which the `assert_eq!(actual, old)` and `assert_eq!(paid, old_paid)` branch in the test relies on.

**Fixture check**
- The source-binding test (`tests.rs`, `whole_current_function_is_exact_prior_source_plus_named_empty_guard`) should hold if `additive_guard` is exactly the comment plus the `if` block and `registration` is exactly the `#[cfg(test)] mod empty_ensures;` lines.
- `before.rs` ends with `    }\n\n}\n`, which matches the expected `original[start..end] + "}\n"` slice.

**Not verifiable from what you pasted** (not blockers, nothing here is shown to be wrong):
- `empty_ensures/mod.rs` or `empty_ensures.rs` is not shown. It must declare `mod before; mod tests;` and bring in `super::*`.
- In `lifetime()`, the validator is built outside `measure` and dropped inside it, and the test asserts `allocated == freed` and `live == 0`. This holds only if `HirValidator::new` makes no heap allocation, or if `measure` tolerates frees of pre-existing allocations. This is the most likely place for a fixture failure, but I can't tell without `test_allocation_observer`.

Local verification of packet omissions: empty_ensures.rs declares both before/tests and imports its parent. test_allocation_observer.rs139–149 explicitly returns on unknown/preexisting frees, as required by the unchanged full function receiver observation. No Rust execution.
