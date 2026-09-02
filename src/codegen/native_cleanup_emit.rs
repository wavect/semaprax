//! Unit-test-only C control-flow scaffold for the first native cleanup slice.
//!
//! The production resource gate remains closed. This emitter consumes a
//! [`NativeCleanupIndex`] and explicit C identifier bindings; it never derives
//! cleanup from HIR or repairs the attached plan. Missing observations or
//! storage names fail closed with `SPX-B104`.
//! Canonical checked-success continuations are revalidated independently and
//! may only leave empty regions without changing ownership.

#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "the plan scaffold remains unreachable until native conformance is complete"
    )
)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use crate::cleanup::LivenessFlagId;
use crate::cleanup_plan::{
    BlockId, CleanupPlace, CleanupResultSource, CleanupTerminator, CleanupTransition,
    ContractPhase, EdgeCondition, ExitContinuation, StatusCase, StatusLane, StatusProducer,
    StatusSourceId, StorageId,
};
use crate::conformance::TraceEventKind;
use crate::diagnostic::Diagnostic;
use crate::hir::{DeclarationId, ExpressionId};
use crate::semantic_trace::{
    SemanticEventDictionary, SemanticEventEntry, SEMANTIC_EVENT_DICTIONARY_V1,
};

use super::native_cleanup::{NativeCleanupIndex, NativeCleanupLeaf};

/// Physical C identifiers supplied by the value/status emitter.
///
/// Every value is restricted to one C identifier. This scaffold deliberately
/// does not accept arbitrary snippets that could hide evaluation or ownership
/// changes inside an expression. The surrounding value emitter must allocate
/// every name inside the dedicated `spx_bind_` namespace; arbitrary C/runtime
/// identifiers and object-like macros cannot cross this boundary.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct NativeCleanupBindings {
    pub(crate) context: String,
    pub(crate) storage_values: BTreeMap<StorageId, String>,
    pub(crate) boolean_values: BTreeMap<ExpressionId, String>,
    pub(crate) status_tokens: BTreeMap<StatusSourceId, String>,
    pub(crate) scalar_results: BTreeMap<ExpressionId, String>,
    pub(crate) result_out: Option<String>,
    pub(crate) semantic_events: Option<SemanticEventDictionary>,
    /// Planner-produced boolean observations for exact decision-chain edges.
    pub(crate) decision_edges: BTreeMap<crate::cleanup_plan::EdgeId, String>,
}

/// Emit one deterministic function-body fragment from an already-classified
/// cleanup plan.
///
/// The fragment assumes storage values, boolean observations, status tokens,
/// the native status/trace runtime types and helpers, and
/// `spx_runtime_invariant_failure` are declared by the surrounding emitter.
/// Trace event inputs are canonically zero-initialized before semantic fields
/// are populated, so private trace-storage ownership fields remain zero until
/// `spx_trace_push` copies the event into its claimed buffer slot.
pub(crate) fn emit(
    index: &NativeCleanupIndex<'_>,
    bindings: &NativeCleanupBindings,
) -> Result<String, Diagnostic> {
    emit_with_block_prologues(index, bindings, |_, _| Ok(()))
}

/// Emit the cleanup scaffold while allowing the surrounding value emitter to
/// materialize each block's observations immediately after its label.
pub(crate) fn emit_with_block_prologues(
    index: &NativeCleanupIndex<'_>,
    bindings: &NativeCleanupBindings,
    mut emit_block_prologue: impl FnMut(BlockId, &mut String) -> Result<(), Diagnostic>,
) -> Result<String, Diagnostic> {
    validate_bindings(index, bindings)?;

    let mut output = String::from("/* semaprax.native-cleanup-scaffold.v1 */\n");
    for leaf in index.leaves() {
        writeln!(output, "bool {} = false;", flag_symbol(leaf.flag))
            .expect("writing to a string cannot fail");
    }
    output.push_str("spx_status_token spx_cleanup_selected_status = SPX_STATUS_SUCCESS;\n");
    for place in index.live_owned_parameters() {
        let leaf = leaf_for_place(index, place)?;
        writeln!(output, "{} = true;", flag_symbol(leaf.flag))
            .expect("writing to a string cannot fail");
    }
    writeln!(output, "goto {};", block_label(index.entry()))
        .expect("writing to a string cannot fail");

    for indexed in index.blocks() {
        writeln!(output, "{}:", block_label(indexed.block.id))
            .expect("writing to a string cannot fail");
        emit_block_prologue(indexed.block.id, &mut output)?;
        for transition in indexed.transitions {
            emit_transition(&mut output, index, bindings, transition)?;
        }
        emit_terminator(
            &mut output,
            index,
            bindings,
            indexed.block.id,
            &indexed.block.terminator,
        )?;
    }

    for indexed in index.exits() {
        writeln!(output, "{}:", exit_label(indexed.exit.id.0))
            .expect("writing to a string cannot fail");
        for action in indexed.finalizers {
            let leaf = index.leaf(action.guard_flag).ok_or_else(|| {
                cleanup_error(format!(
                    "exit {} references unknown finalizer flag {}",
                    indexed.exit.id.0, action.guard_flag.0
                ))
            })?;
            if leaf.place != action.source || leaf.lifecycle_id != &action.lifecycle_id {
                return Err(cleanup_error(format!(
                    "exit {} finalizer flag {} disagrees with its classified leaf",
                    indexed.exit.id.0, action.guard_flag.0
                )));
            }
            let flag = flag_symbol(action.guard_flag);
            writeln!(output, "if ({flag}) {{").expect("writing to a string cannot fail");
            writeln!(output, "    {flag} = false;").expect("writing to a string cannot fail");
            emit_finalize_trace(
                &mut output,
                index,
                bindings,
                "SPX_TRACE_FINALIZE_BEGIN",
                &action.source,
                &action.lifecycle_id,
                action.guard_flag,
            )?;
            if action.lifecycle_id.as_str() == crate::cleanup::BYTES_DROP_LIFECYCLE_ID {
                let value = storage_binding(bindings, &action.source.storage)?;
                writeln!(output, "    spx_bytes_drop(&{value});")
                    .expect("writing to a string cannot fail");
            }
            emit_finalize_trace(
                &mut output,
                index,
                bindings,
                "SPX_TRACE_FINALIZE_END",
                &action.source,
                &action.lifecycle_id,
                action.guard_flag,
            )?;
            output.push_str("}\n");
        }
        emit_continuation(&mut output, index, bindings, indexed.exit)?;
    }

    Ok(output)
}

fn begin_trace_event(
    output: &mut String,
    index: &NativeCleanupIndex<'_>,
    kind: &str,
    semantic_ordinal: u32,
) -> Result<(), Diagnostic> {
    output.push_str("{\n");
    output.push_str("    struct spx_trace_event spx_cleanup_event = {0};\n");
    writeln!(output, "    spx_cleanup_event.kind = {kind};")
        .expect("writing to a string cannot fail");
    if semantic_ordinal != 0 {
        writeln!(
            output,
            "    spx_cleanup_event.semantic_ordinal = UINT32_C({semantic_ordinal});"
        )
        .expect("writing to a string cannot fail");
    }
    writeln!(
        output,
        "    spx_cleanup_event.function_id = \"{}\";",
        c_string(index.function_id().as_str())
    )
    .expect("writing to a string cannot fail");
    Ok(())
}

fn end_trace_event(output: &mut String, bindings: &NativeCleanupBindings) {
    writeln!(
        output,
        "    spx_trace_push({}, &spx_cleanup_event);",
        bindings.context
    )
    .expect("writing to a string cannot fail");
    output.push_str("}\n");
}

fn semantic_ordinal_for_event(
    bindings: &NativeCleanupBindings,
    index: &NativeCleanupIndex<'_>,
    event: &TraceEventKind,
) -> Result<u32, Diagnostic> {
    semantic_ordinal_matching(
        bindings,
        index,
        |entry| &entry.event == event,
        "exact semantic event",
    )
}

fn semantic_ordinal_matching(
    bindings: &NativeCleanupBindings,
    index: &NativeCleanupIndex<'_>,
    predicate: impl Fn(&SemanticEventEntry) -> bool,
    description: &str,
) -> Result<u32, Diagnostic> {
    let Some(dictionary) = &bindings.semantic_events else {
        return Ok(0);
    };
    if dictionary.schema() != SEMANTIC_EVENT_DICTIONARY_V1
        || dictionary.function() != index.function_id()
    {
        return Err(cleanup_error(format!(
            "semantic event dictionary identity disagrees with function `{}`",
            index.function_id()
        )));
    }
    let mut matches = dictionary.entries().iter().filter(|entry| predicate(entry));
    let Some(entry) = matches.next() else {
        return Err(cleanup_error(format!(
            "semantic event dictionary has no {description} for function `{}`",
            index.function_id()
        )));
    };
    if matches.next().is_some() {
        return Err(cleanup_error(format!(
            "semantic event dictionary has ambiguous {description} for function `{}`",
            index.function_id()
        )));
    }
    if entry.ordinal == 0 {
        return Err(cleanup_error(
            "semantic event dictionary contains the reserved zero ordinal",
        ));
    }
    Ok(entry.ordinal)
}

fn emit_trace_place(
    output: &mut String,
    field: &str,
    place: &CleanupPlace,
) -> Result<(), Diagnostic> {
    if !place.projections.is_empty() {
        return Err(cleanup_error(
            "projected cleanup place reached trace event emission",
        ));
    }
    match &place.storage {
        StorageId::Value(value) => {
            writeln!(
                output,
                "    spx_cleanup_event.{field}.storage.kind = SPX_TRACE_STORAGE_VALUE;"
            )
            .expect("writing to a string cannot fail");
            writeln!(
                output,
                "    spx_cleanup_event.{field}.storage.value_id = \"{}\";",
                c_string(value.as_str())
            )
            .expect("writing to a string cannot fail");
        }
        StorageId::Temporary(expression) => {
            writeln!(
                output,
                "    spx_cleanup_event.{field}.storage.kind = SPX_TRACE_STORAGE_TEMPORARY;"
            )
            .expect("writing to a string cannot fail");
            writeln!(
                output,
                "    spx_cleanup_event.{field}.storage.expression_id = \"{}\";",
                c_string(expression.as_str())
            )
            .expect("writing to a string cannot fail");
        }
        StorageId::ProvisionalResult => {
            writeln!(
                output,
                "    spx_cleanup_event.{field}.storage.kind = SPX_TRACE_STORAGE_PROVISIONAL_RESULT;"
            )
            .expect("writing to a string cannot fail");
        }
        StorageId::CallArgument { .. } => {
            return Err(cleanup_error(
                "call-argument storage reached root-frame trace event emission",
            ));
        }
    }
    Ok(())
}

fn emit_transfer_trace(
    output: &mut String,
    index: &NativeCleanupIndex<'_>,
    bindings: &NativeCleanupBindings,
    at: &ExpressionId,
    source: &CleanupPlace,
    destination: &CleanupPlace,
) -> Result<(), Diagnostic> {
    let event = TraceEventKind::Transfer {
        at: at.clone(),
        source: source.clone(),
        destination: destination.clone(),
    };
    let semantic_ordinal = semantic_ordinal_for_event(bindings, index, &event)?;
    begin_trace_event(output, index, "SPX_TRACE_TRANSFER", semantic_ordinal)?;
    writeln!(
        output,
        "    spx_cleanup_event.data.transfer.at_expression_id = \"{}\";",
        c_string(at.as_str())
    )
    .expect("writing to a string cannot fail");
    emit_trace_place(output, "data.transfer.source", source)?;
    emit_trace_place(output, "data.transfer.destination", destination)?;
    end_trace_event(output, bindings);
    Ok(())
}

fn emit_select_failure_trace(
    output: &mut String,
    index: &NativeCleanupIndex<'_>,
    bindings: &NativeCleanupBindings,
    source: &StatusSourceId,
) -> Result<(), Diagnostic> {
    let semantic_source = index
        .status_sources()
        .iter()
        .find(|candidate| candidate.id == *source)
        .ok_or_else(|| cleanup_error(format!("unknown trace status source `{source:?}`")))?;
    if !matches!(
        (&semantic_source.producer, source.lane),
        (
            StatusProducer::ContractFalse { .. },
            StatusLane::ContractFalse
        ) | (
            StatusProducer::CheckedArithmetic { .. },
            StatusLane::OperationFailure
        )
    ) {
        return Err(cleanup_error(format!(
            "trace status source `{source:?}` has a producer/lane mismatch or unsupported propagated call"
        )));
    }
    let status = status_binding(bindings, source)?;
    let semantic_ordinal = semantic_ordinal_matching(
        bindings,
        index,
        |entry| {
            matches!(
                &entry.event,
                TraceEventKind::SelectFailure {
                    source: candidate,
                    ..
                } if candidate == source
            )
        },
        "failure selection",
    )?;
    begin_trace_event(output, index, "SPX_TRACE_SELECT_FAILURE", semantic_ordinal)?;
    writeln!(
        output,
        "    spx_cleanup_event.data.select_failure.source.expression_id = \"{}\";",
        c_string(source.expression.as_str())
    )
    .expect("writing to a string cannot fail");
    let lane = match source.lane {
        StatusLane::OperationFailure => "SPX_TRACE_STATUS_OPERATION_FAILURE",
        StatusLane::ContractFalse => "SPX_TRACE_STATUS_CONTRACT_FALSE",
    };
    writeln!(
        output,
        "    spx_cleanup_event.data.select_failure.source.lane = {lane};"
    )
    .expect("writing to a string cannot fail");
    writeln!(
        output,
        "    const struct spx_normalized_status *spx_cleanup_normalized_status = spx_status_resolve({}, {status});",
        bindings.context
    )
    .expect("writing to a string cannot fail");
    emit_status_source_check(output, &semantic_source.producer)?;
    output.push_str(
        "    spx_cleanup_event.data.select_failure.status.schema = spx_cleanup_normalized_status->schema;\n",
    );
    output.push_str(
        "    spx_cleanup_event.data.select_failure.status.domain_id = spx_cleanup_normalized_status->domain_id;\n",
    );
    output.push_str(
        "    spx_cleanup_event.data.select_failure.status.code = spx_cleanup_normalized_status->code;\n",
    );
    output.push_str(
        "    spx_cleanup_event.data.select_failure.status.status_class = spx_cleanup_normalized_status->status_class;\n",
    );
    output.push_str(
        "    spx_cleanup_event.data.select_failure.status.retryability = spx_cleanup_normalized_status->retryability;\n",
    );
    end_trace_event(output, bindings);
    Ok(())
}

fn emit_status_source_check(
    output: &mut String,
    producer: &StatusProducer,
) -> Result<(), Diagnostic> {
    let producer_check = match producer {
        StatusProducer::ContractFalse { phase, .. } => {
            let code = match phase {
                ContractPhase::Requires => "SPX_STATUS_CONTRACT_REQUIRES_FALSE",
                ContractPhase::Ensures => "SPX_STATUS_CONTRACT_ENSURES_FALSE",
            };
            format!(
                "strcmp(spx_cleanup_normalized_status->domain_id, \"semaprax.contract.v1\") != 0 || spx_cleanup_normalized_status->code != {code} || spx_cleanup_normalized_status->status_class != SPX_STATUS_CLASS_CONTRACT"
            )
        }
        StatusProducer::CheckedArithmetic {
            normalized_cases, ..
        } => {
            if normalized_cases.is_empty() {
                return Err(cleanup_error(
                    "checked arithmetic trace source has no normalized cases",
                ));
            }
            let mut code_check = String::new();
            for (position, case) in normalized_cases.iter().enumerate() {
                if position != 0 {
                    code_check.push_str(" && ");
                }
                write!(
                    code_check,
                    "spx_cleanup_normalized_status->code != {}",
                    status_case_macro(*case)
                )
                .expect("writing to a string cannot fail");
            }
            format!(
                "strcmp(spx_cleanup_normalized_status->domain_id, \"semaprax.arithmetic.v1\") != 0 || ({code_check}) || spx_cleanup_normalized_status->status_class != SPX_STATUS_CLASS_ARITHMETIC"
            )
        }
        StatusProducer::PropagatedCall { callee } => {
            return Err(cleanup_error(format!(
                "propagated call status from `{callee}` reached single-frame trace emission"
            )));
        }
    };
    writeln!(
        output,
        "    if (spx_cleanup_normalized_status == NULL || spx_cleanup_normalized_status->schema == NULL || spx_cleanup_normalized_status->domain_id == NULL || strcmp(spx_cleanup_normalized_status->schema, SPX_STATUS_SCHEMA_V1) != 0 || {producer_check} || spx_cleanup_normalized_status->retryability != SPX_RETRYABILITY_FALSE) spx_runtime_invariant_failure(\"cleanup selected status disagrees with its semantic source\");"
    )
    .expect("writing to a string cannot fail");
    Ok(())
}

fn status_case_macro(case: StatusCase) -> &'static str {
    match case {
        StatusCase::AddOverflow => "SPX_STATUS_ARITHMETIC_ADD_OVERFLOW",
        StatusCase::SubOverflow => "SPX_STATUS_ARITHMETIC_SUB_OVERFLOW",
        StatusCase::MulOverflow => "SPX_STATUS_ARITHMETIC_MUL_OVERFLOW",
        StatusCase::DivisionByZero => "SPX_STATUS_ARITHMETIC_DIVISION_BY_ZERO",
        StatusCase::DivisionOverflow => "SPX_STATUS_ARITHMETIC_DIVISION_OVERFLOW",
        StatusCase::RemainderByZero => "SPX_STATUS_ARITHMETIC_REMAINDER_BY_ZERO",
        StatusCase::RemainderOverflow => "SPX_STATUS_ARITHMETIC_REMAINDER_OVERFLOW",
        StatusCase::NegationOverflow => "SPX_STATUS_ARITHMETIC_NEGATION_OVERFLOW",
    }
}

fn emit_finalize_trace(
    output: &mut String,
    index: &NativeCleanupIndex<'_>,
    bindings: &NativeCleanupBindings,
    kind: &str,
    source: &CleanupPlace,
    lifecycle: &DeclarationId,
    flag: LivenessFlagId,
) -> Result<(), Diagnostic> {
    let semantic_ordinal = semantic_ordinal_matching(
        bindings,
        index,
        |entry| match (&entry.event, kind) {
            (
                TraceEventKind::FinalizeBegin {
                    source: candidate_source,
                    lifecycle_id,
                    guard_flag,
                    ..
                },
                "SPX_TRACE_FINALIZE_BEGIN",
            )
            | (
                TraceEventKind::FinalizeEnd {
                    source: candidate_source,
                    lifecycle_id,
                    guard_flag,
                    ..
                },
                "SPX_TRACE_FINALIZE_END",
            ) => candidate_source == source && lifecycle_id == lifecycle && *guard_flag == flag,
            _ => false,
        },
        "resource finalization",
    )?;
    begin_trace_event(output, index, kind, semantic_ordinal)?;
    emit_trace_place(output, "data.finalize.source", source)?;
    writeln!(
        output,
        "    spx_cleanup_event.data.finalize.lifecycle_id = \"{}\";",
        c_string(lifecycle.as_str())
    )
    .expect("writing to a string cannot fail");
    writeln!(
        output,
        "    spx_cleanup_event.data.finalize.guard_flag = UINT32_C({});",
        flag.0
    )
    .expect("writing to a string cannot fail");
    end_trace_event(output, bindings);
    Ok(())
}

fn emit_result_commit_trace(
    output: &mut String,
    index: &NativeCleanupIndex<'_>,
    bindings: &NativeCleanupBindings,
    source: &CleanupResultSource,
) -> Result<(), Diagnostic> {
    let event = TraceEventKind::ResultCommit {
        source: source.clone(),
    };
    let semantic_ordinal = semantic_ordinal_for_event(bindings, index, &event)?;
    begin_trace_event(output, index, "SPX_TRACE_RESULT_COMMIT", semantic_ordinal)?;
    match source {
        CleanupResultSource::Scalar { expression } => {
            output.push_str(
                "    spx_cleanup_event.data.result_commit.source.kind = SPX_TRACE_RESULT_SCALAR;\n",
            );
            writeln!(
                output,
                "    spx_cleanup_event.data.result_commit.source.scalar_expression_id = \"{}\";",
                c_string(expression.as_str())
            )
            .expect("writing to a string cannot fail");
        }
        CleanupResultSource::Owned { storage } => {
            output.push_str(
                "    spx_cleanup_event.data.result_commit.source.kind = SPX_TRACE_RESULT_OWNED;\n",
            );
            emit_trace_place(output, "data.result_commit.source.owned_storage", storage)?;
        }
    }
    end_trace_event(output, bindings);
    Ok(())
}

fn emit_transition(
    output: &mut String,
    index: &NativeCleanupIndex<'_>,
    bindings: &NativeCleanupBindings,
    transition: &CleanupTransition,
) -> Result<(), Diagnostic> {
    match transition {
        CleanupTransition::Initialize { at, .. } => {
            return Err(cleanup_error(format!(
                "initialize transition `{at}` has no physical payload source in the cleanup scaffold"
            )));
        }
        CleanupTransition::InitializeVariant { at, .. } => {
            return Err(cleanup_error(format!(
                "conditional variant initialization `{at}` reached the resource cleanup scaffold"
            )));
        }
        CleanupTransition::Transfer {
            at,
            source,
            destination,
        } => {
            let source_leaf = leaf_for_place(index, source)?;
            let destination_leaf = leaf_for_place(index, destination)?;
            if source_leaf.lifecycle_id != destination_leaf.lifecycle_id {
                return Err(cleanup_error(format!(
                    "transfer `{at}` changes the classified lifecycle"
                )));
            }
            let source_flag = flag_symbol(source_leaf.flag);
            let destination_flag = flag_symbol(destination_leaf.flag);
            let source_value = storage_binding(bindings, &source.storage)?;
            let destination_value = storage_binding(bindings, &destination.storage)?;
            writeln!(
                output,
                "if (!{source_flag} || {destination_flag}) spx_runtime_invariant_failure(\"cleanup transfer liveness\");"
            )
            .expect("writing to a string cannot fail");
            let source_slot = index.slot(&source.storage).ok_or_else(|| {
                cleanup_error(format!("transfer `{at}` has no classified source slot"))
            })?;
            if matches!(source_slot.slot.ty, crate::hir::ResolvedType::Bytes) {
                writeln!(
                    output,
                    "{destination_value} = spx_bytes_move(&{source_value});"
                )
                .expect("writing to a string cannot fail");
            } else {
                writeln!(output, "{destination_value} = {source_value};")
                    .expect("writing to a string cannot fail");
            }
            writeln!(output, "{source_flag} = false;").expect("writing to a string cannot fail");
            writeln!(output, "{destination_flag} = true;")
                .expect("writing to a string cannot fail");
            emit_transfer_trace(output, index, bindings, at, source, destination)?;
        }
        CleanupTransition::AuthenticateVariantCase { at, .. } => {
            return Err(cleanup_error(format!(
                "variant authentication transition `{at}` reached the resource cleanup scaffold"
            )));
        }
        CleanupTransition::TransferVariant { at, .. } => {
            return Err(cleanup_error(format!(
                "conditional variant transfer `{at}` reached the resource cleanup scaffold"
            )));
        }
        CleanupTransition::CallCommit { call, .. } => {
            return Err(cleanup_error(format!(
                "call-commit transition `{call}` reached the cleanup scaffold"
            )));
        }
        CleanupTransition::SelectFailure { source } => {
            let status = status_binding(bindings, source)?;
            output.push_str(
                "if (spx_cleanup_selected_status != SPX_STATUS_SUCCESS) \
                 spx_runtime_invariant_failure(\"cleanup failure selection is not write-once\");\n",
            );
            writeln!(
                output,
                "if ({status} == SPX_STATUS_SUCCESS) spx_runtime_invariant_failure(\"cleanup selected a successful status\");"
            )
            .expect("writing to a string cannot fail");
            writeln!(output, "spx_cleanup_selected_status = {status};")
                .expect("writing to a string cannot fail");
            emit_select_failure_trace(output, index, bindings, source)?;
        }
        CleanupTransition::StageCopyResult { .. } => {
            return Err(cleanup_error(
                "staged Copy result reached the native owned-resource cleanup scaffold",
            ));
        }
    }
    Ok(())
}

fn emit_terminator(
    output: &mut String,
    index: &NativeCleanupIndex<'_>,
    bindings: &NativeCleanupBindings,
    owner: BlockId,
    terminator: &CleanupTerminator,
) -> Result<(), Diagnostic> {
    match terminator {
        CleanupTerminator::Goto(edge) => {
            let edge = index.edge(*edge).ok_or_else(|| {
                cleanup_error(format!(
                    "block {} references unknown edge {}",
                    owner.0, edge.0
                ))
            })?;
            if edge.from != owner || !matches!(edge.condition, EdgeCondition::Always) {
                return Err(cleanup_error(format!(
                    "block {} has a noncanonical goto edge {}",
                    owner.0, edge.id.0
                )));
            }
            writeln!(output, "goto {};", block_label(edge.to))
                .expect("writing to a string cannot fail");
        }
        CleanupTerminator::Branch(edges) => {
            if edges.is_empty() {
                return Err(cleanup_error(format!(
                    "block {} has an empty cleanup branch",
                    owner.0
                )));
            }
            for (position, edge_id) in edges.iter().enumerate() {
                let edge = index.edge(*edge_id).ok_or_else(|| {
                    cleanup_error(format!(
                        "block {} references unknown edge {}",
                        owner.0, edge_id.0
                    ))
                })?;
                if edge.from != owner {
                    return Err(cleanup_error(format!(
                        "edge {} is not owned by block {}",
                        edge.id.0, owner.0
                    )));
                }
                let condition = edge_condition(bindings, edge.id, &edge.condition)?;
                let keyword = if position == 0 { "if" } else { "else if" };
                writeln!(
                    output,
                    "{keyword} ({condition}) goto {};",
                    block_label(edge.to)
                )
                .expect("writing to a string cannot fail");
            }
            output.push_str(
                "else spx_runtime_invariant_failure(\"cleanup branch selected no edge\");\n",
            );
        }
        CleanupTerminator::Exit(exit) => {
            let indexed = index.exit(*exit).ok_or_else(|| {
                cleanup_error(format!(
                    "block {} references unknown exit {}",
                    owner.0, exit.0
                ))
            })?;
            if indexed.exit.from != owner {
                return Err(cleanup_error(format!(
                    "exit {} is not owned by block {}",
                    exit.0, owner.0
                )));
            }
            writeln!(output, "goto {};", exit_label(exit.0))
                .expect("writing to a string cannot fail");
        }
    }
    Ok(())
}

fn emit_continuation(
    output: &mut String,
    index: &NativeCleanupIndex<'_>,
    bindings: &NativeCleanupBindings,
    exit: &crate::cleanup_plan::ExitTarget,
) -> Result<(), Diagnostic> {
    match &exit.continuation {
        ExitContinuation::Continue(edge) => {
            let edge = bounded_continue_edge(index, exit, *edge)?;
            writeln!(output, "goto {};", block_label(edge.to))
                .expect("writing to a string cannot fail");
        }
        ExitContinuation::CommitResult { source } => {
            let result_out = result_out(bindings)?;
            output.push_str(
                "if (spx_cleanup_selected_status != SPX_STATUS_SUCCESS) \
                 spx_runtime_invariant_failure(\"cleanup result commit selected failure\");\n",
            );
            match source {
                CleanupResultSource::Scalar { expression } => {
                    emit_assert_all_dead(
                        output,
                        index,
                        "cleanup scalar result commit retains a live resource",
                    );
                    let value = scalar_binding(bindings, expression)?;
                    writeln!(output, "*{result_out} = {value};")
                        .expect("writing to a string cannot fail");
                }
                CleanupResultSource::Owned { storage } => {
                    if storage.storage != StorageId::ProvisionalResult {
                        return Err(cleanup_error(format!(
                            "exit {} publishes owned non-provisional storage",
                            exit.id.0
                        )));
                    }
                    let leaf = leaf_for_place(index, storage)?;
                    let flag = flag_symbol(leaf.flag);
                    let value = storage_binding(bindings, &storage.storage)?;
                    emit_assert_only_leaf_live(output, index, leaf.flag);
                    if leaf.lifecycle_id.as_str() == crate::cleanup::BYTES_DROP_LIFECYCLE_ID {
                        writeln!(output, "*{result_out} = spx_bytes_move(&{value});")
                            .expect("writing to a string cannot fail");
                    } else {
                        writeln!(output, "*{result_out} = {value};")
                            .expect("writing to a string cannot fail");
                    }
                    writeln!(output, "{flag} = false;").expect("writing to a string cannot fail");
                }
            }
            emit_result_commit_trace(output, index, bindings, source)?;
            output.push_str("return SPX_STATUS_SUCCESS;\n");
        }
        ExitContinuation::ReturnFailure { source } => {
            let status = status_binding(bindings, source)?;
            writeln!(
                output,
                "if (spx_cleanup_selected_status == SPX_STATUS_SUCCESS || spx_cleanup_selected_status != {status}) spx_runtime_invariant_failure(\"cleanup failure return changed status\");"
            )
            .expect("writing to a string cannot fail");
            emit_assert_all_dead(
                output,
                index,
                "cleanup failure return retains a live resource",
            );
            output.push_str("return spx_cleanup_selected_status;\n");
        }
        ExitContinuation::ReturnUnit => {
            return Err(cleanup_error(format!(
                "exit {} uses unsupported unit return",
                exit.id.0
            )));
        }
    }
    Ok(())
}

fn bounded_continue_edge<'a>(
    index: &'a NativeCleanupIndex<'a>,
    exit: &crate::cleanup_plan::ExitTarget,
    edge_id: crate::cleanup_plan::EdgeId,
) -> Result<&'a crate::cleanup_plan::CleanupEdge, Diagnostic> {
    let reject = |detail: &str| {
        cleanup_error(format!(
            "exit {} continuation {detail}; only the canonical empty-region success continuation is supported",
            exit.id.0
        ))
    };
    if !exit.finalize_in_order.is_empty() || exit.leaves_regions.is_empty() {
        return Err(reject("performs cleanup or leaves no region"));
    }
    let source = index
        .block(exit.from)
        .ok_or_else(|| reject("has an unknown source block"))?;
    if !source.transitions.is_empty() || source.block.terminator != CleanupTerminator::Exit(exit.id)
    {
        return Err(reject("changes state before continuing"));
    }
    let edge = index
        .edge(edge_id)
        .ok_or_else(|| reject("references an unknown edge"))?;
    if edge.from != exit.from || !matches!(edge.condition, EdgeCondition::Always) {
        return Err(reject("does not own one unconditional edge"));
    }
    let incoming = index
        .edges()
        .iter()
        .filter(|candidate| candidate.to == source.block.id)
        .collect::<Vec<_>>();
    if incoming.len() != 1
        || !matches!(
            incoming[0].condition,
            EdgeCondition::BooleanResult(_, true) | EdgeCondition::StatusZero(_)
        )
    {
        return Err(reject("is not reached by one successful checked branch"));
    }

    let mut expected_region = Some(source.block.region);
    for region_id in &exit.leaves_regions {
        if expected_region != Some(*region_id) {
            return Err(reject("does not leave one contiguous region chain"));
        }
        let region = index
            .regions()
            .iter()
            .find(|region| region.id == *region_id)
            .ok_or_else(|| reject("references an unknown region"))?;
        if !region.slots.is_empty() || region.normal_scope_end != exit.id {
            return Err(reject("leaves a resource-owning or non-normal region"));
        }
        expected_region = region.parent;
    }
    let Some(parent_region) = expected_region else {
        return Err(reject("escapes the root region"));
    };
    let target = index
        .block(edge.to)
        .ok_or_else(|| reject("targets an unknown block"))?;
    if target.block.region != parent_region
        || index
            .edges()
            .iter()
            .filter(|candidate| candidate.to == target.block.id)
            .count()
            != 1
        || index
            .exits()
            .iter()
            .filter(|candidate| {
                matches!(candidate.exit.continuation, ExitContinuation::Continue(id) if id == edge_id)
            })
            .count()
            != 1
    {
        return Err(reject("does not have one target in the surviving region"));
    }
    Ok(edge)
}

fn emit_assert_all_dead(output: &mut String, index: &NativeCleanupIndex<'_>, message: &str) {
    for leaf in index.leaves() {
        writeln!(
            output,
            "if ({}) spx_runtime_invariant_failure(\"{message}\");",
            flag_symbol(leaf.flag)
        )
        .expect("writing to a string cannot fail");
    }
}

fn emit_assert_only_leaf_live(
    output: &mut String,
    index: &NativeCleanupIndex<'_>,
    live_flag: LivenessFlagId,
) {
    for leaf in index.leaves() {
        let flag = flag_symbol(leaf.flag);
        if leaf.flag == live_flag {
            writeln!(
                output,
                "if (!{flag}) spx_runtime_invariant_failure(\"cleanup publishes a dead owned result\");"
            )
            .expect("writing to a string cannot fail");
        } else {
            writeln!(
                output,
                "if ({flag}) spx_runtime_invariant_failure(\"cleanup owned result commit retains another live resource\");"
            )
            .expect("writing to a string cannot fail");
        }
    }
}

fn edge_condition(
    bindings: &NativeCleanupBindings,
    edge: crate::cleanup_plan::EdgeId,
    condition: &EdgeCondition,
) -> Result<String, Diagnostic> {
    match condition {
        EdgeCondition::Always => Err(cleanup_error(
            "unconditional edge reached cleanup branch emission",
        )),
        EdgeCondition::BooleanResult(expression, expected) => {
            let value = bindings.boolean_values.get(expression).ok_or_else(|| {
                cleanup_error(format!(
                    "missing boolean binding for expression `{expression}`"
                ))
            })?;
            Ok(if *expected {
                value.clone()
            } else {
                format!("!{value}")
            })
        }
        EdgeCondition::StatusZero(source) => Ok(format!(
            "{} == SPX_STATUS_SUCCESS",
            status_binding(bindings, source)?
        )),
        EdgeCondition::StatusNonzero(source) => Ok(format!(
            "{} != SPX_STATUS_SUCCESS",
            status_binding(bindings, source)?
        )),
        EdgeCondition::VariantCase { .. } | EdgeCondition::ArmSelected { .. } => {
            bindings.decision_edges.get(&edge).cloned().ok_or_else(|| {
                cleanup_error("decision-chain edge has no exact observation binding")
            })
        }
    }
}

fn validate_bindings(
    index: &NativeCleanupIndex<'_>,
    bindings: &NativeCleanupBindings,
) -> Result<(), Diagnostic> {
    if bindings.context != "spx_bind_context" {
        return Err(cleanup_error(
            "the trace context binding must be exactly `spx_bind_context`",
        ));
    }
    if let Some(dictionary) = &bindings.semantic_events {
        if dictionary.schema() != SEMANTIC_EVENT_DICTIONARY_V1
            || dictionary.function() != index.function_id()
        {
            return Err(cleanup_error(format!(
                "semantic event dictionary identity disagrees with function `{}`",
                index.function_id()
            )));
        }
    }
    let expected_storage = index
        .slots()
        .iter()
        .map(|slot| slot.slot.storage.clone())
        .collect::<BTreeSet<_>>();
    require_exact_keys(
        &expected_storage,
        bindings.storage_values.keys().cloned().collect(),
        "storage",
    )?;

    let mut expected_booleans = BTreeSet::new();
    let mut expected_statuses = BTreeSet::new();
    for edge in index.edges() {
        match &edge.condition {
            EdgeCondition::BooleanResult(expression, _) => {
                expected_booleans.insert(expression.clone());
            }
            EdgeCondition::StatusZero(source) | EdgeCondition::StatusNonzero(source) => {
                expected_statuses.insert(source.clone());
            }
            EdgeCondition::VariantCase { .. } | EdgeCondition::ArmSelected { .. } => {}
            EdgeCondition::Always => {}
        }
    }
    let mut expected_scalars = BTreeSet::new();
    let mut publishes_result = false;
    for block in index.blocks() {
        for transition in block.transitions {
            match transition {
                CleanupTransition::SelectFailure { source } => {
                    expected_statuses.insert(source.clone());
                }
                CleanupTransition::CallCommit { .. }
                | CleanupTransition::Initialize { .. }
                | CleanupTransition::InitializeVariant { .. }
                | CleanupTransition::Transfer { .. }
                | CleanupTransition::TransferVariant { .. }
                | CleanupTransition::AuthenticateVariantCase { .. } => {}
                CleanupTransition::StageCopyResult { .. } => {
                    return Err(cleanup_error(
                        "staged Copy result reached owned-resource binding preflight",
                    ));
                }
            }
        }
    }
    for indexed in index.exits() {
        match &indexed.exit.continuation {
            ExitContinuation::Continue(edge) => {
                bounded_continue_edge(index, indexed.exit, *edge)?;
            }
            ExitContinuation::CommitResult { source } => {
                publishes_result = true;
                if let CleanupResultSource::Scalar { expression } = source {
                    expected_scalars.insert(expression.clone());
                }
            }
            ExitContinuation::ReturnFailure { source } => {
                expected_statuses.insert(source.clone());
            }
            ExitContinuation::ReturnUnit => {
                return Err(cleanup_error(format!(
                    "exit {} uses unsupported unit return",
                    indexed.exit.id.0
                )));
            }
        }
    }
    require_exact_keys(
        &expected_booleans,
        bindings.boolean_values.keys().cloned().collect(),
        "boolean",
    )?;
    let expected_decisions = index
        .edges()
        .iter()
        .filter(|edge| {
            matches!(
                edge.condition,
                EdgeCondition::VariantCase { .. } | EdgeCondition::ArmSelected { .. }
            )
        })
        .map(|edge| edge.id)
        .collect::<BTreeSet<_>>();
    require_exact_keys(
        &expected_decisions,
        bindings.decision_edges.keys().copied().collect(),
        "decision edge",
    )?;
    require_exact_keys(
        &expected_statuses,
        bindings.status_tokens.keys().cloned().collect(),
        "status",
    )?;
    require_exact_keys(
        &expected_scalars,
        bindings.scalar_results.keys().cloned().collect(),
        "scalar result",
    )?;
    if publishes_result != bindings.result_out.is_some() {
        return Err(cleanup_error(if publishes_result {
            "missing caller result-out binding"
        } else {
            "unexpected caller result-out binding"
        }));
    }

    let mut physical_identifiers = BTreeSet::new();
    for identifier in all_binding_identifiers(bindings) {
        if !is_c_identifier(identifier) {
            return Err(cleanup_error(format!(
                "binding `{identifier}` is not one C identifier"
            )));
        }
        if is_c_keyword(identifier) {
            return Err(cleanup_error(format!(
                "binding `{identifier}` is a reserved C keyword"
            )));
        }
        if is_reserved_binding_identifier(identifier) {
            return Err(cleanup_error(format!(
                "binding `{identifier}` is reserved by C or the SEMAPRAX compiler/runtime"
            )));
        }
        if identifier
            .strip_prefix("spx_bind_")
            .is_none_or(str::is_empty)
        {
            return Err(cleanup_error(format!(
                "binding `{identifier}` is outside the dedicated `spx_bind_` namespace"
            )));
        }
        if !physical_identifiers.insert(identifier) {
            return Err(cleanup_error(format!(
                "binding `{identifier}` aliases two cleanup scaffold inputs"
            )));
        }
    }
    Ok(())
}

fn all_binding_identifiers(bindings: &NativeCleanupBindings) -> impl Iterator<Item = &str> {
    std::iter::once(bindings.context.as_str()).chain(
        bindings
            .storage_values
            .values()
            .chain(bindings.boolean_values.values())
            .chain(bindings.status_tokens.values())
            .chain(bindings.scalar_results.values())
            .chain(bindings.result_out.iter())
            .chain(bindings.decision_edges.values())
            .map(String::as_str),
    )
}

fn require_exact_keys<T: Ord + std::fmt::Debug>(
    expected: &BTreeSet<T>,
    actual: BTreeSet<T>,
    kind: &str,
) -> Result<(), Diagnostic> {
    if let Some(missing) = expected.difference(&actual).next() {
        return Err(cleanup_error(format!(
            "missing {kind} binding for `{missing:?}`"
        )));
    }
    if let Some(extra) = actual.difference(expected).next() {
        return Err(cleanup_error(format!(
            "unexpected {kind} binding for `{extra:?}`"
        )));
    }
    Ok(())
}

fn leaf_for_place<'a>(
    index: &'a NativeCleanupIndex<'a>,
    place: &CleanupPlace,
) -> Result<&'a NativeCleanupLeaf<'a>, Diagnostic> {
    if !place.projections.is_empty() {
        return Err(cleanup_error(
            "projected cleanup place reached direct-resource emission",
        ));
    }
    let slot = index.slot(&place.storage).ok_or_else(|| {
        cleanup_error(format!(
            "cleanup place references unknown storage `{:?}`",
            place.storage
        ))
    })?;
    if slot.leaf.place != *place {
        return Err(cleanup_error(
            "cleanup place disagrees with its classified slot",
        ));
    }
    Ok(&slot.leaf)
}

fn storage_binding<'a>(
    bindings: &'a NativeCleanupBindings,
    storage: &StorageId,
) -> Result<&'a str, Diagnostic> {
    bindings
        .storage_values
        .get(storage)
        .map(String::as_str)
        .ok_or_else(|| cleanup_error(format!("missing storage binding for `{storage:?}`")))
}

fn status_binding<'a>(
    bindings: &'a NativeCleanupBindings,
    source: &StatusSourceId,
) -> Result<&'a str, Diagnostic> {
    bindings
        .status_tokens
        .get(source)
        .map(String::as_str)
        .ok_or_else(|| cleanup_error(format!("missing status binding for `{source:?}`")))
}

fn scalar_binding<'a>(
    bindings: &'a NativeCleanupBindings,
    expression: &ExpressionId,
) -> Result<&'a str, Diagnostic> {
    bindings
        .scalar_results
        .get(expression)
        .map(String::as_str)
        .ok_or_else(|| cleanup_error(format!("missing scalar result binding for `{expression}`")))
}

fn result_out(bindings: &NativeCleanupBindings) -> Result<&str, Diagnostic> {
    bindings
        .result_out
        .as_deref()
        .ok_or_else(|| cleanup_error("missing caller result-out binding"))
}

fn flag_symbol(flag: LivenessFlagId) -> String {
    format!("spx_cleanup_flag_{}", flag.0)
}

fn block_label(block: BlockId) -> String {
    format!("spx_cleanup_block_{}", block.0)
}

fn exit_label(exit: u32) -> String {
    format!("spx_cleanup_exit_{exit}")
}

fn is_c_identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    (first == b'_' || first.is_ascii_alphabetic())
        && bytes.all(|byte| byte == b'_' || byte.is_ascii_alphanumeric())
}

fn is_c_keyword(value: &str) -> bool {
    matches!(
        value,
        "_Alignas"
            | "_Alignof"
            | "_Atomic"
            | "_Bool"
            | "_Complex"
            | "_Generic"
            | "_Imaginary"
            | "_Noreturn"
            | "_Static_assert"
            | "_Thread_local"
            | "auto"
            | "break"
            | "case"
            | "char"
            | "const"
            | "continue"
            | "default"
            | "do"
            | "double"
            | "else"
            | "enum"
            | "extern"
            | "float"
            | "for"
            | "goto"
            | "if"
            | "inline"
            | "int"
            | "long"
            | "register"
            | "restrict"
            | "return"
            | "short"
            | "signed"
            | "sizeof"
            | "static"
            | "struct"
            | "switch"
            | "typedef"
            | "union"
            | "unsigned"
            | "void"
            | "volatile"
            | "while"
    )
}

fn is_reserved_binding_identifier(value: &str) -> bool {
    matches!(
        value,
        "bool" | "true" | "false" | "NULL" | "SPX_STATUS_SUCCESS"
    ) || value.starts_with("__")
        || value
            .strip_prefix('_')
            .and_then(|rest| rest.bytes().next())
            .is_some_and(|byte| byte.is_ascii_uppercase())
        || (value.starts_with("spx_") && !value.starts_with("spx_bind_"))
        || value.starts_with("SPX_")
}

fn c_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match *byte {
            b'\\' => escaped.push_str("\\\\"),
            b'"' => escaped.push_str("\\\""),
            b'\n' => escaped.push_str("\\n"),
            b'\r' => escaped.push_str("\\r"),
            b'\t' => escaped.push_str("\\t"),
            b'?' | 0x00..=0x1f | 0x7f..=0xff => {
                write!(escaped, "\\{byte:03o}").expect("writing to a string cannot fail");
            }
            value => escaped.push(char::from(value)),
        }
    }
    escaped
}

fn cleanup_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io(
        "SPX-B104",
        format!("native cleanup scaffold: {}", message.into()),
    )
}

#[cfg(test)]
#[path = "native_cleanup_emit/tests.rs"]
mod tests;
