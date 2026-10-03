//! Continuation provenance is owned: later turns retain their actual prior Step.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::ProspectiveOwnedReduceHoldV8;

pub(super) enum ContinueStepLineageV8<'j> {
    First(StepLineageV8<'j>),
    Later(later::LaterContinueLineageV8<'j>),
}
pub(super) struct ContinueOriginV8<'a, 'j> {
    pub(super) hold: &'a ProspectiveOwnedReduceHoldV8<'j>,
    pub(super) proposal: &'a crate::resumable_effects::owned_frame::v2::CheckedOwnedWaitProposalV8,
    pub(super) policy: &'j crate::resumable_effects::CapabilityPolicy,
    pub(super) cancellation: &'j crate::agent_runtime::AgentCancellation,
    pub(super) clock: &'a dyn crate::live_invocation::SourceInvocationClock,
}
impl<'j> ContinueStepLineageV8<'j> {
    pub(super) fn origin(&self) -> ContinueOriginV8<'_, 'j> {
        match self {
            Self::First(step) => {
                let origin = step.origin();
                ContinueOriginV8 {
                    hold: &origin.hold,
                    proposal: &origin.proposal,
                    policy: origin.policy,
                    cancellation: origin.cancellation,
                    clock: origin.clock,
                }
            }
            Self::Later(lineage) => lineage.join_origin(),
        }
    }
    pub(super) fn journal(&self) -> &'j SourceOwnedWaitJournalV8 {
        match self {
            Self::First(step) => step.journal(),
            Self::Later(lineage) => lineage.join_journal(),
        }
    }
    pub(super) fn current(&self) -> Result<&StepAckV8<'j>, SourceJournalError> {
        match self {
            Self::First(step) => step.current(),
            Self::Later(_) => Err(SourceJournalError::Order),
        }
    }
    pub(super) fn validate_current(&self, incurred: bool) -> Result<(), SourceJournalError> {
        match self {
            Self::First(step) => step.validate_current(incurred),
            Self::Later(_) => Err(SourceJournalError::Order),
        }
    }
    #[cfg(test)]
    pub(super) fn first_mut(&mut self) -> &mut StepLineageV8<'j> {
        match self {
            Self::First(step) => step,
            Self::Later(_) => panic!("first continuation test hook"),
        }
    }
}
