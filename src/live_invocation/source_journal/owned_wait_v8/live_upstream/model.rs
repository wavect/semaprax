//! Actual model Intent permit. Constructor remains private to this live actor.
use super::*;
use crate::provider_adapter_sdk::CheckedOwnedModelRequestV8;
pub(crate) struct LiveModelIntentPermitV8<'j> {
    held: HeldOwnedWaitStoreV8<'j>,
    request: CheckedOwnedModelRequestV8,
    clock: &'j dyn crate::live_invocation::SourceInvocationClock,
    sequence: usize,
    bytes: usize,
    cancellation: &'j crate::agent_runtime::AgentCancellation,
}
impl LiveModelIntentPermitV8<'_> {
    pub(crate) fn validate_guard(&self) -> Result<(), SourceJournalError> {
        if self.cancellation.is_cancelled() {
            return Err(SourceJournalError::Binding);
        }
        self.validate_store()
    }
    pub(crate) fn validate_store(&self) -> Result<(), SourceJournalError> {
        self.held.validate_prefix(self.sequence, self.bytes)
    }
    pub(crate) fn guard_failure(&self) -> super::super::super::SourceAttemptFailure {
        if self.cancellation.is_cancelled() {
            super::super::super::SourceAttemptFailure::Cancelled
        } else {
            super::super::super::SourceAttemptFailure::Refused
        }
    }
    pub(crate) fn quarantine(&self) {
        self.held.quarantine();
    }
    pub(crate) fn clock(&self) -> &dyn crate::live_invocation::SourceInvocationClock {
        self.clock
    }
    pub(crate) fn request(&self) -> &CheckedOwnedModelRequestV8 {
        &self.request
    }
}

/// Original Resume reservation, minted only by this actual actor after Usage ACK.
pub(crate) struct LiveWaitResumePermitV8<'j> {
    held: HeldOwnedWaitStoreV8<'j>,
    fuel: usize,
    sequence: usize,
    bytes: usize,
    cancellation: &'j crate::agent_runtime::AgentCancellation,
    clock: &'j dyn crate::live_invocation::SourceInvocationClock,
    deadline: i64,
    initial: i64,
    domain: &'j str,
}
impl LiveWaitResumePermitV8<'_> {
    pub(crate) fn validate_guard(&self) -> Result<(), SourceJournalError> {
        check_clock_v8(
            &self.held,
            self.sequence,
            self.bytes,
            self.cancellation,
            self.clock,
            self.domain,
            self.initial,
            self.deadline,
        )
    }
    pub(crate) fn fuel(&self) -> usize {
        self.fuel
    }
}
pub(super) fn check_clock_v8(
    held: &HeldOwnedWaitStoreV8<'_>,
    sequence: usize,
    bytes: usize,
    cancellation: &crate::agent_runtime::AgentCancellation,
    clock: &dyn crate::live_invocation::SourceInvocationClock,
    domain: &str,
    initial: i64,
    deadline: i64,
) -> Result<(), SourceJournalError> {
    let guard = || {
        held.validate_prefix(sequence, bytes)?;
        if cancellation.is_cancelled() {
            return Err(SourceJournalError::Binding);
        }
        Ok(())
    };
    guard()?;
    let read_domain =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| clock.clock_domain()));
    let read_domain = match read_domain {
        Ok(domain) => domain,
        Err(_) => {
            held.quarantine();
            return Err(SourceJournalError::Poisoned);
        }
    };
    guard()?;
    if read_domain != domain {
        return Err(SourceJournalError::Binding);
    }
    let now = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| clock.now_millis()));
    let now = match now {
        Ok(now) => now,
        Err(_) => {
            held.quarantine();
            return Err(SourceJournalError::Poisoned);
        }
    };
    guard()?;
    if now < initial || now >= deadline {
        return Err(SourceJournalError::Time);
    }
    Ok(())
}
use super::wait::ParkedLiveOwnedRunV8;
use crate::interpreter::resumable::owned_frame::registered_stage::live_run::{
    resume_live_owned_wait_v8, LiveResumedStateV8, LiveWaitResumeOutcomeV8,
};
use crate::live_invocation::source_journal::SourceReportedUsage;
use crate::live_invocation::SourceInvocationClock;
use crate::provider_adapter_sdk::{OwnedModelSettlementV8, StreamingSourceProposalAdapter};
use crate::resumable_effects::owned_frame::v2::{
    bind_owned_wait_proposal_v8, CheckedOwnedWaitProposalV8,
};
pub(super) enum LiveModelFailureOwnerV8 {
    Parked(
        crate::interpreter::resumable::owned_frame::registered_stage::live_run::LiveParkedStateV8,
    ),
    Resume(LiveWaitResumeOutcomeV8),
}
pub(super) struct LiveModelFailureV8<'j> {
    owner: LiveModelFailureOwnerV8,
    held: HeldOwnedWaitStoreV8<'j>,
    error: SourceJournalError,
    reason: Option<super::super::super::SourceAttemptFailure>,
    diagnostics: Vec<crate::diagnostic::Diagnostic>,
    usage: Option<SourceReportedUsage>,
}
/// Same physical State remains staged. Transfer/Authorize have not occurred.
pub(super) struct CompletedLiveOwnedRunV8<'j> {
    pub(super) owner: LiveResumedStateV8,
    pub(super) session: AppendSessionV8<'j>,
    pub(super) held: HeldOwnedWaitStoreV8<'j>,
    pub(super) journal: &'j SourceOwnedWaitJournalV8,
    pub(super) proposal: CheckedOwnedWaitProposalV8,
    pub(super) completed: u32,
    pub(super) cancellation: &'j crate::agent_runtime::AgentCancellation,
    pub(super) wait: String,
    pub(super) clock: &'j dyn SourceInvocationClock,
}
fn reported_usage(value: Option<(u64, u64, i64)>) -> Option<SourceReportedUsage> {
    value.map(|(input, output, _)| SourceReportedUsage {
        total: input.checked_add(output),
        input: Some(input),
        output: Some(output),
        reasoning: None,
        cache_read: None,
        cache_write: None,
    })
}
pub(super) fn model_live_actor_v8<'j>(
    parked: ParkedLiveOwnedRunV8<'j>,
    adapter: &mut StreamingSourceProposalAdapter<'_>,
    clock: &'j dyn SourceInvocationClock,
) -> Result<CompletedLiveOwnedRunV8<'j>, LiveModelFailureV8<'j>> {
    let ParkedLiveOwnedRunV8 {
        owner,
        session,
        held,
        journal,
        observation,
        wait,
        cancellation,
        ..
    } = parked;
    macro_rules! fail {
        ($owner:expr,$error:expr) => {
            LiveModelFailureV8 {
                owner: $owner,
                held,
                error: $error,
                reason: None,
                diagnostics: Vec::new(),
                usage: None,
            }
        };
    }
    macro_rules! guard {
        ($current:expr) => {
            if cancellation.is_cancelled() {
                return Err(fail!(
                    LiveModelFailureOwnerV8::Parked(owner),
                    SourceJournalError::Binding
                ));
            }
            if let Err(error) =
                held.validate_prefix($current.sequence(), $current.acknowledged_bytes())
            {
                return Err(fail!(LiveModelFailureOwnerV8::Parked(owner), error));
            }
        };
    }
    guard!(session);
    let context = journal.context();
    let Some((runtime, execution)) = context.ready_runtime() else {
        return Err(fail!(
            LiveModelFailureOwnerV8::Parked(owner),
            SourceJournalError::Binding
        ));
    };
    let scope = &held.registration().expected_facts().scope;
    if let Err(error) = check_clock_v8(
        &held,
        session.sequence(),
        session.acknowledged_bytes(),
        cancellation,
        clock,
        context.ordinary().clock_domain(),
        context.ordinary().initial_millis(),
        context.ordinary().deadline_millis(),
    ) {
        return Err(fail!(LiveModelFailureOwnerV8::Parked(owner), error));
    }
    let request = match adapter.checked_owned_model_request_v8(
        runtime,
        execution,
        scope,
        &owner,
        &observation,
    ) {
        Ok(request) => request,
        Err(diagnostics) => {
            let mut failed = fail!(
                LiveModelFailureOwnerV8::Parked(owner),
                SourceJournalError::Binding
            );
            failed.diagnostics = diagnostics;
            return Err(failed);
        }
    };
    guard!(session);
    let identity = request.identity();
    let intent = match context.ordinary().attempt_intent_at_ordinal(
        0,
        0,
        identity.request_digest.clone(),
        identity.prompt_digest.clone(),
        identity.request_bytes,
        0,
    ) {
        Ok(row) => row,
        Err(error) => return Err(fail!(LiveModelFailureOwnerV8::Parked(owner), error)),
    };
    let session = match session.append(EntryV8::Ordinary(intent)) {
        Ok(session) => session,
        Err(_) => {
            return Err(fail!(
                LiveModelFailureOwnerV8::Parked(owner),
                SourceJournalError::Uncertain
            ))
        }
    };
    let permit_hold = match journal.hold() {
        Ok(held) => held,
        Err(error) => return Err(fail!(LiveModelFailureOwnerV8::Parked(owner), error)),
    };
    let permit = LiveModelIntentPermitV8 {
        held: permit_hold,
        request,
        clock,
        sequence: session.sequence(),
        bytes: session.acknowledged_bytes(),
        cancellation,
    };
    adapter.configure_durable_boundary(cancellation, context.ordinary().deadline_millis());
    let dispatched = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        adapter.dispatch_owned_wait_v8(permit)
    }));
    let dispatched = match dispatched {
        Ok(result) => result,
        Err(_) => {
            held.quarantine();
            return Err(fail!(
                LiveModelFailureOwnerV8::Parked(owner),
                SourceJournalError::Poisoned
            ));
        }
    };
    let (permit, result) = dispatched.into_parts();
    // Cancellation may record the actual failure/usage, but may never Resume.
    if let Err(error) = permit.validate_store() {
        let mut failed = fail!(LiveModelFailureOwnerV8::Parked(owner), error);
        if let OwnedModelSettlementV8::Failed {
            reason,
            diagnostics,
            ..
        } = &result
        {
            failed.reason = Some(*reason);
            failed.diagnostics = diagnostics.clone();
        }
        return Err(failed);
    }
    let (settlement, usage, decoded, reason, diagnostics) = match result {
        OwnedModelSettlementV8::Settled {
            decoded,
            response,
            usage,
        } => (
            SourceJournalEntry::AttemptSettled {
                turn: 0,
                attempt: 0,
                response_digest: super::super::super::source_response_digest(&response),
                response,
            },
            usage,
            Some(decoded),
            None,
            Vec::new(),
        ),
        OwnedModelSettlementV8::Failed {
            diagnostics,
            reason,
            attempted_bytes,
            usage,
        } => (
            SourceJournalEntry::AttemptFailed {
                turn: 0,
                attempt: 0,
                reason,
                attempted_bytes,
            },
            usage,
            None,
            Some(reason),
            diagnostics,
        ),
    };
    drop(permit); // Retire the Intent phase before appending its settlement.
    let session = match session.append(EntryV8::Ordinary(settlement)) {
        Ok(session) => session,
        Err(_) => {
            return Err(fail!(
                LiveModelFailureOwnerV8::Parked(owner),
                SourceJournalError::Uncertain
            ))
        }
    };
    let reported = reported_usage(usage);
    let session = match session.append(EntryV8::Ordinary(SourceJournalEntry::AttemptUsage {
        turn: 0,
        attempt: 0,
        reported: reported.clone(),
    })) {
        Ok(session) => session,
        Err(_) => {
            return Err(fail!(
                LiveModelFailureOwnerV8::Parked(owner),
                SourceJournalError::Uncertain
            ))
        }
    };
    let Some(decoded) = decoded else {
        let mut failed = fail!(
            LiveModelFailureOwnerV8::Parked(owner),
            SourceJournalError::Binding
        );
        failed.reason = reason;
        failed.diagnostics = diagnostics;
        failed.usage = reported;
        return Err(failed);
    };
    guard!(session);
    if let Err(error) = check_clock_v8(
        &held,
        session.sequence(),
        session.acknowledged_bytes(),
        cancellation,
        clock,
        context.ordinary().clock_domain(),
        context.ordinary().initial_millis(),
        context.ordinary().deadline_millis(),
    ) {
        return Err(fail!(LiveModelFailureOwnerV8::Parked(owner), error));
    }
    let proposal = match bind_owned_wait_proposal_v8(execution.wait(), scope, &decoded) {
        Ok(p) => p,
        Err(_) => {
            return Err(fail!(
                LiveModelFailureOwnerV8::Parked(owner),
                SourceJournalError::Binding
            ))
        }
    };
    let argument = match owner.checked_facts(execution.wait()) {
        Some(facts) => wire::record_argument_digest(&facts),
        None => {
            return Err(fail!(
                LiveModelFailureOwnerV8::Parked(owner),
                SourceJournalError::Binding
            ))
        }
    };
    let result_digest = match proposal.result_digest(&argument) {
        Ok(d) => d,
        Err(_) => {
            return Err(fail!(
                LiveModelFailureOwnerV8::Parked(owner),
                SourceJournalError::Binding
            ))
        }
    };
    let fuel = execution.evaluation_fuel();
    if context.ordinary().max_steps_per_stage() != Some(fuel) {
        return Err(fail!(
            LiveModelFailureOwnerV8::Parked(owner),
            SourceJournalError::Binding
        ));
    }
    let reservation = session.sequence() as u32;
    let session = match session.append(EntryV8::Owned(
        journal_model::OwnedBodyV8::OwnedWaitReserved {
            turn: 0,
            attempt: 0,
            wait: wait.clone(),
            phase: journal_model::PhaseV8::Resume,
            replay_of: None,
            fuel: fuel as u64,
        },
    )) {
        Ok(session) => session,
        Err(_) => {
            return Err(fail!(
                LiveModelFailureOwnerV8::Parked(owner),
                SourceJournalError::Uncertain
            ))
        }
    };
    guard!(session);
    let permit_hold = match journal.hold() {
        Ok(held) => held,
        Err(error) => return Err(fail!(LiveModelFailureOwnerV8::Parked(owner), error)),
    };
    let outcome = resume_live_owned_wait_v8(
        LiveWaitResumePermitV8 {
            held: permit_hold,
            fuel,
            sequence: session.sequence(),
            bytes: session.acknowledged_bytes(),
            cancellation,
            clock,
            deadline: context.ordinary().deadline_millis(),
            initial: context.ordinary().initial_millis(),
            domain: context.ordinary().clock_domain(),
        },
        owner,
        proposal.carrier().clone(),
    );
    let LiveWaitResumeOutcomeV8::Resumed(owner) = outcome else {
        return Err(fail!(
            LiveModelFailureOwnerV8::Resume(outcome),
            SourceJournalError::Binding
        ));
    };
    if owner.checked_facts(execution.wait()).is_none() {
        return Err(fail!(
            LiveModelFailureOwnerV8::Resume(LiveWaitResumeOutcomeV8::GuardLost(owner)),
            SourceJournalError::Binding
        ));
    }
    let completed = session.sequence() as u32;
    let session = match session.append(EntryV8::Owned(
        journal_model::OwnedBodyV8::OwnedWaitCompleted {
            turn: 0,
            attempt: 0,
            wait: wait.clone(),
            reservation,
            proposal: proposal.value().clone(),
            proposal_digest: proposal.ordinary_digest().into(),
            result_digest,
            consumed: owner.consumed(),
        },
    )) {
        Ok(session) => session,
        Err(_) => {
            return Err(fail!(
                LiveModelFailureOwnerV8::Resume(LiveWaitResumeOutcomeV8::GuardLost(owner)),
                SourceJournalError::Uncertain
            ))
        }
    };
    if cancellation.is_cancelled()
        || held
            .validate_prefix(session.sequence(), session.acknowledged_bytes())
            .is_err()
    {
        return Err(fail!(
            LiveModelFailureOwnerV8::Resume(LiveWaitResumeOutcomeV8::GuardLost(owner)),
            SourceJournalError::Binding
        ));
    }
    Ok(CompletedLiveOwnedRunV8 {
        owner,
        session,
        held,
        journal,
        proposal,
        completed,
        cancellation,
        wait,
        clock,
    })
}
#[cfg(all(test, unix))]
pub(super) mod tests;
