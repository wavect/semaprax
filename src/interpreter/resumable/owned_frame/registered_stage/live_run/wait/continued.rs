//! Fixed mapping of the whole actual continued source outcome. This delegate
//! cannot construct a parked owner from inert facts or a detached raw root.
use super::*;
use crate::interpreter::resumable::owned_frame::registered_stage::reduce::{
    enter_continued_wait_v8, EvaluatedContinuedWaitV2, PreparedHeldContinuedWaitV2,
};
use crate::live_invocation::source_journal::SourceJournalError;

pub(crate) struct LiveContinuedParkedStateV8<'j> {
    parked: LiveParkedStateV8,
    predecessor: PreparedHeldContinuedWaitV2<'j>,
}
pub(crate) struct LiveContinuedTerminalStateV8<'j> {
    _terminal: OwnedCopyWaitTerminalV2,
    _predecessor: PreparedHeldContinuedWaitV2<'j>,
    pub(crate) error: Option<SourceJournalError>,
    consumed: u64,
}
pub(crate) enum LiveContinuedWaitStartOutcomeV8<'j> {
    Parked(LiveContinuedParkedStateV8<'j>),
    Refused(PreparedHeldContinuedWaitV2<'j>),
    Terminal(LiveContinuedTerminalStateV8<'j>),
    GuardLost {
        owner: LiveContinuedParkedStateV8<'j>,
        error: SourceJournalError,
    },
}
pub(crate) fn begin_live_continued_wait_v8<'j>(
    permit: LiveWaitStartPermitV8<'_>,
    prepared: PreparedHeldContinuedWaitV2<'j>,
) -> LiveContinuedWaitStartOutcomeV8<'j> {
    let evaluated = match enter_continued_wait_v8(permit, prepared) {
        Ok(owner) => owner,
        Err(owner) => return LiveContinuedWaitStartOutcomeV8::Refused(owner),
    };
    let EvaluatedContinuedWaitV2 {
        outcome,
        consumed,
        predecessor,
        guard_error,
    } = evaluated;
    match outcome {
        OwnedCopyWaitStepV2::Terminal(terminal) => {
            LiveContinuedWaitStartOutcomeV8::Terminal(LiveContinuedTerminalStateV8 {
                _terminal: terminal,
                _predecessor: predecessor,
                error: guard_error,
                consumed,
            })
        }
        OwnedCopyWaitStepV2::Parked(parked) => {
            let owner = LiveContinuedParkedStateV8 {
                parked: LiveParkedStateV8 { parked, consumed },
                predecessor,
            };
            match guard_error {
                Some(error) => LiveContinuedWaitStartOutcomeV8::GuardLost { owner, error },
                None => LiveContinuedWaitStartOutcomeV8::Parked(owner),
            }
        }
    }
}
impl LiveContinuedParkedStateV8<'_> {
    pub(crate) fn consumed(&self) -> u64 {
        self.parked.consumed()
    }
    pub(crate) fn checked_facts(
        &self,
        binding: &CheckedOwnedAgentWaitBindingV8,
    ) -> Option<serde_json::Value> {
        self.predecessor.validate_retained_context().ok()?;
        self.parked.checked_facts(binding)
    }
    pub(crate) fn request(&self) -> &ResumableChannelValue {
        self.parked.request()
    }
    /// The fixed encoder borrows the actual owner internally; no parked/root
    /// getter is exposed even to the source choreography.
    pub(crate) fn encode_checkpoint(
        &self,
        permit: &LiveWaitStartPermitV8<'_>,
        binding: &CheckedOwnedAgentWaitBindingV8,
        key: &crate::resumable_effects::source_checkpoint::SourceCheckpointKey,
        expected: &crate::resumable_effects::owned_frame::v2::OwnedWaitCheckpointExpectationV8<'_>,
    ) -> Result<
        (
            Vec<u8>,
            crate::resumable_effects::owned_frame::v2::CheckedOwnedWaitCheckpointV8,
        ),
        crate::resumable_effects::owned_frame::OwnedFrameError,
    > {
        if !self.predecessor.matches_start_permit(permit) {
            return Err(crate::resumable_effects::owned_frame::OwnedFrameError::Binding);
        }
        permit
            .validate_guard()
            .map_err(|_| crate::resumable_effects::owned_frame::OwnedFrameError::Binding)?;
        self.predecessor
            .validate_retained_context()
            .map_err(|_| crate::resumable_effects::owned_frame::OwnedFrameError::Binding)?;
        let result =
            crate::resumable_effects::owned_frame::v2::encode_live_owned_wait_checkpoint_v8(
                binding,
                key,
                expected,
                &self.parked,
            )?;
        if !self.predecessor.matches_start_permit(permit) {
            return Err(crate::resumable_effects::owned_frame::OwnedFrameError::Binding);
        }
        permit
            .validate_guard()
            .map_err(|_| crate::resumable_effects::owned_frame::OwnedFrameError::Binding)?;
        self.predecessor
            .validate_retained_context()
            .map_err(|_| crate::resumable_effects::owned_frame::OwnedFrameError::Binding)?;
        Ok(result)
    }
}

impl LiveContinuedTerminalStateV8<'_> {
    pub(crate) fn consumed(&self) -> u64 {
        self.consumed
    }
}

#[cfg(test)]
mod tests_support;

pub(in crate::interpreter::resumable::owned_frame::registered_stage) mod model;

impl LiveContinuedParkedStateV8<'_> {
    // Only descriptive root facts for recording an already incurred SDK result.
    pub(crate) fn checked_incurred_facts(
        &self,
        binding: &CheckedOwnedAgentWaitBindingV8,
    ) -> Option<serde_json::Value> {
        self.predecessor.validate_incurred_context().ok()?;
        self.parked.checked_facts(binding)
    }
}
