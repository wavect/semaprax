//! Closed sum of the two actual live cleanup permits, with no caller guard.
use super::*;
use crate::live_invocation::source_journal::LiveContinuedDecisionCleanupPermitV8;
pub(crate) enum DecisionCleanupGuardV8<'g, 'p, 'j> {
    Initial(&'g LiveEffectDecisionCleanupPermitV8<'p, 'j>),
    Continued(&'g LiveContinuedDecisionCleanupPermitV8<'p, 'j>),
}
impl DecisionCleanupGuardV8<'_, '_, '_> {
    pub(super) fn validate_guard(
        &self,
        inputs: &OwnedEffectInputsV8<'_>,
    ) -> Result<(), SourceJournalError> {
        match self {
            Self::Initial(p) => p.validate_guard(inputs),
            Self::Continued(p) => p.validate_guard(inputs),
        }
    }
    pub(super) fn validate_cleanup_current(&self) -> Result<(), SourceJournalError> {
        match self {
            Self::Initial(p) => p.validate_cleanup_current(),
            Self::Continued(p) => p.validate_cleanup_current(),
        }
    }
    pub(super) fn references(
        &self,
    ) -> Result<(u32, u32, u32, u32, u32, u32, u32), SourceJournalError> {
        match self {
            Self::Initial(p) => p.references(),
            Self::Continued(p) => p.references(),
        }
    }
    pub(super) fn operations(&self) -> Result<&serde_json::Value, SourceJournalError> {
        match self {
            Self::Initial(p) => p.operations(),
            Self::Continued(p) => p.operations(),
        }
    }
    pub(super) fn matches_settlement(
        &self,
        intent: u32,
        evidence: &str,
        observation: Option<&[u8]>,
        reason: Option<crate::live_invocation::source_journal::SourceEffectFailure>,
    ) -> bool {
        match self {
            Self::Initial(p) => p.matches_settlement(intent, evidence, observation, reason),
            Self::Continued(p) => p.matches_settlement(intent, evidence, observation, reason),
        }
    }
}
