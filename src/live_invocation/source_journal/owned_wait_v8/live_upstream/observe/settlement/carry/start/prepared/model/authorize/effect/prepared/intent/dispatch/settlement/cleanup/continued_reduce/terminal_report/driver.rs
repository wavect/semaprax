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

// A terminal owner includes the complete acknowledged run. Moving each owner
// and failure through a separate call keeps debug-build Result temporaries out
// of the enclosing terminal frame, which must fit an ordinary 2 MiB stack.
#[inline(never)]
fn phase<'j, T>(run: impl FnOnce() -> Result<T, Failure<'j>>) -> Result<Box<T>, Box<Failure<'j>>> {
    run().map(Box::new).map_err(Box::new)
}

fn acknowledge<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    current: Phase,
    selected: Box<LiveOwnedStepAppendV8<'j>>,
) -> Result<Box<LiveContinuedStagedStepV8<'j>>, Box<Failure<'j>>> {
    let appended = phase(|| {
        let session = match journal.begin_session() {
            Ok(session) => session,
            Err(error) => {
                return Err(Failure::Session {
                    phase: current,
                    owner: *selected,
                    error,
                })
            }
        };
        session
            .append_owned_step(*selected)
            .map_err(|owner| Failure::Append {
                phase: current,
                owner,
            })
    })?;
    let acknowledged = phase(|| {
        (*appended)
            .advance_step()
            .map_err(|owner| Failure::Advance {
                phase: current,
                owner,
            })
    })?;
    phase(|| match *acknowledged {
        LiveStepAcknowledgedV8::Continued(owner) => Ok(owner),
        owner => {
            journal.quarantine();
            Err(Failure::Shape {
                phase: current,
                owner,
            })
        }
    })
}

#[inline(never)]
fn finish_complete_report_boxed<'j>(
    owner: Box<LiveContinuedStagedStepV8<'j>>,
    journal: &'j SourceOwnedWaitJournalV8,
    observe: impl FnMut(&crate::cleanup_plan::FinalizeAction),
    input: crate::live_invocation::source_journal::SourceTerminalEvidenceInput,
) -> Result<Box<LiveClaimedReportV8<'j>>, Box<Failure<'j>>> {
    let owner = phase(|| {
        let admission = (|| {
            if !std::ptr::eq(journal, owner.journal()) {
                return Err(SourceJournalError::Binding);
            }
            owner.validate_live()?;
            if owner.staged.is_none()
                || owner.cleanup_ack.is_some()
                || !owner.checked_step()?.is_complete()
            {
                return Err(SourceJournalError::Order);
            }
            Ok(())
        })();
        match admission {
            Ok(()) => Ok(*owner),
            Err(error) => {
                owner.journal().quarantine();
                Err(Failure::Select {
                    phase: Phase::Admission,
                    owner: *owner,
                    error,
                })
            }
        }
    })?;
    macro_rules! select {
        ($operation:expr, $current:ident) => {
            phase(|| {
                $operation.map_err(|(owner, error)| Failure::Select {
                    phase: Phase::$current,
                    owner,
                    error,
                })
            })?
        };
    }
    macro_rules! ack {
        ($operation:expr, $current:ident) => {
            acknowledge(journal, Phase::$current, select!($operation, $current))?
        };
    }
    let started = ack!((*owner).prepare_cleanup(), CleanupStarted);
    let released = phase(|| (*started).release(observe).map_err(Failure::Release))?;
    let settled = ack!((*released).prepare_receipt(), CleanupSettled);
    let ready = select!((*settled).into_ready(), Ready);
    let reserved = ack!((*ready).prepare_transfer(), TransferReserved);
    let moved = phase(|| (*reserved).move_fields().map_err(Failure::Move))?;
    let completed = ack!((*moved).prepare_completed(), TransferCompleted);
    let transitioned = ack!((*completed).prepare_transition(), Transition);
    let terminal = ack!((*transitioned).prepare_terminal(input), Terminal);
    phase(|| {
        (*terminal)
            .claim_complete_report()
            .map_err(|(owner, error)| Failure::Select {
                phase: Phase::Claim,
                owner,
                error,
            })
    })
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
    #[cfg(test)]
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn finish_complete_report(
        self,
        journal: &'j SourceOwnedWaitJournalV8,
        observe: impl FnMut(&crate::cleanup_plan::FinalizeAction),
        input: crate::live_invocation::source_journal::SourceTerminalEvidenceInput,
    ) -> Result<LiveClaimedReportV8<'j>, Failure<'j>> {
        finish_complete_report_boxed(Box::new(self), journal, observe, input)
            .map(|owner| *owner)
            .map_err(|failure| *failure)
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
        let claimed = finish_complete_report_boxed(Box::new(self), journal, observe, input)
            .map_err(|failure| *failure)?;
        phase(|| {
            (*claimed)
                .into_delivery_projection()
                .map_err(|(owner, error)| Failure::Delivery { owner, error })
        })
        .map(|projection| *projection)
        .map_err(|failure| *failure)
    }
}
