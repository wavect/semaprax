use super::*;
#[test]
fn repeated_scalar_and_borrowed_field_reads_exhaust_no_owning_budget() {
    let source = include_str!("../../../tests/fixtures/scoped-vec-field-reads.spx");
    let program = hir::resolve(&crate::check(source, "scoped.spx").unwrap()).unwrap();
    let entry = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    let admitted = program
        .functions
        .iter()
        .map(|f| (f.id.as_str(), f))
        .collect();
    let (outcome, _, _, usage) = evaluate_resolved_entry_with_utf8_budget(
        entry,
        &[],
        &admitted,
        &program,
        1_000_000,
        false,
        Utf8MaterializationBudget::Fixed {
            used_materializations: MAX_OWNED_UTF8_LOGICAL_ALLOCATIONS - 1,
            used_bytes: MAX_OWNED_UTF8_LOGICAL_ALLOCATION_BYTES - 3,
        },
    );
    assert!(matches!(outcome, Ok(Value::Int(42))), "{outcome:?}");
    assert_eq!(
        usage,
        (
            MAX_OWNED_UTF8_LOGICAL_ALLOCATIONS,
            MAX_OWNED_UTF8_LOGICAL_ALLOCATION_BYTES
        )
    );
}
