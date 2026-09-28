//! Original actual State after complete failed Decision observation.
//! Release requires the sealed actual State Started ACK; no Outcome is minted.
use super::*;
use crate::interpreter::resumable::owned_frame::registered_stage::authorize::{
    ObserverFailedStateReleaseRejectionV8, ObserverFailedStateReleaseV8,
};
use crate::live_invocation::source_journal::LiveObserverFailedStateCleanupPermitV8;
impl PendingOwnedEffectReceiptV8<'_> {
    pub(crate) fn live_observer_failed_state_facts_v8(
        &self,
    ) -> Result<serde_json::Value, SourceJournalError> {
        if self.failure.is_none()
            || self.receipt["kind"] != "observed"
            || self.receipt["settlement"] != "failed"
        {
            return Err(SourceJournalError::Binding);
        }
        self.released
            .observer_failed_state_facts_v8()
            .ok_or(SourceJournalError::Binding)
    }
}

pub(crate) struct ReleasedObserverFailedStateV8<'j> {
    _released: ObserverFailedStateReleaseV8,
    inputs: OwnedEffectInputsV8<'j>,
    receipt: serde_json::Value,
    failure: OwnedEffectFailureV8,
}
impl ReleasedObserverFailedStateV8<'_> {
    pub(crate) fn receipt(&self) -> &serde_json::Value {
        &self.receipt
    }
    pub(crate) fn failure(&self) -> OwnedEffectFailureV8 {
        self.failure
    }
}
pub(crate) enum LiveObserverFailedStateReleaseFailureV8<'j> {
    Before {
        pending: PendingOwnedEffectReceiptV8<'j>,
        error: SourceJournalError,
    },
    Engine {
        pending: PendingOwnedEffectReceiptV8<'j>,
        error: SourceJournalError,
    },
    After {
        released: ReleasedObserverFailedStateV8<'j>,
        error: SourceJournalError,
    },
}

pub(crate) fn release_live_observer_failed_state_v8<'j>(
    pending: PendingOwnedEffectReceiptV8<'j>,
    permit: &LiveObserverFailedStateCleanupPermitV8<'_, 'j>,
    observe: impl FnMut(&FinalizeAction),
) -> Result<ReleasedObserverFailedStateV8<'j>, LiveObserverFailedStateReleaseFailureV8<'j>> {
    let valid = (|| {
        permit.validate_guard(&pending.inputs)?;
        let facts = pending.live_observer_failed_state_facts_v8()?;
        permit.validate_actual_state(pending.inputs.execution.wait().helper(), &facts)?;
        Ok(())
    })();
    if let Err(error) = valid {
        return Err(LiveObserverFailedStateReleaseFailureV8::Before { pending, error });
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
    let actual_failure = failure.expect("checked sticky actual failure");
    let released = match released.release_observer_failed_state_v8(permit, observe) {
        Ok(released) => released,
        Err(ObserverFailedStateReleaseRejectionV8 {
            owner: released,
            error,
        }) => {
            return Err(LiveObserverFailedStateReleaseFailureV8::Engine {
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
    let actual = ReleasedObserverFailedStateV8 {
        _released: released,
        inputs,
        receipt,
        failure: actual_failure,
    };
    match permit.validate_guard(&actual.inputs) {
        Ok(()) => Ok(actual),
        Err(error) => Err(LiveObserverFailedStateReleaseFailureV8::After {
            released: actual,
            error,
        }),
    }
}
