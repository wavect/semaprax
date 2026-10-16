//! Complete original lexical marker producer, bound to the original observation enum.
use super::*;
use crate::cleanup_plan::ExitTargetId;

pub(super) fn plan_exit_observation(
    budget: &mut ReplayBudget,
    function: &ResolvedFunction,
    exit: ExitTargetId,
) -> Result<Option<SkeletonObservation>, Diagnostic> {
    let Some(control) = function.cleanup_plan.lexical_control.as_deref() else {
        return Ok(None);
    };
    let Some(certificate) = control
        .exits
        .iter()
        .find(|certificate| certificate.exit == exit)
    else {
        return Ok(None);
    };
    Ok(Some(SkeletonObservation::LexicalExit {
        site: skeleton_clone(budget, function, &certificate.site, "plan exit site clone")?,
        kind: certificate.kind,
        target: skeleton_clone(
            budget,
            function,
            &certificate.target,
            "plan exit target clone",
        )?,
    }))
}
