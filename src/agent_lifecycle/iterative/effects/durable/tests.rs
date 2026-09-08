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
