//! Sequential profile body shared by scalar and aggregate-boundary lowering.

use super::*;

pub(super) fn lower(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    allow_aggregate_boundary: bool,
) -> Result<SequentialResumablePlan, Diagnostic> {
    let aggregate_bytes_channel = function.yields.as_ref().is_some_and(|yields| {
        crate::hir::yield_aggregate::has_bytes_leaf(&program.declarations, &yields.request_type)
    });
    let yields = control::check_resumable_profile(
        program,
        function,
        aggregate_bytes_channel,
        true,
        allow_aggregate_boundary,
    )?;
    reject_yield_in_contracts(function)?;
    let sites = locate_direct_yields(function)?;
    for (yield_expression, request, _) in &sites {
        if request.ty != yields.request_type
            || yield_expression.ty != yields.response_type
            || !matches!(request.ownership, OwnershipMode::Value | OwnershipMode::Own)
            || yield_expression.ownership != OwnershipMode::Value
        {
            return Err(invalid(
                "resumable yield request/response types or ownership disagree with its declaration",
            ));
        }
    }
    let mut admitted_boundary_types = vec![&yields.request_type, &yields.response_type];
    if allow_aggregate_boundary {
        admitted_boundary_types.push(&function.return_type);
        admitted_boundary_types.extend(function.params.iter().map(|parameter| &parameter.ty));
    }
    require_scalar_expression_tree(
        &function.body,
        aggregate_bytes_channel,
        Some(&admitted_boundary_types),
    )?;
    reject_reachable_resumable_callees(program, function)?;

    let identity = plan_identity(program, function, &sites)?;

    let entry = state(&function.id, ResumableStateKind::Entry, None);
    let complete = state(&function.id, ResumableStateKind::Complete, None);
    let start = start_projection(program, function, sites[0].1, sites[0].2)?;
    let mut suspensions = Vec::with_capacity(sites.len());
    let mut resumes = Vec::with_capacity(sites.len());
    for (index, (yield_expression, request, position)) in sites.iter().enumerate() {
        suspensions.push(ResumableSuspension {
            state: state(
                &function.id,
                ResumableStateKind::Suspended,
                Some(&yield_expression.id),
            ),
            expression: yield_expression.id.clone(),
            request_expression: request.id.clone(),
            position: *position,
            request_type: yields.request_type.clone(),
            response_type: yields.response_type.clone(),
        });
        resumes.push(ResumableProjection {
            function: resume_projection(program, function, &sites, index)?,
        });
    }

    Ok(SequentialResumablePlan {
        function_id: function.id.clone(),
        identity,
        entry,
        suspensions,
        complete,
        start: ResumableProjection { function: start },
        resumes,
    })
}
