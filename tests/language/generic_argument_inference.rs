//! Bounded omitted generic vectors retain explicit checked HIR meaning.
fn checked(source: &str) -> semaprax::hir::ResolvedProgram {
    let parsed = semaprax::check(source, "generic-inference.spx").unwrap();
    let canonical = semaprax::format::canonical(&parsed);
    let reparsed = semaprax::check(&canonical, "canonical-inference.spx").unwrap();
    assert_eq!(canonical, semaprax::format::canonical(&reparsed));
    let hir = semaprax::hir::resolve(&reparsed).unwrap();
    semaprax::hir::validate(&hir).unwrap();
    let graph = semaprax::graph::to_json(&reparsed).unwrap();
    semaprax::graph::verify_json(&reparsed, &graph).unwrap();
    hir
}
const SCALAR: &str = r#"
module test.generic_inference;
@id("infer.identity") fn identity<T>(value:T)->T{value}
@id("infer.same") fn same<T>(a:T,b:T)->T{a}
"#;
#[test]
fn argument_inference_materializes_exact_scalar_vectors() {
    let source = format!("{SCALAR} @id(\"infer.main\") fn main()->i64{{let flag=identity(true); if flag {{same(41,42)}} else {{0}}}}");
    let inferred = checked(&source);
    let explicit = checked(
        &source
            .replace("identity(true)", "identity<bool>(true)")
            .replace("same(41,42)", "same<i64>(41,42)"),
    );
    assert_eq!(inferred.function_instances, explicit.function_instances);
    assert_eq!(inferred.functions.len(), explicit.functions.len());
}
#[test]
fn argument_inference_rejects_conflicting_and_unsupported_evidence() {
    for call in [
        "same(1,true)",
        "identity(1+2)",
        "identity(identity<i64>(1))",
    ] {
        let source = format!("{SCALAR} @id(\"infer.main\") fn main()->i64{{{call}}}");
        let errors = semaprax::check(&source, "bad-inference.spx").unwrap_err();
        assert!(
            errors.iter().any(|error| error.code == "SPX-T225"),
            "{call}: {errors:?}"
        );
    }
}
#[test]
fn argument_inference_all_scalars_preserves_owned_variant_substitution() {
    let mut source = String::from(
        r#"
module test.inferred_owned;
@id("infer.choice") variant Choice<P,T> {
 @id("infer.data") Data { @id("infer.payload") payload:P, @id("infer.marker") marker:T, },
 @id("infer.empty") Empty { @id("infer.empty.marker") marker:T, },
}
@id("infer.relay") fn relay<T>(value:own Choice<Bytes,T>)->Choice<Bytes,T>{value}
"#,
    );
    for ty in ["i64", "i32", "u8", "usize", "char", "f32", "f64", "bool"] {
        source.push_str(&format!("@id(\"infer.invoke.{ty}\") fn invoke_{ty}(value:own Choice<Bytes,{ty}>)->Choice<Bytes,{ty}>{{relay(value)}}\n"));
    }
    source.push_str("@id(\"infer.main\") fn main()->i64{0}");
    let hir = checked(&source);
    assert_eq!(hir.function_instances.len(), 8);
    for instance in &hir.function_instances {
        assert_eq!(instance.type_arguments.len(), 1);
        assert_eq!(
            instance.function.params[0].ownership,
            semaprax::hir::OwnershipMode::Own
        );
    }
}

#[test]
fn argument_inference_cannot_use_result_context_or_erase_partial_vectors() {
    let source = r#"
module test.inference_context;
@id("infer.unused") fn unused<T>(value:i64)->i64{value}
@id("infer.main") fn main()->i64{unused(1)}
"#;
    let errors = semaprax::check(source, "no-argument-binding.spx").unwrap_err();
    assert!(
        errors.iter().any(|error| error.code == "SPX-T225"),
        "{errors:?}"
    );
    let source = format!("{SCALAR} @id(\"infer.main\") fn main()->i64{{identity<i64,bool>(1)}}");
    let errors = semaprax::check(&source, "surplus-vector.spx").unwrap_err();
    assert!(
        errors.iter().any(|error| error.code == "SPX-T225"),
        "{errors:?}"
    );
}

#[test]
fn argument_inference_does_not_bypass_concrete_instance_replay() {
    let source = format!("{SCALAR} @id(\"infer.main\") fn main()->i64{{identity(1)}}");
    let mut hir = checked(&source);
    assert_eq!(hir.function_instances.len(), 1);
    hir.function_instances[0].type_arguments[0] = semaprax::hir::ResolvedType::Bool;
    assert!(semaprax::hir::validate(&hir).is_err());
}

#[test]
fn argument_inference_keeps_owned_move_checks() {
    let source = r#"
module test.inference_moves;
@id("infer.pair") record Pair<P,T> {
 @id("infer.payload") payload:P,
 @id("infer.marker") marker:T,
}
@id("infer.relay") fn relay<T>(value:own Pair<Bytes,T>)->Pair<Bytes,T>{value}
@id("infer.bad") fn bad(value:own Pair<Bytes,bool>)->Pair<Bytes,bool>{
 let first=relay(value);
 relay(value)
}
@id("infer.main") fn main()->i64{0}
"#;
    let errors = semaprax::check(source, "inferred-double-move.spx").unwrap_err();
    assert!(
        errors.iter().any(|error| error.code == "SPX-O101"),
        "{errors:?}"
    );
}

#[test]
fn argument_inference_all_scalars_preserves_box_and_vec_carriers() {
    let mut source = String::from(
        r#"
module test.inferred_collections;
@id("infer.box.prelude") fn box_prelude<T>(value:T)->Box<T>{box_new<T>(value)}
@id("infer.vec.prelude") fn vec_prelude<T>(value:T)->Vec<T>{vec_push<T>(vec_with_capacity<T>(1usize),value)}
@id("infer.box") fn box_relay<T>(value:own Box<T>)->Box<T>{value}
@id("infer.vec") fn vec_relay<T>(value:own Vec<T>)->Vec<T>{value}
"#,
    );
    for ty in ["i64", "i32", "u8", "usize", "char", "f32", "f64", "bool"] {
        source.push_str(&format!("@id(\"infer.box.{ty}\") fn box_{ty}(value:own Box<{ty}>)->Box<{ty}>{{box_relay(value)}}\n"));
        source.push_str(&format!("@id(\"infer.vec.{ty}\") fn vec_{ty}(value:own Vec<{ty}>)->Vec<{ty}>{{vec_relay(value)}}\n"));
    }
    source.push_str("@id(\"infer.main\") fn main()->i64{0}");
    let hir = checked(&source);
    assert_eq!(hir.function_instances.len(), 16);
}
