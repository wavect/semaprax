//! Checked source Agent association. This proof grants no store or owner authority.
use super::{
    compile_owned_authorize_v2, compile_owned_frame_helper_v2, compile_owned_observe_v2,
    CheckedOwnedAuthorizeV2, CheckedOwnedFrameHelperV2, CheckedOwnedObserveV2,
};
use crate::agent_lifecycle::iterative::{
    compile_source_agent_lifecycle_v2, CompiledIterativeLifecycle,
};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    self, DeclarationId, ResolvedAgentOperationKind as Kind,
    ResolvedAgentOperationRoleKind as Operation, ResolvedAgentTypeRoleKind as Role, ResolvedType,
};
use serde_json::{json, Value};
use std::path::Path;

/// Fields are private: arbitrary host strings or decoded metadata cannot mint B.
pub(crate) struct CheckedOwnedAgentWaitBindingV8 {
    helper: CheckedOwnedFrameHelperV2,
    observe: CheckedOwnedObserveV2,
    authorize: CheckedOwnedAuthorizeV2,
    lifecycle: CompiledIterativeLifecycle,
    agent: DeclarationId,
    binding: String,
    signature: Value,
    cleanup_digest: String,
}
impl CheckedOwnedAgentWaitBindingV8 {
    pub(crate) fn helper(&self) -> &CheckedOwnedFrameHelperV2 {
        &self.helper
    }
    pub(crate) fn observe(&self) -> &CheckedOwnedObserveV2 {
        &self.observe
    }
    pub(crate) fn authorize(&self) -> &CheckedOwnedAuthorizeV2 {
        &self.authorize
    }
    pub(crate) fn lifecycle(&self) -> &CompiledIterativeLifecycle {
        &self.lifecycle
    }
    pub(crate) fn agent(&self) -> &DeclarationId {
        &self.agent
    }
    pub(crate) fn binding(&self) -> &str {
        &self.binding
    }
    pub(crate) fn signature(&self) -> &Value {
        &self.signature
    }
    pub(crate) fn cleanup_digest(&self) -> &str {
        &self.cleanup_digest
    }
}
fn refused() -> Diagnostic {
    Diagnostic::io("SPX-G583", "source owned Agent wait association refused")
}

pub(crate) fn compile_owned_agent_wait_v8(
    source: &str,
    path: &Path,
    agent_id: &str,
    step_id: &str,
) -> Result<CheckedOwnedAgentWaitBindingV8, Vec<Diagnostic>> {
    let original = crate::check(source, path)?;
    let program = hir::resolve(&original)?;
    // Replay original source, even when every decoded HIR field appears plausible.
    hir::replay_agent_source_associations(&original, &program.agents).map_err(|e| vec![e])?;
    let mut selected = program
        .agents
        .iter()
        .filter(|a| a.stable_id.as_str() == agent_id);
    let agent = selected.next().ok_or_else(|| vec![refused()])?;
    if selected.next().is_some() || agent.source_association.is_none() {
        return Err(vec![refused()]);
    }
    let association = agent.source_association.as_ref().expect("checked presence");
    let wait = agent.model_wait.as_ref().ok_or_else(|| vec![refused()])?;
    if association.module != original.module
        || !association.helper_top_level
        || association.operations != agent.operations
        || association.model_wait != agent.model_wait
    {
        return Err(vec![refused()]);
    }
    let operation = |role, kind| {
        agent
            .operations
            .iter()
            .find(|o| o.role == role && o.kind == kind)
            .map(|o| &o.stable_id)
            .ok_or_else(|| vec![refused()])
    };
    let model = operation(Operation::Propose, Kind::Model)?;
    let observe_id = operation(Operation::Observe, Kind::Deterministic)?;
    let authorize_id = operation(Operation::Authorize, Kind::Deterministic)?;
    let role_type = |role| {
        agent
            .types
            .iter()
            .find(|t| t.role == role)
            .map(|t| &t.stable_id)
            .ok_or_else(|| vec![refused()])
    };
    let helper = compile_owned_frame_helper_v2(&program, &wait.helper_id).map_err(|e| vec![e])?;
    let f = helper.function();
    if f.params[0].ty.nominal_id() != Some(role_type(Role::State)?)
        || f.params[1].ty.nominal_id() != Some(role_type(Role::Observation)?)
        || f.yields
            .as_ref()
            .expect("checked yields")
            .response_type
            .nominal_id()
            != Some(role_type(Role::Proposal)?)
    {
        return Err(vec![refused()]);
    }
    // Admit the real lifecycle ABI and all six role declarations, including Step.
    // This deliberately retains its existing narrower carrier profile.
    let lifecycle = compile_source_agent_lifecycle_v2(source, path, agent_id, step_id)?;
    if lifecycle.source_revision() != crate::graph::revision(&original) {
        return Err(vec![refused()]);
    }
    let observe = compile_owned_observe_v2(&helper, observe_id).map_err(|e| vec![e])?;
    let authorize = compile_owned_authorize_v2(&helper, authorize_id).map_err(|e| vec![e])?;
    let type_key = |ty: &ResolvedType| format!("semaprax.resolved-type.v1:{}", ty.identity_key());
    let yields = f.yields.as_ref().expect("checked yields");
    let signature = json!({
        "profile":"source-owned-frame.v2", "function":f.id.as_str(),
        "parameters":[
            {"id":f.params[0].id.as_str(),"mode":"own","type":type_key(&f.params[0].ty)},
            {"id":f.params[1].id.as_str(),"mode":"copy","type":type_key(&f.params[1].ty)}
        ],
        "result":type_key(&f.return_type),"request":type_key(&yields.request_type),
        "response":type_key(&yields.response_type),"yield_count":1
    });
    let cleanup_digest = super::super::codec::digest(
        b"semaprax.source-owned-frame-cleanup-plan.v2\0",
        crate::graph_cleanup::cleanup_plan_json(&f.cleanup_plan).as_bytes(),
    );
    let checked_graph: Value =
        serde_json::from_str(&crate::graph::to_json(&original)?).map_err(|_| vec![refused()])?;
    if checked_graph["schema"] != "semaprax.graph.v50" {
        return Err(vec![refused()]);
    }
    let binding = super::super::codec::fact_digest(
        b"semaprax.source-owned-frame-plan.v2\0",
        &json!({"source_revision":lifecycle.source_revision(),"agent":agent_id,
            "model_operation":model.as_str(),"helper":f.id.as_str(),"signature":signature,
            "checked_graph":checked_graph,"cleanup_plan_digest":cleanup_digest}),
    );
    Ok(CheckedOwnedAgentWaitBindingV8 {
        helper,
        observe,
        authorize,
        lifecycle,
        agent: agent.stable_id.clone(),
        binding,
        signature,
        cleanup_digest,
    })
}

#[cfg(test)]
mod tests;
