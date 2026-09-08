//! Private stage/effect driver. Hooks cannot mint authorization or replace checked values.
use super::*;

pub(crate) enum DriverFailure {
    Diagnostics(Vec<Diagnostic>),
    Persistence {
        terminal: IterativeRun,
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

impl CompiledIterativeLifecycle {
    pub(crate) fn run_with_driver(
        &self,
        task: &LifecycleTask,
        proposals: &[String],
        driver: &mut dyn IterativeDriver,
        budget: IterativeBudget,
        cancellation: &AgentCancellation,
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
            invocation_digest: invocation_digest(task, proposals, budget),
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
                let evaluation = inner.evaluate($stage, $arguments, budget.max_steps_per_stage)?;
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
        let mut state = evaluate!(
            &inner.binding.initialize,
            &[payload(
                &inner.binding.task,
                task.objective.clone(),
                task.budget
            )]
        );
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
            let mut args = vec![state.clone()];
            args.extend(projected.iter().cloned());
            driver.before_stage("authorize", run.iterations, budget.max_steps_per_stage)?;
            let (decision, record) = authorization::run_authorize_stage(
                &inner.program,
                &inner.binding.authorize,
                &args,
                budget.max_steps_per_stage,
                &policy,
                &state,
                decoded.canonical_json(),
            )?;
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
            let Some(bytes) = driver.read(&request)? else {
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
                        terminal: run.finish(status, Some(value), self.digest()),
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
        reject_stage: Option<&'static str>,
        reject_transition: Option<usize>,
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
            Ok(())
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
            reject_stage: None,
            reject_transition: None,
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
}
