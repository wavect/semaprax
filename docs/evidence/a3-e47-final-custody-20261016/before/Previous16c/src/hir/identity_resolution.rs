//! Explicit identity-only payment through the original two resolution passes.
//! Source and semantic errors remain owned compatibility diagnostics. This
//! private route changes neither verifier authority nor selected admission.
use super::resolve_program::agent_identity::AgentIdentityFailure;
use super::*;
use declaration_index::identity_payer::IdentityIndexError;

#[derive(Debug)]
pub(crate) enum Failure<E> {
    Source(Vec<Diagnostic>),
    Semantic(Diagnostic),
    Identity(E),
}
impl<E> From<IdentityIndexError<E>> for Failure<E> {
    fn from(error: IdentityIndexError<E>) -> Self {
        match error {
            IdentityIndexError::Semantic(error) => Self::Semantic(error),
            IdentityIndexError::Identity(error) => Self::Identity(error),
        }
    }
}
impl<E> From<AgentIdentityFailure<E>> for Failure<E> {
    fn from(error: AgentIdentityFailure<E>) -> Self {
        match error {
            AgentIdentityFailure::Semantic(error) => Self::Semantic(error),
            AgentIdentityFailure::Identity(error) => Self::Identity(error),
        }
    }
}

/// One explicit callback survives both invariant passes. It cannot replace
/// IDs or authorize a source shape; full ordinary verification runs first.
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "private identity-only caller migration; ordinary API remains legacy"
    )
)]
pub(crate) fn resolve<E>(
    program: &Program,
    private: Option<source_verify::private_names::PrivateNames<'_>>,
    mut reserve: impl FnMut(usize) -> Result<(), E>,
) -> Result<(ResolvedProgram, FunctionResolutionWork), Failure<E>> {
    let diagnostics = source_verify::verify_presenting(program, private);
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity.is_error())
    {
        return Err(Failure::Source(diagnostics));
    }
    resolve_verified_paid(program, None, &mut reserve)
}

/// Paid sibling of `resolve_with_function_reuse`: identical environment
/// equality and reuse selection, identical verification-before-index order.
#[cfg(test)]
pub(crate) fn resolve_checked<'p, E>(
    program: &'p Program,
    previous_program: Option<&Program>,
    previous_resolved: Option<&ResolvedProgram>,
    previous_costs: Option<&BTreeMap<String, usize>>,
    private: Option<source_verify::private_names::PrivateNames<'p>>,
    mut reserve: impl FnMut(usize) -> Result<(), E>,
) -> Result<(ResolvedProgram, FunctionResolutionWork), Failure<E>> {
    #[cfg(feature = "unstable-workflow-profiling")]
    let _workflow_span = crate::workflow_profile::span(crate::workflow_profile::Stage::Resolve);
    let diagnostics = source_verify::verify_presenting(program, private);
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity.is_error())
    {
        return Err(Failure::Source(diagnostics));
    }
    let reuse = previous_program
        .zip(previous_resolved)
        .zip(previous_costs)
        .filter(|((previous, _), _)| function_reuse_environment_is_exact(program, previous))
        .map(|((previous, resolved), costs)| FunctionReuse {
            current: program,
            program: previous,
            resolved,
            costs,
        });
    resolve_verified_paid(program, reuse, &mut reserve)
}

#[cfg(test)]
pub(crate) fn resolve_source<'p, E>(
    program: &'p dyn ProgramSource,
    private: Option<source_verify::private_names::PrivateNames<'p>>,
    reserve: impl FnMut(usize) -> Result<(), E>,
) -> Result<(ResolvedProgram, FunctionResolutionWork), Failure<E>> {
    resolve_source_checked(program, None, None, None, private, reserve)
}

/// Paid sibling of `resolve_source`: full verification and its explicit
/// invariant inventory/desugaring preserve ordinary identity request order.
#[cfg(test)]
pub(crate) fn resolve_source_checked<'p, E>(
    program: &'p dyn ProgramSource,
    previous_program: Option<&Program>,
    previous_resolved: Option<&ResolvedProgram>,
    previous_costs: Option<&BTreeMap<String, usize>>,
    private: Option<source_verify::private_names::PrivateNames<'p>>,
    mut reserve: impl FnMut(usize) -> Result<(), E>,
) -> Result<(ResolvedProgram, FunctionResolutionWork), Failure<E>> {
    #[cfg(feature = "unstable-workflow-profiling")]
    let _workflow_span = crate::workflow_profile::span(crate::workflow_profile::Stage::Resolve);
    let diagnostics = source_verify::verify_presenting(program, private);
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity.is_error())
    {
        return Err(Failure::Source(diagnostics));
    }
    let reuse = previous_program
        .zip(previous_resolved)
        .zip(previous_costs)
        .filter(|((previous, _), _)| function_reuse_environment_is_exact(program, previous))
        .map(|((previous, resolved), costs)| FunctionReuse {
            current: program,
            program: previous,
            resolved,
            costs,
        });
    let clause_program = record_invariants::source::clause_program(program);
    let inventory: &dyn ProgramSource = clause_program.as_ref().map_or(program, |owned| owned);
    let resolved = source_pass(
        inventory,
        if clause_program.is_some() {
            None
        } else {
            reuse
        },
        &mut reserve,
    )?;
    let Some(desugared) = record_invariants::source::desugar(program, &resolved.0) else {
        return Ok(resolved);
    };
    pass(&desugared, None, &mut reserve)
}

#[cfg(test)]
fn source_pass<E>(
    program: &dyn ProgramSource,
    reuse: Option<FunctionReuse<'_>>,
    reserve: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<(ResolvedProgram, FunctionResolutionWork), Failure<E>> {
    // SourceIndex precedes the DeclarationIndex, as in the legacy route.
    let byte_helpers = crate::loop_calls::byte_writer::SourceIndex::new(program);
    let declarations = DeclarationIndex::from_verified_with_identity_reservation(
        program,
        crate::prelude::declarations_for_program(program),
        &mut *reserve,
    )?;
    (Resolver {
        program,
        byte_helpers,
        declarations,
        reuse,
        function_work: FunctionResolutionWork::default(),
        fused_producer: None,
        partition_profile: false,
    })
    .resolve_with_agent_identities(|text| DeclarationId::paid_borrowed(text, &mut *reserve))
    .map_err(Failure::from)
}

/// Both invariant passes share one callback. The inventory pass suppresses
/// reuse exactly when a clause program exists; the desugared pass never reuses.
fn resolve_verified_paid<E>(
    program: &Program,
    reuse: Option<FunctionReuse<'_>>,
    reserve: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<(ResolvedProgram, FunctionResolutionWork), Failure<E>> {
    let clause_program = record_invariants::clause_program(program);
    let inventory_program = clause_program.as_ref().unwrap_or(program);
    let reuse = if clause_program.is_some() {
        None
    } else {
        reuse
    };
    let resolved = pass(inventory_program, reuse, reserve)?;
    let Some(desugared) = record_invariants::desugar(program, &resolved.0) else {
        return Ok(resolved);
    };
    pass(&desugared, None, reserve)
}

fn pass<E>(
    program: &Program,
    reuse: Option<FunctionReuse<'_>>,
    reserve: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<(ResolvedProgram, FunctionResolutionWork), Failure<E>> {
    // Preserve the old Resolver field-evaluation order before index creation.
    let byte_helpers = crate::loop_calls::byte_writer::SourceIndex::new(program);
    let declarations = DeclarationIndex::from_verified_with_identity_reservation(
        program,
        crate::prelude::declarations_for_program(program),
        &mut *reserve,
    )?;
    (Resolver {
        program,
        byte_helpers,
        declarations,
        reuse,
        function_work: FunctionResolutionWork::default(),
        fused_producer: None,
        partition_profile: false,
    })
    .resolve_with_agent_identities(|text| DeclarationId::paid_borrowed(text, &mut *reserve))
    .map_err(Failure::from)
}

/// Actual caller acquires the fixed report BEFORE invoking either pass.
/// Setup is Copy-only; owned Source/Semantic errors are explicitly unfunded.
#[derive(Debug)]
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "private identity-only outward route remains inactive"
    )
)]
pub(crate) enum CallerFailure {
    #[allow(
        dead_code,
        reason = "retained original receiver or refusal payload; not every legacy entry is selected by current tests"
    )]
    Setup(crate::bounded_output::identity_reservation::Refused),
    #[allow(
        dead_code,
        reason = "retained original receiver or refusal payload; not every legacy entry is selected by current tests"
    )]
    Legacy(Failure<std::convert::Infallible>),
    Identity(Vec<Diagnostic>),
}
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "private identity-only caller migration; ordinary API remains legacy"
    )
)]
pub(crate) fn resolve_with_report(
    program: &Program,
    private: Option<source_verify::private_names::PrivateNames<'_>>,
) -> Result<(ResolvedProgram, FunctionResolutionWork), CallerFailure> {
    resolve_with_report_observation(program, private, |_| {})
}
fn resolve_with_report_observation(
    program: &Program,
    private: Option<source_verify::private_names::PrivateNames<'_>>,
    observe: impl FnOnce(&crate::bounded_output::identity_reservation::IdentityReservation),
) -> Result<(ResolvedProgram, FunctionResolutionWork), CallerFailure> {
    let mut owner = crate::bounded_output::identity_reservation::IdentityReservation::acquire()
        .map_err(CallerFailure::Setup)?;
    observe(&owner);
    match resolve(program, private, |bytes| owner.reserve(bytes)) {
        Ok(value) => Ok(value),
        Err(Failure::Identity(_)) => Err(CallerFailure::Identity(owner.publish())),
        Err(Failure::Source(error)) => Err(CallerFailure::Legacy(Failure::Source(error))),
        Err(Failure::Semantic(error)) => Err(CallerFailure::Legacy(Failure::Semantic(error))),
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod agent_tests;

#[cfg(test)]
mod previous_source;
#[cfg(test)]
pub(crate) use previous_source::resolve_source_checked_previous;

mod nominal;
pub(crate) use nominal::{
    resolve_checked_nominal, resolve_source_checked_previous_nominal, resolve_source_nominal,
};

#[cfg(test)]
mod nominal_tests;

pub(crate) use nominal::{resolve_source_checked_previous_with_base, NominalReceiverFailure};

#[cfg(test)]
pub(crate) mod nominal_original;

#[cfg(test)]
mod nominal_invalid_layout_tests;

// Independently instantiated complete immediate-old resolver receiver.
#[cfg(test)]
#[path = "identity_resolution/nominal.rs"]
#[allow(
    clippy::duplicate_mod,
    reason = "Independently compile the complete immediate-old resolver receiver."
)]
pub(crate) mod before_function_alias;
