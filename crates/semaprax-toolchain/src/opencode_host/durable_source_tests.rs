use super::super::source::OpenCodeDurableProposalSource;
use super::*;
use semaprax::agent_lifecycle::iterative::driver::{ProposalRequest, ProposalSource};
use semaprax::agent_lifecycle::{CheckpointStore, CheckpointStoreError};
use semaprax::interpreter::retained_call::RetainedValue;
use semaprax::live_invocation::{
    source_journal::{
        SourceCheckpointSink, SourceInvocationBinding, SourceInvocationSeed, SourceJournalEntry,
        SourceStageRole, SourceTerminalStatus,
    },
    CumulativeBudgetLedger, SourceInvocationClock,
};
use std::cell::Cell;
use std::rc::Rc;

const DEPLOYMENT: &str = "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
const REVISION: &str = "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
const LIFECYCLE: &str = "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";

#[derive(Default)]
struct Store {
    fail_at: Option<u64>,
    advance_at: Option<(u64, Rc<Cell<i64>>, i64)>,
}
impl CheckpointStore for Store {
    fn commit(&mut self, generation: u64, _: &str) -> Result<(), CheckpointStoreError> {
        if self.fail_at == Some(generation) {
            Err(CheckpointStoreError)
        } else {
            if let Some((expected, clock, millis)) = &self.advance_at {
                if *expected == generation {
                    clock.set(*millis);
                }
            }
            Ok(())
        }
    }
}
struct SourceClock(Rc<Cell<i64>>);
impl InvocationClock for SourceClock {
    fn now_millis(&self) -> i64 {
        self.0.get()
    }
}
impl SourceInvocationClock for SourceClock {
    fn clock_domain(&self) -> &str {
        "fixture-clock-v1"
    }
}

fn binding(task: &LifecycleTask, grammar: &OpenCodeGrammar) -> SourceInvocationBinding {
    SourceInvocationBinding::bind_execution(
        SourceInvocationSeed {
            lifecycle_digest: LIFECYCLE.into(),
            source_revision: REVISION.into(),
            deployment_binding: DEPLOYMENT.into(),
            task: task.objective.clone(),
            task_budget: task.budget,
            proposal_schema_digest: grammar.digest.clone(),
            response_limit: 4096,
            max_iterations: 1,
            max_stages: 4,
            max_attempts: 1,
            max_steps_per_stage: 8,
            max_total_steps: 32,
            ceiling: 1,
            reservation_units: 1,
            unit: "fixture-unit-v1".into(),
            clock_domain: "fixture-clock-v1".into(),
            initial_millis: 0,
            deadline_millis: 10,
            program_root: None,
        },
        LIFECYCLE,
    )
    .unwrap()
}
fn begin(sink: &mut SourceCheckpointSink<'_>) {
    sink.append_at(SourceJournalEntry::RunOpened, 0).unwrap();
    sink.append_at(
        SourceJournalEntry::StageReservation {
            turn: 0,
            attempt: None,
            role: SourceStageRole::Initialize,
            fuel: 8,
        },
        0,
    )
    .unwrap();
    sink.append_at(
        SourceJournalEntry::StageReservation {
            turn: 0,
            attempt: None,
            role: SourceStageRole::Observe,
            fuel: 8,
        },
        0,
    )
    .unwrap();
    sink.append_at(
        SourceJournalEntry::TurnObserved {
            turn: 0,
            state: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
            observation: "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                .into(),
            feedback: "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                .into(),
        },
        0,
    )
    .unwrap();
}
fn request<'a>(
    task: &'a LifecycleTask,
    grammar: &'a OpenCodeGrammar,
    value: &'a RetainedValue,
) -> ProposalRequest<'a> {
    ProposalRequest {
        turn: 0,
        attempt: 0,
        source_revision: REVISION,
        proposal_schema_digest: &grammar.digest,
        task,
        state: value,
        observation: value,
        previous_effect: None,
        previous_rejection: None,
        remaining_iterations: 1,
    }
}

#[test]
fn durable_source_refuses_ordinary_route_before_the_runner() {
    let (_, mut handler, task, grammar) = setup(compiled_proposal());
    let capability = ModelInvokeCapability::grant("durable source fixture");
    let state = RetainedValue::I64(0);
    let mut source = OpenCodeDurableProposalSource::new(
        &mut handler,
        &capability,
        "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd".into(),
        grammar.clone(),
        4_096,
        1,
    )
    .unwrap();
    let result = source.propose(ProposalRequest {
        turn: 0,
        attempt: 0,
        source_revision: "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        proposal_schema_digest: &grammar.digest,
        task: &task,
        state: &state,
        observation: &state,
        previous_effect: None,
        previous_rejection: None,
        remaining_iterations: 1,
    });
    assert!(result.is_err());
    drop(source);
    assert_eq!(handler.runner.calls, 0);
}

#[test]
fn durable_source_policy_binds_exact_transport_limits() {
    let (_, mut handler, _, grammar) = setup(compiled_proposal());
    let capability = ModelInvokeCapability::grant("durable source fixture");
    let source = OpenCodeDurableProposalSource::new(
        &mut handler,
        &capability,
        "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd".into(),
        grammar,
        123,
        7,
    )
    .unwrap();
    let policy = source.checkpoint_policy().unwrap();
    assert_eq!(policy.response_limit, 123);
    assert_eq!(policy.reservation_units, 7);
}

#[test]
fn durable_checkpointed_source_dispatches_once_charges_once_and_persists_usage() {
    let (_, mut handler, task, grammar) = setup(compiled_proposal());
    let capability = ModelInvokeCapability::grant("durable source fixture");
    let value = RetainedValue::I64(0);
    let shared = Rc::new(Cell::new(0));
    let clock = SourceClock(shared);
    let mut store = Store::default();
    let bound = binding(&task, &grammar);
    let mut ledger = CumulativeBudgetLedger::start_source(&bound, &clock).unwrap();
    let mut sink = SourceCheckpointSink::new(&mut store, bound);
    begin(&mut sink);
    let mut source = OpenCodeDurableProposalSource::new(
        &mut handler,
        &capability,
        DEPLOYMENT.into(),
        grammar.clone(),
        4096,
        1,
    )
    .unwrap();
    let outcome = source.propose_checkpointed(
        request(&task, &grammar, &value),
        &mut sink,
        &mut ledger,
        &clock,
    );
    assert_eq!(outcome.model_dispatches, 1);
    assert_eq!(outcome.result.unwrap(), compiled_proposal());
    assert_eq!(ledger.committed(), 1);
    assert!(matches!(
        sink.journal().entries().last(),
        Some(SourceJournalEntry::AttemptUsage {
            turn: 0,
            attempt: 0,
            ..
        })
    ));
    drop(source);
    assert_eq!(handler.runner.calls, 1);
}

#[test]
fn durable_preview_is_pure_and_intent_store_failure_dispatches_nothing() {
    let (_, mut handler, task, grammar) = setup(compiled_proposal());
    let capability = ModelInvokeCapability::grant("durable source fixture");
    let value = RetainedValue::I64(0);
    let shared = Rc::new(Cell::new(0));
    let clock = SourceClock(shared);
    let source = OpenCodeDurableProposalSource::new(
        &mut handler,
        &capability,
        DEPLOYMENT.into(),
        grammar.clone(),
        4096,
        1,
    )
    .unwrap();
    let identity = source
        .checkpoint_attempt_identity(&request(&task, &grammar, &value))
        .unwrap();
    assert!(identity.request_bytes > 0);
    drop(source);
    assert_eq!(handler.runner.calls, 0);
    let mut source = OpenCodeDurableProposalSource::new(
        &mut handler,
        &capability,
        DEPLOYMENT.into(),
        grammar.clone(),
        4096,
        1,
    )
    .unwrap();
    let mut store = Store {
        // V2 prefix consumes generations 1 through 4; generation 5 is intent.
        fail_at: Some(5),
        ..Store::default()
    };
    let bound = binding(&task, &grammar);
    let mut ledger = CumulativeBudgetLedger::start_source(&bound, &clock).unwrap();
    let mut sink = SourceCheckpointSink::new(&mut store, bound);
    begin(&mut sink);
    let outcome = source.propose_checkpointed(
        request(&task, &grammar, &value),
        &mut sink,
        &mut ledger,
        &clock,
    );
    assert_eq!(outcome.model_dispatches, 0);
    drop(source);
    assert_eq!(handler.runner.calls, 0);
}

#[test]
fn durable_source_reports_deadline_expiry_after_usage_ack_as_terminal_failure() {
    let (_, mut handler, task, grammar) = setup(compiled_proposal());
    let capability = ModelInvokeCapability::grant("durable source fixture");
    let value = RetainedValue::I64(0);
    let shared = Rc::new(Cell::new(0));
    let clock = SourceClock(shared.clone());
    let bound = binding(&task, &grammar);
    let mut ledger = CumulativeBudgetLedger::start_source(&bound, &clock).unwrap();
    // V2 has four acknowledged prefix entries, then intent and settlement.
    // Advancing only on usage ACK proves the wrapper rechecks the shared
    // ledger after its own sideband checkpoint, rather than raw time before it.
    let mut store = Store {
        advance_at: Some((7, shared, 10)),
        ..Store::default()
    };
    let mut sink = SourceCheckpointSink::new(&mut store, bound);
    begin(&mut sink);
    let mut source = OpenCodeDurableProposalSource::new(
        &mut handler,
        &capability,
        DEPLOYMENT.into(),
        grammar.clone(),
        4096,
        1,
    )
    .unwrap();
    let outcome = source.propose_checkpointed(
        request(&task, &grammar, &value),
        &mut sink,
        &mut ledger,
        &clock,
    );
    assert!(outcome.result.is_err());
    assert_eq!(
        outcome.terminal_failure,
        Some(SourceTerminalStatus::DeadlineExceeded)
    );
    assert_eq!(outcome.model_dispatches, 1);
    assert_eq!(ledger.committed(), 1);
    assert!(matches!(
        sink.journal()
            .entries()
            .iter()
            .rev()
            .find(|entry| matches!(entry, SourceJournalEntry::AttemptSettled { .. })),
        Some(SourceJournalEntry::AttemptSettled { .. })
    ));
}
