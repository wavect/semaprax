use super::*;
use semaprax::agent_lifecycle::iterative::driver::ProposalRequest;
use semaprax::agent_lifecycle::{CheckpointStore, CheckpointStoreError};
use semaprax::interpreter::retained_call::RetainedValue;
use semaprax::live_invocation::{
    source_journal::{
        SourceCheckpointSink, SourceInvocationBinding, SourceInvocationSeed, SourceJournalEntry,
    },
    CumulativeBudgetLedger, InvocationClock, SourceInvocationClock,
};

const DEPLOYMENT: &str = "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
const REVISION: &str = "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
const LIFECYCLE: &str = "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";

#[derive(Default)]
struct Store {
    document: String,
    fail_at: Option<(u64, bool)>,
    advance_at: Option<(u64, Rc<Cell<i64>>, i64)>,
}
impl CheckpointStore for Store {
    fn commit(&mut self, generation: u64, document: &str) -> Result<(), CheckpointStoreError> {
        if self.fail_at == Some((generation, false)) {
            return Err(CheckpointStoreError);
        }
        self.document = document.to_owned();
        if let Some((at, clock, value)) = &self.advance_at {
            if *at == generation {
                clock.set(*value);
            }
        }
        if self.fail_at.is_some_and(|(at, _)| at == generation) {
            return Err(CheckpointStoreError);
        }
        Ok(())
    }
}
struct CheckpointClock(Rc<Cell<i64>>);
impl InvocationClock for CheckpointClock {
    fn now_millis(&self) -> i64 {
        self.0.get()
    }
}
impl SourceInvocationClock for CheckpointClock {
    fn clock_domain(&self) -> &str {
        "source-test-ms-v1"
    }
}

fn binding(
    task: &LifecycleTask,
    grammar: &OpenCodeGrammar,
    limit: usize,
    units: i64,
) -> SourceInvocationBinding {
    SourceInvocationBinding::bind(SourceInvocationSeed {
        lifecycle_digest: LIFECYCLE.into(),
        source_revision: REVISION.into(),
        deployment_binding: DEPLOYMENT.into(),
        task: task.objective.clone(),
        task_budget: task.budget,
        proposal_schema_digest: grammar.digest.clone(),
        response_limit: limit,
        max_iterations: 1,
        max_stages: 3,
        max_attempts: 1,
        max_steps_per_stage: 8,
        max_total_steps: 8,
        ceiling: units,
        reservation_units: units,
        unit: "test-unit-v1".into(),
        clock_domain: "source-test-ms-v1".into(),
        initial_millis: 0,
        deadline_millis: 10,
        program_root: None,
    })
    .unwrap()
}
fn begin(sink: &mut SourceCheckpointSink<'_>) {
    sink.append_at(SourceJournalEntry::RunOpened, 0).unwrap();
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
fn source<'a>(
    handler: &'a mut OpenCodeModelHandler<FixtureRunner>,
    cap: &'a ModelInvokeCapability,
    grammar: OpenCodeGrammar,
    ledger: &'a mut CumulativeBudgetLedger<'_>,
    units: i64,
    limit: usize,
) -> OpenCodeProposalSource<'a, FixtureRunner> {
    OpenCodeProposalSource::new(
        handler,
        cap,
        DEPLOYMENT.into(),
        grammar,
        limit,
        OpenCodeSourceAccounting::new(ledger, units, 1).unwrap(),
    )
    .unwrap()
}

#[test]
fn checkpointed_success_charges_once_after_acked_intent() {
    let (_, mut handler, task, grammar) = setup(compiled_proposal());
    let value = RetainedValue::I64(0);
    let shared = Rc::new(Cell::new(0));
    let mut ledger_clock = Clock(shared.clone());
    let mut ledger = CumulativeBudgetLedger::with_deadline(1, 10, &mut ledger_clock);
    let clock = CheckpointClock(shared.clone());
    let cap = ModelInvokeCapability::grant("test");
    let mut store = Store::default();
    let mut sink = SourceCheckpointSink::new(&mut store, binding(&task, &grammar, 4096, 1));
    begin(&mut sink);
    let mut source = source(&mut handler, &cap, grammar.clone(), &mut ledger, 1, 4096);
    assert_eq!(
        source
            .propose_checkpointed(request(&task, &grammar, &value), &mut sink, &clock)
            .unwrap(),
        compiled_proposal()
    );
    assert_eq!(source.receipts().len(), 1);
    assert!(matches!(
        sink.journal().entries().last(),
        Some(SourceJournalEntry::AttemptSettled { .. })
    ));
    drop(source);
    assert_eq!(handler.runner.calls, 1);
    assert_eq!(ledger.committed(), 1);
}

#[test]
fn preintent_store_failure_never_calls_runner() {
    let (_, mut handler, task, grammar) = setup(compiled_proposal());
    let value = RetainedValue::I64(0);
    let shared = Rc::new(Cell::new(0));
    let mut ledger_clock = Clock(shared.clone());
    let mut ledger = CumulativeBudgetLedger::with_deadline(1, 10, &mut ledger_clock);
    let clock = CheckpointClock(shared.clone());
    let cap = ModelInvokeCapability::grant("test");
    let mut store = Store {
        fail_at: Some((3, false)),
        ..Store::default()
    };
    let mut sink = SourceCheckpointSink::new(&mut store, binding(&task, &grammar, 4096, 1));
    begin(&mut sink);
    let mut source = source(&mut handler, &cap, grammar.clone(), &mut ledger, 1, 4096);
    assert!(source
        .propose_checkpointed(request(&task, &grammar, &value), &mut sink, &clock)
        .is_err());
    assert!(sink.poisoned());
    drop(source);
    assert_eq!(handler.runner.calls, 0);
}

#[test]
fn binding_context_limit_and_units_mismatch_refuse_before_charge_or_runner() {
    for changed in 0..4 {
        let (_, mut handler, task, grammar) = setup(compiled_proposal());
        let value = RetainedValue::I64(0);
        let shared = Rc::new(Cell::new(0));
        let mut ledger_clock = Clock(shared.clone());
        let mut ledger = CumulativeBudgetLedger::with_deadline(1, 10, &mut ledger_clock);
        let clock = CheckpointClock(shared);
        let cap = ModelInvokeCapability::grant("test");
        let mut store = Store::default();
        let mut bound_task = task.clone();
        if changed == 2 {
            bound_task.budget += 1;
        }
        if changed == 3 {
            bound_task.objective.push(b'!');
        }
        let bound = binding(
            &bound_task,
            &grammar,
            if changed == 0 { 1 } else { 4096 },
            if changed == 1 { 2 } else { 1 },
        );
        let mut sink = SourceCheckpointSink::new(&mut store, bound);
        begin(&mut sink);
        let mut source = source(&mut handler, &cap, grammar.clone(), &mut ledger, 1, 4096);
        assert!(source
            .propose_checkpointed(request(&task, &grammar, &value), &mut sink, &clock)
            .is_err());
        assert!(source.receipts().is_empty());
        assert_eq!(sink.generation(), 2);
        drop(source);
        assert_eq!(handler.runner.calls, 0);
        assert_eq!(ledger.committed(), 0);
    }
}

#[test]
fn post_ack_deadline_or_regressed_clock_prevents_runner_dispatch() {
    for (time, diagnostic) in [(10, "deadline_exceeded"), (-1, "clock_regressed")] {
        let (_, mut handler, task, grammar) = setup(compiled_proposal());
        let value = RetainedValue::I64(0);
        let shared = Rc::new(Cell::new(0));
        let mut ledger_clock = Clock(shared.clone());
        let mut ledger = CumulativeBudgetLedger::with_deadline(1, 10, &mut ledger_clock);
        let clock = CheckpointClock(shared.clone());
        let cap = ModelInvokeCapability::grant("test");
        let mut store = Store {
            advance_at: Some((3, shared, time)),
            ..Store::default()
        };
        let mut sink = SourceCheckpointSink::new(&mut store, binding(&task, &grammar, 4096, 1));
        begin(&mut sink);
        let mut source = source(&mut handler, &cap, grammar.clone(), &mut ledger, 1, 4096);
        let errors = source
            .propose_checkpointed(request(&task, &grammar, &value), &mut sink, &clock)
            .unwrap_err();
        assert!(format!("{errors:?}").contains(diagnostic));
        assert!(matches!(
            sink.journal().entries().last(),
            Some(SourceJournalEntry::AttemptFailed { .. })
        ));
        drop(source);
        assert_eq!(handler.runner.calls, 0);
        assert_eq!(ledger.committed(), 1);
    }
}

#[test]
fn post_dispatch_ack_loss_poisoned_sink_cannot_redispatch() {
    for actually_committed in [false, true] {
        let (_, mut handler, task, grammar) = setup(compiled_proposal());
        let value = RetainedValue::I64(0);
        let shared = Rc::new(Cell::new(0));
        let mut ledger_clock = Clock(shared.clone());
        let mut ledger = CumulativeBudgetLedger::with_deadline(1, 10, &mut ledger_clock);
        let clock = CheckpointClock(shared.clone());
        let cap = ModelInvokeCapability::grant("test");
        let mut store = Store {
            fail_at: Some((4, actually_committed)),
            ..Store::default()
        };
        let mut sink = SourceCheckpointSink::new(&mut store, binding(&task, &grammar, 4096, 1));
        begin(&mut sink);
        let mut source = source(&mut handler, &cap, grammar.clone(), &mut ledger, 1, 4096);
        assert!(source
            .propose_checkpointed(request(&task, &grammar, &value), &mut sink, &clock)
            .is_err());
        assert!(sink.poisoned());
        assert!(source
            .propose_checkpointed(request(&task, &grammar, &value), &mut sink, &clock)
            .is_err());
        drop(source);
        assert_eq!(handler.runner.calls, 1);
        assert_eq!(ledger.committed(), 1);
        drop(sink);
        let recovered = semaprax::live_invocation::source_journal::recover_source_checkpoint(
            &store.document,
            &binding(&task, &grammar, 4096, 1),
        )
        .unwrap();
        assert_eq!(recovered.committed_reserved_units(), 1);
        assert_eq!(recovered.is_uncertain(), !actually_committed);
        assert_eq!(
            recovered.generation(),
            if actually_committed { 4 } else { 3 }
        );
    }
}

#[test]
fn malformed_proposal_text_is_settled_and_retained_for_the_driver_decoder() {
    let (_, mut handler, task, grammar) = setup("not-a-canonical-proposal\n".into());
    let value = RetainedValue::I64(0);
    let shared = Rc::new(Cell::new(0));
    let mut ledger_clock = Clock(shared.clone());
    let mut ledger = CumulativeBudgetLedger::with_deadline(1, 10, &mut ledger_clock);
    let clock = CheckpointClock(shared.clone());
    let cap = ModelInvokeCapability::grant("test");
    let mut store = Store::default();
    let mut sink = SourceCheckpointSink::new(&mut store, binding(&task, &grammar, 4096, 1));
    begin(&mut sink);
    let mut source = source(&mut handler, &cap, grammar.clone(), &mut ledger, 1, 4096);
    assert_eq!(
        source
            .propose_checkpointed(request(&task, &grammar, &value), &mut sink, &clock)
            .unwrap(),
        "not-a-canonical-proposal\n"
    );
    assert!(
        matches!(sink.journal().entries().last(), Some(SourceJournalEntry::AttemptSettled { response, .. }) if response == b"not-a-canonical-proposal\n")
    );
}

#[test]
fn post_response_deadline_retains_raw_settlement_but_exposes_no_proposal() {
    let (_, mut handler, task, grammar) = setup(compiled_proposal());
    let value = RetainedValue::I64(0);
    let shared = Rc::new(Cell::new(0));
    let mut ledger_clock = Clock(shared.clone());
    let mut ledger = CumulativeBudgetLedger::with_deadline(1, 10, &mut ledger_clock);
    let clock = CheckpointClock(shared.clone());
    let cap = ModelInvokeCapability::grant("test");
    let mut store = Store {
        advance_at: Some((4, shared.clone(), 10)),
        ..Store::default()
    };
    let mut sink = SourceCheckpointSink::new(&mut store, binding(&task, &grammar, 4096, 1));
    begin(&mut sink);
    let mut source = source(&mut handler, &cap, grammar.clone(), &mut ledger, 1, 4096);
    assert!(source
        .propose_checkpointed(request(&task, &grammar, &value), &mut sink, &clock)
        .is_err());
    assert_eq!(source.receipts().len(), 1);
    assert!(matches!(
        sink.journal().entries().last(),
        Some(SourceJournalEntry::AttemptSettled { .. })
    ));
}

#[test]
fn response_arriving_at_deadline_is_a_charged_failed_attempt() {
    let answer = compiled_proposal();
    let (_, mut handler, task, grammar) = setup(answer.clone());
    let value = RetainedValue::I64(0);
    let shared = Rc::new(Cell::new(0));
    handler.runner.advance_clock = Some((shared.clone(), 10));
    let mut ledger_clock = Clock(shared.clone());
    let mut ledger = CumulativeBudgetLedger::with_deadline(1, 10, &mut ledger_clock);
    let clock = CheckpointClock(shared);
    let cap = ModelInvokeCapability::grant("test");
    let mut store = Store::default();
    let mut sink = SourceCheckpointSink::new(&mut store, binding(&task, &grammar, 4096, 1));
    begin(&mut sink);
    let mut source = source(&mut handler, &cap, grammar.clone(), &mut ledger, 1, 4096);
    let errors = source
        .propose_checkpointed(request(&task, &grammar, &value), &mut sink, &clock)
        .unwrap_err();
    assert!(format!("{errors:?}").contains("deadline_exceeded"));
    assert_eq!(
        sink.journal().entries().last(),
        Some(&SourceJournalEntry::AttemptFailed {
            turn: 0,
            attempt: 0,
            reason:
                semaprax::live_invocation::source_journal::SourceAttemptFailure::DeadlineExceeded,
            attempted_bytes: answer.len(),
        })
    );
    assert!(source.receipts()[0].usage.failed);
    drop(source);
    assert_eq!(handler.runner.calls, 1);
    assert_eq!(ledger.committed(), 1);
}

#[test]
fn settlement_policy_failure_stays_selected_when_store_clock_later_regresses() {
    struct PolicyClock {
        time: Rc<Cell<i64>>,
        reads: Cell<usize>,
    }
    impl InvocationClock for PolicyClock {
        fn now_millis(&self) -> i64 {
            let reads = self.reads.get() + 1;
            self.reads.set(reads);
            // Reserve, post-intent guard, and post-response guard succeed;
            // settlement's own deadline check is the first selected failure.
            if reads == 4 {
                self.time.set(10);
            }
            self.time.get()
        }
    }
    let (_, mut handler, task, grammar) = setup(compiled_proposal());
    let value = RetainedValue::I64(0);
    let shared = Rc::new(Cell::new(0));
    let mut policy_clock = PolicyClock {
        time: shared.clone(),
        reads: Cell::new(0),
    };
    let mut ledger = CumulativeBudgetLedger::with_deadline(1, 10, &mut policy_clock);
    let clock = CheckpointClock(shared.clone());
    let cap = ModelInvokeCapability::grant("test");
    let mut store = Store {
        advance_at: Some((4, shared, -1)),
        ..Store::default()
    };
    let mut sink = SourceCheckpointSink::new(&mut store, binding(&task, &grammar, 4096, 1));
    begin(&mut sink);
    let mut source = source(&mut handler, &cap, grammar.clone(), &mut ledger, 1, 4096);
    let errors = source
        .propose_checkpointed(request(&task, &grammar, &value), &mut sink, &clock)
        .unwrap_err();
    assert!(format!("{errors:?}").contains("deadline_exceeded"));
    assert!(!format!("{errors:?}").contains("clock_regressed"));
    assert!(matches!(
        sink.journal().entries().last(),
        Some(SourceJournalEntry::AttemptFailed {
            reason:
                semaprax::live_invocation::source_journal::SourceAttemptFailure::DeadlineExceeded,
            ..
        })
    ));
    drop(source);
    assert_eq!(handler.runner.calls, 1);
    assert_eq!(ledger.committed(), 1);
}
