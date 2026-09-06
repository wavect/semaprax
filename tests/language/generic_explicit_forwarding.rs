use semaprax::{format, hir, parse, verify};
use std::path::Path;

fn resolve(source: &str) -> hir::ResolvedProgram {
    let parsed = parse(source, Path::new("explicit-forwarding.spx")).unwrap();
    let diagnostics = verify::verify(&parsed);
    assert!(
        diagnostics.iter().all(|d| !d.severity.is_error()),
        "{diagnostics:?}"
    );
    let canonical = format::canonical(&parsed);
    let reparsed = parse(&canonical, Path::new("explicit-forwarding.spx")).unwrap();
    assert_eq!(canonical, format::canonical(&reparsed));
    let resolved = hir::resolve(&reparsed).unwrap();
    hir::validate(&resolved).unwrap();
    resolved
}

const SCALAR: &str = r#"
module test.explicit_forwarding;
@id("map.first") fn first<A, B>(left: A, right: B) -> A { left }
@id("map.permute") fn permute<A, B>(left: A, right: B) -> B { first<B, A>(right, left) }
@id("map.repeat") fn repeat<A>(value: A) -> A { first<A, A>(value, value) }
@id("map.concrete") fn concrete<A>(value: A) -> i64 { first<i64, A>(7, value) }
@id("map.transitive") fn transitive<A, B>(left: A, right: B) -> A { permute<B, A>(right, left) }
@id("map.run.permute") fn run_permute() -> bool { permute<i64, bool>(3, true) }
@id("map.run.repeat") fn run_repeat() -> bool { repeat<bool>(true) }
@id("map.run.concrete") fn run_concrete() -> i64 { concrete<bool>(false) }
@id("map.run.transitive") fn run_transitive() -> i64 { transitive<i64, bool>(3, true) }
@id("app.main") fn main() -> i64 { 0 }
"#;

#[test]
fn explicit_forwarding_permutation_repetition_concrete_and_transitive_are_exact() {
    let resolved = resolve(SCALAR);
    let first = resolved
        .function_instances
        .iter()
        .filter(|instance| instance.template.as_str() == "map.first")
        .map(|instance| instance.type_arguments.clone())
        .collect::<Vec<_>>();
    assert_eq!(first.len(), 3);
    assert!(first.contains(&vec![hir::ResolvedType::Bool, hir::ResolvedType::I64]));
    assert!(first.contains(&vec![hir::ResolvedType::Bool, hir::ResolvedType::Bool]));
    assert!(first.contains(&vec![hir::ResolvedType::I64, hir::ResolvedType::Bool]));
    for argument in [
        hir::ResolvedType::TypeParameter {
            owner: hir::DeclarationId::new("foreign.owner"),
            index: 0,
        },
        hir::ResolvedType::TypeParameter {
            owner: hir::DeclarationId::new("map.permute"),
            index: 2,
        },
    ] {
        let mut hostile = resolved.clone();
        let template = hostile
            .function_templates
            .iter_mut()
            .find(|t| t.id.as_str() == "map.permute")
            .unwrap();
        let hir::ResolvedExprKind::Block { tail, .. } = &mut template.body.kind else {
            panic!("expected template block")
        };
        let hir::ResolvedExprKind::Call {
            callee,
            type_arguments,
            instance,
            ..
        } = &mut tail.kind
        else {
            panic!("expected mapped call")
        };
        type_arguments[0] = argument;
        *instance = Some(hir::FunctionInstanceId::derive(callee, type_arguments));
        assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");
    }
    let mut hostile = resolved.clone();
    hostile.function_instances[0].function.return_type = hir::ResolvedType::Bytes;
    assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");
}

#[test]
fn explicit_forwarding_preserves_complete_owned_signature() {
    let resolved = resolve(
        r#"
module test.explicit_owned_forwarding;
@id("map.pair") record Pair<A, B> { @id("map.pair.left") left: A, @id("map.pair.right") right: B, }
@id("map.owner") fn owner<A, B>(value: own Pair<Bytes, A>, marker: B) -> Pair<Bytes, A> { value }
@id("map.outer") fn outer<A, B>(value: own Pair<Bytes, B>, marker: A) -> Pair<Bytes, B> { owner<B, A>(value, marker) }
@id("map.entry") fn entry(value: own Pair<Bytes, bool>) -> Pair<Bytes, bool> { outer<i64, bool>(value, 4) }
@id("app.main") fn main() -> i64 { 0 }
"#,
    );
    assert_eq!(resolved.function_instances.len(), 2);
    for instance in &resolved.function_instances {
        assert_eq!(
            instance.function.params[0].ownership,
            hir::OwnershipMode::Own
        );
        assert_eq!(
            instance.function.params[0].ty,
            instance.function.return_type
        );
    }
    let mut hostile = resolved.clone();
    hostile.function_instances[0].function.params[0].ownership = hir::OwnershipMode::Value;
    assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");
}

#[test]
fn explicit_forwarding_rejects_invalid_mapping_and_unused_return_mismatch() {
    for body in [
        "first<B, A>(left, right)",
        "first<A>(left, right)",
        "first<Missing, A>(right, left)",
        "first<Bytes, A>(right, left)",
    ] {
        let source = SCALAR.replace("first<B, A>(right, left)", body);
        let parsed = parse(&source, Path::new("hostile-forwarding.spx")).unwrap();
        assert!(
            verify::verify(&parsed)
                .iter()
                .any(|d| d.severity.is_error()),
            "admitted {body}"
        );
    }
    // No call reaches this template: proof must still check all substitutions.
    let source = r#"
module test.unused_wrong_forwarding;
@id("map.first") fn first<A, B>(left: A, right: B) -> A { left }
@id("map.bad") fn bad<A, B>(left: A, right: B) -> A { first<B, A>(right, left) }
@id("app.main") fn main() -> i64 { 0 }
"#;
    let parsed = parse(source, Path::new("unused-forwarding.spx")).unwrap();
    assert!(verify::verify(&parsed)
        .iter()
        .any(|d| d.severity.is_error()));
}

#[test]
fn explicit_forwarding_nonidentity_cycles_remain_rejected() {
    for body in ["second<B, A>(right, left)", "first<B, A>(right, left)"] {
        let source = format!(
            r#"
module test.forwarding_cycle;
@id("map.first") fn first<A, B>(left: A, right: B) -> i64 {{ {body} }}
@id("map.second") fn second<A, B>(left: A, right: B) -> i64 {{ first<B, A>(right, left) }}
@id("app.main") fn main() -> i64 {{ 0 }}
"#
        );
        let parsed = parse(&source, Path::new("cycle-forwarding.spx")).unwrap();
        assert!(verify::verify(&parsed).iter().any(|d| d.code == "SPX-T226"));
    }
}

fn bounded_closure(outer_count: usize) -> String {
    let mut source = String::from(
        "module test.forwarding_bound;\n@id(\"map.inner\") fn inner<T>(value: T) -> T { value }\n",
    );
    for index in 0..outer_count {
        source.push_str(&format!("@id(\"map.outer.{index}\") fn outer_{index}<T>(value: T) -> i64 {{ inner<i64>(0) }}\n@id(\"map.entry.{index}\") fn entry_{index}() -> i64 {{ outer_{index}<bool>(true) }}\n"));
    }
    source.push_str("@id(\"app.main\") fn main() -> i64 { 0 }\n");
    source
}

#[test]
fn explicit_forwarding_retains_256_instance_closure_limit() {
    assert_eq!(resolve(&bounded_closure(255)).function_instances.len(), 256);
    let parsed = parse(&bounded_closure(256), Path::new("forwarding-overflow.spx")).unwrap();
    let diagnostics = hir::resolve(&parsed).unwrap_err();
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code == "SPX-H006" && d.message.contains("256")),
        "{diagnostics:?}"
    );
}
