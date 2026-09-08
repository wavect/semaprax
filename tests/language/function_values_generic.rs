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
