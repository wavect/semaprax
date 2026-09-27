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

    let mut handler = Handler::default();
    let mut store = Store::default();
    run_on(
        &compiled,
        &mut handler,
        &mut store,
        None,
        backend(0, &module_source),
    )
    .unwrap();
    let retained = store.document.clone();
    let before = (handler.calls, store.commits);
    for selector in [1, 2] {
        let label = backend_label(selector);
        assert!(
            run_on(
                &compiled,
                &mut handler,
                &mut store,
                Some(&retained),
                backend(selector, &module_source),
            )
            .is_err(),
            "{label}: cross-backend checkpoint accepted"
        );
        assert_eq!(
            (handler.calls, store.commits),
            before,
            "{label}: cross-backend refusal delivered or persisted"
        );
    }
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
