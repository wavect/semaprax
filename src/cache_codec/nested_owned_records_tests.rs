use super::static_token;

#[test]
fn nested_cleanup_plan_versions_are_exact_closed_static_tokens() {
    for schema in ["semaprax.cleanup-plan.v7", "semaprax.cleanup-plan.v8"] {
        assert_eq!(static_token(schema).unwrap(), schema);
    }
    assert!(static_token("semaprax.cleanup-plan.v7 ").is_err());
    assert!(static_token("semaprax.cleanup-plan.v8+v7").is_err());
    assert!(static_token("semaprax.cleanup-plan.v9").is_err());
}
