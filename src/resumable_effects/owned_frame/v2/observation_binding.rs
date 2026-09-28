//! Sealed Copy-only observation commitments. No owner or dispatch authority.
use super::CheckedOwnedAgentWaitBindingV8;
use crate::diagnostic::Diagnostic;
use crate::interpreter::resumable::{channel, checkpoint, ResumableChannelValue};
use crate::interpreter::retained_call::{RetainedField, RetainedRecord, RetainedValue};
use crate::interpreter::ArgumentValue;
use crate::resumable_effects::source_checkpoint::SourceCheckpointScope;
use serde_json::{json, Value};

/// Private fields and a checked-only constructor prevent caller-shaped digest
/// pairs from replacing the actual source/nominal/value projection.
pub(crate) struct CheckedOwnedWaitObservationV8 {
    ordinary_digest: String,
    request_digest: String,
    copy_arguments: Value,
}
impl CheckedOwnedWaitObservationV8 {
    pub(crate) fn ordinary_digest(&self) -> &str {
        &self.ordinary_digest
    }
    pub(crate) fn request_digest(&self) -> &str {
        &self.request_digest
    }
    pub(crate) fn copy_arguments(&self) -> &Value {
        &self.copy_arguments
    }
}
fn refused() -> Diagnostic {
    Diagnostic::io("SPX-G583", "source owned Agent wait observation refused")
}
fn retained_copy(value: &ArgumentValue) -> Result<RetainedValue, Diagnostic> {
    match value {
        ArgumentValue::Bool(v) => Ok(RetainedValue::Bool(*v)),
        ArgumentValue::Int32(v) => Ok(RetainedValue::I32(*v)),
        ArgumentValue::Int(v) => Ok(RetainedValue::I64(*v)),
        ArgumentValue::Uint8(v) => Ok(RetainedValue::U8(*v)),
        ArgumentValue::Usize(v) if u32::try_from(*v).is_ok() => Ok(RetainedValue::Usize(*v)),
        // The ordinary retained/SDK codec does not admit float/char carriers.
        // This does not widen the checked Agent lifecycle's current profile.
        _ => Err(refused()),
    }
}

pub(crate) fn bind_owned_wait_observation_v8(
    binding: &CheckedOwnedAgentWaitBindingV8,
    scope: &SourceCheckpointScope,
    observation: &ResumableChannelValue,
) -> Result<CheckedOwnedWaitObservationV8, Diagnostic> {
    let helper = binding.helper();
    if !channel::valid_copy_channel_request(
        helper.program(),
        helper.function().id.as_str(),
        observation,
    ) {
        return Err(refused());
    }
    let ResumableChannelValue::Record {
        declaration,
        fields,
    } = observation
    else {
        return Err(refused());
    };
    let checked_fields = helper
        .program()
        .declarations
        .record_fields(declaration)
        .ok_or_else(refused)?;
    if checked_fields.len() != fields.len() || fields.len() > 8 {
        return Err(refused());
    }
    // Only Copy scalars are recreated in this inert retained vocabulary. No
    // State, Decision, Bytes, cleanup guard or restoration permit is created.
    let fields = checked_fields
        .iter()
        .zip(fields)
        .map(|(expected, actual)| {
            Ok(RetainedField {
                field: expected.id.clone(),
                value: retained_copy(actual)?,
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    let retained = RetainedValue::Record(RetainedRecord {
        record: declaration.clone(),
        fields,
    });
    let ordinary_digest = crate::live_invocation::identity::digest(
        b"semaprax.source-observation.v2\0",
        crate::agent_lifecycle::encode_value(&retained).as_bytes(),
    );
    let value = checkpoint::channel_json(observation);
    let copy_arguments =
        json!([{"parameter":helper.function().params[1].id.as_str(),"value":value}]);
    let scope = super::super::codec::scope(scope).map_err(|_| refused())?;
    let request_digest = super::super::codec::fact_digest(
        b"semaprax.source-owned-frame-request.v2\0",
        &json!({"scope":scope,"plan_digest":binding.binding(),"value":value}),
    );
    Ok(CheckedOwnedWaitObservationV8 {
        ordinary_digest,
        request_digest,
        copy_arguments,
    })
}

#[cfg(test)]
mod tests;
