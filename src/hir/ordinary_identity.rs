//! Ordinary analyzed identity construction on the actual caller's ledger.
//! Only identity backing and its affine refusal report are paid here. Source
//! verification, index maps/names/types, invariant transforms and semantic
//! diagnostics retain their existing owners; this is not whole-resolver funding.
use super::resolve_program::agent_identity::AgentIdentityFailure;
use super::*;
use crate::bounded_output::identity_reservation::{IdentityReservation, Refused};
use declaration_index::identity_payer::IdentityIndexError;

pub(super) fn can_acquire() -> bool {
    IdentityReservation::setup_bytes()
        .zip(crate::bounded_output::active_remaining())
        .is_some_and(|(required, remaining)| remaining >= required)
}

/// The caller has verified this exact immutable Program. Initial acquisition
/// fails inline; later identity refusal moves the owner's prepaid report.
/// The public setup-only compatibility wrapper is deliberately kept outside.
#[cfg(test)]
pub(super) fn try_resolve_verified(
    program: &Program,
) -> Result<Result<(ResolvedProgram, FunctionResolutionWork), Vec<Diagnostic>>, Refused> {
    try_resolve_verified_observed(program, |_| {})
}

pub(super) fn try_resolve_verified_compatible(
    program: &Program,
) -> Result<Result<(ResolvedProgram, FunctionResolutionWork), Vec<Diagnostic>>, Refused> {
    resolve_with_acquisition(program, true, IdentityReservation::acquire, |_| {})
}

#[cfg(test)]
fn try_resolve_verified_observed(
    program: &Program,
    observe: impl FnOnce(&IdentityReservation),
) -> Result<Result<(ResolvedProgram, FunctionResolutionWork), Vec<Diagnostic>>, Refused> {
    // The strict private constructor owns setup refusal before building any
    // inventory. The public compatibility receiver below keeps the complete
    // original inventory-first order and legacy setup-refusal channel.
    let owner = IdentityReservation::acquire()?;
    resolve_with_acquisition(program, false, || Ok(owner), observe)
}

fn resolve_with_acquisition(
    program: &Program,
    setup_compatibility: bool,
    acquire: impl FnOnce() -> Result<IdentityReservation, Refused>,
    observe: impl FnOnce(&IdentityReservation),
) -> Result<Result<(ResolvedProgram, FunctionResolutionWork), Vec<Diagnostic>>, Refused> {
    // Match the original inventory and field-evaluation order before the first
    // identity request: clause program, SourceIndex, then compiler prelude.
    let clause_program = record_invariants::clause_program(program);
    let inventory = clause_program.as_ref().unwrap_or(program);
    let helpers = crate::loop_calls::byte_writer::SourceIndex::new(inventory);
    let prelude = crate::prelude::declarations_for_program(inventory);
    let mut owner = match acquire() {
        Ok(owner) => owner,
        Err(_) if setup_compatibility => {
            // Reuse this exact inventory and SourceIndex. Re-running the outer
            // resolver would clone/synthesize the inventory on a refused ledger.
            return Ok(legacy_inventory(program, inventory, helpers, prelude));
        }
        Err(error) => return Err(error),
    };
    observe(&owner);
    let resolved = match pass(inventory, helpers, prelude, &mut owner) {
        Ok(resolved) => resolved,
        Err(PassFailure::Identity) => return Ok(Err(owner.publish())),
        Err(PassFailure::Semantic(error)) => return Ok(Err(vec![error])),
    };
    let Some(desugared) = record_invariants::desugar(program, &resolved.0) else {
        return Ok(Ok(resolved));
    };
    let helpers = crate::loop_calls::byte_writer::SourceIndex::new(&desugared);
    let prelude = crate::prelude::declarations_for_program(&desugared);
    Ok(match pass(&desugared, helpers, prelude, &mut owner) {
        Ok(resolved) => Ok(resolved),
        Err(PassFailure::Identity) => Err(owner.publish()),
        Err(PassFailure::Semantic(error)) => Err(vec![error]),
    })
}

fn legacy_inventory<'a>(
    program: &Program,
    inventory: &'a Program,
    helpers: crate::loop_calls::byte_writer::SourceIndex<'a>,
    prelude: &[crate::ast::TypeDeclaration],
) -> Result<(ResolvedProgram, FunctionResolutionWork), Vec<Diagnostic>> {
    let resolved = legacy_pass(inventory, helpers, prelude).map_err(|error| vec![error])?;
    let Some(desugared) = record_invariants::desugar(program, &resolved.0) else {
        return Ok(resolved);
    };
    let helpers = crate::loop_calls::byte_writer::SourceIndex::new(&desugared);
    let prelude = crate::prelude::declarations_for_program(&desugared);
    legacy_pass(&desugared, helpers, prelude).map_err(|error| vec![error])
}
fn legacy_pass<'a>(
    program: &'a Program,
    helpers: crate::loop_calls::byte_writer::SourceIndex<'a>,
    prelude: &[crate::ast::TypeDeclaration],
) -> Result<(ResolvedProgram, FunctionResolutionWork), Diagnostic> {
    Resolver {
        function_cells: std::borrow::Cow::Owned(crate::source_verify::function_cells(program)),
        program,
        byte_helpers: helpers,
        declarations: DeclarationIndex::from_verified_with_prelude(program, prelude)?,
        reuse: None,
        function_work: FunctionResolutionWork::default(),
        fused_producer: None,
        partition_profile: false,
    }
    .resolve()
}

enum PassFailure {
    Semantic(Diagnostic),
    Identity,
}
fn pass<'a>(
    program: &'a Program,
    helpers: crate::loop_calls::byte_writer::SourceIndex<'a>,
    prelude: &[crate::ast::TypeDeclaration],
    owner: &mut IdentityReservation,
) -> Result<(ResolvedProgram, FunctionResolutionWork), PassFailure> {
    let declarations =
        DeclarationIndex::from_verified_with_identity_reservation(program, prelude, |bytes| {
            owner.reserve(bytes)
        })
        .map_err(|error| match error {
            IdentityIndexError::Semantic(error) => PassFailure::Semantic(error),
            IdentityIndexError::Identity(_) => PassFailure::Identity,
        })?;
    Resolver {
        function_cells: std::borrow::Cow::Owned(crate::source_verify::function_cells(program)),
        program,
        byte_helpers: helpers,
        declarations,
        reuse: None,
        function_work: FunctionResolutionWork::default(),
        fused_producer: None,
        partition_profile: false,
    }
    .resolve_with_agent_identities(|text| {
        DeclarationId::paid_borrowed(text, |bytes| owner.reserve(bytes))
    })
    .map_err(|error| match error {
        AgentIdentityFailure::Semantic(error) => PassFailure::Semantic(error),
        AgentIdentityFailure::Identity(_) => PassFailure::Identity,
    })
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod agent_tests;
