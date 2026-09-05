//! Independent HIR validation for language-native Agent nodes.

use std::collections::BTreeSet;

use crate::diagnostic::Diagnostic;

use super::{
    ResolvedAgentOperationKind, ResolvedAgentOperationRoleKind, ResolvedAgentTypeRoleKind,
    ResolvedProgram,
};

pub(super) fn validate(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    let mut project_agent_ids = BTreeSet::new();
    for agent in &program.agents {
        if agent.types.len() != 6
            || agent.types.iter().map(|role| role.role).ne([
                ResolvedAgentTypeRoleKind::Task,
                ResolvedAgentTypeRoleKind::State,
                ResolvedAgentTypeRoleKind::Observation,
                ResolvedAgentTypeRoleKind::Proposal,
                ResolvedAgentTypeRoleKind::Outcome,
                ResolvedAgentTypeRoleKind::Result,
            ])
        {
            return Err(invalid("resolved Agent type-role inventory is invalid"));
        }
        if agent.operations.len() != 6
            || agent
                .operations
                .iter()
                .map(|operation| (operation.role, operation.kind))
                .ne([
                    (
                        ResolvedAgentOperationRoleKind::Initialize,
                        ResolvedAgentOperationKind::Deterministic,
                    ),
                    (
                        ResolvedAgentOperationRoleKind::Observe,
                        ResolvedAgentOperationKind::Deterministic,
                    ),
                    (
                        ResolvedAgentOperationRoleKind::Propose,
                        ResolvedAgentOperationKind::Model,
                    ),
                    (
                        ResolvedAgentOperationRoleKind::Authorize,
                        ResolvedAgentOperationKind::Deterministic,
                    ),
                    (
                        ResolvedAgentOperationRoleKind::Execute,
                        ResolvedAgentOperationKind::Effect,
                    ),
                    (
                        ResolvedAgentOperationRoleKind::Reduce,
                        ResolvedAgentOperationKind::Deterministic,
                    ),
                ])
        {
            return Err(invalid(
                "resolved Agent operation-role or kind inventory is invalid",
            ));
        }
        if program
            .declarations
            .contains_declaration_id(&agent.stable_id)
        {
            return Err(invalid(
                "resolved Agent declaration identity collides with another declaration",
            ));
        }
        if !project_agent_ids.insert(agent.stable_id.as_str()) {
            return Err(invalid(
                "resolved Agent declaration identity collides with another Agent declaration",
            ));
        }
        let mut local_ids = BTreeSet::new();
        let identities = std::iter::once(&agent.stable_id)
            .chain(agent.types.iter().map(|role| &role.stable_id))
            .chain(agent.operations.iter().map(|role| &role.stable_id));
        for identity in identities {
            if !local_ids.insert(identity.as_str()) {
                return Err(invalid(
                    "resolved Agent identity collides within its local inventory",
                ));
            }
        }
        if agent.runtime_v1_json.contains('\n')
            || agent.runtime_v1_json.contains('\r')
            || !serde_json::from_str::<serde_json::Value>(&agent.runtime_v1_json)
                .is_ok_and(|value| value.is_object())
        {
            return Err(invalid(
                "resolved Agent runtime_v1 carrier is not one JSON object",
            ));
        }
    }
    Ok(())
}

fn invalid(message: &'static str) -> Diagnostic {
    Diagnostic::io("SPX-H006", message)
}
