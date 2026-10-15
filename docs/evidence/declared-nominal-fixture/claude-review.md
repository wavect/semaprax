# claude review (declared-nominal-fixture)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

**Verdict:** the logic holds up on the code shown. I found no definite errors. The remaining risks are things I couldn't see, mainly whether the new fixture source passes `check`/`resolve` and whether a few identifiers resolve.

## Definite (from the supplied source)

- **Generic positive receiver:** `CollectionGeneric<i64>` correctly keeps the diagnostic path alive.
  - `owned_collection_record::admitted` returns false at its first guard because `arguments` is non-empty. The new `assert!(!admitted(..))` is therefore sound.
  - `runtime_admitted` is variant-only, so it is false for a record root. `is_vec` and `VEC_ID` are false, and the root is `Nominal`, so `contains` reaches the traversal.
  - `record_root` is false because `arguments` is non-empty. That is fine, because the hit comes from `vector()` rather than the `record_root` clause.
  - The traversal pushes the declared field type `Vec<CollectionText>`. `vector()` then holds: the Vec is compiler-owned, and `text_element(CollectionText)` is true (explicit id, no type parameters, one field, and it is a `String`).
  - `contains_funded` takes the same route. The early gates are false, and `has_children` should be true for a record with fields, so it returns `Ok(true)`.
  - The `T` field and the `i64` argument just get pushed and skipped.
- **Source check of the fixture declaration:** this is feasible against `declared_type.rs`.
  - `Vec<CollectionText>` hits `admitted_vec` via `declared_collection::vector`, so neither SPX-T268 nor SPX-T223 fires. The arity matches, and the field type needs no `Bytes` handling.
  - `T` is an in-scope parameter and takes no arguments.
  - `CollectionGeneric<i64>` never appears in source, so no source-level instance admission is needed for it. It is built only in the HIR test.
  - SPX-T281 (`reject_function`) only inspects signatures and bodies. `collection_seed(borrow Vec<i64>) -> i64` mentions only `Vec<i64>`, so it isn't refused.
- **The `collection.seed` return type** is `i64`, which satisfies the "real generic declaration applied to `i64`" claim. The `type_parameters().len() == 1` assert is a real check on the declaration, not a fabricated authority.
- **No weakening:** nothing is skipped, ignored or loosened. The assertions the diff adds are strictly additional, and the old `contains`, `contains_funded` and diagnostic assertions are unchanged.
- **Shadowing:** `let mut function = function(&program, ..)` is legal Rust, because the right-hand side resolves before the binding.

## Uncertain (not visible in the supplied material)

1. **Generic record with an owned `Vec` field under `check`.** Nothing shown rejects it, but there may be declaration-level rules for generic templates holding non-Copy fields (`is_nested_owned_byte_record_template` and similar). This is the biggest unknown. If `check(...).unwrap()` panics, every test using `generic_collection_program` fails at setup.
2. **Fixture collisions or syntax.** I couldn't check whether `iterative_validator_tests::SOURCE` already defines `CollectionText`, `CollectionGeneric` or `collection.*` ids. I also couldn't confirm that `borrow Vec<i64>`, `string` and trailing commas in records match the dialect. The original `collection_program` fixture is the reference for syntax, but it wasn't shown.
3. **Compile-level names in the test module:**
   - whether `IdentityOrigin` is in scope in `collection_tests.rs`;
   - whether `ResolvedType::nominal_id()` exists and returns `Option<&DeclarationId>`;
   - whether `IdentityOrigin` implements `Debug`, which `assert_eq!` needs.
4. **The original carrier's admission.** The new control asserts `owned_collection_record::admitted(collection.carrier)` is true, but the carrier's source wasn't shown. If it is already admitted by the old test's premise, this holds. `contains == false` is then somewhat tautological, since it returns at the admission gate. That is acceptable for a control.
5. **`validate_function` on `token.consume` with a swapped return type.** This depends on that function's params and body types also being non-refused. They should be, since the function exists in the base fixture, but I couldn't confirm it.
6. **The unseen tail of the modified test.** Assertions after `contains(` were cut off. Any that depend on `collection.carrier` specifics, such as spans or type names in the diagnostic bytes, would break. A fixed message string would not.
7. **`paid.rs` internals** (`early::*`, `has_children`) weren't shown in full, so my `contains_funded` equivalence rests on its comment that the gates mirror the owned receiver.

**Recommendation:** when a build is available, run the new tests first. If `generic_collection_program` fails setup, the cause is most likely item 1 or 2, not the test logic.
