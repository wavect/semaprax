//! Module cache bills exclude the actually acquired invocation-wide prelude.
//! Every hit reacquires that owner on the live ledger after full validation.
use super::*;

pub(in crate::workspace_graph) fn replay_cached_prelude(
    core: &mut crate::workspace_graph::expected_projection::CoreProviderProofs<'_>,
    synthetic: &Input<'_>,
) -> Result<(), ModuleFailure> {
    core.bind_base_owner();
    let mut owner = IdentityReservation::acquire().map_err(ModuleFailure::Setup)?;
    #[cfg(test)]
    observe_acquired(&owner);
    let result = core
        .base_prelude
        .ensure_selected(
            crate::prelude::declarations_for_program(synthetic.source()),
            &mut |bytes| owner.reserve(bytes),
        )
        .map(|_| ());
    result.map_err(|failure| {
        ModuleFailure::Report(match failure {
            InvocationFailure::Bytes(_) => owner.publish(),
            InvocationFailure::Work(_) => owner.publish_base_prelude_work(),
            InvocationFailure::ExtensionWork(_) => owner.publish_prelude_extension_work(),
            InvocationFailure::Producer(_) => owner.publish_base_prelude_producer(),
        })
    })
}

/// Describes only module-local work for a future replay. Actual shared debits
/// remain spent in the live graph ledger; this neither refunds nor reserves.
pub(in crate::workspace_graph) fn local_resolution_bytes(
    before: usize,
    shared_before: usize,
    core: &crate::workspace_graph::expected_projection::CoreProviderProofs<'_>,
) -> Result<usize, Vec<Diagnostic>> {
    before
        .checked_sub(
            crate::bounded_output::active_remaining().expect("resolved core has a builder budget"),
        )
        .and_then(|total| {
            core.base_prelude
                .selected_bytes()
                .checked_sub(shared_before)
                .and_then(|shared| total.checked_sub(shared))
        })
        .ok_or_else(|| {
            vec![crate::workspace_graph::graph_error(
                "SPX-G257",
                "checked module replay accounting exceeds its original resolver work",
            )]
        })
}

#[cfg(test)]
pub(in crate::workspace_graph) mod source_reference;
#[cfg(test)]
mod tests;
