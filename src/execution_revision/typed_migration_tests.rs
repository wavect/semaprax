use super::*;
use crate::interpreter::retained_call::{RetainedField, RetainedRecord};

const SOURCE: &str = r#"module migration.copy;
@id("old.State")
record OldState { @id("old.count") count: i64, }
@id("new.State")
record NewState { @id("new.count") count: i64, @id("new.ready") ready: bool, }
@id("state.migrate")
fn migrate(old: OldState) -> NewState {
    NewState { count: old.count + 10, ready: true }
}
@id("state.wrong")
fn wrong(old: OldState) -> i64 { old.count }
@id("migration.main")
fn main() -> i64 { 0 }
"#;
fn state(value: RetainedValue) -> RetainedValue {
    RetainedValue::Record(RetainedRecord {
        record: DeclarationId::new("old.State"),
        fields: vec![RetainedField {
            field: DeclarationId::new("old.count"),
            value,
        }],
    })
}
#[test]
fn migration_replays_exact_old_and_new_nominal_state_without_effects() {
    let program = hir::resolve(&crate::check(SOURCE, "migration.spx").unwrap()).unwrap();
    let old = DeclarationId::new("old.State");
    let new = DeclarationId::new("new.State");
    let result = evaluate_migration(
        &program,
        "state.migrate",
        &old,
        &new,
        &state(RetainedValue::I64(32)),
        10_000,
    )
    .unwrap();
    assert_eq!(
        result,
        RetainedValue::Record(RetainedRecord {
            record: new.clone(),
            fields: vec![
                RetainedField {
                    field: DeclarationId::new("new.count"),
                    value: RetainedValue::I64(42)
                },
                RetainedField {
                    field: DeclarationId::new("new.ready"),
                    value: RetainedValue::Bool(true)
                },
            ],
        })
    );
    assert!(evaluate_migration(
        &program,
        "state.wrong",
        &old,
        &new,
        &state(RetainedValue::I64(32)),
        10_000
    )
    .is_err());
    assert!(evaluate_migration(
        &program,
        "state.migrate",
        &old,
        &new,
        &state(RetainedValue::Bool(true)),
        10_000
    )
    .is_err());
    assert!(evaluate_migration(
        &program,
        "state.migrate",
        &new,
        &new,
        &state(RetainedValue::I64(32)),
        10_000
    )
    .is_err());
    assert!(evaluate_migration(
        &program,
        "state.migrate",
        &old,
        &new,
        &state(RetainedValue::I64(32)),
        1
    )
    .is_err());
    let drifted = SOURCE
        .replace("count: i64, }", "count: bool, }")
        .replace("old.count + 10", "10")
        .replace("{ old.count }", "{ 0 }");
    let drifted = hir::resolve(&crate::check(&drifted, "migration.spx").unwrap()).unwrap();
    assert_ne!(
        flat_state(&program, &old).unwrap(),
        flat_state(&drifted, &old).unwrap()
    );
}

#[test]
fn migration_rejects_borrowed_owned_old_state_parameter() {
    let borrowed = SOURCE
        .replace(
            "record OldState { @id(\"old.count\") count: i64, }",
            "record OldState { @id(\"old.count\") count: i64, @id(\"old.payload\") payload: Bytes, }",
        )
        .replace(
            "fn migrate(old: OldState) -> NewState",
            "fn migrate(old: borrow OldState) -> NewState",
        )
        .replace(
            "fn wrong(old: OldState) -> i64",
            "fn wrong(old: own OldState) -> i64",
        );
    let program = hir::resolve(&crate::check(&borrowed, "migration.spx").unwrap()).unwrap();
    let errors = prepare_migration_call(
        &program,
        "state.migrate",
        &DeclarationId::new("old.State"),
        &DeclarationId::new("new.State"),
    )
    .unwrap_err();
    assert!(
        errors.iter().any(|error| {
            error.code == "SPX-G583"
                && error.message
                    == "ExecutionRevision association rejected: migration.pure_signature"
        }),
        "{errors:?}"
    );
}
