//! Same complete source/reuse/invariant passes, with nominal payload bytes
//! acquired on the existing explicit caller reservation. No new budget, source
//! authority, invariant omission, or replay-work credit is introduced.
use super::super::function_reuse_source::environment_is_exact as source_environment_is_exact;
use super::*;
pub(crate) fn resolve_checked_nominal<'p, E>(
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
    resolve_verified_paid_nominal(program, reuse, &mut reserve)
}

fn resolve_verified_paid_nominal<E>(
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
    let resolved = pass_nominal(inventory_program, reuse, None, None, reserve)?;
    let Some(desugared) = record_invariants::desugar(program, &resolved.0) else {
        return Ok(resolved);
    };
    pass_nominal(&desugared, None, None, None, reserve)
}

fn source_pass_nominal<E>(
    program: &dyn ProgramSource,
    reuse: Option<FunctionReuse<'_>>,
    cells: Option<&[crate::ast::program_source::FunctionCell<'_>]>,
    base: Option<&super::super::prelude_bundle::BasePreludeBundle>,
    vector: Option<&super::super::prelude_bundle::VectorExtension>,
    reserve: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<(ResolvedProgram, FunctionResolutionWork), Failure<E>> {
    // SourceIndex precedes the DeclarationIndex, as in the legacy route.
    let byte_helpers = crate::loop_calls::byte_writer::SourceIndex::new(program);
    // The optional frozen-inventory operand and callee parameter are new
    // receivers; acquire them before forwarding the already owned cells.
    reserve(
        std::mem::size_of::<Option<&[crate::ast::program_source::FunctionCell<'_>]>>()
            + std::mem::size_of::<Option<&[crate::ast::program_source::FunctionCell<'_>]>>(),
    )
    .map_err(Failure::Identity)?;
    let declarations = DeclarationIndex::from_verified_with_nominal_cells(
        program,
        crate::prelude::declarations_for_program(program),
        base,
        vector,
        cells,
        &mut *reserve,
    )?;
    let function_cells = match cells {
        Some(cells) => std::borrow::Cow::Borrowed(cells),
        None => std::borrow::Cow::Owned(crate::source_verify::function_cells(program)),
    };
    (Resolver {
        function_cells,
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

fn pass_nominal<E>(
    program: &Program,
    reuse: Option<FunctionReuse<'_>>,
    base: Option<&super::super::prelude_bundle::BasePreludeBundle>,
    vector: Option<&super::super::prelude_bundle::VectorExtension>,
    reserve: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<(ResolvedProgram, FunctionResolutionWork), Failure<E>> {
    // Preserve the old Resolver field-evaluation order before index creation.
    let byte_helpers = crate::loop_calls::byte_writer::SourceIndex::new(program);
    let declarations = DeclarationIndex::from_verified_with_nominal_extensions(
        program,
        crate::prelude::declarations_for_program(program),
        base,
        vector,
        &mut *reserve,
    )?;
    (Resolver {
        function_cells: std::borrow::Cow::Owned(crate::source_verify::function_cells(program)),
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

pub(crate) fn resolve_source_checked_previous_nominal<'p, E>(
    program: &'p dyn ProgramSource,
    previous_program: Option<&dyn ProgramSource>,
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
        .filter(|((previous, _), _)| source_environment_is_exact(program, *previous))
        .map(|((previous, resolved), costs)| FunctionReuse {
            current: program,
            program: previous,
            resolved,
            costs,
        });
    let clause_program = record_invariants::source::clause_program(program);
    let inventory: &dyn ProgramSource = clause_program.as_ref().map_or(program, |owned| owned);
    let resolved = source_pass_nominal(
        inventory,
        if clause_program.is_some() {
            None
        } else {
            reuse
        },
        None,
        None,
        None,
        &mut reserve,
    )?;
    let Some(desugared) = record_invariants::source::desugar(program, &resolved.0) else {
        return Ok(resolved);
    };
    pass_nominal(&desugared, None, None, None, &mut reserve)
}

pub(crate) fn resolve_source_nominal<'p, E>(
    program: &'p dyn ProgramSource,
    private: Option<source_verify::private_names::PrivateNames<'p>>,
    reserve: impl FnMut(usize) -> Result<(), E>,
) -> Result<(ResolvedProgram, FunctionResolutionWork), Failure<E>> {
    resolve_source_checked_previous_nominal(program, None, None, None, private, reserve)
}

#[derive(Debug)]
pub(crate) enum NominalReceiverFailure {
    SourceStop(crate::source_verify::funding::SourceAbort),
    Resolve(Failure<crate::bounded_output::identity_reservation::Refused>),
    Base(
        super::super::prelude_bundle::InvocationFailure<
            crate::bounded_output::identity_reservation::Refused,
        >,
    ),
}
pub(crate) fn resolve_source_checked_previous_with_base<'p>(
    program: &'p dyn ProgramSource,
    previous_program: Option<&dyn ProgramSource>,
    previous_resolved: Option<&ResolvedProgram>,
    previous_costs: Option<&BTreeMap<String, usize>>,
    private: Option<source_verify::private_names::PrivateNames<'p>>,
    invocation: &mut super::super::prelude_bundle::Invocation<
        crate::bounded_output::identity_reservation::Refused,
    >,
    reserve: impl FnMut(usize) -> Result<(), crate::bounded_output::identity_reservation::Refused>,
) -> Result<(ResolvedProgram, FunctionResolutionWork), NominalReceiverFailure> {
    resolve_source_checked_previous_with_base_cells(
        program,
        previous_program,
        previous_resolved,
        previous_costs,
        private,
        invocation,
        None,
        reserve,
    )
}

pub(crate) fn resolve_source_checked_previous_with_owner<'p>(
    program: &'p dyn ProgramSource,
    previous_program: Option<&dyn ProgramSource>,
    previous_resolved: Option<&ResolvedProgram>,
    previous_costs: Option<&BTreeMap<String, usize>>,
    private: Option<source_verify::private_names::PrivateNames<'p>>,
    invocation: &mut super::super::prelude_bundle::Invocation<
        crate::bounded_output::identity_reservation::Refused,
    >,
    owner: &mut crate::bounded_output::identity_reservation::IdentityReservation,
) -> Result<(ResolvedProgram, FunctionResolutionWork), NominalReceiverFailure> {
    #[cfg(feature = "unstable-workflow-profiling")]
    let _workflow_span = crate::workflow_profile::span(crate::workflow_profile::Stage::Resolve);
    let receive = |error| NominalReceiverFailure::Resolve(Failure::Identity(error));
    owner
        .reserve(
            std::mem::size_of::<
                Result<
                    Vec<crate::ast::program_source::FunctionCell<'_>>,
                    crate::bounded_output::identity_reservation::Refused,
                >,
            >() + std::mem::size_of::<Vec<crate::ast::program_source::FunctionCell<'_>>>()
                + crate::source_verify::funding::SourceFunding::entry_receiving_bytes(),
        )
        .map_err(receive)?;
    let cells = match owner.function_cells(program) {
        Ok(cells) => cells,
        Err(error) => return Err(receive(error)),
    };
    owner
        .reserve(
            std::mem::size_of::<crate::ast::program_source::FunctionSlices<'_>>()
                + std::mem::size_of::<
                    crate::ast::program_source::DeclSlices<'_, crate::ast::TypeDeclaration>,
                >()
                + std::mem::size_of::<
                    crate::ast::program_source::DeclIter<'_, crate::ast::TypeDeclaration>,
                >()
                + std::mem::size_of::<bool>(),
        )
        .map_err(receive)?;
    let diagnostics = {
        let mut funding = match crate::source_verify::funding::SourceFunding::funded(owner) {
            Ok(funding) => funding,
            Err(error) => return Err(receive(error)),
        };
        if program.function_refs().has_stored_aliases()
            && program
                .types()
                .iter()
                .any(|declaration| !declaration.invariants().is_empty())
        {
            return Err(NominalReceiverFailure::SourceStop(funding.abort(
                Vec::new(),
                crate::source_verify::funding::SourceStop::InvariantRoute,
            )));
        }
        source_verify::verify_presenting_with_cells_funded(program, private, &cells, &mut funding)
    }
    .map_err(NominalReceiverFailure::SourceStop)?;
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity.is_error())
    {
        return Err(NominalReceiverFailure::Resolve(Failure::Source(
            diagnostics,
        )));
    }
    let (base, vector) = invocation
        .ensure_selected(
            crate::prelude::declarations_for_program(program),
            &mut |bytes| owner.reserve(bytes),
        )
        .map_err(NominalReceiverFailure::Base)?;
    let reuse = previous_program
        .zip(previous_resolved)
        .zip(previous_costs)
        .filter(|((previous, _), _)| source_environment_is_exact(program, *previous))
        .map(|((previous, resolved), costs)| FunctionReuse {
            current: program,
            program: previous,
            resolved,
            costs,
        });
    // A genuine owning invariant transform has a different immutable source.
    // Acquire that source's cells on this physical owner before lending any;
    // the generic callback compatibility path cannot manufacture its failure.
    let clause_program = record_invariants::source::clause_program(program);
    let resolved = if let Some(inventory) = &clause_program {
        let inventory_cells = owner.function_cells(inventory).map_err(receive)?;
        source_pass_nominal(
            inventory,
            None,
            Some(&inventory_cells),
            Some(base),
            vector,
            &mut |bytes| owner.reserve(bytes),
        )
    } else {
        source_pass_nominal(
            program,
            reuse,
            Some(&cells),
            Some(base),
            vector,
            &mut |bytes| owner.reserve(bytes),
        )
    }
    .map_err(NominalReceiverFailure::Resolve)?;
    let Some(desugared) = record_invariants::source::desugar(program, &resolved.0) else {
        return Ok(resolved);
    };
    let desugared_cells = owner.function_cells(&desugared).map_err(receive)?;
    source_pass_nominal(
        &desugared,
        None,
        Some(&desugared_cells),
        Some(base),
        vector,
        &mut |bytes| owner.reserve(bytes),
    )
    .map_err(NominalReceiverFailure::Resolve)
}

pub(crate) fn resolve_source_checked_previous_with_base_cells<'p>(
    program: &'p dyn ProgramSource,
    previous_program: Option<&dyn ProgramSource>,
    previous_resolved: Option<&ResolvedProgram>,
    previous_costs: Option<&BTreeMap<String, usize>>,
    private: Option<source_verify::private_names::PrivateNames<'p>>,
    invocation: &mut super::super::prelude_bundle::Invocation<
        crate::bounded_output::identity_reservation::Refused,
    >,
    cells: Option<&[crate::ast::program_source::FunctionCell<'_>]>,
    mut reserve: impl FnMut(usize) -> Result<(), crate::bounded_output::identity_reservation::Refused>,
) -> Result<(ResolvedProgram, FunctionResolutionWork), NominalReceiverFailure> {
    #[cfg(feature = "unstable-workflow-profiling")]
    let _workflow_span = crate::workflow_profile::span(crate::workflow_profile::Stage::Resolve);
    let diagnostics = match cells {
        Some(cells) => source_verify::verify_presenting_with_cells(program, private, cells),
        None => source_verify::verify_presenting(program, private),
    };
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity.is_error())
    {
        return Err(NominalReceiverFailure::Resolve(Failure::Source(
            diagnostics,
        )));
    }
    let (base, vector) = invocation
        .ensure_selected(
            crate::prelude::declarations_for_program(program),
            &mut reserve,
        )
        .map_err(NominalReceiverFailure::Base)?;
    let reuse = previous_program
        .zip(previous_resolved)
        .zip(previous_costs)
        .filter(|((previous, _), _)| source_environment_is_exact(program, *previous))
        .map(|((previous, resolved), costs)| FunctionReuse {
            current: program,
            program: previous,
            resolved,
            costs,
        });
    let clause_program = record_invariants::source::clause_program(program);
    let inventory: &dyn ProgramSource = clause_program.as_ref().map_or(program, |owned| owned);
    let resolved = source_pass_nominal(
        inventory,
        if clause_program.is_some() {
            None
        } else {
            reuse
        },
        if clause_program.is_some() {
            None
        } else {
            cells
        },
        Some(base),
        vector,
        &mut reserve,
    )
    .map_err(NominalReceiverFailure::Resolve)?;
    let Some(desugared) = record_invariants::source::desugar(program, &resolved.0) else {
        return Ok(resolved);
    };
    pass_nominal(&desugared, None, Some(base), vector, &mut reserve)
        .map_err(NominalReceiverFailure::Resolve)
}
