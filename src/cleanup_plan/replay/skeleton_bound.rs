//! Per-visit charge bound for the cleanup-plan skeleton traversal.
//!
//! `branch_sensitive_cfg_bounds` multiplies this weight by each block's
//! incoming path count. Keep these charges paired with `plan_skeleton_paths`:
//! the base charge, transition clones/pushes, edge clones/pushes, and the
//! queued or terminal state each happen once per visit.
use super::*;
use crate::cleanup_plan::{CleanupBlock, CleanupPlan};

pub(super) fn plan_skeleton_block_weight(plan: &CleanupPlan, block: &CleanupBlock) -> usize {
    let mut weight = block.transitions.len().saturating_add(1);
    for transition in &block.transitions {
        let extra = match transition {
            CleanupTransition::Initialize { .. }
            | CleanupTransition::InitializeVariant { .. }
            | CleanupTransition::ReserveRenewal { .. } => 3,
            CleanupTransition::Transfer { .. }
            | CleanupTransition::Renew { .. }
            | CleanupTransition::TransferVariant { .. } => 4,
            CleanupTransition::CallCommit { arguments, .. } => {
                2usize.saturating_add(arguments.len().saturating_mul(2))
            }
            CleanupTransition::StageCopyResult { .. } => 2,
            CleanupTransition::AuthenticateVariantCase { .. }
            | CleanupTransition::SelectFailure { .. } => 0,
        };
        weight = weight.saturating_add(extra);
    }
    let edge_weight = |edge: EdgeId| match &plan.edges[edge.0 as usize].condition {
        EdgeCondition::VariantCase { .. } => 2,
        EdgeCondition::BooleanResult(..)
        | EdgeCondition::ArmSelected { .. }
        | EdgeCondition::StatusZero(..)
        | EdgeCondition::StatusNonzero(..) => 1,
        EdgeCondition::Always => 0,
    };
    let terminator = match &block.terminator {
        CleanupTerminator::Goto(edge) => {
            // Variant-case gotos append one observation; every goto enqueues.
            1 + if edge_weight(*edge) == 2 { 3 } else { 0 }
        }
        CleanupTerminator::Branch(edges) => edges.iter().fold(0usize, |sum, edge| {
            // Condition clones, history clone, observation push, queue push.
            sum.saturating_add(edge_weight(*edge) + 3)
        }),
        CleanupTerminator::Exit(_) => 1, // Continue queues; terminal exits push.
    };
    weight.saturating_add(terminator)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{hir, parse};
    use std::path::Path;

    fn program() -> ResolvedProgram {
        let source = r#"module replay.weight;
@id("weight.token") resource Token { @id("weight.drop") drop trivial; }
@id("weight.consume") fn consume(value: own Token) -> i64 { 0 }
@id("weight.choose") fn choose(flag: bool, value: own Token) -> i64 {
    if flag { consume(value) } else { 0 }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
        let parsed = parse(source, Path::new("replay-weight.spx")).unwrap();
        assert!(crate::verify::verify(&parsed).is_empty());
        hir::resolve(&parsed).unwrap()
    }

    fn charged_plan_work(function: &ResolvedFunction, bound: usize) -> usize {
        let mut budget = ReplayBudget::with_skeleton_limit(bound);
        reset_skeleton_materializations();
        let paths = plan_skeleton_paths(function, &mut budget).unwrap();
        assert!(paths.len() > 1);
        let charged = bound - budget.skeleton_remaining;
        assert!(skeleton_materializations() > 0);
        assert!(skeleton_materializations() <= charged);
        charged
    }

    #[test]
    fn per_transition_cfg_weight_covers_materialized_branch_and_call() {
        let program = program();
        let function = program
            .functions
            .iter()
            .find(|function| function.id.as_str() == "weight.choose")
            .unwrap();
        let cfg = branch_sensitive_cfg_bounds(function).unwrap();
        assert_eq!(
            charged_plan_work(function, cfg.skeleton_work),
            cfg.skeleton_work
        );
        // The former global factor charged at least ten for every work unit.
        assert!(cfg.skeleton_work < cfg.work * 10);
    }

    #[test]
    fn hostile_wide_call_is_counted_before_any_skeleton_materialization() {
        let mut program = program();
        let function = program
            .functions
            .iter_mut()
            .find(|function| function.id.as_str() == "weight.choose")
            .unwrap();
        let original = branch_sensitive_cfg_bounds(function).unwrap();
        let arguments = function
            .cleanup_plan
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.transitions)
            .find_map(|transition| match transition {
                CleanupTransition::CallCommit { arguments, .. } => Some(arguments),
                _ => None,
            })
            .unwrap();
        let repeated = arguments[0].clone();
        arguments.extend(std::iter::repeat_n(repeated, 64));
        let function = program
            .functions
            .iter()
            .find(|function| function.id.as_str() == "weight.choose")
            .unwrap();
        let widened = branch_sensitive_cfg_bounds(function).unwrap();
        assert_eq!(widened.skeleton_work, original.skeleton_work + 128);
        assert_eq!(
            charged_plan_work(function, widened.skeleton_work),
            widened.skeleton_work
        );

        let bound = skeleton_work_upper(&program, function).unwrap();
        reset_skeleton_materializations();
        let mut insufficient = ReplayBudget {
            remaining: bound - 1,
            skeleton_remaining: 0,
        };
        let error =
            reserve_program_skeleton_work(&program, std::iter::once(function), &mut insufficient)
                .unwrap_err();
        assert_eq!(error.code, "SPX-H006");
        assert!(error.message.contains("skeleton-work preflight exceeds"));
        assert_eq!(skeleton_materializations(), 0);
    }
}
