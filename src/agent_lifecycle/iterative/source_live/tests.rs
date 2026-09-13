//! End-to-end source-journal fixtures for the checkpointed live route.
//!
//! These tests use the real checked lifecycle, cumulative source ledger and
//! journal sink. The proposal source is only a deterministic host seam: it
//! records the same intent/settlement pair a provider adapter must record and
//! never bypasses the ledger or checkpoint acknowledgement.

#[path = "adversarial_tests.rs"]
mod adversarial_tests;

use super::*;
use crate::agent_lifecycle::iterative::driver::{ProposalRequest, ProposalSource};
use crate::agent_lifecycle::iterative::tests::source;
use crate::agent_lifecycle::tests::{DEFINITION, RUNTIME_V1};
use crate::agent_lifecycle::{AgentReadOperation, CheckpointStore, CheckpointStoreError};
use crate::agent_runtime::AgentCancellation;
use crate::live_invocation::model_invoke::{InvocationBudgetHook, ModelInvocationRequest};
use crate::live_invocation::source_journal::{
    source_response_digest, SourceCheckpointSink, SourceJournalEntry,
};
use crate::live_invocation::{InvocationClock, SourceInvocationClock};

struct Clock {
    now: i64,
}

impl InvocationClock for Clock {
    fn now_millis(&self) -> i64 {
        self.now
    }
}

impl SourceInvocationClock for Clock {
    fn clock_domain(&self) -> &str {
        "source-test-clock-v1"
    }
}

#[derive(Default)]
struct Store {
    document: String,
    fail: bool,
    fail_transition_once: bool,
}

impl CheckpointStore for Store {
    fn commit(&mut self, _generation: u64, document: &str) -> Result<(), CheckpointStoreError> {
        if self.fail {
            return Err(CheckpointStoreError);
        }
        if self.fail_transition_once && document.contains("\"transition\"") {
            self.fail_transition_once = false;
            return Err(CheckpointStoreError);
        }
        self.document = document.to_owned();
        Ok(())
    }
}

struct Read {
    calls: usize,
}

impl AgentReadOperation for Read {
    fn read(&mut self, _: &crate::agent_lifecycle::AuthorizedRequest) -> Option<Vec<u8>> {
        self.calls += 1;
        Some(b"fixture-read".to_vec())
    }
}

struct Source {
    responses: Vec<Vec<u8>>,
    calls: usize,
    deployment: String,
    response_limit: usize,
    reservation_units: i64,
}

const DEPLOYMENT: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

impl Source {
    fn valid_response(compiled: &CompiledIterativeLifecycle) -> Vec<u8> {
        crate::agent_lifecycle::tests::proposal(&compiled.inner, "1", "1").into_bytes()
    }

    fn policy(&self) -> SourceProposalPolicy<'_> {
        SourceProposalPolicy {
            deployment_binding: &self.deployment,
            response_limit: self.response_limit,
            reservation_units: self.reservation_units,
        }
    }

    fn identity(request: &ProposalRequest<'_>) -> SourceAttemptIdentity {
        SourceAttemptIdentity {
            request_digest: request.source_revision.to_owned(),
            prompt_digest: request.proposal_schema_digest.to_owned(),
            request_bytes: 1,
        }
    }
}

impl ProposalSource for Source {
    fn checkpoint_policy(&self) -> Option<SourceProposalPolicy<'_>> {
        Some(self.policy())
    }

    fn checkpoint_attempt_identity(
        &self,
        request: &ProposalRequest<'_>,
    ) -> Result<SourceAttemptIdentity, Vec<Diagnostic>> {
        Ok(Self::identity(request))
    }

    fn propose_checkpointed(
        &mut self,
        request: ProposalRequest<'_>,
        sink: &mut SourceCheckpointSink<'_>,
        ledger: &mut crate::live_invocation::CumulativeBudgetLedger<'_>,
        clock: &dyn SourceInvocationClock,
    ) -> SourceProposalOutcome {
        let response = self
            .responses
            .get(self.calls)
            .cloned()
            .unwrap_or_else(|| panic!("fixture source called beyond its scripted responses"));
        let identity = Self::identity(&request);
        let binding = sink.journal().binding();
        let model_request = ModelInvocationRequest {
            turn: request.turn as u32,
            task: request.task.objective.clone(),
            observation: Vec::new(),
            proposal_grammar_digest: request.proposal_schema_digest.to_owned(),
            deployment_binding: self.deployment.clone(),
            max_response_bytes: self.response_limit,
            effective_budget: self.reservation_units,
        };
        if ledger.reserve(&model_request).is_err() {
            return SourceProposalOutcome {
                terminal_failure: None,
                result: Err(vec![bad("source.model_budget")]),
                model_dispatches: 0,
            };
        }
        let intent = SourceJournalEntry::AttemptIntent {
            turn: request.turn as u32,
            attempt: request.attempt as u32,
            attempt_digest: binding.attempt_digest(
                request.turn as u32,
                request.attempt as u32,
                &identity.request_digest,
                &identity.prompt_digest,
                identity.request_bytes,
            ),
            request_digest: identity.request_digest,
            prompt_digest: identity.prompt_digest,
            request_bytes: identity.request_bytes,
            reserved_units: binding.reservation_units(),
            response_limit: binding.response_limit(),
        };
        let result = sink.append_at(intent, clock.now_millis()).and_then(|_| {
            sink.append_at(
                SourceJournalEntry::AttemptSettled {
                    turn: request.turn as u32,
                    attempt: request.attempt as u32,
                    response: response.clone(),
                    response_digest: source_response_digest(&response),
                },
                clock.now_millis(),
            )
        });
        self.calls += 1;
        match result {
            Ok(()) => SourceProposalOutcome {
                terminal_failure: None,
                result: Ok(String::from_utf8(response).expect("fixture response is UTF-8")),
                model_dispatches: 1,
            },
            Err(_) => SourceProposalOutcome {
                terminal_failure: None,
                result: Err(vec![bad("source.attempt_receipt")]),
                model_dispatches: 1,
            },
        }
    }

    fn propose(&mut self, _: ProposalRequest<'_>) -> Result<String, Vec<Diagnostic>> {
        Err(vec![bad("source.unjournaled")])
    }
}

fn lifecycle() -> CompiledIterativeLifecycle {
    compile_agent_lifecycle_v2(
        &source("Step::Complete { summary: state.objective, budget: state.budget, status: state.epoch }"),
        "source-live-unit.spx",
        &DEFINITION.replace("RUNTIME", RUNTIME_V1),
        "fixture.agent.type.step",
    )
    .expect("source-live fixture compiles")
}

fn policy(ceiling: i64) -> SourceLivePolicy {
    SourceLivePolicy {
        deployment_binding: DEPLOYMENT.into(),
        response_limit: 4096,
        ceiling,
        reservation_units: 1,
        unit: "fixture_unit_v1".into(),
        clock_domain: "source-test-clock-v1".into(),
        initial_millis: 0,
        deadline_millis: 10_000,
        max_total_steps: 2_000_000,
        program_root: None,
    }
}

fn task() -> LifecycleTask {
    LifecycleTask {
        objective: b"source task".to_vec(),
        budget: 10,
    }
}

fn request<'a>(
    task: &'a LifecycleTask,
    policy: &'a SourceLivePolicy,
    clock: &'a Clock,
    cancellation: &'a AgentCancellation,
) -> SourceLiveRequest<'a> {
    SourceLiveRequest {
        task,
        budget: IterativeBudget::default(),
        policy,
        clock,
        cancellation,
        checkpoint: None,
    }
}

#[test]
fn normal_source_run_completes_with_real_journal_and_effect() {
    let compiled = lifecycle();
    let response = Source::valid_response(&compiled);
    let mut source = Source {
        responses: vec![response.clone(), response.clone(), response],
        calls: 0,
        deployment: DEPLOYMENT.into(),
        response_limit: 4096,
        reservation_units: 1,
    };
    let mut read = Read { calls: 0 };
    let mut store = Store::default();
    let clock = Clock { now: 1 };
    let outcome = compiled
        .run_live_durable(
            request(&task(), &policy(3), &clock, &AgentCancellation::default()),
            &mut source,
            &mut read,
            &mut store,
        )
        .expect("source run completes");
    assert_eq!(
        outcome.checked_run.as_ref().unwrap().status(),
        IterativeStatus::Complete
    );
    assert_eq!(
        (
            source.calls,
            read.calls,
            outcome.model_dispatches,
            outcome.effect_dispatches
        ),
        (3, 3, 3, 3)
    );
    assert!(!store.document.is_empty());
}

#[test]
fn terminal_recovery_returns_without_model_or_effect_calls() {
    let compiled = lifecycle();
    let response = Source::valid_response(&compiled);
    let mut source = Source {
        responses: vec![response.clone(), response.clone(), response],
        calls: 0,
        deployment: DEPLOYMENT.into(),
        response_limit: 4096,
        reservation_units: 1,
    };
    let mut read = Read { calls: 0 };
    let mut store = Store::default();
    let clock = Clock { now: 1 };
    let first = compiled
        .run_live_durable(
            request(&task(), &policy(3), &clock, &AgentCancellation::default()),
            &mut source,
            &mut read,
            &mut store,
        )
        .expect("initial source run completes");
    let checkpoint = store.document.clone();
    let mut recovered_source = Source {
        responses: Vec::new(),
        calls: 0,
        deployment: DEPLOYMENT.into(),
        response_limit: 4096,
        reservation_units: 1,
    };
    let mut recovered_read = Read { calls: 0 };
    let mut recovered_store = Store::default();
    let recovered_task = task();
    let recovered_policy = policy(3);
    let recovered_cancellation = AgentCancellation::default();
    let recovered_request = SourceLiveRequest {
        task: &recovered_task,
        budget: IterativeBudget::default(),
        policy: &recovered_policy,
        clock: &clock,
        cancellation: &recovered_cancellation,
        checkpoint: Some(&checkpoint),
    };
    let recovered = compiled
        .run_live_durable(
            recovered_request,
            &mut recovered_source,
            &mut recovered_read,
            &mut recovered_store,
        )
        .expect("terminal checkpoint is idempotently recoverable");
    assert!(recovered.checked_run.is_none());
    assert_eq!((recovered_source.calls, recovered_read.calls), (0, 0));
    assert_eq!(
        recovered.checkpoint.generation(),
        first.checkpoint.generation()
    );
}

#[test]
fn malformed_proposal_retries_without_an_effect_then_completes() {
    let compiled = lifecycle();
    let valid = Source::valid_response(&compiled);
    let mut source = Source {
        responses: vec![b"malformed".to_vec(), valid.clone(), valid.clone(), valid],
        calls: 0,
        deployment: DEPLOYMENT.into(),
        response_limit: 4096,
        reservation_units: 1,
    };
    let mut read = Read { calls: 0 };
    let mut store = Store::default();
    let clock = Clock { now: 1 };
    let outcome = compiled
        .run_live_durable(
            request(&task(), &policy(4), &clock, &AgentCancellation::default()),
            &mut source,
            &mut read,
            &mut store,
        )
        .expect("malformed proposal is bounded and retried");
    assert_eq!(
        outcome.checked_run.as_ref().unwrap().status(),
        IterativeStatus::Complete
    );
    assert_eq!((source.calls, read.calls), (4, 3));
}

#[test]
fn zero_ceiling_refuses_before_the_first_model_or_effect_call() {
    let compiled = lifecycle();
    let mut source = Source {
        responses: Vec::new(),
        calls: 0,
        deployment: DEPLOYMENT.into(),
        response_limit: 4096,
        reservation_units: 1,
    };
    let mut read = Read { calls: 0 };
    let mut store = Store::default();
    let clock = Clock { now: 1 };
    assert!(compiled
        .run_live_durable(
            request(&task(), &policy(0), &clock, &AgentCancellation::default()),
            &mut source,
            &mut read,
            &mut store,
        )
        .is_err());
    assert_eq!((source.calls, read.calls), (0, 0));
}

#[test]
fn unsupported_source_is_rejected_without_a_model_call() {
    struct Unsupported;
    impl ProposalSource for Unsupported {
        fn propose(&mut self, _: ProposalRequest<'_>) -> Result<String, Vec<Diagnostic>> {
            panic!("unsupported source must be rejected before propose")
        }
    }
    let compiled = lifecycle();
    let mut source = Unsupported;
    let mut read = Read { calls: 0 };
    let mut store = Store::default();
    let clock = Clock { now: 1 };
    assert!(compiled
        .run_live_durable(
            request(&task(), &policy(1), &clock, &AgentCancellation::default()),
            &mut source,
            &mut read,
            &mut store,
        )
        .is_err());
    assert_eq!(read.calls, 0);
}

#[test]
fn store_failure_is_reported_before_the_source_dispatches() {
    let compiled = lifecycle();
    let mut source = Source {
        responses: Vec::new(),
        calls: 0,
        deployment: DEPLOYMENT.into(),
        response_limit: 4096,
        reservation_units: 1,
    };
    let mut read = Read { calls: 0 };
    let mut store = Store {
        document: String::new(),
        fail: true,
        ..Store::default()
    };
    let clock = Clock { now: 1 };
    assert!(compiled
        .run_live_durable(
            request(&task(), &policy(1), &clock, &AgentCancellation::default()),
            &mut source,
            &mut read,
            &mut store,
        )
        .is_err());
    assert_eq!((source.calls, read.calls), (0, 0));
}

#[test]
fn transition_store_failure_recovers_after_effect_observed_without_duplicate_read() {
    let compiled = lifecycle();
    let response = Source::valid_response(&compiled);
    let mut source = Source {
        responses: vec![response.clone(), response.clone(), response],
        calls: 0,
        deployment: DEPLOYMENT.into(),
        response_limit: 4096,
        reservation_units: 1,
    };
    let mut read = Read { calls: 0 };
    let mut store = Store {
        fail_transition_once: true,
        ..Store::default()
    };
    let clock = Clock { now: 1 };
    let failed = compiled.run_live_durable(
        request(&task(), &policy(3), &clock, &AgentCancellation::default()),
        &mut source,
        &mut read,
        &mut store,
    );
    let failed = failed
        .err()
        .expect("transition acknowledgement failure must surface");
    assert_eq!(read.calls, 1);
    assert!(store.document.contains("\"effect_observed\""));
    assert!(!store.document.contains("\"transition\""));
    let checkpoint = store.document.clone();
    let recovered_task = task();
    let recovered_policy = policy(3);
    let recovered_cancellation = AgentCancellation::default();
    let mut resumed_source = Source {
        responses: vec![Source::valid_response(&compiled); 2],
        calls: 0,
        deployment: DEPLOYMENT.into(),
        response_limit: 4096,
        reservation_units: 1,
    };
    let mut resumed_read = Read { calls: 0 };
    let mut resumed_store = Store::default();
    let resumed = compiled
        .run_live_durable(
            SourceLiveRequest {
                task: &recovered_task,
                budget: IterativeBudget::default(),
                policy: &recovered_policy,
                clock: &clock,
                cancellation: &recovered_cancellation,
                checkpoint: Some(&checkpoint),
            },
            &mut resumed_source,
            &mut resumed_read,
            &mut resumed_store,
        )
        .expect("replay resumes after the acknowledged effect");
    assert_eq!(
        resumed.checked_run.as_ref().unwrap().status(),
        IterativeStatus::Complete
    );
    assert_eq!((resumed_source.calls, resumed_read.calls), (2, 2));
    assert!(
        resumed.checkpoint.committed_stage_fuel()
            > failed
                .checkpoint
                .as_ref()
                .expect("failed run retains acknowledged checkpoint")
                .committed_stage_fuel()
    );
}
