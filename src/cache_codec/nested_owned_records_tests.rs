use super::static_token;

#[test]
fn nested_cleanup_plan_versions_are_exact_closed_static_tokens() {
    for schema in [
        "semaprax.cleanup-plan.v7",
        "semaprax.cleanup-plan.v8",
        "semaprax.cleanup-plan.v9",
    ] {
        assert_eq!(static_token(schema).unwrap(), schema);
    }
    assert!(static_token("semaprax.cleanup-plan.v7 ").is_err());
    assert!(static_token("semaprax.cleanup-plan.v8+v7").is_err());
    assert!(static_token("semaprax.cleanup-plan.v9+v8").is_err());
    assert_eq!(
        static_token("semaprax.cleanup-plan.v10").unwrap(),
        "semaprax.cleanup-plan.v10"
    );
}

#[test]
fn iterator_cleanup_cache_retains_exact_version_and_replays_owned_payloads() {
    for schema in [
        "semaprax.cleanup-plan.v11",
        "semaprax.cleanup-plan.v12",
        "semaprax.cleanup-plan.v13",
    ] {
        let bytes = super::encode(&schema).unwrap();
        assert_eq!(super::decode::<&'static str>(&bytes).unwrap(), schema);
        assert!(static_token(&format!("{schema} ")).is_err());
    }
    assert!(static_token("semaprax.cleanup-plan.v14").is_err());
    let source = crate::check(
        "module iterator.cache; @id(\"main\") fn main()->i64 {let iterator=vec_into_iter<Bytes>(vec_with_capacity<Bytes>(0usize));let step=iter_next<Bytes>(iterator);match own step {IterStep::Done{}=>1,IterStep::Yield{item,rest}=>0,}}",
        "iterator-cache.spx",
    ).unwrap();
    let mut program = crate::hir::resolve(&source).unwrap();
    let function = program
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "main")
        .unwrap();
    assert_eq!(function.cleanup_plan.schema, "semaprax.cleanup-plan.v13");
    let bytes = super::encode(&function.cleanup_plan).unwrap();
    function.cleanup_plan = super::decode(&bytes).unwrap();
    assert_eq!(super::encode(&function.cleanup_plan).unwrap(), bytes);
    crate::hir::validate(&program).unwrap();
}
