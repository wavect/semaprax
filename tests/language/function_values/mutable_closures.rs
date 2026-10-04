//! Reserved transactional mutable syntax stays fail-closed until every runtime
//! implements the same state publication and receiver guard contract.
use semaprax::{ast::Type, hir::ResolvedType};

#[test]
fn mutable_closures_keep_distinct_canonical_syntax_and_types() {
    let source = r#"module test.mutable_closures;
@id("mut.factory") fn factory(state: i64) -> FnMutI64(i64) -> i64 {
    mut fn(value: i64) -> i64 { state + value }
}
@id("mut.main") fn main() -> i64 { 0 }
"#;
    let ast = semaprax::parse(source, "mutable.spx").unwrap();
    assert_eq!(ast.functions[0].return_type, Type::MutFunctionI64);
    let canonical = semaprax::format::canonical(&ast);
    assert!(canonical.contains("mut fn(value: i64) -> i64"));
    assert!(canonical.contains("FnMutI64(i64) -> i64"));
    assert_eq!(
        canonical,
        semaprax::format::canonical(&semaprax::parse(&canonical, "mutable.spx").unwrap())
    );
    assert!(!Type::MutFunctionI64.is_once_function());
    assert!(!ResolvedType::MutFunctionI64.is_once_function());
    assert!(ResolvedType::MutFunctionI64.is_mut_function());
    assert_ne!(
        ResolvedType::MutFunctionI64.identity_key(),
        ResolvedType::Function {
            parameters: vec![ResolvedType::I64],
            result: Box::new(ResolvedType::I64),
        }
        .identity_key()
    );
    let errors = semaprax::check(source, "mutable.spx").unwrap_err();
    assert!(errors.iter().any(|error| error.code == "SPX-T308"), "{errors:?}");
}

#[test]
fn mutable_closures_refuse_inferred_literals_before_runtime_admission() {
    let source = r#"module test.mutable_closures;
@id("mut.main") fn main() -> i64 {
    let state = 1;
    let callback = mut fn(value: i64) -> i64 { state + value };
    callback(2)
}
"#;
    let errors = semaprax::check(source, "mutable.spx").unwrap_err();
    assert!(errors.iter().any(|error| error.code == "SPX-T308"), "{errors:?}");
}

#[test]
fn mutable_closures_reject_every_noncanonical_fixed_signature() {
    for signature in [
        "FnMutI64() -> i64",
        "FnMutI64(bool) -> i64",
        "FnMutI64(i64, i64) -> i64",
        "FnMutI64(i64) -> bool",
    ] {
        let source = format!(
            "module test.mutable_closures; @id(\"mut.bad\") fn bad(value: {signature}) -> i64 {{ 0 }}"
        );
        let error = semaprax::parse(&source, "mutable.spx").unwrap_err();
        assert_eq!(error.code, "SPX-T308", "{signature}: {error:?}");
    }
}

#[test]
fn mutable_closures_reject_forged_retained_signature_independently() {
    let source =
        "module test.mutable_closures; @id(\"mut.bad\") fn bad(value: i64) -> i64 { value } @id(\"mut.main\") fn main() -> i64 { 0 }";
    let ast = semaprax::check(source, "mutable.spx").unwrap();
    let mut resolved = semaprax::hir::resolve(&ast).unwrap();
    resolved.functions[0].params[0].ty = ResolvedType::MutFunctionI64;
    let error = semaprax::hir::validate(&resolved).unwrap_err();
    assert!(error.message.contains("mutable"), "{error:?}");
}
