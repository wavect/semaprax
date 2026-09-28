//! Actual same-source initializer proof for the private initialized live actor.
//! This plan is inert; neither input equality nor proof data grants an ACK.
use super::{
    compile_owned_initialize_v2, CheckedOwnedAgentWaitBindingV8, CheckedOwnedInitializeV2,
};
use crate::diagnostic::Diagnostic;
use crate::hir::{ResolvedAgentOperationKind as Kind, ResolvedAgentOperationRoleKind as Role};
use crate::interpreter::resumable::owned_frame::{OwnedFrameInput, OwnedFrameInputValue};
use crate::interpreter::retained_call::RetainedValue;
use crate::interpreter::ArgumentValue;

pub(crate) fn live_initializer_plan_v8(
    binding: &CheckedOwnedAgentWaitBindingV8,
) -> Result<CheckedOwnedInitializeV2, Diagnostic> {
    let agent = binding
        .helper()
        .program()
        .agents
        .iter()
        .find(|agent| agent.stable_id == *binding.agent())
        .ok_or_else(refused)?;
    let operation = agent
        .operations
        .iter()
        .find(|operation| {
            operation.role == Role::Initialize && operation.kind == Kind::Deterministic
        })
        .ok_or_else(refused)?;
    compile_owned_initialize_v2(binding.helper(), &operation.stable_id)
}
fn refused() -> Diagnostic {
    Diagnostic::io("SPX-T303", "initialized owned Agent source proof differs")
}
/// Pure ordered equality against actual retained typed Task, before admission.
/// The retained carrier has no Char/F32/F64 variants: no approximation or
/// nominal-only acceptance can silently substitute those into the instance.
pub(crate) fn exact_runtime_task_v8(input: &OwnedFrameInput, task: &RetainedValue) -> bool {
    let RetainedValue::Record(record) = task else {
        return false;
    };
    input.declaration == record.record
        && input.fields.len() == record.fields.len()
        && input.fields.iter().all(|field| {
            let Some(retained) = record
                .fields
                .iter()
                .find(|retained| retained.field == field.identity)
            else {
                return false;
            };
            field.identity == retained.field
                && match (&field.value, &retained.value) {
                    (OwnedFrameInputValue::Bytes(a), RetainedValue::Bytes(b)) => a == b,
                    (
                        OwnedFrameInputValue::Scalar(ArgumentValue::Bool(a)),
                        RetainedValue::Bool(b),
                    ) => a == b,
                    (
                        OwnedFrameInputValue::Scalar(ArgumentValue::Int32(a)),
                        RetainedValue::I32(b),
                    ) => a == b,
                    (
                        OwnedFrameInputValue::Scalar(ArgumentValue::Int(a)),
                        RetainedValue::I64(b),
                    ) => a == b,
                    (
                        OwnedFrameInputValue::Scalar(ArgumentValue::Uint8(a)),
                        RetainedValue::U8(b),
                    ) => a == b,
                    (
                        OwnedFrameInputValue::Scalar(ArgumentValue::Usize(a)),
                        RetainedValue::Usize(b),
                    ) => a == b,
                    _ => false,
                }
        })
}

/// Inert ordered Task facts borrowed from the typed instance, no language owner.
pub(crate) fn runtime_task_document_v8(
    task: &RetainedValue,
    plan: &CheckedOwnedInitializeV2,
) -> Option<serde_json::Value> {
    let RetainedValue::Record(record) = task else {
        return None;
    };
    let declaration = plan.function().params[0].ty.nominal_id()?;
    let declared = plan
        .helper()
        .program()
        .declarations
        .record_fields(declaration)?;
    if record.record != *declaration || record.fields.len() != declared.len() {
        return None;
    }
    let fields = declared.iter().map(|declared| {
        let field = record.fields.iter().find(|field| field.field == declared.id)?;
        let value = match &field.value {
            RetainedValue::Bytes(bytes) => serde_json::json!({"kind":"bytes","hex":crate::live_invocation::identity::hex(bytes)}),
            RetainedValue::Bool(v) => serde_json::json!({"tag":"bool","value":v}),
            RetainedValue::I32(v) => serde_json::json!({"tag":"i32","value":v}),
            RetainedValue::I64(v) => serde_json::json!({"tag":"i64","value":v}),
            RetainedValue::U8(v) => serde_json::json!({"tag":"u8","value":v}),
            RetainedValue::Usize(v) => serde_json::json!({"tag":"usize","value":v}),
            _ => return None,
        };
        Some(serde_json::json!({"identity":field.field.as_str(),"value":value}))
    }).collect::<Option<Vec<_>>>()?;
    Some(serde_json::json!({"declaration":record.record.as_str(),"fields":fields}))
}
