use super::static_token;

#[test]
fn cleanup_plan_v7_is_an_exact_closed_static_token() {
    assert_eq!(
        static_token("semaprax.cleanup-plan.v7").unwrap(),
        "semaprax.cleanup-plan.v7"
    );
    assert!(static_token("semaprax.cleanup-plan.v7 ").is_err());
    assert!(static_token("semaprax.cleanup-plan.v8").is_err());
}
