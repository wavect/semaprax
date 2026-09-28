//! The sole live owner of a registered invocation and its physical backing.
use super::{
    checkpoint, codec,
    fold::{self, Phase, State},
    journal::{Journal, Kind, Record},
    store::RegisteredJournalLease,
    CheckedOwnedFramePlan, OwnedFrameError as Error,
};
use crate::cleanup_plan::FinalizeAction;
use crate::interpreter::{
    resumable::owned_frame::{
        durable::DurableOwner, snapshot, OwnedFrameArgument, OwnedFrameBudget, OwnedFrameFailure,
        OwnedFrameInput, OwnedFrameResult,
    },
    ArgumentValue,
};
use crate::resumable_effects::{
    source_checkpoint::{SourceCheckpointKey, SourceCheckpointScope},
    CapabilityPolicy,
};
use serde_json::{json, Value};

mod capacity;

pub(super) struct PreparedOwnedFrame {
    plan: CheckedOwnedFramePlan,
    input: OwnedFrameInput,
    scope: SourceCheckpointScope,
    max_steps: u64,
    max_reserved_fuel: u64,
}
impl PreparedOwnedFrame {
    pub(super) fn new(
        plan: &CheckedOwnedFramePlan,
        argument: &OwnedFrameArgument,
        scope: SourceCheckpointScope,
        max_steps: u64,
        max_reserved_fuel: u64,
    ) -> Result<Self, Error> {
        if snapshot::argument_binding(argument) != plan.binding() {
            return Err(Error::Binding);
        }
        let input = snapshot::argument_input(argument).map_err(|_| Error::Binding)?;
        codec::input(plan, &input)?;
        codec::scope(&scope)?;
        OwnedFrameBudget::new(usize::try_from(max_steps).map_err(|_| Error::Fuel)?)
            .map_err(|_| Error::Fuel)?;
        if max_reserved_fuel < max_steps {
            return Err(Error::Fuel);
        }
        Ok(Self {
            plan: plan.clone(),
            input,
            scope,
            max_steps,
            max_reserved_fuel,
        })
    }
}
pub(super) struct OwnedFrameInvocation<'key> {
    journal: Journal<'key>,
    owner: Option<DurableOwner>,
    replay_checked: bool,
}
pub(super) enum OwnedFrameStart<'key> {
    Rejected {
        argument: OwnedFrameArgument,
        error: Error,
    },
    Invocation {
        invocation: OwnedFrameInvocation<'key>,
        acknowledgement: Result<(), Error>,
    },
}
// A scoped host assertion, never an observed release or result permission.
pub(super) struct OwnedFrameCleanupConfirmation {
    facts: Value,
}
impl<'key> OwnedFrameInvocation<'key> {
    pub(super) fn grant_cleanup_confirmation_for_trusted_host(
        &self,
        policy: &CapabilityPolicy,
        scope: &SourceCheckpointScope,
    ) -> Result<OwnedFrameCleanupConfirmation, Error> {
        self.guard(policy, scope)?;
        if self.journal.state.phase != Phase::CleanupStarted
            || self
                .journal
                .state
                .terminal()
                .is_none_or(|row| row.kind != Kind::Failed)
        {
            return Err(Error::Binding);
        }
        Ok(OwnedFrameCleanupConfirmation {
            facts: self.journal.state.confirmation()?,
        })
    }
    pub(super) fn confirm_cleanup(
        &mut self,
        policy: &CapabilityPolicy,
        scope: &SourceCheckpointScope,
        grant: OwnedFrameCleanupConfirmation,
    ) -> Result<(), Error> {
        self.guard(policy, scope)?;
        if self.journal.state.phase != Phase::CleanupStarted
            || grant.facts != self.journal.state.confirmation()?
            || self
                .journal
                .state
                .terminal()
                .is_none_or(|row| row.kind != Kind::Failed)
        {
            return Err(Error::Binding);
        }
        let receipt = json!({"kind":"host_confirmed","confirmation_digest":codec::fact_digest(b"semaprax.source-owned-frame-confirmation.v1\0",&grant.facts)});
        self.append(Record::new(
            Kind::CleanupSettled,
            json!({"cleanup_started_sequence":self.journal.state.phase_sequence,"receipt":receipt}),
        )?)
    }
    pub(super) fn recover(
        lease: RegisteredJournalLease,
        key: &'key SourceCheckpointKey,
        plan: &CheckedOwnedFramePlan,
        scope: SourceCheckpointScope,
        max_steps: u64,
        max_reserved_fuel: u64,
        policy: &CapabilityPolicy,
    ) -> Result<(Self, Result<(), Error>), Error> {
        lease.validate_current()?;
        if !policy.allows(plan.function().id.as_str()) {
            return Err(Error::Policy);
        }
        let state = State::new(
            plan.clone(),
            scope.clone(),
            max_steps,
            max_reserved_fuel,
            lease.identity(),
        )?;
        let mut journal = Journal::reopen(lease, key, state)?;
        let owner = match journal.state.phase {
            Phase::Committed
            | Phase::Starting
            | Phase::Yielded
            | Phase::Dispatched
            | Phase::Answered
            | Phase::Resuming
            | Phase::Completed
            | Phase::Failed => Some(
                DurableOwner::restore(plan, journal.restore_permit()?)
                    .map_err(|_| Error::Binding)?,
            ),
            // Created alone, in-doubt cleanup and claimed delivery never mint a
            // second owner. Recovery still returns their authenticated evidence.
            _ => None,
        };
        let mut invocation = Self {
            journal,
            owner,
            replay_checked: false,
        };
        let mut validation = Ok(());
        if matches!(
            invocation.journal.state.phase,
            Phase::Committed
                | Phase::Starting
                | Phase::Yielded
                | Phase::Dispatched
                | Phase::Answered
                | Phase::Resuming
        ) {
            validation = invocation.validate_replay(policy, &scope);
        }
        invocation.replay_checked = validation.is_ok();
        Ok((invocation, validation))
    }
    fn validate_replay(
        &mut self,
        policy: &CapabilityPolicy,
        scope: &SourceCheckpointScope,
    ) -> Result<(), Error> {
        self.guard(policy, scope)?;
        let phase = self.journal.state.phase;
        let basis = self.journal.state.basis()?;
        let mut budget = self.reserve(Kind::ReplayReserved)?;
        self.guard(policy, scope)?;
        let facts = self
            .owner
            .as_ref()
            .ok_or(Error::Binding)?
            .replay_start(&mut budget);
        let mut exhausted = matches!(facts.failure(), Some(OwnedFrameFailure::FuelExhausted));
        if matches!(
            phase,
            Phase::Yielded | Phase::Dispatched | Phase::Answered | Phase::Resuming
        ) {
            let (_, row) = self.journal.state.yielded().ok_or(Error::Binding)?;
            let original = codec::parse(
                codec::text(&row.fields["checkpoint"], codec::MAX_CHECKPOINT)?.as_bytes(),
                codec::MAX_CHECKPOINT,
            )?;
            if facts.failure().is_some()
                || facts.request().map(codec::scalar).transpose()?.as_ref()
                    != Some(&original["request"])
            {
                self.journal.poisoned = true;
                return Err(Error::Binding);
            }
        }
        if matches!(phase, Phase::Answered | Phase::Resuming) {
            let answer = codec::decode_scalar(
                &self
                    .journal
                    .state
                    .answered()
                    .ok_or(Error::Binding)?
                    .1
                    .fields["answer"],
            )?;
            let resumed = self.owner.as_ref().ok_or(Error::Binding)?.replay_resume(
                &facts,
                &answer,
                &mut budget,
            );
            exhausted |= matches!(resumed.failure(), Some(OwnedFrameFailure::FuelExhausted));
            // Historical resume is borrowed validation, never publication or a
            // second cleanup owner. Exhaustion is ACK-recorded below before a
            // retry/abandon decision; the original answer remains authoritative.
            if resumed.request().is_some() {
                self.journal.poisoned = true;
                return Err(Error::Binding);
            }
        }
        self.append(Record::new(Kind::ReplayValidated,json!({"reservation_sequence":self.journal.state.records.len() as u64-1,"basis":basis,"consumed_steps":budget.consumed()}))?)?;
        self.guard(policy, scope)?;
        if matches!(
            phase,
            Phase::Yielded | Phase::Dispatched | Phase::Answered | Phase::Resuming
        ) {
            let owner = self.owner.take().ok_or(Error::Binding)?;
            self.owner = Some(match owner.install_replayed_park(facts) {
                Ok(owner) => owner,
                Err(owner) => {
                    self.owner = Some(owner);
                    self.journal.poisoned = true;
                    return Err(Error::Binding);
                }
            });
        }
        if exhausted {
            return Err(Error::Fuel);
        }
        Ok(())
    }
    pub(super) fn start(
        prepared: PreparedOwnedFrame,
        argument: OwnedFrameArgument,
        lease: RegisteredJournalLease,
        key: &'key SourceCheckpointKey,
        policy: &CapabilityPolicy,
    ) -> OwnedFrameStart<'key> {
        let preflight = (|| {
            lease.validate_current()?;
            if !policy.allows(prepared.plan.function().id.as_str()) {
                return Err(Error::Policy);
            }
            let actual = snapshot::argument_input(&argument).map_err(|_| Error::Binding)?;
            if snapshot::argument_binding(&argument) != prepared.plan.binding() {
                return Err(Error::Binding);
            }
            if codec::input(&prepared.plan, &actual)?
                != codec::input(&prepared.plan, &prepared.input)?
            {
                return Err(Error::Binding);
            }
            let state = State::new(
                prepared.plan.clone(),
                prepared.scope.clone(),
                prepared.max_steps,
                prepared.max_reserved_fuel,
                lease.identity(),
            )?;
            let row = created(&state, &actual)?;
            Ok((state, row))
        })();
        let (state, row) = match preflight {
            Ok(value) => value,
            Err(error) => return OwnedFrameStart::Rejected { argument, error },
        };
        let mut journal = match Journal::fresh(lease, key, state) {
            Ok(j) => j,
            Err(error) => return OwnedFrameStart::Rejected { argument, error },
        };
        let branches = match capacity::remaining(&journal.state, &row, key) {
            Ok(b) => b,
            Err(error) => return OwnedFrameStart::Rejected { argument, error },
        };
        if let Err(error) = journal.preflight(&row, &branches) {
            return OwnedFrameStart::Rejected { argument, error };
        }
        // From the first persistence attempt onward uncertainty cannot return a
        // transferable Argument. This root's Drop is backing disposal only.
        let owner = DurableOwner::from_argument(argument);
        let acknowledgement=journal.append(row,&branches).and_then(|_| {
            let row=Record::new(Kind::ArgumentCommitted,json!({"argument_digest":journal.state.argument_digest()?,"storage":codec::storage(&journal.state.plan.liveness().storage)?,"leaf_flags":codec::leaf_flags(&journal.state.plan)}))?;
            let branches=capacity::remaining(&journal.state,&row,key)?;
            journal.append(row,&branches)
        });
        OwnedFrameStart::Invocation {
            invocation: Self {
                journal,
                owner: Some(owner),
                replay_checked: true,
            },
            acknowledgement,
        }
    }
    fn guard(&self, policy: &CapabilityPolicy, scope: &SourceCheckpointScope) -> Result<(), Error> {
        self.journal.validate_current()?;
        if *scope != self.journal.state.scope
            || !policy.allows(self.journal.state.plan.function().id.as_str())
        {
            return Err(Error::Policy);
        }
        Ok(())
    }
    fn append(&mut self, row: Record) -> Result<(), Error> {
        let branches = capacity::remaining(&self.journal.state, &row, self.journal.key)?;
        self.journal.append(row, &branches)
    }
    fn reserve(&mut self, kind: Kind) -> Result<OwnedFrameBudget, Error> {
        let state = &self.journal.state;
        let total = state
            .reserved_total
            .checked_add(state.max_steps)
            .filter(|n| *n <= state.max_reserved_fuel)
            .ok_or(Error::Fuel)?;
        let fields = match kind {
            Kind::StartReserved => {
                json!({"causal_sequence":state.records.len() as u64-1,"reservation":state.max_steps,"reserved_total":total})
            }
            Kind::ResumeReserved => {
                json!({"answered_sequence":state.answered().ok_or(Error::Binding)?.0,"reservation":state.max_steps,"reserved_total":total})
            }
            Kind::ReplayReserved => {
                json!({"basis":state.basis()?,"reservation":state.max_steps,"reserved_total":total})
            }
            _ => return Err(Error::Binding),
        };
        self.append(Record::new(kind, fields)?)?;
        OwnedFrameBudget::new(self.journal.state.max_steps as usize).map_err(|_| Error::Fuel)
    }
    pub(super) fn begin(
        &mut self,
        policy: &CapabilityPolicy,
        scope: &SourceCheckpointScope,
    ) -> Result<(), Error> {
        self.guard(policy, scope)?;
        if !self.replay_checked {
            return Err(Error::Binding);
        }
        if !matches!(self.journal.state.phase, Phase::Committed | Phase::Starting) {
            return Err(Error::Binding);
        }
        let mut budget = match self.reserve(Kind::StartReserved) {
            Ok(b) => b,
            Err(Error::Fuel) => return self.fail(OwnedFrameFailure::FuelExhausted, 0),
            Err(e) => return Err(e),
        };
        self.guard(policy, scope)?;
        self.owner = Some(self.owner.take().ok_or(Error::Binding)?.start(&mut budget));
        self.record_evaluation(budget.consumed() as u64)
    }
    fn record_evaluation(&mut self, steps: u64) -> Result<(), Error> {
        let owner = self.owner.as_ref().ok_or(Error::Binding)?;
        if let Some(failure) = owner.failure().cloned() {
            return self.fail(failure, steps);
        }
        let state = &self.journal.state;
        if let Some(request) = owner.request() {
            let input = owner.input().map_err(|_| Error::Binding)?;
            let bytes = checkpoint::encode(
                &state.plan,
                self.journal.key,
                &state.scope,
                &input,
                state.argument_digest()?,
                request,
                &state.generation,
                state.records.len() as u64,
                state.reserved_total,
            )?;
            self.append(Record::new(Kind::Yielded,json!({"causal_sequence":state.phase_sequence,"checkpoint_digest":checkpoint::digest(&bytes),"checkpoint":String::from_utf8(bytes).map_err(|_|Error::Malformed)?,"consumed_steps":steps}))?)
        } else {
            let result = codec::input(&state.plan, &owner.input().map_err(|_| Error::Binding)?)?;
            self.append(Record::new(Kind::Completed,json!({"causal_sequence":state.phase_sequence,"result_digest":codec::fact_digest(b"semaprax.source-owned-frame-result.v1\0",&result),"result":result,"pending_cleanup":codec::operations(owner.pending_cleanup().ok_or(Error::Binding)?)?,"consumed_steps":steps}))?)
        }
    }
    fn fail(&mut self, failure: OwnedFrameFailure, steps: u64) -> Result<(), Error> {
        let owner = self
            .owner
            .take()
            .ok_or(Error::Binding)?
            .abandon(failure.clone());
        let cleanup = codec::operations(owner.pending_cleanup().ok_or(Error::Binding)?)?;
        self.owner = Some(owner);
        let (name, status) = failure_fields(&failure)?;
        let causal = if steps > 0 {
            self.journal.state.phase_sequence
        } else {
            self.journal.state.records.len() as u64 - 1
        };
        self.append(Record::new(Kind::Failed,json!({"causal_sequence":causal,"failure":name,"language_status":status,"pending_cleanup":cleanup,"consumed_steps":steps}))?)
    }
    pub(super) fn dispatch(
        &mut self,
        policy: &CapabilityPolicy,
        scope: &SourceCheckpointScope,
        handler: &mut dyn FnMut(&ArgumentValue) -> Result<ArgumentValue, ()>,
    ) -> Result<(), Error> {
        self.guard(policy, scope)?;
        if !self.replay_checked {
            return Err(Error::Binding);
        }
        if self.journal.state.phase != Phase::Yielded {
            return Err(if self.journal.state.phase == Phase::Dispatched {
                Error::InDoubt
            } else {
                Error::Binding
            });
        }
        let request = self
            .owner
            .as_ref()
            .and_then(DurableOwner::request)
            .ok_or(Error::Binding)?
            .clone();
        let state = &self.journal.state;
        let (_, yielded) = state.yielded().ok_or(Error::Binding)?;
        self.append(Record::new(Kind::Dispatched,json!({"yielded_sequence":state.phase_sequence,"checkpoint_digest":yielded.fields["checkpoint_digest"],"request_digest":codec::fact_digest(b"semaprax.source-owned-frame-request.v1\0",&codec::scalar(&request)?)}))?)?;
        self.guard(policy, scope)?;
        let answer = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| handler(&request)))
            .map_err(|_| Error::InDoubt)?;
        self.guard(policy, scope)?; // includes the creator PID after callback
        let answer = match answer {
            Ok(a) => a,
            Err(()) => return self.fail(OwnedFrameFailure::HandlerFailed, 0),
        };
        if !snapshot::answer_valid(&self.journal.state.plan, &answer) {
            return self.fail(OwnedFrameFailure::AnswerTypeMismatch, 0);
        }
        let answer = codec::scalar(&answer)?;
        self.append(Record::new(Kind::Answered,json!({"dispatched_sequence":self.journal.state.phase_sequence,"answer_digest":codec::fact_digest(b"semaprax.source-owned-frame-answer.v1\0",&answer),"answer":answer}))?)
    }
    pub(super) fn resume(
        &mut self,
        policy: &CapabilityPolicy,
        scope: &SourceCheckpointScope,
    ) -> Result<(), Error> {
        self.guard(policy, scope)?;
        if !self.replay_checked {
            return Err(Error::Binding);
        }
        if !matches!(self.journal.state.phase, Phase::Answered | Phase::Resuming) {
            return Err(Error::Binding);
        }
        let answer = codec::decode_scalar(
            &self
                .journal
                .state
                .answered()
                .ok_or(Error::Binding)?
                .1
                .fields["answer"],
        )?;
        let mut budget = match self.reserve(Kind::ResumeReserved) {
            Ok(b) => b,
            Err(Error::Fuel) => return self.fail(OwnedFrameFailure::FuelExhausted, 0),
            Err(e) => return Err(e),
        };
        self.guard(policy, scope)?;
        self.owner = Some(
            self.owner
                .take()
                .ok_or(Error::Binding)?
                .resume(answer, &mut budget),
        );
        self.record_evaluation(budget.consumed() as u64)
    }
    pub(super) fn abandon(
        &mut self,
        policy: &CapabilityPolicy,
        scope: &SourceCheckpointScope,
    ) -> Result<(), Error> {
        self.guard(policy, scope)?;
        self.fail(OwnedFrameFailure::HostAbandoned, 0)
    }
    pub(super) fn settle(
        &mut self,
        policy: &CapabilityPolicy,
        scope: &SourceCheckpointScope,
        observer: &mut dyn FnMut(&FinalizeAction) -> bool,
    ) -> Result<(), Error> {
        self.guard(policy, scope)?;
        let state = &self.journal.state;
        if !matches!(state.phase, Phase::Completed | Phase::Failed) {
            return Err(if state.phase == Phase::CleanupStarted {
                Error::InDoubt
            } else {
                Error::Binding
            });
        }
        let pending = &state.terminal().ok_or(Error::Binding)?.fields["pending_cleanup"];
        self.append(Record::new(Kind::CleanupStarted,json!({"terminal_sequence":state.terminal_sequence.ok_or(Error::Binding)?,"cleanup_digest":codec::fact_digest(b"semaprax.source-owned-frame-cleanup.v1\0",pending)}))?)?;
        let journal = &self.journal;
        let mut guard = || {
            journal.validate_current().is_ok()
                && *scope == journal.state.scope
                && policy.allows(journal.state.plan.function().id.as_str())
        };
        let release = match self
            .owner
            .take()
            .ok_or(Error::InDoubt)?
            .settle_guarded(observer, &mut guard)
        {
            Ok(release) => release,
            Err((owner, _)) => {
                self.owner = Some(owner);
                self.journal.poisoned = true;
                return Err(Error::InDoubt);
            }
        };
        self.owner = release.unpublished.map(DurableOwner::from_unpublished);
        self.guard(policy, scope)?;
        let operations=release.observations.iter().map(|(action,success)|Ok(json!({"operation":codec::operations(std::slice::from_ref(action))?[0],"outcome":if *success{"completed"}else{"failed"}}))).collect::<Result<Vec<_>,Error>>()?;
        let receipt = json!({"kind":"observed","settlement":if release.observations.iter().all(|(_,success)|*success){"completed"}else{"failed"},"operations":operations});
        self.append(Record::new(
            Kind::CleanupSettled,
            json!({"cleanup_started_sequence":self.journal.state.phase_sequence,"receipt":receipt}),
        )?)
    }
    pub(super) fn claim(
        &mut self,
        policy: &CapabilityPolicy,
        scope: &SourceCheckpointScope,
    ) -> Result<OwnedFrameResult, Error> {
        self.guard(policy, scope)?;
        if self.owner.is_none() {
            return Err(Error::Binding);
        }
        let state = &self.journal.state;
        if state.phase != Phase::CleanupSettled
            || state
                .terminal()
                .is_none_or(|row| row.kind != Kind::Completed)
        {
            return Err(Error::Binding);
        }
        let completed = state.terminal().ok_or(Error::Binding)?;
        self.append(Record::new(Kind::ResultClaimed,json!({"completed_sequence":state.terminal_sequence.ok_or(Error::Binding)?,"cleanup_settled_sequence":state.phase_sequence,"result_digest":completed.fields["result_digest"]}))?)?;
        self.guard(policy, scope)?;
        let permit = self.journal.claim_permit()?;
        match self.owner.take().ok_or(Error::InDoubt)?.claim(permit) {
            Ok(result) => Ok(result),
            Err(owner) => {
                self.owner = Some(owner);
                self.journal.poisoned = true;
                Err(Error::InDoubt)
            }
        }
    }
    pub(super) fn evidence(&self) -> Result<(Vec<u8>, String), Error> {
        self.journal.evidence()
    }
}
fn created(state: &State, input: &OwnedFrameInput) -> Result<Record, Error> {
    let argument = codec::input(&state.plan, input)?;
    Record::new(
        Kind::Created,
        json!({"profile":super::plan::PROFILE,"scope":codec::scope(&state.scope)?,"function":state.plan.function().id.as_str(),"plan_digest":state.plan.binding(),"signature":codec::signature(&state.plan),"argument_digest":codec::fact_digest(b"semaprax.source-owned-frame-arguments.v1\0",&argument),"argument":argument,"max_steps":state.max_steps,"max_reserved_fuel":state.max_reserved_fuel,"limits":fold::limits()}),
    )
}
fn failure_fields(failure: &OwnedFrameFailure) -> Result<(&'static str, Value), Error> {
    Ok(match failure {
        OwnedFrameFailure::Language(status) => (
            "language_failure",
            codec::parse(status.to_json().as_bytes(), codec::MAX_CARRIER)?,
        ),
        OwnedFrameFailure::FuelExhausted => ("fuel_exhausted", Value::Null),
        OwnedFrameFailure::HostAbandoned => ("host_abandoned", Value::Null),
        OwnedFrameFailure::AnswerTypeMismatch => ("answer_type_mismatch", Value::Null),
        OwnedFrameFailure::EvaluationRejected => ("evaluation_rejected", Value::Null),
        OwnedFrameFailure::HandlerFailed => ("handler_failed", Value::Null),
        OwnedFrameFailure::CallDepthExceeded => ("call_depth_exceeded", Value::Null),
    })
}

#[cfg(all(test, unix))]
mod tests;
