//! Scoped scalar callbacks in reusable bounded collection templates.
use semaprax::{graph, hir};

pub const ADAPTERS: &str = r#"
module test.generic_callbacks;
@id("gc.map") fn map<T>(values:own Vec<T>,callback:fn(T)->T)->Vec<T>{
    let length=vec_len<T>(values);
    let mut output=vec_with_capacity<T>(length);
    let mut index=0usize;
    while index<length {
        output=vec_push<T>(output,callback(vec_get<T>(values,index)));
        index=index+1usize;
        index<length
    }
    output
}
@id("gc.filter") fn filter<T>(values:own Vec<T>,callback:fn(T)->bool)->Vec<T>{
    let length=vec_len<T>(values);
    let mut output=vec_with_capacity<T>(length);
    let mut index=0usize;
    while index<length {
        let item=vec_get<T>(values,index);
        let kept=if callback(item) { output=vec_push<T>(output,item); true } else { false };
        index=index+1usize;
        index<length
    }
    output
}
@id("gc.fold") fn fold<T>(values:own Vec<T>,initial:T,callback:fn(T,T)->T)->T{
    let length=vec_len<T>(values);
    let mut total=initial;
    let mut index=0usize;
    while index<length {
        total=callback(total,vec_get<T>(values,index));
        index=index+1usize;
        index<length
    }
    total
}
@id("gc.inc") fn inc(value:i64)->i64 {value+1}
@id("gc.keep") fn keep(value:i64)->bool {value>2}
@id("gc.add") fn add(left:i64,right:i64)->i64 {left+right}
@id("gc.main") fn main()->i64 {
    let values=vec_push<i64>(vec_push<i64>(vec_with_capacity<i64>(2usize),1),3);
    fold<i64>(filter<i64>(map<i64>(values,inc),keep),0,add)
}
"#;

#[test]
fn function_values_generic_adapters_replay_source_hir_and_graph() {
    let parsed = semaprax::check(ADAPTERS, "generic-callbacks.spx").unwrap();
    let canonical = semaprax::format::canonical(&parsed);
    let reparsed = semaprax::check(&canonical, "generic-callbacks.spx").unwrap();
    assert_eq!(canonical, semaprax::format::canonical(&reparsed));
    let program = hir::resolve(&reparsed).unwrap();
    hir::validate(&program).unwrap();
    assert_eq!(program.function_instances.len(), 3);
    let graph = graph::to_json(&reparsed).unwrap();
    assert!(graph.contains("semaprax.graph.v36"));
    assert!(graph.contains("\"kind\":\"invoke\""));
    graph::verify_json(&reparsed, &graph).unwrap();
}

#[test]
fn function_values_generic_callbacks_reject_noncollection_and_wrong_scalar_signature() {
    let unsupported="module bad; @id(\"bad.apply\") fn apply<T>(value:T,callback:fn(T)->T)->T{callback(value)} @id(\"bad.main\") fn main()->i64{0}";
    assert!(semaprax::check(unsupported, "bad.spx")
        .unwrap_err()
        .iter()
        .any(|d| d.code == "SPX-T287"));
    let wrong = ADAPTERS.replace("map<i64>(values,inc)", "map<i64>(values,keep)");
    assert!(semaprax::check(&wrong, "bad.spx").is_err());
}

#[test]
fn function_values_generic_unmaterialized_templates_reject_hostile_mutations() {
    let source = ADAPTERS.replace(
        "fold<i64>(filter<i64>(map<i64>(values,inc),keep),0,add)",
        "0",
    );
    let ast = semaprax::check(&source, "generic-callbacks.spx").unwrap();
    let program = hir::resolve(&ast).unwrap();
    assert!(program.function_instances.is_empty());
    hir::validate(&program).unwrap();
    let unmaterialized_graph = graph::to_json(&ast).unwrap();
    assert!(unmaterialized_graph.contains("semaprax.graph.v36"));
    graph::verify_json(&ast, &unmaterialized_graph).unwrap();
    for mutation in 0..3 {
        let mut forged = program.clone();
        let template = forged
            .function_templates
            .iter_mut()
            .find(|f| f.id.as_str() == "gc.map")
            .unwrap();
        match mutation {
            0 => {
                let hir::ResolvedType::Function { parameters, .. } = &mut template.params[1].ty
                else {
                    panic!("callback")
                };
                parameters[0] = hir::ResolvedType::TypeParameter {
                    owner: hir::DeclarationId::new("foreign.owner"),
                    index: 0,
                };
            }
            1 => {
                let hir::ResolvedExprKind::Block { statements, .. } = &mut template.body.kind
                else {
                    panic!("body")
                };
                let hir::ResolvedStatement::Let { mutable, .. } = &mut statements[2] else {
                    panic!("index")
                };
                *mutable = false;
            }
            _ => {
                let hir::ResolvedExprKind::Block { statements, .. } = &mut template.body.kind
                else {
                    panic!("body")
                };
                let hir::ResolvedStatement::While {
                    condition, body, ..
                } = &mut statements[3]
                else {
                    panic!("while")
                };
                condition.id = body.id.clone();
            }
        }
        let error = hir::validate(&forged).unwrap_err();
        assert_eq!(error.code, "SPX-H006", "mutation {mutation}: {error:?}");
    }
}

const GENERIC_TEMPLATE_CLOSURE: &str = r#"
module test.generic_template_closure;
@id("gc.fill") fn fill<T>(values:own Vec<T>,replacement:T)->Vec<T>{
    let length=vec_len<T>(values);
    let mut output=vec_with_capacity<T>(length);
    let mut index=0usize;
    while index<length {
        let callback=fn(item:T)->T{replacement};
        output=vec_push<T>(output,callback(vec_get<T>(values,index)));
        index=index+1usize;
        index<length
    }
    output
}
@id("gc.main") fn main()->i64{
    let i64s=fill<i64>(vec_push<i64>(vec_with_capacity<i64>(1usize),1),2);
    let i32s=fill<i32>(vec_push<i32>(vec_with_capacity<i32>(1usize),1i32),2i32);
    let u8s=fill<u8>(vec_push<u8>(vec_with_capacity<u8>(1usize),1u8),2u8);
    let usizes=fill<usize>(vec_push<usize>(vec_with_capacity<usize>(1usize),1usize),2usize);
    let chars=fill<char>(vec_push<char>(vec_with_capacity<char>(1usize),'a'),'b');
    let f32s=fill<f32>(vec_push<f32>(vec_with_capacity<f32>(1usize),1.0f32),2.0f32);
    let f64s=fill<f64>(vec_push<f64>(vec_with_capacity<f64>(1usize),1.0f64),2.0f64);
    let bools=fill<bool>(vec_push<bool>(vec_with_capacity<bool>(1usize),true),false);
    if vec_len<i64>(i64s)+vec_len<i32>(i32s)+vec_len<u8>(u8s)+vec_len<usize>(usizes)+vec_len<char>(chars)+vec_len<f32>(f32s)+vec_len<f64>(f64s)+vec_len<bool>(bools)==8usize {0} else {1}
}
"#;

#[test]
fn generic_closure_templates_materialize_per_scalar_instance_with_distinct_creation_ids() {
    let ast = semaprax::check(GENERIC_TEMPLATE_CLOSURE, "generic-template-closures.spx").unwrap();
    let program = hir::resolve(&ast).unwrap();
    hir::validate(&program).unwrap();
    assert_eq!(program.function_instances.len(), 8);
    let closures = hir::closure::inventory(&program);
    assert_eq!(closures.len(), 8);
    let ids = closures
        .iter()
        .map(|closure| closure.id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        ids.len(),
        closures.len(),
        "each concrete generic body owns its closure creation identity"
    );
    assert!(hir::closure::requires_closure_projection(&program));
    let graph = graph::to_json(&ast).unwrap();
    assert!(graph.contains("semaprax.graph.v37"), "{graph}");
    graph::verify_json(&ast, &graph).unwrap();
}

#[test]
fn generic_closure_template_keeps_unused_symbolic_shape_and_closed_profiles_closed() {
    let unused = GENERIC_TEMPLATE_CLOSURE.replace(
        "    let i64s=fill<i64>(vec_push<i64>(vec_with_capacity<i64>(1usize),1),2);\n    let i32s=fill<i32>(vec_push<i32>(vec_with_capacity<i32>(1usize),1i32),2i32);\n    let u8s=fill<u8>(vec_push<u8>(vec_with_capacity<u8>(1usize),1u8),2u8);\n    let usizes=fill<usize>(vec_push<usize>(vec_with_capacity<usize>(1usize),1usize),2usize);\n    let chars=fill<char>(vec_push<char>(vec_with_capacity<char>(1usize),'a'),'b');\n    let f32s=fill<f32>(vec_push<f32>(vec_with_capacity<f32>(1usize),1.0f32),2.0f32);\n    let f64s=fill<f64>(vec_push<f64>(vec_with_capacity<f64>(1usize),1.0f64),2.0f64);\n    let bools=fill<bool>(vec_push<bool>(vec_with_capacity<bool>(1usize),true),false);\n    if vec_len<i64>(i64s)+vec_len<i32>(i32s)+vec_len<u8>(u8s)+vec_len<usize>(usizes)+vec_len<char>(chars)+vec_len<f32>(f32s)+vec_len<f64>(f64s)+vec_len<bool>(bools)==8usize {0} else {1}",
        "    0",
    );
    let ast = semaprax::check(&unused, "unused-generic-template-closure.spx").unwrap();
    let program = hir::resolve(&ast).unwrap();
    assert!(program.function_instances.is_empty());
    assert!(hir::closure::requires_closure_projection(&program));
    assert!(!hir::closure::requires_closures(&program));
    let graph = graph::to_json(&ast).unwrap();
    assert!(graph.contains("semaprax.graph.v37"), "{graph}");
    assert!(graph.contains("\"kind\":\"closure\""), "{graph}");
    graph::verify_json(&ast, &graph).unwrap();

    for source in [
        unused.replace(
            "fn(item:T)->T{replacement}",
            "fn(item:T)->T{let nested=fn()->T{replacement};nested()}",
        ),
        unused.replace(
            "fn(item:T)->T{replacement}",
            "fn(item:T)->T{bytes_zeroed(1usize)}",
        ),
        unused.replace(
            "@id(\"gc.fill\") fn fill<T>(values:own Vec<T>,replacement:T)->Vec<T>{\n    let length=vec_len<T>(values);\n    let mut output=vec_with_capacity<T>(length);\n    let mut index=0usize;\n    while index<length {\n        let callback=fn(item:T)->T{replacement};\n        output=vec_push<T>(output,callback(vec_get<T>(values,index)));\n        index=index+1usize;\n        index<length\n    }\n    output\n}",
            "@id(\"gc.make\") fn make<T>(replacement:T)->i64{let callback=fn(item:T)->T{replacement};0}",
        ),
    ] {
        let diagnostics =
            semaprax::check(&source, "closed-generic-template-closure.spx").unwrap_err();
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "SPX-T288"),
            "{diagnostics:?}"
        );
    }
}
