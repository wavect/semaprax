//! Public bounded continuation execution for the admitted sequential and
//! control-dependent Copy-scalar profiles (`semaprax.resumable-continuation.v1`).
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
mod lane;
#[cfg(test)]
mod tests;

use super::capability::CapabilityPolicy;
use super::core::{CleanupHandler, EffectHandler};
use super::source_checkpoint::{SourceCheckpointError, SourceCheckpointKey, SourceCheckpointScope};
use super::source_driver::{scalar, tag};
use super::source_signature::{derive_source_effect_signature, SourceEffectSignature};
use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedProgram;
use crate::interpreter::resumable::checkpoint::scalar_json;
use crate::interpreter::{ArgumentValue, MAX_STEPS_LIMIT};
use crate::resumable_effects::lowering::control::MAX_CONTROL_SUSPENSIONS;
use journal::{answer_digest, hex, sha256, Journal, Record};
pub use journal::{JournalDirectory, TornTailPolicy, RESUMABLE_JOURNAL_SCHEMA_V1};
use lane::{Carrier, LaneStep};

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
    BudgetMismatch,
    JournalBusy,
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
    /// A control-dependent invocation reached more suspensions than its bound.
    SuspensionBoundExceeded,
}

impl DurableFailure {
    const ALL: [Self; 8] = [
        Self::LanguageFailure,
        Self::FuelExhausted,
        Self::CallDepthExceeded,
        Self::EvaluationRejected,
        Self::HandlerFailed,
        Self::AnswerTypeMismatch,
        Self::HostAbandoned,
        Self::SuspensionBoundExceeded,
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
            Self::SuspensionBoundExceeded => "suspension_bound_exceeded",
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
    /// A terminal record is durable and cleanup has not started. The result
    /// stays unpublished; at most the failure class is visible.
    CleanupPending { failure: Option<DurableFailure> },
    /// Journaled `CleanupStarted` without `CleanupSettled`.
    CleanupInDoubt { failure: Option<DurableFailure> },
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
    continuation: Carrier,
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
    /// Issue #296, spec section 11.6: the exact carried owned `Bytes` bytes
    /// still awaiting settlement at the moment the sticky terminal outcome
    /// was recorded -- non-empty only for `HandlerFailed`, `AnswerTypeMismatch`,
    /// or `HostAbandoned`, the three failures recorded while a site was
    /// dispatched (`DurableFailure::settles_dispatch`) and so never re-ran
    /// the interpreter to its own natural, in-process drop. A `Completed` or
    /// any other `Failed` outcome only ever follows a resume or start that
    /// ran the interpreter through to that outcome, which already dropped
    /// every carried value itself; this stays empty for those. A caller's
    /// `CleanupHandler` reads this before calling [`Self::settle`] or
    /// [`Self::drive`] to run the carried values' own settlement inside the
    /// existing exactly-once `CleanupStarted`/`CleanupSettled` window.
    pending_cleanup_carried: Vec<Vec<u8>>,
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
    if !signature.is_control_dependent() && signature.yield_count() < 2 {
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
        max_steps: max_steps as u64,
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
        let first = lane::start(
            program,
            function_id,
            arguments,
            max_steps,
            facts.signature.is_control_dependent(),
        )
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
        invocation.settle_step(first)?;
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
            pending_cleanup_carried: Vec::new(),
        }
    }

    /// The exact carried owned `Bytes` bytes still awaiting settlement, in
    /// cleanup-inventory order, valid while [`Self::status`] reports
    /// [`ContinuationStatus::CleanupPending`] or
    /// [`ContinuationStatus::CleanupInDoubt`]. Proof data only: reading it
    /// dispatches nothing. See the field's own doc comment for exactly when
    /// it is non-empty.
    pub fn pending_cleanup_carried(&self) -> &[Vec<u8>] {
        &self.pending_cleanup_carried
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
            Phase::CleanupPending(outcome) => ContinuationStatus::CleanupPending {
                failure: failure_class(outcome),
            },
            Phase::CleanupInDoubt(outcome) => ContinuationStatus::CleanupInDoubt {
                failure: failure_class(outcome),
            },
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
            site: pending.continuation.site(),
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
            site: pending.continuation.site(),
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
                return Err(if answer.site < pending.continuation.site() {
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
        let site = pending.continuation.site();
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
        let carried = self.dispatched_carried();
        self.terminal(
            DurableOutcome::Failed(DurableFailure::HostAbandoned),
            carried,
        )
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
                        Ok(_) => {
                            let carried = self.dispatched_carried();
                            self.terminal(
                                DurableOutcome::Failed(DurableFailure::AnswerTypeMismatch),
                                carried,
                            )?
                        }
                        Err(_) => {
                            let carried = self.dispatched_carried();
                            self.terminal(
                                DurableOutcome::Failed(DurableFailure::HandlerFailed),
                                carried,
                            )?
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

    /// `carried` is the exact carried owned bytes still awaiting settlement,
    /// which the caller -- not `terminal` itself -- knows: a failure recorded
    /// while a site was dispatched (`HandlerFailed`, `AnswerTypeMismatch`,
    /// `HostAbandoned`) passes its `Phase::AwaitingAnswer` pending carrier's
    /// bytes, since the interpreter never re-ran to drop them in-process;
    /// every other caller (a completion or a failure reached by actually
    /// running the resumed suffix, including a request-drift rejection after
    /// an already-appended `Answered` record) passes an empty vector, since
    /// that run already dropped whatever it carried.
    fn terminal(
        &mut self,
        outcome: DurableOutcome,
        carried: Vec<Vec<u8>>,
    ) -> Result<(), ContinuationError> {
        let record = match &outcome {
            DurableOutcome::Completed(result) => Record::Completed {
                result: result.clone(),
            },
            DurableOutcome::Failed(failure) => Record::Failed {
                class: failure.class().to_owned(),
            },
        };
        self.append(&record)?;
        self.pending_cleanup_carried = carried;
        self.phase = Phase::CleanupPending(outcome);
        Ok(())
    }

    /// The exact carried bytes of the current `Phase::AwaitingAnswer`
    /// pending carrier, for a caller settling a dispatched site as a sticky
    /// failure without re-entering the interpreter. Empty if called from any
    /// other phase (defensive; every real caller only calls this from
    /// `AwaitingAnswer`).
    fn dispatched_carried(&self) -> Vec<Vec<u8>> {
        match &self.phase {
            Phase::AwaitingAnswer(pending, _) => pending.continuation.carried_bytes(),
            _ => Vec::new(),
        }
    }

    fn resume_with(
        &mut self,
        continuation: &Carrier,
        answer: &ArgumentValue,
    ) -> Result<(), ContinuationError> {
        match lane::resume(
            self.program,
            &self.function_id,
            &self.arguments,
            continuation,
            answer,
            self.max_steps,
        ) {
            Ok(step) => self.settle_step(step),
            Err(_) => self.terminal(
                DurableOutcome::Failed(DurableFailure::EvaluationRejected),
                Vec::new(),
            ),
        }
    }

    fn settle_step(&mut self, step: LaneStep) -> Result<(), ContinuationError> {
        match step {
            LaneStep::Suspended(continuation) => {
                let envelope = lane::encode(
                    self.program,
                    self.key,
                    &self.scope,
                    &self.function_id,
                    &self.arguments,
                    &continuation,
                )
                .map_err(ContinuationError::Envelope);
                // A prior record is already acknowledged: any failure to
                // journal this step leaves the in-memory state behind storage.
                let Ok(envelope) = envelope.and_then(|bytes| {
                    String::from_utf8(bytes).map_err(|_| ContinuationError::TamperedJournal)
                }) else {
                    self.poisoned = true;
                    return Err(ContinuationError::Poisoned);
                };
                let envelope_digest = sha256(envelope.as_bytes());
                self.append(&Record::Yielded {
                    site: continuation.site(),
                    envelope,
                    envelope_digest,
                })?;
                self.phase = Phase::AwaitingDispatch(Pending {
                    continuation,
                    envelope_digest,
                });
                Ok(())
            }
            LaneStep::Completed(result) => {
                self.terminal(DurableOutcome::Completed(result), Vec::new())
            }
            LaneStep::Failed(failure) => self.terminal(DurableOutcome::Failed(failure), Vec::new()),
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
            /// The second field is the exact carried owned `Bytes` bytes
            /// still awaiting settlement -- see
            /// `DurableInvocation::pending_cleanup_carried`'s own doc
            /// comment for exactly when it is non-empty.
            Terminal(DurableOutcome, Vec<Vec<u8>>),
            CleanupStarted(DurableOutcome, Vec<Vec<u8>>),
            Settled(DurableOutcome, CleanupSettlement),
        }
        let control_dependent = self.signature.is_control_dependent();
        let site_bound = if control_dependent {
            MAX_CONTROL_SUSPENSIONS as u32
        } else {
            self.signature.yield_count()
        };
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
                        || *site >= site_bound
                        || sha256(envelope.as_bytes()) != *envelope_digest
                    {
                        return Err(ContinuationError::TamperedJournal);
                    }
                    let continuation = lane::decode(
                        self.program,
                        self.key,
                        &self.scope,
                        &self.function_id,
                        &self.arguments,
                        envelope.as_bytes(),
                        control_dependent,
                        self.signature.carries_owned_bytes(),
                    )
                    .map_err(ContinuationError::Envelope)?;
                    if continuation.site() != *site {
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
                    Tail::Terminal(DurableOutcome::Completed(result.clone()), Vec::new())
                }
                (
                    prior @ (Tail::Started | Tail::Answered(..) | Tail::Dispatched(_)),
                    Record::Failed { class },
                ) => {
                    let failure = failure_of(class)?;
                    if failure.settles_dispatch() != matches!(prior, Tail::Dispatched(_)) {
                        return Err(ContinuationError::TamperedJournal);
                    }
                    let carried = match &prior {
                        Tail::Dispatched(pending) => pending.continuation.carried_bytes(),
                        _ => Vec::new(),
                    };
                    Tail::Terminal(DurableOutcome::Failed(failure), carried)
                }
                (Tail::Terminal(outcome, carried), Record::CleanupStarted) => {
                    Tail::CleanupStarted(outcome, carried)
                }
                (Tail::CleanupStarted(outcome, carried), Record::CleanupSettled { settlement }) => {
                    let settlement = CleanupSettlement::ALL
                        .into_iter()
                        .find(|candidate| candidate.class() == settlement)
                        .ok_or(ContinuationError::TamperedJournal)?;
                    let _ = carried;
                    Tail::Settled(outcome, settlement)
                }
                _ => return Err(ContinuationError::TamperedJournal),
            };
        }
        match tail {
            Tail::Started => {
                let step = lane::start(
                    self.program,
                    &self.function_id,
                    &self.arguments,
                    self.max_steps,
                    control_dependent,
                )
                .map_err(ContinuationError::Admission)?;
                self.settle_step(step)?;
            }
            Tail::Yielded(pending) => self.phase = Phase::AwaitingDispatch(pending),
            Tail::Dispatched(pending) => self.phase = Phase::AwaitingAnswer(pending, true),
            Tail::Answered(pending, answer) => self.resume_with(&pending.continuation, &answer)?,
            Tail::Terminal(outcome, carried) => {
                self.pending_cleanup_carried = carried;
                self.phase = Phase::CleanupPending(outcome);
            }
            Tail::CleanupStarted(outcome, carried) => {
                self.pending_cleanup_carried = carried;
                self.phase = Phase::CleanupInDoubt(outcome);
            }
            Tail::Settled(outcome, settlement) => self.phase = Phase::Settled(outcome, settlement),
        }
        Ok(self)
    }
}

fn failure_class(outcome: &DurableOutcome) -> Option<DurableFailure> {
    match outcome {
        DurableOutcome::Completed(_) => None,
        DurableOutcome::Failed(failure) => Some(*failure),
    }
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
            max_steps,
        }),
        Record::Started {
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
