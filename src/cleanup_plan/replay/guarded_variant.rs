//! Independent case and Boolean observations for guarded Copy variants.
use super::*;
#[allow(clippy::too_many_arguments)]
pub(super) fn finish_arm(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    scrutinee: &ResolvedExpr,
    arm: &ResolvedMatchArm,
    final_arm: bool,
    remaining: Vec<ExprSkeletonPath>,
    arm_paths: &[ExprSkeletonPath],
    results: &mut Vec<ExprSkeletonPath>,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    let ResolvedExprKind::Match { mode, .. } = &expression.kind else {
        unreachable!()
    };
    let guard = arm
        .guard
        .as_ref()
        .ok_or_else(|| replay_error(function, "guarded variant arm lost its guard"))?;
    if final_arm
        || !crate::variant_guards::admitted(
            &program.declarations,
            &scrutinee.ty,
            *mode,
            &arm.pattern,
            guard,
        )
        || guard.ty != ResolvedType::Bool
    {
        return Err(replay_error(
            function,
            "guarded variant has an invalid Boolean guard or fallback",
        ));
    }
    // Re-enter only the bounded guard subtree; every materialized prefix and
    // observation is charged by the same independent skeleton work budget.
    let guards = expression_skeleton(program, function, guard, work)?;
    let mut selected_paths = Vec::new();
    let mut rejected_paths = Vec::new();
    let cases = arm.pattern.variant_cases();
    for mut path in remaining {
        if path.failed || path.residual {
            work.push_expr_path(results, path, "variant terminal prefix")?;
            continue;
        }
        if matches!(arm.pattern, ResolvedMatchPattern::Wildcard) {
            work.push_expr_path(&mut selected_paths, path, "guarded wildcard prefix")?;
            continue;
        }
        let cases = cases.as_ref().ok_or_else(|| {
            replay_error(
                function,
                "guarded variant pattern has no authenticated cases",
            )
        })?;
        for case in cases {
            let mut selected = work.clone_expr_path(&path, "guarded variant selected prefix")?;
            let scrutinee_id = work.clone_owned(&scrutinee.id, "guarded variant scrutinee")?;
            let case_id = work.clone_owned(*case, "guarded variant case")?;
            work.push_observation(
                &mut selected,
                SkeletonObservation::VariantCase {
                    scrutinee: scrutinee_id,
                    case: case_id,
                    matches: true,
                },
                "guarded variant selected observation",
            )?;
            work.push_expr_path(
                &mut selected_paths,
                selected,
                "guarded variant selected path",
            )?;
            let scrutinee_id =
                work.clone_owned(&scrutinee.id, "guarded variant rejected scrutinee")?;
            let case_id = work.clone_owned(*case, "guarded variant rejected case")?;
            work.push_observation(
                &mut path,
                SkeletonObservation::VariantCase {
                    scrutinee: scrutinee_id,
                    case: case_id,
                    matches: false,
                },
                "guarded variant rejected observation",
            )?;
        }
        work.push_expr_path(&mut rejected_paths, path, "guarded variant rejected path")?;
    }
    let guarded = sequence_skeleton_paths(selected_paths, &guards, work)?;
    let (terminal, when_true, when_false) = split_boolean_prefixes(guarded, &guard.id, work)?;
    append_expr_paths(results, terminal, work, "variant guard terminal")?;
    append_expr_paths(&mut rejected_paths, when_false, work, "variant guard false")?;
    let selected = sequence_skeleton_paths(when_true, arm_paths, work)?;
    let selected = finish_owned(program, function, expression, &arm.value, selected, work)?;
    append_expr_paths(results, selected, work, "guarded variant result")?;
    Ok(rejected_paths)
}

/// Conservative materialization bound: all children sequence, with each
/// guard's Boolean split and every possible selected arm counted separately.
pub(super) fn census(arms: &[ResolvedMatchArm], counts: HirPathCounts) -> HirPathCounts {
    let factor = arms.iter().filter(|arm| arm.guard.is_some()).fold(
        arms.iter()
            .fold(0usize, |sum, arm| {
                sum.saturating_add(arm.pattern.variant_cases().map_or(1, |cases| cases.len()))
            })
            .max(1),
        |factor, _| factor.saturating_mul(2),
    );
    HirPathCounts {
        normal: counts.normal.saturating_mul(factor),
        failed: counts.failed.saturating_mul(factor),
        residual: counts.residual.saturating_mul(factor),
    }
}

pub(super) fn selected(expression: &ResolvedExpr) -> bool {
    matches!(&expression.kind, ResolvedExprKind::Match { scrutinee, arms, .. } if !crate::hir::is_refutable_match_scalar(&scrutinee.ty) && arms.iter().any(|arm| arm.guard.is_some()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    const SOURCE: &str = r#"
module test.general_guard_replay;
@id("g.choice") variant Choice { @id("g.a") A, @id("g.b") B, }
@id("g.positive") fn positive(value:i64)->bool {value>=0}
@id("g.main") fn main()->i64 {
 let choice=Choice::A {}; let held="held"; let mut count=0;
 while count<2 {
  let number=match choice {
   Choice::A {} | Choice::B {} if string_len(string_concat("a","b"))==2 && positive(count) => 1,
   _ if { let temp="x"; string_len(temp)==99 } => 9,
   _ => 0,
  };
  count=count+number; 0
 } count+string_len(held)
}
"#;
    #[test]
    fn general_variant_guards_builder_and_independent_replay_agree() {
        let program = crate::hir::resolve(
            &crate::parse(SOURCE, Path::new("general-guard-replay.spx")).unwrap(),
        )
        .unwrap();
        for function in &program.functions {
            validate_structure(&program, function).unwrap();
            crate::cleanup_plan::build::assert_expression_lowering_oracle(
                &program,
                function,
                &function.body,
            );
        }
    }
    #[test]
    fn general_variant_guards_replay_rejects_modified_boolean_and_scope_exit() {
        let program = crate::hir::resolve(
            &crate::parse(SOURCE, Path::new("general-guard-hostile.spx")).unwrap(),
        )
        .unwrap();
        let original = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "g.main")
            .unwrap();
        validate_structure(&program, original).unwrap();
        for mutation in 0..3 {
            let mut function = original.clone();
            match mutation {
                0 => {
                    let edge = function
                        .cleanup_plan
                        .edges
                        .iter_mut()
                        .find(|e| matches!(e.condition, EdgeCondition::BooleanResult(_, false)))
                        .unwrap();
                    let EdgeCondition::BooleanResult(_, value) = &mut edge.condition else {
                        unreachable!()
                    };
                    *value = true;
                }
                1 => {
                    let exit = function
                        .cleanup_plan
                        .exits
                        .iter_mut()
                        .find(|e| !e.finalize_in_order.is_empty())
                        .unwrap();
                    exit.finalize_in_order.clear();
                }
                2 => {
                    let region = function
                        .cleanup_plan
                        .regions
                        .iter_mut()
                        .find(|r| r.parent.is_some() && !r.slots.is_empty())
                        .unwrap();
                    region.slots.clear();
                }
                _ => unreachable!(),
            }
            assert_eq!(
                validate_structure(&program, &function).unwrap_err().code,
                "SPX-H006",
                "mutation {mutation}"
            );
        }
    }
}
