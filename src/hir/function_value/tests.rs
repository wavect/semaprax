use super::*;
const SOURCE: &str = r#"
module test.function_values;
@id("fn.increment") fn increment(value:i64)->i64{value+1}
@id("fn.decrement") fn decrement(value:i64)->i64{value-1}
@id("fn.apply") fn apply(callback:fn(i64)->i64,value:i64)->i64{callback(value)}
@id("fn.main") fn main()->i64{let callback=if true{increment}else{decrement};apply(callback,41)}
"#;
fn resolved() -> ResolvedProgram {
    crate::hir::resolve(&crate::check(SOURCE, "function-values.spx").unwrap()).unwrap()
}
#[test]
fn function_values_hir_replays_target_universe_and_exact_invocation() {
    let program = resolved();
    crate::hir::validate(&program).unwrap();
    assert_eq!(
        target_universe(&program)
            .iter()
            .map(|f| f.id.as_str())
            .collect::<Vec<_>>(),
        vec!["fn.decrement", "fn.increment"]
    );
    let apply = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "fn.apply")
        .unwrap();
    assert_eq!(compatible_targets(&program, &apply.params[0].ty).len(), 2);
    let ResolvedExprKind::Block { tail, .. } = &apply.body.kind else {
        panic!("block")
    };
    assert!(matches!(tail.kind, ResolvedExprKind::Invoke { .. }));
    assert!(signature(apply).is_none());
    assert!(
        program
            .declarations
            .type_facts(&apply.params[0].ty)
            .unwrap()
            .copy
    );
}
#[test]
fn function_values_hir_rejects_foreign_target_and_forged_invocation_signature() {
    let program = resolved();
    let mut forged = program.clone();
    let main = forged
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "fn.main")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &mut main.body.kind else {
        panic!("block")
    };
    let super::super::ResolvedStatement::Let { value, .. } = &mut statements[0] else {
        panic!("binding")
    };
    let ResolvedExprKind::If { then_branch, .. } = &mut value.kind else {
        panic!("if")
    };
    let ResolvedExprKind::Block { tail, .. } = &mut then_branch.kind else {
        panic!("branch")
    };
    let ResolvedExprKind::FunctionReference { target } = &mut tail.kind else {
        panic!("reference")
    };
    *target = DeclarationId::new("foreign.target");
    assert_eq!(crate::hir::validate(&forged).unwrap_err().code, "SPX-H006");
    let mut status_forgery = program.clone();
    let apply = status_forgery
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "fn.apply")
        .unwrap();
    let source = apply
        .cleanup_plan
        .status_sources
        .iter_mut()
        .find(|s| {
            matches!(
                s.producer,
                crate::cleanup_plan::StatusProducer::PropagatedCall { .. }
            )
        })
        .unwrap();
    let crate::cleanup_plan::StatusProducer::PropagatedCall { callee } = &mut source.producer
    else {
        unreachable!()
    };
    assert_eq!(callee.as_str(), "core.function.invoke");
    *callee = DeclarationId::new("fn.increment");
    assert_eq!(
        crate::hir::validate(&status_forgery).unwrap_err().code,
        "SPX-H006"
    );
    let mut forged = program;
    let apply = forged
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "fn.apply")
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &mut apply.body.kind else {
        panic!("block")
    };
    let ResolvedExprKind::Invoke { args, .. } = &mut tail.kind else {
        panic!("invoke")
    };
    args[0].ty = ResolvedType::Bool;
    assert_eq!(crate::hir::validate(&forged).unwrap_err().code, "SPX-H006");
}
#[test]
fn function_values_graph_v36_binds_candidates_and_rejects_legacy_projection() {
    let parsed = crate::check(SOURCE, "function-values.spx").unwrap();
    let canonical = crate::format::canonical(&parsed);
    let reparsed = crate::check(&canonical, "function-values-roundtrip.spx").unwrap();
    assert_eq!(canonical, crate::format::canonical(&reparsed));
    let graph = crate::graph::to_json(&parsed).unwrap();
    assert!(graph.contains("semaprax.graph.v36"));
    assert!(graph.contains("function_reference"));
    assert!(graph.contains("candidate_targets"));
    crate::graph::verify_json(&parsed, &graph).unwrap();
    assert!(crate::graph::to_legacy_json(&parsed).is_err());
    let forged = graph.replace("fn.increment", "forged.target");
    assert!(crate::graph::verify_json(&parsed, &forged).is_err());
    assert!(crate::graph::verify_json(
        &parsed,
        &graph.replace("semaprax.graph.v36", "semaprax.graph.v35")
    )
    .is_err());
}

#[test]
fn function_values_target_universe_bound_is_exact() {
    fn source(count: usize) -> String {
        let mut source = String::from("module test.function_value_bound;\n");
        for i in 0..count {
            source.push_str(&format!(
                "@id(\"bound.target.{i:03}\") fn target_{i}()->i64{{{i}}}\n"
            ));
        }
        source.push_str("@id(\"bound.main\") fn main()->i64{");
        for i in 0..count {
            source.push_str(&format!("let ref_{i}=target_{i};"));
        }
        source.push_str("0}");
        source
    }
    let accepted = crate::check(&source(256), "function-bound.spx").unwrap();
    let accepted = crate::hir::resolve(&accepted).unwrap();
    crate::hir::validate(&accepted).unwrap();
    assert_eq!(target_universe(&accepted).len(), 256);
    let rejected = crate::check(&source(257), "function-bound-over.spx").unwrap();
    let errors = crate::hir::resolve(&rejected).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| e.code == "SPX-H006" && e.message.contains("256")),
        "{errors:?}"
    );
}

#[test]
fn function_values_unreferenced_eligible_wrapper_is_not_an_invocation_candidate() {
    let source = r#"
module test.function_value_candidates;
@id("candidate.target") fn target(value:i64)->i64{value+1}
@id("candidate.apply") fn apply(callback:fn(i64)->i64,value:i64)->i64{callback(value)}
@id("candidate.wrapper") fn wrapper(value:i64)->i64{apply(target,value)}
@id("candidate.main") fn main()->i64{wrapper(41)}
"#;
    let parsed = crate::check(source, "function-candidate-cycle.spx").unwrap();
    let program = crate::hir::resolve(&parsed).unwrap();
    crate::hir::validate(&program).unwrap();
    let wrapper = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "candidate.wrapper")
        .unwrap();
    assert!(signature(wrapper).is_some());
    let apply = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "candidate.apply")
        .unwrap();
    assert_eq!(
        compatible_targets(&program, &apply.params[0].ty)
            .iter()
            .map(|f| f.id.as_str())
            .collect::<Vec<_>>(),
        vec!["candidate.target"]
    );
    let graph = crate::graph::to_json(&parsed).unwrap();
    assert!(graph.contains("\"candidate_targets\":[\"candidate.target\"]"));
}

#[test]
fn function_values_coherent_alternative_graph_cannot_rebind_retained_source() {
    let original = crate::check(SOURCE, "function-original.spx").unwrap();
    let altered = SOURCE.replace(
        "if true{increment}else{decrement}",
        "if true{decrement}else{decrement}",
    );
    let altered = crate::check(&altered, "function-altered.spx").unwrap();
    let altered_graph = crate::graph::to_json(&altered).unwrap();
    crate::graph::verify_json(&altered, &altered_graph).unwrap();
    let reminted = altered_graph.replace(
        &crate::graph::revision(&altered),
        &crate::graph::revision(&original),
    );
    assert_eq!(
        crate::graph::verify_json(&original, &reminted).unwrap_err()[0].code,
        "SPX-G411"
    );
}

#[test]
fn function_values_synthetic_invocation_cannot_be_forged_as_an_ordinary_call() {
    let mut program = resolved();
    let apply = program
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "fn.apply")
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &mut apply.body.kind else {
        panic!("block")
    };
    let ResolvedExprKind::Invoke { callable, args } = &tail.kind else {
        panic!("invoke")
    };
    tail.kind = ResolvedExprKind::Call {
        callee: INVOKE_ID.clone(),
        type_arguments: vec![callable.ty.clone()],
        instance: None,
        args: args.clone(),
    };
    assert_eq!(crate::hir::validate(&program).unwrap_err().code, "SPX-H006");
}

#[test]
fn function_values_references_cannot_target_templates_methods_or_imports() {
    let source = format!(
        "{SOURCE}{}",
        r#"
@id("excluded.generic") fn generic<T>(value:T)->T{value}
@id("excluded.class") class Point {
 @id("excluded.field") value:i64,
 @id("excluded.method") fn observe(self:Point)->i64{self.value}
}
@id("excluded.token") resource Token {
 @id("excluded.drop") drop import "excluded.import";
}
@id("excluded.host") interface TokenHost permits { excluded.release } {
 @id("excluded.import") import fn finalize(token:own Token)->unit
 effects { excluded.release } failure infallible consumes token always;
}
"#
    );
    let program =
        crate::hir::resolve(&crate::check(&source, "excluded-callable-targets.spx").unwrap())
            .unwrap();
    for id in ["excluded.generic", "excluded.method", "excluded.import"] {
        assert!(program
            .declarations
            .declaration(&DeclarationId::new(id))
            .is_some());
        let mut forged = program.clone();
        let main = forged
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "fn.main")
            .unwrap();
        let ResolvedExprKind::Block { statements, .. } = &mut main.body.kind else {
            panic!("block")
        };
        let super::super::ResolvedStatement::Let { value, .. } = &mut statements[0] else {
            panic!("binding")
        };
        let ResolvedExprKind::If { then_branch, .. } = &mut value.kind else {
            panic!("if")
        };
        let ResolvedExprKind::Block { tail, .. } = &mut then_branch.kind else {
            panic!("branch")
        };
        let ResolvedExprKind::FunctionReference { target } = &mut tail.kind else {
            panic!("reference")
        };
        *target = DeclarationId::new(id);
        assert_eq!(
            crate::hir::validate(&forged).unwrap_err().code,
            "SPX-H006",
            "{id}"
        );
    }
}
