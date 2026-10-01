//! Consuming source entry from the later original Start reservation ACK.
use super::*;
use crate::agent_lifecycle::authorization::target_protocol::TargetAccounting;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::step::r#continue::later::settlement::start::{
    LaterStartEntryFailureV8, LaterStartedWaitV8,
};

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveLaterStartedPhaseV8<'j> {
    owner: LaterStartedWaitV8<'j>,
    observation: CheckedOwnedWaitObservationV8,
    _observe_acks: Vec<ObserveSettlementAckV8<'j>>,
    acks: Vec<LaterStartAckV8<'j>>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveLaterSourceEntryFailureV8<'j>
{
    Before {
        owner: LiveLaterStartPhaseV8<'j>,
        error: SourceJournalError,
    },
    Source {
        owner: LaterStartEntryFailureV8<'j>,
        observation: CheckedOwnedWaitObservationV8,
        observe_acks: Vec<ObserveSettlementAckV8<'j>>,
        acks: Vec<LaterStartAckV8<'j>>,
    },
}

impl<'j> LiveLaterStartPhaseV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn enter_actual_source(
        self,
    ) -> Result<LiveLaterStartedPhaseV8<'j>, LiveLaterSourceEntryFailureV8<'j>> {
        let before = (|| {
            if self.acks.len() != 2 {
                return Err(SourceJournalError::Order);
            }
            self.validate_live()?;
            if !matches!(
                self.acks
                    .last()
                    .expect("actual Start")
                    .witness
                    .selected_row(),
                EntryV8::Owned(journal_model::OwnedBodyV8::OwnedWaitReserved {
                    phase: journal_model::PhaseV8::Start,
                    replay_of: None,
                    ..
                })
            ) {
                return Err(SourceJournalError::Binding);
            }
            Ok(())
        })();
        if let Err(error) = before {
            return Err(LiveLaterSourceEntryFailureV8::Before { owner: self, error });
        }
        let LiveLaterStartPhaseV8 { owner, acks } = self;
        let LiveLaterWaitV8 { owner, observation } = owner;
        let LiveSettledObserveV8 {
            owner,
            acks: observe_acks,
        } = owner;
        let LiveObserveSettlementOwnerV8::Later(owner) = owner else {
            unreachable!("checked later owner")
        };
        let current = acks.last().expect("actual full-fuel Start ACK");
        let started = match owner.enter_actual_wait(&current.session, &current.witness) {
            Ok(owner) => owner,
            Err(owner) => {
                return Err(LiveLaterSourceEntryFailureV8::Source {
                    owner,
                    observation,
                    observe_acks,
                    acks,
                })
            }
        };
        Ok(LiveLaterStartedPhaseV8 {
            owner: started,
            observation,
            _observe_acks: observe_acks,
            acks,
        })
    }
}
impl LiveLaterStartedPhaseV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        let current = self.acks.last().ok_or(SourceJournalError::Order)?;
        self.owner.validate_live(&current.session, &current.witness)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn consumed(
        &self,
    ) -> Option<u64> {
        self.owner.consumed()
    }
    #[cfg(test)]
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn test_ordinary_start(
        &self,
    ) -> (
        serde_json::Value,
        crate::interpreter::resumable::ResumableChannelValue,
        usize,
    ) {
        self.owner.test_ordinary_start()
    }
    #[cfg(test)]
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn test_accounting(
        &self,
    ) -> &TargetAccounting {
        self.owner.test_accounting()
    }
    #[cfg(test)]
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn test_observation(
        &self,
    ) -> &CheckedOwnedWaitObservationV8 {
        &self.observation
    }
}
