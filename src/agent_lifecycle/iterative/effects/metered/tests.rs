use super::*;
use crate::agent_lifecycle::authorization::target_protocol::{
    TargetHostError, TargetHostRequest, TargetResponseSink, TypedCarrier,
};
use crate::agent_lifecycle::iterative::driver::ProposalRequest;

struct Source {
    proposal: String,
    malformed: bool,
    calls: usize,
}
impl ProposalSource for Source {
    fn propose(&mut self, _: ProposalRequest<'_>) -> Result<String, Vec<Diagnostic>> {
        self.calls += 1;
        Ok(if self.malformed {
            "{}".into()
        } else {
            self.proposal.clone()
        })
    }
}
#[derive(Default)]
struct Handler {
    fail: bool,
    cancel: Option<AgentCancellation>,
    wires: Vec<Vec<u8>>,
    responses: Vec<Vec<u8>>,
}
impl TargetHostHandler for Handler {
    fn dispatch(
        &mut self,
        request: &TargetHostRequest,
        sink: &mut TargetResponseSink,
    ) -> Result<(), TargetHostError> {
        self.wires.push(request.canonical_wire());
        if let Some(cancel) = &self.cancel {
            cancel.cancel();
        }
        if self.fail {
            return Err(TargetHostError::Failed);
        }
        let wire = TypedCarrier::new(
            request.operation().result_type(),
            encode_fields(&[("value".into(), RetainedValue::I64(8))]).into_bytes(),
        )
        .unwrap()
        .encode();
        self.responses.push(wire.clone());
        sink.write(&wire).map_err(|_| TargetHostError::Failed)
    }
}
fn budget() -> EffectBudget {
    EffectBudget {
        max_calls: 4,
        max_argument_bytes: 4096,
        max_result_bytes: 4096,
        max_total_bytes: 8192,
    }
}
fn source() -> String {
    super::super::tests::typed_effect_source()
        .replace("if state.epoch < 3", "if state.epoch < 1")
        .replace(
            "    State { objective: task.objective, budget: task.budget, epoch: 1 }",
            r#"    let tag = [65u8, 66u8];
    let scratch = bytes_copy(array_as_slice(tag));
    let rounds = task.budget;
    let mut index = 0;
    while index < rounds {
        index = index + 1;
        0
    }
    let check = 10 / (task.budget + 1);
    State { objective: task.objective, budget: task.budget, epoch: 1 }"#,
        )
}
fn wasm() -> WasmTargetHost {
    std::env::var_os("SEMAPRAX_TEST_WASM_STAGE_NODE")
        .map(std::path::PathBuf::from)
        .into_iter()
        .chain(
            [
                "/usr/bin/node",
                "/usr/local/bin/node",
                "/opt/homebrew/bin/node",
            ]
            .map(std::path::PathBuf::from),
        )
        .find_map(|path| WasmTargetHost::open(path).ok())
        .expect("public metering requires held Node")
}
fn proposal(compiled: &CompiledTypedEffects) -> Source {
    Source {
        proposal: crate::agent_lifecycle::tests::proposal(&compiled.lifecycle.inner, "1", "0"),
        malformed: false,
        calls: 0,
    }
}

#[test]
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn public_selectors_observe_fuel_cleanup_and_settlement() {
    let fixture = crate::agent_lifecycle::tests::native_stage_host().expect("held compiler");
    let native = NativeTargetHost::open(fixture.compiler_path()).unwrap();
    let wasm = wasm();
    let compiled = super::super::tests::compile_from_source(&source());
    for (label, task_budget, fuel, fail, malformed, cancel_at_effect, stage_limit, status) in [
        (
            "complete-exact-limit",
            2,
            3,
            false,
            false,
            false,
            97,
            IterativeStatus::Complete,
        ),
        (
            "fuel-exhaustion",
            2,
            1,
            false,
            false,
            false,
            97,
            IterativeStatus::BudgetExhausted,
        ),
        (
            "checked-failure",
            -1,
            3,
            false,
            false,
            false,
            97,
            IterativeStatus::Rejected,
        ),
        (
            "effect-failure",
            2,
            3,
            true,
            false,
            false,
            97,
            IterativeStatus::EffectFailed,
        ),
        (
            "model-malformed",
            2,
            3,
            false,
            true,
            false,
            97,
            IterativeStatus::ModelFailed,
        ),
        (
            "effect-cancellation",
            2,
            3,
            false,
            false,
            true,
            97,
            IterativeStatus::Cancelled,
        ),
        (
            "stage-ceiling",
            2,
            3,
            false,
            false,
            false,
            1,
            IterativeStatus::BudgetExhausted,
        ),
    ] {
        let mut reference = None;
        let mut native_events = None;
        let mut reference_wires: Option<Vec<Vec<u8>>> = None;
        for selected in [
            TargetStageBackend::Interpreter,
            TargetStageBackend::Native(&native),
            TargetStageBackend::CoreWasmHeld(&wasm),
        ] {
            let cancellation = AgentCancellation::new();
            let mut source = proposal(&compiled);
            source.malformed = malformed;
            let mut handler = Handler {
                fail,
                cancel: cancel_at_effect.then(|| cancellation.clone()),
                ..Default::default()
            };
            let result = compiled
                .run_target_live_metered(
                    &LifecycleTask {
                        objective: vec![1, 2],
                        budget: task_budget,
                    },
                    &mut source,
                    &mut handler,
                    IterativeBudget {
                        max_stages: stage_limit,
                        ..IterativeBudget::default()
                    },
                    budget(),
                    &cancellation,
                    selected,
                    fuel,
                )
                .unwrap_or_else(|e| panic!("{label} {selected:?}: {e:?}"));
            assert_eq!(
                result.run().lifecycle().status(),
                status,
                "{label} {selected:?}"
            );
            let comparable = (
                result.run().lifecycle().value().cloned(),
                result
                    .run()
                    .lifecycle()
                    .stages()
                    .iter()
                    .map(|s| (s.function_id().to_owned(), s.outcome()))
                    .collect::<Vec<_>>(),
                result
                    .observations()
                    .iter()
                    .map(|o| {
                        (
                            o.function_id().to_owned(),
                            o.work().fuel_used,
                            o.work().fuel_limit,
                            o.work().exhausted,
                        )
                    })
                    .collect::<Vec<_>>(),
                handler.wires.len(),
                source.calls,
            );
            if let Some(reference) = &reference {
                assert_eq!(&comparable, reference, "{label} {selected:?}");
            } else {
                reference = Some(comparable);
            }
            assert_eq!(
                result.observations()[0].work().fuel_used,
                if task_budget == 2 && fuel == 3 { 3 } else { 1 }
            );
            assert_eq!(
                result.observations()[0].work().exhausted,
                label == "fuel-exhaustion"
            );
            let events = result
                .observations()
                .iter()
                .map(|o| o.work().finalizer_events.clone())
                .collect::<Vec<_>>();
            if matches!(selected, TargetStageBackend::Interpreter) {
                assert!(events.iter().all(Option::is_none));
            } else {
                assert!(events.iter().all(Option::is_some));
                assert!(
                    events.iter().flatten().any(|events| !events.is_empty()),
                    "non-vacuous cleanup: {label}"
                );
                if let Some(expected) = &native_events {
                    assert_eq!(&events, expected, "{label}: native/Wasm cleanup order");
                } else {
                    native_events = Some(events);
                }
            }
            assert_eq!(result.run().target_evidence().len(), handler.wires.len());
            if label == "complete-exact-limit" {
                assert_eq!(handler.wires.len(), 1);
            }
            for (evidence, wire) in result.run().target_evidence().iter().zip(&handler.wires) {
                evidence.replay_wire(wire).unwrap();
            }
            if let Some(wires) = &reference_wires {
                for (evidence, wire) in result.run().target_evidence().iter().zip(wires) {
                    assert!(
                        evidence.replay_wire(wire).is_err(),
                        "cross-target grant replay fails"
                    );
                }
            } else {
                reference_wires = Some(handler.wires);
            }
            let evidence: serde_json::Value = serde_json::from_str(result.evidence()).unwrap();
            assert_eq!(
                evidence["stages"].as_array().unwrap().len(),
                result.observations().len()
            );
            assert_eq!(evidence["semantic_fuel_limit"], fuel);
        }
    }
}

#[test]
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn invalid_semantic_limits_refuse_and_precancellation_has_no_observations() {
    let fixture = crate::agent_lifecycle::tests::native_stage_host().expect("held compiler");
    let native = NativeTargetHost::open(fixture.compiler_path()).unwrap();
    let wasm = wasm();
    let compiled = super::super::tests::compile_from_source(&source());
    for (selected, fuel) in [
        TargetStageBackend::Interpreter,
        TargetStageBackend::Native(&native),
        TargetStageBackend::CoreWasmHeld(&wasm),
    ]
    .into_iter()
    .flat_map(|selected| [0, 1_000_001, u64::MAX].map(|fuel| (selected, fuel)))
    {
        let mut source = proposal(&compiled);
        let mut handler = Handler::default();
        let cancellation = AgentCancellation::new();
        let run = |source: &mut Source, handler: &mut Handler| {
            compiled.run_target_live_metered(
                &LifecycleTask {
                    objective: vec![],
                    budget: 2,
                },
                source,
                handler,
                IterativeBudget::default(),
                budget(),
                &cancellation,
                selected,
                fuel,
            )
        };
        let errors = run(&mut source, &mut handler)
            .err()
            .expect("invalid limit rejected");
        assert!(
            errors
                .iter()
                .any(|e| e.message.contains("semantic_work.fuel_limit"))
        );
        assert_eq!(source.calls, 0);
        assert!(handler.wires.is_empty());
        cancellation.cancel();
        let result = run(&mut source, &mut handler).unwrap();
        assert_eq!(
            result.run().lifecycle().status(),
            IterativeStatus::Cancelled
        );
        assert!(result.observations().is_empty());
        assert_eq!(result.run().stage_work().reserved_steps(), 0);
    }
}

#[test]
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn semantic_limit_changes_grant_identity_and_refuses_unmetered_profiles() {
    let compiled = super::super::tests::compile_from_source(&source());
    let mut wires: Option<Vec<u8>> = None;
    for fuel in [3, 4] {
        let mut source = proposal(&compiled);
        let mut handler = Handler::default();
        let result = compiled
            .run_target_live_metered(
                &LifecycleTask {
                    objective: vec![1],
                    budget: 2,
                },
                &mut source,
                &mut handler,
                IterativeBudget::default(),
                budget(),
                &AgentCancellation::new(),
                TargetStageBackend::Interpreter,
                fuel,
            )
            .unwrap();
        assert_eq!(result.run().lifecycle().status(), IterativeStatus::Complete);
        assert_eq!(handler.wires.len(), 1);
        if let Some(wire) = &wires {
            assert!(result.run().target_evidence()[0].replay_wire(wire).is_err());
        }
        wires = Some(handler.wires.remove(0));
    }
    let unsupported = source().replace(
        "    let check = 10 / (task.budget + 1);",
        "    let divide = divide;\n    let check = divide(10, task.budget + 1);",
    ).replace(
        "@id(\"app.main\")",
        "@id(\"fixture.divide\")\nfn divide(left: i64, right: i64) -> i64 { left / right }\n\n@id(\"app.main\")",
    );
    let compiled = super::super::tests::compile_from_source(&unsupported);
    let fixture = crate::agent_lifecycle::tests::native_stage_host().expect("held compiler");
    let native = NativeTargetHost::open(fixture.compiler_path()).unwrap();
    let wasm = wasm();
    for selected in [
        TargetStageBackend::Interpreter,
        TargetStageBackend::Native(&native),
        TargetStageBackend::CoreWasmHeld(&wasm),
    ] {
        let mut source = proposal(&compiled);
        let mut handler = Handler::default();
        let errors = compiled
            .run_target_live_metered(
                &LifecycleTask {
                    objective: vec![],
                    budget: 2,
                },
                &mut source,
                &mut handler,
                IterativeBudget::default(),
                budget(),
                &AgentCancellation::new(),
                selected,
                3,
            )
            .err()
            .expect("function values must not run unmetered");
        assert!(
            errors
                .iter()
                .any(|e| e.message.contains("semantic_work.profile.function_value"))
        );
        assert_eq!(source.calls, 0);
        assert!(handler.wires.is_empty());
    }
}
