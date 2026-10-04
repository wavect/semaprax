//! Private stage/effect driver. Hooks cannot mint authorization or replace checked values.
use super::*;

mod live;

pub(crate) enum DriverFailure {
    Diagnostics(Vec<Diagnostic>),
    Persistence {
        terminal: Box<IterativeRun>,
        diagnostics: Vec<Diagnostic>,
    },
}
impl From<Vec<Diagnostic>> for DriverFailure {
    fn from(diagnostics: Vec<Diagnostic>) -> Self {
        Self::Diagnostics(diagnostics)
    }
}
impl DriverFailure {
    pub(crate) fn into_diagnostics(self) -> Vec<Diagnostic> {
        match self {
            Self::Diagnostics(errors)
            | Self::Persistence {
                diagnostics: errors,
                ..
            } => errors,
        }
    }
}
impl std::fmt::Debug for DriverFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Diagnostics(errors) => f.debug_tuple("Diagnostics").field(errors).finish(),
            Self::Persistence {
                terminal,
                diagnostics,
            } => f
                .debug_struct("Persistence")
                .field("terminal_status", &terminal.status())
                .field("diagnostics", diagnostics)
                .finish(),
        }
    }
}

pub(crate) struct EffectContext<'a> {
    pub(crate) turn: usize,
    pub(crate) policy: &'a str,
    pub(crate) state: &'a RetainedValue,
    pub(crate) proposal_canonical: &'a str,
    pub(crate) authorization: &'a AuthorizedRequest,
}

/// Exact byte and call accounting held by a typed effect dispatcher. This is
/// descriptive only: obtaining it cannot replay or authorize an effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EffectAccounting {
    pub max_calls: usize,
    pub max_argument_bytes: usize,
    pub max_result_bytes: usize,
    pub max_total_bytes: usize,
    pub dispatched_calls: u32,
    pub argument_bytes: usize,
    pub result_bytes: usize,
    pub replayed_calls: u32,
    pub replayed_argument_bytes: usize,
    pub replayed_result_bytes: usize,
    pub failure: Option<&'static str>,
}

/// The one point where an iterative executor may elect to spend the freshly
/// minted authorization on a more constrained target boundary.  The kernel
/// owns every value here: a driver can neither substitute a model proposal nor
/// choose a different invocation root.
pub(crate) struct TargetEffectContext<'a> {
    pub(crate) invocation_root: &'a str,
    pub(crate) turn: usize,
    pub(crate) max_steps: usize,
    pub(crate) proposal_canonical: &'a str,
    pub(crate) projected: &'a [RetainedValue],
    pub(crate) cancellation: &'a AgentCancellation,
}

/// A target driver either returns the still-unspent authorization to the
/// ordinary effect route, or consumes it at its own bounded protocol edge.
/// This keeps the default driver behavior byte-for-byte unchanged while
/// making a target route prove that it received the real move-only value.
pub(crate) enum TargetEffect {
    Ordinary(Authorized),
    Dispatched {
        dispatch: authorization::target_protocol::TargetDispatch,
        result: Option<Vec<u8>>,
    },
}

/// One owner coordinates stage reservations, effect persistence and transitions.
/// Reservations run before every stage, including the new tail after replay.
/// A driver failure propagates before subsequent stage or host execution.
pub(crate) trait IterativeDriver {
    fn before_stage(
        &mut self,
        _role: &'static str,
        _turn: usize,
        _max_steps: usize,
    ) -> Result<(), Vec<Diagnostic>> {
        Ok(())
    }
    fn before_effect(&mut self, _context: EffectContext<'_>) -> Result<(), Vec<Diagnostic>> {
        Ok(())
    }
    /// Target execution is opt-in.  The default returns the original moved
    /// authorization unchanged, so ordinary AgentRead and typed-effect routes
    /// retain their existing `AuthorizedRequest` boundary.
    fn target_effect(
        &mut self,
        _context: TargetEffectContext<'_>,
        authorization: Authorized,
    ) -> Result<TargetEffect, Vec<Diagnostic>> {
        Ok(TargetEffect::Ordinary(authorization))
    }
    /// A checked dispatcher may bind its own operation and exact request to
    /// the existing source journal. None preserves the AgentRead v2 identity.
    fn source_effect_identity(
        &mut self,
        _authorization: &AuthorizedRequest,
    ) -> Result<Option<(String, String)>, Vec<Diagnostic>> {
        Ok(None)
    }
    /// Revalidate an acknowledged effect without redispatch. Typed drivers
    /// also restore their cumulative admission accounting at this boundary.
    fn validate_replayed_read(
        &mut self,
        _authorization: &AuthorizedRequest,
        _observation: Option<&[u8]>,
    ) -> Result<(), Vec<Diagnostic>> {
        Ok(())
    }
    /// Typed dispatchers expose their already charged accounting for a
    /// retained terminal receipt. Ordinary read drivers have no such ledger.
    fn effect_accounting(&self) -> Option<EffectAccounting> {
        None
    }
    fn read(
        &mut self,
        authorization: &AuthorizedRequest,
    ) -> Result<Option<Vec<u8>>, Vec<Diagnostic>>;
    fn after_transition(
        &mut self,
        _turn: usize,
        _kind: &str,
        _value: &RetainedValue,
    ) -> Result<(), Vec<Diagnostic>> {
        Ok(())
    }
}

pub(super) struct ReadDriver<'a> {
    pub(super) read: &'a mut dyn AgentReadOperation,
}
impl IterativeDriver for ReadDriver<'_> {
    fn read(
        &mut self,
        authorization: &AuthorizedRequest,
    ) -> Result<Option<Vec<u8>>, Vec<Diagnostic>> {
        Ok(self.read.read(authorization))
    }
}

/// Everything the live route has actually checked before it asks for the
/// current turn's proposal: the exact observation and state this turn holds,
/// the real previous effect result (not the initial task context replayed on
/// every turn), and the identities/limits the source must stay inside.
///
/// The driver builds this and chooses when to build it; nothing in it is
/// derived from a prior model response, so a source cannot manufacture an
/// extra request by anything it returns.
pub struct ProposalRequest<'a> {
    pub turn: usize,
    pub attempt: usize,
    pub source_revision: &'a str,
    pub proposal_schema_digest: &'a str,
    pub task: &'a LifecycleTask,
    pub state: &'a RetainedValue,
    pub observation: &'a RetainedValue,
    /// The bytes the injected read operation returned last turn, `None` on
    /// the first turn. This is the actual prior effect result, never a
    /// precomputed or replayed value.
    pub previous_effect: Option<&'a [u8]>,
    /// Set when this is a retry after `attempt - 1` produced a proposal this
    /// grammar refused to decode; `None` on a turn's first attempt.
    pub previous_rejection: Option<&'a str>,
    pub remaining_iterations: usize,
}

/// A live, feedback-driven proposal source. The frozen route reads a
/// predeclared `&[String]` slice by turn index; this trait is the live
/// route's equivalent, called by the driver only after the current turn's
/// checked observation exists.
///
/// An `Err` ends the run's decode/retry loop immediately with those
/// diagnostics; it does not retry. A source that wants a bounded retry
/// returns `Ok` with proposal text the schema will reject, and reads the
/// rejection back on `request.previous_rejection` next attempt.
pub trait ProposalSource {
    /// Checks the source route's shared host deadline without reserving work.
    /// Fixture sources accept by default; an explicit host source delegates to
    /// its bound accounting policy.
    fn check_deadline(&self) -> Result<(), Vec<Diagnostic>> {
        Ok(())
    }

    /// Explicit capability declaration for the checkpointed source route.
    /// Ordinary sources never fall back to unjournaled execution there.
    fn checkpoint_policy(&self) -> Option<super::source_live::SourceProposalPolicy<'_>> {
        None
    }

    fn checkpoint_attempt_identity(
        &self,
        _request: &ProposalRequest<'_>,
    ) -> Result<super::source_live::SourceAttemptIdentity, Vec<Diagnostic>> {
        Err(vec![bad("source.checkpoint_unsupported")])
    }

    /// Optional first phase of the V6 durable policy route. It may quote the
    /// exact canonical request but must not reserve, append, or dispatch.
    fn policy_quote_checkpointed(
        &mut self,
        _request: &ProposalRequest<'_>,
        _identity: &super::source_live::SourceAttemptIdentity,
    ) -> Result<Option<crate::live_invocation::source_journal::SourcePolicyQuoteV6>, Vec<Diagnostic>>
    {
        Ok(None)
    }

    /// Optional second V6 phase. The source must reserve exactly `intent`,
    /// acknowledge it through `sink`, and only then approach its provider.
    fn propose_policy_checkpointed(
        &mut self,
        _request: ProposalRequest<'_>,
        _quote: &crate::live_invocation::source_journal::SourcePolicyQuoteV6,
        _intent: &crate::live_invocation::source_journal::PolicyAttemptIntentV6,
        _sink: &mut crate::live_invocation::source_journal::SourceCheckpointSink<'_>,
        _ledger: &mut crate::live_invocation::CumulativeBudgetLedger<'_>,
        _clock: &dyn crate::live_invocation::SourceInvocationClock,
    ) -> super::source_live::SourceProposalOutcome {
        super::source_live::SourceProposalOutcome {
            terminal_failure: None,
            result: Err(vec![bad("source.policy_checkpoint_unsupported")]),
            model_dispatches: 0,
        }
    }

    /// Restores already independently validated V6 reservations before a
    /// resumed source can continue. Implementations must not treat this as a
    /// fresh policy ledger.
    fn restore_policy_checkpointed(
        &mut self,
        _reservations: &[crate::live_invocation::source_journal::PolicyAttemptReservationV6],
    ) -> Result<(), Vec<Diagnostic>> {
        Ok(())
    }

    fn propose_checkpointed(
        &mut self,
        _request: ProposalRequest<'_>,
        _sink: &mut crate::live_invocation::source_journal::SourceCheckpointSink<'_>,
        _ledger: &mut crate::live_invocation::CumulativeBudgetLedger<'_>,
        _clock: &dyn crate::live_invocation::SourceInvocationClock,
    ) -> super::source_live::SourceProposalOutcome {
        super::source_live::SourceProposalOutcome {
            terminal_failure: None,
            result: Err(vec![bad("source.checkpoint_unsupported")]),
            model_dispatches: 0,
        }
    }

    fn propose(&mut self, request: ProposalRequest<'_>) -> Result<String, Vec<Diagnostic>>;
}

/// Attempts per turn a malformed proposal may consume before the run ends as
/// [`IterativeStatus::ModelFailed`]. Bounded so a source that never produces
/// a decodable proposal cannot spin the loop unboundedly; each attempt is
/// counted and produces no effect call.
pub const MAX_PROPOSAL_ATTEMPTS: usize = 4;

impl CompiledIterativeLifecycle {
    pub(crate) fn run_with_driver(
        &self,
        task: &LifecycleTask,
        proposals: &[String],
        driver: &mut dyn IterativeDriver,
        budget: IterativeBudget,
        cancellation: &AgentCancellation,
    ) -> Result<IterativeRun, DriverFailure> {
        self.run_with_driver_initial(
            task,
            proposals,
            driver,
            budget,
            cancellation,
            None,
            authorization::StageBackend::Interpreter,
        )
    }

    pub(crate) fn run_with_driver_seed(
        &self,
        task: &LifecycleTask,
        proposals: &[String],
        driver: &mut dyn IterativeDriver,
        budget: IterativeBudget,
        cancellation: &AgentCancellation,
        seed: &crate::execution_revision::typed::migration::MigrationSeed,
    ) -> Result<IterativeRun, DriverFailure> {
        self.run_with_driver_initial(
            task,
            proposals,
            driver,
            budget,
            cancellation,
            Some(seed),
            authorization::StageBackend::Interpreter,
        )
    }

    /// Run through one explicitly selected, held stage backend. The caller
    /// still owns all host capabilities and the ordinary driver owns replay.
    pub(in crate::agent_lifecycle) fn run_with_driver_on(
        &self,
        task: &LifecycleTask,
        proposals: &[String],
        driver: &mut dyn IterativeDriver,
        budget: IterativeBudget,
        cancellation: &AgentCancellation,
        backend: authorization::StageBackend<'_>,
    ) -> Result<IterativeRun, DriverFailure> {
        self.run_with_driver_initial(task, proposals, driver, budget, cancellation, None, backend)
    }

    /// Migration-seeded counterpart of [`Self::run_with_driver_on`].
    #[allow(clippy::too_many_arguments)]
    pub(in crate::agent_lifecycle) fn run_with_driver_seed_on(
        &self,
        task: &LifecycleTask,
        proposals: &[String],
        driver: &mut dyn IterativeDriver,
        budget: IterativeBudget,
        cancellation: &AgentCancellation,
        seed: &crate::execution_revision::typed::migration::MigrationSeed,
        backend: authorization::StageBackend<'_>,
    ) -> Result<IterativeRun, DriverFailure> {
        self.run_with_driver_initial(
            task,
            proposals,
            driver,
            budget,
            cancellation,
            Some(seed),
            backend,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn run_with_driver_initial(
        &self,
        task: &LifecycleTask,
        proposals: &[String],
        driver: &mut dyn IterativeDriver,
        budget: IterativeBudget,
        cancellation: &AgentCancellation,
        seed: Option<&crate::execution_revision::typed::migration::MigrationSeed>,
        backend: authorization::StageBackend<'_>,
    ) -> Result<IterativeRun, DriverFailure> {
        if budget.max_iterations > 4096 || budget.max_stages > 12289 {
            return Err(vec![bad("budget.capacity")].into());
        }
        let inner = &self.inner;
        let mut run = IterativeRun {
            status: IterativeStatus::BudgetExhausted,
            iterations: 0,
            stages: Vec::new(),
            effects: 0,
            value: None,
            authorization_bindings: Vec::new(),
            invocation_digest: match seed {
                None => invocation_digest(task, proposals, budget),
                Some(seed) => digest(
                    b"semaprax.agent-migrated-invocation.v1\0",
                    format!(
                        "{}\0{}\0{}",
                        invocation_digest(task, proposals, budget),
                        seed.binding_digest(),
                        encode_value(seed.value())
                    )
                    .as_bytes(),
                ),
            },
            evidence: String::new(),
            digest: String::new(),
        };
        macro_rules! stop {
            ($status:expr, $value:expr) => {
                return Ok(run.finish($status, $value, self.digest()))
            };
        }
        macro_rules! boundary {
            () => {
                if cancellation.is_cancelled() {
                    stop!(IterativeStatus::Cancelled, None);
                }
                if run.stages.len() >= budget.max_stages {
                    stop!(IterativeStatus::BudgetExhausted, None);
                }
            };
        }
        macro_rules! evaluate {
            ($stage:expr, $arguments:expr) => {{
                boundary!();
                driver.before_stage($stage.role(), run.iterations, budget.max_steps_per_stage)?;
                let evaluation = match authorization::dispatch_on_cancellable(
                    backend,
                    &inner.program,
                    $stage.prepared(),
                    $arguments,
                    budget.max_steps_per_stage,
                    cancellation,
                ) {
                    Ok(evaluation) => evaluation,
                    Err(_) if cancellation.is_cancelled() => {
                        stop!(IterativeStatus::Cancelled, None);
                    }
                    Err(errors) => return Err(errors.into()),
                };
                run.stages.push(StageRecord::of($stage, &evaluation));
                match evaluation.outcome {
                    RetainedCallOutcome::Returned(value) => value,
                    RetainedCallOutcome::FuelExhausted | RetainedCallOutcome::CallDepthExceeded => {
                        stop!(IterativeStatus::BudgetExhausted, None);
                    }
                    _ => {
                        stop!(IterativeStatus::Rejected, None);
                    }
                }
            }};
        }
        if budget.max_iterations == 0 {
            stop!(IterativeStatus::BudgetExhausted, None);
        }
        let mut state = match seed {
            Some(seed) => seed.value().clone(),
            None => evaluate!(
                &inner.binding.initialize,
                &[payload(
                    &inner.binding.task,
                    task.objective.clone(),
                    task.budget
                )]
            ),
        };
        if !inner.carries(&state, "state") {
            return Err(vec![bad("initialize.identity")].into());
        }
        loop {
            if run.iterations >= budget.max_iterations {
                stop!(IterativeStatus::BudgetExhausted, None);
            }
            let observation = evaluate!(&inner.binding.observe, std::slice::from_ref(&state));
            if !inner.carries(&observation, "observation") {
                return Err(vec![bad("observe.identity")].into());
            }
            let Some(proposal_source) = proposals.get(run.iterations) else {
                stop!(IterativeStatus::ModelFailed, None);
            };
            let Ok(decoded) = inner.proposal.decode(proposal_source) else {
                stop!(IterativeStatus::ModelFailed, None);
            };
            let Some(projected) = inner.project(&decoded) else {
                stop!(IterativeStatus::ModelFailed, None);
            };
            boundary!();
            let policy = digest(
                b"semaprax.agent-iteration-policy.v2\0",
                format!("{}\0{}", self.digest(), run.iterations).as_bytes(),
            );
            let policy = match seed {
                None => policy,
                Some(seed) => digest(
                    b"semaprax.agent-migrated-iteration-policy.v1\0",
                    format!("{}\0{}", policy, seed.binding_digest()).as_bytes(),
                ),
            };
            let mut args = vec![state.clone()];
            args.extend(projected.iter().cloned());
            driver.before_stage("authorize", run.iterations, budget.max_steps_per_stage)?;
            let authorized_run = authorization::run_authorize_stage_on_cancellable(
                backend,
                &inner.program,
                &inner.binding.authorize,
                &args,
                budget.max_steps_per_stage,
                &policy,
                &state,
                decoded.canonical_json(),
                Some(cancellation),
            );
            let (decision, record) = match authorized_run {
                Ok(outcome) => outcome,
                Err(_) if cancellation.is_cancelled() => {
                    stop!(IterativeStatus::Cancelled, None);
                }
                Err(errors) => return Err(errors.into()),
            };
            run.stages.push(record);
            let authorized = match decision {
                authorization::AuthorizationOutcome::Granted(value) => value,
                authorization::AuthorizationOutcome::Refused(_) => {
                    stop!(IterativeStatus::Rejected, None);
                }
                authorization::AuthorizationOutcome::Undecided("fuel" | "depth") => {
                    stop!(IterativeStatus::BudgetExhausted, None);
                }
                _ => {
                    stop!(IterativeStatus::Rejected, None);
                }
            };
            if cancellation.is_cancelled() {
                stop!(IterativeStatus::Cancelled, None);
            }
            // Reserve reducer capacity before dispatch: no known-doomed effect.
            if run.stages.len() >= budget.max_stages {
                stop!(IterativeStatus::BudgetExhausted, None);
            }
            let expected = authorization::binding(
                &policy,
                &state,
                decoded.canonical_json(),
                inner.binding.authorize.grant_case(),
                authorized.seal(),
            );
            let authorization_binding = authorized.binding().to_owned();
            if expected != authorization_binding {
                return Err(vec![bad("authorization.binding")].into());
            }
            let read = match driver.target_effect(
                TargetEffectContext {
                    invocation_root: &run.invocation_digest,
                    turn: run.iterations,
                    max_steps: budget.max_steps_per_stage,
                    proposal_canonical: decoded.canonical_json(),
                    projected: &projected,
                    cancellation,
                },
                authorized,
            )? {
                TargetEffect::Ordinary(authorized) => {
                    let request = authorized.consume();
                    driver.before_effect(EffectContext {
                        turn: run.iterations,
                        policy: &policy,
                        state: &state,
                        proposal_canonical: decoded.canonical_json(),
                        authorization: &request,
                    })?;
                    run.authorization_bindings
                        .push(request.binding().to_owned());
                    run.effects += 1;
                    driver.read(&request)?
                }
                TargetEffect::Dispatched { dispatch, result } => {
                    if dispatch.authorization_binding() != authorization_binding {
                        return Err(vec![bad("target.authorization.binding")].into());
                    }
                    if result.as_deref()
                        != dispatch
                            .result()
                            .map(authorization::target_protocol::TypedCarrier::payload)
                        && result.is_some()
                    {
                        return Err(vec![bad("target.result.binding")].into());
                    }
                    run.authorization_bindings.push(authorization_binding);
                    run.effects += 1;
                    result
                }
            };
            let Some(bytes) = read else {
                stop!(IterativeStatus::EffectFailed, None);
            };
            if bytes.len() > MAX_READ_BYTES {
                stop!(IterativeStatus::EffectFailed, None);
            }
            let mut args = vec![state];
            args.extend(projected);
            args.push(payload(&inner.binding.outcome, bytes, 0));
            let value = evaluate!(&inner.binding.reduce, &args);
            run.iterations += 1;
            let (transition, value) = self.step.decode(value)?;
            if let Err(diagnostics) =
                driver.after_transition(run.iterations - 1, transition, &value)
            {
                let selected = match transition {
                    "Complete" => Some(IterativeStatus::Complete),
                    "Suspend" => Some(IterativeStatus::Suspend),
                    "Fail" => Some(IterativeStatus::Fail),
                    _ => None,
                };
                return Err(if let Some(status) = selected {
                    DriverFailure::Persistence {
                        terminal: Box::new(run.finish(status, Some(value), self.digest())),
                        diagnostics,
                    }
                } else {
                    diagnostics.into()
                });
            }
            match transition {
                "Continue" => state = value,
                "Complete" => stop!(IterativeStatus::Complete, Some(value)),
                "Suspend" => stop!(IterativeStatus::Suspend, Some(value)),
                "Fail" => stop!(IterativeStatus::Fail, Some(value)),
                _ => return Err(vec![bad("step.transition")].into()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Driver {
        events: Vec<String>,
        calls: usize,
        target_calls: usize,
        reject_stage: Option<&'static str>,
        reject_transition: Option<usize>,
        advance_stage: Option<(&'static str, std::rc::Rc<std::cell::Cell<i64>>)>,
        advance_effect: Option<std::rc::Rc<std::cell::Cell<i64>>>,
        advance_transition: Option<std::rc::Rc<std::cell::Cell<i64>>>,
        cancel_transition: Option<AgentCancellation>,
    }
    impl IterativeDriver for Driver {
        fn before_stage(
            &mut self,
            role: &'static str,
            turn: usize,
            allowance: usize,
        ) -> Result<(), Vec<Diagnostic>> {
            assert_eq!(allowance, DEFAULT_STAGE_STEPS);
            self.events.push(format!("{turn}:{role}"));
            if let Some((advance_role, clock)) = &self.advance_stage {
                if *advance_role == role {
                    clock.set(10);
                }
            }
            if self.reject_stage == Some(role) {
                return Err(vec![bad("fixture.reservation_refused")]);
            }
            Ok(())
        }
        fn before_effect(&mut self, context: EffectContext<'_>) -> Result<(), Vec<Diagnostic>> {
            assert_eq!(context.turn, self.calls);
            assert!(context.policy.starts_with("sha256:"));
            assert!(context.authorization.binding().starts_with("sha256:"));
            assert!(!context.proposal_canonical.is_empty());
            assert!(matches!(context.state, RetainedValue::Record(_)));
            self.events.push(format!("{}:intent_context", context.turn));
            if let Some(clock) = &self.advance_effect {
                clock.set(10);
            }
            Ok(())
        }
        fn target_effect(
            &mut self,
            _: TargetEffectContext<'_>,
            authorization: Authorized,
        ) -> Result<TargetEffect, Vec<Diagnostic>> {
            self.target_calls += 1;
            Ok(TargetEffect::Ordinary(authorization))
        }
        fn read(&mut self, _: &AuthorizedRequest) -> Result<Option<Vec<u8>>, Vec<Diagnostic>> {
            self.events.push(format!("{}:read", self.calls));
            self.calls += 1;
            Ok(Some(Vec::new()))
        }
        fn after_transition(
            &mut self,
            turn: usize,
            kind: &str,
            _: &RetainedValue,
        ) -> Result<(), Vec<Diagnostic>> {
            self.events.push(format!("{turn}:{kind}"));
            if let Some(cancellation) = &self.cancel_transition {
                cancellation.cancel();
            }
            if let Some(clock) = &self.advance_transition {
                clock.set(10);
            }
            if self.reject_transition == Some(turn) {
                return Err(vec![bad("fixture.transition_not_durable")]);
            }
            Ok(())
        }
    }
    fn make_driver() -> Driver {
        Driver {
            events: Vec::new(),
            calls: 0,
            target_calls: 0,
            reject_stage: None,
            reject_transition: None,
            advance_stage: None,
            advance_effect: None,
            advance_transition: None,
            cancel_transition: None,
        }
    }
    #[test]
    fn driver_reserves_every_stage_and_preserves_ordinary_evidence() {
        let source = super::super::tests::source("Step::Fail { code: 7 }");
        let compiled = compile_agent_lifecycle_v2(
            &source,
            "driver.spx",
            &crate::agent_lifecycle::tests::DEFINITION
                .replace("RUNTIME", crate::agent_lifecycle::tests::RUNTIME_V1),
            "fixture.agent.type.step",
        )
        .unwrap();
        let proposals = vec![crate::agent_lifecycle::tests::proposal(&compiled.inner, "1", "1"); 3];
        let task = LifecycleTask {
            objective: b"task".to_vec(),
            budget: 10,
        };
        let budget = IterativeBudget::default();
        let cancellation = AgentCancellation::new();
        let mut ordinary = FixtureRead::new(Vec::new());
        let expected = compiled
            .run(&task, &proposals, &mut ordinary, budget, &cancellation)
            .unwrap();
        let mut driver = make_driver();
        let actual = compiled
            .run_with_driver(&task, &proposals, &mut driver, budget, &cancellation)
            .unwrap();
        assert_eq!(actual.evidence(), expected.evidence());
        assert_eq!(driver.calls, 3);
        assert_eq!(
            driver.events,
            [
                "0:initialize",
                "0:observe",
                "0:authorize",
                "0:intent_context",
                "0:read",
                "0:reduce",
                "0:Continue",
                "1:observe",
                "1:authorize",
                "1:intent_context",
                "1:read",
                "1:reduce",
                "1:Continue",
                "2:observe",
                "2:authorize",
                "2:intent_context",
                "2:read",
                "2:reduce",
                "2:Fail"
            ]
        );
        for role in ["initialize", "observe", "authorize", "reduce"] {
            let mut blocked = Driver {
                reject_stage: Some(role),
                ..make_driver()
            };
            assert!(compiled
                .run_with_driver(&task, &proposals, &mut blocked, budget, &cancellation)
                .is_err());
            assert_eq!(blocked.calls, usize::from(role == "reduce"));
        }
        let mut blocked = Driver {
            reject_transition: Some(0),
            ..make_driver()
        };
        assert!(compiled
            .run_with_driver(&task, &proposals, &mut blocked, budget, &cancellation)
            .is_err());
        assert_eq!(blocked.calls, 1);
        let mut terminal_failure = Driver {
            reject_transition: Some(2),
            ..make_driver()
        };
        match compiled
            .run_with_driver(
                &task,
                &proposals,
                &mut terminal_failure,
                budget,
                &cancellation,
            )
            .err()
            .unwrap()
        {
            DriverFailure::Persistence {
                terminal,
                diagnostics,
            } => {
                assert_eq!(terminal.status(), IterativeStatus::Fail);
                assert_eq!(terminal.evidence(), expected.evidence());
                assert!(!diagnostics.is_empty());
            }
            _ => panic!("selected terminal must survive persistence failure"),
        }
        assert_eq!(terminal_failure.calls, 3);
    }

    // --- Live, feedback-driven proposal source (SPX-AI-012) ---

    /// Always proposes the same granted document, ignoring any feedback;
    /// used only where a test needs a route to reach a terminal state and
    /// does not itself assert on feedback dependence.
    struct AlwaysGrant<'a> {
        compiled: &'a CompiledAgentLifecycle,
    }
    impl ProposalSource for AlwaysGrant<'_> {
        fn propose(&mut self, _: ProposalRequest<'_>) -> Result<String, Vec<Diagnostic>> {
            Ok(crate::agent_lifecycle::tests::proposal(
                self.compiled,
                "1",
                "1",
            ))
        }
    }

    /// Source-local view of the shared absolute deadline. Driver callbacks
    /// mutate this clock to prove post-callback guards prevent later work.
    struct DeadlineProbe<'a> {
        compiled: &'a CompiledAgentLifecycle,
        clock: std::rc::Rc<std::cell::Cell<i64>>,
    }
    impl ProposalSource for DeadlineProbe<'_> {
        fn check_deadline(&self) -> Result<(), Vec<Diagnostic>> {
            if self.clock.get() >= 10 {
                return Err(vec![bad("fixture.deadline_exceeded")]);
            }
            Ok(())
        }

        fn propose(&mut self, _: ProposalRequest<'_>) -> Result<String, Vec<Diagnostic>> {
            Ok(crate::agent_lifecycle::tests::proposal(
                self.compiled,
                "1",
                "1",
            ))
        }
    }

    /// Replays one fixed sequence of proposal documents by turn index, the
    /// same way the frozen `&[String]` route indexes its slice. Used to prove
    /// the two routes share one kernel: fed the same effective sequence, they
    /// must produce byte-identical evidence.
    struct Replay<'a> {
        proposals: &'a [String],
    }
    impl ProposalSource for Replay<'_> {
        fn propose(&mut self, request: ProposalRequest<'_>) -> Result<String, Vec<Diagnostic>> {
            self.proposals
                .get(request.turn)
                .cloned()
                .ok_or_else(|| vec![bad("fixture.replay_exhausted")])
        }
    }

    /// Proves the live route is feedback-driven, not merely iteration-driven.
    /// Turn 0 has no prior effect and always proposes the same document.
    /// Every later turn REJECTS a request that lacks a prior effect result
    /// (the issue's required negative check on missing observation), and
    /// only proposes the budget that gets granted when the prior effect
    /// returned the expected marker; otherwise it proposes a budget that the
    /// checked authorize stage refuses. A driver that ignored feedback (never
    /// threaded the real read result through) would either hit the missing-
    /// observation rejection or grant/refuse identically regardless of what
    /// the effect actually returned, so the two runs below could not diverge.
    struct Feedback<'a> {
        compiled: &'a CompiledAgentLifecycle,
        unlock: &'static [u8],
    }
    impl ProposalSource for Feedback<'_> {
        fn propose(&mut self, request: ProposalRequest<'_>) -> Result<String, Vec<Diagnostic>> {
            assert!(!request.source_revision.is_empty());
            assert!(request.proposal_schema_digest.starts_with("sha256:"));
            assert_eq!(request.task.budget, 10);
            assert!(matches!(request.state, RetainedValue::Record(_)));
            assert!(matches!(request.observation, RetainedValue::Record(_)));
            assert_eq!(request.attempt, 0);
            assert_eq!(
                request.remaining_iterations,
                IterativeBudget::default().max_iterations - request.turn
            );
            if request.turn == 0 {
                assert!(request.previous_effect.is_none());
                return Ok(crate::agent_lifecycle::tests::proposal(
                    self.compiled,
                    "5",
                    "1",
                ));
            }
            let Some(previous) = request.previous_effect else {
                return Err(vec![bad("fixture.missing_prior_effect")]);
            };
            let budget = if previous == self.unlock { "5" } else { "999" };
            Ok(crate::agent_lifecycle::tests::proposal(
                self.compiled,
                budget,
                "1",
            ))
        }
    }

    /// Proposes malformed text `fail_attempts` times, then a granted document.
    struct MalformedThenValid<'a> {
        compiled: &'a CompiledAgentLifecycle,
        fail_attempts: usize,
        calls: usize,
    }
    impl ProposalSource for MalformedThenValid<'_> {
        fn propose(&mut self, request: ProposalRequest<'_>) -> Result<String, Vec<Diagnostic>> {
            self.calls += 1;
            assert_eq!(
                request.previous_rejection.is_some(),
                request.attempt > 0,
                "a rejection reason must appear starting the attempt after it, and not before"
            );
            if request.attempt < self.fail_attempts {
                return Ok("not a proposal document\n".to_owned());
            }
            Ok(crate::agent_lifecycle::tests::proposal(
                self.compiled,
                "1",
                "1",
            ))
        }
    }

    fn live_task() -> LifecycleTask {
        LifecycleTask {
            objective: b"task".to_vec(),
            budget: 10,
        }
    }

    #[test]
    fn live_second_proposal_depends_on_the_actual_first_effect_result() {
        let compiled = compile_agent_lifecycle_v2(
            &super::super::tests::source(
                "Step::Complete { summary: state.objective, budget: state.budget, status: state.epoch }",
            ),
            "driver-live.spx",
            &crate::agent_lifecycle::tests::DEFINITION
                .replace("RUNTIME", crate::agent_lifecycle::tests::RUNTIME_V1),
            "fixture.agent.type.step",
        )
        .unwrap();
        let task = live_task();
        let budget = IterativeBudget::default();
        let cancellation = AgentCancellation::new();

        // The first effect reveals the unlock marker: authorize keeps
        // granting and the run reaches Complete after three turns.
        let mut unlocked = FixtureRead::new(b"unlock".to_vec());
        let mut source = Feedback {
            compiled: &compiled.inner,
            unlock: b"unlock",
        };
        let unlocked_run = compiled
            .run_live(&task, &mut source, &mut unlocked, budget, &cancellation)
            .unwrap();
        assert_eq!(unlocked_run.status(), IterativeStatus::Complete);
        assert_eq!(unlocked.calls(), 3);

        // The same source, told the first effect returned something else,
        // proposes a budget the checked authorize stage refuses: the run
        // stops Rejected after exactly one effect, and never repeats the
        // refused request no matter how many turns remain.
        let mut locked = FixtureRead::new(b"locked".to_vec());
        let mut source = Feedback {
            compiled: &compiled.inner,
            unlock: b"unlock",
        };
        let locked_run = compiled
            .run_live(&task, &mut source, &mut locked, budget, &cancellation)
            .unwrap();
        assert_eq!(locked_run.status(), IterativeStatus::Rejected);
        assert_eq!(locked.calls(), 1);
        assert_ne!(unlocked_run.evidence(), locked_run.evidence());
    }

    fn deadline_probe_compiled(step: &str) -> CompiledIterativeLifecycle {
        compile_agent_lifecycle_v2(
            &super::super::tests::source(step).replace("state.epoch < 3", "state.epoch < 0"),
            "driver-live-deadline.spx",
            &crate::agent_lifecycle::tests::DEFINITION
                .replace("RUNTIME", crate::agent_lifecycle::tests::RUNTIME_V1),
            "fixture.agent.type.step",
        )
        .unwrap()
    }

    #[test]
    fn live_driver_rechecks_deadline_after_stage_and_effect_callbacks_before_dispatch() {
        let compiled = deadline_probe_compiled("Step::Complete { summary: state.objective, budget: state.budget, status: state.epoch }");
        let stage_clock = std::rc::Rc::new(std::cell::Cell::new(0));
        let mut stage_source = DeadlineProbe {
            compiled: &compiled.inner,
            clock: std::rc::Rc::clone(&stage_clock),
        };
        let mut stage_driver = Driver {
            advance_stage: Some(("observe", stage_clock)),
            ..make_driver()
        };
        let stage_error = compiled
            .run_with_driver_live(
                &live_task(),
                &mut stage_source,
                &mut stage_driver,
                IterativeBudget::default(),
                &AgentCancellation::new(),
            )
            .err()
            .expect("an expired stage callback must stop before evaluation");
        assert!(matches!(stage_error, DriverFailure::Diagnostics(_)));
        assert_eq!(stage_driver.calls, 0);
        assert_eq!(stage_driver.events, ["0:initialize", "0:observe"]);

        let effect_clock = std::rc::Rc::new(std::cell::Cell::new(0));
        let mut effect_source = DeadlineProbe {
            compiled: &compiled.inner,
            clock: std::rc::Rc::clone(&effect_clock),
        };
        let mut effect_driver = Driver {
            advance_effect: Some(effect_clock),
            ..make_driver()
        };
        let effect_error = compiled
            .run_with_driver_live(
                &live_task(),
                &mut effect_source,
                &mut effect_driver,
                IterativeBudget {
                    max_iterations: 1,
                    ..IterativeBudget::default()
                },
                &AgentCancellation::new(),
            )
            .err()
            .expect("an expired effect callback must stop before host dispatch");
        assert!(matches!(effect_error, DriverFailure::Diagnostics(_)));
        assert_eq!(effect_driver.calls, 0);
        assert_eq!(effect_driver.target_calls, 0);
        assert_eq!(
            effect_driver.events,
            [
                "0:initialize",
                "0:observe",
                "0:authorize",
                "0:intent_context"
            ]
        );
    }

    #[test]
    fn live_driver_rechecks_deadline_after_transition_before_publication() {
        let compiled = deadline_probe_compiled("Step::Complete { summary: state.objective, budget: state.budget, status: state.epoch }");
        let clock = std::rc::Rc::new(std::cell::Cell::new(0));
        let mut source = DeadlineProbe {
            compiled: &compiled.inner,
            clock: std::rc::Rc::clone(&clock),
        };
        let mut driver = Driver {
            advance_transition: Some(clock),
            ..make_driver()
        };
        let error = compiled
            .run_with_driver_live(
                &live_task(),
                &mut source,
                &mut driver,
                IterativeBudget {
                    max_iterations: 1,
                    ..IterativeBudget::default()
                },
                &AgentCancellation::new(),
            )
            .err()
            .expect("an expired transition callback cannot publish a result");

        assert!(matches!(error, DriverFailure::Diagnostics(_)));
        assert_eq!(driver.calls, 1);
        assert_eq!(
            driver.events,
            [
                "0:initialize",
                "0:observe",
                "0:authorize",
                "0:intent_context",
                "0:read",
                "0:reduce",
                "0:Complete"
            ]
        );
    }

    #[test]
    fn live_transition_cancellation_blocks_complete_but_preserves_selected_fail() {
        for (step, expected) in [
            ("Step::Complete { summary: state.objective, budget: state.budget, status: state.epoch }", IterativeStatus::Cancelled),
            ("Step::Fail { code: 7 }", IterativeStatus::Fail),
        ] {
            let compiled = deadline_probe_compiled(step);
            let clock = std::rc::Rc::new(std::cell::Cell::new(0));
            let mut source = DeadlineProbe { compiled: &compiled.inner, clock: clock.clone() };
            let cancellation = AgentCancellation::new();
            let mut driver = Driver {
                cancel_transition: Some(cancellation.clone()),
                advance_transition: Some(clock),
                ..make_driver()
            };
            let run = compiled.run_with_driver_live(
                &live_task(), &mut source, &mut driver,
                IterativeBudget::default(), &cancellation,
            ).unwrap();
            assert_eq!(run.status(), expected);
            assert_eq!(driver.calls, 1);
            assert_eq!(run.effects(), 1);
        }
    }

    #[test]
    fn live_malformed_proposal_retries_bounded_with_zero_effects_until_valid() {
        let compiled = compile_agent_lifecycle_v2(
            &super::super::tests::source("Step::Fail { code: 7 }"),
            "driver-live.spx",
            &crate::agent_lifecycle::tests::DEFINITION
                .replace("RUNTIME", crate::agent_lifecycle::tests::RUNTIME_V1),
            "fixture.agent.type.step",
        )
        .unwrap();
        let task = live_task();
        let one_turn = IterativeBudget {
            max_iterations: 1,
            ..IterativeBudget::default()
        };
        let cancellation = AgentCancellation::new();

        // One malformed attempt, then a valid retry: the retry is not free
        // (it is counted) but it costs no effect call by itself.
        let mut read = FixtureRead::new(Vec::new());
        let mut source = MalformedThenValid {
            compiled: &compiled.inner,
            fail_attempts: 1,
            calls: 0,
        };
        let run = compiled
            .run_live(&task, &mut source, &mut read, one_turn, &cancellation)
            .unwrap();
        assert_eq!(run.status(), IterativeStatus::BudgetExhausted);
        assert_eq!(read.calls(), 1);
        assert_eq!(source.calls, 2);

        // A source that never produces a decodable proposal exhausts the
        // bounded attempt count and ends the run with zero effects, rather
        // than looping unboundedly.
        let mut read = FixtureRead::new(Vec::new());
        let mut source = MalformedThenValid {
            compiled: &compiled.inner,
            fail_attempts: usize::MAX,
            calls: 0,
        };
        let run = compiled
            .run_live(&task, &mut source, &mut read, one_turn, &cancellation)
            .unwrap();
        assert_eq!(run.status(), IterativeStatus::ModelFailed);
        assert_eq!(read.calls(), 0);
        assert_eq!(source.calls, MAX_PROPOSAL_ATTEMPTS);
    }

    #[test]
    fn live_route_reaches_every_reducer_selected_terminal_and_is_budget_exhaustion_evidence_bearing(
    ) {
        for (expression, expected) in [
            (
                "Step::Complete { summary: state.objective, budget: state.budget, status: state.epoch }",
                IterativeStatus::Complete,
            ),
            (
                "Step::Suspend { objective: state.objective, budget: state.budget, epoch: state.epoch }",
                IterativeStatus::Suspend,
            ),
            ("Step::Fail { code: 37 }", IterativeStatus::Fail),
        ] {
            let compiled = compile_agent_lifecycle_v2(
                &super::super::tests::source(expression),
                "driver-live.spx",
                &crate::agent_lifecycle::tests::DEFINITION
                    .replace("RUNTIME", crate::agent_lifecycle::tests::RUNTIME_V1),
                "fixture.agent.type.step",
            )
            .unwrap();
            let mut read = FixtureRead::new(Vec::new());
            let mut source = AlwaysGrant {
                compiled: &compiled.inner,
            };
            let run = compiled
                .run_live(
                    &live_task(),
                    &mut source,
                    &mut read,
                    IterativeBudget::default(),
                    &AgentCancellation::new(),
                )
                .unwrap();
            assert_eq!(run.status(), expected);
            assert_eq!(read.calls(), 3);
        }

        // Every remaining turn stops requesting once a terminal is reached:
        // a source that panics on a fourth turn never gets called, because a
        // budget too small to reach Fail ends the run first.
        let compiled = compile_agent_lifecycle_v2(
            &super::super::tests::source("Step::Fail { code: 1 }"),
            "driver-live.spx",
            &crate::agent_lifecycle::tests::DEFINITION
                .replace("RUNTIME", crate::agent_lifecycle::tests::RUNTIME_V1),
            "fixture.agent.type.step",
        )
        .unwrap();
        let mut read = FixtureRead::new(Vec::new());
        let mut source = AlwaysGrant {
            compiled: &compiled.inner,
        };
        let run = compiled
            .run_live(
                &live_task(),
                &mut source,
                &mut read,
                IterativeBudget {
                    max_iterations: 1,
                    ..IterativeBudget::default()
                },
                &AgentCancellation::new(),
            )
            .unwrap();
        assert_eq!(run.status(), IterativeStatus::BudgetExhausted);
        assert!(run.evidence().starts_with('{'));
        assert!(run.evidence_digest().starts_with("sha256:"));
    }

    #[test]
    fn live_route_and_frozen_route_share_one_kernel_on_an_identical_sequence() {
        let compiled = compile_agent_lifecycle_v2(
            &super::super::tests::source(
                "Step::Complete { summary: state.objective, budget: state.budget, status: state.epoch }",
            ),
            "driver-live.spx",
            &crate::agent_lifecycle::tests::DEFINITION
                .replace("RUNTIME", crate::agent_lifecycle::tests::RUNTIME_V1),
            "fixture.agent.type.step",
        )
        .unwrap();
        let task = live_task();
        let budget = IterativeBudget::default();
        let cancellation = AgentCancellation::new();
        let proposals = vec![crate::agent_lifecycle::tests::proposal(&compiled.inner, "1", "1"); 4];

        let mut frozen_read = FixtureRead::new(b"read".to_vec());
        let frozen = compiled
            .run(&task, &proposals, &mut frozen_read, budget, &cancellation)
            .unwrap();

        let mut live_read = FixtureRead::new(b"read".to_vec());
        let mut replay = Replay {
            proposals: &proposals,
        };
        let live = compiled
            .run_live(&task, &mut replay, &mut live_read, budget, &cancellation)
            .unwrap();

        assert_eq!(frozen.status(), IterativeStatus::Complete);
        assert_eq!(live.status(), frozen.status());
        assert_eq!(live.iterations(), frozen.iterations());
        assert_eq!(live.effects(), frozen.effects());
        assert_eq!(
            live.authorization_bindings(),
            frozen.authorization_bindings()
        );
        // The invocation digest is intentionally route-specific (a live run
        // never had a predeclared sequence to bind), so only the part of the
        // evidence that reports the shared kernel's behavior is compared.
        assert_ne!(live.invocation_digest(), frozen.invocation_digest());
    }
}
