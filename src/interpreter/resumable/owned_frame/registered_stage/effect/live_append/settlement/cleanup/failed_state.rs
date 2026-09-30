//! Failed-target State cleanup consumes the actual Decision-released owner.
//! No accepted payload, Outcome, reducer admission or raw ACK factory exists.
use super::*;
use crate::interpreter::resumable::owned_frame::registered_stage::authorize::{
    FailedEffectStateReleaseRejectionV8, FailedEffectStateReleaseV8,
};
use crate::live_invocation::source_journal::LiveFailedEffectStateCleanupPermitV8;

pub(crate) struct ReleasedFailedEffectStateV8<'j> {
    _released: FailedEffectStateReleaseV8,
    inputs: OwnedEffectInputsV8<'j>,
    receipt: serde_json::Value,
    failure: OwnedEffectFailureV8,
}
impl ReleasedFailedEffectStateV8<'_> {
    pub(crate) fn receipt(&self) -> &serde_json::Value {
        &self.receipt
    }
    pub(crate) fn failure(&self) -> OwnedEffectFailureV8 {
        self.failure
    }
}
pub(crate) enum LiveFailedEffectStateReleaseFailureV8<'j> {
    Before {
        pending: PendingOwnedEffectReceiptV8<'j>,
        error: SourceJournalError,
    },
    Engine {
        pending: PendingOwnedEffectReceiptV8<'j>,
        error: SourceJournalError,
    },
    After {
        released: ReleasedFailedEffectStateV8<'j>,
        error: SourceJournalError,
    },
}
impl PendingOwnedEffectReceiptV8<'_> {
    pub(crate) fn live_failed_state_reason_v8(&self) -> Option<SourceEffectFailure> {
        match self.failure? {
            OwnedEffectFailureV8::Target(Settlement::ResultBudget) => {
                Some(SourceEffectFailure::ResultLimit)
            }
            OwnedEffectFailureV8::Target(
                Settlement::HostFailed
                | Settlement::HostPanicked
                | Settlement::MalformedResult
                | Settlement::ResultTypeMismatch,
            )
            | OwnedEffectFailureV8::ResultShape => Some(SourceEffectFailure::HandlerFailed),
            _ => None,
        }
    }
    pub(crate) fn live_failed_state_facts_v8(
        &self,
    ) -> Result<serde_json::Value, SourceJournalError> {
        if !matches!(
            self.failure,
            Some(
                OwnedEffectFailureV8::Target(
                    Settlement::HostFailed
                        | Settlement::HostPanicked
                        | Settlement::ResultBudget
                        | Settlement::MalformedResult
                        | Settlement::ResultTypeMismatch
                ) | OwnedEffectFailureV8::ResultShape
            )
        ) || self.accepted.is_some()
            || self.receipt["settlement"] != "completed"
        {
            return Err(SourceJournalError::Binding);
        }
        self.released
            .failed_state_facts_v8()
            .ok_or(SourceJournalError::Binding)
    }
}
pub(crate) fn release_live_failed_effect_state_v8<'j>(
    pending: PendingOwnedEffectReceiptV8<'j>,
    permit: &LiveFailedEffectStateCleanupPermitV8<'_, 'j>,
    observe: impl FnMut(&FinalizeAction),
) -> Result<ReleasedFailedEffectStateV8<'j>, LiveFailedEffectStateReleaseFailureV8<'j>> {
    let valid = (|| {
        permit.validate_guard(&pending.inputs)?;
        let facts = pending.live_failed_state_facts_v8()?;
        permit.validate_actual_state(pending.inputs.execution.wait().helper(), &facts)?;
        Ok(())
    })();
    if let Err(error) = valid {
        return Err(LiveFailedEffectStateReleaseFailureV8::Before { pending, error });
    }
    let PendingOwnedEffectReceiptV8 {
        released,
        plan,
        inputs,
        basis,
        started,
        receipt,
        accepted,
        failure,
        creator,
    } = pending;
    let actual_failure = failure.expect("checked actual target failure");
    let released = match released.release_failed_state_v8(permit, observe) {
        Ok(released) => released,
        Err(FailedEffectStateReleaseRejectionV8 {
            owner: released,
            error,
        }) => {
            return Err(LiveFailedEffectStateReleaseFailureV8::Engine {
                pending: PendingOwnedEffectReceiptV8 {
                    released,
                    plan,
                    inputs,
                    basis,
                    started,
                    receipt,
                    accepted,
                    failure,
                    creator,
                },
                error,
            })
        }
    };
    let receipt = released.receipt().clone();
    let actual = ReleasedFailedEffectStateV8 {
        _released: released,
        inputs,
        receipt,
        failure: actual_failure,
    };
    match permit.validate_guard(&actual.inputs) {
        Ok(()) => Ok(actual),
        Err(error) => Err(LiveFailedEffectStateReleaseFailureV8::After {
            released: actual,
            error,
        }),
    }
}
