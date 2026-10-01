use super::*;
use crate::agent_lifecycle::CheckpointStoreError;
#[derive(Default)]
struct Store {
    document: String,
    fail_at: Option<u64>,
    commits: usize,
}
impl CheckpointStore for Store {
    fn commit(&mut self, generation: u64, document: &str) -> Result<(), CheckpointStoreError> {
        self.commits += 1;
        self.document = document.to_owned();
        if self.fail_at == Some(generation) {
            self.fail_at = None;
            Err(CheckpointStoreError)
        } else {
            Ok(())
        }
    }
}
#[derive(Default)]
struct Handler {
    calls: usize,
    wrong: bool,
}
impl TypedEffectHandler for Handler {
    fn execute(&mut self, _: &TypedEffectRequest<'_>) -> Option<Vec<(String, RetainedValue)>> {
        self.calls += 1;
        Some(vec![(
            "value".into(),
            if self.wrong {
                RetainedValue::Bool(true)
            } else {
                RetainedValue::I64(8)
            },
        )])
    }
}
fn task() -> LifecycleTask {
    LifecycleTask {
        objective: b"durable".to_vec(),
        budget: 10,
    }
}
fn proposals(compiled: &CompiledTypedEffects) -> Vec<String> {
    vec![crate::agent_lifecycle::tests::proposal(&compiled.lifecycle.inner, "1", "0"); 3]
}
fn budget() -> EffectBudget {
    EffectBudget {
        max_calls: 4,
        max_argument_bytes: 4096,
        max_result_bytes: 4096,
        max_total_bytes: 8192,
    }
}
fn root() -> String {
    digest(b"test-root\0", b"exact")
}
fn backend(
    selector: u8,
    module_source: &str,
) -> crate::agent_lifecycle::authorization::StageBackend<'_> {
    let native_host = crate::agent_lifecycle::tests::native_stage_host()
        .expect("durable native parity requires an explicit held compiler");
    match selector {
        0 => crate::agent_lifecycle::tests::native_backend(Box::leak(Box::new(native_host))),
        1 => crate::agent_lifecycle::tests::native_o2_backend(Box::leak(Box::new(native_host))),
        2 => crate::agent_lifecycle::authorization::StageBackend::Wasm {
            source: module_source,
        },
        _ => unreachable!("durable backend fixture selector"),
    }
}
fn backend_label(selector: u8) -> &'static str {
    match selector {
        0 => "native -O0",
        1 => "native -O2",
        2 => "Core Wasm",
        _ => unreachable!("durable backend fixture selector"),
    }
}

fn held_wasm_target() -> super::super::WasmTargetHost {
    std::env::var_os("SEMAPRAX_TEST_WASM_STAGE_NODE")
        .map(std::path::PathBuf::from)
        .into_iter()
        .chain([
            std::path::PathBuf::from("/usr/bin/node"),
            std::path::PathBuf::from("/usr/local/bin/node"),
            std::path::PathBuf::from("/opt/homebrew/bin/node"),
        ])
        .find_map(|path| super::super::WasmTargetHost::open(path).ok())
        .expect("durable target test requires an explicit held Node runtime")
}

/// The three claimed target routes, for cross-backend checkpoint parity.
/// Native `-O2` is intentionally excluded here: it is the same executor and
/// registry as `-O0`, already exercised per-backend above, and adding it
/// would only multiply this 3x3 matrix without a new claim.
fn matrix_backends<'a>(
    module_source: &'a str,
    native_host: &'a crate::agent_lifecycle::authorization::NativeStageHost,
) -> [(
    &'static str,
    crate::agent_lifecycle::authorization::StageBackend<'a>,
); 3] {
    [
        (
            "interpreter",
            crate::agent_lifecycle::authorization::StageBackend::Interpreter,
        ),
        (
            "native -O0",
            crate::agent_lifecycle::tests::native_backend(native_host),
        ),
        (
            "Core Wasm",
            crate::agent_lifecycle::authorization::StageBackend::Wasm {
                source: module_source,
            },
        ),
    ]
}
fn run(
    compiled: &CompiledTypedEffects,
    handler: &mut Handler,
    store: &mut Store,
    retained: Option<&str>,
) -> Result<DurableTypedRun, DurableTypedFailure> {
    compiled.run_durable(
        &task(),
        &proposals(compiled),
        handler,
        IterativeBudget::default(),
        budget(),
        &AgentCancellation::new(),
        &root(),
        &root(),
        retained,
        store,
        10_000_000,
    )
}

fn run_on(
    compiled: &CompiledTypedEffects,
    handler: &mut Handler,
    store: &mut Store,
    retained: Option<&str>,
    backend: crate::agent_lifecycle::authorization::StageBackend<'_>,
) -> Result<DurableTypedRun, DurableTypedFailure> {
    run_on_with_fuel(compiled, handler, store, retained, 10_000_000, backend)
}

fn run_on_with_fuel(
    compiled: &CompiledTypedEffects,
    handler: &mut Handler,
    store: &mut Store,
    retained: Option<&str>,
    max_reserved_fuel: u64,
    backend: crate::agent_lifecycle::authorization::StageBackend<'_>,
) -> Result<DurableTypedRun, DurableTypedFailure> {
    compiled.run_durable_on(
        &task(),
        &proposals(compiled),
        handler,
        IterativeBudget::default(),
        budget(),
        &AgentCancellation::new(),
        &root(),
        &root(),
        retained,
        store,
        max_reserved_fuel,
        backend,
    )
}

fn run_selected(
    compiled: &CompiledTypedEffects,
    handler: &mut Handler,
    store: &mut Store,
    retained: Option<&str>,
    selected: super::super::TargetStageBackend<'_>,
) -> Result<DurableTypedRun, DurableTypedFailure> {
    compiled.run_durable_with_backend(
        &task(),
        &proposals(compiled),
        handler,
        IterativeBudget::default(),
        budget(),
        &AgentCancellation::new(),
        &root(),
        &root(),
        retained,
        store,
        10_000_000,
        selected,
    )
}

fn run_metered(
    compiled: &CompiledTypedEffects,
    handler: &mut Handler,
    store: &mut Store,
    retained: Option<&str>,
) -> Result<super::super::MeteredDurableTypedRun, DurableTypedFailure> {
    run_metered_selected(
        compiled,
        handler,
        store,
        retained,
        super::super::TargetStageBackend::Interpreter,
    )
}

fn run_metered_selected(
    compiled: &CompiledTypedEffects,
    handler: &mut Handler,
    store: &mut Store,
    retained: Option<&str>,
    selected: super::super::TargetStageBackend<'_>,
) -> Result<super::super::MeteredDurableTypedRun, DurableTypedFailure> {
    compiled.run_durable_metered_with_backend(
        &task(),
        &proposals(compiled),
        handler,
        IterativeBudget::default(),
        budget(),
        &AgentCancellation::new(),
        &root(),
        &root(),
        retained,
        store,
        10_000_000,
        selected,
        100,
    )
}

fn migration_seed() -> crate::execution_revision::typed::migration::MigrationSeed {
    use crate::execution_revision::root as execution_root;
    use crate::hir::DeclarationId;
    use crate::interpreter::retained_call::{RetainedField, RetainedRecord};

    crate::execution_revision::typed::migration::MigrationSeed::for_test(
        RetainedValue::Record(RetainedRecord {
            record: DeclarationId::new("fixture.agent.type.state"),
            fields: vec![
                RetainedField {
                    field: DeclarationId::new("fixture.agent.type.state.objective"),
                    value: RetainedValue::Bytes(vec![9, 8, 7]),
                },
                RetainedField {
                    field: DeclarationId::new("fixture.agent.type.state.budget"),
                    value: RetainedValue::I64(10),
                },
                RetainedField {
                    field: DeclarationId::new("fixture.agent.type.state.epoch"),
                    value: RetainedValue::I64(2),
                },
            ],
        }),
        execution_root(
            "test.migration-seed.v1",
            serde_json::json!({"fixture": "durable-metered-seed"}),
        ),
        CheckpointUsage::default(),
        0,
        0,
        10_000_000,
    )
}

fn run_migration_seed_metered(
    compiled: &CompiledTypedEffects,
    handler: &mut Handler,
    store: &mut Store,
    retained: Option<&str>,
) -> Result<super::super::MeteredDurableTypedRun, DurableTypedFailure> {
    run_migration_seed_metered_selected(
        compiled,
        handler,
        store,
        retained,
        super::super::TargetStageBackend::Interpreter,
    )
}

fn run_migration_seed_metered_selected(
    compiled: &CompiledTypedEffects,
    handler: &mut Handler,
    store: &mut Store,
    retained: Option<&str>,
    selected: super::super::TargetStageBackend<'_>,
) -> Result<super::super::MeteredDurableTypedRun, DurableTypedFailure> {
    let seed = migration_seed();
    compiled.run_durable_from_seed_metered_with_backend(
        &task(),
        &proposals(compiled),
        handler,
        IterativeBudget::default(),
        budget(),
        &AgentCancellation::new(),
        &root(),
        &root(),
        retained,
        store,
        10_000_000,
        &seed,
        selected,
        100,
    )
}

mod target_cleanup_parity;

#[test]
fn fresh_durable_semantic_metering_records_every_committed_stage() {
    let compiled = super::super::tests::compile();
    let mut handler = Handler::default();
    let mut store = Store::default();
    let metered = run_metered(&compiled, &mut handler, &mut store, None).unwrap();
    assert!(metered.observations_complete());
    assert_eq!(
        metered.observations().len(),
        metered.run().run().lifecycle().stages().len()
    );
    let checkpoint: serde_json::Value = serde_json::from_str(metered.run().checkpoint()).unwrap();
    assert_eq!(checkpoint["schema"], "semaprax.agent-operation-checkpoint-metered.v2");
    let evidence: serde_json::Value = serde_json::from_str(metered.evidence()).unwrap();
    assert_eq!(evidence["schema"], "semaprax.agent-durable-semantic-work.v3");
    assert_eq!(
        evidence["target_execution_binding"],
        metered.target_execution_binding()
    );
    assert_eq!(
        checkpoint["binding"]["target_execution_binding"],
        metered.target_execution_binding()
    );
    let entries = checkpoint["entries"].as_array().unwrap();
    assert_eq!(
        entries
            .iter()
            .filter(|entry| entry["event"]["kind"] == "stage_reservation")
            .count(),
        entries
            .iter()
            .filter(|entry| entry["event"]["kind"] == "semantic_work")
            .count(),
        "every published metered stage reservation has an authenticated receipt",
    );
    assert!(!metered.evidence().is_empty());
    assert!(metered.evidence_digest().starts_with("sha256:"));
}

#[test]
fn durable_semantic_metering_refuses_target_substitution_before_store_or_handler_work() {
    let compiled = super::super::tests::compile();
    let mut handler = Handler::default();
    let mut store = Store::default();
    let fresh = run_metered(&compiled, &mut handler, &mut store, None).unwrap();
    let retained = fresh.run().checkpoint().to_owned();
    let before = (handler.calls, store.commits);
    let failure = run_metered_selected(
        &compiled,
        &mut handler,
        &mut store,
        Some(&retained),
        super::super::TargetStageBackend::CoreWasm,
    )
    .err()
    .expect("metered durable recovery must bind its selected target");
    assert!(failure
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.message.contains("semantic_work.profile")));
    assert_eq!((handler.calls, store.commits), before);
}

#[test]
fn durable_semantic_metering_recovers_authenticated_receipts_without_redelivery() {
    let compiled = super::super::tests::compile();
    let mut handler = Handler::default();
    let mut store = Store::default();
    let fresh = run_metered(&compiled, &mut handler, &mut store, None).unwrap();
    let retained = fresh.run().checkpoint().to_owned();
    let before = (handler.calls, store.commits);
    let recovered = run_metered(&compiled, &mut handler, &mut store, Some(&retained))
        .expect("authenticated metered checkpoint recovers");
    assert!(recovered.observations_complete());
    assert_eq!(
        recovered.target_execution_binding(),
        fresh.target_execution_binding(),
        "recovery evidence retains the selected target bound by its checkpoint"
    );
    assert_eq!(
        recovered.run().run().lifecycle().status(),
        IterativeStatus::Complete
    );
    assert_eq!(
        handler.calls, before.0,
        "recovery redelivered retained host work"
    );
    assert!(
        store.commits >= before.1,
        "recovery lost its authenticated journal"
    );
}

#[test]
fn migration_seeded_durable_semantic_metering_replays_receipts_without_redelivery() {
    let compiled = super::super::tests::compile();
    let mut handler = Handler::default();
    let mut store = Store::default();
    let fresh = run_migration_seed_metered(&compiled, &mut handler, &mut store, None).unwrap();
    assert!(fresh.observations_complete());
    let fresh_evidence: serde_json::Value = serde_json::from_str(fresh.evidence()).unwrap();
    assert_eq!(
        fresh_evidence["target_execution_binding"],
        fresh.target_execution_binding()
    );
    let checkpoint: serde_json::Value = serde_json::from_str(fresh.run().checkpoint()).unwrap();
    assert_eq!(
        checkpoint["binding"]["target_execution_binding"],
        fresh.target_execution_binding()
    );
    let entries = checkpoint["entries"].as_array().unwrap();
    assert_eq!(
        entries
            .iter()
            .filter(|entry| entry["event"]["kind"] == "stage_reservation")
            .count(),
        entries
            .iter()
            .filter(|entry| entry["event"]["kind"] == "semantic_work")
            .count(),
        "every migration-seeded stage reservation has an authenticated receipt",
    );
    let retained = fresh.run().checkpoint().to_owned();
    let before = handler.calls;
    let recovered =
        run_migration_seed_metered(&compiled, &mut handler, &mut store, Some(&retained))
            .expect("migration-seeded metered checkpoint recovers");
    assert!(recovered.observations_complete());
    assert_eq!(
        recovered.target_execution_binding(),
        fresh.target_execution_binding(),
        "migration-seeded recovery retains the selected target binding"
    );
    assert_eq!(
        recovered.run().run().lifecycle().status(),
        IterativeStatus::Complete
    );
    assert_eq!(
        handler.calls, before,
        "recovery redelivered retained host work"
    );
}

#[test]
fn durable_semantic_metering_refuses_ordinary_checkpoint_before_store_or_handler_work() {
    let compiled = super::super::tests::compile();
    let mut handler = Handler::default();
    let mut store = Store::default();
    let ordinary = run(&compiled, &mut handler, &mut store, None).unwrap();
    let retained = ordinary.checkpoint().to_owned();
    let before = (handler.calls, store.commits);
    let failure = run_metered(&compiled, &mut handler, &mut store, Some(&retained))
        .err()
        .expect("ordinary v2 checkpoint lacks retained semantic receipts");
    assert!(failure
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.message.contains("semantic_work.profile")));
    assert_eq!((handler.calls, store.commits), before);
}

#[test]
fn ordinary_durable_route_refuses_metered_checkpoint_before_store_or_handler_work() {
    let compiled = super::super::tests::compile();
    let mut handler = Handler::default();
    let mut store = Store::default();
    let metered = run_metered(&compiled, &mut handler, &mut store, None).unwrap();
    let retained = metered.run().checkpoint().to_owned();
    let before = (handler.calls, store.commits);
    let failure = run(&compiled, &mut handler, &mut store, Some(&retained))
        .err()
        .expect("ordinary route must not reinterpret metered receipts");
    assert!(failure
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.message.contains("semantic_work.profile")));
    assert_eq!((handler.calls, store.commits), before);
}

#[test]
fn durable_semantic_metering_refuses_tampered_receipt_before_store_or_handler_work() {
    let compiled = super::super::tests::compile();
    let mut handler = Handler::default();
    let mut store = Store::default();
    let metered = run_metered(&compiled, &mut handler, &mut store, None).unwrap();
    let retained = metered.run().checkpoint().replacen(
        "\"kind\":\"semantic_work\"",
        "\"kind\":\"semantic_work_tampered\"",
        1,
    );
    let before = (handler.calls, store.commits);
    assert!(run_metered(&compiled, &mut handler, &mut store, Some(&retained)).is_err());
    assert_eq!((handler.calls, store.commits), before);
}

#[test]
fn public_durable_selector_replays_a_partial_checkpoint_across_targets() {
    if !crate::agent_lifecycle::tests::stage_process_host_supported() {
        return;
    }
    let native_fixture = crate::agent_lifecycle::tests::native_stage_host()
        .expect("public durable selector needs a held native compiler");
    let native = super::super::NativeTargetHost::open(native_fixture.compiler_path())
        .expect("public native target reopens the held compiler");
    let wasm = held_wasm_target();
    let source = super::super::tests::typed_effect_source();
    let compiled = super::super::tests::compile_from_source(&source);
    let targets = [
        super::super::TargetStageBackend::Interpreter,
        super::super::TargetStageBackend::Native(&native),
        super::super::TargetStageBackend::CoreWasmHeld(&wasm),
    ];
    let mut handler = Handler::default();
    let mut store = Store {
        fail_at: Some(11),
        ..Default::default()
    };
    assert!(run_selected(&compiled, &mut handler, &mut store, None, targets[1]).is_err());
    let retained = store.document.clone();
    assert!(!retained.is_empty());
    for selected in targets {
        let mut handler = Handler::default();
        let mut store = Store {
            document: retained.clone(),
            ..Default::default()
        };
        let restored = run_selected(
            &compiled,
            &mut handler,
            &mut store,
            Some(&retained),
            selected,
        )
        .expect("public selector restores the exact partial checkpoint");
        assert_eq!(
            restored.run().lifecycle().status(),
            IterativeStatus::Complete
        );
        assert_eq!(handler.calls, 1, "only outstanding dispatch may run");
    }
}

#[test]
fn held_wasm_durable_selector_refuses_missing_retained_source_before_store_or_host() {
    if !crate::agent_lifecycle::tests::stage_process_host_supported() {
        return;
    }
    let source = super::super::tests::typed_effect_source();
    let mut compiled = super::super::tests::compile_from_source(&source);
    compiled.target_source = None;
    let wasm = held_wasm_target();
    let mut handler = Handler::default();
    let mut store = Store::default();
    let failure = run_selected(
        &compiled,
        &mut handler,
        &mut store,
        None,
        super::super::TargetStageBackend::CoreWasmHeld(&wasm),
    )
    .err()
    .expect("missing retained source must refuse before checkpoint work");
    assert!(failure.diagnostics()[0]
        .message
        .contains("backend.wasm_source"));
    assert_eq!((handler.calls, store.commits), (0, 0));
}

#[test]
fn native_o0_o2_durable_recovery_replays_the_checked_grant_without_a_second_handler_call() {
    if !crate::agent_lifecycle::tests::stage_process_host_supported() {
        return;
    }
    let module_source = super::super::tests::typed_effect_source();
    let compiled = super::super::tests::compile_from_source(&module_source);
    for selector in [0, 1] {
        let label = backend_label(selector);
        let mut handler = Handler::default();
        let mut store = Store::default();
        let first = run_on(
            &compiled,
            &mut handler,
            &mut store,
            None,
            backend(selector, &module_source),
        )
        .unwrap_or_else(|error| panic!("{label}: first durable run: {error:?}"));
        assert_eq!(first.run().lifecycle().status(), IterativeStatus::Complete);
        assert_eq!(handler.calls, 3, "{label}: first handler deliveries");
        let retained = store.document.clone();
        let replay = run_on(
            &compiled,
            &mut handler,
            &mut store,
            Some(&retained),
            backend(selector, &module_source),
        )
        .unwrap_or_else(|error| panic!("{label}: retained replay: {error:?}"));
        assert_eq!(replay.run().lifecycle().status(), IterativeStatus::Complete);
        assert_eq!(
            (handler.calls, replay.run().dispatched()),
            (3, 0),
            "{label}: completed work redelivered"
        );
    }
    // Cross-backend restore is covered by
    // `checkpoint_bytes_are_target_neutral_across_the_full_backend_matrix`,
    // which additionally proves it for a genuinely partial (not yet
    // complete) checkpoint.
}

#[test]
fn core_wasm_durable_recovery_replays_without_host_delivery() {
    if !crate::agent_lifecycle::tests::stage_process_host_supported() {
        return;
    }
    let module_source = super::super::tests::typed_effect_source();
    let compiled = super::super::tests::compile_from_source(&module_source);
    let mut handler = Handler::default();
    let mut store = Store::default();
    let first = run_on(
        &compiled,
        &mut handler,
        &mut store,
        None,
        crate::agent_lifecycle::authorization::StageBackend::Wasm {
            source: &module_source,
        },
    )
    .unwrap();
    assert_eq!(first.run().lifecycle().status(), IterativeStatus::Complete);
    assert_eq!(handler.calls, 3);
    let retained = store.document.clone();
    let replay = run_on(
        &compiled,
        &mut handler,
        &mut store,
        Some(&retained),
        crate::agent_lifecycle::authorization::StageBackend::Wasm {
            source: &module_source,
        },
    )
    .unwrap();
    assert_eq!(replay.run().lifecycle().status(), IterativeStatus::Complete);
    assert_eq!((handler.calls, replay.run().dispatched()), (3, 0));

    let altered_source = format!("{module_source}\n");
    let before = (handler.calls, store.commits);
    assert!(run_on(
        &compiled,
        &mut handler,
        &mut store,
        Some(&retained),
        crate::agent_lifecycle::authorization::StageBackend::Wasm {
            source: &altered_source,
        },
    )
    .is_err());
    assert_eq!(
        (handler.calls, store.commits),
        before,
        "altered Wasm source reached delivery or persistence"
    );
}
#[test]
fn durable_three_turn_run_and_completed_replay_do_not_repeat_host_work() {
    let compiled = super::super::tests::compile();
    let mut handler = Handler::default();
    let mut store = Store::default();
    let first = run(&compiled, &mut handler, &mut store, None).unwrap();
    assert_eq!(first.run().lifecycle().status(), IterativeStatus::Complete);
    assert_eq!(
        (handler.calls, first.usage().calls, store.commits),
        (3, 3, 19)
    );
    let retained = store.document.clone();
    let replay = run(&compiled, &mut handler, &mut store, Some(&retained)).unwrap();
    assert_eq!(replay.run().lifecycle().status(), IterativeStatus::Complete);
    assert_eq!(
        (
            handler.calls,
            replay.run().dispatched(),
            replay.usage().calls
        ),
        (3, 0, 3)
    );
    assert!(replay.usage().reserved_fuel > first.usage().reserved_fuel);
}
#[test]
fn lost_ack_intent_is_uncertain_but_observed_and_transitions_replay_once() {
    let compiled = super::super::tests::compile();
    for generation in [4, 5, 7, 11, 17, 19] {
        let mut handler = Handler::default();
        let mut store = Store {
            fail_at: Some(generation),
            ..Default::default()
        };
        let failure = match run(&compiled, &mut handler, &mut store, None) {
            Err(f) => f,
            Ok(_) => panic!("injected failure was ignored"),
        };
        if generation == 19 {
            assert_eq!(
                failure.terminal().unwrap().status(),
                IterativeStatus::Complete
            );
        }
        let retained = store.document.clone();
        let before = handler.calls;
        let resumed = run(&compiled, &mut handler, &mut store, Some(&retained));
        if generation == 4 {
            assert!(resumed.is_err());
            assert_eq!(handler.calls, before);
        } else {
            let resumed = resumed.unwrap();
            assert_eq!(
                resumed.run().lifecycle().status(),
                IterativeStatus::Complete
            );
            assert_eq!(handler.calls, 3);
            assert_eq!(resumed.usage().calls, 3);
        }
    }
}
#[test]
fn retained_failure_replays_without_handler_and_wrong_root_rejects_before_store() {
    let compiled = super::super::tests::compile();
    let mut handler = Handler {
        wrong: true,
        ..Default::default()
    };
    let mut store = Store::default();
    let first = run(&compiled, &mut handler, &mut store, None).unwrap();
    assert_eq!(first.run().failure(), Some("result_type"));
    assert!(first.usage().result_bytes > 0);
    let retained = store.document.clone();
    let before = handler.calls;
    let replay = run(&compiled, &mut handler, &mut store, Some(&retained)).unwrap();
    assert_eq!(replay.run().failure(), Some("result_type"));
    assert_eq!(handler.calls, before);
    let commits = store.commits;
    let wrong = digest(b"test-root\0", b"wrong");
    assert!(compiled
        .run_durable(
            &task(),
            &proposals(&compiled),
            &mut handler,
            IterativeBudget::default(),
            budget(),
            &AgentCancellation::new(),
            &wrong,
            &root(),
            Some(&retained),
            &mut store,
            10_000_000
        )
        .is_err());
    assert_eq!((handler.calls, store.commits), (before, commits));
}

#[test]
fn target_backends_retain_terminal_failures_and_lost_acks_without_redelivery() {
    if !crate::agent_lifecycle::tests::stage_process_host_supported() {
        return;
    }
    let module_source = super::super::tests::typed_effect_source();
    let compiled = super::super::tests::compile_from_source(&module_source);
    for selector in [0, 1, 2] {
        let label = backend_label(selector);
        let mut handler = Handler {
            wrong: true,
            ..Default::default()
        };
        let mut store = Store::default();
        let failed = run_on(
            &compiled,
            &mut handler,
            &mut store,
            None,
            backend(selector, &module_source),
        )
        .unwrap_or_else(|error| panic!("{label}: terminal failure run: {error:?}"));
        assert_eq!(
            failed.run().lifecycle().status(),
            IterativeStatus::EffectFailed
        );
        assert_eq!(failed.run().failure(), Some("result_type"));
        let retained = store.document.clone();
        let delivered = handler.calls;
        let replay = run_on(
            &compiled,
            &mut handler,
            &mut store,
            Some(&retained),
            backend(selector, &module_source),
        )
        .unwrap_or_else(|error| panic!("{label}: retained failure replay: {error:?}"));
        assert_eq!(
            replay.run().lifecycle().status(),
            IterativeStatus::EffectFailed
        );
        assert_eq!(replay.run().failure(), Some("result_type"));
        assert_eq!(
            (handler.calls, replay.run().dispatched()),
            (delivered, 0),
            "{label}: retained terminal failure redelivered"
        );

        let mut handler = Handler::default();
        let mut store = Store {
            // The commit writes the fully observed terminal checkpoint before
            // reporting the lost acknowledgement to this invocation.
            fail_at: Some(19),
            ..Default::default()
        };
        let failure = match run_on(
            &compiled,
            &mut handler,
            &mut store,
            None,
            backend(selector, &module_source),
        ) {
            Err(failure) => failure,
            Ok(_) => panic!("{label}: lost terminal acknowledgement completed"),
        };
        assert_eq!(
            failure.terminal().map(|run| run.status()),
            Some(IterativeStatus::Complete),
            "{label}: lost acknowledgement selected a different terminal state"
        );
        let retained = store.document.clone();
        let delivered = handler.calls;
        let replay = run_on(
            &compiled,
            &mut handler,
            &mut store,
            Some(&retained),
            backend(selector, &module_source),
        )
        .unwrap_or_else(|error| panic!("{label}: observed lost-ack replay: {error:?}"));
        assert_eq!(replay.run().lifecycle().status(), IterativeStatus::Complete);
        assert_eq!(
            (handler.calls, replay.run().dispatched()),
            (delivered, 0),
            "{label}: observed work redelivered after a lost acknowledgement"
        );
    }
}

#[test]
fn target_backend_fuel_refusal_cannot_replay_or_refund_handler_work() {
    if !crate::agent_lifecycle::tests::stage_process_host_supported() {
        return;
    }
    let module_source = super::super::tests::typed_effect_source();
    let compiled = super::super::tests::compile_from_source(&module_source);
    for selector in [0, 1, 2] {
        let label = backend_label(selector);
        let mut handler = Handler::default();
        let mut store = Store::default();
        assert!(
            run_on_with_fuel(
                &compiled,
                &mut handler,
                &mut store,
                None,
                300_000,
                backend(selector, &module_source),
            )
            .is_err(),
            "{label}: insufficient durable fuel unexpectedly completed"
        );
        assert_eq!(handler.calls, 1, "{label}: first fuel refusal deliveries");
        let retained = store.document.clone();
        let before = (handler.calls, store.commits);
        assert!(
            run_on_with_fuel(
                &compiled,
                &mut handler,
                &mut store,
                Some(&retained),
                300_000,
                backend(selector, &module_source),
            )
            .is_err(),
            "{label}: fuel-exhausted checkpoint replay unexpectedly completed"
        );
        assert_eq!(
            (handler.calls, store.commits),
            before,
            "{label}: fuel-exhausted replay redelivered or persisted work"
        );
    }
}
#[test]
fn reducer_and_recovery_fuel_reservations_cannot_be_refunded() {
    let compiled = super::super::tests::compile();
    let mut handler = Handler::default();
    let mut store = Store::default();
    let first = compiled.run_durable(
        &task(),
        &proposals(&compiled),
        &mut handler,
        IterativeBudget::default(),
        budget(),
        &AgentCancellation::new(),
        &root(),
        &root(),
        None,
        &mut store,
        300_000,
    );
    assert!(first.is_err());
    assert_eq!(handler.calls, 1);
    let retained = store.document.clone();
    let commits = store.commits;
    let replay = compiled.run_durable(
        &task(),
        &proposals(&compiled),
        &mut handler,
        IterativeBudget::default(),
        budget(),
        &AgentCancellation::new(),
        &root(),
        &root(),
        Some(&retained),
        &mut store,
        300_000,
    );
    assert!(replay.is_err());
    assert_eq!((handler.calls, store.commits), (1, commits));
}

/// R15 #293 (checkpoint/migration target parity): the same canonical
/// checkpoint bytes, produced under any of Interpreter/Native/Core Wasm, must
/// decode and continue to completion under any other -- including a
/// genuinely partial checkpoint with real dispatches still outstanding, not
/// only an idempotent replay of an already-complete run.
#[test]
fn checkpoint_bytes_are_target_neutral_across_the_full_backend_matrix() {
    if !crate::agent_lifecycle::tests::stage_process_host_supported() {
        return;
    }
    let native_host = crate::agent_lifecycle::tests::native_stage_host()
        .expect("cross-backend checkpoint parity requires an explicit held compiler");
    let module_source = super::super::tests::typed_effect_source();
    let compiled = super::super::tests::compile_from_source(&module_source);

    for (save_label, save_backend) in matrix_backends(&module_source, &native_host) {
        // A lost acknowledgement mid-run (proven against the ordinary route
        // in `lost_ack_intent_is_uncertain_but_observed_and_transitions_replay_once`)
        // leaves a real, genuinely partial checkpoint: further stage
        // dispatches remain outstanding.
        let mut handler = Handler::default();
        let mut store = Store {
            fail_at: Some(11),
            ..Default::default()
        };
        assert!(
            run_on(&compiled, &mut handler, &mut store, None, save_backend).is_err(),
            "{save_label}: expected a lost acknowledgement, not a clean completion"
        );
        let retained = store.document.clone();
        assert!(!retained.is_empty(), "{save_label}: no partial checkpoint");

        let mut expected: Option<(
            IterativeStatus,
            Option<RetainedValue>,
            CheckpointUsage,
            usize,
            usize,
        )> = None;
        for (restore_label, restore_backend) in matrix_backends(&module_source, &native_host) {
            let mut restore_handler = Handler::default();
            let mut restore_store = Store {
                document: retained.clone(),
                ..Default::default()
            };
            let restored = run_on(
                &compiled,
                &mut restore_handler,
                &mut restore_store,
                Some(&retained),
                restore_backend,
            )
            .unwrap_or_else(|error| panic!("{save_label} -> {restore_label}: {error:?}"));
            assert_eq!(
                restored.run().lifecycle().status(),
                IterativeStatus::Complete,
                "{save_label} -> {restore_label}"
            );
            let summary = (
                restored.run().lifecycle().status(),
                restored.run().lifecycle().value().cloned(),
                restored.usage(),
                restored.iterations(),
                restored.stages(),
            );
            if let Some(expected_summary) = &expected {
                assert_eq!(
                    &summary, expected_summary,
                    "{save_label} -> {restore_label}"
                );
            } else {
                expected = Some(summary);
            }
        }
    }
}

/// Negative control run by hand and reverted rather than committed:
/// commenting out the `wasm_source_mismatch` check in `durable.rs` (so a
/// selected Wasm executor is no longer compared against this registry's own
/// retained source) reproduces a failure of the two "altered Wasm source"
/// assertions below, on both the fresh and the resumed leg, proving this
/// test is not vacuous.
#[test]
fn hostile_checkpoints_are_refused_identically_on_every_backend() {
    if !crate::agent_lifecycle::tests::stage_process_host_supported() {
        return;
    }
    let native_host = crate::agent_lifecycle::tests::native_stage_host()
        .expect("hostile checkpoint parity requires an explicit held compiler");
    let module_source = super::super::tests::typed_effect_source();
    let compiled = super::super::tests::compile_from_source(&module_source);

    let mut handler = Handler::default();
    let mut store = Store {
        fail_at: Some(11),
        ..Default::default()
    };
    assert!(run_on(
        &compiled,
        &mut handler,
        &mut store,
        None,
        crate::agent_lifecycle::authorization::StageBackend::Interpreter,
    )
    .is_err());
    let retained = store.document.clone();
    assert!(!retained.is_empty());

    // Tamper: flip one byte in the middle of the canonical document. The
    // re-rendering check inside `AgentCheckpoint`/`OperationCheckpoint`
    // decode must reject it before any store write or handler dispatch.
    let mut tampered = retained.clone().into_bytes();
    let mid = tampered.len() / 2;
    tampered[mid] = if tampered[mid] == b'0' { b'1' } else { b'0' };
    let tampered = String::from_utf8(tampered).unwrap();

    // Foreign: a program root the checkpoint was never bound to.
    let foreign_root = digest(b"test-root\0", b"foreign");

    // Foreign Wasm source: a Wasm dispatch handed anything but this exact
    // registry's own retained source is refused before decode, identically
    // on the fresh (save) leg and the resumed (restore) leg.
    let altered_source = format!("{module_source}\n");
    let mut h = Handler::default();
    let mut s = Store::default();
    assert!(
        run_on(
            &compiled,
            &mut h,
            &mut s,
            None,
            crate::agent_lifecycle::authorization::StageBackend::Wasm {
                source: &altered_source,
            },
        )
        .is_err(),
        "altered Wasm source accepted on the fresh leg"
    );
    assert_eq!(
        (h.calls, s.commits),
        (0, 0),
        "altered Wasm source delivered or persisted on the fresh leg"
    );
    let mut h = Handler::default();
    let mut s = Store {
        document: retained.clone(),
        ..Default::default()
    };
    assert!(
        run_on(
            &compiled,
            &mut h,
            &mut s,
            Some(&retained),
            crate::agent_lifecycle::authorization::StageBackend::Wasm {
                source: &altered_source,
            },
        )
        .is_err(),
        "altered Wasm source accepted on the resumed leg"
    );
    assert_eq!(
        (h.calls, s.commits),
        (0, 0),
        "altered Wasm source delivered or persisted on the resumed leg"
    );

    for (label, backend) in matrix_backends(&module_source, &native_host) {
        let mut h = Handler::default();
        let mut s = Store {
            document: retained.clone(),
            ..Default::default()
        };
        assert!(
            run_on(&compiled, &mut h, &mut s, Some(&tampered), backend).is_err(),
            "{label}: tampered checkpoint accepted"
        );
        assert_eq!(
            (h.calls, s.commits),
            (0, 0),
            "{label}: tampered checkpoint delivered or persisted"
        );

        let mut h = Handler::default();
        let mut s = Store {
            document: retained.clone(),
            ..Default::default()
        };
        assert!(
            compiled
                .run_durable_on(
                    &task(),
                    &proposals(&compiled),
                    &mut h,
                    IterativeBudget::default(),
                    budget(),
                    &AgentCancellation::new(),
                    &foreign_root,
                    &root(),
                    Some(&retained),
                    &mut s,
                    10_000_000,
                    backend,
                )
                .is_err(),
            "{label}: foreign program root accepted"
        );
        assert_eq!(
            (h.calls, s.commits),
            (0, 0),
            "{label}: foreign checkpoint delivered or persisted"
        );

        // Stale: a caller ceiling that no longer agrees with the one this
        // checkpoint was reserved under.
        let mut h = Handler::default();
        let mut s = Store {
            document: retained.clone(),
            ..Default::default()
        };
        assert!(
            run_on_with_fuel(&compiled, &mut h, &mut s, Some(&retained), 300_000, backend).is_err(),
            "{label}: stale fuel ceiling accepted"
        );
        assert_eq!(
            (h.calls, s.commits),
            (0, 0),
            "{label}: stale-ceiling checkpoint delivered or persisted"
        );
    }
}

/// R15 #293: the migration-seeded route ("migration ... target-parity")
/// gains the same backend selector as the ordinary checkpoint route.
/// The production selected migration route uses the same checked seed and
/// journal; this remains local target evidence only.
#[test]
fn migration_seeded_checkpoint_restores_on_a_different_backend_than_it_saved_on() {
    if !crate::agent_lifecycle::tests::stage_process_host_supported() {
        return;
    }
    use crate::execution_revision::root as execution_root;
    use crate::execution_revision::typed::migration::MigrationSeed;
    use crate::hir::DeclarationId;
    use crate::interpreter::retained_call::{RetainedField, RetainedRecord};

    let native_host = crate::agent_lifecycle::tests::native_stage_host()
        .expect("migration backend parity requires an explicit held compiler");
    let public_native = super::super::NativeTargetHost::open(native_host.compiler_path())
        .expect("migration selector reopens the held compiler");
    let held_wasm = held_wasm_target();
    let module_source = super::super::tests::typed_effect_source();
    let compiled = super::super::tests::compile_from_source(&module_source);

    // A hand-built State one turn short of completion (epoch 2 of 3):
    // exactly what a checked migration function would have produced, without
    // exercising the full destination-runtime/handoff pipeline here.
    let seed_state = RetainedValue::Record(RetainedRecord {
        record: DeclarationId::new("fixture.agent.type.state"),
        fields: vec![
            RetainedField {
                field: DeclarationId::new("fixture.agent.type.state.objective"),
                value: RetainedValue::Bytes(vec![9, 8, 7]),
            },
            RetainedField {
                field: DeclarationId::new("fixture.agent.type.state.budget"),
                value: RetainedValue::I64(10),
            },
            RetainedField {
                field: DeclarationId::new("fixture.agent.type.state.epoch"),
                value: RetainedValue::I64(2),
            },
        ],
    });
    let seed = MigrationSeed::for_test(
        seed_state,
        execution_root(
            "test.migration-seed.v1",
            serde_json::json!({"fixture": "durable-backend-parity"}),
        ),
        CheckpointUsage::default(),
        0,
        0,
        10_000_000,
    );

    let mut expected_status = None;
    let mut expected_value = None;
    let mut retained: Option<String> = None;
    for (label, selected) in [
        ("interpreter", super::super::TargetStageBackend::Interpreter),
        (
            "native -O0",
            super::super::TargetStageBackend::Native(&public_native),
        ),
        (
            "Core Wasm",
            super::super::TargetStageBackend::CoreWasmHeld(&held_wasm),
        ),
    ] {
        let mut handler = Handler::default();
        let mut store = Store::default();
        if let Some(document) = &retained {
            store.document = document.clone();
        }
        let run = compiled
            .run_durable_from_seed_with_backend(
                &task(),
                &proposals(&compiled),
                &mut handler,
                IterativeBudget::default(),
                budget(),
                &AgentCancellation::new(),
                &root(),
                &root(),
                retained.as_deref(),
                &mut store,
                10_000_000,
                &seed,
                selected,
            )
            .unwrap_or_else(|error| panic!("{label}: {error:?}"));
        assert_eq!(
            run.run().lifecycle().status(),
            IterativeStatus::Complete,
            "{label}"
        );
        if let (Some(status), Some(value)) = (&expected_status, &expected_value) {
            assert_eq!(
                Some(run.run().lifecycle().status()),
                Some(*status),
                "{label}"
            );
            assert_eq!(&run.run().lifecycle().value().cloned(), value, "{label}");
            assert_eq!(
                handler.calls, 0,
                "{label}: cross-backend replay of a complete checkpoint redelivered"
            );
        } else {
            assert_eq!(
                handler.calls, 2,
                "{label}: fresh seeded run delivered a different call count"
            );
            expected_status = Some(run.run().lifecycle().status());
            expected_value = Some(run.run().lifecycle().value().cloned());
            retained = Some(run.checkpoint().to_owned());
        }
    }
}
