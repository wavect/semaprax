//! Generic compiler-owned carriers preserve exact substitutions and ownership.
fn check(source: &str) -> semaprax::hir::ResolvedProgram {
    let parsed = semaprax::check(source, "generic collections").unwrap();
    let canonical = semaprax::format::canonical(&parsed);
    let reparsed = semaprax::check(&canonical, "canonical collections").unwrap();
    assert_eq!(canonical, semaprax::format::canonical(&reparsed));
    let hir = semaprax::hir::resolve(&reparsed).unwrap();
    semaprax::hir::validate(&hir).unwrap();
    let graph = semaprax::graph::to_json(&reparsed).unwrap();
    semaprax::graph::verify_json(&reparsed, &graph).unwrap();
    hir
}
#[test]
fn generic_box_and_vec_all_copy_scalars_materialize_exactly() {
    let mut source = String::from(
        r#"module test.collections;
@id("c.make") fn make<T>(value:T) -> Box<T> { box_new<T>(value) }
@id("c.relay") fn relay<T>(value:own Box<T>) -> Box<T> { value }
@id("c.take") fn take<T>(value:own Box<T>) -> T { box_into_inner<T>(value) }
@id("c.vec") fn vector<T>(value:T) -> Vec<T> { vec_push<T>(vec_with_capacity<T>(1usize), value) }
@id("c.read") fn read<T>(value:own Vec<T>) -> T { vec_get<T>(value, 0usize) }
@id("c.local") fn local<T>(value:T) -> T { let boxed = box_new<T>(value); let copied = box_get<T>(boxed); let values = vec_push<T>(vec_with_capacity<T>(1usize), copied); vec_get<T>(values, 0usize) }
"#,
    );
    for ty in ["i64", "i32", "u8", "usize", "char", "f32", "f64", "bool"] {
        source.push_str(&format!("@id(\"c.invoke.{ty}\") fn invoke_{ty}(value:{ty}) -> {ty} {{ let first = take<{ty}>(relay<{ty}>(make<{ty}>(value))); let second = read<{ty}>(vector<{ty}>(first)); local<{ty}>(second) }}\n"));
    }
    source.push_str("@id(\"c.main\") fn main()->i64 {0}\n");
    let hir = check(&source);
    assert_eq!(hir.function_instances.len(), 48);
    let mut forged = hir.clone();
    forged.function_instances[0].type_arguments[0] = semaprax::hir::ResolvedType::Bytes;
    assert!(semaprax::hir::validate(&forged).is_err());
}
#[test]
fn generic_collection_owned_elements_and_owner_reuse_are_rejected() {
    let source = "module test.bad; @id(\"c.make\") fn make<T>(value:T)->Box<T>{box_new<T>(value)} @id(\"c.run\") fn run(value:own Bytes)->i64{let boxed=make<Bytes>(value); 0} @id(\"c.main\") fn main()->i64{0}";
    let errors = semaprax::check(source, "owned element").unwrap_err();
    assert!(errors.iter().any(|d| d.code == "SPX-T225"), "{errors:?}");
    assert!(!errors.iter().any(|d| d.code == "SPX-T105"));
    let source = "module test.bad; @id(\"c.take\") fn take<T>(value:own Box<T>)->T{box_into_inner<T>(value)} @id(\"c.run\") fn run()->i64{let boxed=box_new<i64>(7);let first=take<i64>(boxed);take<i64>(boxed)} @id(\"c.main\") fn main()->i64{0}";
    let errors = semaprax::check(source, "owner reuse").unwrap_err();
    assert!(
        errors.iter().any(|d| d.code.starts_with("SPX-O")),
        "{errors:?}"
    );
}
