//! Ordinary builder branch clones reconstruct an empty renewal map directly.
//! The original clone and census remain complete for every other route.
use super::*;

/// The receiving constructor cannot clone the original renewal map: this
/// affine borrowed carrier contains only the other seven state fields.
pub(in crate::cleanup_plan::replay::path_join) struct Payload<'a> {
    live_order: &'a Vec<LivenessFlagId>,
    conditional_variants: &'a Vec<ReplayConditionalVariant>,
    pending_failure: &'a Option<StatusSourceId>,
    selected_failure: &'a Option<StatusSourceId>,
    staged_copy_result: &'a Option<StagedCopyResultSource>,
    published: bool,
    returned: &'a Option<ReturnedSource>,
}

fn controls_bytes() -> usize {
    // Producer view, outward Result and receiving view are separately priced.
    2 * size_of::<Payload<'_>>() + size_of::<Result<Payload<'_>, Diagnostic>>()
}

fn clone_leaf(ty: &ResolvedType) -> bool {
    matches!(
        ty,
        ResolvedType::Never
            | ResolvedType::Unit
            | ResolvedType::I64
            | ResolvedType::I32
            | ResolvedType::Char
            | ResolvedType::U8
            | ResolvedType::Usize
            | ResolvedType::F32
            | ResolvedType::F64
            | ResolvedType::Bool
            | ResolvedType::String
            | ResolvedType::ArrayU8(_)
            | ResolvedType::Bytes
            | ResolvedType::Str
            | ResolvedType::SliceU8
            | ResolvedType::TypeParameter { .. }
            | ResolvedType::OnceFunction
            | ResolvedType::OnceFunctionI64
            | ResolvedType::OnceFunctionI64Pair
            | ResolvedType::MutFunctionI64
    )
}

fn needs_type_scratch(source: &StagedCopyResultSource) -> bool {
    match source {
        StagedCopyResultSource::Body { instance, .. } => !clone_leaf(instance),
        StagedCopyResultSource::TryResidual {
            source_instance,
            target_instance,
            ..
        }
        | StagedCopyResultSource::TryOptionNone {
            source_instance,
            target_instance,
            ..
        } => !clone_leaf(source_instance) || !clone_leaf(target_instance),
    }
}

fn staged_bytes(source: &StagedCopyResultSource) -> Option<usize> {
    if needs_type_scratch(source) {
        staged(source)
    } else {
        Some(0)
    }
}

pub(in crate::cleanup_plan::replay::path_join) fn selected(
    state: &PathState,
    budget: &ReplayBudget,
) -> bool {
    matches!(budget.payer, AllocationPayer::ActiveBuilder)
        && budget.admission == AdmissionMode::Ordinary
        && state.renewals.is_empty()
}

fn payload_bytes(state: &PathState) -> Option<usize> {
    if !state.renewals.is_empty() {
        return None;
    }
    let mut bytes = array::<LivenessFlagId>(state.live_order.len())?.checked_add(array::<
        ReplayConditionalVariant,
    >(
        state.conditional_variants.len(),
    )?)?;
    for value in &state.conditional_variants {
        bytes = bytes.checked_add(variant(value)?)?;
    }
    if let Some(value) = &state.staged_copy_result {
        bytes = bytes.checked_add(staged_bytes(value)?)?;
    }
    if let Some(value) = &state.returned {
        bytes = bytes.checked_add(returned(value)?)?;
    }
    Some(bytes)
}

pub(in crate::cleanup_plan::replay::path_join) fn reserve<'a>(
    function: &ResolvedFunction,
    state: &'a PathState,
    budget: &mut ReplayBudget,
    phase: &str,
) -> Result<Payload<'a>, Diagnostic> {
    if !selected(state, budget) {
        return Err(Diagnostic::io_static(
            "SPX-H006",
            "empty renewal clone requires an ordinary builder and an empty map",
        ));
    }
    budget.reserve_bytes(function, controls_bytes(), phase)?;
    // Atomic type tags have no child containers, so no cursor is constructed.
    // Compound types keep the complete original depth/work census and scratch.
    if state
        .staged_copy_result
        .as_ref()
        .is_some_and(needs_type_scratch)
    {
        budget.reserve_array::<Option<(&ResolvedType, usize)>>(
            function,
            crate::cache_codec::MAX_DEPTH + 1,
            "clone type scratch",
        )?;
    }
    budget.reserve_bytes(function, payload_bytes(state).unwrap_or(usize::MAX), phase)?;
    Ok(Payload {
        live_order: &state.live_order,
        conditional_variants: &state.conditional_variants,
        pending_failure: &state.pending_failure,
        selected_failure: &state.selected_failure,
        staged_copy_result: &state.staged_copy_result,
        published: state.published,
        returned: &state.returned,
    })
}

pub(in crate::cleanup_plan::replay::path_join) fn copy(payload: Payload<'_>) -> PathState {
    PathState {
        renewals: BTreeMap::new(),
        live_order: payload.live_order.clone(),
        conditional_variants: payload.conditional_variants.clone(),
        pending_failure: payload.pending_failure.clone(),
        selected_failure: payload.selected_failure.clone(),
        staged_copy_result: payload.staged_copy_result.clone(),
        published: payload.published,
        returned: payload.returned.clone(),
    }
}

#[cfg(test)]
mod tests;
