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
    pub embedded: bool,
}

/// A real HIR node for one parser-admitted Agent declaration. Role bodies are
/// retained once as ordinary ResolvedFunction nodes; these fields carry only origin facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedAgentDeclaration {
    pub stable_id: DeclarationId,
    pub name: String,
    pub types: Vec<ResolvedAgentTypeRole>,
    pub operations: Vec<ResolvedAgentOperationRole>,
    pub runtime_v1_json: String,
    pub model_wait: Option<Box<ResolvedAgentModelWaitBinding>>,
    pub(crate) source_association: Option<Box<AgentExecutionSourceAssociation>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedAgentModelWaitBinding {
    pub helper_id: DeclarationId,
}

/// Private structural evidence, not source authority. Private decode is inert:
/// authenticated cache binding and original-source replay are required before use.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentExecutionSourceAssociation {
    pub(crate) module: String,
    pub(crate) operations: Vec<ResolvedAgentOperationRole>,
    pub(crate) model_wait: Option<Box<ResolvedAgentModelWaitBinding>>,
    pub(crate) helper_top_level: bool,
}

impl ResolvedAgentDeclaration {
    pub(crate) fn has_execution_metadata(&self) -> bool {
        self.model_wait.is_some() || self.operations.iter().any(|operation| operation.embedded)
    }

    pub(crate) fn execution_functions_present(
        &self,
        functions: &[super::ResolvedFunction],
    ) -> bool {
        self.operations
            .iter()
            .filter(|operation| operation.kind == ResolvedAgentOperationKind::Deterministic)
            .all(|operation| {
                functions
                    .iter()
                    .any(|function| function.id == operation.stable_id)
            })
            && self.model_wait.as_ref().is_none_or(|binding| {
                functions
                    .iter()
                    .any(|function| function.id == binding.helper_id)
            })
    }

    pub(crate) fn bind_source_association(mut self, module: &str) -> Self {
        if self.has_execution_metadata() {
            self.source_association = Some(Box::new(AgentExecutionSourceAssociation {
                module: module.to_owned(),
                operations: self.operations.clone(),
                model_wait: self.model_wait.clone(),
                helper_top_level: self.model_wait.is_some(),
            }));
        }
        self
    }

    pub(crate) fn source_association_owned_bytes(&self) -> Option<usize> {
        let Some(source) = &self.source_association else {
            return Some(0);
        };
        let mut bytes = std::mem::size_of_val(source.as_ref()).checked_add(source.module.len())?;
        bytes = bytes.checked_add(
            source
                .operations
                .capacity()
                .checked_mul(std::mem::size_of::<ResolvedAgentOperationRole>())?,
        )?;
        for operation in &source.operations {
            bytes = bytes.checked_add(operation.stable_id.as_str().len())?;
        }
        if let Some(binding) = &source.model_wait {
            bytes = bytes
                .checked_add(std::mem::size_of_val(binding.as_ref()))?
                .checked_add(binding.helper_id.as_str().len())?;
        }
        Some(bytes)
    }
}
