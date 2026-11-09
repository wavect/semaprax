//! Ordinary control-key cloning for one present, structurally atomic result type.
//! Compound types and every other payer retain the original complete receiver.
use super::*;

/// An immutable, affine proof of the exact selected staged result. No reference
/// to the PathState or its unselected live/renewal/variant containers escapes.
pub(in crate::cleanup_plan::replay::path_join) struct Payload<'a> {
    pending: &'a Option<StatusSourceId>,
    selected: &'a Option<StatusSourceId>,
    staged: &'a StagedCopyResultSource,
    published: bool,
    returned: &'a Option<ReturnedSource>,
}

fn controls_bytes() -> usize {
    // Producer/receiving views, outward Result, and returned key carrier.
    2 * size_of::<Payload<'_>>()
        + size_of::<Result<Payload<'_>, Diagnostic>>()
        + size_of::<ControlKey>()
}

pub(in crate::cleanup_plan::replay::path_join) fn selected(
    state: &PathState,
    budget: &ReplayBudget,
) -> bool {
    matches!(budget.payer, AllocationPayer::ActiveBuilder)
        && budget.admission == AdmissionMode::Ordinary
        && matches!(
            state.staged_copy_result.as_ref(),
            Some(StagedCopyResultSource::Body { instance, .. })
                if empty_renewals::clone_leaf(instance)
        )
}

pub(in crate::cleanup_plan::replay::path_join) fn reserve<'a>(
    function: &ResolvedFunction,
    state: &'a PathState,
    budget: &mut ReplayBudget,
) -> Result<Payload<'a>, Diagnostic> {
    if !selected(state, budget) {
        return Err(Diagnostic::io_static(
            "SPX-H006",
            "atomic control clone requires an ordinary builder and an atomic body result",
        ));
    }
    budget.reserve_bytes(function, controls_bytes(), "atomic control clone carriers")?;
    let bytes = state.returned.as_ref().map_or(Some(0), returned);
    budget.reserve_bytes(
        function,
        bytes.unwrap_or(usize::MAX),
        "ownership group control clone",
    )?;
    // Atomic result metadata has no child container: no census cursor exists
    // in this constructor. Source immutability keeps the selection true until
    // this borrowed payload is consumed. Returned projections are still paid.
    Ok(Payload {
        pending: &state.pending_failure,
        selected: &state.selected_failure,
        staged: state.staged_copy_result.as_ref().expect("selected result"),
        published: state.published,
        returned: &state.returned,
    })
}

pub(in crate::cleanup_plan::replay::path_join) fn copy(payload: Payload<'_>) -> ControlKey {
    (
        payload.pending.clone(),
        payload.selected.clone(),
        Some(payload.staged.clone()),
        payload.published,
        payload.returned.clone(),
    )
}

#[cfg(test)]
mod tests;
