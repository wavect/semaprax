//! Checked identity wrapper for the bounded standalone FixtureAgent wait.
use super::*;
use crate::diagnostic::quote_json;
use crate::hir::{ResolvedExprKind, ResolvedFunction, ResolvedTypeDeclarationKind};
use crate::interpreter::resumable::{checkpoint, ResumableChannelValue};
use crate::interpreter::ArgumentValue;

const BINDING_DOMAIN: &[u8] = b"semaprax.source-model-wait.binding.v1\0";

/// Authority-free compiler product. Construction checks the exact Agent,
/// nominal channel, one-yield identity graph and compiler cleanup inventory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceModelWaitBinding {
    digest: String,
    lifecycle: String,
    source_revision: String,
    wrapper: String,
    evaluation_fuel: usize,
    observation: hir::DeclarationId,
    proposal: hir::DeclarationId,
}

impl SourceModelWaitBinding {
    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub fn wrapper_id(&self) -> &str {
        &self.wrapper
    }
    pub fn evaluation_fuel(&self) -> usize {
        self.evaluation_fuel
    }
    pub fn source_revision(&self) -> &str {
        &self.source_revision
    }
    pub(crate) fn matches(&self, lifecycle: &CompiledIterativeLifecycle) -> bool {
        self.lifecycle == lifecycle.digest() && self.source_revision == lifecycle.source_revision()
    }
}

impl CompiledIterativeLifecycle {
    pub(crate) fn model_wait_binding(
        &self,
        wrapper_id: &str,
        evaluation_fuel: usize,
    ) -> Result<SourceModelWaitBinding, Vec<Diagnostic>> {
        let refuse = || vec![bad("source.model_wait_binding")];
        if evaluation_fuel == 0 || evaluation_fuel > crate::interpreter::MAX_STEPS_LIMIT {
            return Err(refuse());
        }
        let program = &self.inner.program;
        hir::validate(program).map_err(|_| refuse())?;
        let agent = program
            .agents
            .iter()
            .find(|a| a.stable_id.as_str() == self.inner.agent_id)
            .filter(|a| a.stable_id.as_str() == "fixture.agent")
            .ok_or_else(refuse)?;
        let model = agent
            .operations
            .iter()
            .find(|o| {
                o.role == hir::ResolvedAgentOperationRoleKind::Propose
                    && o.kind == hir::ResolvedAgentOperationKind::Model
            })
            .ok_or_else(refuse)?;
        let type_id = |role| {
            self.inner
                .binding
                .types
                .iter()
                .find(|(r, _)| *r == role)
                .map(|(_, id)| id.clone())
                .ok_or_else(refuse)
        };
        let observation = type_id("observation")?;
        let proposal = type_id("proposal")?;
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == wrapper_id)
            .ok_or_else(refuse)?;
        let parameter = function
            .params
            .first()
            .filter(|_| function.params.len() == 1)
            .ok_or_else(refuse)?;
        let yields = function.yields.as_ref().ok_or_else(refuse)?;
        if parameter.ownership != hir::OwnershipMode::Value
            || parameter.ty.nominal_id() != Some(&observation)
            || function.return_type.nominal_id() != Some(&proposal)
            || yields.request_type != parameter.ty
            || yields.response_type != function.return_type
            || !function.effects.is_empty()
            || !function.requires.is_empty()
            || !function.ensures.is_empty()
            || !empty_cleanup(function)
        {
            return Err(refuse());
        }
        let ResolvedExprKind::Block { statements, tail } = &function.body.kind else {
            return Err(refuse());
        };
        let ResolvedExprKind::Yield { request } = &tail.kind else {
            return Err(refuse());
        };
        let ResolvedExprKind::Place(place) = &request.kind else {
            return Err(refuse());
        };
        if !statements.is_empty() || place.root != parameter.id || !place.projections.is_empty() {
            return Err(refuse());
        }
        let fields = record_fields(program, &observation).ok_or_else(refuse)?;
        let proposal_fields = record_fields(program, &proposal).ok_or_else(refuse)?;
        if proposal_fields.iter().any(|f| zero(&f.ty).is_none()) {
            return Err(refuse());
        }
        let arguments = [ResumableChannelValue::Record {
            declaration: observation.clone(),
            fields: fields
                .iter()
                .map(|f| zero(&f.ty))
                .collect::<Option<Vec<_>>>()
                .ok_or_else(refuse)?,
        }];
        // Checked whole-function admission also validates all Proposal leaves.
        let (plan, _) = checkpoint::checked_channel_arguments_plan(program, wrapper_id, &arguments)
            .map_err(|_| refuse())?;
        if plan.suspensions.len() != 1
            || !empty_cleanup(&plan.start.function)
            || plan.resumes.len() != 1
            || !empty_cleanup(&plan.resumes[0].function)
        {
            return Err(refuse());
        }
        let prefix = crate::resumable_effects::source_signature::SOURCE_TYPE_SHAPE_PREFIX;
        // Identical canonical signature object to source checkpoint v7.
        let signature = serde_json::json!({
            "request_shape": format!("{prefix}{}", yields.request_type.identity_key()),
            "answer_shape": format!("{prefix}{}", yields.response_type.identity_key()),
            "plan_identity": format!("sha256:{:x}", crate::digest_hex::LowerHex(plan.identity.as_bytes())),
            "yield_count": 1,
        });
        let payload = format!(
            "{{\"source_revision\":{},\"agent\":{},\"model_operation\":{},\"wrapper\":{},\"observation\":{},\"proposal\":{},\"signature\":{},\"cleanup\":\"empty-copy\",\"checkpoint_schema\":\"semaprax.source-resumable-checkpoint.v7\",\"evaluation_fuel\":{},\"checkpoint_limit\":32768}}",
            quote_json(self.source_revision()), quote_json(&self.inner.agent_id),
            quote_json(model.stable_id.as_str()), quote_json(wrapper_id),
            quote_json(observation.as_str()), quote_json(proposal.as_str()), signature, evaluation_fuel,
        );
        Ok(SourceModelWaitBinding {
            digest: super::super::digest(BINDING_DOMAIN, payload.as_bytes()),
            lifecycle: self.digest().into(),
            source_revision: self.source_revision().into(),
            wrapper: wrapper_id.into(),
            evaluation_fuel,
            observation,
            proposal,
        })
    }
}

fn empty_cleanup(function: &ResolvedFunction) -> bool {
    function.cleanup.slots.is_empty()
        && function.cleanup.flags.is_empty()
        && function.cleanup_plan.slots.is_empty()
        && function
            .cleanup_plan
            .exits
            .iter()
            .all(|e| e.finalize_in_order.is_empty())
}

fn record_fields<'a>(
    program: &'a hir::ResolvedProgram,
    id: &hir::DeclarationId,
) -> Option<&'a [hir::ResolvedFieldDeclaration]> {
    match &program.types.iter().find(|t| &t.id == id)?.kind {
        ResolvedTypeDeclarationKind::Record { fields } => Some(fields),
        _ => None,
    }
}
fn zero(ty: &hir::ResolvedType) -> Option<ArgumentValue> {
    Some(match ty {
        hir::ResolvedType::Bool => ArgumentValue::Bool(false),
        hir::ResolvedType::I32 => ArgumentValue::Int32(0),
        hir::ResolvedType::I64 => ArgumentValue::Int(0),
        hir::ResolvedType::U8 => ArgumentValue::Uint8(0),
        hir::ResolvedType::Usize => ArgumentValue::Usize(0),
        _ => return None,
    })
}

#[cfg(test)]
mod tests;
