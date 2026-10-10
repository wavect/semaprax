use super::*;

#[test]
fn repeated_projected_views_do_not_charge_a_string_materialization() {
    let source = include_str!("../../../tests/fixtures/projected-string-views.spx");
    let program = hir::resolve(&crate::check(source, "views.spx").unwrap()).unwrap();
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
    // Exactly the two authored String literals fit. Any semantic copy on a
    // projected or forwarded read must exceed the unchanged fixed cap.
    let (outcome, _, _, usage) = evaluate_resolved_entry_with_utf8_budget(
        entry,
        &[],
        &admitted,
        &program,
        100_000,
        false,
        Utf8MaterializationBudget::Fixed {
            used_materializations: MAX_OWNED_UTF8_LOGICAL_ALLOCATIONS - 2,
            used_bytes: MAX_OWNED_UTF8_LOGICAL_ALLOCATION_BYTES - 8,
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

#[test]
fn projected_byte_range_failures_keep_closed_compiler_statuses() {
    for code in [
        crate::byte_ops::RANGE_START_AFTER_END_CODE,
        crate::byte_ops::RANGE_END_OUT_OF_BOUNDS_CODE,
    ] {
        let status = normalize_byte_range(code);
        let document: serde_json::Value = serde_json::from_str(&status.to_json()).unwrap();
        verify_status(&document).unwrap();
        for (key, value) in [
            ("code", serde_json::json!(0)),
            ("code", serde_json::json!(3)),
            ("code", serde_json::json!(4294967297u64)),
            ("class", serde_json::json!("arithmetic")),
            ("retryable", serde_json::json!(true)),
            ("domain_id", serde_json::json!("foreign.byte-range")),
        ] {
            let mut hostile = document.clone();
            hostile[key] = value;
            assert_eq!(
                verify_status(&hostile).unwrap_err().code,
                "SPX-F106",
                "{hostile}"
            );
        }
    }
}
