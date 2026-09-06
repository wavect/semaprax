use semaprax::{format, hir, parse, verify};
use std::path::Path;

const PREFIX: &str = r#"module test.generic_owned_result;
@id("result.propagate")
fn propagate<E>(value: own Result<Bytes, E>) -> Result<Bytes, E> {
    let payload = value?;
    Result<Bytes, E>::Ok { value: payload, }
}
@id("result.relay")
fn relay<E>(value: own Result<Bytes, E>) -> Result<Bytes, E> { value }
@id("result.forward")
fn forward<E>(value: own Result<Bytes, E>) -> Result<Bytes, E> { propagate<E>(relay<E>(value)) }
"#;

fn source() -> String {
    let mut source = PREFIX.to_owned();
    for ty in [
        "i64", "i32", "char", "u8", "usize", "f32", "f64", "bool", "Bytes",
    ] {
        source.push_str(&format!("@id(\"result.invoke.{ty}\") fn invoke_{ty}(value: own Result<Bytes, {ty}>) -> Result<Bytes, {ty}> {{ forward<{ty}>(value) }}\n"));
    }
    source.push_str("@id(\"app.main\") fn main() -> i64 { 0 }\n");
    source
}

#[test]
fn generic_owned_result_materializes_exact_relay_and_propagation() {
    let parsed = parse(&source(), Path::new("generic-owned-result.spx")).unwrap();
    let diagnostics = verify::verify(&parsed);
    assert!(
        diagnostics.iter().all(|d| !d.severity.is_error()),
        "{diagnostics:?}"
    );
    let canonical = format::canonical(&parsed);
    let reparsed = parse(&canonical, Path::new("generic-owned-result.spx")).unwrap();
    assert_eq!(canonical, format::canonical(&reparsed));
    let program = hir::resolve(&parsed).unwrap();
    hir::validate(&program).unwrap();
    assert_eq!(program.function_instances.len(), 27);
    for instance in &program.function_instances {
        assert_eq!(
            instance.function.params[0].ownership,
            hir::OwnershipMode::Own
        );
        assert_eq!(
            instance.function.params[0].ty,
            instance.function.return_type
        );
        assert_eq!(
            instance.function.cleanup_plan.schema,
            "semaprax.cleanup-plan.v6"
        );
    }
    let mut hostile = program.clone();
    hostile.function_instances[0].type_arguments = vec![hir::ResolvedType::String];
    assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");
    let mut hostile = program.clone();
    hostile.function_instances[0].function.params[0].ownership = hir::OwnershipMode::Value;
    assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");
}

#[test]
fn generic_owned_result_closed_arguments_and_signature_reject_stably() {
    for (before, after, expected) in [
        ("forward<i64>(value)", "forward<String>(value)", "SPX-T225"),
        (
            "fn propagate<E>(value: own Result<Bytes, E>)",
            "fn propagate<E>(value: borrow Result<Bytes, E>)",
            "SPX-T224",
        ),
        ("let payload = value?;", "let payload = value;", "SPX-T215"),
    ] {
        let text = source().replacen(before, after, 1);
        let parsed = parse(&text, Path::new("generic-owned-result-hostile.spx")).unwrap();
        let diagnostics = verify::verify(&parsed);
        assert!(
            diagnostics.iter().any(|d| d.code == expected),
            "{before} -> {after}: {diagnostics:?}"
        );
    }
}

#[test]
fn generic_copy_success_result_materializes_eight_scalars_and_rejects_bytes() {
    let prefix = PREFIX.replace("Result<Bytes, E>", "Result<E, Bytes>");
    let mut source = prefix.clone();
    for ty in ["i64", "i32", "char", "u8", "usize", "f32", "f64", "bool"] {
        source.push_str(&format!("@id(\"result.invoke.{ty}\") fn invoke_{ty}(value: own Result<{ty}, Bytes>) -> Result<{ty}, Bytes> {{ forward<{ty}>(value) }}\n"));
    }
    source.push_str("@id(\"app.main\") fn main() -> i64 { 0 }\n");
    let parsed = semaprax::check(&source, "copy-success-result.spx").unwrap();
    let canonical = format::canonical(&parsed);
    let reparsed = semaprax::check(&canonical, "copy-success-result.spx").unwrap();
    assert_eq!(canonical, format::canonical(&reparsed));
    let program = hir::resolve(&reparsed).unwrap();
    hir::validate(&program).unwrap();
    assert_eq!(program.function_instances.len(), 24);
    for instance in &program.function_instances {
        assert_eq!(
            instance.function.params[0].ownership,
            hir::OwnershipMode::Own
        );
        assert_eq!(
            instance.function.params[0].ty,
            instance.function.return_type
        );
        assert_eq!(
            instance.function.cleanup_plan.schema,
            "semaprax.cleanup-plan.v6"
        );
    }
    let rejected = format!(
        "{prefix}\n@id(\"result.invalid\") fn invalid(value: own Result<Bytes, Bytes>) -> Result<Bytes, Bytes> {{ forward<Bytes>(value) }}\n@id(\"app.main\") fn main() -> i64 {{ 0 }}\n"
    );
    let parsed = parse(&rejected, Path::new("copy-success-bytes-rejected.spx")).unwrap();
    assert!(verify::verify(&parsed).iter().any(|d| d.code == "SPX-T225"));
    let mut hostile = program.clone();
    hostile.function_instances[0].type_arguments = vec![hir::ResolvedType::Bytes];
    assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");
}
