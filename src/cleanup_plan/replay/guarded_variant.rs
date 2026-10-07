//! Independent case and Boolean observations for guarded Copy variants.
use super::*;
#[allow(clippy::too_many_arguments)]
pub(super) fn finish(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    scrutinee: &ResolvedExpr,
    arms: &[ResolvedMatchArm],
    mut remaining: Vec<ExprSkeletonPath>,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    let ResolvedExprKind::Match { mode, .. } = &expression.kind else {
        unreachable!()
    };
    if *mode != crate::hir::ResolvedMatchMode::Value
        || !crate::variant_guards::copy_variant(&program.declarations, &scrutinee.ty)
    {
        return Err(replay_error(
            function,
            "guarded variant skeleton needs Copy scalar payloads",
        ));
    }
    let mut results = Vec::new();
    for (index, arm) in arms.iter().enumerate() {
        let mut selected_paths = Vec::new();
        let mut rejected_paths = Vec::new();
        for mut path in remaining {
            if path.failed || path.residual {
                work.push_expr_path(&mut results, path, "variant terminal prefix")?;
                continue;
            }
            if index + 1 == arms.len() {
                work.push_expr_path(&mut selected_paths, path, "variant final prefix")?;
                continue;
            }
            let cases = arm.pattern.variant_cases().ok_or_else(|| {
                replay_error(function, "guarded variant chain has a nonfinal wildcard")
            })?;
            for case in cases {
                let mut selected =
                    work.clone_expr_path(&path, "guarded variant selected prefix")?;
                let scrutinee_id = work.clone_owned(&scrutinee.id, "guarded variant scrutinee")?;
                let case_id = work.clone_owned(case, "guarded variant case")?;
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
                let case_id = work.clone_owned(case, "guarded variant rejected case")?;
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
        if let Some(guard) = &arm.guard {
            if index + 1 == arms.len()
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
                    "guarded variant has an invalid scalar guard or fallback",
                ));
            }
            let guards = expression_skeleton(program, function, guard, work)?;
            let guarded = sequence_skeleton_paths(selected_paths, &guards, work)?;
            let (terminal, when_true, when_false) =
                split_boolean_prefixes(guarded, &guard.id, work)?;
            append_expr_paths(&mut results, terminal, work, "variant guard terminal")?;
            append_expr_paths(&mut rejected_paths, when_false, work, "variant guard false")?;
            selected_paths = when_true;
        }
        let values = expression_skeleton(program, function, &arm.value, work)?;
        let selected = sequence_skeleton_paths(selected_paths, &values, work)?;
        let selected = finish_owned(program, function, expression, &arm.value, selected, work)?;
        append_expr_paths(&mut results, selected, work, "guarded variant result")?;
        remaining = rejected_paths;
    }
    append_expr_paths(&mut results, remaining, work, "guarded variant remaining")?;
    Ok(results)
}

/// Conservative materialization bound: all children sequence, with each
/// guard's Boolean split and every possible selected arm counted separately.
pub(super) fn census(arms: &[ResolvedMatchArm], counts: HirPathCounts) -> HirPathCounts {
    let factor = arms
        .iter()
        .filter(|arm| arm.guard.is_some())
        .fold(arms.len().max(1), |factor, _| factor.saturating_mul(2));
    HirPathCounts {
        normal: counts.normal.saturating_mul(factor),
        failed: counts.failed.saturating_mul(factor),
        residual: counts.residual.saturating_mul(factor),
    }
}

pub(super) fn selected(expression: &ResolvedExpr) -> bool {
    matches!(&expression.kind, ResolvedExprKind::Match { scrutinee, arms, .. } if !crate::hir::is_refutable_match_scalar(&scrutinee.ty) && arms.iter().any(|arm| arm.guard.is_some()))
}
