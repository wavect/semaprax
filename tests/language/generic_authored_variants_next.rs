//! Authored generic owned-variant source, HIR, and graph regressions.
const PREFIX: &str = r#"
module test.authored_generic_variants;
@id("v.choice") variant Choice<P,T> {
 @id("v.data") Data { @id("v.payload") payload:P, @id("v.marker") marker:T, },
 @id("v.empty") Empty { @id("v.empty.marker") marker:T, },
}
@id("v.relay") fn relay<T>(value:own Choice<Bytes,T>)->Choice<Bytes,T>{value}
@id("v.rebuild") fn rebuild<T>(value:own Choice<Bytes,T>)->Choice<Bytes,T>{
 match own value {
  Choice::Data {payload,marker} => Choice<Bytes,T>::Data {payload:payload,marker:marker},
  Choice::Empty {marker} => Choice<Bytes,T>::Empty {marker:marker},
 }
}
@id("v.observe") fn observe<T>(value:own Choice<Bytes,T>)->T{
 match borrow value { Choice::Data {payload,marker}=>marker, Choice::Empty {marker}=>marker, }
}
@id("v.compose") fn compose<T>(value:own Choice<Bytes,T>)->Choice<Bytes,T>{relay<T>(rebuild<T>(value))}
"#;
fn checked() -> semaprax::hir::ResolvedProgram {
    let mut source = PREFIX.to_owned();
    for ty in ["i64", "i32", "u8", "usize", "char", "f32", "f64", "bool"] {
        source.push_str(&format!("@id(\"v.invoke.{ty}\") fn invoke_{ty}(value:own Choice<Bytes,{ty}>)->{ty}{{observe<{ty}>(compose<{ty}>(value))}}\n"));
    }
    source.push_str("@id(\"v.main\") fn main()->i64{0}");
    let parsed = semaprax::check(&source, "authored-variants.spx").unwrap();
    let canonical = semaprax::format::canonical(&parsed);
    let reparsed = semaprax::check(&canonical, "canonical-authored-variants.spx").unwrap();
    assert_eq!(canonical, semaprax::format::canonical(&reparsed));
    let hir = semaprax::hir::resolve(&reparsed).unwrap();
    semaprax::hir::validate(&hir).unwrap();
    let graph = semaprax::graph::to_json(&reparsed).unwrap();
    semaprax::graph::verify_json(&reparsed, &graph).unwrap();
    hir
}
#[test]
fn authored_variant_all_copy_scalars_have_exact_materialized_ownership() {
    let hir = checked();
    assert_eq!(hir.function_instances.len(), 32);
    for function in &hir.function_instances {
        assert_eq!(function.type_arguments.len(), 1);
        assert_eq!(
            function.function.params[0].ownership,
            semaprax::hir::OwnershipMode::Own
        );
    }
}
#[test]
fn authored_variant_template_pattern_case_and_field_forgery_are_rejected() {
    use semaprax::hir::{DeclarationId, ResolvedExprKind, ResolvedMatchPattern};
    for corrupt_case in [false, true] {
        let mut hir = checked();
        let template = hir
            .function_templates
            .iter_mut()
            .find(|f| f.id.as_str() == "v.rebuild")
            .unwrap();
        // The function body is a block. Reach its actual match and assert the
        // corruption happened; a test that silently skips a block proves nothing.
        let ResolvedExprKind::Block { tail, .. } = &mut template.body.kind else {
            panic!("expected function block")
        };
        let ResolvedExprKind::Match { arms, .. } = &mut tail.kind else {
            panic!("expected owned match")
        };
        let ResolvedMatchPattern::Variant { case, fields, .. } = &mut arms[0].pattern else {
            panic!("expected variant pattern")
        };
        if corrupt_case {
            *case = DeclarationId::new("v.forged.case");
        } else {
            fields[0].field = DeclarationId::new("v.forged.field");
        }
        assert!(semaprax::hir::validate(&hir).is_err());
    }
}
#[test]
fn authored_variant_materialized_binding_type_forgery_is_rejected() {
    let mut hir = checked();
    hir.function_instances[0].function.params[0].ty = semaprax::hir::ResolvedType::Bytes;
    assert!(semaprax::hir::validate(&hir).is_err());
}
#[test]
fn authored_variant_second_owned_case_substitution_stays_closed() {
    let source=format!("{PREFIX}\n@id(\"v.bad\") fn bad(value:own Choice<Bytes,Bytes>)->Choice<Bytes,Bytes>{{relay<Bytes>(value)}}\n@id(\"v.main\") fn main()->i64{{0}}");
    assert!(semaprax::check(&source, "closed-authored-variant.spx").is_err());
}
