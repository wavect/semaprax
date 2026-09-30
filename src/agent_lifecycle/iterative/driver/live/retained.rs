//! Frozen retained stage operations behind the one live control kernel.
//! Relocated stage bodies preserve guard/callback/reservation/evaluation order.
use super::kernel::{self, Flow, Proposal, Route, Transition};
use super::*;

struct Observed {
    state: RetainedValue,
    observation: RetainedValue,
}
struct Proposed {
    state: RetainedValue,
    decoded: DecodedProposal,
    projected: Vec<RetainedValue>,
}
struct Granted {
    state: RetainedValue,
    decoded: DecodedProposal,
    projected: Vec<RetainedValue>,
    authorized: authorization::Authorized,
    policy: String,
}
struct Executed {
    state: RetainedValue,
    projected: Vec<RetainedValue>,
    bytes: Vec<u8>,
}
struct Step {
    transition: &'static str,
    value: RetainedValue,
}

struct RetainedRoute<'a, 'j> {
    compiled: &'a CompiledIterativeLifecycle,
    task: &'a LifecycleTask,
    source: &'a mut dyn ProposalSource,
    driver: &'a mut dyn IterativeDriver,
    budget: IterativeBudget,
    cancellation: &'a AgentCancellation,
    session: Option<&'a mut source_live::SourceExecutionSession<'j>>,
    migrated: Option<&'a source_live::SourceMigrationSeed>,
    allow_target_effect: bool,
    backend: authorization::StageBackend<'a>,
    run: Option<IterativeRun>,
    last_effect: Option<Vec<u8>>,
    prior_stages: usize,
}
impl RetainedRoute<'_, '_> {
    fn finish(&mut self, status: IterativeStatus, value: Option<RetainedValue>) -> IterativeRun {
        self.run
            .take()
            .expect("one terminal run")
            .finish(status, value, self.compiled.digest())
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run<'a, 'j>(
    compiled: &'a CompiledIterativeLifecycle,
    task: &'a LifecycleTask,
    source: &'a mut dyn ProposalSource,
    driver: &'a mut dyn IterativeDriver,
    budget: IterativeBudget,
    cancellation: &'a AgentCancellation,
    session: Option<&'a mut source_live::SourceExecutionSession<'j>>,
    migrated: Option<&'a source_live::SourceMigrationSeed>,
    allow_target_effect: bool,
    backend: authorization::StageBackend<'a>,
) -> Result<IterativeRun, DriverFailure> {
    if budget.max_iterations > 4096 || budget.max_stages > 12289 {
        return Err(vec![bad("budget.capacity")].into());
    }
    let inner = &compiled.inner;
    let run = IterativeRun {
        status: IterativeStatus::BudgetExhausted,
        iterations: migrated.map_or(0, |seed| seed.prior_turns),
        stages: Vec::new(),
        effects: migrated.map_or(0, |seed| seed.prior_effects),
        value: None,
        authorization_bindings: Vec::new(),
        invocation_digest: super::super::live_invocation_digest(
            task,
            budget,
            inner.proposal.schema().digest(),
        ),
        evidence: String::new(),
        digest: String::new(),
    };
    kernel::run(RetainedRoute {
        compiled,
        task,
        source,
        driver,
        budget,
        cancellation,
        session,
        migrated,
        allow_target_effect,
        backend,
        run: Some(run),
        last_effect: None,
        prior_stages: migrated.map_or(0, |seed| seed.prior_stages),
    })
}

impl Route for RetainedRoute<'_, '_> {
    type State = RetainedValue;
    type Observed = Observed;
    type Proposed = Proposed;
    type Granted = Granted;
    type Executed = Executed;
    type Step = Step;
    type Output = IterativeRun;
    type Failure = DriverFailure;
    fn initialize(&mut self) -> Result<Flow<Self::State, Self::Output>, Self::Failure> {
        let inner = &self.compiled.inner;
        let task = self.task;
        let budget = self.budget;
        let run = self.run.as_mut().expect("live run");
        let source = &mut *self.source;
        let driver = &mut *self.driver;
        let cancellation = self.cancellation;
        let mut session = self.session.as_deref_mut();
        let prior_stages = self.prior_stages;
        let backend = self.backend;
        macro_rules! stop {
            ($status:expr, $value:expr) => {
                return Ok(Flow::Stopped(self.finish($status, $value)))
            };
        }
        macro_rules! live_guard {
            () => {
                if cancellation.is_cancelled() {
                    stop!(IterativeStatus::Cancelled, None);
                }
                source.check_deadline()?;
                if let Some(session) = session.as_deref_mut() {
                    session.guard()?;
                }
            };
        }
        macro_rules! boundary {
            () => {
                if cancellation.is_cancelled() {
                    stop!(IterativeStatus::Cancelled, None);
                }
                if prior_stages.saturating_add(run.stages.len()) >= budget.max_stages {
                    stop!(IterativeStatus::BudgetExhausted, None);
                }
                source.check_deadline()?;
                if let Some(session) = session.as_deref_mut() {
                    session.guard()?;
                }
            };
        }
        macro_rules! evaluate {
            ($stage:expr, $arguments:expr, $attempt:expr) => {{
                boundary!();
                driver.before_stage($stage.role(), run.iterations, budget.max_steps_per_stage)?;
                live_guard!();
                if let Some(session) = session.as_deref_mut() {
                    session.before_stage(
                        $stage.role(),
                        run.iterations,
                        $attempt,
                        budget.max_steps_per_stage,
                    )?;
                }
                // Every public target selector crosses the same cancellable
                // sealed boundary. Interpreter used to bypass this route,
                // which made a cancellation racing its stage reservation a
                // backend-dependent diagnostic instead of a lifecycle
                // settlement.
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
                if let Some(session) = session.as_deref_mut() {
                    session.record_stage(run.stages.last().expect("stage just pushed"));
                }
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
        let state = if let Some(seed) = self.migrated {
            seed.state.clone()
        } else {
            evaluate!(
                &inner.binding.initialize,
                &[payload(
                    &inner.binding.task,
                    task.objective.clone(),
                    task.budget
                )],
                None
            )
        };
        if !inner.carries(&state, "state") {
            return Err(vec![bad("initialize.identity")].into());
        }
        Ok(Flow::Advance(state))
    }
    fn begin_turn(
        &mut self,
        state: Self::State,
    ) -> Result<Flow<Self::State, Self::Output>, Self::Failure> {
        let budget = self.budget;
        let run = self.run.as_mut().expect("live run");
        macro_rules! stop {
            ($status:expr, $value:expr) => {
                return Ok(Flow::Stopped(self.finish($status, $value)))
            };
        }
        if run.iterations >= budget.max_iterations {
            stop!(IterativeStatus::BudgetExhausted, None);
        }
        Ok(Flow::Advance(state))
    }
    fn observe(
        &mut self,
        state: Self::State,
    ) -> Result<Flow<Self::Observed, Self::Output>, Self::Failure> {
        let inner = &self.compiled.inner;
        let budget = self.budget;
        let run = self.run.as_mut().expect("live run");
        let source = &mut *self.source;
        let driver = &mut *self.driver;
        let cancellation = self.cancellation;
        let mut session = self.session.as_deref_mut();
        let prior_stages = self.prior_stages;
        let backend = self.backend;
        let last_effect = &self.last_effect;
        macro_rules! stop {
            ($status:expr, $value:expr) => {
                return Ok(Flow::Stopped(self.finish($status, $value)))
            };
        }
        macro_rules! live_guard {
            () => {
                if cancellation.is_cancelled() {
                    stop!(IterativeStatus::Cancelled, None);
                }
                source.check_deadline()?;
                if let Some(session) = session.as_deref_mut() {
                    session.guard()?;
                }
            };
        }
        macro_rules! boundary {
            () => {
                if cancellation.is_cancelled() {
                    stop!(IterativeStatus::Cancelled, None);
                }
                if prior_stages.saturating_add(run.stages.len()) >= budget.max_stages {
                    stop!(IterativeStatus::BudgetExhausted, None);
                }
                source.check_deadline()?;
                if let Some(session) = session.as_deref_mut() {
                    session.guard()?;
                }
            };
        }
        macro_rules! evaluate {
            ($stage:expr, $arguments:expr, $attempt:expr) => {{
                boundary!();
                driver.before_stage($stage.role(), run.iterations, budget.max_steps_per_stage)?;
                live_guard!();
                if let Some(session) = session.as_deref_mut() {
                    session.before_stage(
                        $stage.role(),
                        run.iterations,
                        $attempt,
                        budget.max_steps_per_stage,
                    )?;
                }
                // Every public target selector crosses the same cancellable
                // sealed boundary. Interpreter used to bypass this route,
                // which made a cancellation racing its stage reservation a
                // backend-dependent diagnostic instead of a lifecycle
                // settlement.
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
                if let Some(session) = session.as_deref_mut() {
                    session.record_stage(run.stages.last().expect("stage just pushed"));
                }
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
        let observation = evaluate!(&inner.binding.observe, std::slice::from_ref(&state), None);
        if !inner.carries(&observation, "observation") {
            return Err(vec![bad("observe.identity")].into());
        }
        if let Some(session) = session.as_deref_mut() {
            session.observed(run.iterations, &state, &observation, last_effect.as_deref())?;
        }
        Ok(Flow::Advance(Observed { state, observation }))
    }
    fn propose(
        &mut self,
        observed: Self::Observed,
        attempt: usize,
        rejection: Option<&str>,
    ) -> Result<Proposal<Self::Observed, Self::Proposed, Self::Output>, Self::Failure> {
        let inner = &self.compiled.inner;
        let task = self.task;
        let budget = self.budget;
        let run = self.run.as_mut().expect("live run");
        let source = &mut *self.source;
        let cancellation = self.cancellation;
        let mut session = self.session.as_deref_mut();
        let prior_stages = self.prior_stages;
        let last_effect = &self.last_effect;
        macro_rules! stop {
            ($status:expr, $value:expr) => {
                return Ok(Proposal::Stopped(self.finish($status, $value)))
            };
        }
        macro_rules! live_guard {
            () => {
                if cancellation.is_cancelled() {
                    stop!(IterativeStatus::Cancelled, None);
                }
                source.check_deadline()?;
                if let Some(session) = session.as_deref_mut() {
                    session.guard()?;
                }
            };
        }
        macro_rules! boundary {
            () => {
                if cancellation.is_cancelled() {
                    stop!(IterativeStatus::Cancelled, None);
                }
                if prior_stages.saturating_add(run.stages.len()) >= budget.max_stages {
                    stop!(IterativeStatus::BudgetExhausted, None);
                }
                source.check_deadline()?;
                if let Some(session) = session.as_deref_mut() {
                    session.guard()?;
                }
            };
        }
        let Observed { state, observation } = observed;
        boundary!();
        let request = ProposalRequest {
            turn: run.iterations,
            attempt,
            source_revision: &inner.source_revision,
            proposal_schema_digest: inner.proposal.schema().digest(),
            task,
            state: &state,
            observation: &observation,
            previous_effect: last_effect.as_deref(),
            previous_rejection: rejection,
            remaining_iterations: budget.max_iterations - run.iterations,
        };
        let proposal_text = if let Some(session) = session.as_deref_mut() {
            session.propose(source, request)?
        } else {
            source.propose(request)?
        };
        live_guard!();
        let decoded = match inner.proposal.decode(&proposal_text) {
            Ok(decoded) => decoded,
            Err(_) => {
                if let Some(session) = session.as_deref_mut() {
                    session.proposal_refused(run.iterations, attempt)?;
                }
                return Ok(Proposal::Rejected(Observed { state, observation }));
            }
        };
        let Some(projected) = inner.project(&decoded) else {
            stop!(IterativeStatus::ModelFailed, None);
        };
        if let Some(session) = session.as_deref_mut() {
            session.model_wait_proposal(run.iterations, attempt, &decoded)?;
            session.proposal_admitted(run.iterations, attempt, decoded.canonical_json())?;
        }
        Ok(Proposal::Accepted(Proposed {
            state,
            decoded,
            projected,
        }))
    }
    fn attempt_limit(&mut self, _observed: Self::Observed) -> Result<Self::Output, Self::Failure> {
        Ok(self.finish(IterativeStatus::ModelFailed, None))
    }
    fn authorize(
        &mut self,
        proposed: Self::Proposed,
        attempt: usize,
    ) -> Result<Flow<Self::Granted, Self::Output>, Self::Failure> {
        let inner = &self.compiled.inner;
        let budget = self.budget;
        let run = self.run.as_mut().expect("live run");
        let source = &mut *self.source;
        let driver = &mut *self.driver;
        let cancellation = self.cancellation;
        let mut session = self.session.as_deref_mut();
        let prior_stages = self.prior_stages;
        let backend = self.backend;
        macro_rules! stop {
            ($status:expr, $value:expr) => {
                return Ok(Flow::Stopped(self.finish($status, $value)))
            };
        }
        macro_rules! live_guard {
            () => {
                if cancellation.is_cancelled() {
                    stop!(IterativeStatus::Cancelled, None);
                }
                source.check_deadline()?;
                if let Some(session) = session.as_deref_mut() {
                    session.guard()?;
                }
            };
        }
        macro_rules! boundary {
            () => {
                if cancellation.is_cancelled() {
                    stop!(IterativeStatus::Cancelled, None);
                }
                if prior_stages.saturating_add(run.stages.len()) >= budget.max_stages {
                    stop!(IterativeStatus::BudgetExhausted, None);
                }
                source.check_deadline()?;
                if let Some(session) = session.as_deref_mut() {
                    session.guard()?;
                }
            };
        }
        let Proposed {
            state,
            decoded,
            projected,
        } = proposed;
        boundary!();
        let policy = digest(
            b"semaprax.agent-iteration-policy.v2\0",
            format!("{}\0{}", self.compiled.digest(), run.iterations).as_bytes(),
        );
        let mut args = vec![state.clone()];
        args.extend(projected.iter().cloned());
        driver.before_stage("authorize", run.iterations, budget.max_steps_per_stage)?;
        live_guard!();
        if let Some(session) = session.as_deref_mut() {
            session.before_stage(
                "authorize",
                run.iterations,
                Some(attempt),
                budget.max_steps_per_stage,
            )?;
        }
        let (decision, record) = match authorization::run_authorize_stage_on_cancellable(
            backend,
            &inner.program,
            &inner.binding.authorize,
            &args,
            budget.max_steps_per_stage,
            &policy,
            &state,
            decoded.canonical_json(),
            Some(cancellation),
        ) {
            Ok(value) => value,
            Err(_) if cancellation.is_cancelled() => {
                stop!(IterativeStatus::Cancelled, None);
            }
            Err(errors) => return Err(errors.into()),
        };
        if let Some(session) = session.as_deref_mut() {
            session.record_stage(&record);
        }
        run.stages.push(record);
        let authorized = match decision {
            authorization::AuthorizationOutcome::Granted(value) => value,
            authorization::AuthorizationOutcome::Refused(_) => {
                if let Some(session) = session.as_deref_mut() {
                    session.authorization_refused(run.iterations, attempt, false)?;
                }
                stop!(IterativeStatus::Rejected, None);
            }
            authorization::AuthorizationOutcome::Undecided("fuel" | "depth") => {
                if let Some(session) = session.as_deref_mut() {
                    session.authorization_refused(run.iterations, attempt, true)?;
                }
                stop!(IterativeStatus::BudgetExhausted, None);
            }
            _ => {
                if let Some(session) = session.as_deref_mut() {
                    session.authorization_refused(run.iterations, attempt, false)?;
                }
                stop!(IterativeStatus::Rejected, None);
            }
        };
        Ok(Flow::Advance(Granted {
            state,
            decoded,
            projected,
            authorized,
            policy,
        }))
    }
    fn effect(
        &mut self,
        granted: Self::Granted,
        attempt: usize,
    ) -> Result<Flow<Self::Executed, Self::Output>, Self::Failure> {
        let inner = &self.compiled.inner;
        let budget = self.budget;
        let run = self.run.as_mut().expect("live run");
        let source = &mut *self.source;
        let driver = &mut *self.driver;
        let cancellation = self.cancellation;
        let mut session = self.session.as_deref_mut();
        let prior_stages = self.prior_stages;
        let allow_target_effect = self.allow_target_effect;
        macro_rules! stop {
            ($status:expr, $value:expr) => {
                return Ok(Flow::Stopped(self.finish($status, $value)))
            };
        }
        macro_rules! live_guard {
            () => {
                if cancellation.is_cancelled() {
                    stop!(IterativeStatus::Cancelled, None);
                }
                source.check_deadline()?;
                if let Some(session) = session.as_deref_mut() {
                    session.guard()?;
                }
            };
        }
        let Granted {
            state,
            decoded,
            projected,
            authorized,
            policy,
        } = granted;
        if cancellation.is_cancelled() {
            stop!(IterativeStatus::Cancelled, None);
        }
        // Reserve reducer capacity before dispatch: no known-doomed effect.
        if prior_stages.saturating_add(run.stages.len()) >= budget.max_stages {
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
        live_guard!();
        let target_effect = if allow_target_effect {
            driver.target_effect(
                TargetEffectContext {
                    invocation_root: &run.invocation_digest,
                    turn: run.iterations,
                    max_steps: budget.max_steps_per_stage,
                    proposal_canonical: decoded.canonical_json(),
                    projected: &projected,
                    cancellation,
                },
                authorized,
            )?
        } else {
            TargetEffect::Ordinary(authorized)
        };
        let read = match target_effect {
            TargetEffect::Ordinary(authorized) => {
                let request = authorized.consume();
                if let Some(session) = session.as_deref_mut() {
                    session.authorized(run.iterations, attempt, &request)?;
                }
                live_guard!();
                driver.before_effect(EffectContext {
                    turn: run.iterations,
                    policy: &policy,
                    state: &state,
                    proposal_canonical: decoded.canonical_json(),
                    authorization: &request,
                })?;
                live_guard!();
                run.authorization_bindings
                    .push(request.binding().to_owned());
                run.effects += 1;
                if let Some(session) = session.as_deref_mut() {
                    session.read(run.iterations, attempt, &request, driver)?
                } else {
                    driver.read(&request)?
                }
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
                // Cancellation observed after target dispatch is sticky:
                // the evidence preserves the charged host call, but no
                // result becomes input to `reduce` and no later stage may
                // replace the lifecycle's terminal cancellation.
                if cancellation.is_cancelled() {
                    stop!(IterativeStatus::Cancelled, None);
                }
                result
            }
        };
        let Some(bytes) = read else {
            stop!(IterativeStatus::EffectFailed, None);
        };
        if bytes.len() > MAX_READ_BYTES {
            stop!(IterativeStatus::EffectFailed, None);
        }
        self.last_effect = Some(bytes.clone());
        Ok(Flow::Advance(Executed {
            state,
            projected,
            bytes,
        }))
    }
    fn reduce(
        &mut self,
        executed: Self::Executed,
        attempt: usize,
    ) -> Result<Flow<Self::Step, Self::Output>, Self::Failure> {
        let inner = &self.compiled.inner;
        let budget = self.budget;
        let run = self.run.as_mut().expect("live run");
        let source = &mut *self.source;
        let driver = &mut *self.driver;
        let cancellation = self.cancellation;
        let mut session = self.session.as_deref_mut();
        let prior_stages = self.prior_stages;
        let backend = self.backend;
        macro_rules! stop {
            ($status:expr, $value:expr) => {
                return Ok(Flow::Stopped(self.finish($status, $value)))
            };
        }
        macro_rules! live_guard {
            () => {
                if cancellation.is_cancelled() {
                    stop!(IterativeStatus::Cancelled, None);
                }
                source.check_deadline()?;
                if let Some(session) = session.as_deref_mut() {
                    session.guard()?;
                }
            };
        }
        macro_rules! boundary {
            () => {
                if cancellation.is_cancelled() {
                    stop!(IterativeStatus::Cancelled, None);
                }
                if prior_stages.saturating_add(run.stages.len()) >= budget.max_stages {
                    stop!(IterativeStatus::BudgetExhausted, None);
                }
                source.check_deadline()?;
                if let Some(session) = session.as_deref_mut() {
                    session.guard()?;
                }
            };
        }
        macro_rules! evaluate {
            ($stage:expr, $arguments:expr, $attempt:expr) => {{
                boundary!();
                driver.before_stage($stage.role(), run.iterations, budget.max_steps_per_stage)?;
                live_guard!();
                if let Some(session) = session.as_deref_mut() {
                    session.before_stage(
                        $stage.role(),
                        run.iterations,
                        $attempt,
                        budget.max_steps_per_stage,
                    )?;
                }
                // Every public target selector crosses the same cancellable
                // sealed boundary. Interpreter used to bypass this route,
                // which made a cancellation racing its stage reservation a
                // backend-dependent diagnostic instead of a lifecycle
                // settlement.
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
                if let Some(session) = session.as_deref_mut() {
                    session.record_stage(run.stages.last().expect("stage just pushed"));
                }
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
        let Executed {
            state,
            projected,
            bytes,
        } = executed;
        let mut args = vec![state];
        args.extend(projected);
        args.push(payload(&inner.binding.outcome, bytes, 0));
        let value = evaluate!(&inner.binding.reduce, &args, Some(attempt));
        run.iterations += 1;
        let (transition, value) = self.compiled.step.decode(value)?;
        Ok(Flow::Advance(Step { transition, value }))
    }
    fn transition(
        &mut self,
        step: Self::Step,
        attempt: usize,
    ) -> Result<Transition<Self::State, Self::Output>, Self::Failure> {
        let run = self.run.as_mut().expect("live run");
        let source = &mut *self.source;
        let driver = &mut *self.driver;
        let cancellation = self.cancellation;
        let mut session = self.session.as_deref_mut();
        macro_rules! stop {
            ($status:expr, $value:expr) => {
                return Ok(Transition::Stopped(self.finish($status, $value)))
            };
        }
        macro_rules! live_guard {
            () => {
                if cancellation.is_cancelled() {
                    stop!(IterativeStatus::Cancelled, None);
                }
                source.check_deadline()?;
                if let Some(session) = session.as_deref_mut() {
                    session.guard()?;
                }
            };
        }
        let Step { transition, value } = step;
        // A reducer-selected Fail remains sticky: a later deadline cannot
        // replace it. Other transitions are checked before publication.
        if transition != "Fail" {
            live_guard!();
        }
        macro_rules! transition_persistence_failure {
            ($diagnostics:expr) => {{
                let selected = match transition {
                    "Complete" => Some(IterativeStatus::Complete),
                    "Suspend" => Some(IterativeStatus::Suspend),
                    "Fail" => Some(IterativeStatus::Fail),
                    _ => None,
                };
                return Err(if let Some(status) = selected {
                    DriverFailure::Persistence {
                        terminal: Box::new(self.finish(status, Some(value))),
                        diagnostics: $diagnostics,
                    }
                } else {
                    $diagnostics.into()
                });
            }};
        }
        if let Some(session) = session.as_deref_mut() {
            if let Err(diagnostics) =
                session.transition(run.iterations - 1, attempt, transition, &value)
            {
                transition_persistence_failure!(diagnostics);
            }
        }
        if let Err(diagnostics) = driver.after_transition(run.iterations - 1, transition, &value) {
            transition_persistence_failure!(diagnostics);
        }
        if transition != "Fail" {
            live_guard!();
        }
        match transition {
            "Continue" => Ok(Transition::Continue(value)),
            "Complete" => Ok(Transition::Stopped(
                self.finish(IterativeStatus::Complete, Some(value)),
            )),
            "Suspend" => Ok(Transition::Stopped(
                self.finish(IterativeStatus::Suspend, Some(value)),
            )),
            "Fail" => Ok(Transition::Stopped(
                self.finish(IterativeStatus::Fail, Some(value)),
            )),
            _ => Err(vec![bad("step.transition")].into()),
        }
    }
}
