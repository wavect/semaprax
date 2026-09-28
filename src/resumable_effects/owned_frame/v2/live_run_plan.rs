//! Actual same-source initializer proof for the private initialized live actor.
//! This plan is inert; neither input equality nor proof data grants an ACK.
use super::{
    compile_owned_initialize_v2, CheckedOwnedAgentWaitBindingV8, CheckedOwnedInitializeV2,
};
use crate::agent_lifecycle::LifecycleTask;
use crate::diagnostic::Diagnostic;
use crate::hir::{ResolvedAgentOperationKind as Kind, ResolvedAgentOperationRoleKind as Role};
use crate::interpreter::resumable::owned_frame::{OwnedFrameInput, OwnedFrameInputValue};
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
/// Pure nominal and declaration-ordered equality against the actual typed Task.
/// Field roles come from retained compiled PayloadShape, never field spelling,
/// position, or a guessed unique scalar in host data.
pub(crate) fn exact_runtime_task_v8(
    input: &OwnedFrameInput,
    task: &LifecycleTask,
    binding: &CheckedOwnedAgentWaitBindingV8,
) -> bool {
    let metadata = binding.lifecycle().owned_wait_task_v8();
    input.declaration == *metadata.id && input.fields.len() == metadata.fields().count()
        && input.fields.iter().zip(metadata.fields()).all(|(field,(identity,ty))| {
            if field.identity != *identity { return false; }
            if identity == metadata.objective_field && *ty == crate::hir::ResolvedType::Bytes {
                matches!(&field.value,OwnedFrameInputValue::Bytes(bytes) if bytes == &task.objective)
            } else if identity == metadata.budget_field && *ty == crate::hir::ResolvedType::I64 {
                matches!(&field.value,OwnedFrameInputValue::Scalar(ArgumentValue::Int(value)) if *value == task.budget)
            } else { false }
        })
}
/// Descriptive inert projection, preserving actual compiled declaration order.
/// Hex text grants no language backing, grant or owner restoration authority.
pub(crate) fn runtime_task_document_v8(
    task: &LifecycleTask,
    binding: &CheckedOwnedAgentWaitBindingV8,
) -> Option<serde_json::Value> {
    let metadata = binding.lifecycle().owned_wait_task_v8();
    let fields = metadata.fields().map(|(id,ty)| {
        let value = if id == metadata.objective_field && *ty == crate::hir::ResolvedType::Bytes {
            serde_json::json!({"kind":"bytes","hex":crate::live_invocation::identity::hex(&task.objective)})
        } else if id == metadata.budget_field && *ty == crate::hir::ResolvedType::I64 {
            serde_json::json!({"tag":"i64","value":task.budget})
        } else { return None; };
        Some(serde_json::json!({"identity":id.as_str(),"value":value}))
    }).collect::<Option<Vec<_>>>()?;
    Some(serde_json::json!({"declaration":metadata.id.as_str(),"fields":fields}))
}
