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
    for call in ["same(1,true)", "identity({let x=1;x})"] {
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

#[test]
fn argument_inference_v2_reads_expression_types_without_evaluating_them() {
    let prefix = format!("{SCALAR} @id(\"infer.declared\") fn declared(value:i64)->i64{{value}} ");
    for (expression, ty, result) in [
        ("1+2", "i64", "i64"),
        ("{1}", "i64", "i64"),
        ("-(1+2)", "i64", "i64"),
        ("!false", "bool", "bool"),
        ("1<2", "bool", "bool"),
        ("if true {declared(7)} else {9}", "i64", "i64"),
        ("identity<i64>(1)", "i64", "i64"),
        ("identity(1)", "i64", "i64"),
        ("declared(identity<i64>(7))", "i64", "i64"),
    ] {
        let inferred=format!("{prefix} @id(\"infer.invoke\") fn invoke()->{result}{{identity({expression})}} @id(\"infer.main\") fn main()->i64{{0}}");
        let explicit = inferred.replace(
            &format!("{{identity({expression})}}"),
            &format!("{{identity<{ty}>({expression})}}"),
        );
        let inferred = checked(&inferred);
        let explicit = checked(&explicit);
        assert_eq!(
            inferred.function_instances, explicit.function_instances,
            "{expression}"
        );
    }
}

#[test]
fn argument_inference_v2_materializes_complete_parameter_vectors_in_declared_order() {
    let source = r#"
module test.inference_order;
@id("infer.ordered") fn ordered<T,U>(left:T,right:U)->T{left}
@id("infer.main") fn main()->i64 {
    if ordered(true,7) { ordered(41,false) } else { 0 }
}
"#;
    let inferred = checked(source);
    let explicit = checked(
        &source
            .replace("ordered(true,7)", "ordered<bool,i64>(true,7)")
            .replace("ordered(41,false)", "ordered<i64,bool>(41,false)"),
    );
    assert_eq!(inferred.function_instances, explicit.function_instances);
    let vectors: Vec<_> = inferred
        .function_instances
        .iter()
        .filter(|instance| instance.template.as_str() == "infer.ordered")
        .map(|instance| instance.type_arguments.clone())
        .collect();
    assert_eq!(vectors.len(), 2);
    assert!(vectors.contains(&vec![
        semaprax::hir::ResolvedType::Bool,
        semaprax::hir::ResolvedType::I64
    ]));
    assert!(vectors.contains(&vec![
        semaprax::hir::ResolvedType::I64,
        semaprax::hir::ResolvedType::Bool
    ]));
    for instance in &inferred.function_instances {
        assert_eq!(
            instance.id,
            semaprax::hir::FunctionInstanceId::derive(&instance.template, &instance.type_arguments)
        );
    }
}

#[test]
fn argument_inference_keeps_missing_and_partial_evidence_closed() {
    for body in [
        "@id(\"infer.partial\") fn partial<T,U>(left:T,right:U)->T{left} @id(\"infer.main\") fn main()->i64{partial<i64>(1,true)}",
        "@id(\"infer.missing\") fn missing<T,U>(left:T)->T{left} @id(\"infer.main\") fn main()->i64{missing(1)}",
    ] {
        let errors=semaprax::check(&format!("{SCALAR} {body}"),"inference-v2-hostile.spx").unwrap_err();
        assert!(errors.iter().any(|error|error.code=="SPX-T225"),"{body}: {errors:?}");
    }
}

#[test]
fn argument_inference_v2_evidence_depth_boundary_is_exact() {
    use semaprax::ast::{Expr, ExprKind, UnaryOp};
    // The text parser has its own enclosing-expression depth bound. Construct
    // the argument AST after parsing to isolate this evidence walk's boundary.
    let program = |depth: usize, vector: &str, nested_calls: bool| {
        let source = format!("{SCALAR} @id(\"infer.invoke\") fn invoke()->bool{{identity{vector}(true)}} @id(\"infer.main\") fn main()->i64{{0}}");
        let mut parsed = semaprax::check(&source, "inference-depth.spx").unwrap();
        let function = parsed
            .functions
            .iter_mut()
            .find(|f| f.name == "invoke")
            .unwrap();
        let ExprKind::Block { tail, .. } = &mut function.body.kind else {
            panic!("function block")
        };
        let ExprKind::Call { args, .. } = &mut tail.kind else {
            panic!("identity call")
        };
        for _ in 0..depth {
            args[0] = Expr {
                span: args[0].span,
                kind: if nested_calls {
                    ExprKind::Call {
                        name: "identity".into(),
                        type_arguments: Vec::new(),
                        args: vec![args[0].clone()],
                    }
                } else {
                    ExprKind::Unary {
                        op: UnaryOp::Not,
                        value: Box::new(args[0].clone()),
                    }
                },
            };
        }
        parsed
    };
    for (depth, vector, calls) in [(127, "", false), (128, "<bool>", false), (127, "", true)] {
        let parsed = program(depth, vector, calls);
        assert!(semaprax::verify::verify(&parsed).is_empty());
        let resolved = semaprax::hir::resolve(&parsed).unwrap();
        semaprax::hir::validate(&resolved).unwrap();
    }
    for calls in [false, true] {
        let errors = semaprax::verify::verify(&program(128, "", calls));
        assert!(
            errors.iter().any(|error| error.code == "SPX-T225"),
            "nested calls={calls}: {errors:?}"
        );
    }
}

#[test]
fn argument_inference_v2_evidence_node_budget_is_shared_across_arguments() {
    fn tree(depth: usize) -> String {
        if depth == 0 {
            "1".into()
        } else {
            let child = tree(depth - 1);
            format!("({child}+{child})")
        }
    }
    // Each balanced depth10 tree has2047 nodes. Two unary nodes put the
    // complete two-argument evidence walk at4096; a third is the first excess.
    let tree = tree(10);
    let program = |operators: &str, vector: &str| {
        format!("{SCALAR} @id(\"infer.invoke\") fn invoke()->i64{{same{vector}({tree},{operators}{tree})}} @id(\"infer.main\") fn main()->i64{{0}}")
    };
    checked(&program("--", ""));
    let errors = semaprax::check(&program("---", ""), "inference-nodes-over.spx").unwrap_err();
    assert!(
        errors.iter().any(|error| error.code == "SPX-T225"),
        "{errors:?}"
    );
    checked(&program("---", "<i64>"));
}

#[test]
fn argument_inference_v2_combines_nominal_and_scalar_slots_without_refunding_moves() {
    let source = r#"
module test.inferred_nominal_slots;
@id("infer.pair") record Pair<P,T> {
 @id("infer.payload") payload:P,
 @id("infer.marker") marker:T,
}
@id("infer.owner") fn owner<T,U>(value:own Pair<Bytes,T>,tag:U)->Pair<Bytes,T>{value}
@id("infer.invoke") fn invoke(value:own Pair<Bytes,bool>)->Pair<Bytes,bool>{owner(value,1+2)}
@id("infer.main") fn main()->i64{0}
"#;
    let inferred = checked(source);
    let explicit = checked(&source.replace("owner(value,1+2)", "owner<bool,i64>(value,1+2)"));
    assert_eq!(inferred.function_instances, explicit.function_instances);
    assert_eq!(inferred.function_instances.len(), 1);
    assert_eq!(
        inferred.function_instances[0].type_arguments,
        vec![
            semaprax::hir::ResolvedType::Bool,
            semaprax::hir::ResolvedType::I64
        ]
    );
    let moved = source.replace(
        "{owner(value,1+2)}",
        "{let first=owner(value,1+2);owner(value,4)}",
    );
    let errors = semaprax::check(&moved, "inferred-nominal-double-move.spx").unwrap_err();
    assert!(
        errors.iter().any(|error| error.code == "SPX-O101"),
        "{errors:?}"
    );
}

#[test]
fn argument_inference_v3_symbolic_forwarding_matches_explicit_instances_and_replays() {
    let explicit = r#"
module test.inferred_symbolic;
@id("map.first") fn first<A,B>(left:A,right:B)->A{left}
@id("map.permute") fn permute<A,B>(left:A,right:B)->B{first<B,A>(right,left)}
@id("map.repeat") fn repeat<A>(value:A)->A{first<A,A>(value,value)}
@id("map.concrete") fn concrete<A>(value:A)->i64{first<i64,A>(7,value)}
@id("map.transitive") fn transitive<A,B>(left:A,right:B)->A{permute<B,A>(right,left)}
@id("map.run") fn run()->bool{permute<i64,bool>(7,true)}
@id("map.main") fn main()->i64{transitive<i64,bool>(concrete<bool>(repeat<bool>(true)),true)}
"#;
    // Keep source positions equal so complete HIR instance equality includes
    // spans; canonical projection and graph replay are also checked separately.
    let mut inferred = explicit.to_owned();
    for (callee, vector) in [
        ("first", "<B,A>"),
        ("first", "<A,A>"),
        ("first", "<i64,A>"),
        ("permute", "<B,A>"),
        ("permute", "<i64,bool>"),
        ("transitive", "<i64,bool>"),
        ("concrete", "<bool>"),
        ("repeat", "<bool>"),
    ] {
        inferred = inferred.replace(
            &format!("{callee}{vector}("),
            &format!("{callee}{}(", " ".repeat(vector.len())),
        );
    }
    checked(&inferred);
    checked(explicit);
    let resolve = |source: &str| {
        semaprax::hir::resolve(&semaprax::check(source, "symbolic-inference.spx").unwrap()).unwrap()
    };
    let actual = resolve(&inferred);
    let expected = resolve(explicit);
    assert_eq!(actual.function_instances, expected.function_instances);
    assert_eq!(actual.function_templates, expected.function_templates);
    for replacement in [
        semaprax::hir::ResolvedType::TypeParameter {
            owner: semaprax::hir::DeclarationId::new("foreign"),
            index: 0,
        },
        semaprax::hir::ResolvedType::TypeParameter {
            owner: semaprax::hir::DeclarationId::new("map.permute"),
            index: 2,
        },
        semaprax::hir::ResolvedType::Bool,
    ] {
        let mut forged = actual.clone();
        let template = forged
            .function_templates
            .iter_mut()
            .find(|t| t.id.as_str() == "map.permute")
            .unwrap();
        let semaprax::hir::ResolvedExprKind::Block { tail, .. } = &mut template.body.kind else {
            panic!("block")
        };
        let semaprax::hir::ResolvedExprKind::Call {
            callee,
            type_arguments,
            instance,
            ..
        } = &mut tail.kind
        else {
            panic!("call")
        };
        type_arguments[0] = replacement;
        *instance = Some(semaprax::hir::FunctionInstanceId::derive(
            callee,
            type_arguments,
        ));
        assert_eq!(
            semaprax::hir::validate(&forged).unwrap_err().code,
            "SPX-H006"
        );
    }
}

#[test]
fn argument_inference_v3_symbolic_missing_conflict_and_cycles_fail_closed() {
    for (declarations, code) in [
        ("@id(\"bad.outer\") fn outer<T>(value:T)->T{same(value,true)}", "SPX-T225"),
        ("@id(\"bad.missing\") fn missing<T,U>(value:T)->T{value} @id(\"bad.outer\") fn outer<T>(value:T)->T{missing(value)}", "SPX-T225"),
        ("@id(\"bad.first\") fn first<T,U>(left:T,right:U)->i64{second(right,left)} @id(\"bad.second\") fn second<T,U>(left:T,right:U)->i64{first(right,left)}", "SPX-T226"),
        ("@id(\"bad.recursive\") fn recursive<T>(value:T)->T{recursive(value)}", "SPX-T226"),
    ] {
        let source=format!("{SCALAR} {declarations} @id(\"infer.main\") fn main()->i64{{0}}");
        let errors=semaprax::check(&source,"symbolic-inference-hostile.spx").unwrap_err();
        assert!(errors.iter().any(|e|e.code==code),"{declarations}: {errors:?}");
    }
}
