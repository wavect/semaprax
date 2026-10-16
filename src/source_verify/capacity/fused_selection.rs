//! Program capacity entry with an explicit allocation strategy.
//!
//! Every source function's flow is projected exactly as before. With no
//! selection the program is analyzed under `AllocationStrategy::Legacy`, which
//! is the unchanged default. A selection is one held, authenticated
//! [`SourceProof`] over this exact program. It changes exactly one thing:
//! its own construction loop projects as `CapacityFlow::FusedConstruction`,
//! named by the proof's actual `producer_site`, and the program is analyzed
//! under `FusedConstructionV1`. Every other loop, quota and limit is checked
//! as usual. The resulting summary must charge exactly one fused backing event
//! in the selected function. There is no skip flag and no raised limit, and
//! the flow grants no physical metadata or runtime authority.
use super::*;
use crate::ast::program_source::ProgramSource;
use crate::source_verify::fused_construction::SourceProof;

/// True only for the exact borrowed loop statement of a held selection.
pub(super) fn selected_loop(context: &SourceCapacityContext<'_>, statement: &Statement) -> bool {
    context.fused.is_some_and(|proof| {
        proof.still_held(context.program) && std::ptr::eq(statement, proof.construction().statement)
    })
}

/// The selected loop's flow, built from its actually projected condition and
/// body. The capacity rule then requires the body's single `BytesCopy` at
/// exactly `producer_site` with payload equal to the proof's width.
pub(super) fn selected_flow(
    context: &SourceCapacityContext<'_>,
    condition: crate::byte_data_capacity::CapacityFlow,
    body: crate::byte_data_capacity::CapacityFlow,
) -> Result<crate::byte_data_capacity::CapacityFlow, ()> {
    let proof = context.fused.ok_or(())?;
    Ok(crate::byte_data_capacity::CapacityFlow::FusedConstruction {
        producer: proof.producer_site(),
        rows: proof.rows(),
        width: proof.width(),
        condition: Box::new(condition),
        body: Box::new(body),
    })
}

#[cfg(test)]
pub(in crate::source_verify) fn verify_selected_capacity(
    program: &dyn ProgramSource,
    types: &TypeTable<'_>,
    selected: Option<&SourceProof<'_>>,
) -> Result<(), crate::byte_data_capacity::CapacityError> {
    let cells = source_capacity_functions(program);
    verify_selected_capacity_with_cells(program, types, selected, &cells)
}

pub(in crate::source_verify) fn verify_selected_capacity_with_cells(
    program: &dyn ProgramSource,
    types: &TypeTable<'_>,
    selected: Option<&SourceProof<'_>>,
    cells: &[crate::ast::program_source::FunctionCell<'_>],
) -> Result<(), crate::byte_data_capacity::CapacityError> {
    use crate::byte_data_capacity::{ArrayStorageKind, CapacityFlow, FunctionCapacityInput};

    if selected.is_some_and(|proof| !proof.authenticate(program)) {
        return Err(selection_invariant(
            None,
            "fused selection is not held by this program",
        ));
    }
    let ordinary = cells
        .iter()
        .take(program.function_refs().len())
        .map(|(_, function)| (function.name(), function))
        .collect::<BTreeMap<_, _>>();
    let inputs = cells
        .iter()
        .copied()
        .map(|(enclosing_class, function)| {
            let context = SourceCapacityContext {
                program,
                types,
                ordinary: &ordinary,
                enclosing_class: enclosing_class.map(|declaration| declaration.name.as_str()),
                fused: selected,
            };
            let mut slots = Vec::new();
            let mut bindings = BTreeMap::new();
            let mut transcript_roots = function
                .payload()
                .params
                .iter()
                .map(|parameter| {
                    (
                        parameter.name.clone(),
                        crate::byte_data_capacity::TranscriptSource::Unknown,
                    )
                })
                .collect::<BTreeMap<_, _>>();
            for (index, parameter) in function.payload().params.iter().enumerate() {
                source_capacity_slot(
                    &mut slots,
                    types,
                    format!("{}.param.{index}", function.payload().stable_id),
                    ArrayStorageKind::Parameter,
                    &parameter.ty,
                )
                .map_err(|()| {
                    command_io::source_capacity_invariant(&function.payload().stable_id)
                })?;
                bindings.insert(parameter.name.clone(), parameter.ty.clone());
            }
            source_capacity_slot(
                &mut slots,
                types,
                format!("{}.result", function.payload().stable_id),
                ArrayStorageKind::ProvisionalResult,
                &function.payload().return_type,
            )
            .map_err(|()| command_io::source_capacity_invariant(&function.payload().stable_id))?;
            let mut execution = function
                .payload()
                .requires
                .iter()
                .enumerate()
                .map(|(index, expression)| {
                    source_capacity_expr(
                        expression,
                        &format!("{}.requires.{index}", function.payload().stable_id),
                        &mut bindings.clone(),
                        &mut transcript_roots.clone(),
                        &mut slots,
                        &context,
                        false,
                    )
                    .map_err(|()| {
                        command_io::source_capacity_invariant(&function.payload().stable_id)
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            execution.push(
                source_capacity_expr(
                    &function.payload().body,
                    &format!("{}.body", function.payload().stable_id),
                    &mut bindings,
                    &mut transcript_roots,
                    &mut slots,
                    &context,
                    true,
                )
                .map_err(|()| {
                    command_io::source_capacity_invariant(&function.payload().stable_id)
                })?,
            );
            execution.extend(
                function
                    .payload()
                    .ensures
                    .iter()
                    .enumerate()
                    .map(|(index, expression)| {
                        source_capacity_expr(
                            expression,
                            &format!("{}.ensures.{index}", function.payload().stable_id),
                            &mut bindings.clone(),
                            &mut transcript_roots.clone(),
                            &mut slots,
                            &context,
                            false,
                        )
                        .map_err(|()| {
                            command_io::source_capacity_invariant(&function.payload().stable_id)
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            );
            Ok(FunctionCapacityInput {
                function: function.payload().stable_id.clone(),
                array_slots: slots,
                execution: CapacityFlow::Sequence(execution),
            })
        })
        .collect::<Result<Vec<_>, crate::byte_data_capacity::CapacityError>>()?;
    let Some(proof) = selected else {
        return crate::byte_data_capacity::analyze(&inputs).map(|_| ());
    };
    let summary = crate::byte_data_capacity::analyze_with_strategy(
        &inputs,
        crate::byte_data_capacity::AllocationStrategy::FusedConstructionV1,
    )?;
    let identity = proof.function().stable_id.as_str();
    if summary
        .function(identity)
        .is_none_or(|function| function.fused_backing_events != 1)
    {
        return Err(selection_invariant(
            Some(identity),
            "selected fused construction did not project exactly one fused backing",
        ));
    }
    Ok(())
}

fn selection_invariant(
    function: Option<&str>,
    detail: &str,
) -> crate::byte_data_capacity::CapacityError {
    crate::byte_data_capacity::CapacityError {
        witness_help: None,
        diagnostic: crate::byte_data_capacity::CapacityDiagnostic::Invariant,
        function: function.map(str::to_owned),
        detail: detail.to_owned(),
    }
}
