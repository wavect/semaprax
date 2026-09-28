use super::*;
use semaprax::agent_lifecycle::iterative::source_live::{SourceLivePolicy, SourceProposalPolicy};
use semaprax::agent_lifecycle::{CheckpointStore, CheckpointStoreError};
use semaprax::agent_runtime::AgentCancellation;
use semaprax::agent_runtime_v2::{SourceModelAdapterIdentity, SourceModelBinding};
use semaprax::execution_revision::typed::{
    AgentRuntimeV2DurableModelFailure, AgentRuntimeV2DurableModelWaitEvidence,
};
use semaprax::live_invocation::source_journal::{SourceJournalEntry, SourceTerminalStatus};
use semaprax::live_invocation::{InvocationClock, SourceInvocationClock};
use semaprax::provider_adapter_sdk::fixture_adapters::{usage, ScriptedStreamingAdapter};
use semaprax::provider_adapter_sdk::{
    AdapterInvocationCapability, ProviderAdapter, StreamingSourceProposalAdapter,
};
use semaprax::resumable_effects::source_checkpoint::SourceCheckpointKey;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

const WRAPPER: &str = r#"
@id("fixture.agent.fn.await_proposal")
fn await_proposal(observation: Observation) -> Proposal
    yields Observation -> Proposal
{
    yield observation
}
"#;
struct Clock;
impl InvocationClock for Clock {
    fn now_millis(&self) -> i64 {
        1
    }
}
impl SourceInvocationClock for Clock {
    fn clock_domain(&self) -> &str {
        "model-wait-test.v1"
    }
}

#[derive(Default)]
struct Store {
    documents: Vec<String>,
    fail: Option<(&'static str, Option<&'static str>)>,
    fail_before_persistence: bool,
    attempted: Vec<String>,
    cancel_intent: Option<AgentCancellation>,
}
impl CheckpointStore for Store {
    fn commit(&mut self, _: u64, document: &str) -> Result<(), CheckpointStoreError> {
        let value: serde_json::Value = serde_json::from_str(document).unwrap();
        let entry = value["entries"].as_array().unwrap().last().unwrap();
        self.attempted
            .push(entry["kind"].as_str().unwrap().to_owned());
        let phase = entry.get("phase").or_else(|| {
            entry
                .get("reservation")
                .and_then(|r| r.as_u64())
                .and_then(|r| value["entries"].get(r as usize))
                .and_then(|r| r.get("phase"))
        });
        let fail = self.fail.is_some_and(|(kind, expected)| {
            entry["kind"] == kind
                && expected.is_none_or(|p| phase.is_some_and(|actual| actual == p))
        });
        if fail && self.fail_before_persistence {
            self.fail = None;
            return Err(CheckpointStoreError);
        }
        self.documents.push(document.to_owned());
        if entry["kind"] == "attempt_intent" {
            if let Some(cancel) = &self.cancel_intent {
                cancel.cancel();
            }
        }
        if fail {
            self.fail = None;
            return Err(CheckpointStoreError);
        }
        Ok(())
    }
}
fn fixture() -> super::super::Fixture {
    let fixture = typed_fixture();
    let path = fixture.0.join("src/app.spx");
    let source = std::fs::read_to_string(&path).unwrap();
    std::fs::write(path, format!("{source}{WRAPPER}")).unwrap();
    fixture
}
fn identity() -> SourceModelAdapterIdentity {
    SourceModelAdapterIdentity {
        provider_id: "fake.local".into(),
        model_id: "fake-basic".into(),
        adapter_identity: "scripted-streaming-adapter".into(),
        adapter_version: "1.0.0".into(),
        provider_profile: "fixture".into(),
    }
}
fn policy(binding: &SourceModelBinding) -> SourceLivePolicy {
    SourceLivePolicy {
        deployment_binding: binding.digest().into(),
        response_limit: binding.max_response_bytes(),
        ceiling: 4,
        reservation_units: 1,
        unit: "model_wait_test".into(),
        clock_domain: "model-wait-test.v1".into(),
        initial_millis: 0,
        deadline_millis: 1000,
        max_total_steps: 300_000,
        program_root: None,
    }
}

fn reservations(document: &str) -> Vec<(u64, u64)> {
    let value: serde_json::Value = serde_json::from_str(document).unwrap();
    value["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["kind"] == "wait_evaluation_reserved")
        .map(|e| (e["seq"].as_u64().unwrap(), e["fuel"].as_u64().unwrap()))
        .collect()
}

#[test]
fn historical_start_and_resume_replay_ack_faults_keep_charges_and_one_closure() {
    let fixture = fixture();
    with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        let project = snapshot.retain_revision();
        let root = project.program_root()?;
        let key = SourceCheckpointKey::new([17; 32]);
        for fail_before_persistence in [true, false] {
            for phase in ["start", "resume"] {
                for kind in ["wait_evaluation_reserved", "wait_replay_checked"] {
                    let calls = Rc::new(Cell::new(0));
                    let mut store = Store {
                        fail: Some(("wait_completed", None)),
                        ..Store::default()
                    };
                    assert!(run(
                        project.clone(),
                        &root,
                        &key,
                        1000,
                        None,
                        &mut store,
                        &AgentCancellation::new(),
                        false,
                        true,
                        calls.clone()
                    )
                    .is_err());
                    assert_eq!(calls.get(), 1);
                    let mut retained = store.documents.last().unwrap().clone();
                    for _ in 0..2 {
                        store.fail = Some((kind, Some(phase)));
                        store.fail_before_persistence = fail_before_persistence;
                        let before = reservations(&retained);
                        assert!(run(
                            project.clone(),
                            &root,
                            &key,
                            1000,
                            Some(&retained),
                            &mut store,
                            &AgentCancellation::new(),
                            false,
                            true,
                            calls.clone()
                        )
                        .is_err());
                        assert_eq!(store.attempted.last().unwrap(), kind);
                        assert_eq!(calls.get(), 1, "historical settlement cannot redispatch");
                        retained = store.documents.last().unwrap().clone();
                        assert!(reservations(&retained).starts_with(&before));
                        let rows: serde_json::Value = serde_json::from_str(&retained).unwrap();
                        for closure in ["wait_prepared", "wait_completed"] {
                            assert_eq!(
                                rows["entries"]
                                    .as_array()
                                    .unwrap()
                                    .iter()
                                    .filter(|e| e["kind"] == closure
                                        && e["turn"] == 0
                                        && e["attempt"] == 0)
                                    .count(),
                                1
                            );
                        }
                    }
                    let before = reservations(&retained);
                    let result = run(
                        project.clone(),
                        &root,
                        &key,
                        1000,
                        Some(&retained),
                        &mut store,
                        &AgentCancellation::new(),
                        false,
                        true,
                        calls.clone(),
                    )
                    .ok()
                    .expect("interrupted historical phases resume");
                    assert_eq!(calls.get(), 3);
                    let final_reservations = reservations(store.documents.last().unwrap());
                    assert!(final_reservations.starts_with(&before));
                    let evidence: serde_json::Value =
                        serde_json::from_slice(result.wait_evidence()).unwrap();
                    assert_eq!(
                        evidence["total_wait_fuel"].as_u64().unwrap(),
                        final_reservations.iter().map(|(_, fuel)| fuel).sum::<u64>()
                    );
                }
            }
        }
        Ok(())
    })
    .unwrap();
}

#[allow(clippy::too_many_arguments)]
fn run(
    project: std::sync::Arc<semaprax::project::ProjectRevision>,
    root: &semaprax::project::ProgramRoot,
    key: &SourceCheckpointKey,
    fuel: usize,
    retained: Option<&str>,
    store: &mut Store,
    cancellation: &AgentCancellation,
    malformed: bool,
    checkpointed: bool,
    calls: Rc<Cell<usize>>,
) -> Result<AgentRuntimeV2DurableModelWaitEvidence, Box<AgentRuntimeV2DurableModelFailure>> {
    let source = &project.sources()[0];
    let compiled = compile_source_agent_lifecycle_v2(
        source.source(),
        source.path(),
        "fixture.agent",
        "fixture.agent.type.step",
    )
    .unwrap();
    let (_, deployment) = migrate_agent_definition_v1(
        project.agent_definitions()[0]
            .definition()
            .canonical_source(),
        "fixture.model_wait.runtime",
    )
    .unwrap();
    let runtime = bind_agent_runtime_v2_live(
        project,
        ProgramRootRef::V1(root),
        root.program_root_digest(),
        "src/app.spx",
        "fixture.agent",
        "fixture.agent.type.step",
        "fixture.agent.type.proposal.sequence",
        operations(),
        &deployment,
        LifecycleTask {
            objective: b"wait fuel".to_vec(),
            budget: 12,
        },
        IterativeBudget {
            max_steps_per_stage: 1000,
            ..IterativeBudget::default()
        },
        EffectBudget {
            max_calls: 3,
            max_argument_bytes: 4096,
            max_result_bytes: 4096,
            max_total_bytes: 8192,
        },
    )
    .unwrap();
    let wait = runtime
        .source_model_wait_binding("fixture.agent.fn.await_proposal", fuel)
        .unwrap();
    let binding = runtime.source_model_binding(identity()).unwrap();
    let mut documents: Vec<_> = ["0", "1", "0"]
        .into_iter()
        .map(|sequence| {
            crate::agent_lifecycle_v1::proposal(
                compiled.proposal_schema().schema().digest(),
                "5",
                false,
                sequence,
            )
        })
        .collect();
    if malformed {
        documents.insert(0, "not a canonical proposal\n".into());
    }
    let settled = retained
        .map(|document| {
            let value: serde_json::Value = serde_json::from_str(document).unwrap();
            value["entries"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|entry| entry["kind"] == "attempt_settled")
                .count()
        })
        .unwrap_or(0);
    let documents = RefCell::new(VecDeque::from(
        documents.into_iter().skip(settled).collect::<Vec<_>>(),
    ));
    let mut factory = move || {
        calls.set(calls.get() + 1);
        let document = documents
            .borrow_mut()
            .pop_front()
            .expect("only remaining model attempts dispatch");
        Box::new(ScriptedStreamingAdapter::new(
            vec![document.as_bytes().to_vec()],
            document.into_bytes(),
            usage(1, 1, 0),
            true,
        )) as Box<dyn ProviderAdapter>
    };
    let policy = policy(&binding);
    let cap = AdapterInvocationCapability::grant("explicit model wait fixture");
    let mut adapter = if checkpointed {
        StreamingSourceProposalAdapter::new_bound_checkpointed(
            &mut factory,
            cap,
            compiled.proposal_schema(),
            binding.clone(),
            binding.invocation_capability(),
            SourceProposalPolicy {
                deployment_binding: binding.digest(),
                response_limit: binding.max_response_bytes(),
                reservation_units: 1,
            },
        )
        .unwrap()
    } else {
        StreamingSourceProposalAdapter::new_bound(
            &mut factory,
            cap,
            compiled.proposal_schema(),
            binding.clone(),
            binding.invocation_capability(),
        )
        .unwrap()
    };
    let mut handler = Handler {
        calls: vec![],
        wrong: false,
    };
    runtime
        .run_live_bound_model_durable_with_wait(
            &wait,
            key,
            &mut adapter,
            &mut handler,
            policy,
            &Clock,
            cancellation,
            retained,
            store,
        )
        .map_err(Box::new)
}

#[test]
fn real_model_wait_completes_with_exact_terminal_fuel() {
    let fixture = fixture();
    with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        let project = snapshot.retain_revision();
        let root = project.program_root()?;
        let key = SourceCheckpointKey::new([17; 32]);
        let malformed = false;
        let calls = Rc::new(Cell::new(0));
        let mut store = Store::default();
        let result = run(
            project.clone(),
            &root,
            &key,
            1000,
            None,
            &mut store,
            &AgentCancellation::new(),
            malformed,
            true,
            calls.clone(),
        )
        .unwrap_or_else(|failure| {
            let details = failure.failure();
            panic!(
                "real wait completes (malformed={malformed}, provider_calls={}): diagnostics={:?}, selected={:?}, journal_error={:?}, last_attempted={:?}",
                calls.get(),
                details.diagnostics,
                details.selected,
                details.journal_error,
                store.attempted.last(),
            )
        });
        let checkpoint = &result.model().run().checkpoint;
        assert_eq!(
            checkpoint.terminal_snapshot().unwrap().status(),
            SourceTerminalStatus::Complete
        );
        assert_eq!(calls.get(), 3 + usize::from(malformed));
        assert_eq!(checkpoint.committed_reserved_units(), calls.get() as i64);
        assert_eq!(
            checkpoint
                .entries()
                .iter()
                .filter(|e| matches!(e, SourceJournalEntry::ProposalRefused { .. }))
                .count(),
            usize::from(malformed)
        );
        let evidence: serde_json::Value =
            serde_json::from_slice(result.wait_evidence()).unwrap();
        let wait_fuel = evidence["total_wait_fuel"].as_u64().unwrap();
        assert_eq!(wait_fuel, (6 + u64::from(malformed)) * 1000);
        let ordinary: u64 = checkpoint
            .entries()
            .iter()
            .filter_map(|e| match e {
                SourceJournalEntry::StageReservation { fuel, .. }
                | SourceJournalEntry::ReplayStageReservation { fuel, .. } => Some(*fuel as u64),
                _ => None,
            })
            .sum();
        assert_eq!(checkpoint.committed_stage_fuel(), ordinary + wait_fuel);
        assert_eq!(
            result
                .model()
                .run()
                .checked_run
                .as_ref()
                .unwrap()
                .stages()
                .len(),
            10
        );
        assert_eq!(evidence["invocation"], checkpoint.invocation());
        assert_eq!(
            evidence["ordinary_model_evidence_digest"],
            result.model().evidence_root().digest()
        );
        let terminal = match checkpoint.entries().last().unwrap() {
            SourceJournalEntry::TerminalSnapshot {
                evidence_digest, ..
            } => evidence_digest,
            _ => unreachable!(),
        };
        assert_eq!(evidence["terminal_evidence_digest"], terminal.as_str());
        use sha2::{Digest, Sha256};
        let mut hash = Sha256::new();
        hash.update(b"semaprax.source-model-wait.evidence.v1\0");
        hash.update(result.wait_evidence());
        assert_eq!(
            result.evidence_root().digest(),
            format!(
                "sha256:{}",
                hash.finalize()
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
            )
        );
        assert_eq!(
            result.evidence_root().canonical_json().as_bytes(),
            result.wait_evidence()
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn raw_malformed_model_response_keeps_sdk_failure_class_without_resume_or_effect() {
    let fixture = fixture();
    with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        let project = snapshot.retain_revision();
        let root = project.program_root()?;
        let key = SourceCheckpointKey::new([17; 32]);
        let calls = Rc::new(Cell::new(0));
        let mut store = Store::default();
        let failure = run(
            project,
            &root,
            &key,
            1000,
            None,
            &mut store,
            &AgentCancellation::new(),
            true,
            true,
            calls.clone(),
        )
        .err()
        .expect("SDK malformed response must refuse");
        let details = failure.failure();
        assert_eq!(calls.get(), 1);
        assert_eq!(details.selected, Some(SourceTerminalStatus::ModelFailed));
        assert!(details
            .diagnostics
            .iter()
            .any(|d| d.code == "source.adapter_decode"));
        assert_eq!(details.journal_error, None);
        assert_eq!(details.model_dispatches, 1);
        assert_eq!(details.effect_dispatches, 0);
        let checkpoint = details
            .checkpoint
            .as_ref()
            .expect("failure retains journal");
        assert_eq!(
            checkpoint.terminal_snapshot().unwrap().status(),
            SourceTerminalStatus::ModelFailed
        );
        assert_eq!(checkpoint.committed_reserved_units(), 1);
        let document: serde_json::Value =
            serde_json::from_str(store.documents.last().unwrap()).unwrap();
        let rows = document["entries"].as_array().unwrap();
        assert_eq!(
            rows.iter().filter(|e| e["kind"] == "wait_prepared").count(),
            1
        );
        assert_eq!(
            rows.iter()
                .filter(|e| e["kind"] == "wait_evaluation_reserved")
                .count(),
            1
        );
        assert!(!rows.iter().any(|e| e["kind"] == "wait_completed"
            || e["kind"] == "proposal_refused"
            || e["phase"] == "resume"));
        assert_eq!(
            reservations(store.documents.last().unwrap())
                .iter()
                .map(|(_, fuel)| fuel)
                .sum::<u64>(),
            1000
        );
        assert_eq!(store.attempted.last().unwrap(), "terminal_snapshot");
        Ok(())
    })
    .unwrap();
}

#[test]
fn wait_ack_windows_recover_without_model_redispatch() {
    let fixture = fixture();
    with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        let project = snapshot.retain_revision();
        let root = project.program_root()?;
        let key = SourceCheckpointKey::new([17; 32]);
        for fail_before_persistence in [true, false] {
            for fail in [
                ("wait_evaluation_reserved", Some("start")),
                ("wait_prepared", None),
                ("wait_evaluation_reserved", Some("resume")),
                ("wait_completed", None),
                ("attempt_intent", None),
            ] {
                let calls = Rc::new(Cell::new(0));
                let mut store = Store {
                    fail: Some(fail),
                    fail_before_persistence,
                    ..Store::default()
                };
                let failed = run(
                    project.clone(),
                    &root,
                    &key,
                    1000,
                    None,
                    &mut store,
                    &AgentCancellation::new(),
                    false,
                    true,
                    calls.clone(),
                );
                assert!(failed.is_err());
                assert_eq!(
                    store.attempted.last().unwrap(),
                    fail.0,
                    "failed ACK must stop before any evaluation closure or later dispatch row"
                );
                if fail == ("wait_evaluation_reserved", Some("start")) {
                    assert_eq!(calls.get(), 0);
                } else if fail == ("wait_evaluation_reserved", Some("resume")) {
                    assert_eq!(calls.get(), 1);
                }
                let retained = store.documents.last().unwrap().clone();
                let charged = reservations(&retained);
                let before = calls.get();
                let resumed = run(
                    project.clone(),
                    &root,
                    &key,
                    1000,
                    Some(&retained),
                    &mut store,
                    &AgentCancellation::new(),
                    false,
                    true,
                    calls.clone(),
                );
                if fail.0 == "attempt_intent" && !fail_before_persistence {
                    assert!(resumed.is_err());
                    assert_eq!(calls.get(), before);
                } else {
                    let result = resumed.ok().expect("settled or pure wait prefix resumes");
                    assert_eq!(
                        result
                            .model()
                            .run()
                            .checkpoint
                            .terminal_snapshot()
                            .unwrap()
                            .status(),
                        SourceTerminalStatus::Complete
                    );
                    assert_eq!(calls.get(), 3);
                    assert!(reservations(store.documents.last().unwrap()).starts_with(&charged));
                }
            }
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn wrong_key_binding_and_noncheckpoint_adapter_refuse_before_writes() {
    let fixture = fixture();
    with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        let project = snapshot.retain_revision();
        let root = project.program_root()?;
        let key = SourceCheckpointKey::new([17; 32]);
        let calls = Rc::new(Cell::new(0));
        let mut store = Store::default();
        assert!(run(
            project.clone(),
            &root,
            &key,
            1000,
            None,
            &mut store,
            &AgentCancellation::new(),
            false,
            false,
            calls.clone()
        )
        .is_err());
        assert!(store.documents.is_empty());
        store.fail = Some(("wait_prepared", None));
        assert!(run(
            project.clone(),
            &root,
            &key,
            1000,
            None,
            &mut store,
            &AgentCancellation::new(),
            false,
            true,
            calls.clone()
        )
        .is_err());
        let retained = store.documents.last().unwrap().clone();
        for (key, fuel) in [
            (SourceCheckpointKey::new([18; 32]), 1000),
            (SourceCheckpointKey::new([17; 32]), 999),
        ] {
            let before = store.documents.clone();
            assert!(run(
                project.clone(),
                &root,
                &key,
                fuel,
                Some(&retained),
                &mut store,
                &AgentCancellation::new(),
                false,
                true,
                calls.clone()
            )
            .is_err());
            assert_eq!(store.documents, before);
        }
        let mut tampered: serde_json::Value = serde_json::from_str(&retained).unwrap();
        let prepared = tampered["entries"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|entry| entry["kind"] == "wait_prepared")
            .unwrap();
        prepared["observation_digest"] =
            serde_json::Value::String(format!("sha256:{}", "0".repeat(64)));
        let tampered = format!("{}\n", tampered);
        let before = store.documents.clone();
        assert!(run(
            project.clone(),
            &root,
            &key,
            1000,
            Some(&tampered),
            &mut store,
            &AgentCancellation::new(),
            false,
            true,
            calls.clone()
        )
        .is_err());
        assert_eq!(store.documents, before);
        assert_eq!(calls.get(), 0);
        Ok(())
    })
    .unwrap();
}

#[test]
fn repeated_start_replay_crashes_are_charged_with_one_causal_closure() {
    let fixture = fixture();
    with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        let project = snapshot.retain_revision();
        let root = project.program_root()?;
        let key = SourceCheckpointKey::new([17; 32]);
        let calls = Rc::new(Cell::new(0));
        let mut store = Store::default();
        let mut retained = None;
        for _ in 0..2 {
            store.fail = Some(("wait_evaluation_reserved", Some("start")));
            assert!(run(
                project.clone(),
                &root,
                &key,
                1000,
                retained.as_deref(),
                &mut store,
                &AgentCancellation::new(),
                false,
                true,
                calls.clone()
            )
            .is_err());
            retained = Some(store.documents.last().unwrap().clone());
        }
        let result = run(
            project,
            &root,
            &key,
            1000,
            retained.as_deref(),
            &mut store,
            &AgentCancellation::new(),
            false,
            true,
            calls.clone(),
        )
        .ok()
        .expect("second charged reconstruction closes original phase");
        assert_eq!(calls.get(), 3);
        let evidence: serde_json::Value = serde_json::from_slice(result.wait_evidence()).unwrap();
        assert_eq!(evidence["total_wait_fuel"], 8000);
        let journal: serde_json::Value =
            serde_json::from_str(store.documents.last().unwrap()).unwrap();
        assert_eq!(
            journal["entries"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|e| e["kind"] == "wait_prepared" && e["turn"] == 0 && e["attempt"] == 0)
                .count(),
            1
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn pure_wait_budget_and_post_intent_cancellation_keep_outer_failure_classes() {
    let fixture = fixture();
    with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        let project = snapshot.retain_revision();
        let root = project.program_root()?;
        let key = SourceCheckpointKey::new([17; 32]);
        let calls = Rc::new(Cell::new(0));
        let mut store = Store::default();
        let result = run(
            project.clone(),
            &root,
            &key,
            1,
            None,
            &mut store,
            &AgentCancellation::new(),
            false,
            true,
            calls.clone(),
        );
        let failure = result
            .err()
            .expect("pure wrapper cannot evaluate with one step");
        assert_eq!(
            failure.failure().selected,
            Some(SourceTerminalStatus::BudgetExhausted)
        );
        assert_eq!(calls.get(), 0);
        assert!(!store
            .documents
            .last()
            .unwrap()
            .contains("\"kind\":\"attempt_failed\""));
        let cancellation = AgentCancellation::new();
        let mut store = Store {
            cancel_intent: Some(cancellation.clone()),
            ..Store::default()
        };
        let result = run(
            project.clone(),
            &root,
            &key,
            1000,
            None,
            &mut store,
            &cancellation,
            false,
            true,
            calls.clone(),
        );
        let failure = result.err().expect("post-intent cancellation is closed");
        assert_eq!(
            failure.failure().selected,
            Some(SourceTerminalStatus::Cancelled)
        );
        assert_eq!(calls.get(), 0);
        assert_eq!(
            failure
                .failure()
                .checkpoint
                .as_ref()
                .unwrap()
                .committed_reserved_units(),
            1
        );
        Ok(())
    })
    .unwrap();
}
