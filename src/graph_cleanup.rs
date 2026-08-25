//! Deterministic Graph v10 serialization for verified cleanup plans.
//!
//! This module is intentionally a renderer, not a validator or canonicalizer.
//! Every vector is emitted in the order supplied by [`CleanupPlan`]. Sorting,
//! deduplicating, or otherwise repairing malformed HIR here could make invalid
//! compiler input appear canonical before the cleanup replay boundary rejects
//! it.

use crate::cleanup::{FieldLiveness, FieldLivenessShape};
use crate::cleanup_plan::{
    CallArgumentTransfer, CheckedOperation, CleanupBlock, CleanupEdge, CleanupEntryState,
    CleanupPlace, CleanupPlan, CleanupRegion, CleanupResultSource, CleanupSlot, CleanupTerminator,
    CleanupTransition, ContractPhase, EdgeCondition, ExitContinuation, ExitTarget, FinalizeAction,
    StagedCopyResultSource, StatusCase, StatusLane, StatusProducer, StatusSource, StatusSourceId,
    StorageId,
};
use crate::diagnostic::quote_json;
use crate::hir::ResolvedType;

macro_rules! format {
    ($($argument:tt)*) => {
        crate::bounded_output::budgeted_format(format_args!($($argument)*))
    };
}

/// Serialize one already-validated cleanup plan for embedding in Graph v10.
///
/// Callers must validate the enclosing resolved program before invoking this
/// function. Keeping validation out of the renderer makes it impossible for
/// serialization to normalize malformed ordering into an apparently valid
/// plan.
pub(crate) fn cleanup_plan_json(plan: &CleanupPlan) -> String {
    format!(
        "{{\"kind\":\"cleanup_plan\",\"schema\":{},\"entry\":{},\"entry_state\":{},\"slots\":{},\"status_sources\":{},\"blocks\":{},\"edges\":{},\"regions\":{},\"exits\":{}}}",
        quote_json(plan.schema),
        plan.entry.0,
        entry_state_json(&plan.entry_state),
        array_json(&plan.slots, slot_json),
        array_json(&plan.status_sources, status_source_json),
        array_json(&plan.blocks, block_json),
        array_json(&plan.edges, edge_json),
        array_json(&plan.regions, region_json),
        array_json(&plan.exits, exit_json),
    )
}

fn entry_state_json(state: &CleanupEntryState) -> String {
    format!(
        "{{\"kind\":\"entry_state\",\"live_owned_parameters\":{}}}",
        array_json(&state.live_owned_parameters, place_json)
    )
}

fn slot_json(slot: &CleanupSlot) -> String {
    format!(
        "{{\"kind\":\"cleanup_slot\",\"id\":{},\"storage\":{},\"type\":{},\"storage_index\":{},\"field_liveness_shape\":{}}}",
        slot.id.0,
        storage_json(&slot.storage),
        type_json(&slot.ty),
        slot.storage_index,
        liveness_shape_json(&slot.field_liveness_shape),
    )
}

fn storage_json(storage: &StorageId) -> String {
    match storage {
        StorageId::Value(value) => format!(
            "{{\"kind\":\"value\",\"value\":{}}}",
            quote_json(value.as_str())
        ),
        StorageId::Temporary(expression) => format!(
            "{{\"kind\":\"temporary\",\"expression\":{}}}",
            quote_json(expression.as_str())
        ),
        StorageId::CallArgument {
            call,
            parameter_index,
            value_expression,
        } => format!(
            "{{\"kind\":\"call_argument\",\"call\":{},\"parameter_index\":{},\"value_expression\":{}}}",
            quote_json(call.as_str()),
            parameter_index,
            quote_json(value_expression.as_str())
        ),
        StorageId::ProvisionalResult => {
            "{\"kind\":\"provisional_result\"}".to_owned()
        }
    }
}

fn place_json(place: &CleanupPlace) -> String {
    format!(
        "{{\"kind\":\"cleanup_place\",\"storage\":{},\"projections\":{}}}",
        storage_json(&place.storage),
        array_json(&place.projections, |projection| quote_json(
            projection.as_str()
        )),
    )
}

fn liveness_shape_json(shape: &FieldLivenessShape) -> String {
    match shape {
        FieldLivenessShape::NoDrop => "{\"kind\":\"no_drop\"}".to_owned(),
        FieldLivenessShape::Leaf { flag, lifecycle } => format!(
            "{{\"kind\":\"leaf\",\"flag\":{},\"lifecycle\":{}}}",
            flag.0,
            quote_json(lifecycle.as_str())
        ),
        FieldLivenessShape::Record {
            declaration,
            fields,
        } => format!(
            "{{\"kind\":\"record\",\"declaration\":{},\"fields\":{}}}",
            quote_json(declaration.as_str()),
            array_json(fields, field_liveness_json)
        ),
    }
}

fn field_liveness_json(field: &FieldLiveness) -> String {
    format!(
        "{{\"kind\":\"field_liveness\",\"field\":{},\"field_index\":{},\"shape\":{}}}",
        quote_json(field.field.as_str()),
        field.field_index,
        liveness_shape_json(&field.shape),
    )
}

fn status_source_id_json(source: &StatusSourceId) -> String {
    format!(
        "{{\"kind\":\"status_source_id\",\"expression\":{},\"lane\":{}}}",
        quote_json(source.expression.as_str()),
        quote_json(status_lane_text(source.lane)),
    )
}

fn status_source_json(source: &StatusSource) -> String {
    format!(
        "{{\"kind\":\"status_source\",\"id\":{},\"producer\":{}}}",
        status_source_id_json(&source.id),
        status_producer_json(&source.producer),
    )
}

fn status_producer_json(producer: &StatusProducer) -> String {
    match producer {
        StatusProducer::PropagatedCall { callee } => format!(
            "{{\"kind\":\"propagated_call\",\"callee\":{}}}",
            quote_json(callee.as_str())
        ),
        StatusProducer::CheckedArithmetic {
            operation,
            normalized_cases,
        } => format!(
            "{{\"kind\":\"checked_arithmetic\",\"operation\":{},\"normalized_cases\":{}}}",
            quote_json(checked_operation_text(*operation)),
            array_json(normalized_cases, status_case_json)
        ),
        StatusProducer::ContractFalse { phase, ordinal } => format!(
            "{{\"kind\":\"contract_false\",\"phase\":{},\"ordinal\":{}}}",
            quote_json(contract_phase_text(*phase)),
            ordinal
        ),
    }
}

fn status_case_json(case: &StatusCase) -> String {
    format!(
        "{{\"kind\":{},\"code\":{}}}",
        quote_json(status_case_text(*case)),
        case.code()
    )
}

fn block_json(block: &CleanupBlock) -> String {
    format!(
        "{{\"kind\":\"cleanup_block\",\"id\":{},\"region\":{},\"transitions\":{},\"terminator\":{}}}",
        block.id.0,
        block.region.0,
        array_json(&block.transitions, transition_json),
        terminator_json(&block.terminator),
    )
}

fn transition_json(transition: &CleanupTransition) -> String {
    match transition {
        CleanupTransition::Initialize { at, destination } => format!(
            "{{\"kind\":\"initialize\",\"at\":{},\"destination\":{}}}",
            quote_json(at.as_str()),
            place_json(destination)
        ),
        CleanupTransition::Transfer {
            at,
            source,
            destination,
        } => format!(
            "{{\"kind\":\"transfer\",\"at\":{},\"source\":{},\"destination\":{}}}",
            quote_json(at.as_str()),
            place_json(source),
            place_json(destination)
        ),
        CleanupTransition::CallCommit { call, arguments } => format!(
            "{{\"kind\":\"call_commit\",\"call\":{},\"arguments\":{}}}",
            quote_json(call.as_str()),
            array_json(arguments, call_argument_transfer_json)
        ),
        CleanupTransition::SelectFailure { source } => format!(
            "{{\"kind\":\"select_failure\",\"source\":{}}}",
            status_source_id_json(source)
        ),
        CleanupTransition::StageCopyResult { source } => format!(
            "{{\"kind\":\"stage_copy_result\",\"source\":{}}}",
            staged_copy_result_source_json(source)
        ),
    }
}

fn staged_copy_result_source_json(source: &StagedCopyResultSource) -> String {
    match source {
        StagedCopyResultSource::Body {
            expression,
            instance,
        } => format!(
            "{{\"kind\":\"body\",\"expression\":{},\"instance\":{}}}",
            quote_json(expression.as_str()),
            type_json(instance)
        ),
        StagedCopyResultSource::TryResidual {
            expression,
            operand,
            source_instance,
            target_instance,
            result,
            ok_case,
            ok_field,
            err_case,
            err_field,
        } => format!(
            "{{\"kind\":\"try_residual\",\"expression\":{},\"operand\":{},\"source_instance\":{},\"target_instance\":{},\"result\":{},\"ok_case\":{},\"ok_field\":{},\"err_case\":{},\"err_field\":{}}}",
            quote_json(expression.as_str()),
            quote_json(operand.as_str()),
            type_json(source_instance),
            type_json(target_instance),
            quote_json(result.as_str()),
            quote_json(ok_case.as_str()),
            quote_json(ok_field.as_str()),
            quote_json(err_case.as_str()),
            quote_json(err_field.as_str()),
        ),
        StagedCopyResultSource::TryOptionNone {
            expression,
            operand,
            source_instance,
            target_instance,
            option,
            some_case,
            some_field,
            none_case,
        } => format!(
            "{{\"kind\":\"try_option_none\",\"expression\":{},\"operand\":{},\"source_instance\":{},\"target_instance\":{},\"option\":{},\"some_case\":{},\"some_field\":{},\"none_case\":{}}}",
            quote_json(expression.as_str()),
            quote_json(operand.as_str()),
            type_json(source_instance),
            type_json(target_instance),
            quote_json(option.as_str()),
            quote_json(some_case.as_str()),
            quote_json(some_field.as_str()),
            quote_json(none_case.as_str()),
        ),
    }
}

fn call_argument_transfer_json(argument: &CallArgumentTransfer) -> String {
    format!(
        "{{\"kind\":\"call_argument_transfer\",\"parameter_index\":{},\"source\":{}}}",
        argument.parameter_index,
        place_json(&argument.source),
    )
}

fn terminator_json(terminator: &CleanupTerminator) -> String {
    match terminator {
        CleanupTerminator::Goto(edge) => {
            format!("{{\"kind\":\"goto\",\"edge\":{}}}", edge.0)
        }
        CleanupTerminator::Branch(edges) => format!(
            "{{\"kind\":\"branch\",\"edges\":{}}}",
            array_json(edges, |edge| edge.0.to_string())
        ),
        CleanupTerminator::Exit(exit) => {
            format!("{{\"kind\":\"exit\",\"target\":{}}}", exit.0)
        }
    }
}

fn edge_json(edge: &CleanupEdge) -> String {
    format!(
        "{{\"kind\":\"cleanup_edge\",\"id\":{},\"from\":{},\"to\":{},\"condition\":{}}}",
        edge.id.0,
        edge.from.0,
        edge.to.0,
        edge_condition_json(&edge.condition),
    )
}

fn edge_condition_json(condition: &EdgeCondition) -> String {
    match condition {
        EdgeCondition::Always => "{\"kind\":\"always\"}".to_owned(),
        EdgeCondition::BooleanResult(expression, value) => format!(
            "{{\"kind\":\"boolean_result\",\"expression\":{},\"value\":{}}}",
            quote_json(expression.as_str()),
            value
        ),
        EdgeCondition::VariantCase {
            scrutinee,
            case,
            matches,
        } => format!(
            "{{\"kind\":\"variant_case\",\"scrutinee\":{},\"case\":{},\"matches\":{}}}",
            quote_json(scrutinee.as_str()),
            quote_json(case.as_str()),
            matches
        ),
        EdgeCondition::ArmSelected {
            scrutinee,
            arm,
            selected,
        } => format!(
            "{{\"kind\":\"arm_selected\",\"scrutinee\":{},\"arm\":{arm},\"selected\":{selected}}}",
            quote_json(scrutinee.as_str())
        ),
        EdgeCondition::StatusZero(source) => format!(
            "{{\"kind\":\"status_zero\",\"source\":{}}}",
            status_source_id_json(source)
        ),
        EdgeCondition::StatusNonzero(source) => format!(
            "{{\"kind\":\"status_nonzero\",\"source\":{}}}",
            status_source_id_json(source)
        ),
    }
}

fn region_json(region: &CleanupRegion) -> String {
    let parent = region
        .parent
        .map_or_else(|| "null".to_owned(), |parent| parent.0.to_string());
    format!(
        "{{\"kind\":\"cleanup_region\",\"id\":{},\"parent\":{},\"slots\":{},\"normal_scope_end\":{}}}",
        region.id.0,
        parent,
        array_json(&region.slots, storage_json),
        region.normal_scope_end.0,
    )
}

fn exit_json(exit: &ExitTarget) -> String {
    format!(
        "{{\"kind\":\"exit_target\",\"id\":{},\"from\":{},\"leaves_regions\":{},\"finalize_in_order\":{},\"continuation\":{}}}",
        exit.id.0,
        exit.from.0,
        array_json(&exit.leaves_regions, |region| region.0.to_string()),
        array_json(&exit.finalize_in_order, finalize_action_json),
        exit_continuation_json(&exit.continuation),
    )
}

fn finalize_action_json(action: &FinalizeAction) -> String {
    format!(
        "{{\"kind\":\"finalize\",\"source\":{},\"lifecycle_id\":{},\"guard_flag\":{}}}",
        place_json(&action.source),
        quote_json(action.lifecycle_id.as_str()),
        action.guard_flag.0,
    )
}

fn exit_continuation_json(continuation: &ExitContinuation) -> String {
    match continuation {
        ExitContinuation::Continue(edge) => {
            format!("{{\"kind\":\"continue\",\"edge\":{}}}", edge.0)
        }
        ExitContinuation::CommitResult { source } => format!(
            "{{\"kind\":\"commit_result\",\"source\":{}}}",
            result_source_json(source)
        ),
        ExitContinuation::ReturnFailure { source } => format!(
            "{{\"kind\":\"return_failure\",\"source\":{}}}",
            status_source_id_json(source)
        ),
        ExitContinuation::ReturnUnit => "{\"kind\":\"return_unit\"}".to_owned(),
    }
}

fn result_source_json(source: &CleanupResultSource) -> String {
    match source {
        CleanupResultSource::Scalar { expression } => format!(
            "{{\"kind\":\"scalar\",\"expression\":{}}}",
            quote_json(expression.as_str())
        ),
        CleanupResultSource::Owned { storage } => {
            format!("{{\"kind\":\"owned\",\"storage\":{}}}", place_json(storage))
        }
    }
}

fn type_json(ty: &ResolvedType) -> String {
    match ty {
        ResolvedType::Unit => "{\"kind\":\"primitive\",\"name\":\"unit\"}".to_owned(),
        ResolvedType::I64 => "{\"kind\":\"primitive\",\"name\":\"i64\"}".to_owned(),
        ResolvedType::I32 => "{\"kind\":\"primitive\",\"name\":\"i32\"}".to_owned(),
        ResolvedType::Char => "{\"kind\":\"primitive\",\"name\":\"char\"}".to_owned(),
        ResolvedType::U8 => "{\"kind\":\"primitive\",\"name\":\"u8\"}".to_owned(),
        ResolvedType::Usize => "{\"kind\":\"primitive\",\"name\":\"usize\"}".to_owned(),
        ResolvedType::ArrayU8(length) => format!(
            "{{\"element\":{{\"kind\":\"primitive\",\"name\":\"u8\"}},\"kind\":\"fixed_array\",\"length\":{length}}}"
        ),
        ResolvedType::F32 => "{\"kind\":\"primitive\",\"name\":\"f32\"}".to_owned(),
        ResolvedType::F64 => "{\"kind\":\"primitive\",\"name\":\"f64\"}".to_owned(),
        ResolvedType::String => "{\"kind\":\"primitive\",\"name\":\"string\"}".to_owned(),
        ResolvedType::Bytes => "{\"kind\":\"owned_bytes\"}".to_owned(),
        ResolvedType::Str => "{\"kind\":\"primitive\",\"name\":\"str\"}".to_owned(),
        ResolvedType::SliceU8 => {
            "{\"element\":{\"kind\":\"primitive\",\"name\":\"u8\"},\"kind\":\"borrowed_slice\"}"
                .to_owned()
        }
        ResolvedType::Bool => "{\"kind\":\"primitive\",\"name\":\"bool\"}".to_owned(),
        ResolvedType::TypeParameter { owner, index } => format!(
            "{{\"kind\":\"type_parameter\",\"owner\":{},\"index\":{}}}",
            quote_json(owner.as_str()),
            index
        ),
        ResolvedType::Nominal {
            declaration,
            arguments,
        } => format!(
            "{{\"kind\":\"nominal\",\"declaration\":{},\"arguments\":{}}}",
            quote_json(declaration.as_str()),
            array_json(arguments, type_json)
        ),
    }
}

fn status_lane_text(lane: StatusLane) -> &'static str {
    match lane {
        StatusLane::OperationFailure => "operation_failure",
        StatusLane::ContractFalse => "contract_false",
    }
}

fn checked_operation_text(operation: CheckedOperation) -> &'static str {
    match operation {
        CheckedOperation::Neg => "neg",
        CheckedOperation::Add => "add",
        CheckedOperation::Sub => "sub",
        CheckedOperation::Mul => "mul",
        CheckedOperation::Div => "div",
        CheckedOperation::Rem => "rem",
    }
}

fn status_case_text(case: StatusCase) -> &'static str {
    match case {
        StatusCase::AddOverflow => "add_overflow",
        StatusCase::SubOverflow => "sub_overflow",
        StatusCase::MulOverflow => "mul_overflow",
        StatusCase::DivisionByZero => "division_by_zero",
        StatusCase::DivisionOverflow => "division_overflow",
        StatusCase::RemainderByZero => "remainder_by_zero",
        StatusCase::RemainderOverflow => "remainder_overflow",
        StatusCase::NegationOverflow => "negation_overflow",
    }
}

fn contract_phase_text(phase: ContractPhase) -> &'static str {
    match phase {
        ContractPhase::Requires => "requires",
        ContractPhase::Ensures => "ensures",
    }
}

fn array_json<T>(values: &[T], mut render: impl FnMut(&T) -> String) -> String {
    let items = values.iter().map(&mut render).collect::<Vec<_>>();
    format!("[{}]", crate::bounded_output::budgeted_join(items, ","))
}
