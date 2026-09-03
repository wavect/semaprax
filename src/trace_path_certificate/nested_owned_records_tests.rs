use super::trace_path_schema_is_admitted;

#[test]
fn trace_path_v1_rejects_every_cleanup_schema_with_conditional_or_nested_ownership() {
    for schema in [
        crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V2,
        crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V3,
        crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V4,
        crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V5,
    ] {
        assert!(trace_path_schema_is_admitted(schema));
    }
    assert!(!trace_path_schema_is_admitted(
        crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V6
    ));
    assert!(!trace_path_schema_is_admitted(
        crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V7
    ));
}
