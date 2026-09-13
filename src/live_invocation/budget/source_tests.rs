//! Source recovery restores policy from checked journal bytes, never a fresh
//! caller-provided ceiling or an unverified claimed spent total.
use super::*;
use std::rc::Rc;

use crate::agent_lifecycle::{CheckpointStore, CheckpointStoreError};
use crate::live_invocation::source_journal::{
    recover_source_checkpoint, RecoveredSourceCheckpoint, SourceCheckpointSink,
    SourceInvocationBinding, SourceInvocationSeed, SourceJournalEntry,
};

struct Clock {
    now: Rc<Cell<i64>>,
    domain: &'static str,
}
impl InvocationClock for Clock {
    fn now_millis(&self) -> i64 {
        self.now.get()
    }
}
impl SourceInvocationClock for Clock {
    fn clock_domain(&self) -> &str {
        self.domain
    }
}
#[derive(Default)]
struct Store(String);
impl CheckpointStore for Store {
    fn commit(&mut self, _: u64, document: &str) -> Result<(), CheckpointStoreError> {
        self.0 = document.to_owned();
        Ok(())
    }
}
fn digest() -> String {
    format!("sha256:{}", "a".repeat(64))
}
fn uncertain_checkpoint() -> RecoveredSourceCheckpoint {
    let binding = SourceInvocationBinding::bind(SourceInvocationSeed {
        lifecycle_digest: digest(),
        source_revision: digest(),
        deployment_binding: digest(),
        task: b"source budget recovery".to_vec(),
        task_budget: 5,
        proposal_schema_digest: digest(),
        response_limit: 1024,
        max_iterations: 2,
        max_stages: 7,
        max_attempts: 4,
        max_steps_per_stage: 100,
        max_total_steps: 700,
        ceiling: 5,
        reservation_units: 3,
        unit: "operator_microcredit_v1".into(),
        clock_domain: "fixture_restart_stable_millis_v1".into(),
        initial_millis: 0,
        deadline_millis: 10,
        program_root: None,
    })
    .unwrap();
    let mut store = Store::default();
    {
        let mut sink = SourceCheckpointSink::new(&mut store, binding.clone());
        sink.append_at(SourceJournalEntry::RunOpened, 0).unwrap();
        sink.append_at(
            SourceJournalEntry::TurnObserved {
                turn: 0,
                state: digest(),
                observation: digest(),
                feedback: digest(),
            },
            7,
        )
        .unwrap();
        sink.append_at(
            SourceJournalEntry::AttemptIntent {
                turn: 0,
                attempt: 0,
                attempt_digest: binding.attempt_digest(0, 0, &digest(), &digest(), 12),
                request_digest: digest(),
                prompt_digest: digest(),
                request_bytes: 12,
                reserved_units: 3,
                response_limit: 1024,
            },
            8,
        )
        .unwrap();
    }
    recover_source_checkpoint(&store.0, &binding).unwrap()
}
fn clock(now: i64) -> Clock {
    Clock {
        now: Rc::new(Cell::new(now)),
        domain: "fixture_restart_stable_millis_v1",
    }
}
fn request(units: i64) -> ModelInvocationRequest {
    ModelInvocationRequest {
        turn: 0,
        task: b"source budget recovery".to_vec(),
        observation: vec![],
        proposal_grammar_digest: digest(),
        deployment_binding: digest(),
        max_response_bytes: 1024,
        effective_budget: units,
    }
}

#[test]
fn source_recovery_keeps_uncertain_charge_and_rejects_a_smaller_reservation() {
    let checkpoint = uncertain_checkpoint();
    let mut time = clock(9);
    let mut ledger = CumulativeBudgetLedger::resume_source(&checkpoint, &mut time).unwrap();
    assert_eq!(ledger.committed(), 3);
    assert_eq!(ledger.remaining(), 2);
    assert_eq!(
        ledger.reserve(&request(2)).unwrap_err().0,
        RESERVATION_MISMATCH
    );
    assert_eq!(ledger.reserve(&request(3)).unwrap_err().0, BUDGET_EXHAUSTED);
    assert_eq!(ledger.committed(), 3);
}

#[test]
fn source_recovery_refuses_changed_epoch_regression_and_exact_expiry() {
    let checkpoint = uncertain_checkpoint();
    for (domain, now, expected) in [
        ("fresh_process_instant", 9, CLOCK_DOMAIN_MISMATCH),
        ("fixture_restart_stable_millis_v1", 7, CLOCK_REGRESSED),
        ("fixture_restart_stable_millis_v1", 10, DEADLINE_EXCEEDED),
        ("fixture_restart_stable_millis_v1", 11, DEADLINE_EXCEEDED),
    ] {
        let mut time = Clock {
            now: Rc::new(Cell::new(now)),
            domain,
        };
        let error = CumulativeBudgetLedger::resume_source(&checkpoint, &mut time)
            .err()
            .expect("changed or expired source clock must fail before dispatch");
        assert_eq!(error.0, expected);
    }
    let mut time = clock(8);
    assert!(CumulativeBudgetLedger::resume_source(&checkpoint, &mut time).is_ok());
}

#[test]
fn source_clock_checks_cannot_refund_elapsed_time_after_restore() {
    let checkpoint = uncertain_checkpoint();
    let mut time = clock(8);
    let now = time.now.clone();
    let ledger = CumulativeBudgetLedger::resume_source(&checkpoint, &mut time).unwrap();
    now.set(9);
    ledger.check_deadline().unwrap();
    now.set(8);
    assert_eq!(ledger.check_deadline().unwrap_err().0, CLOCK_REGRESSED);
    now.set(10);
    assert_eq!(ledger.check_deadline().unwrap_err().0, DEADLINE_EXCEEDED);
    assert_eq!(ledger.committed(), 3);
}
