//! Closure carriers must retain the distinct AST and HIR expression tags.

use super::*;
use crate::cache_codec;
use crate::interpreter::{self, InterpreterOptions};

const SOURCE: &str = r#"
module cache.closure;
@id("cache.helper") fn helper(value:i64)->i64 {
    let bytes=bytes_zeroed(1usize);
    value
}
@id("cache.main") fn main()->i64 {
    let offset=40;
    let callback=fn(value:i64)->i64 { helper(offset + value) };
    callback(2)
}
"#;

#[test]
fn closure_ast_tag_25_and_hir_tag_31_round_trip_with_graph_and_interpreter() {
    let ast = crate::check(SOURCE, "cache-closure.spx").unwrap();
    let ast_wire = cache_codec::encode(&ast).unwrap();
    let main = ast
        .functions
        .iter()
        .find(|function| function.stable_id == "cache.main")
        .unwrap();
    let crate::ast::ExprKind::Block { statements, .. } = &main.body.kind else {
        panic!("block");
    };
    let crate::ast::Statement::Let { value, .. } = &statements[1] else {
        panic!("closure binding");
    };
    assert_eq!(
        &cache_codec::encode(&value.kind).unwrap()[..2],
        &25u16.to_le_bytes()
    );
    let decoded_ast: crate::ast::Program = cache_codec::decode(&ast_wire).unwrap();
    assert_eq!(
        crate::format::canonical(&decoded_ast),
        crate::format::canonical(&ast)
    );
    assert_eq!(cache_codec::encode(&decoded_ast).unwrap(), ast_wire);

    let resolved = crate::hir::resolve(&decoded_ast).unwrap();
    crate::hir::validate(&resolved).unwrap();
    let hir_wire = cache_codec::encode(&resolved).unwrap();
    let closure = crate::hir::closure::inventory(&resolved)[0];
    assert_eq!(
        &cache_codec::encode(&closure.kind).unwrap()[..2],
        &31u16.to_le_bytes()
    );
    let decoded_hir: ResolvedProgram = cache_codec::decode(&hir_wire).unwrap();
    crate::hir::validate(&decoded_hir).unwrap();
    assert_eq!(cache_codec::encode(&decoded_hir).unwrap(), hir_wire);

    let graph = crate::graph::to_json(&decoded_ast).unwrap();
    assert!(graph.contains("semaprax.graph.v37"), "{graph}");
    crate::graph::verify_json(&decoded_ast, &graph).unwrap();
    let path =
        std::env::temp_dir().join(format!("semaprax-cache-closure-{}.spx", std::process::id()));
    std::fs::write(&path, crate::format::canonical(&decoded_ast)).unwrap();
    let interpreted =
        interpreter::interpret(&path, "cache.main", &[], &InterpreterOptions::default()).unwrap();
    let _ = std::fs::remove_file(path);
    assert!(interpreted.envelope.contains("\"value\":\"42\""));
}

#[test]
fn generic_closure_cache_preserves_distinct_concrete_private_bodies() {
    let source = r#"
module cache.generic_closure;
@id("cache.fill") fn fill<T>(values:own Vec<T>, replacement:T)->Vec<T> {
    let callback=fn(item:T)->T { replacement };
    let count=vec_len<T>(values);
    let mut output=vec_with_capacity<T>(count);
    let mut index=0usize;
    while index<count {
        output=vec_push<T>(output,callback(vec_get<T>(values,index)));
        index=index+1usize;
        true
    }
    output
}
@id("cache.main") fn main()->i64 {
    let integers=fill<i64>(vec_push<i64>(vec_with_capacity<i64>(1usize),1),40);
    let flags=fill<bool>(vec_push<bool>(vec_with_capacity<bool>(1usize),false),true);
    if vec_get<bool>(flags,0usize) { vec_get<i64>(integers,0usize)+2 } else {0}
}
"#;
    let ast = crate::check(source, "generic-closure-cache.spx").unwrap();
    let source_wire = cache_codec::encode(&ast).unwrap();
    let replayed_ast: crate::ast::Program = cache_codec::decode(&source_wire).unwrap();
    assert_eq!(cache_codec::encode(&replayed_ast).unwrap(), source_wire);
    let program = crate::hir::resolve(&replayed_ast).unwrap();
    let wire = cache_codec::encode(&program).unwrap();
    let replayed: ResolvedProgram = cache_codec::decode(&wire).unwrap();
    crate::hir::validate(&replayed).unwrap();
    assert_eq!(cache_codec::encode(&replayed).unwrap(), wire);

    let sites = crate::hir::closure::inventory(&replayed);
    assert_eq!(sites.len(), 2);
    let products = sites
        .iter()
        .map(|site| crate::hir::closure::closure_function(&replayed, site).unwrap())
        .collect::<Vec<_>>();
    assert_ne!(sites[0].id, sites[1].id);
    assert_ne!(products[0].id, products[1].id);
    assert_ne!(products[0].params[0].id, products[1].params[0].id);
    assert_ne!(products[0].body.id, products[1].body.id);
    assert_ne!(products[0].return_type, products[1].return_type);
    for product in &products {
        assert!(product.params.iter().all(|p| p.ty == product.return_type));
    }
    let expected = crate::graph::to_json(&ast).unwrap();
    assert_eq!(crate::graph::to_json(&replayed_ast).unwrap(), expected);
    crate::graph::verify_json(&replayed_ast, &expected).unwrap();
}
