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

fn stream_next_program() -> crate::hir::ResolvedProgram {
    hir::resolve(&crate::parse(r#"module test.stream_next_cleanup;
permit { process.stdin.read }
@id("stream.advance") fn advance(reader: own StdinReader) -> StdinReader uses { process.stdin.read } {
    stdin_stream_next(reader)
}
@id("app.main") fn main() -> i64 { 0 }
"#, "stream-next-cleanup.spx").unwrap()).unwrap()
}

#[test]
fn stream_next_stages_before_status_and_commits_only_on_success() {
    use crate::cleanup_plan::{CleanupTerminator, CleanupTransition, EdgeCondition, StorageId};
    let program = stream_next_program();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "stream.advance")
        .unwrap();
    super::assert_expression_lowering_oracle(&program, function, &function.body);
    hir::validate(&program).unwrap();
    let ResolvedExprKind::Block { tail, .. } = &function.body.kind else {
        panic!()
    };
    let plan = &function.cleanup_plan;
    let success = plan
        .edges
        .iter()
        .find(|edge| {
            matches!(&edge.condition,
        EdgeCondition::StatusZero(source) if source.expression == tail.id)
        })
        .unwrap();
    let failed = plan
        .edges
        .iter()
        .find(|edge| {
            matches!(&edge.condition,
        EdgeCondition::StatusNonzero(source) if source.expression == tail.id)
        })
        .unwrap();
    assert_eq!(success.from, failed.from);
    let before = &plan.blocks[success.from.0 as usize];
    assert!(before
        .transitions
        .iter()
        .all(|transition| !matches!(transition,
        CleanupTransition::CallCommit { call, .. } if call == &tail.id)));
    let staged = before.transitions.iter().find_map(|transition| match transition {
        CleanupTransition::Transfer { destination, .. }
            if matches!(&destination.storage, StorageId::CallArgument { call, parameter_index: 0, .. } if call == &tail.id) => Some(destination),
        _ => None,
    }).unwrap();
    let committed = plan.blocks[success.to.0 as usize]
        .transitions
        .iter()
        .find_map(|transition| match transition {
            CleanupTransition::CallCommit { call, arguments } if call == &tail.id => {
                Some(arguments)
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(committed.len(), 1);
    assert_eq!(committed[0].parameter_index, 0);
    assert_eq!(&committed[0].source, staged);
    let CleanupTerminator::Exit(exit) = &plan.blocks[failed.to.0 as usize].terminator else {
        panic!()
    };
    let failed_exit = &plan.exits[exit.0 as usize];
    assert!(failed_exit
        .finalize_in_order
        .iter()
        .any(|action| &action.source == staged
            && action.lifecycle_id.as_str() == crate::stdin_stream_ops::DROP_ID));
    let mut hostile = program.clone();
    let function = hostile
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "stream.advance")
        .unwrap();
    for block in &mut function.cleanup_plan.blocks {
        for transition in &mut block.transitions {
            if let CleanupTransition::CallCommit { arguments, .. } = transition {
                arguments.clear();
            }
        }
    }
    assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");
}

#[test]
fn stream_next_rejects_nonowned_reader_and_legacy_owned_host_argument() {
    use crate::hir::{OwnershipMode, ResolvedHostCommandOperation};
    let program = stream_next_program();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "stream.advance")
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &function.body.kind else {
        panic!()
    };
    for old_host in [false, true] {
        let mut hostile = tail.as_ref().clone();
        let ResolvedExprKind::HostCommandCall(call) = &mut hostile.kind else {
            panic!()
        };
        let expected = if old_host {
            call.operation = ResolvedHostCommandOperation::StderrWrite;
            hostile.ty = ResolvedType::I64;
            hostile.ownership = OwnershipMode::Value;
            "cleanup plan: host-command operation received an owned argument"
        } else {
            call.args[0].ownership = OwnershipMode::Borrow;
            "cleanup plan: streaming advancement requires one exact owned reader"
        };
        for lower in [
            PlanBuilder::lower_expr_iterative,
            PlanBuilder::lower_expr_recursive_reference,
        ] {
            let mut builder = PlanBuilder::new(&program, function).unwrap();
            let state = builder.initial_state.clone();
            let error = lower(
                &mut builder,
                &hostile,
                BlockId(0),
                state,
                CleanupRegionId(0),
            )
            .unwrap_err();
            assert_eq!(error.code, "SPX-H006");
            assert_eq!(error.message, expected);
        }
    }
}
