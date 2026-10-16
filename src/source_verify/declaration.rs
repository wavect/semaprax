//! Program-level verification: `verify` builds the shared declaration state
//! and runs the ordered passes that own each concern.

use self::classes::{
    check_class_cycles, check_class_methods, check_class_overrides, check_class_parents,
};
use self::declarations::{
    check_byte_data_declarations, check_declared_fields, check_native_rust_imports,
    check_record_layouts, check_resource_lifecycles,
};
pub(super) use self::functions::SignatureOrigin;
use self::functions::{
    check_function_bodies, check_function_declarations, check_generic_function_cycles_with_types,
};
use self::identity::{check_interface_identities, check_type_identities};
use super::capacity::source_capacity_functions;
use super::diagnostics::error;
use super::type_table::TypeTable;
use crate::ast::program_source::ProgramSource;
use crate::ast::{Span, Type};
use crate::diagnostic::Diagnostic;
use std::collections::{HashMap, HashSet};

mod classes;
mod declarations;
mod functions;
pub(super) mod fusion;
mod identity;
pub(super) mod record_shape;
#[cfg(test)]
mod record_shape_before;

pub(crate) fn verify(program: &dyn ProgramSource) -> Vec<Diagnostic> {
    verify_selected(program, None, None)
}

/// [`verify`] with a Workspace Graph private layout presentation bound to
/// this exact `program`. It changes diagnostic text only.
pub(crate) fn verify_presenting<'p>(
    program: &'p dyn ProgramSource,
    private: Option<crate::source_verify::private_names::PrivateNames<'p>>,
) -> Vec<Diagnostic> {
    verify_selected(program, None, private)
}

/// The actual paid resolver lends this invocation's pre-acquired inventory.
/// Diagnostics retain exactly the ordinary pass order and all six fields.
pub(crate) fn verify_presenting_with_cells<'p>(
    program: &'p dyn ProgramSource,
    private: Option<crate::source_verify::private_names::PrivateNames<'p>>,
    cells: &[crate::ast::program_source::FunctionCell<'_>],
) -> Vec<Diagnostic> {
    verify_profile(program, None, private, false, Some(cells))
}

pub(crate) fn verify_presenting_with_cells_funded<'p>(
    program: &'p dyn ProgramSource,
    private: Option<crate::source_verify::private_names::PrivateNames<'p>>,
    cells: &[crate::ast::program_source::FunctionCell<'_>],
    funding: &mut super::funding::SourceFunding<'_>,
) -> Result<Vec<Diagnostic>, super::funding::SourceAbort> {
    verify_profile_funded(program, None, private, false, Some(cells), funding)
}

/// Every ordinary source pass. A held `FusedConstructionV1` selection only
/// changes the two places that would otherwise apply the legacy 32-event rule
/// to its own loop: the while-body producer admission and that loop's capacity
/// flow. Callers outside the private candidate route always pass `None`.
pub(in crate::source_verify) fn verify_selected<'p>(
    program: &'p dyn ProgramSource,
    selected: Option<&crate::source_verify::fused_construction::SourceProof<'p>>,
    private: Option<crate::source_verify::private_names::PrivateNames<'p>>,
) -> Vec<Diagnostic> {
    verify_profile(program, selected, private, false, None)
}

/// Private factory only: all source passes, with the explicit partition
/// availability guard removed. This does not grant HIR or runtime authority.
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "private partition source factory has no ordinary route"
    )
)]
pub(crate) fn verify_partition(program: &dyn ProgramSource) -> Vec<Diagnostic> {
    verify_profile(program, None, None, true, None)
}

fn verify_profile<'p>(
    program: &'p dyn ProgramSource,
    selected: Option<&crate::source_verify::fused_construction::SourceProof<'p>>,
    private: Option<crate::source_verify::private_names::PrivateNames<'p>>,
    partition: bool,
    cells: Option<&[crate::ast::program_source::FunctionCell<'_>]>,
) -> Vec<Diagnostic> {
    let mut funding = super::funding::SourceFunding::ordinary();
    match verify_profile_funded(program, selected, private, partition, cells, &mut funding) {
        Ok(diagnostics) => diagnostics,
        Err(_) => unreachable!("ordinary source funding cannot refuse"),
    }
}

fn verify_profile_funded<'p>(
    program: &'p dyn ProgramSource,
    selected: Option<&crate::source_verify::fused_construction::SourceProof<'p>>,
    private: Option<crate::source_verify::private_names::PrivateNames<'p>>,
    partition: bool,
    cells: Option<&[crate::ast::program_source::FunctionCell<'_>]>,
    funding: &mut super::funding::SourceFunding<'_>,
) -> Result<Vec<Diagnostic>, super::funding::SourceAbort> {
    let mut diagnostics = Vec::new();
    if let Err(diagnostic) = crate::static_protocol::validate_source_with_cells(program, cells) {
        diagnostics.push(diagnostic.at_path(program.path()));
        return Ok(diagnostics);
    }
    if !program.module_uses().is_empty() {
        diagnostics.push(
            Diagnostic::io(
                "SPX-G172",
                "source module imports require Workspace Semantic Graph resolution",
            )
            .at_path(program.path())
            .with_help(super::hints::PROJECT_IMPORTS_HELP),
        );
        return Ok(diagnostics);
    }
    let ordinary_cells;
    let capacity_functions = if let Some(cells) = cells {
        cells
    } else {
        ordinary_cells = source_capacity_functions(program);
        &ordinary_cells
    };
    super::stdin_stream::check_with_cells(program, capacity_functions, &mut diagnostics);
    if !partition {
        let mut private_call = None;
        for (_, function) in capacity_functions.iter() {
            for root in function
                .payload()
                .requires
                .iter()
                .chain(std::iter::once(&function.payload().body))
                .chain(&function.payload().ensures)
            {
                root.visit_calls(&mut |name, span| {
                    if name == crate::construction_fusion::partition::NAME && private_call.is_none()
                    {
                        private_call = Some(span);
                    }
                });
            }
        }
        for declaration in program.types() {
            for invariant in declaration.invariants() {
                invariant.visit_calls(&mut |name, span| {
                    if name == crate::construction_fusion::partition::NAME && private_call.is_none()
                    {
                        private_call = Some(span);
                    }
                });
            }
        }
        if let Some(span) = private_call {
            diagnostics.push(error(
                program,
                "SPX-T263",
                "byte partition requires the private checked source profile",
                span,
            ));
            return Ok(diagnostics);
        }
    }
    if capacity_functions.len() > crate::byte_data_capacity::MAX_FUNCTIONS {
        let (_, function) = capacity_functions[crate::byte_data_capacity::MAX_FUNCTIONS];
        diagnostics.push(
            error(
                program,
                "SPX-T270",
                format!(
                    "module declares more than the admitted {} functions",
                    crate::byte_data_capacity::MAX_FUNCTIONS
                ),
                function.payload().name_span,
            )
            .with_help("split the program into modules with at most 4096 functions each"),
        );
        return Ok(diagnostics);
    }
    let mut functions = crate::source_verify::function_map::FunctionMap::new();
    let mut ids = crate::prelude::all_reserved_ids()
        .into_iter()
        .collect::<HashSet<_>>();
    let mut type_names = HashSet::new();

    check_type_identities(program, &mut ids, &mut type_names, &mut diagnostics);

    let mut interface_names = HashSet::new();
    let mut import_keys = HashMap::new();
    check_interface_identities(
        program,
        &mut ids,
        &mut interface_names,
        &mut import_keys,
        &mut diagnostics,
    );

    let mut types = TypeTable::new(program);
    types.private = private;
    check_byte_data_declarations(program, &types, &mut diagnostics);
    let mut native_rust_names = HashSet::new();
    check_native_rust_imports(program, &types, &mut native_rust_names, &mut diagnostics);
    check_resource_lifecycles(program, &import_keys, &mut diagnostics);
    check_declared_fields(program, &types, &mut diagnostics);
    let mut checked_layouts = HashSet::new();
    check_record_layouts(program, &types, &mut checked_layouts, &mut diagnostics);

    check_function_declarations(
        program,
        &capacity_functions,
        &types,
        &mut functions,
        &mut ids,
        &mut diagnostics,
    );
    diagnostics.extend(crate::session_protocol::source::check(program));
    diagnostics.extend(crate::session_protocol::typestate::check(program));

    check_class_methods(program, &types, &mut ids, &mut diagnostics);

    // Class Inheritance v1: static structural checks over `extends` links —
    // unknown or non-class parents, cycles, member collisions with ancestors,
    // and exact-signature override validation.
    let mut class_parents: HashMap<&str, &str> = HashMap::new();
    check_class_parents(program, &types, &mut class_parents, &mut diagnostics);
    check_class_cycles(program, &class_parents, &mut diagnostics);
    check_class_overrides(program, &types, &class_parents, &mut diagnostics);

    let function_value_targets =
        crate::source_verify::function_value_inventory::function_value_targets(program, &functions);
    let call_graph = capacity_functions
        .iter()
        .take(program.functions().len())
        .map(|(_, function)| {
            (
                function.name().to_owned(),
                crate::source_verify::function_value_inventory::calls(
                    function.payload(),
                    &functions,
                    &function_value_targets,
                ),
            )
        })
        .collect::<HashMap<_, _>>();
    let generic_functions = capacity_functions
        .iter()
        .take(program.functions().len())
        .filter(|(_, function)| !function.payload().type_parameters.is_empty())
        .map(|(_, function)| function.name())
        .collect::<HashSet<_>>();
    check_generic_function_cycles_with_types(
        program,
        capacity_functions,
        &call_graph,
        &generic_functions,
        &types,
        &functions,
        &mut diagnostics,
    );

    super::lexical_exit::validate(program, &mut diagnostics);
    if let Err(stop) = check_function_bodies(
        program,
        &capacity_functions,
        &functions,
        &import_keys,
        &types,
        &mut diagnostics,
        selected.map(|proof| proof.producer().call),
        funding,
    ) {
        return Err(funding.abort(diagnostics, stop));
    }
    crate::source_verify::record_invariants::check_record_invariants(
        program,
        &functions,
        &types,
        &mut diagnostics,
    );

    if let Some(main) = functions.get("main") {
        if !main.payload().type_parameters.is_empty()
            || !main.payload().params.is_empty()
            || main.payload().return_type != Type::I64
        {
            diagnostics.push(
                error(
                    program,
                    "SPX-T104",
                    "entry function must be monomorphic with signature `fn main() -> i64`",
                    main.payload().span,
                )
                .with_help(crate::source_verify::hints::entry_signature_help(
                    main.payload(),
                )),
            );
        }
    } else {
        diagnostics.push(
            Diagnostic::error(
                "SPX-T105",
                "executable module must define `fn main() -> i64`",
                program
                    .functions()
                    .first()
                    .map_or(Span::default(), |function| function.span),
            )
            .at_path(program.path())
            .with_help(super::hints::LIBRARY_MODULE_HELP),
        );
    }
    if let Err(capacity) = super::capacity::verify_selected_capacity_with_cells(
        program,
        &types,
        selected,
        capacity_functions,
    ) {
        if capacity.diagnostic != crate::byte_data_capacity::CapacityDiagnostic::Invariant
            || !diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity.is_error())
        {
            let span = capacity
                .function
                .as_deref()
                .and_then(|identity| {
                    capacity_functions
                        .iter()
                        .find(|(_, function)| function.payload().stable_id == identity)
                })
                .map_or(Span::default(), |(_, function)| function.payload().span);
            let help = (capacity.diagnostic
                == crate::byte_data_capacity::CapacityDiagnostic::Allocation)
                .then(|| fusion::refusal_help(program))
                .flatten();
            let witness_help = capacity.witness_help;
            let mut diagnostic = error(program, capacity.diagnostic.code(), capacity.detail, span);
            if let Some(witness_help) = witness_help {
                diagnostic = super::hints::with_capacity_witness(diagnostic, witness_help);
            }
            if let Some(help) = help {
                diagnostic = diagnostic.with_help(help);
            }
            diagnostics.push(diagnostic);
        }
    }
    let mut native_interop_failures = HashSet::new();
    diagnostics.retain(|diagnostic| {
        diagnostic.code != "SPX-B107" || native_interop_failures.insert(diagnostic.message.clone())
    });
    if let Some(native_failure) = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "SPX-B107")
        .cloned()
    {
        return Ok(vec![native_failure]);
    }
    Ok(diagnostics)
}

#[cfg(test)]
#[rustfmt::skip]
pub(super) mod old;

#[cfg(test)]
#[rustfmt::skip]
pub(super) mod public_signature_before;
