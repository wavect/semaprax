//! Checked feedback-driven source loop; shares the parent driver hooks.
use super::super::source_live;
use super::*;

impl CompiledIterativeLifecycle {
    /// The live route. Shares initialize/observe/authorize/effect/reduce with
    /// [`Self::run_with_driver_initial`] exactly; the only difference is where
    /// each turn's proposal text comes from: a call to `source.propose`,
    /// carrying the real previous effect result, instead of indexing a
    /// predeclared slice by turn.
    ///
    /// A decode failure does not end the turn immediately: the source gets a
    /// bounded number of attempts (see [`MAX_PROPOSAL_ATTEMPTS`]), each one
    /// counted and producing no effect call, before the run ends as
    /// `ModelFailed`. Every other terminal condition — refusal, budget
    /// exhaustion, cancellation, and the reducer's own Complete/Suspend/Fail
    /// selection — is exactly the frozen route's, because this reuses the
    /// same authorize/effect/reduce calls on the same driver.
    pub(crate) fn run_with_driver_live(
        &self,
        task: &LifecycleTask,
        source: &mut dyn ProposalSource,
        driver: &mut dyn IterativeDriver,
        budget: IterativeBudget,
        cancellation: &AgentCancellation,
    ) -> Result<IterativeRun, DriverFailure> {
        self.run_with_driver_live_session(task, source, driver, budget, cancellation, None)
    }

    pub(crate) fn run_with_driver_live_session(
        &self,
        task: &LifecycleTask,
        source: &mut dyn ProposalSource,
        driver: &mut dyn IterativeDriver,
        budget: IterativeBudget,
        cancellation: &AgentCancellation,
        mut session: Option<&mut source_live::SourceExecutionSession<'_>>,
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
            invocation_digest: super::live_invocation_digest(
                task,
                budget,
                inner.proposal.schema().digest(),
            ),
            evidence: String::new(),
            digest: String::new(),
        };
        let mut last_effect: Option<Vec<u8>> = None;
        macro_rules! stop {
            ($status:expr, $value:expr) => {
                return Ok(run.finish($status, $value, self.digest()))
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
                if run.stages.len() >= budget.max_stages {
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
                let evaluation = inner.evaluate($stage, $arguments, budget.max_steps_per_stage)?;
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
        let mut state = evaluate!(
            &inner.binding.initialize,
            &[payload(
                &inner.binding.task,
                task.objective.clone(),
                task.budget
            )],
            None
        );
        if !inner.carries(&state, "state") {
            return Err(vec![bad("initialize.identity")].into());
        }
        loop {
            if run.iterations >= budget.max_iterations {
                stop!(IterativeStatus::BudgetExhausted, None);
            }
            let observation = evaluate!(&inner.binding.observe, std::slice::from_ref(&state), None);
            if !inner.carries(&observation, "observation") {
                return Err(vec![bad("observe.identity")].into());
            }
            if let Some(session) = session.as_deref_mut() {
                session.observed(run.iterations, &state, &observation, last_effect.as_deref())?;
            }
            let mut attempt = 0usize;
            let mut rejection: Option<String> = None;
            let decoded = loop {
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
                    previous_rejection: rejection.as_deref(),
                    remaining_iterations: budget.max_iterations - run.iterations,
                };
                let proposal_text = if let Some(session) = session.as_deref_mut() {
                    session.propose(source, request)?
                } else {
                    source.propose(request)?
                };
                live_guard!();
                match inner.proposal.decode(&proposal_text) {
                    Ok(decoded) => break decoded,
                    Err(_) => {
                        if let Some(session) = session.as_deref_mut() {
                            session.proposal_refused(run.iterations, attempt)?;
                        }
                        attempt += 1;
                        if attempt >= MAX_PROPOSAL_ATTEMPTS {
                            stop!(IterativeStatus::ModelFailed, None);
                        }
                        rejection = Some(format!("proposal.decode.attempt.{attempt}"));
                    }
                }
            };
            let Some(projected) = inner.project(&decoded) else {
                stop!(IterativeStatus::ModelFailed, None);
            };
            if let Some(session) = session.as_deref_mut() {
                session.proposal_admitted(run.iterations, attempt, decoded.canonical_json())?;
            }
            boundary!();
            let policy = digest(
                b"semaprax.agent-iteration-policy.v2\0",
                format!("{}\0{}", self.digest(), run.iterations).as_bytes(),
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
            let (decision, record) = authorization::run_authorize_stage(
                &inner.program,
                &inner.binding.authorize,
                &args,
                budget.max_steps_per_stage,
                &policy,
                &state,
                decoded.canonical_json(),
            )?;
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
            if cancellation.is_cancelled() {
                stop!(IterativeStatus::Cancelled, None);
            }
            // Reserve reducer capacity before dispatch: no known-doomed effect.
            if run.stages.len() >= budget.max_stages {
                stop!(IterativeStatus::BudgetExhausted, None);
            }
            let request = authorized.consume();
            let expected = authorization::binding(
                &policy,
                &state,
                decoded.canonical_json(),
                inner.binding.authorize.grant_case(),
                request.seal(),
            );
            if expected != request.binding() {
                return Err(vec![bad("authorization.binding")].into());
            }
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
            let read = if let Some(session) = session.as_deref_mut() {
                session.read(run.iterations, attempt, &request, driver)?
            } else {
                driver.read(&request)?
            };
            let Some(bytes) = read else {
                stop!(IterativeStatus::EffectFailed, None);
            };
            if bytes.len() > MAX_READ_BYTES {
                stop!(IterativeStatus::EffectFailed, None);
            }
            last_effect = Some(bytes.clone());
            let mut args = vec![state];
            args.extend(projected);
            args.push(payload(&inner.binding.outcome, bytes, 0));
            let value = evaluate!(&inner.binding.reduce, &args, Some(attempt));
            run.iterations += 1;
            let (transition, value) = self.step.decode(value)?;
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
                            terminal: Box::new(run.finish(status, Some(value), self.digest())),
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
            if let Err(diagnostics) =
                driver.after_transition(run.iterations - 1, transition, &value)
            {
                transition_persistence_failure!(diagnostics);
            }
            if transition != "Fail" {
                live_guard!();
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
