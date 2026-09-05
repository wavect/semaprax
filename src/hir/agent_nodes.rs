//! Verified language-native Agent declarations retained in HIR.

use super::DeclarationId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedAgentTypeRoleKind {
    Task,
    State,
    Observation,
    Proposal,
    Outcome,
    Result,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedAgentOperationRoleKind {
    Initialize,
    Observe,
    Propose,
    Authorize,
    Execute,
    Reduce,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedAgentOperationKind {
    Deterministic,
    Model,
    Effect,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedAgentTypeRole {
    pub role: ResolvedAgentTypeRoleKind,
    pub stable_id: DeclarationId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedAgentOperationRole {
    pub role: ResolvedAgentOperationRoleKind,
    pub kind: ResolvedAgentOperationKind,
    pub stable_id: DeclarationId,
}

/// A real HIR node for one parser-admitted Agent declaration. Role bodies are
/// intentionally absent until the deterministic-role execution tranche.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedAgentDeclaration {
    pub stable_id: DeclarationId,
    pub name: String,
    pub types: Vec<ResolvedAgentTypeRole>,
    pub operations: Vec<ResolvedAgentOperationRole>,
    pub runtime_v1_json: String,
}
