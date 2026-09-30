//! Shared exact typed operation/result planning. All products are inert data;
//! no owner, authorization, ACK or host-dispatch authority is created here.
use super::*;

pub(super) fn planned_call_projected(
    compiled: &CompiledTypedEffects,
    index: usize,
    projected: &[RetainedValue],
) -> Result<(usize, TargetOperation, TypedCarrier), Vec<Diagnostic>> {
    let operation = compiled
        .operations
        .get(index)
        .ok_or_else(|| error("target.selector_range"))?;
    let mut arguments = Vec::new();
    for argument in &operation.arguments {
        let position = compiled
            .lifecycle
            .inner
            .binding
            .proposal
            .iter()
            .position(|field| field.field.as_str() == argument.proposal_field_id)
            .ok_or_else(|| error("target.argument_identity"))?;
        let value = projected
            .get(position)
            .ok_or_else(|| error("target.argument_index"))?;
        if !argument.kind.accepts(value) {
            return Err(error("target.argument_type"));
        }
        arguments.push((argument.argument_id.clone(), value.clone()));
    }
    for ((_, value), limit) in arguments.iter().zip(&compiled.field_limits[index].0) {
        if scalar_bytes(value).is_none_or(|size| size > *limit) {
            return Err(error("target.argument_field_budget"));
        }
    }
    let argument_type = TargetLiveDispatch::carrier_type(operation, "argument");
    let result_type = TargetLiveDispatch::carrier_type(operation, "result");
    let operation = TargetOperation::new(
        operation.operation_id.clone(),
        operation.effect_id.clone(),
        argument_type.clone(),
        result_type,
    )
    .map_err(|_| error("target.operation"))?;
    let carrier = TypedCarrier::new(argument_type, encode_fields(&arguments).into_bytes())
        .map_err(|_| error("target.argument_carrier"))?;
    Ok((index, operation, carrier))
}

pub(super) fn accepted_result(
    compiled: &CompiledTypedEffects,
    index: usize,
    payload: &[u8],
) -> Option<Vec<u8>> {
    let operation = compiled.operations.get(index)?;
    let value: Value = serde_json::from_slice(payload).ok()?;
    let object = value.as_object()?;
    if object.get("schema")?.as_str()? != "semaprax.agent-effect-fields.v1" {
        return None;
    }
    let fields = object.get("fields")?.as_array()?;
    if fields.len() != operation.results.len() {
        return None;
    }
    let mut decoded = Vec::new();
    for (field, expected) in fields.iter().zip(&operation.results) {
        let pair = field.as_array()?;
        if pair.len() != 2 || pair.first()?.as_str()? != expected.result_id {
            return None;
        }
        let value = match expected.kind {
            EffectScalar::Bool => RetainedValue::Bool(pair.get(1)?.as_bool()?),
            EffectScalar::I32 => RetainedValue::I32(pair.get(1)?.as_str()?.parse().ok()?),
            EffectScalar::I64 => RetainedValue::I64(pair.get(1)?.as_str()?.parse().ok()?),
            EffectScalar::U8 => RetainedValue::U8(pair.get(1)?.as_str()?.parse().ok()?),
            EffectScalar::Usize => RetainedValue::Usize(pair.get(1)?.as_str()?.parse().ok()?),
        };
        decoded.push((expected.result_id.clone(), value));
    }
    let canonical = encode_fields(&decoded);
    if canonical.as_bytes() != payload
        || decoded
            .iter()
            .zip(&compiled.field_limits[index].1)
            .any(|((_, value), limit)| scalar_bytes(value).is_none_or(|size| size > *limit))
    {
        return None;
    }
    Some(canonical.into_bytes())
}

use crate::execution_revision::typed::{AgentRuntimeV2, CheckedTypedOwnedWaitExecutionV8};
use crate::interpreter::{resumable::ResumableChannelValue, ArgumentValue};
use crate::resumable_effects::owned_frame::v2::CheckedOwnedWaitProposalV8;
use crate::resumable_effects::source_checkpoint::SourceCheckpointScope;

/// Immutable checked request data. It grants no host call or ownership.
pub(crate) struct CheckedOwnedEffectPlanV8<'a> {
    compiled: &'a CompiledTypedEffects,
    index: usize,
    operation: TargetOperation,
    argument: TypedCarrier,
    limits: EffectBudget,
}
impl CheckedOwnedEffectPlanV8<'_> {
    pub(crate) fn operation(&self) -> &TargetOperation {
        &self.operation
    }
    pub(crate) fn argument(&self) -> &TypedCarrier {
        &self.argument
    }
    pub(crate) fn limits(&self) -> EffectBudget {
        self.limits
    }
    /// The portable owned Outcome profile bounds the response sink before I/O.
    /// This intersection does not rewrite the retained typed execution E.
    pub(crate) fn target_limits(&self) -> TargetLimits {
        TargetLimits {
            max_calls: self.limits.max_calls as u64,
            max_request_bytes: self.limits.max_argument_bytes as u64,
            max_result_bytes: self.limits.max_result_bytes as u64,
            max_total_bytes: self.limits.max_total_bytes as u64,
            // One unit per call under the invocation-wide accounting ceiling.
            max_fuel: self.limits.max_calls as u64,
        }
    }
    pub(crate) fn accepted_result(&self, payload: &[u8]) -> Option<Vec<u8>> {
        if payload.len() > self.limits.max_result_bytes {
            return None;
        }
        accepted_result(self.compiled, self.index, payload)
    }
}

/// Uses the already checked SDK projection, never reparses its model document.
pub(crate) fn plan_owned_effect_v8<'a>(
    runtime: &'a AgentRuntimeV2,
    execution: &CheckedTypedOwnedWaitExecutionV8,
    scope: &SourceCheckpointScope,
    proposal: &CheckedOwnedWaitProposalV8,
) -> Result<CheckedOwnedEffectPlanV8<'a>, Vec<Diagnostic>> {
    let compiled = runtime
        .owned_wait_effects_v8(execution)
        .map_err(|_| error("owned_wait.runtime"))?;
    let limits = runtime
        .owned_wait_effect_limits_v8(execution)
        .map_err(|_| error("owned_wait.runtime"))?;
    let limits = EffectBudget {
        max_calls: limits.max_calls.min(compiled.limits.max_calls),
        max_argument_bytes: limits
            .max_argument_bytes
            .min(compiled.limits.max_argument_bytes),
        max_result_bytes: limits
            .max_result_bytes
            .min(compiled.limits.max_result_bytes)
            .min(1024),
        max_total_bytes: limits.max_total_bytes.min(compiled.limits.max_total_bytes),
    };
    let invocation = crate::live_invocation::identity::digest(
        b"semaprax.live-invocation.source-id.v8\0",
        serde_json::to_string(&serde_json::json!({"execution":execution.ordinary().invocation(),"owned_wait_binding":execution.wait().binding()}))
            .map_err(|_| error("owned_wait.execution_scope"))?.as_bytes(),
    );
    if scope.program_root() != execution.wait().lifecycle().source_revision()
        || scope.invocation_id() != invocation
    {
        return Err(error("owned_wait.execution_scope"));
    }
    let scope = serde_json::json!({"program_root":scope.program_root(),"invocation":scope.invocation_id(),"policy_epoch":scope.policy_epoch()});
    if !proposal.matches(execution.wait().binding(), &scope) {
        return Err(error("owned_wait.proposal_binding"));
    }
    let ResumableChannelValue::Record { fields, .. } = proposal.carrier() else {
        return Err(error("owned_wait.proposal_shape"));
    };
    let projected = fields
        .iter()
        .map(|value| {
            Some(match value {
                ArgumentValue::Bool(v) => RetainedValue::Bool(*v),
                ArgumentValue::Int32(v) => RetainedValue::I32(*v),
                ArgumentValue::Int(v) => RetainedValue::I64(*v),
                ArgumentValue::Uint8(v) => RetainedValue::U8(*v),
                ArgumentValue::Usize(v) => RetainedValue::Usize(*v),
                _ => return None,
            })
        })
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| error("owned_wait.proposal_shape"))?;
    if projected.len() != compiled.lifecycle.inner.binding.proposal.len() {
        return Err(error("owned_wait.proposal_shape"));
    }
    let selector = compiled
        .lifecycle
        .inner
        .binding
        .proposal
        .iter()
        .position(|f| f.field.as_str() == compiled.selector)
        .ok_or_else(|| error("target.selector_type"))?;
    let Some(RetainedValue::Usize(selector)) = projected.get(selector) else {
        return Err(error("target.selector_type"));
    };
    let index = usize::try_from(*selector).map_err(|_| error("target.selector_range"))?;
    let (index, operation, argument) = planned_call_projected(compiled, index, &projected)?;
    Ok(CheckedOwnedEffectPlanV8 {
        compiled,
        index,
        operation,
        argument,
        limits,
    })
}
