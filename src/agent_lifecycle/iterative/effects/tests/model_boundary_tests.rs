use super::*;

struct ParityModelHandler {
    proposal: String,
    calls: usize,
    reply: ModelReply,
    response_wires: Vec<Vec<u8>>,
}

fn backend_label(backend: crate::agent_lifecycle::authorization::StageBackend<'_>) -> String {
    if let crate::agent_lifecycle::authorization::StageBackend::Wasm { source } = backend {
        let held = super::public_targets::held_wasm();
        return super::live::target_backend_identity(
            crate::agent_lifecycle::authorization::StageBackend::WasmHeld {
                host: &held.host,
                source,
            },
        );
    }
    super::live::target_backend_identity(backend)
}

fn native_o0() -> crate::agent_lifecycle::authorization::StageBackend<'static> {
    let host = Box::leak(Box::new(
        crate::agent_lifecycle::tests::native_stage_host()
            .expect("native parity requires an explicit held compiler"),
    ));
    crate::agent_lifecycle::tests::native_backend(host)
}

fn native_o2() -> crate::agent_lifecycle::authorization::StageBackend<'static> {
    let host = Box::leak(Box::new(
        crate::agent_lifecycle::tests::native_stage_host()
            .expect("native parity requires an explicit held compiler"),
    ));
    crate::agent_lifecycle::tests::native_o2_backend(host)
}

fn assert_rebound_model_evidence(
    reference_requests: &[Vec<u8>],
    reference_evidence: &[crate::agent_lifecycle::iterative::model::ModelEvidence],
    actual_requests: &[Vec<u8>],
    actual_evidence: &[crate::agent_lifecycle::iterative::model::ModelEvidence],
    actual_responses: &[Vec<u8>],
    label: &str,
) {
    assert_eq!(
        actual_requests.len(),
        reference_requests.len(),
        "{label}: requests"
    );
    assert_eq!(
        actual_evidence.len(),
        reference_evidence.len(),
        "{label}: evidence"
    );
    assert!(
        actual_responses.len() <= actual_evidence.len(),
        "{label}: responses"
    );
    for (ordinal, ((reference_request, reference), (actual_request, actual))) in reference_requests
        .iter()
        .zip(reference_evidence)
        .zip(actual_requests.iter().zip(actual_evidence))
        .enumerate()
    {
        assert_eq!(
            actual.settlement(),
            reference.settlement(),
            "{label}: settlement"
        );
        assert_eq!(
            actual.dispatched(),
            reference.dispatched(),
            "{label}: dispatch"
        );
        assert_eq!(
            actual.accounting(),
            reference.accounting(),
            "{label}: accounting"
        );
        actual
            .replay_exchange_wire(
                actual_request,
                actual_responses.get(ordinal).map(Vec::as_slice),
            )
            .unwrap();
        assert!(
            reference.replay_wire(actual_request).is_err(),
            "{label}: cross-backend model evidence replayed"
        );
        assert_ne!(
            actual_request, reference_request,
            "{label}: model request binding"
        );
    }
}

fn assert_rebound_target_evidence(
    reference: &TargetEffectRun,
    reference_wires: &[Vec<u8>],
    actual: &TargetEffectRun,
    actual_wires: &[Vec<u8>],
    label: &str,
) {
    assert_eq!(
        actual_wires.len(),
        reference_wires.len(),
        "{label}: effect requests"
    );
    assert_eq!(
        actual.target_evidence().len(),
        reference.target_evidence().len(),
        "{label}: effect evidence"
    );
    for ((reference_wire, reference), (actual_wire, actual)) in reference_wires
        .iter()
        .zip(reference.target_evidence())
        .zip(actual_wires.iter().zip(actual.target_evidence()))
    {
        assert_eq!(
            actual.settlement(),
            reference.settlement(),
            "{label}: effect settlement"
        );
        assert_eq!(
            actual.dispatched(),
            reference.dispatched(),
            "{label}: effect dispatch"
        );
        assert_eq!(
            actual.accounting(),
            reference.accounting(),
            "{label}: effect accounting"
        );
        actual.replay_wire(actual_wire).unwrap();
        assert!(
            reference.replay_wire(actual_wire).is_err(),
            "{label}: cross-backend effect evidence replayed"
        );
        assert_ne!(
            actual_wire, reference_wire,
            "{label}: effect request binding"
        );
    }
}

#[derive(Clone, Copy)]
enum ModelReply {
    Valid,
    Malformed,
    Failed,
    Panicked,
    ResponseOverLimit,
}

impl crate::agent_lifecycle::iterative::model::ModelHostHandler for ParityModelHandler {
    fn dispatch(
        &mut self,
        request: &crate::agent_lifecycle::iterative::model::ModelHostRequest,
        sink: &mut crate::agent_lifecycle::iterative::model::ModelResponseSink,
    ) -> Result<(), crate::agent_lifecycle::iterative::model::ModelHostError> {
        assert_eq!(request.attempt(), 0);
        assert!(!request.context().is_empty());
        assert!(!request.proposal_schema_digest().is_empty());
        self.calls += 1;
        match self.reply {
            ModelReply::Valid => {
                self.response_wires.push(self.proposal.as_bytes().to_vec());
                sink.write(self.proposal.as_bytes())
            }
            ModelReply::Malformed => {
                self.response_wires.push(vec![0xff]);
                sink.write(&[0xff])
            }
            ModelReply::Failed => {
                return Err(crate::agent_lifecycle::iterative::model::ModelHostError::Failed)
            }
            ModelReply::Panicked => panic!("deliberate model host panic"),
            ModelReply::ResponseOverLimit => {
                // The source must observe the sink's sticky overflow state;
                // a host cannot turn a rejected write into a successful result.
                let _ = sink.write(&vec![b'x'; 16 * 1024 + 1]);
                return Ok(());
            }
        }
        .map_err(|_| crate::agent_lifecycle::iterative::model::ModelHostError::Failed)
    }
}

fn model_target_run_on(
    compiled: &CompiledTypedEffects,
    backend: crate::agent_lifecycle::authorization::StageBackend<'_>,
    limits: crate::agent_lifecycle::iterative::model::ModelLimits,
    cancellation: &AgentCancellation,
    reply: ModelReply,
) -> (
    Result<TargetEffectRun, Vec<Diagnostic>>,
    Vec<Vec<u8>>,
    Vec<crate::agent_lifecycle::iterative::model::ModelEvidence>,
    crate::agent_lifecycle::iterative::model::ModelAccounting,
    usize,
    ParityTargetHandler,
    Vec<Vec<u8>>,
) {
    let proposal = crate::agent_lifecycle::tests::proposal(&compiled.lifecycle.inner, "1", "0");
    let mut model_handler = ParityModelHandler {
        proposal,
        calls: 0,
        reply,
        response_wires: Vec::new(),
    };
    let backend_label = backend_label(backend);
    let binding = crate::agent_lifecycle::iterative::model::ModelSourceBinding::for_target(
        "fixture.agent.model.target-parity",
        compiled.digest(),
        &backend_label,
    )
    .unwrap();
    let mut source = crate::agent_lifecycle::iterative::model::TargetModelSource::new(
        binding,
        limits,
        cancellation,
        &mut model_handler,
    );
    let mut target = ParityTargetHandler {
        calls: 0,
        request_wires: Vec::new(),
        grants: Vec::new(),
    };
    let run = compiled.run_public_target_fixture(
        &LifecycleTask {
            objective: vec![],
            budget: 10,
        },
        &mut source,
        &mut target,
        IterativeBudget::default(),
        budgets(),
        cancellation,
        backend,
    );
    let requests = source.request_wires().to_vec();
    let evidence = source.evidence().to_vec();
    let accounting = source.accounting();
    drop(source);
    let model_calls = model_handler.calls;
    let responses = model_handler.response_wires;
    (
        run,
        requests,
        evidence,
        accounting,
        model_calls,
        target,
        responses,
    )
}

fn model_limits() -> crate::agent_lifecycle::iterative::model::ModelLimits {
    crate::agent_lifecycle::iterative::model::ModelLimits {
        max_calls: 4,
        max_request_bytes: 64 * 1024,
        max_response_bytes: 16 * 1024,
        max_total_bytes: 96 * 1024,
        max_fuel: 4,
    }
}

fn target_attempt_on(
    compiled: &CompiledTypedEffects,
    backend: crate::agent_lifecycle::authorization::StageBackend<'_>,
    cancellation: &AgentCancellation,
    effects: EffectBudget,
    proposals: Vec<String>,
    handler: &mut dyn crate::agent_lifecycle::authorization::target_protocol::TargetHostHandler,
) -> Result<TargetEffectRun, Vec<Diagnostic>> {
    let mut source = TargetSource { proposals, next: 0 };
    compiled.run_public_target_fixture(
        &LifecycleTask {
            objective: vec![],
            budget: 10,
        },
        &mut source,
        handler,
        IterativeBudget::default(),
        effects,
        cancellation,
        backend,
    )
}

struct MalformedTargetHandler {
    calls: usize,
}
impl crate::agent_lifecycle::authorization::target_protocol::TargetHostHandler
    for MalformedTargetHandler
{
    fn dispatch(
        &mut self,
        _: &crate::agent_lifecycle::authorization::target_protocol::TargetHostRequest,
        sink: &mut crate::agent_lifecycle::authorization::target_protocol::TargetResponseSink,
    ) -> Result<(), crate::agent_lifecycle::authorization::target_protocol::TargetHostError> {
        self.calls += 1;
        sink.write(b"not a target carrier").map_err(|_| {
            crate::agent_lifecycle::authorization::target_protocol::TargetHostError::Failed
        })
    }
}

/// A successful target must be observed once for every turn, rather than
/// allowing a later backend leg to replay an earlier host response.  Distinct
/// values make a cached or duplicated callback visible at the protocol edge.
struct SequentialTargetHandler {
    calls: usize,
    request_wires: Vec<Vec<u8>>,
    result_wires: Vec<Vec<u8>>,
    grants: Vec<String>,
    returned_values: Vec<i64>,
}

impl crate::agent_lifecycle::authorization::target_protocol::TargetHostHandler
    for SequentialTargetHandler
{
    fn dispatch(
        &mut self,
        request: &crate::agent_lifecycle::authorization::target_protocol::TargetHostRequest,
        sink: &mut crate::agent_lifecycle::authorization::target_protocol::TargetResponseSink,
    ) -> Result<(), crate::agent_lifecycle::authorization::target_protocol::TargetHostError> {
        self.calls += 1;
        self.request_wires.push(request.canonical_wire());
        self.grants.push(request.grant_id().to_owned());
        let value = 7 + i64::try_from(self.calls).expect("bounded fixture callback ordinal");
        self.returned_values.push(value);
        let payload = encode_fields(&[("value".into(), RetainedValue::I64(value))]);
        let result = crate::agent_lifecycle::authorization::target_protocol::TypedCarrier::new(
            request.operation().result_type(),
            payload.into_bytes(),
        )
        .map_err(|_| {
            crate::agent_lifecycle::authorization::target_protocol::TargetHostError::Failed
        })?
        .encode();
        self.result_wires.push(result.clone());
        sink.write(&result).map_err(|_| {
            crate::agent_lifecycle::authorization::target_protocol::TargetHostError::Failed
        })
    }
}

fn successful_target_run_on(
    compiled: &CompiledTypedEffects,
    module_source: &str,
    backend: crate::agent_lifecycle::authorization::StageBackend<'_>,
    cancellation: &AgentCancellation,
) -> (TargetEffectRun, SequentialTargetHandler) {
    let proposal = crate::agent_lifecycle::tests::proposal(&compiled.lifecycle.inner, "1", "0");
    let mut source = TargetSource {
        proposals: vec![proposal; 4],
        next: 0,
    };
    let mut handler = SequentialTargetHandler {
        calls: 0,
        request_wires: Vec::new(),
        result_wires: Vec::new(),
        grants: Vec::new(),
        returned_values: Vec::new(),
    };
    let run = compiled
        .run_public_target_fixture(
            &LifecycleTask {
                objective: vec![],
                budget: 10,
            },
            &mut source,
            &mut handler,
            IterativeBudget::default(),
            budgets(),
            cancellation,
            backend,
        )
        .unwrap_or_else(|errors| panic!("successful target {module_source:?}: {errors:?}"));
    (run, handler)
}

fn assert_successful_target_settlements(
    run: &TargetEffectRun,
    handler: &SequentialTargetHandler,
    label: &str,
) {
    assert_eq!(
        run.lifecycle().status(),
        IterativeStatus::Complete,
        "{label}"
    );
    assert_eq!(run.failure(), None, "{label}: terminal failure");
    assert_eq!(handler.calls, 3, "{label}: callback count");
    assert_eq!(
        handler.returned_values,
        [8, 9, 10],
        "{label}: callback values"
    );
    assert_eq!(
        handler.request_wires.len(),
        handler.calls,
        "{label}: requests"
    );
    assert_eq!(
        handler.result_wires.len(),
        handler.calls,
        "{label}: results"
    );
    assert_eq!(
        run.target_evidence().len(),
        handler.calls,
        "{label}: evidence"
    );
    assert_eq!(
        handler
            .grants
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        handler.calls,
        "{label}: fresh grants"
    );

    let mut previous =
        crate::agent_lifecycle::authorization::target_protocol::TargetAccounting::default();
    for (ordinal, ((evidence, request_wire), result_wire)) in run
        .target_evidence()
        .iter()
        .zip(&handler.request_wires)
        .zip(&handler.result_wires)
        .enumerate()
    {
        assert_eq!(
            evidence.settlement(),
            crate::agent_lifecycle::authorization::target_protocol::Settlement::Returned,
            "{label}: callback {ordinal} settlement"
        );
        assert!(
            evidence.dispatched(),
            "{label}: callback {ordinal} dispatch"
        );
        let accounting = evidence.accounting();
        assert_eq!(accounting.calls(), u64::try_from(ordinal + 1).unwrap());
        assert!(
            accounting.request_bytes() > previous.request_bytes()
                && accounting.result_bytes() > previous.result_bytes()
                && accounting.fuel() > previous.fuel(),
            "{label}: callback {ordinal} accounting must remain cumulative"
        );
        let decoded =
            crate::agent_lifecycle::authorization::target_protocol::TargetEvidence::decode(
                &evidence.canonical_wire(),
            )
            .unwrap_or_else(|error| panic!("{label}: callback {ordinal} evidence: {error:?}"));
        assert_eq!(
            &decoded, evidence,
            "{label}: callback {ordinal} evidence bytes"
        );
        decoded
            .replay_exchange_wire(request_wire, Some(result_wire))
            .unwrap_or_else(|error| panic!("{label}: callback {ordinal} replay: {error:?}"));
        previous = accounting;
    }
    assert_eq!(run.accounting(), previous, "{label}: final accounting");
    assert_eq!(handler.calls, 3, "{label}: replay must not invoke callback");
}

#[test]
fn successful_target_settlement_is_cumulative_and_backend_source_bound() {
    if !target_backend_tools_available() {
        eprintln!("skipping successful target settlement parity: clang or node unavailable");
        return;
    }
    let module_source = typed_effect_source();
    // This remains the same program but has distinct Core Wasm source bytes.
    // The target grant commits the exact source digest, so its retained
    // observation cannot be replayed as authority for this sibling input.
    let equivalent_wasm_source = format!("{module_source}\n");
    let compiled = compile_from_source(&module_source);
    let cancellation = AgentCancellation::new();
    let interpreter = successful_target_run_on(
        &compiled,
        &module_source,
        crate::agent_lifecycle::authorization::StageBackend::Interpreter,
        &cancellation,
    );
    let native_o0 = successful_target_run_on(&compiled, &module_source, native_o0(), &cancellation);
    let native_o2 = successful_target_run_on(&compiled, &module_source, native_o2(), &cancellation);
    let wasm = successful_target_run_on(
        &compiled,
        &module_source,
        crate::agent_lifecycle::authorization::StageBackend::Wasm {
            source: &module_source,
        },
        &cancellation,
    );
    // Public selectors retain their own source; callers cannot substitute
    // sibling source bytes at dispatch. Compile that sibling independently.
    let equivalent_compiled = compile_from_source(&equivalent_wasm_source);
    let wasm_equivalent_source = successful_target_run_on(
        &equivalent_compiled,
        &equivalent_wasm_source,
        crate::agent_lifecycle::authorization::StageBackend::Wasm {
            source: &equivalent_wasm_source,
        },
        &cancellation,
    );
    let runs = [
        ("interpreter", &interpreter.0, &interpreter.1),
        ("native -O0", &native_o0.0, &native_o0.1),
        ("native -O2", &native_o2.0, &native_o2.1),
        ("Core Wasm", &wasm.0, &wasm.1),
        (
            "Core Wasm equivalent source",
            &wasm_equivalent_source.0,
            &wasm_equivalent_source.1,
        ),
    ];
    for (label, run, handler) in &runs {
        assert_successful_target_settlements(run, handler, label);
        assert_eq!(
            run.lifecycle().value(),
            interpreter.0.lifecycle().value(),
            "{label}: lifecycle value"
        );
        assert_eq!(
            run.accounting(),
            interpreter.0.accounting(),
            "{label}: final accounting"
        );
    }
    for (label, run, handler) in &runs {
        for (other_label, _, other_handler) in &runs {
            if label == other_label {
                continue;
            }
            for (ordinal, (evidence, own_request)) in run
                .target_evidence()
                .iter()
                .zip(&handler.request_wires)
                .enumerate()
            {
                let other_request = &other_handler.request_wires[ordinal];
                assert_ne!(
                    own_request, other_request,
                    "{label} / {other_label}: callback {ordinal} grant binding"
                );
                assert!(
                    evidence.replay_wire(other_request).is_err(),
                    "{label} / {other_label}: callback {ordinal} cross-boundary replay"
                );
            }
        }
    }
}

#[test]
fn target_hostile_proposals_budgets_and_results_settle_without_extra_dispatch_on_every_backend() {
    if !target_backend_tools_available() {
        eprintln!("skipping target hostile bridge: clang or node unavailable");
        return;
    }
    let module_source = typed_effect_source();
    let compiled = compile_from_source(&module_source);
    let proposal = crate::agent_lifecycle::tests::proposal(&compiled.lifecycle.inner, "1", "0");
    for (label, backend) in [
        (
            "interpreter",
            crate::agent_lifecycle::authorization::StageBackend::Interpreter,
        ),
        ("native -O0", native_o0()),
        ("native -O2", native_o2()),
        (
            "Core Wasm",
            crate::agent_lifecycle::authorization::StageBackend::Wasm {
                source: &module_source,
            },
        ),
    ] {
        let cancellation = AgentCancellation::new();
        let mut malformed_proposal = ParityTargetHandler {
            calls: 0,
            request_wires: Vec::new(),
            grants: Vec::new(),
        };
        let malformed = target_attempt_on(
            &compiled,
            backend,
            &cancellation,
            budgets(),
            vec![
                "not canonical proposal".into();
                crate::agent_lifecycle::iterative::driver::MAX_PROPOSAL_ATTEMPTS
            ],
            &mut malformed_proposal,
        )
        .unwrap();
        assert_eq!(
            malformed.lifecycle().status(),
            IterativeStatus::ModelFailed,
            "{label}: proposal"
        );
        assert!(
            malformed.target_evidence().is_empty(),
            "{label}: proposal evidence"
        );
        assert_eq!(
            malformed_proposal.calls, 0,
            "{label}: malformed proposal dispatched host work"
        );

        let mut budget_handler = ParityTargetHandler {
            calls: 0,
            request_wires: Vec::new(),
            grants: Vec::new(),
        };
        let exhausted = target_attempt_on(
            &compiled,
            backend,
            &cancellation,
            EffectBudget {
                max_calls: 0,
                ..budgets()
            },
            vec![proposal.clone()],
            &mut budget_handler,
        )
        .unwrap();
        assert_eq!(
            exhausted.lifecycle().status(),
            IterativeStatus::EffectFailed,
            "{label}: budget status"
        );
        assert_eq!(
            exhausted.target_evidence().len(),
            1,
            "{label}: budget evidence"
        );
        assert_eq!(
            exhausted.target_evidence()[0].settlement(),
            crate::agent_lifecycle::authorization::target_protocol::Settlement::CallBudget,
            "{label}: budget settlement"
        );
        assert!(
            !exhausted.target_evidence()[0].dispatched(),
            "{label}: budget dispatch"
        );
        assert_eq!(
            budget_handler.calls, 0,
            "{label}: exhausted budget dispatched host work"
        );

        let mut malformed_handler = MalformedTargetHandler { calls: 0 };
        let malformed_result = target_attempt_on(
            &compiled,
            backend,
            &cancellation,
            budgets(),
            vec![proposal.clone()],
            &mut malformed_handler,
        )
        .unwrap();
        assert_eq!(
            malformed_result.lifecycle().status(),
            IterativeStatus::EffectFailed,
            "{label}: result status"
        );
        assert_eq!(
            malformed_result.target_evidence().len(),
            1,
            "{label}: result evidence"
        );
        assert_eq!(
            malformed_result.target_evidence()[0].settlement(),
            crate::agent_lifecycle::authorization::target_protocol::Settlement::MalformedResult,
            "{label}: result settlement"
        );
        assert!(
            malformed_result.target_evidence()[0].dispatched(),
            "{label}: malformed result was not observed"
        );
        assert_eq!(
            malformed_handler.calls, 1,
            "{label}: malformed result repeated host work"
        );
    }
}

#[test]
fn target_grants_reject_stale_program_evidence_on_every_backend() {
    if !target_backend_tools_available() {
        eprintln!("skipping stale target binding bridge: clang or node unavailable");
        return;
    }
    let module_source = typed_effect_source();
    // These two source programs produce the same bounded fixture turns, but
    // are distinct checked programs. A target grant must bind that program
    // identity rather than treating matching tool/result bytes as authority.
    let stale_source = module_source.replace("sequence <= 1usize", "sequence < 2usize");
    let compiled = compile_from_source(&module_source);
    let stale = compile_from_source(&stale_source);
    assert_ne!(
        compiled.digest(),
        stale.digest(),
        "fixture programs must bind different roots"
    );
    for (label, backend, stale_backend) in [
        (
            "interpreter",
            crate::agent_lifecycle::authorization::StageBackend::Interpreter,
            crate::agent_lifecycle::authorization::StageBackend::Interpreter,
        ),
        ("native -O0", native_o0(), native_o0()),
        ("native -O2", native_o2(), native_o2()),
        (
            "Core Wasm",
            crate::agent_lifecycle::authorization::StageBackend::Wasm {
                source: &module_source,
            },
            crate::agent_lifecycle::authorization::StageBackend::Wasm {
                source: &stale_source,
            },
        ),
    ] {
        let cancellation = AgentCancellation::new();
        let (current, current_handler) =
            target_run_on(&compiled, &module_source, backend, &cancellation);
        let (stale_run, stale_handler) =
            target_run_on(&stale, &stale_source, stale_backend, &cancellation);
        assert_eq!(
            current.lifecycle().status(),
            stale_run.lifecycle().status(),
            "{label}: terminal status"
        );
        assert_eq!(
            current.accounting(),
            stale_run.accounting(),
            "{label}: accounting"
        );
        assert_eq!(
            current_handler.calls, stale_handler.calls,
            "{label}: host calls"
        );
        assert_eq!(
            current.target_evidence().len(),
            stale_run.target_evidence().len(),
            "{label}: evidence count"
        );
        for ((current_evidence, current_wire), stale_wire) in current
            .target_evidence()
            .iter()
            .zip(&current_handler.request_wires)
            .zip(&stale_handler.request_wires)
        {
            current_evidence.replay_wire(current_wire).unwrap();
            assert!(
                current_evidence.replay_wire(stale_wire).is_err(),
                "{label}: stale program evidence replayed"
            );
            assert_ne!(
                current_wire, stale_wire,
                "{label}: stale program reused grant wire"
            );
        }
    }
}

#[test]
fn model_and_effect_host_boundaries_replay_identically_across_stage_backends() {
    if !target_backend_tools_available() {
        eprintln!("skipping target model bridge: clang or node unavailable");
        return;
    }
    let module_source = typed_effect_source();
    let compiled = compile_from_source(&module_source);
    let cancellation = AgentCancellation::new();
    let expected = model_target_run_on(
        &compiled,
        crate::agent_lifecycle::authorization::StageBackend::Interpreter,
        model_limits(),
        &cancellation,
        ModelReply::Valid,
    );
    let expected_run = expected.0.as_ref().unwrap();
    assert_eq!(expected_run.lifecycle().status(), IterativeStatus::Complete);
    assert_eq!((expected.4, expected.5.calls), (3, 3));
    assert_eq!(expected.1.len(), 3);
    assert_eq!(expected.2.len(), 3);
    assert_eq!(expected.6.len(), 3);
    assert_eq!(expected.3.calls(), 3);
    assert_eq!(
        expected
            .2
            .iter()
            .map(|evidence| evidence.grant_id())
            .collect::<std::collections::HashSet<_>>()
            .len(),
        3,
        "per-turn model grants must be distinct"
    );
    for ((wire, evidence), response) in expected.1.iter().zip(&expected.2).zip(&expected.6) {
        crate::agent_lifecycle::iterative::model::ModelEvidence::decode(&evidence.canonical_wire())
            .unwrap()
            .replay_exchange_wire(wire, Some(response))
            .unwrap();
        let mut forged_request = wire.clone();
        *forged_request.last_mut().unwrap() ^= 1;
        assert!(evidence.replay_wire(&forged_request).is_err());
        let mut forged_evidence = evidence.canonical_wire();
        *forged_evidence.last_mut().unwrap() ^= 1;
        assert!(
            crate::agent_lifecycle::iterative::model::ModelEvidence::decode(&forged_evidence)
                .is_err()
        );
    }

    for (label, backend) in [
        ("native -O0", native_o0()),
        ("native -O2", native_o2()),
        (
            "Core Wasm",
            crate::agent_lifecycle::authorization::StageBackend::Wasm {
                source: &module_source,
            },
        ),
    ] {
        let cancellation = AgentCancellation::new();
        let actual = model_target_run_on(
            &compiled,
            backend,
            model_limits(),
            &cancellation,
            ModelReply::Valid,
        );
        let actual_run = actual
            .0
            .as_ref()
            .unwrap_or_else(|errors| panic!("{label}: {errors:?}"));
        assert_eq!(
            actual_run.lifecycle().status(),
            expected_run.lifecycle().status(),
            "{label}"
        );
        assert_eq!(
            actual_run.lifecycle().value(),
            expected_run.lifecycle().value(),
            "{label}"
        );
        assert_rebound_model_evidence(
            &expected.1,
            &expected.2,
            &actual.1,
            &actual.2,
            &actual.6,
            label,
        );
        assert_eq!(actual.3, expected.3, "{label}: model accounting");
        assert_rebound_target_evidence(
            expected_run,
            &expected.5.request_wires,
            actual_run,
            &actual.5.request_wires,
            label,
        );
        assert_eq!(
            actual_run.accounting(),
            expected_run.accounting(),
            "{label}: effect accounting"
        );
    }
}

#[test]
fn model_boundary_settles_every_refusal_before_effect_dispatch_on_all_stage_backends() {
    if !target_backend_tools_available() {
        eprintln!("skipping target model refusal parity: clang or node unavailable");
        return;
    }
    let module_source = typed_effect_source();
    let compiled = compile_from_source(&module_source);
    for (label, limits, reply, expected, dispatched, model_calls) in [
        (
            "fuel",
            crate::agent_lifecycle::iterative::model::ModelLimits {
                max_fuel: 0,
                ..model_limits()
            },
            ModelReply::Valid,
            crate::agent_lifecycle::iterative::model::ModelSettlement::FuelExhausted,
            false,
            0,
        ),
        (
            "call budget",
            crate::agent_lifecycle::iterative::model::ModelLimits {
                max_calls: 0,
                ..model_limits()
            },
            ModelReply::Valid,
            crate::agent_lifecycle::iterative::model::ModelSettlement::CallBudget,
            false,
            0,
        ),
        (
            "request budget",
            crate::agent_lifecycle::iterative::model::ModelLimits {
                max_request_bytes: 1,
                ..model_limits()
            },
            ModelReply::Valid,
            crate::agent_lifecycle::iterative::model::ModelSettlement::RequestBudget,
            false,
            0,
        ),
        (
            "total budget",
            crate::agent_lifecycle::iterative::model::ModelLimits {
                max_total_bytes: 1,
                ..model_limits()
            },
            ModelReply::Valid,
            crate::agent_lifecycle::iterative::model::ModelSettlement::RequestBudget,
            false,
            0,
        ),
        (
            "malformed response",
            model_limits(),
            ModelReply::Malformed,
            crate::agent_lifecycle::iterative::model::ModelSettlement::MalformedResponse,
            true,
            1,
        ),
        (
            "response budget",
            model_limits(),
            ModelReply::ResponseOverLimit,
            crate::agent_lifecycle::iterative::model::ModelSettlement::ResponseBudget,
            true,
            1,
        ),
        (
            "host failed",
            model_limits(),
            ModelReply::Failed,
            crate::agent_lifecycle::iterative::model::ModelSettlement::HostFailed,
            true,
            1,
        ),
        (
            "host panicked",
            model_limits(),
            ModelReply::Panicked,
            crate::agent_lifecycle::iterative::model::ModelSettlement::HostPanicked,
            true,
            1,
        ),
    ] {
        let cancellation = AgentCancellation::new();
        let reference = model_target_run_on(
            &compiled,
            crate::agent_lifecycle::authorization::StageBackend::Interpreter,
            limits,
            &cancellation,
            reply,
        );
        assert!(reference.0.is_err());
        assert_eq!(reference.2.len(), 1);
        assert_eq!(reference.2[0].settlement(), expected);
        assert_eq!(reference.2[0].dispatched(), dispatched);
        assert_eq!(reference.4, model_calls);
        assert_eq!(reference.5.calls, 0);

        for (label, backend) in [
            ("native -O0", native_o0()),
            ("native -O2", native_o2()),
            (
                "Core Wasm",
                crate::agent_lifecycle::authorization::StageBackend::Wasm {
                    source: &module_source,
                },
            ),
        ] {
            let cancellation = AgentCancellation::new();
            let actual = model_target_run_on(&compiled, backend, limits, &cancellation, reply);
            assert_eq!(
                actual.0.as_ref().err().map(|errors| errors
                    .iter()
                    .map(|error| (&error.code, &error.message))
                    .collect::<Vec<_>>()),
                reference.0.as_ref().err().map(|errors| errors
                    .iter()
                    .map(|error| (&error.code, &error.message))
                    .collect::<Vec<_>>()),
                "{label}: model refusal"
            );
            assert_rebound_model_evidence(
                &reference.1,
                &reference.2,
                &actual.1,
                &actual.2,
                &actual.6,
                label,
            );
            assert_eq!(actual.3, reference.3, "{label}: model accounting");
            assert_eq!(actual.4, reference.4, "{label}: model calls");
            assert_eq!(actual.5.calls, 0, "{label}: effects must not dispatch");
            assert!(
                actual.5.request_wires.is_empty(),
                "{label}: effects must not request"
            );
        }
    }

    let cancellation = AgentCancellation::new();
    cancellation.cancel();
    let reference = model_target_run_on(
        &compiled,
        crate::agent_lifecycle::authorization::StageBackend::Interpreter,
        model_limits(),
        &cancellation,
        ModelReply::Valid,
    );
    assert_eq!(
        reference.0.unwrap().lifecycle().status(),
        IterativeStatus::Cancelled
    );
    assert!(reference.1.is_empty());
    assert!(reference.2.is_empty());
    assert_eq!(reference.3, Default::default());
    assert_eq!((reference.4, reference.5.calls), (0, 0));
    for (label, backend) in [
        ("native -O0", native_o0()),
        ("native -O2", native_o2()),
        (
            "Core Wasm",
            crate::agent_lifecycle::authorization::StageBackend::Wasm {
                source: &module_source,
            },
        ),
    ] {
        let cancellation = AgentCancellation::new();
        cancellation.cancel();
        let actual = model_target_run_on(
            &compiled,
            backend,
            model_limits(),
            &cancellation,
            ModelReply::Valid,
        );
        assert_eq!(
            actual.0.unwrap().lifecycle().status(),
            IterativeStatus::Cancelled,
            "{label}"
        );
        assert_eq!(actual.1, reference.1, "{label}: model requests");
        assert_eq!(actual.2, reference.2, "{label}: model evidence");
        assert_eq!(actual.3, reference.3, "{label}: model accounting");
        assert_eq!(actual.4, reference.4, "{label}: model calls");
        assert_eq!(actual.5.calls, reference.5.calls, "{label}: effect calls");
        assert_eq!(
            actual.5.request_wires, reference.5.request_wires,
            "{label}: effect requests"
        );
    }
}
