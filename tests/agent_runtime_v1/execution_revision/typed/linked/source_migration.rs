//! Actual retained-Project source handoff through the checked source driver.
use super::super::migration::durable;
use super::*;
use semaprax::agent_lifecycle::iterative::driver::{ProposalRequest, ProposalSource};
use semaprax::agent_lifecycle::iterative::source_live::{
    prepare_source_live_migration, prepare_source_live_migration_from_hot_reload_handoff,
    prepare_source_live_priced_migration, run_source_live_migration_from_hot_reload_session,
    SourceAgentHandoffFailure, SourceAttemptIdentity, SourceLiveMigrationEndpoint,
    SourceLiveMigrationRequest, SourceLivePolicy, SourceLivePricing, SourceLiveRequest,
    SourceProposalOutcome, SourceProposalPolicy,
};
use semaprax::agent_lifecycle::iterative::{
    compile_project_agent_lifecycle_v2, CompiledIterativeLifecycle,
};
use semaprax::agent_lifecycle::{
    AgentReadOperation, AuthorizedRequest, CheckpointStore, CheckpointStoreError, LifecycleTask,
};
use semaprax::interpreter::retained_call::RetainedValue;
use semaprax::live_invocation::model_invoke::{InvocationBudgetHook, ModelInvocationRequest};
use semaprax::live_invocation::source_journal::{
    recover_source_checkpoint, source_response_digest, SourceCheckpointSink,
    SourceInvocationBinding, SourceJournalEntry, SourceJournalError, SourceTerminalStatus,
};
use semaprax::live_invocation::{CumulativeBudgetLedger, InvocationClock, SourceInvocationClock};
use semaprax::project::{
    HotReloadDecision, HotReloadSession, HotReloadSourceAgentHandoffStatus,
    PreparedProjectInterpreterOptions,
};
use sha2::{Digest, Sha256};
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

#[path = "source_migration_io.rs"]
mod source_migration_io;
#[path = "source_migration_priced.rs"]
mod source_migration_priced;

const DEPLOYMENT: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
const DEADLINE: i64 = 100_000;

#[derive(Clone)]
struct Clock(Rc<Cell<i64>>);
impl Clock {
    fn at(now: i64) -> Self {
        Self(Rc::new(Cell::new(now)))
    }
}
impl InvocationClock for Clock {
    fn now_millis(&self) -> i64 {
        self.0.get()
    }
}
impl SourceInvocationClock for Clock {
    fn clock_domain(&self) -> &str {
        "source-migration-test-ms"
    }
}

struct RegressingClock {
    reads: Cell<usize>,
    regress_at: usize,
}
impl InvocationClock for RegressingClock {
    fn now_millis(&self) -> i64 {
        let count = self.reads.get() + 1;
        self.reads.set(count);
        if count >= self.regress_at {
            -1
        } else {
            0
        }
    }
}
impl SourceInvocationClock for RegressingClock {
    fn clock_domain(&self) -> &str {
        "source-migration-test-ms"
    }
}

#[derive(Default)]
struct Store {
    document: String,
    lose_ack_on: Option<u64>,
    advance_on: Option<(u64, Rc<Cell<i64>>, i64)>,
    cancel_on: Option<(u64, AgentCancellation)>,
}
impl CheckpointStore for Store {
    fn commit(&mut self, generation: u64, document: &str) -> Result<(), CheckpointStoreError> {
        self.document = document.to_owned();
        if let Some((at, clock, next)) = &self.advance_on {
            if *at == generation {
                clock.set(*next);
            }
        }
        if let Some((at, cancellation)) = &self.cancel_on {
            if *at == generation {
                cancellation.cancel();
            }
        }
        if self.lose_ack_on == Some(generation) {
            self.lose_ack_on = None;
            Err(CheckpointStoreError)
        } else {
            Ok(())
        }
    }
}

#[derive(Default)]
struct Read {
    calls: usize,
}
impl AgentReadOperation for Read {
    fn read(&mut self, _: &AuthorizedRequest) -> Option<Vec<u8>> {
        self.calls += 1;
        Some(b"fixture-read".to_vec())
    }
}
struct Model {
    replies: Vec<Vec<u8>>,
    calls: usize,
    states: Vec<RetainedValue>,
    schemas: Vec<String>,
}
impl Model {
    fn new(compiled: &CompiledIterativeLifecycle) -> Self {
        let schema = compiled.proposal_schema().schema().digest();
        Self {
            replies: (0..3)
                .map(|index| {
                    proposal(schema, "5", false, if index == 1 { "1" } else { "0" }).into_bytes()
                })
                .collect(),
            calls: 0,
            states: Vec::new(),
            schemas: Vec::new(),
        }
    }
}
impl ProposalSource for Model {
    fn checkpoint_policy(&self) -> Option<SourceProposalPolicy<'_>> {
        Some(SourceProposalPolicy {
            deployment_binding: DEPLOYMENT,
            response_limit: 4096,
            reservation_units: 1,
        })
    }
    fn checkpoint_attempt_identity(
        &self,
        request: &ProposalRequest<'_>,
    ) -> Result<SourceAttemptIdentity, Vec<semaprax::diagnostic::Diagnostic>> {
        Ok(SourceAttemptIdentity {
            request_digest: request.source_revision.to_owned(),
            prompt_digest: request.proposal_schema_digest.to_owned(),
            request_bytes: 1,
        })
    }
    fn propose_checkpointed(
        &mut self,
        request: ProposalRequest<'_>,
        sink: &mut SourceCheckpointSink<'_>,
        ledger: &mut CumulativeBudgetLedger<'_>,
        clock: &dyn SourceInvocationClock,
    ) -> SourceProposalOutcome {
        let response = self
            .replies
            .get(self.calls)
            .cloned()
            .expect("unexpected physical model dispatch");
        let identity = self.checkpoint_attempt_identity(&request).unwrap();
        let invocation = ModelInvocationRequest {
            turn: request.turn as u32,
            task: request.task.objective.clone(),
            observation: Vec::new(),
            proposal_grammar_digest: request.proposal_schema_digest.to_owned(),
            deployment_binding: DEPLOYMENT.into(),
            max_response_bytes: 4096,
            effective_budget: 1,
        };
        if ledger.reserve(&invocation).is_err() {
            return SourceProposalOutcome {
                terminal_failure: Some(SourceTerminalStatus::BudgetExhausted),
                result: Err(vec![semaprax::diagnostic::Diagnostic::io(
                    "SPX-G582",
                    "source migration fixture budget refused",
                )]),
                model_dispatches: 0,
            };
        }
        let intent = sink
            .attempt_intent(
                request.turn as u32,
                request.attempt as u32,
                identity.request_digest,
                identity.prompt_digest,
                identity.request_bytes,
            )
            .and_then(|intent| sink.append_at(intent, clock.now_millis()).map(|_| ()))
            .is_ok();
        if !intent {
            return SourceProposalOutcome {
                terminal_failure: None,
                result: Err(vec![semaprax::diagnostic::Diagnostic::io(
                    "SPX-G582",
                    "source migration fixture intent failed",
                )]),
                model_dispatches: 0,
            };
        }
        self.states.push(request.state.clone());
        self.schemas.push(request.proposal_schema_digest.to_owned());
        self.calls += 1;
        let result = sink.append_at(
            SourceJournalEntry::AttemptSettled {
                turn: request.turn as u32,
                attempt: request.attempt as u32,
                response_digest: source_response_digest(&response),
                response: response.clone(),
            },
            clock.now_millis(),
        );
        SourceProposalOutcome {
            terminal_failure: None,
            result: result
                .map(|_| String::from_utf8(response).unwrap())
                .map_err(|_| {
                    vec![semaprax::diagnostic::Diagnostic::io(
                        "SPX-G582",
                        "source migration fixture settlement failed",
                    )]
                }),
            model_dispatches: 1,
        }
    }
    fn propose(
        &mut self,
        _: ProposalRequest<'_>,
    ) -> Result<String, Vec<semaprax::diagnostic::Diagnostic>> {
        panic!("migration fixture must use the checkpointed source route")
    }
}

fn retained(fixture: &Fixture) -> Arc<semaprax::project::ProjectRevision> {
    with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        Ok(snapshot.retain_revision())
    })
    .unwrap()
}
fn compiled(project: &semaprax::project::ProjectRevision) -> CompiledIterativeLifecycle {
    compile_project_agent_lifecycle_v2(
        project,
        "src/app.spx",
        "fixture.agent",
        "fixture.agent.type.step",
    )
    .unwrap()
}
fn policy(project: &semaprax::project::ProjectRevision) -> SourceLivePolicy {
    SourceLivePolicy {
        deployment_binding: DEPLOYMENT.into(),
        response_limit: 4096,
        ceiling: 9,
        reservation_units: 1,
        unit: "fixed_test_unit".into(),
        clock_domain: "source-migration-test-ms".into(),
        initial_millis: 0,
        deadline_millis: DEADLINE,
        max_total_steps: 10_000_000,
        program_root: Some(
            project
                .program_root()
                .unwrap()
                .program_root_digest()
                .to_owned(),
        ),
    }
}
fn budget() -> IterativeBudget {
    IterativeBudget {
        max_iterations: 9,
        max_stages: 96,
        ..IterativeBudget::default()
    }
}
fn task() -> LifecycleTask {
    LifecycleTask {
        objective: b"source migration task".to_vec(),
        budget: 12,
    }
}
fn endpoint<'a>(
    project: &'a semaprax::project::ProjectRevision,
    lifecycle: &'a CompiledIterativeLifecycle,
    policy: &'a SourceLivePolicy,
) -> SourceLiveMigrationEndpoint<'a> {
    SourceLiveMigrationEndpoint {
        project,
        source_path: "src/app.spx",
        agent_id: "fixture.agent",
        lifecycle,
        policy,
        budget: budget(),
    }
}

struct Suspended {
    project: Arc<semaprax::project::ProjectRevision>,
    life: CompiledIterativeLifecycle,
    policy: SourceLivePolicy,
    task: LifecycleTask,
    binding: SourceInvocationBinding,
    committed_stage_fuel: u64,
    store: Store,
}
fn suspended(a: &Fixture) -> Suspended {
    let project = retained(a);
    let life = compiled(&project);
    let policy = policy(&project);
    let task = task();
    let binding = policy.binding(&life, &task, budget()).unwrap();
    let mut store = Store::default();
    let mut model = Model::new(&life);
    let mut read = Read::default();
    let result = life
        .run_live_durable(
            SourceLiveRequest {
                task: &task,
                budget: budget(),
                policy: &policy,
                clock: &Clock::at(0),
                cancellation: &AgentCancellation::new(),
                checkpoint: None,
            },
            &mut model,
            &mut read,
            &mut store,
        )
        .unwrap();
    assert_eq!(
        result.checked_run.as_ref().unwrap().status(),
        IterativeStatus::Suspend
    );
    assert_eq!((model.calls, read.calls), (3, 3));
    Suspended {
        project,
        life,
        policy,
        task,
        binding,
        committed_stage_fuel: result.checkpoint.committed_stage_fuel(),
        store,
    }
}

#[test]
fn source_migration_consumes_hot_reload_selection_only_through_authenticated_handoff() {
    let a = durable::first();
    let b = durable::successor(&a, "State", "StateB", "b", &["marker"], false);
    let c = durable::successor(&b, "StateB", "StateC", "c", &["marker", "extra"], false);
    let old = suspended(&a);
    let project_b = retained(&b);
    let life_b = compiled(&project_b);
    let policy_b = policy(&project_b);

    let mut plan_session = HotReloadSession::new(
        Arc::clone(&old.project),
        PreparedProjectInterpreterOptions::default(),
    )
    .unwrap();
    plan_session
        .admit_candidate(Arc::clone(&project_b))
        .unwrap();
    let plan = plan_session.plan().unwrap();
    assert_eq!(
        plan.decision(),
        HotReloadDecision::EligibleSourceAgentCheckpointHandoff
    );
    let handoff = plan
        .source_agent_handoffs()
        .iter()
        .find(|handoff| handoff.agent_id() == "fixture.agent")
        .expect("the checked Project plan must select the source Agent");

    let prepared = prepare_source_live_migration_from_hot_reload_handoff(
        SourceLiveMigrationRequest {
            previous: endpoint(&old.project, &old.life, &old.policy),
            previous_binding: &old.binding,
            previous_checkpoint: &old.store.document,
            destination: endpoint(&project_b, &life_b, &policy_b),
            task: &old.task,
            migration_function: "fixture.agent.fn.migrate_b",
            max_migration_steps: 10_000,
            expected_handoff_digest: None,
        },
        handoff,
    )
    .unwrap();
    let mut destination_store = Store::default();
    let mut model = Model::new(&life_b);
    let mut read = Read::default();
    let outcome = prepared
        .run(
            &mut model,
            &mut read,
            &mut destination_store,
            &Clock::at(0),
            &AgentCancellation::new(),
        )
        .unwrap();
    assert_eq!(outcome.model_dispatches, 3);
    assert!(outcome
        .checked_run
        .unwrap()
        .stages()
        .iter()
        .all(|stage| stage.role() != "initialize"));

    let project_c = retained(&c);
    let mut wrong_plan_session = HotReloadSession::new(
        Arc::clone(&project_b),
        PreparedProjectInterpreterOptions::default(),
    )
    .unwrap();
    wrong_plan_session
        .admit_candidate(Arc::clone(&project_c))
        .unwrap();
    let wrong_plan = wrong_plan_session.plan().unwrap();
    let wrong_handoff = wrong_plan
        .source_agent_handoffs()
        .iter()
        .find(|handoff| handoff.agent_id() == "fixture.agent")
        .expect("the checked Project plan must select the source Agent");
    let checkpoint_before_refusal = destination_store.document.clone();
    let error = match prepare_source_live_migration_from_hot_reload_handoff(
        SourceLiveMigrationRequest {
            previous: endpoint(&old.project, &old.life, &old.policy),
            previous_binding: &old.binding,
            previous_checkpoint: &old.store.document,
            destination: endpoint(&project_b, &life_b, &policy_b),
            task: &old.task,
            migration_function: "fixture.agent.fn.migrate_b",
            max_migration_steps: 10_000,
            expected_handoff_digest: None,
        },
        wrong_handoff,
    ) {
        Ok(_) => panic!("a handoff row for a different retained Project pair must refuse"),
        Err(error) => error,
    };
    assert_eq!(error.diagnostics[0].code, "SPX-G582");
    assert_eq!(
        error.diagnostics[0].message,
        "Agent iterative lifecycle invariant failed: migration.hot_reload_selection"
    );
    assert_eq!(error.model_dispatches, 0);
    assert_eq!(destination_store.document, checkpoint_before_refusal);
}

#[test]
fn source_agent_handoff_supervisor_activates_once_and_terminalizes_lost_ack() {
    let a = durable::first();
    let b = durable::successor(&a, "State", "StateB", "b", &["marker"], false);
    let old = suspended(&a);
    let project_b = retained(&b);
    let life_b = compiled(&project_b);
    let policy_b = policy(&project_b);
    let clock = Clock::at(0);
    let cancel = AgentCancellation::new();
    let request = || SourceLiveMigrationRequest {
        previous: endpoint(&old.project, &old.life, &old.policy),
        previous_binding: &old.binding,
        previous_checkpoint: &old.store.document,
        destination: endpoint(&project_b, &life_b, &policy_b),
        task: &old.task,
        migration_function: "fixture.agent.fn.migrate_b",
        max_migration_steps: 10_000,
        expected_handoff_digest: None,
    };
    let plan = |session: &mut HotReloadSession| {
        session.admit_candidate(Arc::clone(&project_b)).unwrap();
        let plan = session.plan().unwrap();
        assert_eq!(
            plan.decision(),
            HotReloadDecision::EligibleSourceAgentCheckpointHandoff
        );
        plan
    };

    #[derive(Clone, Copy)]
    enum Transition {
        WaitForCheckpoint,
        Activate,
        Refuse,
        Uncertain,
    }

    // These rows share the same compiler-derived handoff and exercise every
    // source-Agent coordinator state: checkpoint waiting, authenticated
    // activation, a clean migration refusal, and an ambiguous journal
    // acknowledgement.
    for transition in [
        Transition::WaitForCheckpoint,
        Transition::Activate,
        Transition::Refuse,
        Transition::Uncertain,
    ] {
        let mut session = HotReloadSession::new(
            Arc::clone(&old.project),
            PreparedProjectInterpreterOptions::default(),
        )
        .unwrap();
        let mut store = match transition {
            Transition::Uncertain => Store {
                lose_ack_on: Some(2),
                ..Store::default()
            },
            Transition::WaitForCheckpoint | Transition::Activate | Transition::Refuse => {
                Store::default()
            }
        };
        let mut model = Model::new(&life_b);
        let mut read = Read::default();
        let mut migration_request = request();
        if matches!(transition, Transition::Refuse) {
            migration_request.migration_function = "fixture.agent.fn.migrate_c";
        }
        let selected_plan = plan(&mut session);
        if matches!(transition, Transition::WaitForCheckpoint) {
            let handoff = session
                .wait_for_source_agent_handoff(&selected_plan, "fixture.agent")
                .unwrap();
            assert_eq!(handoff.agent_id(), "fixture.agent");
            assert_eq!(
                session.source_agent_handoff_status(),
                HotReloadSourceAgentHandoffStatus::WaitingForCheckpoint
            );
            assert_eq!(
                session.observation().lifecycle(),
                semaprax::project::HotReloadLifecycle::WaitingForSafePoint
            );
            assert_eq!(session.generation(), 0);
            assert_eq!(
                session.active_project_revision(),
                old.project.project_revision()
            );
            assert!(session.retained_source_agent_binding().is_none());
            assert_eq!((model.calls, read.calls), (0, 0));
            assert!(store.document.is_empty());
            continue;
        }
        let result = run_source_live_migration_from_hot_reload_session(
            &mut session,
            selected_plan,
            migration_request,
            &mut model,
            &mut read,
            &mut store,
            &clock,
            &cancel,
        );

        match transition {
            Transition::WaitForCheckpoint => unreachable!("handled before migration"),
            Transition::Activate => {
                let outcome = result.unwrap();
                assert_eq!(
                    session.source_agent_handoff_status(),
                    HotReloadSourceAgentHandoffStatus::Activated
                );
                assert_eq!(
                    session.observation().lifecycle(),
                    semaprax::project::HotReloadLifecycle::Activated
                );
                assert_eq!(session.generation(), 1);
                assert_eq!(
                    session.active_project_revision(),
                    project_b.project_revision()
                );
                let binding = session
                    .retained_source_agent_binding()
                    .expect("acknowledged migration retains its destination binding");
                let recovered = recover_source_checkpoint(&store.document, binding).unwrap();
                assert_eq!(recovered.generation(), outcome.checkpoint.generation());
                assert_eq!(
                    recovered.last_checked_millis(),
                    outcome.checkpoint.last_checked_millis()
                );
                assert_eq!((model.calls, read.calls), (3, 3));
                assert!(outcome
                    .checked_run
                    .as_ref()
                    .unwrap()
                    .stages()
                    .iter()
                    .all(|stage| stage.role() != "initialize"));
                assert!(session
                    .execute_entry(
                        &semaprax::project::PreparedProjectExecutionOptions::default(),
                        &semaprax::project::ProjectExecutionCancellation::new(),
                    )
                    .is_err());
            }
            Transition::Refuse => {
                assert!(matches!(
                    result,
                    Err(SourceAgentHandoffFailure::Migration(_))
                ));
                assert_eq!(
                    session.source_agent_handoff_status(),
                    HotReloadSourceAgentHandoffStatus::MigrationRequired
                );
                assert_eq!(
                    session.observation().lifecycle(),
                    semaprax::project::HotReloadLifecycle::Refused
                );
                assert_eq!(session.generation(), 0);
                assert_eq!(
                    session.active_project_revision(),
                    old.project.project_revision()
                );
                assert!(session.retained_source_agent_binding().is_none());
                assert_eq!((model.calls, read.calls), (0, 0));
                assert!(store.document.is_empty());
            }
            Transition::Uncertain => {
                assert!(matches!(
                    result,
                    Err(SourceAgentHandoffFailure::Migration(_))
                ));
                assert_eq!(
                    session.source_agent_handoff_status(),
                    HotReloadSourceAgentHandoffStatus::TerminalUncertainty
                );
                assert_eq!(
                    session.observation().lifecycle(),
                    semaprax::project::HotReloadLifecycle::TerminalUncertainty
                );
                assert!(session.terminal());
                assert_eq!(session.generation(), 0);
                assert!(session.retained_source_agent_binding().is_none());
                assert_eq!((model.calls, read.calls), (0, 0));
                assert!(store.document.contains("migration_evaluation_intent"));
            }
        }
    }
}

#[test]
#[allow(clippy::result_large_err)]
fn source_migration_refuses_wrong_project_task_function_handoff_and_limits_before_dispatch() {
    let a = durable::first();
    let b = durable::successor(&a, "State", "StateB", "b", &["marker"], false);
    let old = suspended(&a);
    let project_b = retained(&b);
    let life_b = compiled(&project_b);
    let policy_b = policy(&project_b);
    let prepare = |task: &LifecycleTask,
                   policy_b: &SourceLivePolicy,
                   function: &str,
                   checkpoint: &str,
                   expected: Option<&str>| {
        prepare_source_live_migration(SourceLiveMigrationRequest {
            previous: endpoint(&old.project, &old.life, &old.policy),
            previous_binding: &old.binding,
            previous_checkpoint: checkpoint,
            destination: endpoint(&project_b, &life_b, policy_b),
            task,
            migration_function: function,
            max_migration_steps: 10_000,
            expected_handoff_digest: expected,
        })
        .map(|_| ())
    };
    assert!(prepare(
        &old.task,
        &policy_b,
        "fixture.agent.fn.migrate_b",
        &old.store.document,
        None
    )
    .is_ok());
    let changed_task = LifecycleTask {
        objective: b"other task".to_vec(),
        budget: old.task.budget,
    };
    assert!(prepare(
        &changed_task,
        &policy_b,
        "fixture.agent.fn.migrate_b",
        &old.store.document,
        None
    )
    .is_err());
    assert!(prepare(
        &old.task,
        &policy_b,
        "fixture.agent.fn.migrate_c",
        &old.store.document,
        None
    )
    .is_err());
    assert!(prepare(
        &old.task,
        &policy_b,
        "fixture.agent.fn.migrate_b",
        &old.store.document,
        Some("sha256:0000000000000000000000000000000000000000000000000000000000000000")
    )
    .is_err());
    let mut stale = old.store.document.clone();
    stale.push(' ');
    assert!(prepare(
        &old.task,
        &policy_b,
        "fixture.agent.fn.migrate_b",
        &stale,
        None
    )
    .is_err());
    let mut wrong_root = policy_b.clone();
    wrong_root.program_root = old.policy.program_root.clone();
    assert!(prepare(
        &old.task,
        &wrong_root,
        "fixture.agent.fn.migrate_b",
        &old.store.document,
        None
    )
    .is_err());
    let mut cheap = policy_b.clone();
    cheap.ceiling = 2;
    assert!(prepare(
        &old.task,
        &cheap,
        "fixture.agent.fn.migrate_b",
        &old.store.document,
        None
    )
    .is_err());
    let mut extended = policy_b.clone();
    extended.deadline_millis = DEADLINE + 1;
    assert!(prepare(
        &old.task,
        &extended,
        "fixture.agent.fn.migrate_b",
        &old.store.document,
        None
    )
    .is_err());
    let mut changed_unit = policy_b.clone();
    changed_unit.unit = "other_unit".into();
    assert!(prepare(
        &old.task,
        &changed_unit,
        "fixture.agent.fn.migrate_b",
        &old.store.document,
        None
    )
    .is_err());
    let mut insufficient_fuel = policy_b.clone();
    insufficient_fuel.max_total_steps = 20_000;
    assert!(prepare(
        &old.task,
        &insufficient_fuel,
        "fixture.agent.fn.migrate_b",
        &old.store.document,
        None
    )
    .is_err());
    let mut raised_fuel_ceiling = policy_b.clone();
    raised_fuel_ceiling.max_total_steps += 1;
    assert!(prepare(
        &old.task,
        &raised_fuel_ceiling,
        "fixture.agent.fn.migrate_b",
        &old.store.document,
        None
    )
    .is_err());
    for destination_budget in [
        IterativeBudget {
            max_iterations: budget().max_iterations + 1,
            ..budget()
        },
        IterativeBudget {
            max_stages: budget().max_stages + 1,
            ..budget()
        },
        IterativeBudget {
            max_steps_per_stage: budget().max_steps_per_stage + 1,
            ..budget()
        },
    ] {
        let mut destination = endpoint(&project_b, &life_b, &policy_b);
        destination.budget = destination_budget;
        assert!(prepare_source_live_migration(SourceLiveMigrationRequest {
            previous: endpoint(&old.project, &old.life, &old.policy),
            previous_binding: &old.binding,
            previous_checkpoint: &old.store.document,
            destination,
            task: &old.task,
            migration_function: "fixture.agent.fn.migrate_b",
            max_migration_steps: 10_000,
            expected_handoff_digest: None,
        })
        .is_err());
    }
}

#[test]
fn source_migration_lost_ack_reloads_latest_and_charges_retry_without_model_redispatch() {
    let a = durable::first();
    let b = durable::successor(&a, "State", "StateB", "b", &["marker"], false);
    let old = suspended(&a);
    let project_b = retained(&b);
    let life_b = compiled(&project_b);
    let policy_b = policy(&project_b);
    let clock = Clock::at(0);
    let cancel = AgentCancellation::new();
    let prepare = || {
        prepare_source_live_migration(SourceLiveMigrationRequest {
            previous: endpoint(&old.project, &old.life, &old.policy),
            previous_binding: &old.binding,
            previous_checkpoint: &old.store.document,
            destination: endpoint(&project_b, &life_b, &policy_b),
            task: &old.task,
            migration_function: "fixture.agent.fn.migrate_b",
            max_migration_steps: 10_000,
            expected_handoff_digest: None,
        })
        .unwrap()
    };
    let mut store = Store {
        lose_ack_on: Some(2),
        ..Store::default()
    };
    let mut model = Model::new(&life_b);
    let mut read = Read::default();
    let failed = prepare()
        .run(&mut model, &mut read, &mut store, &clock, &cancel)
        .err()
        .expect("intent acknowledgement loss must stop");
    assert!(failed.journal_error.is_some());
    assert_eq!((model.calls, read.calls), (0, 0));
    assert!(store.document.contains("migration_evaluation_intent"));
    let latest = store.document.clone();
    let resumed = prepare()
        .with_checkpoint(&latest)
        .run(&mut model, &mut read, &mut store, &clock, &cancel)
        .unwrap();
    assert_eq!(
        resumed.checked_run.as_ref().unwrap().status(),
        IterativeStatus::Complete
    );
    assert_eq!((model.calls, read.calls), (3, 3));
    assert_eq!(
        store
            .document
            .matches("migration_evaluation_intent")
            .count(),
        2
    );
    assert!(resumed.checkpoint.committed_stage_fuel() >= old.committed_stage_fuel + 40_000);

    // A later lost model-intent ACK is uncertain, unlike a pure migration
    // intent. Reloading it must never call either host boundary again.
    let mut uncertain_store = Store {
        lose_ack_on: Some(7),
        ..Store::default()
    };
    let mut uncertain_model = Model::new(&life_b);
    let mut uncertain_read = Read::default();
    let _ = prepare()
        .run(
            &mut uncertain_model,
            &mut uncertain_read,
            &mut uncertain_store,
            &clock,
            &cancel,
        )
        .err()
        .unwrap();
    assert_eq!((uncertain_model.calls, uncertain_read.calls), (0, 0));
    let latest_uncertain = uncertain_store.document.clone();
    assert!(latest_uncertain.contains("attempt_intent"));
    let mut forbidden_model = Model::new(&life_b);
    let mut forbidden_read = Read::default();
    let recovery = prepare().with_checkpoint(&latest_uncertain).run(
        &mut forbidden_model,
        &mut forbidden_read,
        &mut uncertain_store,
        &clock,
        &cancel,
    );
    assert!(recovery.is_err());
    assert_eq!((forbidden_model.calls, forbidden_read.calls), (0, 0));
}

#[test]
fn source_migration_deadline_after_intent_ack_never_exposes_runnable_seed() {
    let a = durable::first();
    let b = durable::successor(&a, "State", "StateB", "b", &["marker"], false);
    let old = suspended(&a);
    let project_b = retained(&b);
    let life_b = compiled(&project_b);
    let policy_b = policy(&project_b);
    let clock = Clock::at(0);
    let mut store = Store {
        advance_on: Some((2, clock.0.clone(), DEADLINE)),
        ..Store::default()
    };
    let prepared = prepare_source_live_migration(SourceLiveMigrationRequest {
        previous: endpoint(&old.project, &old.life, &old.policy),
        previous_binding: &old.binding,
        previous_checkpoint: &old.store.document,
        destination: endpoint(&project_b, &life_b, &policy_b),
        task: &old.task,
        migration_function: "fixture.agent.fn.migrate_b",
        max_migration_steps: 10_000,
        expected_handoff_digest: None,
    })
    .unwrap();
    let mut model = Model::new(&life_b);
    let mut read = Read::default();
    let failure = prepared
        .run(
            &mut model,
            &mut read,
            &mut store,
            &clock,
            &AgentCancellation::new(),
        )
        .err()
        .unwrap();
    assert_eq!(
        failure.selected,
        Some(SourceTerminalStatus::DeadlineExceeded)
    );
    assert_eq!((model.calls, read.calls), (0, 0));
    assert!(store.document.contains("migration_evaluation_failed"));
    assert!(!store.document.contains("migration_evaluation_settled"));
    assert!(!store.document.contains("run_opened"));
}

#[test]
fn source_migration_cancellation_after_intent_ack_retains_charge_without_dispatch() {
    let a = durable::first();
    let b = durable::successor(&a, "State", "StateB", "b", &["marker"], false);
    let old = suspended(&a);
    let project_b = retained(&b);
    let life_b = compiled(&project_b);
    let policy_b = policy(&project_b);
    let cancellation = AgentCancellation::new();
    let mut store = Store {
        cancel_on: Some((2, cancellation.clone())),
        ..Store::default()
    };
    let prepared = prepare_source_live_migration(SourceLiveMigrationRequest {
        previous: endpoint(&old.project, &old.life, &old.policy),
        previous_binding: &old.binding,
        previous_checkpoint: &old.store.document,
        destination: endpoint(&project_b, &life_b, &policy_b),
        task: &old.task,
        migration_function: "fixture.agent.fn.migrate_b",
        max_migration_steps: 10_000,
        expected_handoff_digest: None,
    })
    .unwrap();
    let binding = prepared.binding().clone();
    let mut model = Model::new(&life_b);
    let mut read = Read::default();
    let failure = prepared
        .run(
            &mut model,
            &mut read,
            &mut store,
            &Clock::at(0),
            &cancellation,
        )
        .err()
        .unwrap();
    assert_eq!(failure.selected, Some(SourceTerminalStatus::Cancelled));
    assert_eq!((model.calls, read.calls), (0, 0));
    assert!(store.document.contains("migration_evaluation_failed"));
    assert!(!store.document.contains("migration_evaluation_settled"));
    assert!(!store.document.contains("run_opened"));
    assert_eq!(
        recover_source_checkpoint(&store.document, &binding)
            .unwrap()
            .committed_stage_fuel(),
        old.committed_stage_fuel + 20_000
    );
}

#[test]
fn recovered_settled_state_is_rechecked_against_destination_schema_before_dispatch() {
    let a = durable::first();
    let b = durable::successor(&a, "State", "StateB", "b", &["marker"], false);
    let old = suspended(&a);
    let project_b = retained(&b);
    let life_b = compiled(&project_b);
    let policy_b = policy(&project_b);
    let prepared = prepare_source_live_migration(SourceLiveMigrationRequest {
        previous: endpoint(&old.project, &old.life, &old.policy),
        previous_binding: &old.binding,
        previous_checkpoint: &old.store.document,
        destination: endpoint(&project_b, &life_b, &policy_b),
        task: &old.task,
        migration_function: "fixture.agent.fn.migrate_b",
        max_migration_steps: 10_000,
        expected_handoff_digest: None,
    })
    .unwrap();
    let binding = prepared.binding().clone();
    let malformed_state = b"{}".to_vec();
    let mut hasher = Sha256::new();
    hasher.update(b"semaprax.live-invocation.source-migrated-state.v3\0");
    hasher.update(&malformed_state);
    let digest = format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    );
    let mut store = Store::default();
    let mut sink = SourceCheckpointSink::new(&mut store, binding.clone());
    sink.append_at(
        SourceJournalEntry::MigrationOpened {
            handoff_digest: prepared.handoff_digest().to_owned(),
        },
        0,
    )
    .unwrap();
    sink.append_at(
        SourceJournalEntry::MigrationEvaluationIntent {
            attempt: 0,
            fuel: 20_000,
        },
        0,
    )
    .unwrap();
    sink.append_at(
        SourceJournalEntry::MigrationEvaluationSettled {
            attempt: 0,
            state_digest: digest,
            state: malformed_state,
        },
        0,
    )
    .unwrap();
    drop(sink);
    let latest = store.document.clone();
    assert!(recover_source_checkpoint(&latest, &binding).is_ok());
    let mut model = Model::new(&life_b);
    let mut read = Read::default();
    let failure = prepared
        .with_checkpoint(&latest)
        .run(
            &mut model,
            &mut read,
            &mut store,
            &Clock::at(0),
            &AgentCancellation::new(),
        )
        .err()
        .unwrap();
    assert_eq!((model.calls, read.calls), (0, 0));
    assert!(failure
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("migration.state_shape")));
    assert_eq!(store.document, latest);
}

#[test]
fn failed_checked_evaluation_samples_clock_and_regression_keeps_intent_charged() {
    let a = durable::first();
    let b = durable::successor(&a, "State", "StateB", "b", &["marker"], false);
    let old = suspended(&a);
    let project_b = retained(&b);
    let life_b = compiled(&project_b);
    let policy_b = policy(&project_b);
    let prepare = || {
        prepare_source_live_migration(SourceLiveMigrationRequest {
            previous: endpoint(&old.project, &old.life, &old.policy),
            previous_binding: &old.binding,
            previous_checkpoint: &old.store.document,
            destination: endpoint(&project_b, &life_b, &policy_b),
            task: &old.task,
            migration_function: "fixture.agent.fn.migrate_b",
            max_migration_steps: 1,
            expected_handoff_digest: None,
        })
        .unwrap()
    };
    let normal_clock = RegressingClock {
        reads: Cell::new(0),
        regress_at: usize::MAX,
    };
    let mut normal_store = Store::default();
    let mut normal_model = Model::new(&life_b);
    let mut normal_read = Read::default();
    let normal = prepare().run(
        &mut normal_model,
        &mut normal_read,
        &mut normal_store,
        &normal_clock,
        &AgentCancellation::new(),
    );
    assert!(normal.is_err());
    assert!(normal_store
        .document
        .contains("migration_evaluation_failed"));
    assert_eq!((normal_model.calls, normal_read.calls), (0, 0));
    let regress_at = normal_clock.reads.get() - 1;
    let regressed_clock = RegressingClock {
        reads: Cell::new(0),
        regress_at,
    };
    let mut regressed_store = Store::default();
    let mut regressed_model = Model::new(&life_b);
    let mut regressed_read = Read::default();
    let prepared = prepare();
    let binding = prepared.binding().clone();
    let failure = prepared
        .run(
            &mut regressed_model,
            &mut regressed_read,
            &mut regressed_store,
            &regressed_clock,
            &AgentCancellation::new(),
        )
        .err()
        .unwrap();
    assert_eq!(failure.journal_error, Some(SourceJournalError::Time));
    assert_eq!(failure.selected, Some(SourceTerminalStatus::Rejected));
    assert_eq!((regressed_model.calls, regressed_read.calls), (0, 0));
    assert!(regressed_store
        .document
        .contains("migration_evaluation_intent"));
    assert!(!regressed_store
        .document
        .contains("migration_evaluation_failed"));
    let charged = recover_source_checkpoint(&regressed_store.document, &binding).unwrap();
    assert_eq!(charged.committed_stage_fuel(), old.committed_stage_fuel + 2);
}

#[test]
fn actual_source_a_to_b_to_c_replays_selected_handoffs_and_preserves_accumulated_state() {
    let a = durable::first();
    let b = durable::successor(&a, "State", "StateB", "b", &["marker"], true);
    let c = durable::successor(&b, "StateB", "StateC", "c", &["marker", "extra"], false);
    let (project_a, project_b, project_c) = (retained(&a), retained(&b), retained(&c));
    let (life_a, life_b, life_c) = (
        compiled(&project_a),
        compiled(&project_b),
        compiled(&project_c),
    );
    let (policy_a, policy_b, policy_c) =
        (policy(&project_a), policy(&project_b), policy(&project_c));
    let task = task();
    let clock = Clock::at(0);
    let cancel = AgentCancellation::new();
    let mut a_store = Store::default();
    let mut a_model = Model::new(&life_a);
    let mut a_read = Read::default();
    let a_run = life_a
        .run_live_durable(
            SourceLiveRequest {
                task: &task,
                budget: budget(),
                policy: &policy_a,
                clock: &clock,
                cancellation: &cancel,
                checkpoint: None,
            },
            &mut a_model,
            &mut a_read,
            &mut a_store,
        )
        .unwrap();
    assert_eq!(
        a_run.checked_run.as_ref().unwrap().status(),
        IterativeStatus::Suspend
    );
    assert_eq!((a_model.calls, a_read.calls), (3, 3));
    let binding_a = policy_a.binding(&life_a, &task, budget()).unwrap();
    let mut b_store = Store::default();
    let prepared_b = {
        let mut session = HotReloadSession::new(
            Arc::clone(&project_a),
            PreparedProjectInterpreterOptions::default(),
        )
        .unwrap();
        session.admit_candidate(Arc::clone(&project_b)).unwrap();
        let plan = session.plan().unwrap();
        assert_eq!(
            plan.decision(),
            HotReloadDecision::EligibleSourceAgentCheckpointHandoff
        );
        let selection = plan
            .source_agent_handoffs()
            .iter()
            .find(|selection| selection.agent_id() == "fixture.agent")
            .expect("A-to-B plan must select the retained Agent");
        prepare_source_live_migration_from_hot_reload_handoff(
            SourceLiveMigrationRequest {
                previous: endpoint(&project_a, &life_a, &policy_a),
                previous_binding: &binding_a,
                previous_checkpoint: &a_store.document,
                destination: endpoint(&project_b, &life_b, &policy_b),
                task: &task,
                migration_function: "fixture.agent.fn.migrate_b",
                max_migration_steps: 10_000,
                expected_handoff_digest: None,
            },
            selection,
        )
        .unwrap()
    };
    let binding_b: SourceInvocationBinding = prepared_b.binding().clone();
    let mut b_model = Model::new(&life_b);
    let mut b_read = Read::default();
    let b_run = prepared_b
        .run(&mut b_model, &mut b_read, &mut b_store, &clock, &cancel)
        .unwrap();
    assert_eq!(
        b_run.checked_run.as_ref().unwrap().status(),
        IterativeStatus::Suspend
    );
    assert_eq!((b_model.calls, b_read.calls), (3, 3));
    let RetainedValue::Record(first_b_state) = &b_model.states[0] else {
        panic!("destination model must receive a record State");
    };
    assert_eq!(first_b_state.record.as_str(), "fixture.agent.type.state_b");
    assert!(first_b_state.fields.iter().any(|field| {
        field.field.as_str() == "fixture.agent.type.state_b.marker"
            && field.value == RetainedValue::I64(7)
    }));
    assert!(b_model
        .schemas
        .iter()
        .all(|digest| digest == life_b.proposal_schema().schema().digest()));
    assert!(b_run
        .checked_run
        .as_ref()
        .unwrap()
        .stages()
        .iter()
        .all(|stage| stage.role() != "initialize"));
    assert_eq!(b_run.checkpoint.committed_reserved_units(), 6);
    assert_eq!(b_run.checkpoint.deadline_millis(), DEADLINE);
    assert_eq!(b_run.checkpoint.last_checked_millis(), 0);
    assert!(
        b_run.checkpoint.committed_stage_fuel() >= a_run.checkpoint.committed_stage_fuel() + 20_000
    );
    let b_terminal = b_store.document.clone();
    let replay_b = prepare_source_live_migration(SourceLiveMigrationRequest {
        previous: endpoint(&project_a, &life_a, &policy_a),
        previous_binding: &binding_a,
        previous_checkpoint: &a_store.document,
        destination: endpoint(&project_b, &life_b, &policy_b),
        task: &task,
        migration_function: "fixture.agent.fn.migrate_b",
        max_migration_steps: 10_000,
        expected_handoff_digest: None,
    })
    .unwrap()
    .with_checkpoint(&b_terminal);
    let mut no_b_model = Model::new(&life_b);
    let mut no_b_read = Read::default();
    let recovered_b = replay_b
        .run(
            &mut no_b_model,
            &mut no_b_read,
            &mut b_store,
            &clock,
            &cancel,
        )
        .unwrap();
    assert!(recovered_b.checked_run.is_none());
    assert_eq!((no_b_model.calls, no_b_read.calls), (0, 0));
    assert_eq!(recovered_b.checkpoint.committed_reserved_units(), 6);
    let mut c_store = Store::default();
    let prepared_c = {
        let mut session = HotReloadSession::new(
            Arc::clone(&project_b),
            PreparedProjectInterpreterOptions::default(),
        )
        .unwrap();
        session.admit_candidate(Arc::clone(&project_c)).unwrap();
        let plan = session.plan().unwrap();
        assert_eq!(
            plan.decision(),
            HotReloadDecision::EligibleSourceAgentCheckpointHandoff
        );
        let selection = plan
            .source_agent_handoffs()
            .iter()
            .find(|selection| selection.agent_id() == "fixture.agent")
            .expect("B-to-C plan must select the retained Agent");
        prepare_source_live_migration_from_hot_reload_handoff(
            SourceLiveMigrationRequest {
                previous: endpoint(&project_b, &life_b, &policy_b),
                previous_binding: &binding_b,
                previous_checkpoint: &b_store.document,
                destination: endpoint(&project_c, &life_c, &policy_c),
                task: &task,
                migration_function: "fixture.agent.fn.migrate_c",
                max_migration_steps: 10_000,
                expected_handoff_digest: None,
            },
            selection,
        )
        .unwrap()
    };
    let mut c_model = Model::new(&life_c);
    let mut c_read = Read::default();
    let c_run = prepared_c
        .run(&mut c_model, &mut c_read, &mut c_store, &clock, &cancel)
        .unwrap();
    assert_eq!(
        c_run.checked_run.as_ref().unwrap().status(),
        IterativeStatus::Complete
    );
    assert_eq!((c_model.calls, c_read.calls), (3, 3));
    let RetainedValue::Record(first_c_state) = &c_model.states[0] else {
        panic!("destination model must receive a record State");
    };
    assert_eq!(first_c_state.record.as_str(), "fixture.agent.type.state_c");
    for (identity, value) in [
        ("fixture.agent.type.state_c.marker", 7),
        ("fixture.agent.type.state_c.extra", 9),
    ] {
        assert!(first_c_state.fields.iter().any(|field| {
            field.field.as_str() == identity && field.value == RetainedValue::I64(value)
        }));
    }
    assert!(c_model
        .schemas
        .iter()
        .all(|digest| digest == life_c.proposal_schema().schema().digest()));
    assert!(c_run
        .checked_run
        .as_ref()
        .unwrap()
        .stages()
        .iter()
        .all(|stage| stage.role() != "initialize"));
    assert_eq!(c_run.checkpoint.committed_reserved_units(), 9);
    assert_eq!(c_run.checkpoint.deadline_millis(), DEADLINE);
    assert_eq!(c_run.checkpoint.last_checked_millis(), 0);
    assert!(
        c_run.checkpoint.committed_stage_fuel() >= b_run.checkpoint.committed_stage_fuel() + 20_000
    );
    let terminal = c_store.document.clone();
    let replay = prepare_source_live_migration(SourceLiveMigrationRequest {
        previous: endpoint(&project_b, &life_b, &policy_b),
        previous_binding: &binding_b,
        previous_checkpoint: &b_store.document,
        destination: endpoint(&project_c, &life_c, &policy_c),
        task: &task,
        migration_function: "fixture.agent.fn.migrate_c",
        max_migration_steps: 10_000,
        expected_handoff_digest: None,
    })
    .unwrap()
    .with_checkpoint(&terminal);
    clock.0.set(DEADLINE);
    let mut no_model = Model::new(&life_c);
    let mut no_read = Read::default();
    let result = replay
        .run(&mut no_model, &mut no_read, &mut c_store, &clock, &cancel)
        .unwrap();
    assert!(result.checked_run.is_none());
    assert_eq!((no_model.calls, no_read.calls), (0, 0));
    assert_eq!(result.checkpoint.committed_reserved_units(), 9);
}
