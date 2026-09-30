//! Scalar-rich Result-carrier parity for target-native Agent stages (#182).
//!
//! `Task`, `State`, `Observation`, `Outcome`, and `Decision` remain in the
//! frozen lifecycle's existing `Bytes`/`i64` profile. `Result` is deliberately
//! wider: its `bool` and `usize` leaves prove that the target result codecs do
//! not silently drop source-admitted scalar data. `usize` is carried as the
//! language's target-independent `u64`, including `u64::MAX`; it is never
//! narrowed through a host `usize` or JavaScript `Number`.

use super::*;

const REPORT: &str = r#"@id("fixture.agent.type.result")
record Report {
    @id("fixture.agent.type.result.summary") summary: Bytes,
    @id("fixture.agent.type.result.budget") budget: i64,
    @id("fixture.agent.type.result.status") status: i64,
}
"#;

const SCALAR_REPORT: &str = r#"@id("fixture.agent.type.result")
record Report {
    @id("fixture.agent.type.result.summary") summary: Bytes,
    @id("fixture.agent.type.result.budget") budget: i64,
    @id("fixture.agent.type.result.status") status: i64,
    @id("fixture.agent.type.result.approved") approved: bool,
    @id("fixture.agent.type.result.count") count: usize,
}
"#;

const I32_REPORT: &str = r#"@id("fixture.agent.type.result")
record Report {
    @id("fixture.agent.type.result.summary") summary: Bytes,
    @id("fixture.agent.type.result.budget") budget: i64,
    @id("fixture.agent.type.result.status") status: i64,
    @id("fixture.agent.type.result.phase") phase: i32,
}
"#;

const PROPOSAL: &str = r#"@id("fixture.agent.type.proposal")
record Proposal {
    @id("fixture.agent.type.proposal.budget") budget: i64,
    @id("fixture.agent.type.proposal.urgent") urgent: bool,
    @id("fixture.agent.type.proposal.sequence") sequence: usize,
}
"#;

const I32_PROPOSAL: &str = r#"@id("fixture.agent.type.proposal")
record Proposal {
    @id("fixture.agent.type.proposal.budget") budget: i64,
    @id("fixture.agent.type.proposal.urgent") urgent: bool,
    @id("fixture.agent.type.proposal.sequence") sequence: usize,
    @id("fixture.agent.type.proposal.phase") phase: i32,
}
"#;

const REPORT_TAIL: &str = "        status: outcome.status + state.epoch,\n    }";

fn source(report: &str, tail: &str) -> String {
    MODULE
        .replacen(REPORT, report, 1)
        .replacen(REPORT_TAIL, tail, 1)
}

fn scalar_source() -> String {
    source(
        SCALAR_REPORT,
        "        status: outcome.status + state.epoch,\n        approved: urgent,\n        count: sequence,\n    }",
    )
}

fn unsupported_source() -> String {
    source(
        I32_REPORT,
        "        status: outcome.status + state.epoch,\n        phase: 7i32,\n    }",
    )
}

/// `ScalarKind::I32` is an admitted Proposal projection. Its signed minimum
/// is deliberately exercised because neither target may route it through a
/// positive magnitude or a lossy host number while rebuilding the stage call.
fn i32_proposal_source() -> String {
    MODULE
        .replacen(PROPOSAL, I32_PROPOSAL, 1)
        .replacen(
            "fn authorize(state: borrow State, budget: i64, urgent: bool, sequence: usize) -> Decision",
            "fn authorize(state: borrow State, budget: i64, urgent: bool, sequence: usize, phase: i32) -> Decision",
            1,
        )
        .replacen(
            "fn reduce(state: own State, budget: i64, urgent: bool, sequence: usize, outcome: own Outcome) -> Report",
            "fn reduce(state: own State, budget: i64, urgent: bool, sequence: usize, phase: i32, outcome: own Outcome) -> Report",
            1,
        )
}

fn compile(source: &str) -> CompiledAgentLifecycle {
    compile_agent_lifecycle(
        source,
        "target-scalar-carriers.spx",
        &DEFINITION.replace("RUNTIME", RUNTIME_V1),
    )
    .expect("the checked scalar-result lifecycle binds")
}

fn returned(
    label: &str,
    evaluation: crate::interpreter::retained_call::RetainedCallEvaluation,
) -> RetainedValue {
    let RetainedCallOutcome::Returned(value) = evaluation.outcome else {
        panic!("{label} did not return a carrier");
    };
    value
}

fn dispatch(
    backend: authorization::StageBackend<'_>,
    compiled: &CompiledAgentLifecycle,
    prepared: &crate::interpreter::retained_call::PreparedRetainedCall,
    arguments: &[RetainedValue],
) -> RetainedCallEvaluation {
    authorization::dispatch_on(
        backend,
        &compiled.program,
        prepared,
        arguments,
        DEFAULT_STAGE_STEPS,
    )
    .unwrap_or_else(|errors| panic!("target-native scalar carrier dispatch: {errors:?}"))
}

/// Drives every deterministic stage through one backend. The report carries
/// both extra scalar leaves; the authorizing call also carries `u64::MAX` as
/// a genuine `usize` source argument so the generated drivers cannot route it
/// through a lossy host-number representation.
fn drive(
    backend: authorization::StageBackend<'_>,
    source: &str,
    compiled: &CompiledAgentLifecycle,
) -> Vec<RetainedValue> {
    let task = payload(&compiled.binding.task, b"scalar".to_vec(), 10);
    let initialized = returned(
        "initialize",
        dispatch(
            backend,
            compiled,
            compiled.binding.initialize.prepared(),
            std::slice::from_ref(&task),
        ),
    );

    // Reconstruct the backend selector for each dispatch: it is a small
    // `Copy` enum except for its borrowed source text, and every leg must run
    // the stage that produced its own preceding state carrier.
    let (kind, native_host) = match backend {
        authorization::StageBackend::Metered { .. } => panic!("scalar fixture is unmetered"),
        authorization::StageBackend::Interpreter => (0, None),
        authorization::StageBackend::Native { host } => (1, Some(host)),
        authorization::StageBackend::NativeAtOptimization { host, .. } => (2, Some(host)),
        authorization::StageBackend::Wasm { .. } => (3, None),
        authorization::StageBackend::WasmHeld { .. } => (3, None),
    };
    let select = |kind| match kind {
        0 => authorization::StageBackend::Interpreter,
        1 => native_backend(native_host.expect("native leg retains held host")),
        2 => native_o2_backend(native_host.expect("native leg retains held host")),
        _ => authorization::StageBackend::Wasm { source },
    };

    let observed = returned(
        "observe",
        dispatch(
            select(kind),
            compiled,
            compiled.binding.observe.prepared(),
            std::slice::from_ref(&initialized),
        ),
    );
    let arguments = [
        initialized.clone(),
        RetainedValue::I64(3),
        RetainedValue::Bool(true),
        RetainedValue::Usize(u64::MAX),
    ];
    let decision = returned(
        "authorize",
        dispatch(
            select(kind),
            compiled,
            compiled.binding.authorize.stage().prepared(),
            &arguments,
        ),
    );
    let outcome = payload(&compiled.binding.outcome, b"observed".to_vec(), 4);
    let mut reduce = arguments.to_vec();
    reduce.push(outcome);
    let report = returned(
        "reduce",
        dispatch(
            select(kind),
            compiled,
            compiled.binding.reduce.prepared(),
            &reduce,
        ),
    );
    vec![initialized, observed, decision, report]
}

#[test]
fn all_target_stage_legs_preserve_bool_and_u64_usize_result_leaves() {
    if !native_wasm_tools_available() {
        eprintln!("skipping scalar carrier parity: clang or node unavailable");
        return;
    }
    let native_host = native_stage_host().expect("availability retains native host");
    let source = scalar_source();
    let compiled = compile(&source);
    let expected = drive(authorization::StageBackend::Interpreter, &source, &compiled);
    for (label, backend) in [
        ("native -O0", native_backend(&native_host)),
        ("native -O2", native_o2_backend(&native_host)),
        (
            "Core Wasm",
            authorization::StageBackend::Wasm { source: &source },
        ),
    ] {
        assert_eq!(
            drive(backend, &source, &compiled),
            expected,
            "{label} scalar-rich lifecycle transcript"
        );
    }

    let RetainedValue::Record(report) = expected.last().expect("terminal report") else {
        panic!("reduce returns the Result record");
    };
    assert!(report.fields.iter().any(|field| {
        field.field.as_str() == "fixture.agent.type.result.approved"
            && field.value == RetainedValue::Bool(true)
    }));
    assert!(report.fields.iter().any(|field| {
        field.field.as_str() == "fixture.agent.type.result.count"
            && field.value == RetainedValue::Usize(u64::MAX)
    }));
}

/// Negative `i32` Proposal fields are source-admitted. Exercise the exact
/// signed minimum through authorize and reduce so the Core-Wasm driver and
/// C11 caller cannot silently retain a nonnegative-only scalar vocabulary.
#[test]
fn all_target_stage_legs_preserve_negative_i32_proposal_fields() {
    if !native_wasm_tools_available() {
        eprintln!("skipping negative-i32 proposal parity: clang or node unavailable");
        return;
    }
    let native_host = native_stage_host().expect("availability retains native host");
    let source = i32_proposal_source();
    let compiled = compile(&source);
    let task = payload(&compiled.binding.task, b"i32-minimum".to_vec(), 10);
    let phase = RetainedValue::I32(i32::MIN);

    let drive = |backend| {
        let state = returned(
            "initialize",
            dispatch(
                backend,
                &compiled,
                compiled.binding.initialize.prepared(),
                std::slice::from_ref(&task),
            ),
        );
        let proposal = [
            state.clone(),
            RetainedValue::I64(3),
            RetainedValue::Bool(true),
            RetainedValue::Usize(1),
            phase.clone(),
        ];
        let decision = returned(
            "authorize",
            dispatch(
                backend,
                &compiled,
                compiled.binding.authorize.stage().prepared(),
                &proposal,
            ),
        );
        let mut reduction = proposal.to_vec();
        reduction.push(payload(&compiled.binding.outcome, b"observed".to_vec(), 4));
        let report = returned(
            "reduce",
            dispatch(
                backend,
                &compiled,
                compiled.binding.reduce.prepared(),
                &reduction,
            ),
        );
        vec![state, decision, report]
    };

    let expected = drive(authorization::StageBackend::Interpreter);
    for (label, backend) in [
        ("native -O0", native_backend(&native_host)),
        ("native -O2", native_o2_backend(&native_host)),
        (
            "Core Wasm",
            authorization::StageBackend::Wasm { source: &source },
        ),
    ] {
        assert_eq!(drive(backend), expected, "{label}: negative i32 proposal");
    }
}

#[test]
fn unsupported_target_result_leaf_refuses_before_artifact_execution_and_leaves_healthy_profile_intact(
) {
    if !super::stage_process_host_supported() {
        return;
    }
    let native_host = native_stage_host().expect("native stage test host is available");
    let unsupported = unsupported_source();
    let compiled = compile(&unsupported);
    let task = payload(&compiled.binding.task, b"refuse".to_vec(), 10);
    let state = returned(
        "interpreter initialize",
        dispatch(
            authorization::StageBackend::Interpreter,
            &compiled,
            compiled.binding.initialize.prepared(),
            std::slice::from_ref(&task),
        ),
    );
    let mut reduce = vec![
        state,
        RetainedValue::I64(3),
        RetainedValue::Bool(true),
        RetainedValue::Usize(u64::MAX),
    ];
    reduce.push(payload(&compiled.binding.outcome, b"observed".to_vec(), 4));
    for backend in [
        native_backend(&native_host),
        native_o2_backend(&native_host),
        authorization::StageBackend::Wasm {
            source: &unsupported,
        },
    ] {
        let errors = authorization::dispatch_on(
            backend,
            &compiled.program,
            compiled.binding.reduce.prepared(),
            &reduce,
            DEFAULT_STAGE_STEPS,
        )
        .expect_err("an i32 result leaf is outside the target codec profile");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "SPX-G570");
        assert!(errors[0].message.contains("result.leaf"));
    }

    let healthy_source = scalar_source();
    let healthy = compile(&healthy_source);
    let task = payload(&healthy.binding.task, b"healthy".to_vec(), 10);
    let evaluation = authorization::dispatch_on(
        authorization::StageBackend::Wasm {
            source: &healthy_source,
        },
        &healthy.program,
        healthy.binding.initialize.prepared(),
        std::slice::from_ref(&task),
        DEFAULT_STAGE_STEPS,
    )
    .expect("an earlier refusal cannot corrupt a later legitimate target dispatch");
    assert!(matches!(
        evaluation.outcome,
        RetainedCallOutcome::Returned(_)
    ));
}
