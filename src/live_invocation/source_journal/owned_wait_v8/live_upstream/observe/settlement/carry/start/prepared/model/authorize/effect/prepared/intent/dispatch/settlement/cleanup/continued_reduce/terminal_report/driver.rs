//! Consuming private Complete closure; every failure retains its reached owner.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::LiveOwnedStepAppendFailureV8;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::step::{
    LiveOwnedStepAppendV8, LiveStepAcknowledgedV8, LiveStepAdvanceFailureV8,
};
use super::super::step_transfer::LiveContinuedStepMoveFailureV8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveContinuedTerminalPhaseV8 {
    Admission,
    CleanupStarted,
    CleanupSettled,
    Ready,
    TransferReserved,
    TransferCompleted,
    Transition,
    Terminal,
    Claim,
}

pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveContinuedTerminalDriverFailureV8<
    'j,
> {
    Select {
        phase: LiveContinuedTerminalPhaseV8,
        owner: LiveContinuedStagedStepV8<'j>,
        error: SourceJournalError,
    },
    Session {
        phase: LiveContinuedTerminalPhaseV8,
        owner: LiveOwnedStepAppendV8<'j>,
        error: SourceJournalError,
    },
    Append {
        phase: LiveContinuedTerminalPhaseV8,
        owner: LiveOwnedStepAppendFailureV8<'j>,
    },
    Advance {
        phase: LiveContinuedTerminalPhaseV8,
        owner: LiveStepAdvanceFailureV8<'j>,
    },
    Shape {
        phase: LiveContinuedTerminalPhaseV8,
        owner: LiveStepAcknowledgedV8<'j>,
    },
    Delivery {
        owner: LiveClaimedReportV8<'j>,
        error: SourceJournalError,
    },
    Release(LiveContinuedStepCleanupFailureV8<'j>),
    Move(LiveContinuedStepMoveFailureV8<'j>),
}

type Failure<'j> = LiveContinuedTerminalDriverFailureV8<'j>;
type Phase = LiveContinuedTerminalPhaseV8;

fn select<'j>(
    phase: Phase,
    result: Result<
        LiveContinuedStagedStepV8<'j>,
        (LiveContinuedStagedStepV8<'j>, SourceJournalError),
    >,
) -> Result<LiveContinuedStagedStepV8<'j>, Failure<'j>> {
    result.map_err(|(owner, error)| Failure::Select {
        phase,
        owner,
        error,
    })
}

fn acknowledge<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    phase: Phase,
    selected: Result<
        LiveOwnedStepAppendV8<'j>,
        (LiveContinuedStagedStepV8<'j>, SourceJournalError),
    >,
) -> Result<LiveContinuedStagedStepV8<'j>, Failure<'j>> {
    let owner = selected.map_err(|(owner, error)| Failure::Select {
        phase,
        owner,
        error,
    })?;
    let session = match journal.begin_session() {
        Ok(session) => session,
        Err(error) => {
            return Err(Failure::Session {
                phase,
                owner,
                error,
            })
        }
    };
    let acknowledged = session
        .append_owned_step(owner)
        .map_err(|owner| Failure::Append { phase, owner })?
        .advance_step()
        .map_err(|owner| Failure::Advance { phase, owner })?;
    match acknowledged {
        LiveStepAcknowledgedV8::Continued(owner) => Ok(owner),
        owner => {
            journal.quarantine();
            Err(Failure::Shape { phase, owner })
        }
    }
}

impl<'j> LiveContinuedStagedStepV8<'j> {
    #[cfg(test)]
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn test_terminal_admission_shape(
        &self,
    ) -> (bool, bool, Option<serde_json::Value>) {
        (
            self.staged.is_some(),
            self.cleanup_ack.is_some(),
            self.facts.step().map(|step| step["case"].clone()),
        )
    }

    /// Completes the actual continued Complete Step through the existing six
    /// physical ACKs. No failure reconstructs an owner or replays a finalizer.
    /// The result remains private and borrowed from this registered store.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn finish_complete_report(
        self,
        journal: &'j SourceOwnedWaitJournalV8,
        observe: impl FnMut(&crate::cleanup_plan::FinalizeAction),
        input: crate::live_invocation::source_journal::SourceTerminalEvidenceInput,
    ) -> Result<LiveClaimedReportV8<'j>, Failure<'j>> {
        let admission = (|| {
            if !std::ptr::eq(journal, self.journal()) {
                return Err(SourceJournalError::Binding);
            }
            self.validate_live()?;
            if self.staged.is_none()
                || self.cleanup_ack.is_some()
                || !self.checked_step()?.is_complete()
            {
                return Err(SourceJournalError::Order);
            }
            Ok(())
        })();
        if let Err(error) = admission {
            self.journal().quarantine();
            return Err(Failure::Select {
                phase: Phase::Admission,
                owner: self,
                error,
            });
        }
        let started = acknowledge(journal, Phase::CleanupStarted, self.prepare_cleanup())?;
        let released = started.release(observe).map_err(Failure::Release)?;
        let settled = acknowledge(journal, Phase::CleanupSettled, released.prepare_receipt())?;
        let ready = select(Phase::Ready, settled.into_ready())?;
        let reserved = acknowledge(journal, Phase::TransferReserved, ready.prepare_transfer())?;
        let moved = reserved.move_fields().map_err(Failure::Move)?;
        let completed = acknowledge(journal, Phase::TransferCompleted, moved.prepare_completed())?;
        let transitioned = acknowledge(journal, Phase::Transition, completed.prepare_transition())?;
        let terminal = acknowledge(
            journal,
            Phase::Terminal,
            transitioned.prepare_terminal(input),
        )?;
        terminal
            .claim_complete_report()
            .map_err(|(owner, error)| Failure::Select {
                phase: Phase::Claim,
                owner,
                error,
            })
    }

    /// Completes the authenticated physical terminal path and releases only
    /// its canonical Report projection and exact terminal evidence bytes. No
    /// State, Report, journal lease, or terminal owner crosses this boundary.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn finish_complete_projection(
        self,
        journal: &'j SourceOwnedWaitJournalV8,
        observe: impl FnMut(&crate::cleanup_plan::FinalizeAction),
        input: crate::live_invocation::source_journal::SourceTerminalEvidenceInput,
    ) -> Result<serde_json::Value, Failure<'j>> {
        let claimed = self.finish_complete_report(journal, observe, input)?;
        claimed
            .into_delivery_projection()
            .map_err(|(owner, error)| Failure::Delivery { owner, error })
    }
}
