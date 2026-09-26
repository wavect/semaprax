//! Public bounded continuation execution for the admitted sequential
//! Copy-scalar profile (`semaprax.resumable-continuation.v1`).
//!
//! [`DurableInvocation`] runs the compiler-owned start/resume plans on the
//! interpreter (`interpreter::resumable`) and records every observable step
//! in the durable [`journal`] before acknowledging it. The request/answer
//! exchange is non-bearer: a [`ContinuationAnswer`] is accepted only for the
//! exact program digest, invocation, yield-site index and continuation
//! envelope digest that the journal currently awaits, and only while the
//! host presents a [`CapabilityPolicy`] that holds the selected function's
//! capability. Neither the journal nor the HMAC continuation envelope carries
//! authority: decoding either answers nothing and dispatches nothing.
//!
//! Recovery reads the observed tail and never repeats a settled step. A
//! dispatched yield whose answer is not journaled is *in doubt*: recovery
//! reports it and refuses to dispatch it again; only an explicit bound answer
//! (or an explicit abandon) settles it. Terminal cleanup runs at most once
//! the same way. [`docs/RESUMABLE-EFFECTS-CONTINUATION-V1.md`] owns the
//! contract.
//!
//! [`docs/RESUMABLE-EFFECTS-CONTINUATION-V1.md`]: ../../docs/RESUMABLE-EFFECTS-CONTINUATION-V1.md

pub mod journal;
#[cfg(test)]
mod tests;

use super::capability::CapabilityPolicy;
use super::core::{CleanupHandler, EffectHandler};
use super::source_checkpoint::{
    decode_source_checkpoint_v2, encode_source_checkpoint_v2, SourceCheckpointError,
    SourceCheckpointKey, SourceCheckpointScope,
};
use super::source_driver::{scalar, tag};
use super::source_signature::{derive_source_effect_signature, SourceEffectSignature};
use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedProgram;
use crate::interpreter::resumable::checkpoint::scalar_json;
use crate::interpreter::resumable::{
    resume_sequential_resumable_effect, run_sequential_resumable_effect, ResumableContinuation,
    SequentialResumableStep,
};
use crate::interpreter::{ArgumentValue, MAX_STEPS_LIMIT};
use journal::{answer_digest, hex, sha256, Journal, Record};
pub use journal::{JournalDirectory, TornTailPolicy, RESUMABLE_JOURNAL_SCHEMA_V1};

/// Breaking changes to the request/answer or journal protocol require a new
/// contract identity.
pub const RESUMABLE_CONTINUATION_CONTRACT_V1: &str = "semaprax.resumable-continuation.v1";

/// Stable refusal classes. Every refusal leaves the journal unchanged except
/// [`ContinuationError::Storage`], after which the in-memory invocation is
/// poisoned and only [`DurableInvocation::recover`] may continue.
#[derive(Clone, Debug)]
pub enum ContinuationError {
    Admission(Vec<Diagnostic>),
    UnsupportedProfile,
    InvalidBudget,
    InvalidScope,
    ForeignDirectory,
    Storage,
    Poisoned,
    AlreadyStarted,
    NotStarted,
    TornTail,
    TamperedJournal,
    SchemaMismatch,
    FunctionMismatch,
    ProgramMismatch,
    InvocationMismatch,
    PolicyEpochMismatch,
    ArgumentsMismatch,
    Envelope(SourceCheckpointError),
    CapabilityDenied,
    NotAwaitingDispatch,
    NotDispatched,
    ReplayedAnswer,
    SiteMismatch,
    StaleEnvelope,
    AnswerTypeMismatch,
    AlreadySettled,
    NotAwaitingCleanup,
}

/// A pending request handed to the host. It is a description, not a
/// capability: answering it still requires the held policy.
#[derive(Clone, Debug, PartialEq)]
pub struct ContinuationRequest {
    pub program_digest: [u8; 32],
    pub invocation_id: String,
    pub site: u32,
    pub envelope_digest: [u8; 32],
    pub request: ArgumentValue,
}

impl ContinuationRequest {
    /// Bind `value` to exactly this request's identity.
    pub fn bind_answer(&self, value: ArgumentValue) -> ContinuationAnswer {
        ContinuationAnswer {
            program_digest: self.program_digest,
            invocation_id: self.invocation_id.clone(),
            site: self.site,
            envelope_digest: self.envelope_digest,
            value,
        }
    }
}

/// A host answer. Every identity field must match the awaited request.
#[derive(Clone, Debug, PartialEq)]
pub struct ContinuationAnswer {
    pub program_digest: [u8; 32],
    pub invocation_id: String,
    pub site: u32,
    pub envelope_digest: [u8; 32],
    pub value: ArgumentValue,
}

/// Sticky terminal outcome. Cleanup never replaces it.
#[derive(Clone, Debug, PartialEq)]
pub enum DurableOutcome {
    Completed(ArgumentValue),
    Failed(DurableFailure),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DurableFailure {
    LanguageFailure,
    FuelExhausted,
    CallDepthExceeded,
    EvaluationRejected,
    HandlerFailed,
    AnswerTypeMismatch,
    HostAbandoned,
}

impl DurableFailure {
    const ALL: [Self; 7] = [
        Self::LanguageFailure,
        Self::FuelExhausted,
        Self::CallDepthExceeded,
        Self::EvaluationRejected,
        Self::HandlerFailed,
        Self::AnswerTypeMismatch,
        Self::HostAbandoned,
    ];

    /// Failures that settle a dispatched site rather than a plan step.
    fn settles_dispatch(self) -> bool {
        matches!(
            self,
            Self::HandlerFailed | Self::AnswerTypeMismatch | Self::HostAbandoned
        )
    }

    fn class(self) -> &'static str {
        match self {
            Self::LanguageFailure => "language_failure",
            Self::FuelExhausted => "fuel_exhausted",
            Self::CallDepthExceeded => "call_depth_exceeded",
            Self::EvaluationRejected => "evaluation_rejected",
            Self::HandlerFailed => "handler_failed",
            Self::AnswerTypeMismatch => "answer_type_mismatch",
            Self::HostAbandoned => "host_abandoned",
        }
    }
}

/// How terminal cleanup settled. `HostConfirmed` settles an in-doubt cleanup
/// without running it again.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CleanupSettlement {
    Completed,
    Failed,
    HostConfirmed,
}

impl CleanupSettlement {
    const ALL: [Self; 3] = [Self::Completed, Self::Failed, Self::HostConfirmed];

    fn class(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::HostConfirmed => "host_confirmed",
        }
    }
}

/// The observable state of one durable invocation.
#[derive(Clone, Debug, PartialEq)]
pub enum ContinuationStatus {
    /// Journaled `Yielded`; the request has not been dispatched.
    AwaitingDispatch(ContinuationRequest),
    /// Journaled `Dispatched`. `in_doubt` is true when this state was
    /// recovered from storage: the host may already have acted, so the
    /// driver will not dispatch it again.
    AwaitingAnswer {
        request: ContinuationRequest,
        in_doubt: bool,
    },
    CleanupPending(DurableOutcome),
    /// Journaled `CleanupStarted` without `CleanupSettled`.
    CleanupInDoubt(DurableOutcome),
    /// Fully settled; only now is the outcome published.
    Settled {
        outcome: DurableOutcome,
        cleanup: CleanupSettlement,
    },
}

#[derive(Clone)]
enum Phase {
    AwaitingDispatch(Pending),
    AwaitingAnswer(Pending, bool),
    CleanupPending(DurableOutcome),
    CleanupInDoubt(DurableOutcome),
    Settled(DurableOutcome, CleanupSettlement),
}

#[derive(Clone)]
struct Pending {
    continuation: ResumableContinuation,
    envelope_digest: [u8; 32],
}

/// One invocation of a checked source function driven through the durable
/// journal. It borrows the checked program and the caller's key; it owns only
/// its journal descriptor.
pub struct DurableInvocation<'a> {
    program: &'a ResolvedProgram,
    key: &'a SourceCheckpointKey,
    function_id: String,
    arguments: Vec<ArgumentValue>,
    signature: SourceEffectSignature,
    scope: SourceCheckpointScope,
    max_steps: usize,
    journal: Journal,
    phase: Phase,
    poisoned: bool,
}

struct Facts {
    signature: SourceEffectSignature,
    scope: SourceCheckpointScope,
    started: Record,
}

#[allow(clippy::too_many_arguments)]
fn facts(
    program: &ResolvedProgram,
    function_id: &str,
    arguments: &[ArgumentValue],
    invocation_id: &str,
    policy_epoch: u64,
    max_steps: usize,
) -> Result<Facts, ContinuationError> {
    if !(1..=MAX_STEPS_LIMIT).contains(&max_steps) {
        return Err(ContinuationError::InvalidBudget);
    }
    let signature = derive_source_effect_signature(program, function_id)
        .map_err(|error| ContinuationError::Admission(vec![error]))?;
    if signature.yield_count() < 2 {
        // The v2 continuation envelope exists only for the sequential lane.
        return Err(ContinuationError::UnsupportedProfile);
    }
    let program_digest = *signature.plan_identity();
    let scope = SourceCheckpointScope::new(
        format!("resumable-plan:sha256:{}", hex(&program_digest)),
        invocation_id,
        policy_epoch,
    )
    .map_err(|_| ContinuationError::InvalidScope)?;
    let mut argument_bytes = Vec::new();
    for argument in arguments {
        if scalar(argument).is_none() {
            return Err(ContinuationError::ArgumentsMismatch);
        }
        argument_bytes.extend_from_slice(scalar_json(argument).to_string().as_bytes());
        argument_bytes.push(b'\n');
    }
    let started = Record::Started {
        function: function_id.to_owned(),
        program_digest,
        invocation_id: invocation_id.to_owned(),
        policy_epoch,
        arguments_digest: sha256(&argument_bytes),
        yield_count: signature.yield_count(),
    };
    Ok(Facts {
        signature,
        scope,
        started,
    })
}

fn failure_of(class: &str) -> Result<DurableFailure, ContinuationError> {
    DurableFailure::ALL
        .into_iter()
        .find(|failure| failure.class() == class)
        .ok_or(ContinuationError::TamperedJournal)
}

impl<'a> DurableInvocation<'a> {
    /// Start a fresh invocation. Admission and the pure start plan run before
    /// the journal is created, so a refused start leaves no file behind.
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        directory: &JournalDirectory,
        key: &'a SourceCheckpointKey,
        program: &'a ResolvedProgram,
        function_id: &str,
        arguments: &[ArgumentValue],
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
        let first = run_sequential_resumable_effect(program, function_id, arguments, max_steps)
            .map_err(ContinuationError::Admission)?;
        let journal = Journal::create(directory, invocation_id)?;
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
        invocation.settle_step(first.step)?;
        Ok(invocation)
    }

    /// Recover an invocation from its journal. Every caller-supplied fact
    /// must equal the journaled `Started` record exactly. A tail of `Started`
    /// or `Answered` resumes pure evaluation (no dispatch); every other tail
    /// is restored as-is.
    #[allow(clippy::too_many_arguments)]
    pub fn recover(
        directory: &JournalDirectory,
        key: &'a SourceCheckpointKey,
        program: &'a ResolvedProgram,
        function_id: &str,
        arguments: &[ArgumentValue],
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
        let (journal, records) = Journal::open(directory, invocation_id, key, torn)?;
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
            // `Started` was never acknowledged, so the invocation never
            // began: acknowledge it now from the caller's exact facts.
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
        arguments: &[ArgumentValue],
        facts: Facts,
        max_steps: usize,
        journal: Journal,
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
            // Placeholder until the first record settles; never observable.
            phase: Phase::CleanupInDoubt(DurableOutcome::Failed(
                DurableFailure::EvaluationRejected,
            )),
            poisoned: false,
        }
    }

    fn started_record(&self) -> Record {
        facts(
            self.program,
            &self.function_id,
            &self.arguments,
            self.scope.invocation_id(),
            self.scope.policy_epoch(),
            self.max_steps,
        )
        .expect("facts were already admitted")
        .started
    }

    /// The observable state. Outcomes are published only once settled.
    pub fn status(&self) -> ContinuationStatus {
        match &self.phase {
            Phase::AwaitingDispatch(pending) => {
                ContinuationStatus::AwaitingDispatch(self.request_of(pending))
            }
            Phase::AwaitingAnswer(pending, in_doubt) => ContinuationStatus::AwaitingAnswer {
                request: self.request_of(pending),
                in_doubt: *in_doubt,
            },
            Phase::CleanupPending(outcome) => ContinuationStatus::CleanupPending(outcome.clone()),
            Phase::CleanupInDoubt(outcome) => ContinuationStatus::CleanupInDoubt(outcome.clone()),
            Phase::Settled(outcome, cleanup) => ContinuationStatus::Settled {
                outcome: outcome.clone(),
                cleanup: *cleanup,
            },
        }
    }

    fn request_of(&self, pending: &Pending) -> ContinuationRequest {
        ContinuationRequest {
            program_digest: *self.signature.plan_identity(),
            invocation_id: self.scope.invocation_id().to_owned(),
            site: site_of(&pending.continuation),
            envelope_digest: pending.envelope_digest,
            request: pending.continuation.request().clone(),
        }
    }

    /// Hand the awaiting request to the host. `Dispatched` is durable before
    /// the request is returned, so a crash after this point is in doubt and
    /// the site is never dispatched again.
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
        self.append(&Record::Dispatched {
            site: site_of(&pending.continuation),
            envelope_digest: pending.envelope_digest,
        })?;
        let request = self.request_of(&pending);
        self.phase = Phase::AwaitingAnswer(pending, false);
        Ok(request)
    }

    /// Settle the awaited site with a bound host answer, then evaluate the
    /// compiler-owned resume plan to the next yield or terminal.
    pub fn answer(
        &mut self,
        policy: &CapabilityPolicy,
        answer: &ContinuationAnswer,
    ) -> Result<(), ContinuationError> {
        self.usable()?;
        let pending = match &self.phase {
            Phase::AwaitingAnswer(pending, _) => pending.clone(),
            Phase::AwaitingDispatch(pending) => {
                return Err(if answer.site < site_of(&pending.continuation) {
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
        let site = site_of(&pending.continuation);
        if answer.site < site {
            return Err(ContinuationError::ReplayedAnswer);
        }
        if answer.site != site {
            return Err(ContinuationError::SiteMismatch);
        }
        if answer.envelope_digest != pending.envelope_digest {
            return Err(ContinuationError::StaleEnvelope);
        }
        if !self.answer_type_matches(&answer.value) {
            return Err(ContinuationError::AnswerTypeMismatch);
        }
        self.append(&Record::Answered {
            site,
            envelope_digest: pending.envelope_digest,
            answer: answer.value.clone(),
            answer_digest: answer_digest(&answer.value),
        })?;
        self.resume_with(&pending.continuation, &answer.value)
    }

    /// Settle an awaited (typically in-doubt) site as a sticky host failure
    /// instead of answering it.
    pub fn abandon(&mut self, policy: &CapabilityPolicy) -> Result<(), ContinuationError> {
        self.usable()?;
        if !matches!(self.phase, Phase::AwaitingAnswer(..)) {
            return Err(ContinuationError::NotDispatched);
        }
        if !policy.allows(&self.function_id) {
            return Err(ContinuationError::CapabilityDenied);
        }
        self.terminal(DurableOutcome::Failed(DurableFailure::HostAbandoned))
    }

    /// Run terminal cleanup exactly once. `CleanupStarted` is durable before
    /// the handler runs; a cleanup failure is recorded but never replaces
    /// the sticky outcome.
    pub fn settle(
        &mut self,
        cleanup: &mut dyn CleanupHandler<DurableOutcome>,
    ) -> Result<(), ContinuationError> {
        self.usable()?;
        let Phase::CleanupPending(outcome) = self.phase.clone() else {
            return Err(ContinuationError::NotAwaitingCleanup);
        };
        self.append(&Record::CleanupStarted)?;
        self.phase = Phase::CleanupInDoubt(outcome.clone());
        let settlement = match cleanup.run(&outcome) {
            Ok(()) => CleanupSettlement::Completed,
            Err(_) => CleanupSettlement::Failed,
        };
        self.finish_cleanup(outcome, settlement)
    }

    /// Settle an in-doubt cleanup on the host's word without running it.
    pub fn confirm_cleanup(&mut self) -> Result<(), ContinuationError> {
        self.usable()?;
        let Phase::CleanupInDoubt(outcome) = self.phase.clone() else {
            return Err(ContinuationError::NotAwaitingCleanup);
        };
        self.finish_cleanup(outcome, CleanupSettlement::HostConfirmed)
    }

    /// Drive with an injected host until settled or until a step needs an
    /// explicit host decision (an in-doubt answer or cleanup).
    pub fn drive(
        &mut self,
        policy: &CapabilityPolicy,
        handler: &mut dyn EffectHandler<ArgumentValue, ArgumentValue>,
        cleanup: &mut dyn CleanupHandler<DurableOutcome>,
    ) -> Result<ContinuationStatus, ContinuationError> {
        loop {
            self.usable()?;
            match self.phase.clone() {
                Phase::AwaitingDispatch(_) => {
                    let request = self.dispatch(policy)?;
                    match handler.dispatch(&request.request) {
                        Ok(value) if self.answer_type_matches(&value) => {
                            self.answer(policy, &request.bind_answer(value))?
                        }
                        Ok(_) => self
                            .terminal(DurableOutcome::Failed(DurableFailure::AnswerTypeMismatch))?,
                        Err(_) => {
                            self.terminal(DurableOutcome::Failed(DurableFailure::HandlerFailed))?
                        }
                    }
                }
                Phase::AwaitingAnswer(..) | Phase::CleanupInDoubt(_) | Phase::Settled(..) => {
                    return Ok(self.status())
                }
                Phase::CleanupPending(_) => self.settle(cleanup)?,
            }
        }
    }

    fn usable(&self) -> Result<(), ContinuationError> {
        if self.poisoned {
            return Err(ContinuationError::Poisoned);
        }
        Ok(())
    }

    fn append(&mut self, record: &Record) -> Result<(), ContinuationError> {
        self.journal.append(self.key, record).inspect_err(|_| {
            self.poisoned = true;
        })
    }

    fn answer_type_matches(&self, value: &ArgumentValue) -> bool {
        let Some((_, request_type, _)) = scalar(self.current_request()) else {
            return false;
        };
        let Some((_, answer_type, _)) = scalar(value) else {
            return false;
        };
        self.signature
            .table()
            .check_answer(
                &tag(&self.function_id, &request_type),
                &tag(&self.function_id, &answer_type),
            )
            .is_ok()
    }

    fn current_request(&self) -> &ArgumentValue {
        match &self.phase {
            Phase::AwaitingDispatch(pending) | Phase::AwaitingAnswer(pending, _) => {
                pending.continuation.request()
            }
            _ => &ArgumentValue::Bool(false),
        }
    }

    fn finish_cleanup(
        &mut self,
        outcome: DurableOutcome,
        settlement: CleanupSettlement,
    ) -> Result<(), ContinuationError> {
        self.append(&Record::CleanupSettled {
            settlement: settlement.class().to_owned(),
        })?;
        self.phase = Phase::Settled(outcome, settlement);
        Ok(())
    }

    fn terminal(&mut self, outcome: DurableOutcome) -> Result<(), ContinuationError> {
        let record = match &outcome {
            DurableOutcome::Completed(result) => Record::Completed {
                result: result.clone(),
            },
            DurableOutcome::Failed(failure) => Record::Failed {
                class: failure.class().to_owned(),
            },
        };
        self.append(&record)?;
        self.phase = Phase::CleanupPending(outcome);
        Ok(())
    }

    fn resume_with(
        &mut self,
        continuation: &ResumableContinuation,
        answer: &ArgumentValue,
    ) -> Result<(), ContinuationError> {
        match resume_sequential_resumable_effect(
            self.program,
            &self.function_id,
            &self.arguments,
            continuation,
            answer,
            self.max_steps,
        ) {
            Ok(evaluation) => self.settle_step(evaluation.step),
            Err(_) => self.terminal(DurableOutcome::Failed(DurableFailure::EvaluationRejected)),
        }
    }

    fn settle_step(&mut self, step: SequentialResumableStep) -> Result<(), ContinuationError> {
        let failed = DurableOutcome::Failed;
        match step {
            SequentialResumableStep::Suspended { continuation } => {
                let envelope = encode_source_checkpoint_v2(
                    self.program,
                    self.key,
                    &self.scope,
                    &self.function_id,
                    &self.arguments,
                    &continuation,
                )
                .map_err(ContinuationError::Envelope)?;
                let envelope =
                    String::from_utf8(envelope).map_err(|_| ContinuationError::TamperedJournal)?;
                let envelope_digest = sha256(envelope.as_bytes());
                self.append(&Record::Yielded {
                    site: site_of(&continuation),
                    envelope,
                    envelope_digest,
                })?;
                self.phase = Phase::AwaitingDispatch(Pending {
                    continuation,
                    envelope_digest,
                });
                Ok(())
            }
            SequentialResumableStep::Completed { result, .. } => {
                self.terminal(DurableOutcome::Completed(result))
            }
            SequentialResumableStep::LanguageFailure(_) => {
                self.terminal(failed(DurableFailure::LanguageFailure))
            }
            SequentialResumableStep::FuelExhausted => {
                self.terminal(failed(DurableFailure::FuelExhausted))
            }
            SequentialResumableStep::CallDepthExceeded => {
                self.terminal(failed(DurableFailure::CallDepthExceeded))
            }
            SequentialResumableStep::GuardError(_) => {
                self.terminal(failed(DurableFailure::EvaluationRejected))
            }
        }
    }

    /// Rebuild the phase from verified records in protocol order. Any other
    /// order is a refusal; nothing is repaired.
    fn replay(mut self, records: &[Record]) -> Result<Self, ContinuationError> {
        enum Tail {
            Started,
            Yielded(Pending),
            Dispatched(Pending),
            Answered(Pending, ArgumentValue),
            Terminal(DurableOutcome),
            CleanupStarted(DurableOutcome),
            Settled(DurableOutcome, CleanupSettlement),
        }
        let yield_count = self.signature.yield_count();
        let mut next_site = 0_u32;
        let mut tail = Tail::Started;
        for record in records {
            tail = match (tail, record) {
                (
                    Tail::Started | Tail::Answered(..),
                    Record::Yielded {
                        site,
                        envelope,
                        envelope_digest,
                    },
                ) => {
                    if *site != next_site
                        || *site >= yield_count
                        || sha256(envelope.as_bytes()) != *envelope_digest
                    {
                        return Err(ContinuationError::TamperedJournal);
                    }
                    let continuation = decode_source_checkpoint_v2(
                        self.program,
                        self.key,
                        &self.scope,
                        &self.function_id,
                        &self.arguments,
                        envelope.as_bytes(),
                    )
                    .map_err(ContinuationError::Envelope)?;
                    if site_of(&continuation) != *site {
                        return Err(ContinuationError::TamperedJournal);
                    }
                    Tail::Yielded(Pending {
                        continuation,
                        envelope_digest: *envelope_digest,
                    })
                }
                (
                    Tail::Yielded(pending),
                    Record::Dispatched {
                        site,
                        envelope_digest,
                    },
                ) if *site == next_site && *envelope_digest == pending.envelope_digest => {
                    Tail::Dispatched(pending)
                }
                (
                    Tail::Dispatched(pending),
                    Record::Answered {
                        site,
                        envelope_digest,
                        answer,
                        answer_digest: digest,
                    },
                ) if *site == next_site
                    && *envelope_digest == pending.envelope_digest
                    && *digest == answer_digest(answer) =>
                {
                    next_site += 1;
                    Tail::Answered(pending, answer.clone())
                }
                (Tail::Started | Tail::Answered(..), Record::Completed { result }) => {
                    Tail::Terminal(DurableOutcome::Completed(result.clone()))
                }
                (
                    prior @ (Tail::Started | Tail::Answered(..) | Tail::Dispatched(_)),
                    Record::Failed { class },
                ) => {
                    let failure = failure_of(class)?;
                    if failure.settles_dispatch() != matches!(prior, Tail::Dispatched(_)) {
                        return Err(ContinuationError::TamperedJournal);
                    }
                    Tail::Terminal(DurableOutcome::Failed(failure))
                }
                (Tail::Terminal(outcome), Record::CleanupStarted) => Tail::CleanupStarted(outcome),
                (Tail::CleanupStarted(outcome), Record::CleanupSettled { settlement }) => {
                    let settlement = CleanupSettlement::ALL
                        .into_iter()
                        .find(|candidate| candidate.class() == settlement)
                        .ok_or(ContinuationError::TamperedJournal)?;
                    Tail::Settled(outcome, settlement)
                }
                _ => return Err(ContinuationError::TamperedJournal),
            };
        }
        match tail {
            Tail::Started => {
                let evaluation = run_sequential_resumable_effect(
                    self.program,
                    &self.function_id,
                    &self.arguments,
                    self.max_steps,
                )
                .map_err(ContinuationError::Admission)?;
                self.settle_step(evaluation.step)?;
            }
            Tail::Yielded(pending) => self.phase = Phase::AwaitingDispatch(pending),
            Tail::Dispatched(pending) => self.phase = Phase::AwaitingAnswer(pending, true),
            Tail::Answered(pending, answer) => self.resume_with(&pending.continuation, &answer)?,
            Tail::Terminal(outcome) => self.phase = Phase::CleanupPending(outcome),
            Tail::CleanupStarted(outcome) => self.phase = Phase::CleanupInDoubt(outcome),
            Tail::Settled(outcome, settlement) => self.phase = Phase::Settled(outcome, settlement),
        }
        Ok(self)
    }
}

fn site_of(continuation: &ResumableContinuation) -> u32 {
    u32::try_from(continuation.history().len()).expect("at most eight sites")
}

fn compare_started(observed: Option<&Record>, expected: &Record) -> Result<(), ContinuationError> {
    let (
        Some(Record::Started {
            function,
            program_digest,
            invocation_id,
            policy_epoch,
            arguments_digest,
            yield_count,
        }),
        Record::Started {
            function: expected_function,
            program_digest: expected_program,
            invocation_id: expected_invocation,
            policy_epoch: expected_epoch,
            arguments_digest: expected_arguments,
            yield_count: expected_count,
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
    Ok(())
}
