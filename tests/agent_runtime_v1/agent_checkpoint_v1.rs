//! Agent Checkpoint v1: a revision-bound opaque checkpoint, crash injection at
//! the five boundaries of the single external operation, and a resume that
//! never repeats an uncertain call and never mints an authorization from
//! stored bytes.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use semaprax::agent_definition::compile_agent_definition;
use semaprax::agent_deployment::{bind_agent_deployment, compile_agent_definition_v2};
use semaprax::agent_lifecycle::{
    bind_durable_agent, AgentCheckpoint, AgentReadOperation, AuthorizedRequest, CheckpointStore,
    CheckpointStoreError, CrashPoint, DurableAgent, DurableBudget, DurableStatus, LifecycleTask,
    ProgramCounter, Reconciliation, Retention,
};
use semaprax::agent_runtime::AgentCancellation;

use super::agent_definition_v1::definition;
use super::agent_deployment_v1::migrated;
use super::agent_lifecycle_v1::{proposal, MODULE, MODULE_PATH};
use super::{profile, raw_sha};

/// A caller-owned in-memory store that keeps every generation it accepted, so
/// a test can inspect what a recovery would have found at any point.
#[derive(Default)]
struct Memory {
    generations: Vec<(u64, String)>,
    fail_at: Option<u64>,
}

impl Memory {
    fn failing(generation: u64) -> Self {
        Self {
            generations: Vec::new(),
            fail_at: Some(generation),
        }
    }

    /// The generation a recovery would adopt.
    fn active(&self) -> AgentCheckpoint {
        AgentCheckpoint::decode(&self.generations.last().expect("one generation").1).unwrap()
    }
}

impl CheckpointStore for Memory {
    fn commit(&mut self, generation: u64, document: &str) -> Result<(), CheckpointStoreError> {
        if self.fail_at == Some(generation) {
            return Err(CheckpointStoreError);
        }
        self.generations.push((generation, document.to_owned()));
        Ok(())
    }
}

/// The single registered external read. It counts every boundary crossing, so
/// a repeated uncertain call would be visible.
struct Read {
    value: Vec<u8>,
    fails: bool,
    calls: usize,
}

impl Read {
    fn new() -> Self {
        Self {
            value: b"observed".to_vec(),
            fails: false,
            calls: 0,
        }
    }
}

impl AgentReadOperation for Read {
    fn read(&mut self, request: &AuthorizedRequest) -> Option<Vec<u8>> {
        self.calls += 1;
        assert_eq!(request.seal(), b"AZ");
        (!self.fails).then(|| self.value.clone())
    }
}

fn durable_agent(policy_epoch: u64) -> DurableAgent {
    let (definition_v2, deployment) = migrated();
    let bound = bind_agent_deployment(&definition_v2, &deployment).unwrap();
    bind_durable_agent(MODULE, MODULE_PATH, &bound, policy_epoch).unwrap()
}

/// Rebinds a mutated semantic definition to the deployment that accompanies it.
fn repaired(definition_v2: &str, deployment: &str) -> String {
    let digest = compile_agent_definition_v2(definition_v2)
        .unwrap()
        .digest()
        .to_owned();
    let marker = "\"definition_digest\":\"";
    let start = deployment.find(marker).unwrap() + marker.len();
    let end = start + "sha256:".len() + 64;
    format!("{}{digest}{}", &deployment[..start], &deployment[end..])
}

fn task() -> LifecycleTask {
    LifecycleTask {
        objective: b"alpha".to_vec(),
        budget: 12,
    }
}

fn document(agent: &DurableAgent) -> String {
    proposal(
        agent.lifecycle().proposal_schema().schema().digest(),
        "5",
        false,
        "1",
    )
}

fn scratch(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "semaprax-checkpoint-{label}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn a_durable_run_completes_and_binds_every_generation_to_its_revision() {
    let agent = durable_agent(7);
    let mut read = Read::new();
    let mut store = Memory::default();
    let run = agent
        .start(
            &task(),
            &document(&agent),
            &mut read,
            DurableBudget::default(),
            Retention::ObservationBytes,
            &AgentCancellation::new(),
            &mut store,
            CrashPoint::Never,
        )
        .unwrap();
    assert_eq!(run.status(), DurableStatus::Completed);
    assert_eq!(run.reason(), "reduce_published_a_result");
    assert_eq!(read.calls, 1);
    assert_eq!(run.boundary_crossings(), 1);
    assert!(run.result().is_some());

    // One generation per journal boundary: prefix, intent, settlement,
    // reduction, delivery.
    let generations = store
        .generations
        .iter()
        .map(|(generation, _)| *generation)
        .collect::<Vec<_>>();
    assert_eq!(generations, [1, 2, 3, 4, 5]);
    let counters = store
        .generations
        .iter()
        .map(|(_, document)| {
            AgentCheckpoint::decode(document)
                .unwrap()
                .program_counter()
                .name()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        counters,
        ["prefix", "intent", "settled", "reduced", "delivered"]
    );

    // Every generation is a self-verifying document that decodes back to
    // exactly itself, and each names the revision it is bound to.
    let active = store.active();
    assert_eq!(active.program_counter(), ProgramCounter::Delivered);
    assert_eq!(active.effect_grants_remaining(), 0);
    assert!(active.document().ends_with('\n'));
    assert_eq!(
        AgentCheckpoint::decode(active.document()).unwrap().digest(),
        active.digest()
    );
    for field in [
        "\"bound_digest\":\"sha256:",
        "\"definition_digest\":\"sha256:",
        "\"deployment_digest\":\"sha256:",
        "\"source_digest\":\"sha256:",
        "\"lifecycle_digest\":\"sha256:",
        "\"proposal_schema_digest\":\"sha256:",
        "\"state_schema\":\"fixture.agent.type.state\"",
        "\"policy_epoch\":\"7\"",
    ] {
        assert!(active.document().contains(field), "missing `{field}`");
    }
    assert_eq!(
        run.checkpoint().map(AgentCheckpoint::digest),
        Some(active.digest())
    );

    // The checkpoint is opaque: no authorization value, no grant seal, no
    // state carrier, no task payload, no proposal payload.
    for absent in [
        "AZ",
        "415a",
        "alpha",
        "616c706861",
        "objective",
        "urgent",
        "Granted",
        "fixture.agent.fn.",
    ] {
        assert!(
            !active.document().contains(absent),
            "checkpoint bytes carry `{absent}`"
        );
    }
    // The declared retention does carry the settled observation's bytes.
    assert!(store.generations[2].1.contains("6f62736572766564"));

    // Evidence carries identities, digests and counts only.
    assert!(run.evidence().ends_with('\n'));
    assert!(run.evidence().contains("\"status\":\"completed\""));
    assert!(!run.evidence().contains("observed"));
    assert!(!run.evidence().contains("alpha"));
    assert!(run.evidence_digest().starts_with("sha256:"));
}

/// Every crash boundary, its durable outcome, and what the resume that follows
/// it is and is not allowed to do.
#[test]
fn crash_injection_at_five_boundaries_has_specified_outcomes() {
    let agent = durable_agent(7);
    let proposal = document(&agent);
    let expected = {
        let mut read = Read::new();
        let mut store = Memory::default();
        agent
            .start(
                &task(),
                &proposal,
                &mut read,
                DurableBudget::default(),
                Retention::ObservationBytes,
                &AgentCancellation::new(),
                &mut store,
                CrashPoint::Never,
            )
            .unwrap()
            .result_digest()
            .unwrap()
            .to_owned()
    };

    // Crash point, its reason, the counter a recovery finds, how many times
    // the boundary had been crossed at the crash, and how many times it is
    // ever crossed in total once the run has been resumed to delivery.
    for (crash, reason, counter, calls_at_crash, total_calls) in [
        (
            CrashPoint::BeforeIntent,
            "crashed_before_intent",
            ProgramCounter::Prefix,
            0,
            1,
        ),
        (
            CrashPoint::AfterIntent,
            "crashed_after_intent",
            ProgramCounter::Intent,
            0,
            0,
        ),
        (
            CrashPoint::AfterEffect,
            "crashed_after_effect",
            ProgramCounter::Intent,
            1,
            1,
        ),
        (
            CrashPoint::AfterSettlement,
            "crashed_after_settlement",
            ProgramCounter::Settled,
            1,
            1,
        ),
        (
            CrashPoint::BeforeDelivery,
            "crashed_before_delivery",
            ProgramCounter::Reduced,
            1,
            1,
        ),
    ] {
        let mut read = Read::new();
        let mut store = Memory::default();
        let crashed = agent
            .start(
                &task(),
                &proposal,
                &mut read,
                DurableBudget::default(),
                Retention::ObservationBytes,
                &AgentCancellation::new(),
                &mut store,
                crash,
            )
            .unwrap();
        assert_eq!(crashed.status(), DurableStatus::Crashed, "{reason}");
        assert_eq!(crashed.reason(), reason);
        assert!(crashed.result().is_none());
        assert_eq!(read.calls, calls_at_crash, "{reason}");
        let stored = store.active();
        assert_eq!(stored.program_counter(), counter, "{reason}");

        // Recovery resumes from exactly the bytes a restarted process would
        // read, with no reconciliation offered.
        let resumed = agent
            .resume(
                &stored,
                &task(),
                &proposal,
                &mut read,
                Reconciliation::None,
                &AgentCancellation::new(),
                &mut store,
            )
            .unwrap();
        match counter {
            // The intent was never durable, so the boundary was never
            // approached: the resume performs it exactly once.
            ProgramCounter::Prefix => {
                assert_eq!(resumed.status(), DurableStatus::Completed, "{reason}");
                assert_eq!(read.calls, 1);
                assert_eq!(resumed.boundary_crossings(), 1);
            }
            // Delivery is uncertain. Never retried, with or without the effect
            // actually having happened.
            ProgramCounter::Intent => {
                assert_eq!(resumed.status(), DurableStatus::Unknown, "{reason}");
                assert_eq!(
                    resumed.reason(),
                    "uncertain_delivery_requires_reconciliation"
                );
                assert_eq!(read.calls, calls_at_crash, "{reason}");
                assert_eq!(resumed.boundary_crossings(), 0);
                assert!(resumed.result().is_none());
                // The checkpoint stays reconcilable rather than terminal.
                assert_eq!(store.active().program_counter(), ProgramCounter::Intent);

                let current = store.active();
                let reconciled = agent
                    .resume(
                        &current,
                        &task(),
                        &proposal,
                        &mut read,
                        Reconciliation::Settled(b"observed"),
                        &AgentCancellation::new(),
                        &mut store,
                    )
                    .unwrap();
                assert_eq!(reconciled.status(), DurableStatus::Completed, "{reason}");
                assert_eq!(reconciled.result_digest(), Some(expected.as_str()));
                assert_eq!(read.calls, calls_at_crash, "{reason}");
                assert_eq!(reconciled.boundary_crossings(), 0);
            }
            // The operation settled durably: the resume completes from the
            // recorded observation without approaching the boundary.
            ProgramCounter::Settled | ProgramCounter::Reduced => {
                assert_eq!(resumed.status(), DurableStatus::Completed, "{reason}");
                assert_eq!(read.calls, total_calls, "{reason}");
                assert_eq!(resumed.boundary_crossings(), 0);
                assert_eq!(resumed.result_digest(), Some(expected.as_str()));
            }
            _ => unreachable!(),
        }
        // The boundary is never crossed more often than the crash left it,
        // and a resume that reconciles instead of performing crosses it not
        // at all.
        assert_eq!(read.calls, total_calls, "{reason}");
        assert_eq!(store.active().program_counter(), ProgramCounter::Delivered);
        assert_eq!(store.active().effect_grants_remaining(), 0);

        // A delivered checkpoint performs nothing at all.
        let current = store.active();
        let again = agent
            .resume(
                &current,
                &task(),
                &proposal,
                &mut read,
                Reconciliation::None,
                &AgentCancellation::new(),
                &mut store,
            )
            .unwrap();
        assert_eq!(again.status(), DurableStatus::AlreadyDelivered);
        assert_eq!(again.reason(), "result_already_delivered");
        assert!(again.stages().is_empty());
        assert_eq!(read.calls, total_calls, "{reason}");
    }
}

#[test]
fn an_uncertain_operation_can_be_abandoned_but_never_retried() {
    let agent = durable_agent(7);
    let proposal = document(&agent);
    let mut read = Read::new();
    let mut store = Memory::default();
    agent
        .start(
            &task(),
            &proposal,
            &mut read,
            DurableBudget::default(),
            Retention::ObservationBytes,
            &AgentCancellation::new(),
            &mut store,
            CrashPoint::AfterEffect,
        )
        .unwrap();
    assert_eq!(read.calls, 1);

    let current = store.active();
    let abandoned = agent
        .resume(
            &current,
            &task(),
            &proposal,
            &mut read,
            Reconciliation::Abandoned,
            &AgentCancellation::new(),
            &mut store,
        )
        .unwrap();
    assert_eq!(abandoned.status(), DurableStatus::Abandoned);
    assert_eq!(abandoned.reason(), "host_reconciled_to_abandonment");
    assert!(abandoned.result().is_none());
    assert_eq!(read.calls, 1);
    assert_eq!(store.active().program_counter(), ProgramCounter::Abandoned);

    // Abandonment is terminal and re-runs no stage.
    let current = store.active();
    let again = agent
        .resume(
            &current,
            &task(),
            &proposal,
            &mut read,
            Reconciliation::Settled(b"observed"),
            &AgentCancellation::new(),
            &mut store,
        )
        .unwrap();
    assert_eq!(again.status(), DurableStatus::Abandoned);
    assert_eq!(again.reason(), "operation_already_abandoned");
    assert!(again.stages().is_empty());
    assert_eq!(read.calls, 1);
}

#[test]
fn a_reported_effect_failure_stays_uncertain_and_a_store_failure_fails_closed() {
    let agent = durable_agent(7);
    let proposal = document(&agent);

    // A read that reports failure is not evidence of non-occurrence.
    let mut read = Read::new();
    read.fails = true;
    let mut store = Memory::default();
    let run = agent
        .start(
            &task(),
            &proposal,
            &mut read,
            DurableBudget::default(),
            Retention::ObservationBytes,
            &AgentCancellation::new(),
            &mut store,
            CrashPoint::Never,
        )
        .unwrap();
    assert_eq!(run.status(), DurableStatus::EffectFailed);
    assert_eq!(run.reason(), "effect_reported_failure_delivery_uncertain");
    assert_eq!(store.active().program_counter(), ProgramCounter::Intent);
    let current = store.active();
    let stalled = agent
        .resume(
            &current,
            &task(),
            &proposal,
            &mut read,
            Reconciliation::None,
            &AgentCancellation::new(),
            &mut store,
        )
        .unwrap();
    assert_eq!(stalled.status(), DurableStatus::Unknown);
    assert_eq!(read.calls, 1);

    // A store that cannot make the intent durable never reaches the boundary.
    let mut read = Read::new();
    let mut store = Memory::failing(2);
    let run = agent
        .start(
            &task(),
            &proposal,
            &mut read,
            DurableBudget::default(),
            Retention::ObservationBytes,
            &AgentCancellation::new(),
            &mut store,
            CrashPoint::Never,
        )
        .unwrap();
    assert_eq!(run.status(), DurableStatus::StoreFailed);
    assert_eq!(run.reason(), "intent_not_durable");
    assert_eq!(read.calls, 0);
    assert_eq!(store.active().program_counter(), ProgramCounter::Prefix);
}

#[test]
fn a_resume_refunds_no_budget_and_an_exhausted_effect_grant_stays_exhausted() {
    let agent = durable_agent(7);
    let proposal = document(&agent);

    let mut read = Read::new();
    let mut store = Memory::default();
    let straight = agent
        .start(
            &task(),
            &proposal,
            &mut read,
            DurableBudget::default(),
            Retention::ObservationBytes,
            &AgentCancellation::new(),
            &mut store,
            CrashPoint::Never,
        )
        .unwrap();
    assert_eq!(straight.status(), DurableStatus::Completed);
    let uninterrupted = store.active().total_steps_remaining();

    let mut read = Read::new();
    let mut store = Memory::default();
    agent
        .start(
            &task(),
            &proposal,
            &mut read,
            DurableBudget::default(),
            Retention::ObservationBytes,
            &AgentCancellation::new(),
            &mut store,
            CrashPoint::AfterSettlement,
        )
        .unwrap();
    let at_crash = store.active().total_steps_remaining();
    let current = store.active();
    let resumed = agent
        .resume(
            &current,
            &task(),
            &proposal,
            &mut read,
            Reconciliation::None,
            &AgentCancellation::new(),
            &mut store,
        )
        .unwrap();
    assert_eq!(resumed.status(), DurableStatus::Completed);
    // Re-executing the deterministic prefix spends fuel from the same ledger,
    // so a resumed run always has strictly less left than one that never
    // crashed. Nothing is credited back.
    let after_resume = store.active().total_steps_remaining();
    assert!(after_resume < uninterrupted);
    assert!(after_resume < at_crash);
    assert_eq!(store.active().effect_grants_remaining(), 0);

    // An effect grant consumed at an intent is never restored, and a run that
    // starts with none never approaches the boundary.
    let mut read = Read::new();
    let mut store = Memory::default();
    let denied = agent
        .start(
            &task(),
            &proposal,
            &mut read,
            DurableBudget {
                effect_grants: 0,
                ..DurableBudget::default()
            },
            Retention::ObservationBytes,
            &AgentCancellation::new(),
            &mut store,
            CrashPoint::Never,
        )
        .unwrap();
    assert_eq!(denied.status(), DurableStatus::BudgetExhausted);
    assert_eq!(denied.reason(), "effect_grant_exhausted");
    assert_eq!(read.calls, 0);
    let current = store.active();
    let again = agent
        .resume(
            &current,
            &task(),
            &proposal,
            &mut read,
            Reconciliation::None,
            &AgentCancellation::new(),
            &mut store,
        )
        .unwrap();
    assert_eq!(again.status(), DurableStatus::BudgetExhausted);
    assert_eq!(read.calls, 0);
}

#[test]
fn definition_deployment_source_and_epoch_drift_reject_a_stale_checkpoint() {
    let agent = durable_agent(7);
    let proposal = document(&agent);
    let mut read = Read::new();
    let mut store = Memory::default();
    agent
        .start(
            &task(),
            &proposal,
            &mut read,
            DurableBudget::default(),
            Retention::ObservationBytes,
            &AgentCancellation::new(),
            &mut store,
            CrashPoint::AfterSettlement,
        )
        .unwrap();
    let stored = store.active();

    let resume_against = |other: &DurableAgent, reason: &str| {
        let mut read = Read::new();
        let mut sink = Memory::default();
        let run = other
            .resume(
                &stored,
                &task(),
                &proposal,
                &mut read,
                Reconciliation::None,
                &AgentCancellation::new(),
                &mut sink,
            )
            .unwrap();
        assert_eq!(run.status(), DurableStatus::Stale, "{reason}");
        assert_eq!(run.reason(), reason);
        // A stale checkpoint re-runs no stage and writes no generation.
        assert!(run.stages().is_empty(), "{reason}");
        assert!(sink.generations.is_empty(), "{reason}");
        assert_eq!(read.calls, 0, "{reason}");
    };

    // A revoked policy epoch.
    resume_against(&durable_agent(8), "policy_epoch_revoked");

    let (definition_v2, deployment) = migrated();

    // A changed source-owned semantic definition.
    let widened = definition_v2.replacen("\"max_turns\":2", "\"max_turns\":3", 1);
    assert_ne!(widened, definition_v2);
    let bound = bind_agent_deployment(&widened, &repaired(&widened, &deployment)).unwrap();
    resume_against(
        &bind_durable_agent(MODULE, MODULE_PATH, &bound, 7).unwrap(),
        "definition_drift",
    );

    // A substituted provider and model: the semantics are identical, the
    // deployment is not.
    let substituted = deployment
        .replace("fake.local", "other.local")
        .replace("fake-basic", "other-basic");
    let bound = bind_agent_deployment(&definition_v2, &substituted).unwrap();
    resume_against(
        &bind_durable_agent(MODULE, MODULE_PATH, &bound, 7).unwrap(),
        "deployment_drift",
    );

    let bound = bind_agent_deployment(&definition_v2, &deployment).unwrap();

    // A changed grant-seal identity is policy drift: the same state and
    // proposal authorize to a different value under it.
    let restaged = MODULE.replace(
        "fixture.agent.type.decision.granted.seal",
        "fixture.agent.type.decision.granted.witness",
    );
    resume_against(
        &bind_durable_agent(&restaged, MODULE_PATH, &bound, 7).unwrap(),
        "lifecycle_drift",
    );

    // A pure display rename changes nothing but the source bytes, and is
    // still refused: a checkpoint is bound to the revision, not to its
    // meaning alone.
    let renamed = MODULE.replace("fn reduce(state: own State", "fn reduced(state: own State");
    resume_against(
        &bind_durable_agent(&renamed, MODULE_PATH, &bound, 7).unwrap(),
        "source_drift",
    );

    // Different caller inputs are refused before any stage runs, because the
    // checkpoint holds their digests rather than the inputs themselves.
    let mut read = Read::new();
    let mut sink = Memory::default();
    let run = agent
        .resume(
            &stored,
            &LifecycleTask {
                objective: b"beta".to_vec(),
                budget: 12,
            },
            &proposal,
            &mut read,
            Reconciliation::None,
            &AgentCancellation::new(),
            &mut sink,
        )
        .unwrap();
    assert_eq!(run.status(), DurableStatus::Stale);
    assert_eq!(run.reason(), "task_drift");
    assert_eq!(read.calls, 0);

    let run = agent
        .resume(
            &stored,
            &task(),
            &proposal_for(&agent, "4"),
            &mut read,
            Reconciliation::None,
            &AgentCancellation::new(),
            &mut sink,
        )
        .unwrap();
    assert_eq!(run.status(), DurableStatus::Rejected);
    assert_eq!(run.reason(), "proposal_digest_mismatch");
    assert_eq!(read.calls, 0);
}

fn proposal_for(agent: &DurableAgent, budget: &str) -> String {
    proposal(
        agent.lifecycle().proposal_schema().schema().digest(),
        budget,
        false,
        "1",
    )
}

#[test]
fn truncated_reordered_and_mutated_checkpoints_never_decode() {
    let agent = durable_agent(7);
    let mut read = Read::new();
    let mut store = Memory::default();
    agent
        .start(
            &task(),
            &document(&agent),
            &mut read,
            DurableBudget::default(),
            Retention::ObservationBytes,
            &AgentCancellation::new(),
            &mut store,
            CrashPoint::Never,
        )
        .unwrap();
    let good = store.active().document().to_owned();
    AgentCheckpoint::decode(&good).unwrap();

    let mut rejected = vec![
        // A torn write of any length.
        good[..good.len() / 2].to_owned(),
        good[..good.len() - 1].to_owned(),
        String::new(),
        // A renumbered journal entry.
        good.replacen(
            ",{\"seq\":\"4\",\"kind\":\"delivered\"",
            ",{\"seq\":\"9\",\"kind\":\"delivered\"",
            1,
        ),
    ];
    // A dropped final journal entry: the chain no longer recomputes and the
    // stored program counter no longer agrees with the journal.
    let last = good.find(",{\"seq\":\"4\",\"kind\":\"delivered\"").unwrap();
    let close = good.find("],\"journal_link\"").unwrap();
    rejected.push(format!("{}{}", &good[..last], &good[close..]));
    // A transposed pair of journal entries: the sequence numbers no longer
    // ascend and the chain no longer recomputes.
    let intent_start = good.find("{\"seq\":\"1\",\"kind\":\"intent\"").unwrap();
    let settled_start = good.find("{\"seq\":\"2\",\"kind\":\"settled\"").unwrap();
    let reduced_start = good.find("{\"seq\":\"3\",\"kind\":\"reduced\"").unwrap();
    rejected.push(format!(
        "{}{},{}{}",
        &good[..intent_start],
        &good[settled_start..reduced_start - 1],
        &good[intent_start..settled_start - 1],
        &good[reduced_start - 1..]
    ));
    // A program counter that no longer agrees with its journal, a mutated
    // nonclaim list, and an added key.
    rejected.push(good.replacen(
        "\"program_counter\":\"delivered\"",
        "\"program_counter\":\"prefix\"",
        1,
    ));
    rejected.push(good.replacen("\"no_budget_refund_on_resume\",", "", 1));
    rejected.push(good.replacen("{\"schema\":", "{\"extra\":\"x\",\"schema\":", 1));

    for (index, candidate) in rejected.iter().enumerate() {
        let error = AgentCheckpoint::decode(candidate)
            .err()
            .unwrap_or_else(|| panic!("candidate {index} decoded"));
        assert_eq!(error[0].code, "SPX-G573", "candidate {index}");
    }
}

/// A checkpoint is not authenticated, and says so. What stops a rewritten
/// document is the live invocation's own re-derivation, not the bytes.
#[test]
fn a_rewritten_checkpoint_is_caught_by_the_live_binding_not_by_the_bytes() {
    let agent = durable_agent(7);
    let proposal = document(&agent);
    let mut read = Read::new();
    let mut store = Memory::default();
    agent
        .start(
            &task(),
            &proposal,
            &mut read,
            DurableBudget::default(),
            Retention::ObservationBytes,
            &AgentCancellation::new(),
            &mut store,
            CrashPoint::AfterSettlement,
        )
        .unwrap();
    let good = store.active().document().to_owned();

    // The document publishes its own storage dependence.
    assert!(good
        .contains("no_checkpoint_integrity_or_authenticity_without_the_callers_storage_contract"));

    // A rewritten policy epoch decodes — there is no key to stop it — and is
    // then refused by the live invocation it does not belong to.
    let forged = good.replacen("\"policy_epoch\":\"7\"", "\"policy_epoch\":\"8\"", 1);
    let decoded = AgentCheckpoint::decode(&forged).unwrap();
    assert_ne!(decoded.digest(), store.active().digest());
    let mut sink = Memory::default();
    let run = agent
        .resume(
            &decoded,
            &task(),
            &proposal,
            &mut read,
            Reconciliation::None,
            &AgentCancellation::new(),
            &mut sink,
        )
        .unwrap();
    assert_eq!(run.status(), DurableStatus::Stale);
    assert_eq!(run.reason(), "policy_epoch_revoked");
    assert_eq!(read.calls, 1);
    assert!(sink.generations.is_empty());
}

#[test]
fn a_redacted_observation_needs_a_matching_reconciliation() {
    const SENTINEL: &[u8] = b"SENTINEL-OBSERVATION-VALUE";
    let agent = durable_agent(7);
    let proposal = document(&agent);
    let mut read = Read {
        value: SENTINEL.to_vec(),
        fails: false,
        calls: 0,
    };
    let mut store = Memory::default();
    agent
        .start(
            &LifecycleTask {
                objective: b"SENTINEL-TASK-OBJECTIVE".to_vec(),
                budget: 12,
            },
            &proposal,
            &mut read,
            DurableBudget::default(),
            Retention::ObservationDigestOnly,
            &AgentCancellation::new(),
            &mut store,
            CrashPoint::AfterSettlement,
        )
        .unwrap();

    // Redaction is real at every generation: neither the caller's task nor
    // the external observation reaches checkpoint bytes in any encoding.
    let hex = |bytes: &[u8]| {
        bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    };
    for (_, generation) in &store.generations {
        for secret in [
            "SENTINEL-OBSERVATION-VALUE",
            "SENTINEL-TASK-OBJECTIVE",
            &hex(SENTINEL),
            &hex(b"SENTINEL-TASK-OBJECTIVE"),
        ] {
            assert!(!generation.contains(secret), "checkpoint carries {secret}");
        }
    }
    assert_eq!(store.active().retention(), Retention::ObservationDigestOnly);

    let resume = |reconciliation: Reconciliation<'_>| {
        let mut read = Read::new();
        let mut sink = Memory::default();
        let run = agent
            .resume(
                &store.active(),
                &LifecycleTask {
                    objective: b"SENTINEL-TASK-OBJECTIVE".to_vec(),
                    budget: 12,
                },
                &proposal,
                &mut read,
                reconciliation,
                &AgentCancellation::new(),
                &mut sink,
            )
            .unwrap();
        assert_eq!(read.calls, 0);
        (run.status(), run.reason())
    };
    assert_eq!(
        resume(Reconciliation::None),
        (
            DurableStatus::Unknown,
            "redacted_observation_requires_reconciliation"
        )
    );
    assert_eq!(
        resume(Reconciliation::Settled(b"a different value")),
        (
            DurableStatus::Rejected,
            "reconciled_observation_digest_mismatch"
        )
    );
    assert_eq!(
        resume(Reconciliation::Settled(SENTINEL)),
        (DurableStatus::Completed, "reduce_published_a_result")
    );
}

#[test]
fn a_commit_is_atomic_and_a_torn_generation_is_never_adopted() {
    /// The declared storage contract: write a sibling temporary and rename it
    /// over the target, so a reader sees either the previous whole generation
    /// or the new whole generation.
    struct Atomic {
        active: PathBuf,
        staging: PathBuf,
    }

    impl CheckpointStore for Atomic {
        fn commit(&mut self, _: u64, document: &str) -> Result<(), CheckpointStoreError> {
            fs::write(&self.staging, document).map_err(|_| CheckpointStoreError)?;
            fs::rename(&self.staging, &self.active).map_err(|_| CheckpointStoreError)
        }
    }

    /// A store that violates the contract: it truncates one generation in
    /// place and reports success anyway.
    struct Tearing {
        active: PathBuf,
        tear_at: u64,
        last_whole: Option<String>,
    }

    impl CheckpointStore for Tearing {
        fn commit(&mut self, generation: u64, document: &str) -> Result<(), CheckpointStoreError> {
            if generation == self.tear_at {
                fs::write(&self.active, &document[..document.len() / 3])
                    .map_err(|_| CheckpointStoreError)?;
                return Ok(());
            }
            fs::write(&self.active, document).map_err(|_| CheckpointStoreError)?;
            self.last_whole = Some(document.to_owned());
            Ok(())
        }
    }

    let agent = durable_agent(7);
    let proposal = document(&agent);
    let root = scratch("atomic");

    let mut read = Read::new();
    let mut store = Atomic {
        active: root.join("ACTIVE"),
        staging: root.join("staging"),
    };
    let run = agent
        .start(
            &task(),
            &proposal,
            &mut read,
            DurableBudget::default(),
            Retention::ObservationBytes,
            &AgentCancellation::new(),
            &mut store,
            CrashPoint::Never,
        )
        .unwrap();
    assert_eq!(run.status(), DurableStatus::Completed);
    assert!(!root.join("staging").exists(), "staging leaked");
    let recovered =
        AgentCheckpoint::decode(&fs::read_to_string(root.join("ACTIVE")).unwrap()).unwrap();
    assert_eq!(recovered.program_counter(), ProgramCounter::Delivered);

    // A torn third generation is refused by recovery rather than adopted, and
    // the last whole generation the store did write still decodes.
    let mut read = Read::new();
    let mut store = Tearing {
        active: root.join("TORN"),
        tear_at: 3,
        last_whole: None,
    };
    agent
        .start(
            &task(),
            &proposal,
            &mut read,
            DurableBudget::default(),
            Retention::ObservationBytes,
            &AgentCancellation::new(),
            &mut store,
            CrashPoint::AfterSettlement,
        )
        .unwrap();
    let torn = fs::read_to_string(root.join("TORN")).unwrap();
    let error = AgentCheckpoint::decode(&torn).err().unwrap();
    assert_eq!(error[0].code, "SPX-G573");
    let whole = AgentCheckpoint::decode(store.last_whole.as_deref().unwrap()).unwrap();
    assert_eq!(whole.program_counter(), ProgramCounter::Intent);

    // Recovering from that whole generation is the uncertain path, not a
    // silent completion from a half-written settlement.
    let mut sink = Memory::default();
    let resumed = agent
        .resume(
            &whole,
            &task(),
            &proposal,
            &mut read,
            Reconciliation::None,
            &AgentCancellation::new(),
            &mut sink,
        )
        .unwrap();
    assert_eq!(resumed.status(), DurableStatus::Unknown);
    assert_eq!(read.calls, 1);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn the_durable_path_adds_no_ambient_authority_and_no_cli_surface() {
    for source in [
        include_str!("../../src/agent_lifecycle/durable.rs"),
        include_str!("../../src/agent_lifecycle/durable/checkpoint.rs"),
        include_str!("../../src/agent_lifecycle/durable/journal.rs"),
    ] {
        for forbidden in [
            "std::net::",
            "TcpStream",
            "UdpSocket",
            "reqwest::",
            "Command::new",
            "fs::write",
            "fs::read",
            "File::create",
            "std::env",
        ] {
            assert!(
                !source.contains(forbidden),
                "the durable path names `{forbidden}`"
            );
        }
        // The durable path never names the mint or constructs an
        // authorization: it obtains one only from the validated authorizing
        // transition, through the same call the uninterrupted lifecycle uses.
        assert!(!source.contains("Authorized {"));
        assert!(!source.contains("fn mint"));
    }
    // `resume` and `reconcile` remain unadmitted verbs.
    let cli = include_str!("../../src/cli/agent.rs");
    assert!(cli.contains("`resume` and `reconcile` are not admitted"));
    for verb in ["checkpoint", "AgentCheckpoint", "DurableAgent"] {
        assert!(!cli.contains(verb), "the CLI names `{verb}`");
    }
}

#[test]
fn the_frozen_runtime_v1_known_answers_survive_a_durable_run() {
    let agent = durable_agent(7);
    let mut read = Read::new();
    let mut store = Memory::default();
    agent
        .start(
            &task(),
            &document(&agent),
            &mut read,
            DurableBudget::default(),
            Retention::ObservationBytes,
            &AgentCancellation::new(),
            &mut store,
            CrashPoint::AfterEffect,
        )
        .unwrap();
    let current = store.active();
    agent
        .resume(
            &current,
            &task(),
            &document(&agent),
            &mut read,
            Reconciliation::Settled(b"observed"),
            &AgentCancellation::new(),
            &mut store,
        )
        .unwrap();

    let (definition_v2, deployment) = migrated();
    let bound = bind_agent_deployment(&definition_v2, &deployment).unwrap();
    let source = definition(&profile());
    assert_eq!(bound.runtime_v1_definition(), source);
    assert_eq!(
        compile_agent_definition(&source)
            .unwrap()
            .definition()
            .digest(),
        "sha256:82ab9abbeca5e209c36224d9cab3b7b6a7cdffc3b2fce5db73123fa7425965a0"
    );
    assert_eq!(
        bound.graph().digest(),
        "sha256:0dc7ce1d50d43077042577cf6ac3dcfb5d2a744fb3acd2ca6cea12a6e296ff61"
    );
    assert_eq!(
        raw_sha(bound.runtime_v1_profile()),
        "sha256:14981ee99af965dcea311121a90cacfb9891a00d6365e7ad00cab8cefe69c01a"
    );
}
