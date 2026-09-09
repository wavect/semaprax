const SOURCE: &str = r#"module test.owned_iterator;
@id("owned.main") fn main()->i64 {
    let values=vec_push<Bytes>(vec_with_capacity<Bytes>(1usize),bytes_zeroed(1usize));
    let step=iter_next<Bytes>(vec_into_iter<Bytes>(values));
    match own step {
        IterStep::Done{}=>0,
        IterStep::Yield{item,rest}=>{
            let view=bytes_as_slice(item);
            if byte_len(view)==1usize {7} else {0}
        },
    }
}
"#;
#[test]
fn owned_iterator_payload_source_graph_and_independent_cleanup_agree() {
    let source = crate::check(SOURCE, "owned-iterator.spx").unwrap();
    let program = crate::hir::resolve(&source).unwrap();
    crate::hir::validate(&program).unwrap();
    assert_eq!(
        crate::prelude::selected_for_program(&source).0,
        crate::prelude::SCHEMA_V8
    );
    assert!(program
        .functions
        .iter()
        .any(|f| f.cleanup_plan.schema == crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V13));
    assert_eq!(
        crate::graph::graph_schema(&program).unwrap(),
        "semaprax.graph.v45"
    );
    let mut forged = program.clone();
    for function in &mut forged.functions {
        if function.cleanup_plan.schema == crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V13 {
            function.cleanup_plan.schema = crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V10;
        }
    }
    assert!(crate::hir::validate(&forged).is_err());
    let canonical = crate::format::canonical(&source);
    let roundtrip = crate::check(&canonical, "owned-iterator.spx").unwrap();
    assert_eq!(canonical, crate::format::canonical(&roundtrip));
    let graph = crate::graph::to_json(&source).unwrap();
    let document: serde_json::Value = serde_json::from_str(&graph).unwrap();
    assert_eq!(document["prelude"]["schema"], crate::prelude::SCHEMA_V8);
    assert_eq!(
        document["prelude"]["digest"],
        crate::prelude::digest_text_v8()
    );
    crate::graph::verify_json(&source, &graph).unwrap();
    assert!(crate::graph::verify_json(
        &source,
        &graph.replacen("semaprax.graph.v45", "semaprax.graph.v38", 1)
    )
    .is_err());
    let mut missing_item = program.clone();
    let function = missing_item
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "owned.main")
        .unwrap();
    let flag = function
        .cleanup
        .flags
        .iter()
        .position(|flag| {
            flag.place
                .projections
                .iter()
                .any(|field| field.as_str() == crate::iterator_ops::ITEM_ID)
        })
        .expect("Yield item has its own cleanup flag");
    function.cleanup.flags.remove(flag);
    assert!(crate::hir::validate(&missing_item).is_err());
}
#[test]
fn owned_iterator_item_cannot_be_consumed_twice() {
    let prefix = "@id(\"owned.consume\") fn consume(value: own Bytes)->i64{0}\n";
    let valid = SOURCE
        .replace(
            "@id(\"owned.main\")",
            &format!("{prefix}@id(\"owned.main\")"),
        )
        .replace(
            "let view=bytes_as_slice(item);\n            if byte_len(view)==1usize {7} else {0}",
            "let first=consume(item);first",
        );
    crate::check(&valid, "single-item.spx").unwrap();
    let duplicate = valid.replace(
        "let first=consume(item);first",
        "let first=consume(item);let second=consume(item);first+second",
    );
    assert_ne!(duplicate, valid);
    let errors = crate::check(&duplicate, "duplicate-item.spx").unwrap_err();
    assert!(
        errors.iter().any(|error| error.code == "SPX-O101"),
        "{errors:?}"
    );
}

#[test]
fn owned_iterator_loop_scopes_each_item_as_one_consumed_owner() {
    let source = r#"module test.owned_loop;
@id("owned.consume") fn consume(value: own Bytes)->usize {
    let view=bytes_as_slice(value); byte_len(view)
}
@id("owned.main") fn main()->i64 {
    let values=vec_push<Bytes>(vec_with_capacity<Bytes>(1usize),bytes_zeroed(1usize));
    let mut total=0usize;
    for own item in vec_into_iter<Bytes>(values) { total=total+consume(item); 0 }
    if total==1usize {1} else {0}
}
"#;
    let checked = crate::check(source, "owned-loop.spx").unwrap();
    let program = crate::hir::resolve(&checked).unwrap();
    crate::hir::validate(&program).unwrap();
    assert!(program
        .functions
        .iter()
        .any(|f| crate::hir::iterator_loop::function_contains(f)
            && f.cleanup_plan.schema == crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V13));
}

#[test]
fn owned_iterator_prelude_contract_is_frozen() {
    assert_eq!(
        crate::prelude::contract_bytes_v8(),
        include_bytes!("../../tests/fixtures/prelude-v8.contract")
    );
}

#[test]
fn owned_iterator_unused_generic_fixed_payload_stays_outside_template_profile() {
    let source = r#"module owned.iterator.template;
@id("owned.unused") fn unused<T>(marker:T)->i64 {
 let iterator=vec_into_iter<Bytes>(vec_with_capacity<Bytes>(0usize));0
}
@id("owned.main") fn main()->i64 {0}
"#;
    let checked = crate::check(source, "owned-template.spx").unwrap();
    let errors = crate::hir::resolve(&checked).unwrap_err();
    assert!(
        errors.iter().any(|error| error.code == "SPX-H006"
            && error
                .message
                .contains("invalid direct-scalar signature slot")),
        "{errors:?}"
    );
}

#[test]
fn owned_iterator_class_method_selects_exact_prelude() {
    let source = r#"module owned.iterator.method;
@id("owned.counter") class Counter {
 @id("owned.counter.value") value:i64,
 @id("owned.counter.run") fn run(self:Counter)->i64 {
  let step=iter_next<Bytes>(vec_into_iter<Bytes>(vec_with_capacity<Bytes>(0usize)));
  match own step {IterStep::Done{}=>self.value,IterStep::Yield{item,rest}=>0,}
 }
}
@id("owned.main") fn main()->i64 {let counter=Counter{value:7};counter.run()}
"#;
    let checked = crate::check(source, "owned-method.spx").unwrap();
    assert_eq!(
        crate::prelude::selected_for_program(&checked).0,
        crate::prelude::SCHEMA_V8
    );
    let program = crate::hir::resolve(&checked).unwrap();
    crate::hir::validate(&program).unwrap();
    assert_eq!(
        crate::graph::graph_schema(&program).unwrap(),
        "semaprax.graph.v45"
    );
}

#[test]
fn owned_iterator_into_commits_only_after_success() {
    use crate::cleanup_plan::{CleanupTransition, EdgeCondition, StatusProducer};
    let checked = crate::check(SOURCE, "owned-into-commit.spx").unwrap();
    let mut program = crate::hir::resolve(&checked).unwrap();
    let function = program
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "owned.main")
        .unwrap();
    let plan = &mut function.cleanup_plan;
    let status = plan.status_sources.iter().find(|source| matches!(&source.producer,
        StatusProducer::PropagatedCall {callee} if callee.as_str()==crate::iterator_ops::INTO_ITER_ID)).unwrap().id.clone();
    let edge = plan
        .edges
        .iter()
        .find(|edge| edge.condition == EdgeCondition::StatusZero(status.clone()))
        .unwrap()
        .clone();
    let success = plan
        .blocks
        .iter()
        .position(|block| block.id == edge.to)
        .unwrap();
    let split = plan
        .blocks
        .iter()
        .position(|block| block.id == edge.from)
        .unwrap();
    let commit = plan.blocks[success]
        .transitions
        .iter()
        .position(|transition| {
            matches!(transition,
        CleanupTransition::CallCommit {call, ..} if call==&status.expression)
        })
        .expect("Into owner commits on success");
    assert!(!plan.blocks[split]
        .transitions
        .iter()
        .any(|transition| matches!(transition,
        CleanupTransition::CallCommit {call, ..} if call==&status.expression)));
    let premature = plan.blocks[success].transitions.remove(commit);
    plan.blocks[split].transitions.push(premature);
    assert!(crate::hir::validate(&program).is_err());
}
