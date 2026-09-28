//! Bounded structural proof data. No decoder creates a language owner.
use super::CheckedOwnedAgentWaitBindingV8;
use crate::cleanup_plan::FinalizeAction;
use crate::hir::{DeclarationId, ResolvedType};
use crate::interpreter::ArgumentValue;
use crate::resumable_effects::owned_frame::{codec, OwnedFrameError as Error};
use serde_json::Value;

fn scalar_matches(ty: &ResolvedType, value: &ArgumentValue) -> bool {
    matches!(
        (ty, value),
        (ResolvedType::Bool, ArgumentValue::Bool(_))
            | (ResolvedType::I32, ArgumentValue::Int32(_))
            | (ResolvedType::I64, ArgumentValue::Int(_))
            | (ResolvedType::U8, ArgumentValue::Uint8(_))
            | (ResolvedType::Usize, ArgumentValue::Usize(_))
            | (ResolvedType::Char, ArgumentValue::Char(_))
            | (ResolvedType::F32, ArgumentValue::Float32(_))
            | (ResolvedType::F64, ArgumentValue::Float64(_))
    )
}
fn leaf(ty: &ResolvedType, value: &Value) -> Result<(), Error> {
    if *ty == ResolvedType::Bytes {
        codec::keys(value, &["kind", "hex"])?;
        if value["kind"] != "bytes" {
            return Err(Error::Binding);
        }
        codec::unhex(value["hex"].as_str().ok_or(Error::Malformed)?, 1024)?;
    } else if !scalar_matches(ty, &codec::decode_scalar(value)?) {
        return Err(Error::Binding);
    }
    Ok(())
}
fn fields<'a>(
    value: &Value,
    expected: impl ExactSizeIterator<Item = (&'a DeclarationId, &'a ResolvedType)>,
) -> Result<(), Error> {
    let fields = value.as_array().ok_or(Error::Malformed)?;
    if fields.len() != expected.len() || fields.len() > 8 {
        return Err(Error::Binding);
    }
    for (field, (id, ty)) in fields.iter().zip(expected) {
        codec::keys(field, &["identity", "value"])?;
        if field["identity"] != id.as_str() {
            return Err(Error::Binding);
        }
        leaf(ty, &field["value"])?;
    }
    Ok(())
}
pub(crate) fn validate_owned_wait_state_v8(
    binding: &CheckedOwnedAgentWaitBindingV8,
    value: &Value,
) -> Result<(), Error> {
    codec::keys(value, &["declaration", "fields"])?;
    if codec::canonical(value).len() > codec::MAX_CARRIER {
        return Err(Error::Capacity);
    }
    let helper = binding.helper();
    let id = helper.function().params[0]
        .ty
        .nominal_id()
        .ok_or(Error::Binding)?;
    if value["declaration"] != id.as_str() {
        return Err(Error::Binding);
    }
    let declared = helper
        .program()
        .declarations
        .record_fields(id)
        .ok_or(Error::Binding)?;
    fields(&value["fields"], declared.iter().map(|f| (&f.id, &f.ty)))
}
pub(crate) fn validate_owned_wait_decision_v8(
    binding: &CheckedOwnedAgentWaitBindingV8,
    value: &Value,
) -> Result<(), Error> {
    codec::keys(value, &["declaration", "case", "fields"])?;
    if codec::canonical(value).len() > codec::MAX_CARRIER {
        return Err(Error::Capacity);
    }
    let plan = binding.authorize();
    if value["declaration"] != plan.decision().as_str() {
        return Err(Error::Binding);
    }
    let id = match value["case"].as_str() {
        Some(v) if v == plan.granted().as_str() => plan.granted(),
        Some(v) if v == plan.refused().as_str() => plan.refused(),
        _ => return Err(Error::Binding),
    };
    let declared = plan
        .helper()
        .program()
        .declarations
        .case_fields(id)
        .ok_or(Error::Binding)?;
    fields(&value["fields"], declared.iter().map(|f| (&f.id, &f.ty)))
}
/// Serialize a checked vector using its owning compiler renderer. Do not sort
/// or repair this vector, including conditional cases and Temporary places.
pub(crate) fn owned_wait_operations_v8(actions: &[FinalizeAction]) -> Result<Value, Error> {
    let values = actions
        .iter()
        .map(|a| {
            serde_json::from_str(&crate::graph_cleanup::finalize_action_json(a))
                .map_err(|_| Error::Malformed)
        })
        .collect::<Result<Vec<Value>, _>>()?;
    Ok(Value::Array(values))
}
pub(crate) fn validate_owned_wait_operations_v8(
    actual: &[FinalizeAction],
    value: &Value,
) -> Result<(), Error> {
    if codec::canonical(value).len() > codec::MAX_CARRIER {
        return Err(Error::Capacity);
    }
    if *value != owned_wait_operations_v8(actual)? {
        return Err(Error::Binding);
    }
    Ok(())
}
pub(crate) fn validate_owned_wait_observed_receipt_v8(
    operations: &Value,
    receipt: &Value,
) -> Result<(), Error> {
    if codec::canonical(receipt).len() > codec::MAX_CARRIER {
        return Err(Error::Capacity);
    }
    codec::keys(receipt, &["kind", "settlement", "operations"])?;
    if receipt["kind"] != "observed" {
        return Err(Error::Binding);
    }
    let expected = operations.as_array().ok_or(Error::Malformed)?;
    let observed = receipt["operations"].as_array().ok_or(Error::Malformed)?;
    if expected.len() != observed.len() {
        return Err(Error::Binding);
    }
    let mut completed = true;
    for (actual, expected) in observed.iter().zip(expected) {
        codec::keys(actual, &["operation", "outcome"])?;
        if actual["operation"] != *expected {
            return Err(Error::Binding);
        }
        match actual["outcome"].as_str() {
            Some("completed") => {}
            Some("failed") => completed = false,
            _ => return Err(Error::Malformed),
        }
    }
    if receipt["settlement"] != if completed { "completed" } else { "failed" } {
        return Err(Error::Binding);
    }
    Ok(())
}
pub(crate) fn validate_owned_wait_failure_v8(
    binding: &CheckedOwnedAgentWaitBindingV8,
    function: &DeclarationId,
    status: &Value,
) -> Result<(), Error> {
    codec::keys(status, &["failure", "language_status"])?;
    let f = if *function == binding.helper().function().id {
        binding.helper().function()
    } else if *function == binding.observe().function().id {
        binding.observe().function()
    } else if *function == binding.authorize().function().id {
        binding.authorize().function()
    } else {
        return Err(Error::Binding);
    };
    let failure = status["failure"].as_str().ok_or(Error::Malformed)?;
    if failure != "language_failure" {
        if ![
            "fuel_exhausted",
            "host_abandoned",
            "answer_type_mismatch",
            "evaluation_rejected",
            "handler_failed",
            "call_depth_exceeded",
        ]
        .contains(&failure)
            || !status["language_status"].is_null()
        {
            return Err(Error::Binding);
        }
        return Ok(());
    }
    for source in &f.cleanup_plan.status_sources {
        use crate::cleanup_plan::StatusProducer;
        let values = match &source.producer {
            StatusProducer::ContractFalse { phase, .. } => {
                vec![crate::conformance::NormalizedStatus::contract(*phase)]
            }
            StatusProducer::CheckedArithmetic {
                normalized_cases, ..
            } => normalized_cases
                .iter()
                .map(|c| crate::conformance::NormalizedStatus::arithmetic(*c))
                .collect(),
            StatusProducer::PropagatedCall { .. } => Vec::new(),
        };
        for value in values {
            if status["language_status"]
                == codec::parse(value.to_json().as_bytes(), codec::MAX_CARRIER)?
            {
                return Ok(());
            }
        }
    }
    Err(Error::Binding)
}

#[cfg(test)]
mod tests;
