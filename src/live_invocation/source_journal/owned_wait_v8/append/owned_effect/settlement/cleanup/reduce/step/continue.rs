//! Fixed Continue ACK lineage. The witness is never an owner or dispatch grant.
use super::*;

pub(in crate::live_invocation::source_journal::owned_wait_v8) mod run;

/// Only the fixed physical adapter constructs this after persisted/reread ACK.
/// No Clone, public constructor or independently detachable cursor exists.
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct VerifiedOwnedContinueSuccessorV8<
    'j,
> {
    predecessor: OwnedEffectAppendCursorV8<'j>,
    successor: OwnedEffectAppendCursorV8<'j>,
    selected: EntryV8,
}
impl VerifiedOwnedContinueSuccessorV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_predecessor(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        sequence: usize,
        bytes: usize,
        selected: &EntryV8,
    ) -> Result<(), SourceJournalError> {
        self.validate_predecessor_data(journal, sequence, bytes, selected)?;
        self.successor.validate_current()
    }
    fn validate_predecessor_data(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        sequence: usize,
        bytes: usize,
        selected: &EntryV8,
    ) -> Result<(), SourceJournalError> {
        if !std::ptr::eq(self.predecessor.journal, journal)
            || !std::ptr::eq(self.successor.journal, journal)
            || self.predecessor.sequence != sequence
            || self.predecessor.bytes != bytes
            || &self.selected != selected
            || !matches!(selected,
                EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedStateCommitted{..})
                |EntryV8::Ordinary(SourceJournalEntry::StageReservation{role:crate::live_invocation::source_journal::SourceStageRole::Observe,attempt:None,..}))
            || self.successor.sequence
                != sequence
                    .checked_add(1)
                    .ok_or(SourceJournalError::Capacity)?
            || self.successor.bytes <= bytes
            || self.predecessor.authentication == self.successor.authentication
        {
            return Err(SourceJournalError::Binding);
        }
        Ok(())
    }
    /// Callback-free comparison to the actual acknowledged session. Unlike the
    /// fresh validator this never touches a file or reenters the append marker.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_against_acknowledged_session(
        &self,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        self.validate_predecessor_data(
            session.journal,
            self.predecessor.sequence,
            self.predecessor.bytes,
            &self.selected,
        )?;
        let (_, _, _, selected) = session.inventory.continuation_facts()?;
        if !session
            .inventory
            .belongs_to_context(&session.journal.context)
            || session.sequence() != self.successor.sequence
            || session.acknowledged_bytes() != self.successor.bytes
            || session.inventory.authentication_tail() != self.successor.authentication
            || selected != &self.selected
        {
            return Err(SourceJournalError::Binding);
        }
        Ok(())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_current_session(
        &self,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        self.validate_against_acknowledged_session(session)?;
        self.successor.validate_current()
    }
    /// Borrow-only old registry lineage comparison; never a cursor setter.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_previous_registry(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        sequence: usize,
        bytes: usize,
        authentication: &str,
    ) -> Result<(), SourceJournalError> {
        self.validate_predecessor_data(journal, sequence, bytes, &self.selected)?;
        if self.predecessor.authentication != authentication {
            return Err(SourceJournalError::Binding);
        }
        Ok(())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn selected_row(
        &self,
    ) -> &EntryV8 {
        &self.selected
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn sequence(&self) -> usize {
        self.successor.sequence
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn acknowledged_bytes(
        &self,
    ) -> usize {
        self.successor.bytes
    }
}

use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::step::LiveMovedStepV8;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::step::r#continue::{
    advance_verified_continue_v8, LiveContinueAcknowledgedV8, LiveContinueFailureV8,
    LiveObservedContinueV8, LiveOwnedContinueAppendV8,
};
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::{
    LiveContinuedAuthorizationAdmissionFailureV8, LiveContinuedAuthorizationFailureV8,
    LiveContinuedAuthorizationV8, LiveContinuedAuthorizeAcknowledgmentFailureV8,
    LiveContinuedEffectAcknowledgmentFailureV8, LiveContinuedEffectAdmissionFailureV8,
    LiveContinuedEffectFailureV8, LiveContinuedEffectV8, LiveOwnedContinuedEffectAppendV8,
    LiveContinuedModelAcknowledgmentFailureV8, LiveContinuedModelPreparationFailureV8,
    LiveContinuedModelFailureV8, LiveContinuedModelV8, LiveOwnedContinuedModelAppendV8,
    LiveContinuedPreparedFailureV8, LiveContinuedPreparedPhaseV8,
    LiveContinuedSourceEntryFailureV8, LiveContinuedStartPhaseV8,
    LiveOwnedContinuedPreparedAppendV8,
};
use crate::live_invocation::source_journal::owned_wait_v8::append::{
    LiveOwnedContinuedAuthorizeAppendFailureV8, LiveOwnedContinuedModelAppendFailureV8,
    LiveOwnedContinuedEffectAppendFailureV8, LiveOwnedContinuedPreparedAppendFailureV8,
};

/// The actual unchanged obligation is first. Session/witness never stand alone.
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct VerifiedOwnedContinueAppendV8<
    'j,
> {
    obligation: LiveOwnedContinueAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedContinueSuccessorV8<'j>,
}
impl<'j> VerifiedOwnedContinueAppendV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        self.obligation
            .validate_continue_successor(&self.witness, &self.session)
            .inspect_err(|_| self.session.journal.quarantine())
    }
    /// Closed move into the actual engine ACK consumer; no host or parts API.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_continue(
        self,
    ) -> Result<LiveContinueAcknowledgedV8<'j>, LiveContinueFailureV8<'j>> {
        let Self {
            obligation,
            session,
            witness,
        } = self;
        advance_verified_continue_v8(obligation, session, witness)
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveOwnedContinueAppendFailureV8<
    'j,
> {
    Before {
        _obligation: LiveOwnedContinueAppendV8<'j>,
        _session: AppendSessionV8<'j>,
        error: SourceJournalError,
    },
    Append {
        _obligation: LiveOwnedContinueAppendV8<'j>,
        _failure: AppendFailureV8<'j>,
    },
    Acknowledged {
        _obligation: LiveOwnedContinueAppendV8<'j>,
        _session: AppendSessionV8<'j>,
        _witness: VerifiedOwnedContinueSuccessorV8<'j>,
        error: SourceJournalError,
    },
    After {
        _verified: VerifiedOwnedContinueAppendV8<'j>,
        error: SourceJournalError,
    },
}

/// Owns the unique live holder at every failed continuation boundary. The
/// authenticated rows remain inside these variants: receipt bytes cannot
/// reconstruct State or re-enter Observe.
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveContinueDriverFailureV8<'j> {
    Prepare(LiveContinueFailureV8<'j>),
    StateSession {
        owner: LiveOwnedContinueAppendV8<'j>,
        error: SourceJournalError,
    },
    StateAppend(LiveOwnedContinueAppendFailureV8<'j>),
    StateAdvance(LiveContinueFailureV8<'j>),
    StateAcknowledged(LiveContinueAcknowledgedV8<'j>),
    ObservePrepare(LiveContinueFailureV8<'j>),
    ObserveSession {
        owner: LiveOwnedContinueAppendV8<'j>,
        error: SourceJournalError,
    },
    ObserveAppend(LiveOwnedContinueAppendFailureV8<'j>),
    ObserveAdvance(LiveContinueFailureV8<'j>),
    ObserveAcknowledged(LiveContinueAcknowledgedV8<'j>),
}

/// Advances an actual Step::Continue owner through its two fixed durable
/// acknowledgements. A future public driver may call this narrow boundary;
/// it accepts no snapshot and exposes no append receipt as source authority.
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_live_owned_continue_v8<
    'j,
>(
    journal: &'j SourceOwnedWaitJournalV8,
    moved: LiveMovedStepV8<'j>,
) -> Result<LiveObservedContinueV8<'j>, LiveContinueDriverFailureV8<'j>> {
    let state_append = moved
        .prepare_continue()
        .map_err(LiveContinueDriverFailureV8::Prepare)?;
    let state_session = match journal.begin_session() {
        Ok(session) => session,
        Err(error) => {
            return Err(LiveContinueDriverFailureV8::StateSession {
                owner: state_append,
                error,
            });
        }
    };
    let state_ack = state_session
        .append_owned_continue(state_append)
        .map_err(LiveContinueDriverFailureV8::StateAppend)?;
    let state = match state_ack
        .advance_continue()
        .map_err(LiveContinueDriverFailureV8::StateAdvance)?
    {
        LiveContinueAcknowledgedV8::State(state) => state,
        acknowledged => return Err(LiveContinueDriverFailureV8::StateAcknowledged(acknowledged)),
    };
    let observe_append = state
        .prepare_observe()
        .map_err(LiveContinueDriverFailureV8::ObservePrepare)?;
    let observe_session = match journal.begin_session() {
        Ok(session) => session,
        Err(error) => {
            return Err(LiveContinueDriverFailureV8::ObserveSession {
                owner: observe_append,
                error,
            });
        }
    };
    let observe_ack = observe_session
        .append_owned_continue(observe_append)
        .map_err(LiveContinueDriverFailureV8::ObserveAppend)?;
    match observe_ack
        .advance_continue()
        .map_err(LiveContinueDriverFailureV8::ObserveAdvance)?
    {
        LiveContinueAcknowledgedV8::Observed(observed) => Ok(observed),
        acknowledged => Err(LiveContinueDriverFailureV8::ObserveAcknowledged(
            acknowledged,
        )),
    }
}

/// Moves a next-turn Start owner through its single actual source entry and
/// durable Prepared acknowledgement. The returned owner can later be given to
/// the Model boundary, while every failure retains the unique live holder.
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveContinuedStartDriverFailureV8<
    'j,
> {
    Source(LiveContinuedSourceEntryFailureV8<'j>),
    Checkpoint(LiveContinuedPreparedFailureV8<'j>),
    PreparedSession {
        owner: LiveOwnedContinuedPreparedAppendV8<'j>,
        error: SourceJournalError,
    },
    PreparedAppend(LiveOwnedContinuedPreparedAppendFailureV8<'j>),
    PreparedAdvance(LiveContinuedPreparedFailureV8<'j>),
}

/// Consumes only an actual two-ACK Start owner. Receipt bytes cannot invoke
/// the source evaluator or recover authority for the prepared Model owner.
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_live_owned_continued_start_v8<
    'j,
>(
    journal: &'j SourceOwnedWaitJournalV8,
    start: LiveContinuedStartPhaseV8<'j>,
) -> Result<LiveContinuedPreparedPhaseV8<'j>, LiveContinuedStartDriverFailureV8<'j>> {
    let started = start
        .enter_actual_source()
        .map_err(LiveContinuedStartDriverFailureV8::Source)?;
    let prepared = started
        .prepare_checkpoint()
        .map_err(LiveContinuedStartDriverFailureV8::Checkpoint)?;
    let session = match journal.begin_session() {
        Ok(session) => session,
        Err(error) => {
            return Err(LiveContinuedStartDriverFailureV8::PreparedSession {
                owner: prepared,
                error,
            });
        }
    };
    session
        .append_owned_continued_prepared(prepared)
        .map_err(LiveContinuedStartDriverFailureV8::PreparedAppend)?
        .advance_continued_prepared()
        .map_err(LiveContinuedStartDriverFailureV8::PreparedAdvance)
}

/// Retains the one real Prepared owner at every Model-row boundary. This
/// driver stops before dispatch, so it cannot invoke an SDK or resume source.
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveContinuedModelDriverFailureV8<
    'j,
> {
    Prepare(LiveContinuedModelPreparationFailureV8<'j>),
    ModelSession {
        owner: LiveOwnedContinuedModelAppendV8<'j>,
        error: SourceJournalError,
    },
    ModelAppend(LiveOwnedContinuedModelAppendFailureV8<'j>),
    ModelAdvance(LiveContinuedModelAcknowledgmentFailureV8<'j>),
}

/// Binds the actual Prepared owner to one checked Model request and one
/// durable Model acknowledgement. It takes the live adapter directly; no
/// receipt can reconstruct a request origin or obtain SDK authority.
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_live_owned_continued_model_v8<
    'j,
>(
    journal: &'j SourceOwnedWaitJournalV8,
    prepared: LiveContinuedPreparedPhaseV8<'j>,
    adapter: &crate::provider_adapter_sdk::StreamingSourceProposalAdapter<'_>,
) -> Result<LiveContinuedModelV8<'j>, LiveContinuedModelDriverFailureV8<'j>> {
    let selected = prepared
        .prepare_model_intent(adapter)
        .map_err(LiveContinuedModelDriverFailureV8::Prepare)?;
    let session = match journal.begin_session() {
        Ok(session) => session,
        Err(error) => {
            return Err(LiveContinuedModelDriverFailureV8::ModelSession {
                owner: selected,
                error,
            });
        }
    };
    session
        .append_owned_continued_model(selected)
        .map_err(LiveContinuedModelDriverFailureV8::ModelAppend)?
        .advance_continued_model()
        .map_err(LiveContinuedModelDriverFailureV8::ModelAdvance)
}

/// Holds the actual dispatched Model owner until its response is durable. A
/// failed Settled ACK retains that owner and cannot send the SDK request again.
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveContinuedDispatchDriverFailureV8<
    'j,
> {
    Dispatch(LiveContinuedModelFailureV8<'j>),
    SettledPrepare(LiveContinuedModelFailureV8<'j>),
    SettledSession {
        owner: LiveOwnedContinuedModelAppendV8<'j>,
        error: SourceJournalError,
    },
    SettledAppend(LiveOwnedContinuedModelAppendFailureV8<'j>),
    SettledAdvance(LiveContinuedModelAcknowledgmentFailureV8<'j>),
}

/// Dispatches the one already-acknowledged Model request, then records its
/// settled response before any Usage, Resume, or authorization transition.
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_live_owned_continued_dispatch_v8<
    'j,
>(
    journal: &'j SourceOwnedWaitJournalV8,
    model: LiveContinuedModelV8<'j>,
    adapter: &mut crate::provider_adapter_sdk::StreamingSourceProposalAdapter<'_>,
) -> Result<LiveContinuedModelV8<'j>, LiveContinuedDispatchDriverFailureV8<'j>> {
    let dispatched = model
        .dispatch_model(adapter)
        .map_err(LiveContinuedDispatchDriverFailureV8::Dispatch)?;
    let settled = dispatched
        .prepare_next()
        .map_err(LiveContinuedDispatchDriverFailureV8::SettledPrepare)?;
    let session = match journal.begin_session() {
        Ok(session) => session,
        Err(error) => {
            return Err(LiveContinuedDispatchDriverFailureV8::SettledSession {
                owner: settled,
                error,
            });
        }
    };
    session
        .append_owned_continued_model(settled)
        .map_err(LiveContinuedDispatchDriverFailureV8::SettledAppend)?
        .advance_continued_model()
        .map_err(LiveContinuedDispatchDriverFailureV8::SettledAdvance)
}

/// Carries each unique Model owner through the two durable boundaries that
/// precede the one actual resumed-program evaluation.
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveContinuedResumeDriverFailureV8<
    'j,
> {
    UsagePrepare(LiveContinuedModelFailureV8<'j>),
    UsageSession {
        owner: LiveOwnedContinuedModelAppendV8<'j>,
        error: SourceJournalError,
    },
    UsageAppend(LiveOwnedContinuedModelAppendFailureV8<'j>),
    UsageAdvance(LiveContinuedModelAcknowledgmentFailureV8<'j>),
    ResumePrepare(LiveContinuedModelFailureV8<'j>),
    ResumeSession {
        owner: LiveOwnedContinuedModelAppendV8<'j>,
        error: SourceJournalError,
    },
    ResumeAppend(LiveOwnedContinuedModelAppendFailureV8<'j>),
    ResumeAdvance(LiveContinuedModelAcknowledgmentFailureV8<'j>),
    ResumeActual(LiveContinuedModelFailureV8<'j>),
}

/// Acknowledges Usage and the Resume reservation before the single resumed
/// source evaluation. It stops before publishing the resulting Completed row.
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_live_owned_continued_resume_v8<
    'j,
>(
    journal: &'j SourceOwnedWaitJournalV8,
    model: LiveContinuedModelV8<'j>,
) -> Result<LiveContinuedModelV8<'j>, LiveContinuedResumeDriverFailureV8<'j>> {
    let usage = model
        .prepare_next()
        .map_err(LiveContinuedResumeDriverFailureV8::UsagePrepare)?;
    let usage_session = match journal.begin_session() {
        Ok(session) => session,
        Err(error) => {
            return Err(LiveContinuedResumeDriverFailureV8::UsageSession {
                owner: usage,
                error,
            });
        }
    };
    let usage = usage_session
        .append_owned_continued_model(usage)
        .map_err(LiveContinuedResumeDriverFailureV8::UsageAppend)?
        .advance_continued_model()
        .map_err(LiveContinuedResumeDriverFailureV8::UsageAdvance)?;
    let resume = usage
        .prepare_next()
        .map_err(LiveContinuedResumeDriverFailureV8::ResumePrepare)?;
    let resume_session = match journal.begin_session() {
        Ok(session) => session,
        Err(error) => {
            return Err(LiveContinuedResumeDriverFailureV8::ResumeSession {
                owner: resume,
                error,
            });
        }
    };
    let reserved = resume_session
        .append_owned_continued_model(resume)
        .map_err(LiveContinuedResumeDriverFailureV8::ResumeAppend)?
        .advance_continued_model()
        .map_err(LiveContinuedResumeDriverFailureV8::ResumeAdvance)?;
    reserved
        .resume_actual()
        .map_err(LiveContinuedResumeDriverFailureV8::ResumeActual)
}

/// Keeps the resumed owner inside every failure until its Completed row is
/// durably acknowledged; authorization begins from the returned owner later.
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveContinuedCompletedDriverFailureV8<
    'j,
> {
    Prepare(LiveContinuedModelFailureV8<'j>),
    Session {
        owner: LiveOwnedContinuedModelAppendV8<'j>,
        error: SourceJournalError,
    },
    Append(LiveOwnedContinuedModelAppendFailureV8<'j>),
    Advance(LiveContinuedModelAcknowledgmentFailureV8<'j>),
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_live_owned_continued_completed_v8<
    'j,
>(
    journal: &'j SourceOwnedWaitJournalV8,
    model: LiveContinuedModelV8<'j>,
) -> Result<LiveContinuedModelV8<'j>, LiveContinuedCompletedDriverFailureV8<'j>> {
    let completed = model
        .prepare_next()
        .map_err(LiveContinuedCompletedDriverFailureV8::Prepare)?;
    let session = match journal.begin_session() {
        Ok(session) => session,
        Err(error) => {
            return Err(LiveContinuedCompletedDriverFailureV8::Session {
                owner: completed,
                error,
            });
        }
    };
    session
        .append_owned_continued_model(completed)
        .map_err(LiveContinuedCompletedDriverFailureV8::Append)?
        .advance_continued_model()
        .map_err(LiveContinuedCompletedDriverFailureV8::Advance)
}
/// Carries the sole live authorization owner across each of its four durable
/// acknowledgements, including transfer and guarded stage entry.
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveContinuedAuthorizeDriverFailureV8<
    'j,
> {
    Prepare(LiveContinuedAuthorizationAdmissionFailureV8<'j>),
    Next(LiveContinuedAuthorizationFailureV8<'j>),
    Session {
        owner: crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveOwnedContinuedAuthorizeAppendV8<'j>,
        error: SourceJournalError,
    },
    Append(LiveOwnedContinuedAuthorizeAppendFailureV8<'j>),
    Advance(LiveContinuedAuthorizeAcknowledgmentFailureV8<'j>),
}
fn ack_continued_authorize_v8<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    owner: crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveOwnedContinuedAuthorizeAppendV8<'j>,
) -> Result<LiveContinuedAuthorizationV8<'j>, LiveContinuedAuthorizeDriverFailureV8<'j>> {
    let session = match journal.begin_session() {
        Ok(session) => session,
        Err(error) => {
            return Err(LiveContinuedAuthorizeDriverFailureV8::Session { owner, error });
        }
    };
    session
        .append_owned_continued_authorize(owner)
        .map_err(LiveContinuedAuthorizeDriverFailureV8::Append)?
        .advance_continued_authorize()
        .map_err(LiveContinuedAuthorizeDriverFailureV8::Advance)
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_live_owned_continued_authorize_v8<
    'j,
>(
    journal: &'j SourceOwnedWaitJournalV8,
    completed: LiveContinuedModelV8<'j>,
) -> Result<LiveContinuedAuthorizationV8<'j>, LiveContinuedAuthorizeDriverFailureV8<'j>> {
    let one = completed
        .prepare_authorize()
        .map_err(LiveContinuedAuthorizeDriverFailureV8::Prepare)?;
    let two = ack_continued_authorize_v8(journal, one)?
        .prepare_next()
        .map_err(LiveContinuedAuthorizeDriverFailureV8::Next)?;
    let three = ack_continued_authorize_v8(journal, two)?
        .prepare_next()
        .map_err(LiveContinuedAuthorizeDriverFailureV8::Next)?;
    let four = ack_continued_authorize_v8(journal, three)?
        .prepare_next()
        .map_err(LiveContinuedAuthorizeDriverFailureV8::Next)?;
    let five = ack_continued_authorize_v8(journal, four)?
        .prepare_next()
        .map_err(LiveContinuedAuthorizeDriverFailureV8::Next)?;
    ack_continued_authorize_v8(journal, five)
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveContinuedEffectDriverFailureV8<
    'j,
> {
    Prepare(LiveContinuedEffectAdmissionFailureV8<'j>),
    Next(LiveContinuedEffectFailureV8<'j>),
    Session {
        owner: LiveOwnedContinuedEffectAppendV8<'j>,
        error: SourceJournalError,
    },
    Append(LiveOwnedContinuedEffectAppendFailureV8<'j>),
    Advance(LiveContinuedEffectAcknowledgmentFailureV8<'j>),
}
fn ack_continued_effect_v8<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    owner: LiveOwnedContinuedEffectAppendV8<'j>,
) -> Result<LiveContinuedEffectV8<'j>, LiveContinuedEffectDriverFailureV8<'j>> {
    let session = match journal.begin_session() {
        Ok(x) => x,
        Err(error) => return Err(LiveContinuedEffectDriverFailureV8::Session { owner, error }),
    };
    session
        .append_owned_continued_effect(owner)
        .map_err(LiveContinuedEffectDriverFailureV8::Append)?
        .advance_continued_effect()
        .map_err(LiveContinuedEffectDriverFailureV8::Advance)
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_live_owned_continued_effect_v8<
    'j,
>(
    journal: &'j SourceOwnedWaitJournalV8,
    authorization: LiveContinuedAuthorizationV8<'j>,
) -> Result<LiveContinuedEffectV8<'j>, LiveContinuedEffectDriverFailureV8<'j>> {
    let first = authorization
        .prepare_effect()
        .map_err(LiveContinuedEffectDriverFailureV8::Prepare)?;
    let second = ack_continued_effect_v8(journal, first)?
        .prepare_next()
        .map_err(LiveContinuedEffectDriverFailureV8::Next)?;
    ack_continued_effect_v8(journal, second)
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveContinuedIntentDriverFailureV8<
    'j,
> {
    Prepare(crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveContinuedEffectPreparationFailureV8<'j>),
    Intent(crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveContinuedIntentSelectionFailureV8<'j>),
    Session { owner: crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveOwnedContinuedIntentAppendV8<'j>, error: SourceJournalError },
    Append(crate::live_invocation::source_journal::owned_wait_v8::append::LiveOwnedContinuedIntentAppendFailureV8<'j>),
    Advance(crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveContinuedIntentAcknowledgmentFailureV8<'j>),
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_live_owned_continued_intent_v8<'j>(journal: &'j SourceOwnedWaitJournalV8, effect: LiveContinuedEffectV8<'j>) -> Result<crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveActivatedContinuedEffectV8<'j>, LiveContinuedIntentDriverFailureV8<'j>>{
    let intent = effect
        .prepare_actual_effect()
        .map_err(LiveContinuedIntentDriverFailureV8::Prepare)?
        .prepare_intent()
        .map_err(LiveContinuedIntentDriverFailureV8::Intent)?;
    let session = match journal.begin_session() {
        Ok(x) => x,
        Err(error) => {
            return Err(LiveContinuedIntentDriverFailureV8::Session {
                owner: intent,
                error,
            })
        }
    };
    session
        .append_owned_continued_intent(intent)
        .map_err(LiveContinuedIntentDriverFailureV8::Append)?
        .advance_intent()
        .map_err(LiveContinuedIntentDriverFailureV8::Advance)
}

pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveContinuedEffectDispatchDriverFailureV8<
    'j,
> {
    Dispatch(crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveContinuedDispatchFailureV8<'j>),
    Settlement(crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveContinuedSettlementAcknowledgmentFailureV8<'j>),
    SettlementSession {
        owner: crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveOwnedContinuedSettlementAppendV8<'j>,
        error: SourceJournalError,
    },
    SettlementAppend(crate::live_invocation::source_journal::owned_wait_v8::append::LiveOwnedContinuedSettlementAppendFailureV8<'j>),
    SettlementAdvance(crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveContinuedSettlementAcknowledgmentFailureV8<'j>),
    SettlementShape(crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveContinuedSettlementAcknowledgedV8<'j>),
    Recorded(crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveContinuedSettlementAcknowledgmentFailureV8<'j>),
    RecordedSession {
        owner: crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveOwnedContinuedSettlementAppendV8<'j>,
        error: SourceJournalError,
    },
    RecordedAppend(crate::live_invocation::source_journal::owned_wait_v8::append::LiveOwnedContinuedSettlementAppendFailureV8<'j>),
    RecordedAdvance(crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveContinuedSettlementAcknowledgmentFailureV8<'j>),
    RecordedShape(crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveContinuedSettlementAcknowledgedV8<'j>),
}

/// Dispatches the activated continuation once, then durably acknowledges its
/// ordinary settlement and settlement-record row. Receipt bytes never grant
/// this authority: the explicit host handler and unique live owner do.
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_live_owned_continued_effect_dispatch_v8<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    activated: crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveActivatedContinuedEffectV8<'j>,
    handler: &mut dyn crate::agent_lifecycle::authorization::target_protocol::TargetHostHandler,
) -> Result<crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveRecordedContinuedEffectV8<'j>, LiveContinuedEffectDispatchDriverFailureV8<'j>>{
    let dispatched = activated
        .dispatch(handler)
        .map_err(LiveContinuedEffectDispatchDriverFailureV8::Dispatch)?;
    let settlement = dispatched
        .prepare_settlement()
        .map_err(LiveContinuedEffectDispatchDriverFailureV8::Settlement)?;
    let session = match journal.begin_session() {
        Ok(session) => session,
        Err(error) => {
            return Err(
                LiveContinuedEffectDispatchDriverFailureV8::SettlementSession {
                    owner: settlement,
                    error,
                },
            )
        }
    };
    let settled = match session
        .append_owned_continued_settlement(settlement)
        .map_err(LiveContinuedEffectDispatchDriverFailureV8::SettlementAppend)?
        .advance_settlement()
        .map_err(LiveContinuedEffectDispatchDriverFailureV8::SettlementAdvance)?
    {
        crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveContinuedSettlementAcknowledgedV8::Settled(owner) => owner,
        owner => return Err(LiveContinuedEffectDispatchDriverFailureV8::SettlementShape(owner)),
    };
    let recorded = settled
        .prepare_recorded()
        .map_err(LiveContinuedEffectDispatchDriverFailureV8::Recorded)?;
    let session = match journal.begin_session() {
        Ok(session) => session,
        Err(error) => {
            return Err(
                LiveContinuedEffectDispatchDriverFailureV8::RecordedSession {
                    owner: recorded,
                    error,
                },
            )
        }
    };
    match session
        .append_owned_continued_settlement(recorded)
        .map_err(LiveContinuedEffectDispatchDriverFailureV8::RecordedAppend)?
        .advance_settlement()
        .map_err(LiveContinuedEffectDispatchDriverFailureV8::RecordedAdvance)?
    {
        crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveContinuedSettlementAcknowledgedV8::Recorded(owner) => Ok(owner),
        owner => Err(LiveContinuedEffectDispatchDriverFailureV8::RecordedShape(owner)),
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveContinuedCleanupDriverFailureV8<
    'j,
> {
    Prepare(crate::live_invocation::source_journal::owned_wait_v8::live_upstream::ContinuedDecisionCleanupRejectionV8<'j>),
    StartedSession {
        owner: crate::live_invocation::source_journal::owned_wait_v8::live_upstream::PreparedContinuedDecisionCleanupV8<'j>,
        error: SourceJournalError,
    },
    StartedAppend(crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::LiveOwnedEffectCleanupAppendFailureV8<'j>),
    StartedAdvance(crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveEffectCleanupFailureV8<'j>),
    StartedShape(crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveCleanupAcknowledgedV8<'j>),
    Release(crate::live_invocation::source_journal::owned_wait_v8::live_upstream::ContinuedDecisionReleaseFailureV8<'j>),
    SettledPrepare(crate::live_invocation::source_journal::owned_wait_v8::live_upstream::ReleasedContinuedDecisionCleanupV8<'j>),
    SettledSession {
        owner: crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveOwnedEffectCleanupAppendV8<'j>,
        error: SourceJournalError,
    },
    SettledAppend(crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::LiveOwnedEffectCleanupAppendFailureV8<'j>),
    SettledAdvance(crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveEffectCleanupFailureV8<'j>),
    SettledShape(crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveCleanupAcknowledgedV8<'j>),
}

/// Records the cleanup boundary before releasing physical finalizers, then
/// records their sticky receipt. A post-release failure retains the released
/// owner and deliberately has no route back to the cleanup callback.
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_live_owned_continued_cleanup_v8<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    recorded: crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveRecordedContinuedEffectV8<'j>,
    observe: impl FnMut(&crate::cleanup_plan::FinalizeAction),
) -> Result<crate::live_invocation::source_journal::owned_wait_v8::live_upstream::SettledContinuedDecisionCleanupV8<'j>, LiveContinuedCleanupDriverFailureV8<'j>>{
    let prepared = recorded
        .prepare_decision_cleanup()
        .map_err(LiveContinuedCleanupDriverFailureV8::Prepare)?;
    let session = match journal.begin_session() {
        Ok(session) => session,
        Err(error) => {
            return Err(LiveContinuedCleanupDriverFailureV8::StartedSession {
                owner: prepared,
                error,
            })
        }
    };
    let started = match session
        .append_owned_effect_cleanup(prepared.into_append())
        .map_err(LiveContinuedCleanupDriverFailureV8::StartedAppend)?
        .advance_cleanup()
        .map_err(LiveContinuedCleanupDriverFailureV8::StartedAdvance)?
    {
        crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveCleanupAcknowledgedV8::ContinuedStarted(owner) => *owner,
        owner => return Err(LiveContinuedCleanupDriverFailureV8::StartedShape(owner)),
    };
    let released = started
        .release_decision(observe)
        .map_err(LiveContinuedCleanupDriverFailureV8::Release)?;
    let settled = released
        .prepare_settled()
        .map_err(LiveContinuedCleanupDriverFailureV8::SettledPrepare)?;
    let session = match journal.begin_session() {
        Ok(session) => session,
        Err(error) => {
            return Err(LiveContinuedCleanupDriverFailureV8::SettledSession {
                owner: settled,
                error,
            })
        }
    };
    match session
        .append_owned_effect_cleanup(settled)
        .map_err(LiveContinuedCleanupDriverFailureV8::SettledAppend)?
        .advance_cleanup()
        .map_err(LiveContinuedCleanupDriverFailureV8::SettledAdvance)?
    {
        crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveCleanupAcknowledgedV8::ContinuedSettled(owner) => Ok(*owner),
        owner => Err(LiveContinuedCleanupDriverFailureV8::SettledShape(owner)),
    }
}

pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveContinuedReduceDriverFailureV8<
    'j,
> {
    Outcome(crate::live_invocation::source_journal::owned_wait_v8::live_upstream::ContinuedOutcomeFailureV8<'j>),
    Prepare(crate::live_invocation::source_journal::owned_wait_v8::live_upstream::ContinuedReduceReservationRejectionV8<'j>),
    Session {
        owner: crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveContinuedReduceReservationAppendV8<'j>,
        error: SourceJournalError,
    },
    Append(crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::LiveContinuedReduceAppendFailureV8<'j>),
    Advance(crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveContinuedReduceAdvanceFailureV8<'j>),
}

/// The continued reservation consumes exactly the real cleanup-settled owner.
/// Its output remains private at SpentReduce; evaluation and Step are separate.
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_live_owned_continued_reduce_v8<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    cleanup: crate::live_invocation::source_journal::owned_wait_v8::live_upstream::SettledContinuedDecisionCleanupV8<'j>,
) -> Result<crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveContinuedReduceReservedV8<'j>, LiveContinuedReduceDriverFailureV8<'j>>{
    let cleanup = cleanup
        .mint_outcome()
        .map_err(LiveContinuedReduceDriverFailureV8::Outcome)?;
    let reservation = cleanup
        .prepare_continued_reduce()
        .map_err(LiveContinuedReduceDriverFailureV8::Prepare)?;
    let session = match journal.begin_session() {
        Ok(session) => session,
        Err(error) => {
            return Err(LiveContinuedReduceDriverFailureV8::Session {
                owner: reservation,
                error,
            })
        }
    };
    session
        .append_owned_continued_reduce_reservation(reservation)
        .map_err(LiveContinuedReduceDriverFailureV8::Append)?
        .advance_reduce()
        .map_err(LiveContinuedReduceDriverFailureV8::Advance)
}

impl<'j> AppendSessionV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn append_owned_continue(
        self,
        obligation: LiveOwnedContinueAppendV8<'j>,
    ) -> Result<VerifiedOwnedContinueAppendV8<'j>, LiveOwnedContinueAppendFailureV8<'j>> {
        let same_journal = obligation.belongs_to(self.journal);
        let predecessor = match (|| {
            if !same_journal
                || obligation.sequence() != self.sequence()
                || obligation.acknowledged_bytes() != self.acknowledged_bytes()
                || !matches!(obligation.selected_row(),
                    EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedStateCommitted{..})
                    |EntryV8::Ordinary(SourceJournalEntry::StageReservation{role:crate::live_invocation::source_journal::SourceStageRole::Observe,attempt:None,..}))
            {
                return Err(SourceJournalError::Binding);
            }
            obligation.validate_live()?;
            self.effect_cursor()
        })() {
            Ok(cursor) => cursor,
            Err(error) => {
                // Harmless wrong-container preflight does not poison that other
                // container. Stale actual same-container ownership retires it.
                if same_journal {
                    self.journal.quarantine();
                }
                return Err(LiveOwnedContinueAppendFailureV8::Before {
                    _obligation: obligation,
                    _session: self,
                    error,
                });
            }
        };
        // This permit is borrowed only from this actual owner-containing object.
        let permit = match obligation.fixed_append_permit() {
            Ok(permit) => permit,
            Err(error) => {
                self.journal.quarantine();
                return Err(LiveOwnedContinueAppendFailureV8::Before {
                    _obligation: obligation,
                    _session: self,
                    error,
                });
            }
        };
        let selected = permit.selected_row().clone();
        let (pending, verified, attempting) = match self.begin_fixed_continue_append(&permit) {
            Ok(completion) => completion,
            Err(failure) => {
                return Err(LiveOwnedContinueAppendFailureV8::Append {
                    _obligation: obligation,
                    _failure: failure,
                })
            }
        };
        let session = AppendSessionV8 {
            journal: attempting.journal,
            inventory: pending.acknowledge_verified(verified),
        };
        // Sole literal constructor: actual same-FD write/sync/reread has ACKed
        // this Pending. Recovered inventory never reaches this construction.
        let witness = VerifiedOwnedContinueSuccessorV8 {
            predecessor,
            successor: OwnedEffectAppendCursorV8::capture(&session),
            selected,
        };
        let advanced = witness
            .validate_against_acknowledged_session(&session)
            .and_then(|_| permit.advance_registry(&witness, &session));
        if let Err(error) = advanced {
            // Attempting remains incomplete and poisons before any return.
            drop(attempting);
            return Err(LiveOwnedContinueAppendFailureV8::Acknowledged {
                _obligation: obligation,
                _session: session,
                _witness: witness,
                error,
            });
        }
        attempting.complete.set(true);
        drop(attempting); // continuation postguard requires current cancellation/clock
        let envelope = VerifiedOwnedContinueAppendV8 {
            obligation,
            session,
            witness,
        };
        if let Err(error) = envelope.validate_live() {
            return Err(LiveOwnedContinueAppendFailureV8::After {
                _verified: envelope,
                error,
            });
        }
        Ok(envelope)
    }
}

use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::step::r#continue::later::{
    advance_verified_later_continue_v8, LiveLaterContinueAcknowledgedV8,
    LiveLaterContinueAppendV8, LiveLaterContinueFailureV8, LiveLaterObservedContinueV8,
};

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct VerifiedOwnedLaterContinueAppendV8<
    'j,
> {
    obligation: LiveLaterContinueAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedContinueSuccessorV8<'j>,
}
impl<'j> VerifiedOwnedLaterContinueAppendV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_continue(
        self,
    ) -> Result<LiveLaterContinueAcknowledgedV8<'j>, LiveLaterContinueFailureV8<'j>> {
        advance_verified_later_continue_v8(self.obligation, self.session, self.witness)
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveOwnedLaterContinueAppendFailureV8<
    'j,
> {
    Before {
        owner: LiveLaterContinueAppendV8<'j>,
        session: AppendSessionV8<'j>,
        error: SourceJournalError,
    },
    Append {
        owner: LiveLaterContinueAppendV8<'j>,
        failure: AppendFailureV8<'j>,
    },
    Acknowledged {
        owner: LiveLaterContinueAppendV8<'j>,
        session: AppendSessionV8<'j>,
        witness: VerifiedOwnedContinueSuccessorV8<'j>,
        error: SourceJournalError,
    },
    After {
        verified: VerifiedOwnedLaterContinueAppendV8<'j>,
        error: SourceJournalError,
    },
}
impl<'j> AppendSessionV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn append_owned_later_continue(
        self,
        obligation: LiveLaterContinueAppendV8<'j>,
    ) -> Result<VerifiedOwnedLaterContinueAppendV8<'j>, LiveOwnedLaterContinueAppendFailureV8<'j>>
    {
        let same_journal = obligation.belongs_to(self.journal);
        let predecessor = match (|| {
            if !same_journal || obligation.sequence() != self.sequence()
                || obligation.acknowledged_bytes() != self.acknowledged_bytes()
                || !matches!(obligation.selected_row(),
                    EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedStateCommitted{..})
                    | EntryV8::Ordinary(SourceJournalEntry::StageReservation{role:crate::live_invocation::source_journal::SourceStageRole::Observe,attempt:None,..})) {
                return Err(SourceJournalError::Binding);
            }
            obligation.validate_live()?;
            self.effect_cursor()
        })() {
            Ok(cursor) => cursor,
            Err(error) => {
                if same_journal {
                    self.journal.quarantine()
                }
                return Err(LiveOwnedLaterContinueAppendFailureV8::Before {
                    owner: obligation,
                    session: self,
                    error,
                });
            }
        };
        let permit = match obligation.fixed_append_permit() {
            Ok(permit) => permit,
            Err(error) => {
                self.journal.quarantine();
                return Err(LiveOwnedLaterContinueAppendFailureV8::Before {
                    owner: obligation,
                    session: self,
                    error,
                });
            }
        };
        let selected = permit.selected_row().clone();
        let (pending, verified, attempting) = match self.begin_fixed_continue_append(&permit) {
            Ok(completion) => completion,
            Err(failure) => {
                return Err(LiveOwnedLaterContinueAppendFailureV8::Append {
                    owner: obligation,
                    failure,
                })
            }
        };
        let session = AppendSessionV8 {
            journal: attempting.journal,
            inventory: pending.acknowledge_verified(verified),
        };
        let witness = VerifiedOwnedContinueSuccessorV8 {
            predecessor,
            successor: OwnedEffectAppendCursorV8::capture(&session),
            selected,
        };
        let advanced = witness
            .validate_against_acknowledged_session(&session)
            .and_then(|_| permit.advance_registry(&witness, &session));
        if let Err(error) = advanced {
            drop(attempting);
            return Err(LiveOwnedLaterContinueAppendFailureV8::Acknowledged {
                owner: obligation,
                session,
                witness,
                error,
            });
        }
        attempting.complete.set(true);
        drop(attempting);
        let envelope = VerifiedOwnedLaterContinueAppendV8 {
            obligation,
            session,
            witness,
        };
        if let Err(error) = envelope
            .obligation
            .validate_successor(&envelope.witness, &envelope.session)
        {
            return Err(LiveOwnedLaterContinueAppendFailureV8::After {
                verified: envelope,
                error,
            });
        }
        Ok(envelope)
    }
}

pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveLaterContinueDriverFailureV8<
    'j,
> {
    Prepare(LiveLaterContinueFailureV8<'j>),
    StateSession {
        owner: LiveLaterContinueAppendV8<'j>,
        error: SourceJournalError,
    },
    StateAppend(LiveOwnedLaterContinueAppendFailureV8<'j>),
    StateAdvance(LiveLaterContinueFailureV8<'j>),
    StateAcknowledged(LiveLaterContinueAcknowledgedV8<'j>),
    ObservePrepare(LiveLaterContinueFailureV8<'j>),
    ObserveSession {
        owner: LiveLaterContinueAppendV8<'j>,
        error: SourceJournalError,
    },
    ObserveAppend(LiveOwnedLaterContinueAppendFailureV8<'j>),
    ObserveAdvance(LiveLaterContinueFailureV8<'j>),
    ObserveAcknowledged(LiveLaterContinueAcknowledgedV8<'j>),
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_live_owned_later_continue_v8<
    'j,
>(
    journal: &'j SourceOwnedWaitJournalV8,
    moved: crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveContinuedStagedStepV8<'j>,
) -> Result<LiveLaterObservedContinueV8<'j>, LiveLaterContinueDriverFailureV8<'j>> {
    let state_append = moved
        .prepare_later_continue()
        .map_err(LiveLaterContinueDriverFailureV8::Prepare)?;
    let state_session = match journal.begin_session() {
        Ok(session) => session,
        Err(error) => {
            return Err(LiveLaterContinueDriverFailureV8::StateSession {
                owner: state_append,
                error,
            })
        }
    };
    let state_ack = state_session
        .append_owned_later_continue(state_append)
        .map_err(LiveLaterContinueDriverFailureV8::StateAppend)?;
    let state = match state_ack
        .advance_continue()
        .map_err(LiveLaterContinueDriverFailureV8::StateAdvance)?
    {
        LiveLaterContinueAcknowledgedV8::State(state) => state,
        acknowledged => {
            return Err(LiveLaterContinueDriverFailureV8::StateAcknowledged(
                acknowledged,
            ))
        }
    };
    let observe_append = state
        .prepare_observe()
        .map_err(LiveLaterContinueDriverFailureV8::ObservePrepare)?;
    let observe_session = match journal.begin_session() {
        Ok(session) => session,
        Err(error) => {
            return Err(LiveLaterContinueDriverFailureV8::ObserveSession {
                owner: observe_append,
                error,
            })
        }
    };
    let observe_ack = observe_session
        .append_owned_later_continue(observe_append)
        .map_err(LiveLaterContinueDriverFailureV8::ObserveAppend)?;
    match observe_ack
        .advance_continue()
        .map_err(LiveLaterContinueDriverFailureV8::ObserveAdvance)?
    {
        LiveLaterContinueAcknowledgedV8::Observed(observed) => Ok(observed),
        acknowledged => Err(LiveLaterContinueDriverFailureV8::ObserveAcknowledged(
            acknowledged,
        )),
    }
}

#[cfg(all(test, unix))]
mod tests;
