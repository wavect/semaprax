//! Consuming iterator loops preserve their hidden remainder on every backend.
use super::collections;

const GENERIC_FOLD: &str = r#"
@id("loop.fold") fn fold<T>(values:own Iter<T>,expected:T,callback:fn(T)->T)->i64{
 let mut count=0usize;
 let mut exact=true;
 for own item in values {
  exact=exact && callback(item)==expected;
  count=count+1usize;
  0
 }
 if exact && count==2usize {1}else{0}
}
"#;

fn all_scalar_source() -> String {
    let mut source = String::from("module test.iterator_loops;\n");
    source.push_str(GENERIC_FOLD);
    let mut calls = Vec::new();
    for (ty, item, captured) in [
        ("i64", "-17", "29"),
        ("i32", "-17i32", "29i32"),
        ("u8", "17u8", "29u8"),
        ("usize", "17usize", "29usize"),
        ("char", "'a'", "'z'"),
        ("f32", "-1.25f32", "2.5f32"),
        ("f64", "-1.25", "2.5"),
        ("bool", "false", "true"),
    ] {
        source.push_str(&format!(
            r#"
@id("loop.run.{ty}") fn run_{ty}()->i64{{
 let mut empty_count=0usize;
 for own empty_item in vec_into_iter<{ty}>(vec_with_capacity<{ty}>(0usize)) {{
  empty_count=empty_count+1usize;
  0
 }}
 let mut values=vec_with_capacity<{ty}>(2usize);
 values=vec_push<{ty}>(values,{item});
 values=vec_push<{ty}>(values,{item});
 let mut count=0usize;
 let mut exact=true;
 for own item in vec_into_iter<{ty}>(values) {{
  count=count+1usize;
  exact=exact && item=={item};
  0
 }}
 let captured={captured};
 let callback=fn(value:{ty})->{ty}{{if value=={item}{{captured}}else{{value}}}};
 let mut folded=vec_with_capacity<{ty}>(2usize);
 folded=vec_push<{ty}>(folded,{item});
 folded=vec_push<{ty}>(folded,{item});
 if empty_count==0usize && count==2usize && exact && fold<{ty}>(vec_into_iter<{ty}>(folded),captured,callback)==1{{1}}else{{0}}
}}
"#
        ));
        calls.push(format!("run_{ty}()"));
    }
    source.push_str(&format!(
        "@id(\"app.main\") fn main()->i64{{{}}}",
        calls.join("+")
    ));
    source
}

#[test]
fn consuming_iterator_loops_cover_empty_multiple_yields_and_generic_callbacks_all_scalars() {
    collections::run_source_value(&all_scalar_source(), 8);
}

#[test]
fn consuming_iterator_loop_can_build_a_same_owner_vector_accumulator() {
    let source = r#"module test.iterator_loop_vec_accumulator;
@id("app.main") fn main()->i64{
 let mut input=vec_with_capacity<i64>(3usize);
 input=vec_push<i64>(input,4);
 input=vec_push<i64>(input,5);
 input=vec_push<i64>(input,6);
 let mut output=vec_with_capacity<i64>(3usize);
 for own item in vec_into_iter<i64>(input) {
  output=vec_push<i64>(output,item);
  0
 }
 if vec_len<i64>(output)==3usize && vec_get<i64>(output,0usize)==4 && vec_get<i64>(output,2usize)==6 {1}else{0}
}"#;
    collections::run_source_value(source, 1);
}

#[test]
fn consuming_iterator_loop_body_failure_settles_the_live_remainder() {
    let source = r#"module test.iterator_loop_body_failure;
@id("loop.guard") fn guard(value:i64)->i64 requires value<2 {value}
@id("app.main") fn main()->i64{
 let callback=guard;
 let mut values=vec_with_capacity<i64>(3usize);
 values=vec_push<i64>(values,1);
 values=vec_push<i64>(values,2);
 values=vec_push<i64>(values,3);
 let mut count=0usize;
 for own item in vec_into_iter<i64>(values) {
  let seen=callback(item);
  count=count+1usize;
  0
 }
 0
}"#;
    collections::run_source(source, 1);
}

#[test]
fn consuming_iterator_loop_rejects_source_reuse_stably() {
    let source = r#"module test.iterator_loop_reuse;
@id("app.main") fn main()->i64{
 let iterator=vec_into_iter<i64>(vec_with_capacity<i64>(0usize));
 for own item in iterator {0}
 let reused=iter_next<i64>(iterator);
 0
}"#;
    let first = semaprax::check(source, "iterator-loop-reuse.spx").unwrap_err();
    let second = semaprax::check(source, "iterator-loop-reuse.spx").unwrap_err();
    assert_eq!(format!("{first:?}"), format!("{second:?}"));
    assert!(first.iter().any(|diagnostic| diagnostic.code == "SPX-O101"));
}

#[test]
fn consuming_iterator_loop_selects_graph_v39_and_cleanup_v11() {
    let parsed = semaprax::check(&all_scalar_source(), "iterator-loop-schema.spx").unwrap();
    let resolved = semaprax::hir::resolve(&parsed).unwrap();
    assert!(resolved.functions.iter().any(|function| {
        function.id.as_str() == "loop.run.i64"
            && function.cleanup_plan.schema == semaprax::cleanup_plan::CLEANUP_PLAN_SCHEMA_V11
    }));
    let graph = semaprax::graph::to_json(&parsed).unwrap();
    assert!(graph.contains("semaprax.graph.v39"));
    assert!(graph.contains("semaprax.cleanup-plan.v11"));
}
