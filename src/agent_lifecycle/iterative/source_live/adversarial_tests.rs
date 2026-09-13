//! Crash, clock and capacity oracles for the source-live route.
//! The scripted source still acknowledges real intent/settlement entries.

use super::*;
use crate::live_invocation::source_journal::{
    SourceJournalError, SourceStageRole, SourceStopReason, SourceStopStatus, SourceTerminalStatus,
};
use std::{cell::Cell, rc::Rc};

fn scripted(compiled: &CompiledIterativeLifecycle, count: usize) -> Source {
    Source {
        responses: vec![Source::valid_response(compiled); count],
        calls: 0,
        deployment: DEPLOYMENT.into(),
        response_limit: 4096,
        reservation_units: 1,
    }
}

fn stage(
    turn: u32,
    attempt: Option<u32>,
    role: SourceStageRole,
    fuel: usize,
) -> SourceJournalEntry {
    SourceJournalEntry::StageReservation {
        turn,
        attempt,
        role,
        fuel,
    }
}

fn prefix(
    compiled: &CompiledIterativeLifecycle,
    policy: &SourceLivePolicy,
    budget: IterativeBudget,
    entries: impl IntoIterator<Item = SourceJournalEntry>,
) -> String {
    let binding = policy.binding(compiled, &task(), budget).unwrap();
    let mut store = Store::default();
    {
        let mut sink = SourceCheckpointSink::new(&mut store, binding);
        for (index, entry) in entries.into_iter().enumerate() {
            sink.append_at(entry, index as i64 + 1).unwrap();
        }
    }
    store.document
}

fn observed_prefix(fuel: usize) -> Vec<SourceJournalEntry> {
    let hash = source_response_digest(b"fixture-hash");
    vec![
        SourceJournalEntry::RunOpened,
        stage(0, None, SourceStageRole::Initialize, fuel),
        stage(0, None, SourceStageRole::Observe, fuel),
        SourceJournalEntry::TurnObserved {
            turn: 0,
            state: hash.clone(),
            observation: hash.clone(),
            feedback: hash,
        },
    ]
}

fn identity_intent(
    compiled: &CompiledIterativeLifecycle,
    policy: &SourceLivePolicy,
    budget: IterativeBudget,
) -> SourceJournalEntry {
    let binding = policy.binding(compiled, &task(), budget).unwrap();
    let request_digest = compiled.source_revision().to_owned();
    let prompt_digest = compiled.proposal_schema().schema().digest().to_owned();
    SourceJournalEntry::AttemptIntent {
        turn: 0,
        attempt: 0,
        attempt_digest: binding.attempt_digest(0, 0, &request_digest, &prompt_digest, 1),
        request_digest,
        prompt_digest,
        request_bytes: 1,
        reserved_units: 1,
        response_limit: 4096,
    }
}

fn resume<'a>(
    task: &'a LifecycleTask,
    policy: &'a SourceLivePolicy,
    budget: IterativeBudget,
    clock: &'a Clock,
    cancellation: &'a AgentCancellation,
    checkpoint: &'a str,
) -> SourceLiveRequest<'a> {
    SourceLiveRequest {
        task,
        budget,
        policy,
        clock,
        cancellation,
        checkpoint: Some(checkpoint),
    }
}

#[test]
fn expired_stop_prefix_terminalizes_without_resuming_a_stage_or_model() {
    let compiled = lifecycle();
    let task = task();
    let policy = policy(1);
    let budget = IterativeBudget::default();
    let checkpoint = prefix(
        &compiled,
        &policy,
        budget,
        [
            SourceJournalEntry::RunOpened,
            SourceJournalEntry::Stop {
                turn: None,
                attempt: None,
                status: SourceStopStatus::DeadlineExceeded,
                reason: SourceStopReason::DeadlineExceeded,
            },
        ],
    );
    let clock = Clock {
        now: policy.deadline_millis + 1,
    };
    let cancellation = AgentCancellation::default();
    let mut source = scripted(&compiled, 0);
    let mut read = Read { calls: 0 };
    let mut store = Store::default();
    let outcome = compiled
        .run_live_durable(
            resume(&task, &policy, budget, &clock, &cancellation, &checkpoint),
            &mut source,
            &mut read,
            &mut store,
        )
        .expect("an acknowledged Stop can acquire its terminal receipt after expiry");
    assert!(outcome.checked_run.is_none());
    assert_eq!(
        outcome.checkpoint.terminal_snapshot().unwrap().status(),
        SourceTerminalStatus::DeadlineExceeded
    );
    assert_eq!((source.calls, read.calls), (0, 0));
    assert_eq!(outcome.checkpoint.generation(), 3);
}

#[test]
fn expired_terminal_receipt_is_read_only_and_makes_no_calls() {
    let compiled = lifecycle();
    let task = task();
    let policy = policy(3);
    let cancellation = AgentCancellation::default();
    let mut first_source = scripted(&compiled, 3);
    let mut first_read = Read { calls: 0 };
    let mut first_store = Store::default();
    compiled
        .run_live_durable(
            request(&task, &policy, &Clock { now: 1 }, &cancellation),
            &mut first_source,
            &mut first_read,
            &mut first_store,
        )
        .expect("fixture completes before expiry");
    let checkpoint = first_store.document;
    let mut source = scripted(&compiled, 0);
    let mut read = Read { calls: 0 };
    let mut store = Store {
        fail: true,
        ..Store::default()
    };
    let clock = Clock {
        now: policy.deadline_millis + 1,
    };
    let outcome = compiled
        .run_live_durable(
            resume(
                &task,
                &policy,
                IterativeBudget::default(),
                &clock,
                &cancellation,
                &checkpoint,
            ),
            &mut source,
            &mut read,
            &mut store,
        )
        .expect("terminal receipt needs no continuation clock or store write");
    assert!(outcome.checked_run.is_none());
    assert_eq!(
        outcome.checkpoint.terminal_snapshot().unwrap().status(),
        SourceTerminalStatus::Complete
    );
    assert_eq!((source.calls, read.calls), (0, 0));
}

#[test]
fn regressed_restart_clock_refuses_before_any_physical_call() {
    let compiled = lifecycle();
    let task = task();
    let policy = policy(1);
    let budget = IterativeBudget::default();
    let binding = policy.binding(&compiled, &task, budget).unwrap();
    let mut initial_store = Store::default();
    {
        let mut sink = SourceCheckpointSink::new(&mut initial_store, binding);
        sink.append_at(SourceJournalEntry::RunOpened, 5).unwrap();
    }
    let checkpoint = initial_store.document;
    let clock = Clock { now: 4 };
    let cancellation = AgentCancellation::default();
    let mut source = scripted(&compiled, 0);
    let mut read = Read { calls: 0 };
    let mut store = Store::default();
    let failure = compiled
        .run_live_durable(
            resume(&task, &policy, budget, &clock, &cancellation, &checkpoint),
            &mut source,
            &mut read,
            &mut store,
        )
        .err()
        .expect("clock regression must not replay");
    assert_eq!(failure.journal_error, Some(SourceJournalError::Time));
    assert_eq!((source.calls, read.calls), (0, 0));
}

#[test]
fn unresolved_model_and_effect_intents_never_redispatch() {
    let compiled = lifecycle();
    let task = task();
    let policy = policy(1);
    let budget = IterativeBudget::default();
    let fuel = budget.max_steps_per_stage;
    let mut model_entries = observed_prefix(fuel);
    model_entries.push(identity_intent(&compiled, &policy, budget));

    let mut effect_entries = model_entries.clone();
    let response = Source::valid_response(&compiled);
    let hash = source_response_digest(b"fixture-digest");
    effect_entries.extend([
        SourceJournalEntry::AttemptSettled {
            turn: 0,
            attempt: 0,
            response: response.clone(),
            response_digest: source_response_digest(&response),
        },
        SourceJournalEntry::ProposalAdmitted {
            turn: 0,
            attempt: 0,
            proposal_digest: hash.clone(),
        },
        stage(0, Some(0), SourceStageRole::Authorize, fuel),
        SourceJournalEntry::AuthorizationConsumed {
            turn: 0,
            attempt: 0,
            grant_digest: hash.clone(),
        },
        SourceJournalEntry::EffectIntent {
            turn: 0,
            attempt: 0,
            operation: "agent.read".into(),
            request_digest: hash,
        },
    ]);

    for entries in [model_entries, effect_entries] {
        let checkpoint = prefix(&compiled, &policy, budget, entries);
        let cancellation = AgentCancellation::default();
        let clock = Clock { now: 20 };
        let mut source = scripted(&compiled, 0);
        let mut read = Read { calls: 0 };
        let mut store = Store::default();
        let failure = compiled
            .run_live_durable(
                resume(&task, &policy, budget, &clock, &cancellation, &checkpoint),
                &mut source,
                &mut read,
                &mut store,
            )
            .err()
            .expect("unresolved physical intent must be uncertain");
        assert_eq!(failure.journal_error, Some(SourceJournalError::Uncertain));
        assert_eq!((source.calls, read.calls), (0, 0));
        assert!(store.document.is_empty());
    }
}

#[test]
fn reducer_fuel_is_reserved_before_effect_at_exact_and_one_below_limit() {
    let compiled = lifecycle();
    let task = task();
    let cancellation = AgentCancellation::default();
    let clock = Clock { now: 1 };
    let budget = IterativeBudget {
        max_iterations: 1,
        max_stages: 4,
        ..IterativeBudget::default()
    };
    let fuel = budget.max_steps_per_stage;
    for (limit, expected_reads) in [(4 * fuel - 1, 0), (4 * fuel, 1)] {
        let mut policy = policy(1);
        policy.max_total_steps = limit;
        let mut source = scripted(&compiled, 1);
        let mut read = Read { calls: 0 };
        let mut store = Store::default();
        let result = compiled.run_live_durable(
            SourceLiveRequest {
                task: &task,
                budget,
                policy: &policy,
                clock: &clock,
                cancellation: &cancellation,
                checkpoint: None,
            },
            &mut source,
            &mut read,
            &mut store,
        );
        assert_eq!(read.calls, expected_reads, "total stage fuel {limit}");
        assert_eq!(source.calls, 1);
        assert_eq!(
            store.document.contains("\"kind\":\"effect_intent\""),
            expected_reads == 1
        );
        match result {
            Ok(outcome) => assert_eq!(
                outcome.checkpoint.terminal_snapshot().unwrap().status(),
                SourceTerminalStatus::BudgetExhausted
            ),
            Err(failure) => assert_eq!(
                failure.selected,
                Some(SourceTerminalStatus::BudgetExhausted)
            ),
        }
    }
}

#[test]
fn replay_stage_fuel_is_charged_again_and_exhausts_before_new_work() {
    let compiled = lifecycle();
    let task = task();
    let cancellation = AgentCancellation::default();
    let budget = IterativeBudget {
        max_iterations: 1,
        max_stages: 4,
        ..IterativeBudget::default()
    };
    let fuel = budget.max_steps_per_stage;
    for (limit, expected_committed) in [(2 * fuel - 1, fuel), (2 * fuel, 2 * fuel)] {
        let mut policy = policy(1);
        policy.max_total_steps = limit;
        let checkpoint = prefix(
            &compiled,
            &policy,
            budget,
            [
                SourceJournalEntry::RunOpened,
                stage(0, None, SourceStageRole::Initialize, fuel),
            ],
        );
        let clock = Clock { now: 10 };
        let mut source = scripted(&compiled, 0);
        let mut read = Read { calls: 0 };
        let mut store = Store::default();
        let result = compiled.run_live_durable(
            resume(&task, &policy, budget, &clock, &cancellation, &checkpoint),
            &mut source,
            &mut read,
            &mut store,
        );
        let committed = match result {
            Ok(outcome) => outcome.checkpoint.committed_stage_fuel(),
            Err(failure) => {
                assert_eq!(
                    failure.selected,
                    Some(SourceTerminalStatus::BudgetExhausted)
                );
                failure.checkpoint.unwrap().committed_stage_fuel()
            }
        };
        assert_eq!(committed, expected_committed as u64);
        assert_eq!((source.calls, read.calls), (0, 0));
    }
}

struct LateFailureSource(Source);

impl ProposalSource for LateFailureSource {
    fn checkpoint_policy(&self) -> Option<SourceProposalPolicy<'_>> {
        Some(self.0.policy())
    }

    fn checkpoint_attempt_identity(
        &self,
        request: &ProposalRequest<'_>,
    ) -> Result<SourceAttemptIdentity, Vec<Diagnostic>> {
        Ok(Source::identity(request))
    }

    fn propose_checkpointed(
        &mut self,
        request: ProposalRequest<'_>,
        sink: &mut SourceCheckpointSink<'_>,
        ledger: &mut crate::live_invocation::CumulativeBudgetLedger<'_>,
        clock: &dyn SourceInvocationClock,
    ) -> SourceProposalOutcome {
        let mut outcome = self.0.propose_checkpointed(request, sink, ledger, clock);
        assert!(
            outcome.result.is_ok(),
            "settlement must be acknowledged first"
        );
        outcome.result = Err(vec![bad("fixture.deadline_after_settlement")]);
        outcome.terminal_failure = Some(SourceTerminalStatus::DeadlineExceeded);
        outcome
    }

    fn propose(&mut self, _: ProposalRequest<'_>) -> Result<String, Vec<Diagnostic>> {
        panic!("checkpointed route must never use unjournaled source")
    }
}

#[test]
fn acknowledged_model_response_followed_by_deadline_keeps_deadline_status() {
    let compiled = lifecycle();
    let task = task();
    let policy = policy(1);
    let clock = Clock { now: 1 };
    let cancellation = AgentCancellation::default();
    let mut source = LateFailureSource(scripted(&compiled, 1));
    let mut read = Read { calls: 0 };
    let mut store = Store::default();
    let failure = compiled
        .run_live_durable(
            request(&task, &policy, &clock, &cancellation),
            &mut source,
            &mut read,
            &mut store,
        )
        .err()
        .expect("late adapter deadline is a typed failure");
    assert_eq!(
        failure.selected,
        Some(SourceTerminalStatus::DeadlineExceeded)
    );
    assert_eq!((source.0.calls, read.calls), (1, 0));
    let checkpoint = failure.checkpoint.unwrap();
    assert_eq!(checkpoint.committed_reserved_units(), 1);
    assert!(matches!(
        checkpoint
            .entries()
            .iter()
            .find(|entry| matches!(entry, SourceJournalEntry::AttemptSettled { .. })),
        Some(SourceJournalEntry::AttemptSettled { .. })
    ));
    assert_eq!(
        checkpoint.terminal_snapshot().unwrap().status(),
        SourceTerminalStatus::DeadlineExceeded
    );
}

#[derive(Clone)]
struct MutableClock(Rc<Cell<i64>>);

impl InvocationClock for MutableClock {
    fn now_millis(&self) -> i64 {
        self.0.get()
    }
}

impl SourceInvocationClock for MutableClock {
    fn clock_domain(&self) -> &str {
        "source-test-clock-v1"
    }
}

struct AdvancingStore {
    document: String,
    clock: MutableClock,
    trigger: &'static str,
    deadline: i64,
}

impl CheckpointStore for AdvancingStore {
    fn commit(&mut self, _: u64, document: &str) -> Result<(), CheckpointStoreError> {
        self.document = document.to_owned();
        if document.contains(self.trigger) {
            self.clock.0.set(self.deadline);
        }
        Ok(())
    }
}

#[test]
fn reducer_selected_fail_is_sticky_after_transition_ack_and_deadline() {
    let compiled = compile_agent_lifecycle_v2(
        &source("Step::Fail { code: 37 }"),
        "source-live-fail.spx",
        &DEFINITION.replace("RUNTIME", RUNTIME_V1),
        "fixture.agent.type.step",
    )
    .expect("checked reducer Fail fixture compiles");
    let task = task();
    let policy = policy(3);
    let clock = MutableClock(Rc::new(Cell::new(1)));
    let cancellation = AgentCancellation::default();
    let mut source = scripted(&compiled, 3);
    let mut read = Read { calls: 0 };
    let mut store = AdvancingStore {
        document: String::new(),
        clock: clock.clone(),
        trigger: "\"case\":\"fail\"",
        deadline: policy.deadline_millis,
    };
    let outcome = compiled
        .run_live_durable(
            SourceLiveRequest {
                task: &task,
                budget: IterativeBudget::default(),
                policy: &policy,
                clock: &clock,
                cancellation: &cancellation,
                checkpoint: None,
            },
            &mut source,
            &mut read,
            &mut store,
        )
        .expect("reducer Fail remains selected after deadline");
    assert_eq!(clock.now_millis(), policy.deadline_millis);
    assert_eq!(
        outcome.checked_run.as_ref().unwrap().status(),
        IterativeStatus::Fail
    );
    assert_eq!(
        outcome.checkpoint.terminal_snapshot().unwrap().status(),
        SourceTerminalStatus::Fail
    );
    assert_eq!((source.calls, read.calls), (3, 3));
}

#[test]
fn terminal_ack_after_deadline_does_not_publish_fresh_success() {
    let compiled = lifecycle();
    let task = task();
    let policy = policy(3);
    let clock = MutableClock(Rc::new(Cell::new(1)));
    let cancellation = AgentCancellation::default();
    let mut source = scripted(&compiled, 3);
    let mut read = Read { calls: 0 };
    let mut store = AdvancingStore {
        document: String::new(),
        clock: clock.clone(),
        trigger: "\"kind\":\"terminal_snapshot\"",
        deadline: policy.deadline_millis,
    };
    let failure = compiled
        .run_live_durable(
            SourceLiveRequest {
                task: &task,
                budget: IterativeBudget::default(),
                policy: &policy,
                clock: &clock,
                cancellation: &cancellation,
                checkpoint: None,
            },
            &mut source,
            &mut read,
            &mut store,
        )
        .err()
        .expect("late terminal ACK cannot publish a fresh checked success");
    assert_eq!(
        failure.selected,
        Some(SourceTerminalStatus::DeadlineExceeded)
    );
    assert_eq!((source.calls, read.calls), (3, 3));
    assert_eq!(
        failure
            .checkpoint
            .unwrap()
            .terminal_snapshot()
            .unwrap()
            .status(),
        SourceTerminalStatus::Complete,
        "an acknowledged receipt is immutable even when publication is refused"
    );
}

#[test]
fn clock_regression_after_stage_ack_stops_before_evaluating_that_stage() {
    let compiled = lifecycle();
    let task = task();
    let policy = policy(1);
    let clock = MutableClock(Rc::new(Cell::new(5)));
    let cancellation = AgentCancellation::default();
    let mut source = scripted(&compiled, 0);
    let mut read = Read { calls: 0 };
    let mut store = AdvancingStore {
        document: String::new(),
        clock: clock.clone(),
        trigger: "\"kind\":\"stage_reservation\"",
        deadline: 4,
    };
    let failure = compiled
        .run_live_durable(
            SourceLiveRequest {
                task: &task,
                budget: IterativeBudget::default(),
                policy: &policy,
                clock: &clock,
                cancellation: &cancellation,
                checkpoint: None,
            },
            &mut source,
            &mut read,
            &mut store,
        )
        .err()
        .expect("the acknowledged clock floor rejects a regressed live clock");
    assert_eq!(clock.now_millis(), 4);
    assert_eq!(failure.journal_error, Some(SourceJournalError::Time));
    assert!(
        failure.stage_rows.is_empty(),
        "initialize was not evaluated"
    );
    assert_eq!((source.calls, read.calls), (0, 0));
    assert_eq!(failure.checkpoint.unwrap().last_checked_millis(), 5);
}

struct CancellingStore {
    document: String,
    cancellation: AgentCancellation,
}

impl CheckpointStore for CancellingStore {
    fn commit(&mut self, _: u64, document: &str) -> Result<(), CheckpointStoreError> {
        self.document = document.to_owned();
        if document.contains("\"kind\":\"attempt_settled\"") {
            self.cancellation.cancel();
        }
        Ok(())
    }
}

#[test]
fn cancellation_after_settlement_keeps_charge_and_skips_effect() {
    let compiled = lifecycle();
    let task = task();
    let policy = policy(1);
    let clock = Clock { now: 1 };
    let cancellation = AgentCancellation::default();
    let mut source = scripted(&compiled, 1);
    let mut read = Read { calls: 0 };
    let mut store = CancellingStore {
        document: String::new(),
        cancellation: cancellation.clone(),
    };
    let failure = compiled
        .run_live_durable(
            request(&task, &policy, &clock, &cancellation),
            &mut source,
            &mut read,
            &mut store,
        )
        .err()
        .expect("cooperative cancellation after settlement refuses the effect");
    assert_eq!(failure.selected, Some(SourceTerminalStatus::Cancelled));
    assert_eq!((source.calls, read.calls), (1, 0));
    let checkpoint = failure.checkpoint.unwrap();
    assert_eq!(checkpoint.committed_reserved_units(), 1);
    assert_eq!(
        checkpoint.terminal_snapshot().unwrap().status(),
        SourceTerminalStatus::Cancelled
    );
}
