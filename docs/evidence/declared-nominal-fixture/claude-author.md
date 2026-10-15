# claude author (declared-nominal-fixture)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

**Assessment (from the pasted source only, nothing compiled or run)**

- **Why the fixture fails.** `CollectionCarrier { rows: Vec<CollectionText> }` is a non-generic explicit-id record. Its only leaf is a `Vec` whose element passes `owned_leaf_collection::runtime_element`, so `owned_collection_record::admitted` is true. `contains` then returns false at its second early exit, before the scan runs. The fixture is a correct negative, and the positive expectation was the mistake.
- **Generic carrier.** `CollectionGeneric<T> { rows: Vec<CollectionText>, tag: T }` applied to `i64` should work.
  - **Early exits.** `owned_collection_record::admitted` returns false at its first guard, because `arguments.is_empty()` fails. `is_vec` and the `VEC_ID` check are false for the root.
  - **Scan.** `record_root` is false because of the non-empty arguments, but the scan still walks `record_fields`. It reaches `rows`, and `declared_collection::vector` is true there because `text_element(CollectionText)` holds. `tag: T` is a parameter type, skipped by the `Nominal | Function` filter.
  - **Source side.** `source_verify` only rejects functions whose signature or body mention the type. A declaration-only generic record is never mentioned, so it should check, as the original carrier does. This is the same assumption as for the original fixture.
- **Invariant carrier (not recommended).** An invariant makes `{id}#invariant` exist, so `admitted` is false. But the synthesized invariant function would take the carrier as a parameter, and `reject_function` would then flag it as collection-bearing and reject the source. I expect it to fail at `check`, which would break the positive fixture.
- **No forged metadata.** The only hand-built value is the `ResolvedType::Nominal` application, with a genuine declaration id and one argument matching one declared parameter. Take the `i64` type from `collection.seed`'s return type so no variant name is guessed.

**Suggested patch (`collection_tests.rs`)**

```rust
const COLLECTION_BASE: &str = r#"@id("collection.seed") fn collection_seed(value: borrow Vec<i64>) -> i64 { 0 }
@id("collection.text") record CollectionText { @id("collection.text.value") value: string, }
@id("collection.carrier") record CollectionCarrier { @id("collection.carrier.rows") rows: Vec<CollectionText>, }
"#;
// Declaration-only and generic, so owned_collection_record::admitted refuses it
// (applied arguments) while the logical scan still reaches the Vec field.
const COLLECTION_GENERIC: &str = r#"@id("collection.generic") record CollectionGeneric<T> { @id("collection.generic.rows") rows: Vec<CollectionText>, @id("collection.generic.tag") tag: T, }
"#;
fn checked_program(additions: &str, file: &str) -> ResolvedProgram {
    let source = format!(
        "{}\n{}",
        crate::hir::iterative_validator_tests::SOURCE,
        additions
    );
    crate::hir::resolve(&crate::check(&source, file).unwrap()).unwrap()
}
fn collection_program() -> ResolvedProgram {
    checked_program(COLLECTION_BASE, "collection-positive.spx")
}
fn generic_collection_program() -> ResolvedProgram {
    checked_program(
        &format!("{COLLECTION_BASE}{COLLECTION_GENERIC}"),
        "collection-generic.spx",
    )
}
```

Negative case, with the original fixture and token unchanged:

```rust
#[test]
fn owned_collection_record_fixture_is_classified_runtime_admitted_not_refused() {
    let program = collection_program();
    let token = ResolvedType::Nominal {
        declaration: DeclarationId::new("collection.carrier"),
        arguments: Vec::new(),
    };
    assert!(!crate::hir::declared_collection::contains(&program.declarations, &token));
    assert!(!crate::hir::declared_collection::contains_funded(&program.declarations, &token).unwrap());
}
```

Positive case. The only change from the original test is the program and token; every assertion after that is untouched:

```rust
#[test]
fn funded_scan_keeps_positive_transitive_refusal_and_full_diagnostic_bytes() {
    let program = generic_collection_program();
    let int = function(&program, "collection.seed").return_type.clone();
    let token = ResolvedType::Nominal {
        declaration: DeclarationId::new("collection.generic"),
        arguments: vec![int],
    };
    // ...remainder identical to the current test body...
}
```

**Uncertainties**
1. **Generic syntax.** `record Name<T>` is a guess. Check how `owned.box` or another generic in `SOURCE` is spelled.
2. **`check` on a generic record.** I can't confirm `check` accepts a generic record with a `Vec` field and an explicit `@id` on every field.
3. **`collection_outcome::runtime_admitted`.** Not shown. I assume it is false for a record application.
4. **`paid::contains_funded` and `paid::validate_function`.** Not shown. They must treat applied arguments like `contains` does, and I can't verify that.
5. **Parameter substitution.** I assume `record_fields` returns unsubstituted field types (`T`), so the scan skips `tag`.

**Fallback.** If `check` rejects the generic declaration (2), fall back to a non-generic positive. For that, I need `owned_leaf_collection::runtime_element` and `copy_record_collection::admitted`, to find a field shape that `declared_collection::vector` accepts but `owned_collection_record::vector` rejects.
