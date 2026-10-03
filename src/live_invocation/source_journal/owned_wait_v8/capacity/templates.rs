//! Maximum inert values derived from the admitted checked compiler profile.
use super::super::*;
use crate::hir::{DeclarationId, ResolvedType};
use crate::interpreter::resumable::{channel, checkpoint, ResumableChannelValue};
use crate::interpreter::ArgumentValue;
use crate::resumable_effects::owned_frame::v2 as checked;
use serde_json::json;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Maxima {
    pub state: Value,
    pub proposal: Value,
    pub observation: Value,
    pub decision: Value,
    pub state_operations: Value,
    pub result_operations: Value,
    pub decision_operations: Value,
    pub effect_operations: Value,
    pub partial_operations: Value,
    pub terminal: Value,
}

// The maxima are derived only from the immutable checked wait binding. Keep
// the proof object with the result so an equal-looking reconstructed binding
// cannot reuse this capacity proof.
#[derive(Default)]
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct MaximaTemplateCacheV8 {
    pub(super) entry: std::cell::RefCell<
        Option<(
            std::sync::Arc<
                crate::resumable_effects::owned_frame::v2::CheckedOwnedAgentWaitBindingV8,
            >,
            Result<Maxima, SourceJournalError>,
        )>,
    >,
}
fn binding<T>() -> Result<T, SourceJournalError> {
    Err(SourceJournalError::Binding)
}
fn scalar(ty: &ResolvedType) -> Result<ArgumentValue, SourceJournalError> {
    Ok(match ty {
        ResolvedType::Bool => ArgumentValue::Bool(false),
        ResolvedType::I32 => ArgumentValue::Int32(i32::MIN),
        ResolvedType::I64 => ArgumentValue::Int(i64::MIN),
        ResolvedType::U8 => ArgumentValue::Uint8(u8::MAX),
        ResolvedType::Usize => ArgumentValue::Usize(u32::MAX as u64),
        ResolvedType::Char => ArgumentValue::Char(0x10ffff),
        ResolvedType::F32 => ArgumentValue::Float32(f32::from_bits(u32::MAX)),
        ResolvedType::F64 => ArgumentValue::Float64(f64::from_bits(u64::MAX)),
        _ => return binding(),
    })
}
fn fields<'a>(
    expected: impl ExactSizeIterator<Item = (&'a DeclarationId, &'a ResolvedType)>,
) -> Result<Value, SourceJournalError> {
    if expected.len() > 8 {
        return binding();
    }
    Ok(Value::Array(
        expected
            .map(|(id, ty)| {
                let value = if *ty == ResolvedType::Bytes {
                    json!({"kind":"bytes","hex":"ff".repeat(1024)})
                } else {
                    checkpoint::scalar_json(&scalar(ty)?)
                };
                Ok(json!({"identity":id.as_str(),"value":value}))
            })
            .collect::<Result<Vec<_>, SourceJournalError>>()?,
    ))
}
fn copy_channel(context: &FoldContextV8, request: bool) -> Result<Value, SourceJournalError> {
    let helper = context.checked_binding.helper();
    let yields = helper
        .function()
        .yields
        .as_ref()
        .ok_or(SourceJournalError::Binding)?;
    let ty = if request {
        &yields.request_type
    } else {
        &yields.response_type
    };
    let id = ty.nominal_id().ok_or(SourceJournalError::Binding)?;
    let declared = helper
        .program()
        .declarations
        .record_fields(id)
        .ok_or(SourceJournalError::Binding)?;
    if declared.len() > 8 {
        return binding();
    }
    let carrier = ResumableChannelValue::Record {
        declaration: id.clone(),
        fields: declared
            .iter()
            .map(|f| scalar(&f.ty))
            .collect::<Result<Vec<_>, _>>()?,
    };
    let valid = if request {
        channel::valid_copy_channel_request(
            helper.program(),
            helper.function().id.as_str(),
            &carrier,
        )
    } else {
        channel::valid_copy_channel_response(
            helper.program(),
            helper.function().id.as_str(),
            &carrier,
        )
    };
    if !valid {
        return binding();
    }
    Ok(checkpoint::channel_json(&carrier))
}
fn largest_terminal(context: &FoldContextV8) -> Result<Value, SourceJournalError> {
    let mut largest = Value::Null;
    for failure in [
        "fuel_exhausted",
        "host_abandoned",
        "answer_type_mismatch",
        "evaluation_rejected",
        "handler_failed",
        "call_depth_exceeded",
    ] {
        let value = json!({"failure":failure,"language_status":null});
        if wire::canonical(&value).len() > wire::canonical(&largest).len() {
            largest = value;
        }
    }
    for f in [
        context.checked_binding.helper().function(),
        context.checked_binding.observe().function(),
        context.checked_binding.authorize().function(),
    ] {
        for source in &f.cleanup_plan.status_sources {
            use crate::cleanup_plan::StatusProducer;
            let statuses = match &source.producer {
                StatusProducer::ContractFalse { phase, .. } => {
                    vec![crate::conformance::NormalizedStatus::contract(*phase)]
                }
                StatusProducer::CheckedArithmetic {
                    normalized_cases, ..
                } => normalized_cases
                    .iter()
                    .map(|v| crate::conformance::NormalizedStatus::arithmetic(*v))
                    .collect(),
                StatusProducer::PropagatedCall { .. } => Vec::new(),
            };
            for status in statuses {
                let value = json!({"failure":"language_failure","language_status":wire::parse(status.to_json().as_bytes())?});
                if wire::canonical(&value).len() > wire::canonical(&largest).len() {
                    largest = value;
                }
            }
        }
    }
    Ok(largest)
}
pub(super) fn maxima(context: &FoldContextV8) -> Result<Maxima, SourceJournalError> {
    let b = &context.checked_binding;
    if let Some((binding, retained)) = context.maxima_templates.entry.borrow().as_ref() {
        if std::sync::Arc::ptr_eq(binding, b) {
            return retained.clone();
        }
    }
    let retained = maxima_uncached(context);
    *context.maxima_templates.entry.borrow_mut() =
        Some((std::sync::Arc::clone(b), retained.clone()));
    retained
}

fn maxima_uncached(context: &FoldContextV8) -> Result<Maxima, SourceJournalError> {
    let b = &context.checked_binding;
    let helper = b.helper();
    let id = helper.function().params[0]
        .ty
        .nominal_id()
        .ok_or(SourceJournalError::Binding)?;
    let declared = helper
        .program()
        .declarations
        .record_fields(id)
        .ok_or(SourceJournalError::Binding)?;
    let state =
        json!({"declaration":id.as_str(),"fields":fields(declared.iter().map(|f| (&f.id,&f.ty)))?});
    checked::validate_owned_wait_state_v8(b, &state).map_err(|_| SourceJournalError::Binding)?;
    let authorize = b.authorize();
    let mut decision = Value::Null;
    for case in [authorize.granted(), authorize.refused()] {
        let declared = helper
            .program()
            .declarations
            .case_fields(case)
            .ok_or(SourceJournalError::Binding)?;
        let value = json!({"declaration":authorize.decision().as_str(),"case":case.as_str(),"fields":fields(declared.iter().map(|f| (&f.id,&f.ty)))?});
        checked::validate_owned_wait_decision_v8(b, &value)
            .map_err(|_| SourceJournalError::Binding)?;
        if wire::canonical(&value).len() > wire::canonical(&decision).len() {
            decision = value;
        }
    }
    let operations = |actions| {
        checked::owned_wait_operations_v8(actions).map_err(|_| SourceJournalError::Binding)
    };
    Ok(Maxima {
        state,
        proposal: copy_channel(context, false)?,
        observation: copy_channel(context, true)?,
        decision,
        state_operations: operations(&helper.liveness().failure_cleanup)?,
        result_operations: operations(&helper.liveness().result_disposal)?,
        decision_operations: operations(authorize.disposal())?,
        effect_operations: operations(
            &authorize
                .disposal()
                .iter()
                .filter(|action| {
                    action
                        .active_case
                        .as_ref()
                        .is_some_and(|case| case.case == *authorize.granted())
                })
                .cloned()
                .collect::<Vec<_>>(),
        )?,
        partial_operations: operations(authorize.partial_disposal())?,
        terminal: largest_terminal(context)?,
    })
}
/// Maximum observed receipt using the exact compiler vector, without sorting.
pub(super) fn receipt(operations: &Value) -> Result<Value, SourceJournalError> {
    let values = operations.as_array().ok_or(SourceJournalError::Binding)?;
    let receipt = json!({"kind":"observed","settlement":"completed","operations":values.iter().map(|operation| json!({"operation":operation,"outcome":"completed"})).collect::<Vec<_>>()});
    checked::validate_owned_wait_observed_receipt_v8(operations, &receipt)
        .map_err(|_| SourceJournalError::Binding)?;
    Ok(receipt)
}
