use std::path::Path;

use super::PlanBuilder;
use crate::cleanup_plan::{BlockId, CleanupRegionId};
use crate::hir::{self, ResolvedExprKind, ResolvedType};

#[test]
fn owned_match_result_stays_rejected_by_both_cleanup_lowering_paths() {
    let source = r#"
module test.cleanup_owned_match_result;
@id("choose") fn choose(value: i64) -> i64 {
  match value { 0 => 1, _ => 2, }
}
@id("main") fn main() -> i64 { 0 }
"#;
    let program =
        hir::resolve(&crate::parse(source, Path::new("cleanup-owned-match-result.spx")).unwrap())
            .unwrap();
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "choose")
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &function.body.kind else {
        panic!("resolved function body must retain its block");
    };
    let mut hostile = tail.as_ref().clone();
    let ResolvedExprKind::Match { arms, .. } = &mut hostile.kind else {
        panic!("fixture body must be a match");
    };
    for arm in arms {
        arm.value.ty = ResolvedType::Bytes;
    }
    hostile.ty = ResolvedType::Bytes;

    for lower in [
        PlanBuilder::lower_expr_iterative,
        PlanBuilder::lower_expr_recursive_reference,
    ] {
        let mut builder = PlanBuilder::new(&program, function).unwrap();
        let initial_state = builder.initial_state.clone();
        let error = lower(
            &mut builder,
            &hostile,
            BlockId(0),
            initial_state,
            CleanupRegionId(0),
        )
        .unwrap_err();
        assert_eq!(error.code, "SPX-H006");
        assert_eq!(
            error.message,
            "cleanup plan: droppable match result reached the copy-only cleanup slice"
        );
    }
}
