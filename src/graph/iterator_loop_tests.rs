//! Canonical graph and retained-HIR authentication for consuming iterator loops.

use std::path::Path;

const SOURCE: &str = r#"
module test.iterator_loop_graph;
@id("app.main") fn main()->i64 {
  let mut total=0;
  let values=vec_push<i64>(vec_push<i64>(vec_with_capacity<i64>(2usize),3),4);
  for own item in vec_into_iter<i64>(values) { total=total+item; 0 }
  total
}
"#;

fn checked() -> crate::ast::Program {
    crate::check(SOURCE, Path::new("iterator-loop-graph.spx"))
        .expect("iterator-loop fixture checks")
}

fn resolved() -> crate::hir::ResolvedProgram {
    crate::hir::resolve(&checked()).expect("iterator-loop fixture resolves")
}

fn main_function(program: &crate::hir::ResolvedProgram) -> &crate::hir::ResolvedFunction {
    program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "app.main")
        .expect("main is retained")
}

fn iterator_loop_mut(
    program: &mut crate::hir::ResolvedProgram,
) -> (&mut crate::hir::ResolvedExpr, &mut crate::hir::ResolvedExpr) {
    let main = program
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "app.main")
        .expect("main is retained");
    let crate::hir::ResolvedExprKind::Block { statements, .. } = &mut main.body.kind else {
        panic!("main body is a block");
    };
    let crate::hir::ResolvedStatement::Let { value: wrapper, .. } = statements
        .iter_mut()
        .find(|statement| matches!(statement, crate::hir::ResolvedStatement::Let { binding, .. } if binding.name == "#for-own"))
        .expect("for own lowers to one wrapper binding")
    else {
        panic!("iterator lowering retains its wrapper binding");
    };
    let crate::hir::ResolvedExprKind::Block { statements, .. } = &mut wrapper.kind else {
        panic!("iterator wrapper is a block");
    };
    let crate::hir::ResolvedStatement::While {
        condition, body, ..
    } = statements
        .iter_mut()
        .find(|statement| matches!(statement, crate::hir::ResolvedStatement::While { .. }))
        .expect("iterator wrapper retains a while protocol")
    else {
        panic!("iterator lowering retains a while protocol");
    };
    (condition, body)
}

#[test]
fn iterator_loop_is_canonical_and_selects_v39_v11() {
    let parsed = crate::parse(SOURCE, Path::new("iterator-loop-graph.spx"))
        .expect("iterator-loop source parses");
    let canonical = crate::format::canonical(&parsed);
    let reparsed = crate::parse(&canonical, Path::new("iterator-loop-canonical.spx"))
        .expect("canonical iterator-loop source parses");
    assert_eq!(canonical, crate::format::canonical(&reparsed));

    let program = checked();
    let resolved = crate::hir::resolve(&program).expect("iterator-loop fixture resolves");
    let main = main_function(&resolved);
    assert_eq!(
        main.cleanup_plan.schema,
        crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V11
    );
    assert!(crate::hir::iterator_loop::function_contains(main));
    assert_eq!(
        super::graph_schema(&resolved).unwrap(),
        "semaprax.graph.v39"
    );

    let graph = crate::graph::to_json(&program).expect("iterator-loop graph emits");
    assert!(graph.contains("\"schema\":\"semaprax.graph.v39\""));
    crate::graph::verify_json(&program, &graph).expect("iterator-loop graph replays");
    let downgraded = graph.replacen("semaprax.graph.v39", "semaprax.graph.v38", 1);
    assert!(crate::graph::verify_json(&program, &downgraded).is_err());
}

#[test]
fn changed_iterator_loop_condition_or_next_is_rejected() {
    let mut changed_condition = resolved();
    let (condition, _) = iterator_loop_mut(&mut changed_condition);
    condition.kind = crate::hir::ResolvedExprKind::Bool(true);
    assert!(crate::hir::validate(&changed_condition).is_err());
    assert!(super::graph_schema(&changed_condition).is_err());

    let mut changed_next = resolved();
    let (_, body) = iterator_loop_mut(&mut changed_next);
    let crate::hir::ResolvedExprKind::Block { statements, .. } = &mut body.kind else {
        panic!("iterator protocol body is a block");
    };
    let crate::hir::ResolvedStatement::Assign { value, .. } = &mut statements[0] else {
        panic!("iterator protocol updates the hidden Step slot");
    };
    let crate::hir::ResolvedExprKind::Match { arms, .. } = &mut value.kind else {
        panic!("iterator protocol replaces the Step through an own match");
    };
    let crate::hir::ResolvedExprKind::Block { tail, .. } = &mut arms[1].value.kind else {
        panic!("Yield replacement is a block");
    };
    let crate::hir::ResolvedExprKind::Call { callee, .. } = &mut tail.kind else {
        panic!("Yield replacement calls iter_next");
    };
    *callee = crate::hir::DeclarationId::new("core.iter.next.forged");
    assert!(crate::hir::validate(&changed_next).is_err());
    assert!(super::graph_schema(&changed_next).is_err());
}

#[test]
fn iterator_loop_cache_tag_is_additive_and_cleanup_downgrade_rejects() {
    let parsed = checked();
    let crate::ast::ExprKind::Block { statements, .. } = &parsed.functions[0].body.kind else {
        panic!("fixture has a block body");
    };
    let statement = statements
        .iter()
        .find(|statement| matches!(statement, crate::ast::Statement::ForOwn { .. }))
        .unwrap();
    let encoded = crate::cache_codec::encode(statement).unwrap();
    assert_eq!(encoded[0], 5);
    let decoded: crate::ast::Statement = crate::cache_codec::decode(&encoded).unwrap();
    assert_eq!(crate::cache_codec::encode(&decoded).unwrap(), encoded);
    let crate::ast::Statement::ForOwn {
        item,
        item_span,
        values,
        body,
        span,
    } = decoded
    else {
        panic!("cache retains the consuming statement");
    };
    let legacy = crate::ast::Statement::For {
        item,
        item_span,
        values,
        body,
        span,
    };
    let legacy_encoded = crate::cache_codec::encode(&legacy).unwrap();
    assert_eq!(legacy_encoded[0], 4);
    assert_eq!(&legacy_encoded[1..], &encoded[1..]);

    let mut changed = resolved();
    let main = changed
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    main.cleanup_plan.schema = crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V10;
    assert!(crate::hir::validate(&changed).is_err());
    assert!(super::graph_schema(&changed).is_err());
}

#[test]
fn iterator_loop_unused_template_retains_v39_without_concrete_plan() {
    let source = r#"module test.iterator_template_only;
@id("iter.count") fn count<T>(values:own Iter<T>)->i64 {
 let mut count_value=0;
 for own item in values { count_value=count_value+1; 0 }
 count_value
}
@id("app.main") fn main()->i64 { 0 }
"#;
    let checked = crate::check(source, Path::new("iterator-template-only.spx")).unwrap();
    let program = crate::hir::resolve(&checked).unwrap();
    assert!(program.function_instances.is_empty());
    let template = program
        .function_templates
        .iter()
        .find(|item| item.id.as_str() == "iter.count")
        .unwrap();
    assert!(crate::hir::iterator_loop::template_contains(template));
    assert!(program.functions.iter().all(
        |function| function.cleanup_plan.schema != crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V11
    ));
    assert_eq!(super::graph_schema(&program).unwrap(), "semaprax.graph.v39");
    assert_eq!(
        super::graph_schema_from_parts_and_instances(
            &program.interfaces,
            &program.types,
            &program.functions,
            &program.function_templates,
            &program.function_instances
        )
        .unwrap(),
        "semaprax.graph.v39"
    );
    let graph = crate::graph::to_json(&checked).unwrap();
    crate::graph::verify_json(&checked, &graph).unwrap();
    assert!(crate::graph::verify_json(
        &checked,
        &graph.replacen("semaprax.graph.v39", "semaprax.graph.v38", 1)
    )
    .is_err());
    let mut hostile = template.clone();
    hostile.id = crate::hir::DeclarationId::new("foreign.scope");
    assert!(!crate::hir::iterator_loop::template_contains(&hostile));
    let mut immutable = template.clone();
    let mut pending = vec![&mut immutable.body];
    while let Some(expression) = pending.pop() {
        if let crate::hir::ResolvedExprKind::Block { statements, .. } = &mut expression.kind {
            for statement in statements {
                if let crate::hir::ResolvedStatement::Let {
                    binding,
                    mutable,
                    value,
                    ..
                } = statement
                {
                    if binding.name == "#for-own-step" {
                        *mutable = false;
                    }
                    pending.push(value);
                }
            }
        }
    }
    assert!(!crate::hir::iterator_loop::template_contains(&immutable));
}
