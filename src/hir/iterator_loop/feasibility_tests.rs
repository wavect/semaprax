//! Condition feasibility is reconstructed from HIR, never its cleanup plan.
use super::*;
const SOURCE: &str = r#"module iterator.feasibility;
@id("app.main") fn main()->i64 ensures result==33 {
 let a=vec_push<Bytes>(vec_with_capacity<Bytes>(4usize),bytes_zeroed(1usize));
 let b=vec_push<Bytes>(a,bytes_zeroed(2usize));
 let c=vec_push<Bytes>(b,bytes_zeroed(3usize));
 let values=vec_push<Bytes>(c,bytes_zeroed(4usize));
 let mut seen=0;let mut total=0;
 for own item in vec_into_iter<Bytes>(values) {
  let width=byte_len(bytes_as_slice(item));
  seen=seen+1;
  if seen==1 {continue;} else {0}
  if width==2usize {continue;} else {0}
  total=total+3;
  break;
 }
 return total+seen*10;
}"#;
fn fixture(source: &str) -> ResolvedProgram {
    crate::hir::resolve(&crate::check(source, "iterator-feasibility.spx").unwrap()).unwrap()
}
fn main(program: &ResolvedProgram) -> &ResolvedFunction {
    program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap()
}
fn loop_mut(
    expression: &mut ResolvedExpr,
) -> Option<(
    &mut ResolvedExpr,
    &mut ResolvedExpr,
    &mut crate::hir::LoopLatchKind,
)> {
    let ResolvedExprKind::Block { statements, tail } = &mut expression.kind else {
        return None;
    };
    for statement in statements {
        match statement {
            ResolvedStatement::LexicalWhile {
                condition,
                body,
                latch,
                ..
            } => return Some((condition, body, latch)),
            ResolvedStatement::While { body, .. } => {
                if let Some(found) = loop_mut(body) {
                    return Some(found);
                }
            }
            other => {
                if let Some(found) = loop_mut(other.value_mut()) {
                    return Some(found);
                }
            }
        }
    }
    loop_mut(tail)
}
fn replacement(body: &mut ResolvedExpr) -> &mut ResolvedExpr {
    let ResolvedExprKind::Block { statements, .. } = &mut body.kind else {
        panic!("retained wrapper");
    };
    let [ResolvedStatement::Assign { value, .. }] = statements.as_mut_slice() else {
        panic!("retained assignment");
    };
    value
}
#[test]
fn exact_true_predicate_authenticates_whole_done_leaf_and_yield_latch() {
    let mut program = fixture(SOURCE);
    crate::hir::validate(&program).unwrap();
    let (condition, body, latch) = loop_mut(
        &mut program
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap()
            .body,
    )
    .unwrap();
    let selected = true_body_match(condition, body, latch).unwrap();
    let selected_id = selected.id.clone();
    assert!(matches!(&selected.kind,ResolvedExprKind::Match{arms,..} if arms.len()==2));
    assert_eq!(selected_id, replacement(body).id);
    assert!(true_body_match(condition, body, &crate::hir::LoopLatchKind::Condition).is_none());
    let ResolvedExprKind::Match { arms, .. } = &condition.kind else {
        unreachable!()
    };
    assert!(matches!(arms[0].value.kind, ResolvedExprKind::Bool(false)));
    assert!(matches!(arms[1].value.kind, ResolvedExprKind::Bool(true)));
}
#[test]
fn abrupt_mixed_empty_and_nested_iterations_keep_full_hir_and_exact_backend_routes() {
    // A break statement is represented by an abrupt initializer in its
    // checked block; native lowering must not request a Never C local.
    let abrupt_statement = fixture(
        r#"module iterator.feasibility;
@id("app.main") fn main()->i64 {
 let mut seen=0;while seen<1 {seen=seen+1;break;} seen
}"#,
    );
    crate::hir::validate(&abrupt_statement).unwrap();
    assert!(!crate::codegen::emit_hir_c(&abrupt_statement)
        .unwrap()
        .is_empty());
    let mixed = SOURCE.replace("  break;", "  if seen==3 {break;} else {0} 0");
    let empty = SOURCE
        .replace(
            "let values=vec_push<Bytes>(c,bytes_zeroed(4usize));",
            "let values=vec_with_capacity<Bytes>(0usize);",
        )
        .replace("result==33", "result==0");
    let nested = SOURCE.replace(
        "  seen=seen+1;",
        "  let mut inner=0;while inner<1 {inner=inner+1;0}
  seen=seen+1;",
    );
    for (source, normal) in [
        (SOURCE, false),
        (mixed.as_str(), true),
        (empty.as_str(), false),
        (nested.as_str(), false),
    ] {
        let checked = crate::check(source, "iterator-feasibility.spx").unwrap();
        let canonical = crate::format::canonical(&checked);
        let program = fixture(&canonical);
        crate::hir::validate(&program).unwrap();
        let loop_ = main(&program)
            .cleanup_plan
            .lexical_control
            .as_ref()
            .unwrap()
            .loops
            .iter()
            .find(|loop_| {
                matches!(
                    loop_.latch,
                    crate::hir::LoopLatchKind::OwnedIteratorStep { .. }
                )
            })
            .unwrap();
        assert_eq!(loop_.iteration_end.0 != u32::MAX, normal);
        let graph = crate::graph::to_json(&checked).unwrap();
        crate::graph::verify_json(&checked, &graph).unwrap();
        assert!(!crate::codegen::emit_hir_c(&program).unwrap().is_empty());
        assert!(!crate::wasm::emit_resolved_module(&program)
            .unwrap()
            .is_empty());
    }
}
#[test]
fn forged_predicate_done_latch_and_normal_endpoint_refuse_independent_replay_and_targets() {
    let original = fixture(SOURCE);
    crate::hir::validate(&original).unwrap();
    let mut missing_region = original.clone();
    let function = missing_region
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    let (condition, body, latch) = loop_mut(&mut function.body).unwrap();
    let selected = true_body_match(condition, body, latch).unwrap();
    let ResolvedExprKind::Match { arms, .. } = &selected.kind else {
        unreachable!()
    };
    let done = crate::cleanup_plan::StorageId::Temporary(arms[0].value.id.clone());
    let region = function
        .cleanup_plan
        .regions
        .iter()
        .find(|region| region.slots.contains(&done))
        .unwrap()
        .id;
    let certificate = function.cleanup_plan.lexical_control.as_mut().unwrap();
    assert!(certificate.abrupt_only_regions.contains(&region));
    certificate
        .abrupt_only_regions
        .retain(|candidate| *candidate != region);
    assert!(crate::hir::validate(&missing_region).is_err());
    for mutation in 0..9 {
        let mut changed = original.clone();
        let function = changed
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap();
        if mutation == 8 {
            function
                .cleanup_plan
                .lexical_control
                .as_mut()
                .unwrap()
                .loops[0]
                .iteration_end = crate::cleanup_plan::ExitTargetId(0);
        } else {
            let (condition, body, latch) = loop_mut(&mut function.body).unwrap();
            match mutation {
                0..=2 => {
                    let ResolvedExprKind::Match { arms, .. } = &mut condition.kind else {
                        unreachable!()
                    };
                    match mutation {
                        0 => arms[0].value.kind = ResolvedExprKind::Bool(true),
                        1 => arms[1].value.kind = ResolvedExprKind::Bool(false),
                        _ => arms[0].value.ty = ResolvedType::I64,
                    }
                }
                3..=5 => {
                    let ResolvedExprKind::Match { arms, .. } = &mut replacement(body).kind else {
                        unreachable!()
                    };
                    match mutation {
                        3 => arms[0].value.ownership = OwnershipMode::Borrow,
                        4 => arms[0].value.kind = ResolvedExprKind::Bool(false),
                        _ => arms[0].value.ty = ResolvedType::I64,
                    }
                }
                6 => {
                    let crate::hir::LoopLatchKind::OwnedIteratorStep { next, .. } = latch else {
                        unreachable!()
                    };
                    *next = body.id.clone();
                }
                7 => {
                    let ResolvedExprKind::Match { scrutinee, .. } = &mut condition.kind else {
                        unreachable!()
                    };
                    scrutinee.ownership = OwnershipMode::Borrow;
                }
                _ => unreachable!(),
            }
            assert!(
                true_body_match(condition, body, latch).is_none(),
                "mutation {mutation}"
            );
        }
        assert!(
            crate::hir::validate(&changed).is_err(),
            "mutation {mutation}"
        );
        assert!(
            crate::codegen::emit_hir_c(&changed).is_err(),
            "mutation {mutation}"
        );
        assert!(
            crate::wasm::emit_resolved_module(&changed).is_err(),
            "mutation {mutation}"
        );
    }
}
