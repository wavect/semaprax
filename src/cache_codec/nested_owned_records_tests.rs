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
        "semaprax.cleanup-plan.v14",
        "semaprax.cleanup-plan.v15",
        "semaprax.cleanup-plan.v16",
        "semaprax.cleanup-plan.v17",
    ] {
        let bytes = super::encode(&schema).unwrap();
        assert_eq!(super::decode::<&'static str>(&bytes).unwrap(), schema);
        assert!(static_token(&format!("{schema} ")).is_err());
        let unknown = super::encode(&format!("{schema}+future")).unwrap();
        assert!(super::decode::<&'static str>(&unknown).is_err());
    }
    assert!(static_token("semaprax.cleanup-plan.v18").is_err());
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

    let record_source = crate::check(
        r#"module record.iterator.cache;
@id("cache.item") record Item {@id("cache.item.left") left:Bytes,@id("cache.item.right") right:Bytes,@id("cache.item.marker") marker:i64,}
@id("cache.main") fn main()->i64 {
 let values=vec_push<Item>(vec_with_capacity<Item>(1usize),Item{left:bytes_zeroed(1usize),right:bytes_zeroed(2usize),marker:7});
 match own iter_next<Item>(vec_into_iter<Item>(values)){IterStep::Done{}=>0,IterStep::Yield{item,rest}=>7,}
}"#,
        "record-iterator-cache.spx",
    )
    .unwrap();
    let mut record_program = crate::hir::resolve(&record_source).unwrap();
    let record_function = record_program
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "cache.main")
        .unwrap();
    // Pure traversal has no native owner admission and retains the v13 profile.
    assert_eq!(
        record_function.cleanup_plan.schema,
        "semaprax.cleanup-plan.v13"
    );
    let record_bytes = super::encode(&record_function.cleanup_plan).unwrap();
    record_function.cleanup_plan = super::decode(&record_bytes).unwrap();
    assert_eq!(
        super::encode(&record_function.cleanup_plan).unwrap(),
        record_bytes
    );
    crate::hir::validate(&record_program).unwrap();

    let string_source = crate::check(
        "module string.cache; @id(\"cache.string.main\") fn main()->i64 {let mut text=\"old\";text=\"new\";string_len(text)}",
        "string-replacement-cache.spx",
    )
    .unwrap();
    let mut string_program = crate::hir::resolve(&string_source).unwrap();
    let string_function = &mut string_program.functions[0];
    assert_eq!(
        string_function.cleanup_plan.schema,
        "semaprax.cleanup-plan.v16"
    );
    let string_bytes = super::encode(&string_function.cleanup_plan).unwrap();
    string_function.cleanup_plan = super::decode(&string_bytes).unwrap();
    assert_eq!(
        super::encode(&string_function.cleanup_plan).unwrap(),
        string_bytes
    );
    crate::hir::validate(&string_program).unwrap();

    let byte_source = crate::check(
        "module byte.cache; @id(\"cache.byte.main\") fn main()->i64 {let mut data=bytes_zeroed(1usize);data=bytes_set(data,0usize,1u8);if byte_len(bytes_as_slice(data))==1usize{1}else{0}}",
        "byte-renewal-cache.spx",
    )
    .unwrap();
    let mut byte_program = crate::hir::resolve(&byte_source).unwrap();
    let byte_function = &mut byte_program.functions[0];
    assert_eq!(
        byte_function.cleanup_plan.schema,
        "semaprax.cleanup-plan.v17"
    );
    let byte_plan = super::encode(&byte_function.cleanup_plan).unwrap();
    byte_function.cleanup_plan = super::decode(&byte_plan).unwrap();
    assert_eq!(
        super::encode(&byte_function.cleanup_plan).unwrap(),
        byte_plan
    );
    crate::hir::validate(&byte_program).unwrap();
}
