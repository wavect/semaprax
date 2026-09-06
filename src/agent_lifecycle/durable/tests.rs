//! Crate-internal gates for the durable path's authority invariants.
//!
//! Two of these cannot be observed from outside the crate. The first is a
//! journal forgery that is *internally consistent*: rewritten and rechained,
//! so it decodes exactly like a genuine checkpoint. Building one needs the
//! crate-private journal vocabulary, and the point it proves is the one the
//! public surface cannot demonstrate — that even a checkpoint an attacker
//! wrote produces no authority, because the resume derives its grant from the
//! validated authorizing transition and only then compares. The second is the
//! journal chain's own sensitivity to length, order and substitution.
//!
//! The public terminal behaviour, the crash boundaries, the drift rejections,
//! the redaction policy and the storage contract are owned by the
//! `agent_runtime_v1` harness.

use crate::agent_lifecycle::AuthorizedRequest;

use super::super::tests::{proposal, DEFINITION, MODULE, RUNTIME_V1};
use super::*;

const FIXTURE_PATH: &str = "agent-checkpoint-unit.spx";

/// A store that keeps every generation and fails nothing.
#[derive(Default)]
struct Store(Vec<String>);

impl CheckpointStore for Store {
    fn commit(&mut self, _: u64, document: &str) -> Result<(), CheckpointStoreError> {
        self.0.push(document.to_owned());
        Ok(())
    }
}

struct Counting(usize);

impl AgentReadOperation for Counting {
    fn read(&mut self, _: &AuthorizedRequest) -> Option<Vec<u8>> {
        self.0 += 1;
        Some(b"observed".to_vec())
    }
}

/// A read operation that fails the test if the boundary is ever approached.
struct Never;

impl AgentReadOperation for Never {
    fn read(&mut self, _: &AuthorizedRequest) -> Option<Vec<u8>> {
        panic!("a resumed run reached the external boundary");
    }
}

fn fixture() -> DurableAgent {
    let (definition_v2, deployment) = crate::agent_deployment::migrate_agent_definition_v1(
        &DEFINITION.replace("RUNTIME", RUNTIME_V1),
        "fixture.deployment.local",
    )
    .expect("the fixture definition migrates");
    let bound = crate::agent_deployment::bind_agent_deployment(&definition_v2, &deployment)
        .expect("the fixture deployment binds");
    bind_durable_agent(MODULE, FIXTURE_PATH, &bound, 7).expect("the fixture binds for durability")
}

fn task() -> LifecycleTask {
    LifecycleTask {
        objective: b"alpha".to_vec(),
        budget: 12,
    }
}

fn seeded(byte: &str) -> String {
    format!("sha256:{}", byte.repeat(32))
}

#[test]
fn a_forged_but_internally_consistent_journal_cannot_mint_an_authorization() {
    let agent = fixture();
    let document = proposal(agent.lifecycle(), "5", "1");
    let mut read = Counting(0);
    let mut store = Store::default();
    agent
        .start(
            &task(),
            &document,
            &mut read,
            DurableBudget::default(),
            Retention::ObservationBytes,
            &AgentCancellation::new(),
            &mut store,
            CrashPoint::AfterSettlement,
        )
        .expect("the fixture runs");
    assert_eq!(read.0, 1);
    let genuine = AgentCheckpoint::decode(store.0.last().expect("a generation"))
        .expect("the genuine generation decodes");
    assert_eq!(genuine.program_counter(), ProgramCounter::Settled);

    // Substitute the recorded operation identity and rechain the journal, so
    // the forged document is a fully valid checkpoint: it decodes, its chain
    // recomputes, and its counter agrees with its journal.
    let substituted = seeded("ab");
    let forged_journal = genuine
        .journal()
        .iter()
        .map(|entry| match entry {
            JournalEntry::Intent { granted_budget, .. } => JournalEntry::Intent {
                operation: substituted.clone(),
                granted_budget: *granted_budget,
            },
            JournalEntry::Settled {
                observation_digest,
                observation,
                ..
            } => JournalEntry::Settled {
                operation: substituted.clone(),
                observation_digest: observation_digest.clone(),
                observation: observation.clone(),
            },
            other => other.clone(),
        })
        .collect::<Vec<_>>();
    let forged = AgentCheckpoint::seal(
        genuine.generation(),
        agent.lifecycle().agent_id(),
        agent.binding().clone(),
        genuine.budgets(),
        genuine.retention(),
        genuine.task_digest(),
        forged_journal,
    )
    .expect("the forgery seals");
    let reloaded =
        AgentCheckpoint::decode(forged.document()).expect("the forgery is a valid checkpoint");
    assert_eq!(reloaded.digest(), forged.digest());
    assert_ne!(reloaded.digest(), genuine.digest());
    assert_eq!(reloaded.operation_identity(), Some(substituted.as_str()));

    // The resume mints a fresh authorization from the validated authorizing
    // transition, finds it does not name the recorded operation, and drops it
    // unspent without approaching the boundary or writing a generation.
    let mut never = Never;
    let mut sink = Store::default();
    let run = agent
        .resume(
            &reloaded,
            &task(),
            &document,
            &mut never,
            Reconciliation::None,
            &AgentCancellation::new(),
            &mut sink,
        )
        .expect("the resume is a run, not a fault");
    assert_eq!(run.status(), DurableStatus::Rejected);
    assert_eq!(run.reason(), "operation_identity_mismatch");
    assert_eq!(run.boundary_crossings(), 0);
    assert!(run.result().is_none());
    assert!(sink.0.is_empty());

    // Even reconciliation, which supplies an observation rather than
    // performing one, cannot carry a forged identity past the same check.
    let run = agent
        .resume(
            &reloaded,
            &task(),
            &document,
            &mut never,
            Reconciliation::Settled(b"observed"),
            &AgentCancellation::new(),
            &mut sink,
        )
        .expect("the resume is a run, not a fault");
    assert_eq!(run.reason(), "operation_identity_mismatch");
    assert!(sink.0.is_empty());
}

#[test]
fn the_journal_chain_detects_truncation_reordering_and_substitution() {
    let prefix = JournalEntry::Prefix {
        state_digest: seeded("11"),
        proposal_digest: seeded("22"),
    };
    let intent = JournalEntry::Intent {
        operation: seeded("33"),
        granted_budget: 5,
    };
    let reduced = JournalEntry::Reduced {
        result_digest: seeded("44"),
    };
    let whole = journal::chain(&[prefix.clone(), intent.clone(), reduced.clone()]);

    assert_eq!(
        whole,
        journal::chain(&[prefix.clone(), intent.clone(), reduced.clone()])
    );
    // Truncation.
    assert_ne!(whole, journal::chain(&[prefix.clone(), intent.clone()]));
    assert_ne!(whole, journal::chain(&[prefix.clone()]));
    // Reordering, which also renumbers every entry it moves.
    assert_ne!(
        whole,
        journal::chain(&[prefix.clone(), reduced.clone(), intent.clone()])
    );
    // Substitution of one field of one entry.
    assert_ne!(
        whole,
        journal::chain(&[
            prefix,
            JournalEntry::Intent {
                operation: seeded("33"),
                granted_budget: 6,
            },
            reduced,
        ])
    );
}

#[test]
fn a_terminal_counter_is_never_reachable_with_a_live_grant() {
    // `tail` treats a terminal counter as a fail-closed invariant, because
    // `resume` answers both terminal counters before it re-runs any stage.
    // This is the executable statement of that ordering.
    let agent = fixture();
    let document = proposal(agent.lifecycle(), "5", "1");
    let mut read = Counting(0);
    let mut store = Store::default();
    agent
        .start(
            &task(),
            &document,
            &mut read,
            DurableBudget::default(),
            Retention::ObservationBytes,
            &AgentCancellation::new(),
            &mut store,
            CrashPoint::Never,
        )
        .expect("the fixture runs");
    let delivered = AgentCheckpoint::decode(store.0.last().expect("a generation")).unwrap();
    assert_eq!(delivered.program_counter(), ProgramCounter::Delivered);

    let mut never = Never;
    let mut sink = Store::default();
    let run = agent
        .resume(
            &delivered,
            &task(),
            &document,
            &mut never,
            Reconciliation::None,
            &AgentCancellation::new(),
            &mut sink,
        )
        .expect("the resume is a run, not a fault");
    assert_eq!(run.status(), DurableStatus::AlreadyDelivered);
    assert!(run.stages().is_empty());
    assert!(sink.0.is_empty());
}
