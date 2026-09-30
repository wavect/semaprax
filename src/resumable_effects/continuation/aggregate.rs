//! The separately versioned, Copy-only whole-function aggregate carrier.
//! The checked boundary selects this route before either journal is opened.

use super::journal::channel::{channel_arguments_digest, ChannelJournal, ChannelRecord};
use super::journal::{answer_digest, hex, sha256};
use super::lane::{self, AggregateStep};
use super::{
    channel_type, failure_of, CleanupSettlement, ContinuationAnswer, ContinuationError,
    ContinuationRequest, DurableFailure, JournalDirectory, SourceCheckpointKey,
    SourceCheckpointScope, TornTailPolicy,
};
use crate::hir::ResolvedProgram;
use crate::interpreter::resumable::{ResumableChannelContinuation, ResumableChannelValue};
use crate::interpreter::MAX_STEPS_LIMIT;
use crate::resumable_effects::capability::CapabilityPolicy;
use crate::resumable_effects::core::{CleanupHandler, EffectHandler};
use crate::resumable_effects::source_signature::{
    derive_source_effect_signature_with_arguments, SourceEffectSignature,
};

#[derive(Clone, Debug, PartialEq)]
pub enum AggregateDurableOutcome {
    Completed(ResumableChannelValue),
    Failed(DurableFailure),
}

#[derive(Clone, Debug, PartialEq)]
pub enum AggregateContinuationStatus {
    AwaitingDispatch(ContinuationRequest),
    AwaitingAnswer {
        request: ContinuationRequest,
        in_doubt: bool,
    },
    CleanupPending {
        failure: Option<DurableFailure>,
    },
    CleanupInDoubt {
        failure: Option<DurableFailure>,
    },
    Settled {
        outcome: AggregateDurableOutcome,
        cleanup: CleanupSettlement,
    },
}

#[derive(Clone)]
struct Pending {
    continuation: ResumableChannelContinuation,
    envelope_digest: [u8; 32],
}

impl Pending {
    fn site(&self) -> u32 {
        u32::try_from(self.continuation.history().len()).expect("bounded history")
    }
}

#[derive(Clone)]
enum Phase {
    AwaitingDispatch(Pending),
    AwaitingAnswer(Pending, bool),
    CleanupPending(AggregateDurableOutcome),
    CleanupInDoubt(AggregateDurableOutcome),
    Settled(AggregateDurableOutcome, CleanupSettlement),
}

pub struct AggregateDurableInvocation<'a> {
    program: &'a ResolvedProgram,
    key: &'a SourceCheckpointKey,
    function_id: String,
    arguments: Vec<ResumableChannelValue>,
    signature: SourceEffectSignature,
    scope: SourceCheckpointScope,
    max_steps: usize,
    journal: ChannelJournal,
    phase: Phase,
    poisoned: bool,
}

struct Facts {
    signature: SourceEffectSignature,
    scope: SourceCheckpointScope,
    started: ChannelRecord,
}

fn facts(
    program: &ResolvedProgram,
    function_id: &str,
    arguments: &[ResumableChannelValue],
    invocation_id: &str,
    policy_epoch: u64,
    max_steps: usize,
) -> Result<Facts, ContinuationError> {
    if !(1..=MAX_STEPS_LIMIT).contains(&max_steps) {
        return Err(ContinuationError::InvalidBudget);
    }
    let signature = derive_source_effect_signature_with_arguments(program, function_id)
        .map_err(|error| ContinuationError::Admission(vec![error]))?;
    let program_digest = *signature.plan_identity();
    let scope = SourceCheckpointScope::new(
        format!("resumable-plan:sha256:{}", hex(&program_digest)),
        invocation_id,
        policy_epoch,
    )
    .map_err(|_| ContinuationError::InvalidScope)?;
    // Validate each argument against the checked by-value boundary before
    // creating storage. This is the same admission the interpreter uses.
    crate::interpreter::resumable::checkpoint::checked_channel_arguments_plan(
        program,
        function_id,
        arguments,
    )
    .map_err(|_| ContinuationError::ArgumentsMismatch)?;
    let started = ChannelRecord::Started {
        function: function_id.to_owned(),
        program_digest,
        invocation_id: invocation_id.to_owned(),
        policy_epoch,
        arguments_digest: channel_arguments_digest(arguments),
        yield_count: signature.yield_count(),
        max_steps: max_steps as u64,
    };
    Ok(Facts {
        signature,
        scope,
        started,
    })
}

impl<'a> AggregateDurableInvocation<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        directory: &JournalDirectory,
        key: &'a SourceCheckpointKey,
        program: &'a ResolvedProgram,
        function_id: &str,
        arguments: &[ResumableChannelValue],
        invocation_id: &str,
        policy_epoch: u64,
        max_steps: usize,
    ) -> Result<Self, ContinuationError> {
        let facts = facts(
            program,
            function_id,
            arguments,
            invocation_id,
            policy_epoch,
            max_steps,
        )?;
        let first = lane::start_aggregate(program, function_id, arguments, max_steps)
            .map_err(ContinuationError::Admission)?;
        let journal = ChannelJournal::create(directory, invocation_id)?;
        let mut invocation = Self::assemble(
            program,
            key,
            function_id,
            arguments,
            facts,
            max_steps,
            journal,
        );
        let started = invocation.started_record();
        invocation.append(&started)?;
        invocation.settle_step(first)?;
        Ok(invocation)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn recover(
        directory: &JournalDirectory,
        key: &'a SourceCheckpointKey,
        program: &'a ResolvedProgram,
        function_id: &str,
        arguments: &[ResumableChannelValue],
        invocation_id: &str,
        policy_epoch: u64,
        max_steps: usize,
        torn: TornTailPolicy,
    ) -> Result<Self, ContinuationError> {
        let facts = facts(
            program,
            function_id,
            arguments,
            invocation_id,
            policy_epoch,
            max_steps,
        )?;
        let (journal, records) = ChannelJournal::open(directory, invocation_id, key, torn)?;
        let mut invocation = Self::assemble(
            program,
            key,
            function_id,
            arguments,
            facts,
            max_steps,
            journal,
        );
        if records.is_empty() {
            let started = invocation.started_record();
            invocation.append(&started)?;
            return invocation.replay(&[]);
        }
        compare_started(records.first(), &invocation.started_record())?;
        invocation.replay(&records[1..])
    }

    fn assemble(
        program: &'a ResolvedProgram,
        key: &'a SourceCheckpointKey,
        function_id: &str,
        arguments: &[ResumableChannelValue],
        facts: Facts,
        max_steps: usize,
        journal: ChannelJournal,
    ) -> Self {
        Self {
            program,
            key,
            function_id: function_id.to_owned(),
            arguments: arguments.to_vec(),
            signature: facts.signature,
            scope: facts.scope,
            max_steps,
            journal,
            phase: Phase::CleanupInDoubt(AggregateDurableOutcome::Failed(
                DurableFailure::EvaluationRejected,
            )),
            poisoned: false,
        }
    }

    fn started_record(&self) -> ChannelRecord {
        facts(
            self.program,
            &self.function_id,
            &self.arguments,
            self.scope.invocation_id(),
            self.scope.policy_epoch(),
            self.max_steps,
        )
        .expect("admitted facts")
        .started
    }

    pub fn status(&self) -> AggregateContinuationStatus {
        match &self.phase {
            Phase::AwaitingDispatch(p) => {
                AggregateContinuationStatus::AwaitingDispatch(self.request_of(p))
            }
            Phase::AwaitingAnswer(p, in_doubt) => AggregateContinuationStatus::AwaitingAnswer {
                request: self.request_of(p),
                in_doubt: *in_doubt,
            },
            Phase::CleanupPending(outcome) => AggregateContinuationStatus::CleanupPending {
                failure: failure_class(outcome),
            },
            Phase::CleanupInDoubt(outcome) => AggregateContinuationStatus::CleanupInDoubt {
                failure: failure_class(outcome),
            },
            Phase::Settled(outcome, cleanup) => AggregateContinuationStatus::Settled {
                outcome: outcome.clone(),
                cleanup: *cleanup,
            },
        }
    }

    fn request_of(&self, pending: &Pending) -> ContinuationRequest {
        ContinuationRequest {
            program_digest: *self.signature.plan_identity(),
            invocation_id: self.scope.invocation_id().to_owned(),
            site: pending.site(),
            envelope_digest: pending.envelope_digest,
            request: pending.continuation.request().clone(),
        }
    }

    pub fn dispatch(
        &mut self,
        policy: &CapabilityPolicy,
    ) -> Result<ContinuationRequest, ContinuationError> {
        self.usable()?;
        let Phase::AwaitingDispatch(pending) = self.phase.clone() else {
            return Err(ContinuationError::NotAwaitingDispatch);
        };
        if !policy.allows(&self.function_id) {
            return Err(ContinuationError::CapabilityDenied);
        }
        self.append(&ChannelRecord::Dispatched {
            site: pending.site(),
            envelope_digest: pending.envelope_digest,
        })?;
        let request = self.request_of(&pending);
        self.phase = Phase::AwaitingAnswer(pending, false);
        Ok(request)
    }

    pub fn answer(
        &mut self,
        policy: &CapabilityPolicy,
        answer: &ContinuationAnswer,
    ) -> Result<(), ContinuationError> {
        self.usable()?;
        let pending = match &self.phase {
            Phase::AwaitingAnswer(pending, _) => pending.clone(),
            Phase::AwaitingDispatch(pending) => {
                return Err(if answer.site < pending.site() {
                    ContinuationError::ReplayedAnswer
                } else {
                    ContinuationError::NotDispatched
                })
            }
            _ => return Err(ContinuationError::AlreadySettled),
        };
        if !policy.allows(&self.function_id) {
            return Err(ContinuationError::CapabilityDenied);
        }
        if answer.program_digest != *self.signature.plan_identity() {
            return Err(ContinuationError::ProgramMismatch);
        }
        if answer.invocation_id != self.scope.invocation_id() {
            return Err(ContinuationError::InvocationMismatch);
        }
        if answer.site < pending.site() {
            return Err(ContinuationError::ReplayedAnswer);
        }
        if answer.site != pending.site() {
            return Err(ContinuationError::SiteMismatch);
        }
        if answer.envelope_digest != pending.envelope_digest {
            return Err(ContinuationError::StaleEnvelope);
        }
        if !self.answer_type_matches(&answer.value) {
            return Err(ContinuationError::AnswerTypeMismatch);
        }
        self.append(&ChannelRecord::Answered {
            site: pending.site(),
            envelope_digest: pending.envelope_digest,
            answer: answer.value.clone(),
            answer_digest: answer_digest(&answer.value),
        })?;
        self.resume_with(&pending.continuation, &answer.value)
    }

    pub fn abandon(&mut self, policy: &CapabilityPolicy) -> Result<(), ContinuationError> {
        self.usable()?;
        if !matches!(self.phase, Phase::AwaitingAnswer(..)) {
            return Err(ContinuationError::NotDispatched);
        }
        if !policy.allows(&self.function_id) {
            return Err(ContinuationError::CapabilityDenied);
        }
        self.terminal(AggregateDurableOutcome::Failed(
            DurableFailure::HostAbandoned,
        ))
    }

    pub fn settle(
        &mut self,
        cleanup: &mut dyn CleanupHandler<AggregateDurableOutcome>,
    ) -> Result<(), ContinuationError> {
        self.usable()?;
        let Phase::CleanupPending(outcome) = self.phase.clone() else {
            return Err(ContinuationError::NotAwaitingCleanup);
        };
        self.append(&ChannelRecord::CleanupStarted)?;
        self.phase = Phase::CleanupInDoubt(outcome.clone());
        let settlement = if cleanup.run(&outcome).is_ok() {
            CleanupSettlement::Completed
        } else {
            CleanupSettlement::Failed
        };
        self.finish_cleanup(outcome, settlement)
    }

    pub fn confirm_cleanup(&mut self) -> Result<(), ContinuationError> {
        self.usable()?;
        let Phase::CleanupInDoubt(outcome) = self.phase.clone() else {
            return Err(ContinuationError::NotAwaitingCleanup);
        };
        self.finish_cleanup(outcome, CleanupSettlement::HostConfirmed)
    }

    pub fn drive(
        &mut self,
        policy: &CapabilityPolicy,
        handler: &mut dyn EffectHandler<ResumableChannelValue, ResumableChannelValue>,
        cleanup: &mut dyn CleanupHandler<AggregateDurableOutcome>,
    ) -> Result<AggregateContinuationStatus, ContinuationError> {
        loop {
            self.usable()?;
            match self.phase.clone() {
                Phase::AwaitingDispatch(_) => {
                    let request = self.dispatch(policy)?;
                    match handler.dispatch(&request.request) {
                        Ok(value) if self.answer_type_matches(&value) => {
                            self.answer(policy, &request.bind_answer(value))?
                        }
                        Ok(_) => self.terminal(AggregateDurableOutcome::Failed(
                            DurableFailure::AnswerTypeMismatch,
                        ))?,
                        Err(_) => self.terminal(AggregateDurableOutcome::Failed(
                            DurableFailure::HandlerFailed,
                        ))?,
                    }
                }
                Phase::CleanupPending(_) => self.settle(cleanup)?,
                Phase::AwaitingAnswer(..) | Phase::CleanupInDoubt(_) | Phase::Settled(..) => {
                    return Ok(self.status())
                }
            }
        }
    }

    fn usable(&self) -> Result<(), ContinuationError> {
        if self.poisoned {
            Err(ContinuationError::Poisoned)
        } else {
            Ok(())
        }
    }

    fn append(&mut self, record: &ChannelRecord) -> Result<(), ContinuationError> {
        self.journal.append(self.key, record).inspect_err(|_| {
            self.poisoned = true;
        })
    }

    fn answer_type_matches(&self, value: &ResumableChannelValue) -> bool {
        let Phase::AwaitingAnswer(pending, _) = &self.phase else {
            return false;
        };
        self.answer_type_for(pending, value)
    }

    fn finish_cleanup(
        &mut self,
        outcome: AggregateDurableOutcome,
        settlement: CleanupSettlement,
    ) -> Result<(), ContinuationError> {
        self.append(&ChannelRecord::CleanupSettled {
            settlement: settlement.class().to_owned(),
        })?;
        self.phase = Phase::Settled(outcome, settlement);
        Ok(())
    }

    fn terminal(&mut self, outcome: AggregateDurableOutcome) -> Result<(), ContinuationError> {
        let record = match &outcome {
            AggregateDurableOutcome::Completed(result) => ChannelRecord::Completed {
                result: result.clone(),
            },
            AggregateDurableOutcome::Failed(failure) => ChannelRecord::Failed {
                class: failure.class().to_owned(),
            },
        };
        self.append(&record)?;
        self.phase = Phase::CleanupPending(outcome);
        Ok(())
    }

    fn resume_with(
        &mut self,
        continuation: &ResumableChannelContinuation,
        answer: &ResumableChannelValue,
    ) -> Result<(), ContinuationError> {
        match lane::resume_aggregate(
            self.program,
            &self.function_id,
            &self.arguments,
            continuation,
            answer,
            self.max_steps,
        ) {
            Ok(step) => self.settle_step(step),
            Err(_) => self.terminal(AggregateDurableOutcome::Failed(
                DurableFailure::EvaluationRejected,
            )),
        }
    }

    fn settle_step(&mut self, step: AggregateStep) -> Result<(), ContinuationError> {
        match step {
            AggregateStep::Suspended(continuation) => {
                let envelope = lane::encode_aggregate(
                    self.program,
                    self.key,
                    &self.scope,
                    &self.function_id,
                    &self.arguments,
                    &continuation,
                )
                .map_err(ContinuationError::Envelope)
                .and_then(|bytes| {
                    String::from_utf8(bytes).map_err(|_| ContinuationError::TamperedJournal)
                });
                let Ok(envelope) = envelope else {
                    self.poisoned = true;
                    return Err(ContinuationError::Poisoned);
                };
                let envelope_digest = sha256(envelope.as_bytes());
                let site = u32::try_from(continuation.history().len()).expect("bounded history");
                self.append(&ChannelRecord::Yielded {
                    site,
                    envelope,
                    envelope_digest,
                })?;
                self.phase = Phase::AwaitingDispatch(Pending {
                    continuation,
                    envelope_digest,
                });
                Ok(())
            }
            AggregateStep::Completed(result) => {
                self.terminal(AggregateDurableOutcome::Completed(result))
            }
            AggregateStep::Failed(failure) => {
                self.terminal(AggregateDurableOutcome::Failed(failure))
            }
        }
    }

    fn replay(mut self, records: &[ChannelRecord]) -> Result<Self, ContinuationError> {
        enum Tail {
            Started,
            Yielded(Pending),
            Dispatched(Pending),
            Answered(Pending, ResumableChannelValue),
            Terminal(AggregateDurableOutcome),
            CleanupStarted(AggregateDurableOutcome),
            Settled(AggregateDurableOutcome, CleanupSettlement),
        }
        let mut next_site = 0_u32;
        let mut tail = Tail::Started;
        for record in records {
            tail = match (tail, record) {
                (
                    prior @ (Tail::Started | Tail::Answered(..)),
                    ChannelRecord::Yielded {
                        site,
                        envelope,
                        envelope_digest,
                    },
                ) => {
                    if *site != next_site
                        || *site >= self.signature.yield_count()
                        || sha256(envelope.as_bytes()) != *envelope_digest
                    {
                        return Err(ContinuationError::TamperedJournal);
                    }
                    let continuation = lane::decode_aggregate(
                        self.program,
                        self.key,
                        &self.scope,
                        &self.function_id,
                        &self.arguments,
                        envelope.as_bytes(),
                    )
                    .map_err(ContinuationError::Envelope)?;
                    let pending = Pending {
                        continuation,
                        envelope_digest: *envelope_digest,
                    };
                    if pending.site() != *site {
                        return Err(ContinuationError::TamperedJournal);
                    }
                    match &prior {
                        Tail::Started if pending.continuation.history().len() == 0 => {}
                        Tail::Answered(previous, answer) => {
                            let Some((recorded_request, recorded_answer)) =
                                pending.continuation.history().last()
                            else {
                                return Err(ContinuationError::TamperedJournal);
                            };
                            if recorded_request != previous.continuation.request()
                                || recorded_answer != answer
                            {
                                return Err(ContinuationError::TamperedJournal);
                            }
                        }
                        _ => return Err(ContinuationError::TamperedJournal),
                    }
                    Tail::Yielded(pending)
                }
                (
                    Tail::Yielded(p),
                    ChannelRecord::Dispatched {
                        site,
                        envelope_digest,
                    },
                ) if *site == next_site && *envelope_digest == p.envelope_digest => {
                    Tail::Dispatched(p)
                }
                (
                    Tail::Dispatched(p),
                    ChannelRecord::Answered {
                        site,
                        envelope_digest,
                        answer,
                        answer_digest: digest,
                    },
                ) if *site == next_site
                    && *envelope_digest == p.envelope_digest
                    && *digest == answer_digest(answer) =>
                {
                    if !self.answer_type_for(&p, answer) {
                        return Err(ContinuationError::TamperedJournal);
                    }
                    next_site += 1;
                    Tail::Answered(p, answer.clone())
                }
                (Tail::Started | Tail::Answered(..), ChannelRecord::Completed { result }) => {
                    Tail::Terminal(AggregateDurableOutcome::Completed(result.clone()))
                }
                (
                    prior @ (Tail::Started | Tail::Answered(..) | Tail::Dispatched(_)),
                    ChannelRecord::Failed { class },
                ) => {
                    let failure = failure_of(class)?;
                    if failure.settles_dispatch() != matches!(prior, Tail::Dispatched(_)) {
                        return Err(ContinuationError::TamperedJournal);
                    }
                    Tail::Terminal(AggregateDurableOutcome::Failed(failure))
                }
                (Tail::Terminal(outcome), ChannelRecord::CleanupStarted) => {
                    Tail::CleanupStarted(outcome)
                }
                (Tail::CleanupStarted(outcome), ChannelRecord::CleanupSettled { settlement }) => {
                    let settlement = CleanupSettlement::ALL
                        .into_iter()
                        .find(|s| s.class() == settlement)
                        .ok_or(ContinuationError::TamperedJournal)?;
                    Tail::Settled(outcome, settlement)
                }
                _ => return Err(ContinuationError::TamperedJournal),
            };
        }
        match tail {
            Tail::Started => {
                let step = lane::start_aggregate(
                    self.program,
                    &self.function_id,
                    &self.arguments,
                    self.max_steps,
                )
                .map_err(ContinuationError::Admission)?;
                self.settle_step(step)?;
            }
            Tail::Yielded(p) => self.phase = Phase::AwaitingDispatch(p),
            Tail::Dispatched(p) => self.phase = Phase::AwaitingAnswer(p, true),
            Tail::Answered(p, answer) => self.resume_with(&p.continuation, &answer)?,
            Tail::Terminal(outcome) => self.phase = Phase::CleanupPending(outcome),
            Tail::CleanupStarted(outcome) => self.phase = Phase::CleanupInDoubt(outcome),
            Tail::Settled(outcome, cleanup) => self.phase = Phase::Settled(outcome, cleanup),
        }
        Ok(self)
    }

    fn answer_type_for(&self, pending: &Pending, value: &ResumableChannelValue) -> bool {
        if !crate::interpreter::resumable::channel::valid_copy_channel_response(
            self.program,
            &self.function_id,
            value,
        ) {
            return false;
        }
        let Some(request_type) = channel_type(pending.continuation.request()) else {
            return false;
        };
        let Some(answer_type) = channel_type(value) else {
            return false;
        };
        self.signature
            .table()
            .check_answer(
                &super::tag(&self.function_id, &request_type),
                &super::tag(&self.function_id, &answer_type),
            )
            .is_ok()
    }
}

fn failure_class(outcome: &AggregateDurableOutcome) -> Option<DurableFailure> {
    match outcome {
        AggregateDurableOutcome::Completed(_) => None,
        AggregateDurableOutcome::Failed(failure) => Some(*failure),
    }
}

fn compare_started(
    observed: Option<&ChannelRecord>,
    expected: &ChannelRecord,
) -> Result<(), ContinuationError> {
    let (
        Some(ChannelRecord::Started {
            function,
            program_digest,
            invocation_id,
            policy_epoch,
            arguments_digest,
            yield_count,
            max_steps,
        }),
        ChannelRecord::Started {
            function: expected_function,
            program_digest: expected_program,
            invocation_id: expected_invocation,
            policy_epoch: expected_epoch,
            arguments_digest: expected_arguments,
            yield_count: expected_count,
            max_steps: expected_steps,
        },
    ) = (observed, expected)
    else {
        return Err(ContinuationError::TamperedJournal);
    };
    if invocation_id != expected_invocation {
        return Err(ContinuationError::InvocationMismatch);
    }
    if function != expected_function {
        return Err(ContinuationError::FunctionMismatch);
    }
    if program_digest != expected_program || yield_count != expected_count {
        return Err(ContinuationError::ProgramMismatch);
    }
    if policy_epoch != expected_epoch {
        return Err(ContinuationError::PolicyEpochMismatch);
    }
    if arguments_digest != expected_arguments {
        return Err(ContinuationError::ArgumentsMismatch);
    }
    if max_steps != expected_steps {
        return Err(ContinuationError::BudgetMismatch);
    }
    Ok(())
}
