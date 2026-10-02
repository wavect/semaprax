//! Runtime completion of the already sealed failed Decision-observer tail.
//! No normal journal reopening, arbitrary cleanup, or target redispatch.
use super::super::effect::authorization::observer_failed_state::state::{
    LiveObserverStateAcknowledgedV8, LiveReleasedObserverStateV8,
};
use super::*;

trait RetainedFailure {}
impl<T> RetainedFailure for T {}

pub(super) struct ObserverFailureQuarantineV8<'j> {
    _owner: Box<dyn RetainedFailure + 'j>,
}
pub(super) struct ObserverFailureStoppedV8<'j> {
    _owner: LiveReleasedObserverStateV8<'j>,
}
fn quarantine<'j, T: 'j>(
    journal: &SourceOwnedWaitJournalV8,
    owner: T,
) -> ObserverFailureQuarantineV8<'j> {
    journal.quarantine();
    ObserverFailureQuarantineV8 {
        _owner: Box::new(owner),
    }
}

pub(super) fn settle<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    failed: LiveFailedOwnedEffectV8<'j>,
    observe: impl FnMut(&FinalizeAction),
) -> Result<ObserverFailureStoppedV8<'j>, ObserverFailureQuarantineV8<'j>> {
    macro_rules! join {
        ($operation:expr) => {
            $operation.map_err(|owner| quarantine(journal, owner))?
        };
    }
    macro_rules! ack {
        ($owner:expr) => {{
            let owner = $owner;
            // The actual failed-receipt seal, never normal journal authority,
            // supplies this session. Refusal retains the pending obligation.
            let session = match owner.begin_session() {
                Ok(session) => session,
                Err(error) => return Err(quarantine(journal, (owner, error))),
            };
            join!(join!(session.append_observer_state(owner)).advance_observer_state())
        }};
    }
    let started = match ack!(join!(failed.prepare_observer_state())) {
        LiveObserverStateAcknowledgedV8::Started(owner) => owner,
        owner => return Err(quarantine(journal, owner)),
    };
    let released = join!(started.release(observe));
    let released = match ack!(join!(released.prepare_receipt())) {
        LiveObserverStateAcknowledgedV8::Released(owner) => owner,
        owner => return Err(quarantine(journal, owner)),
    };
    // prepare_stop checks the complete actual State receipt and preserves the
    // selected target failure, or StageRefused after a successful target.
    let released = match ack!(join!(released.prepare_stop())) {
        LiveObserverStateAcknowledgedV8::Released(owner) => owner,
        owner => return Err(quarantine(journal, owner)),
    };
    Ok(ObserverFailureStoppedV8 { _owner: released })
}
