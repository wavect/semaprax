use super::*;
use crate::cleanup_plan::StatusCase;
use std::time::Duration;

const SOURCE: &str = r#"module test.wasm_target_binding;

@id("test.wasm_target_binding.identity")
fn identity(value: i64) -> i64 { value }

@id("test.wasm_target_binding.divide")
fn divide(left: i64, right: i64) -> i64 { left / right }

@id("test.wasm_target_binding.other")
fn other(value: i64) -> i64 { value + 1 }

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn node_host() -> Option<WasmStageHost> {
    std::env::var_os("SEMAPRAX_TEST_WASM_STAGE_NODE")
        .map(std::path::PathBuf::from)
        .into_iter()
        .chain([
            std::path::PathBuf::from("/usr/bin/node"),
            std::path::PathBuf::from("/usr/local/bin/node"),
            std::path::PathBuf::from("/opt/homebrew/bin/node"),
        ])
        .find_map(|path| WasmStageHost::open(&path).ok())
}

fn program() -> hir::ResolvedProgram {
    let checked =
        crate::check(SOURCE, Path::new("wasm-target-binding-test.spx")).expect("fixture checks");
    let program = hir::resolve(&checked).expect("fixture resolves");
    hir::validate(&program).expect("fixture validates");
    program
}

fn prepared(program: &hir::ResolvedProgram) -> PreparedRetainedCall {
    crate::interpreter::retained_call::prepare_retained_call(
        program,
        "test.wasm_target_binding.identity",
    )
    .expect("identity prepares")
}

#[test]
fn target_binding_rejects_source_and_subject_remints_before_any_build_or_node() {
    let program = program();
    let prepared = prepared(&program);
    let entry = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == prepared.function_id())
        .expect("prepared entry belongs to fixture");
    let arguments = [RetainedValue::I64(7)];
    let binding = WasmTargetBinding::bind(SOURCE, &program, entry, &prepared, &arguments, 100)
        .expect("exact checked source binds");
    let selected = vec![entry.id.as_str().to_owned()];
    let descriptor = project::derive_public_api_descriptor(&program, &selected, binding.subject())
        .expect("bound descriptor derives");
    let artifact = binding
        .bind_artifact(&descriptor, &selected)
        .expect("descriptor retains exact target facts");
    artifact
        .verify_invocations(&selected)
        .expect("the selected export is exactly the invocation");
    let mut wrong_digest = artifact.clone();
    wrong_digest.descriptor_digest =
        "sha256:0000000000000000000000000000000000000000000000000000000000000000".to_owned();
    let error = wrong_digest
        .verify_descriptor(&descriptor)
        .expect_err("a descriptor-digest remint cannot reach the carrier verifier");
    assert_eq!(error.code, "SPX-G570");
    assert!(error
        .message
        .contains("wasm_executor.binding.descriptor_drift"));

    let drifted = SOURCE.replace("{ value }", "{ value + 2 }");
    let error = WasmTargetBinding::bind(&drifted, &program, entry, &prepared, &arguments, 100)
        .expect_err("a same-id source remint cannot bind the original program");
    assert_eq!(error.code, "SPX-G570");
    assert!(error.message.contains("wasm_executor.binding.lifecycle"));

    for subject in [
        project::PublicApiSubject {
            project_schema: project::PUBLIC_OWNED_DATA_PROJECT_SCHEMA,
            project_revision:
                "sha256:0000000000000000000000000000000000000000000000000000000000000000",
            workspace_revision: &binding.lifecycle_identity,
            project_graph_digest: &binding.invocation_identity,
        },
        project::PublicApiSubject {
            project_schema: project::PUBLIC_OWNED_DATA_PROJECT_SCHEMA,
            project_revision: &binding.source_revision,
            workspace_revision:
                "sha256:0000000000000000000000000000000000000000000000000000000000000000",
            project_graph_digest: &binding.invocation_identity,
        },
        project::PublicApiSubject {
            project_schema: project::PUBLIC_OWNED_DATA_PROJECT_SCHEMA,
            project_revision: &binding.source_revision,
            workspace_revision: &binding.lifecycle_identity,
            project_graph_digest:
                "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        },
    ] {
        let reminted = project::derive_public_api_descriptor(&program, &selected, subject)
            .expect("a syntactically valid but differently bound descriptor derives");
        let error = binding
            .bind_artifact(&reminted, &selected)
            .expect_err("a reminted descriptor subject is never target-authenticated");
        assert_eq!(error.code, "SPX-G570");
        assert!(error.message.contains("wasm_executor.binding.descriptor"));
    }

    let changed_arguments = [RetainedValue::I64(8)];
    let changed =
        WasmTargetBinding::bind(SOURCE, &program, entry, &prepared, &changed_arguments, 100)
            .expect("a distinct legitimate invocation binds separately");
    assert_ne!(binding.invocation_identity, changed.invocation_identity);
    let error = artifact
        .verify_invocations(&["test.wasm_target_binding.other".to_owned()])
        .expect_err("a selected-export remint cannot invoke a different function");
    assert_eq!(error.code, "SPX-G570");
    assert!(error.message.contains("wasm_executor.binding.invocation"));
}

#[test]
fn structured_outcome_rejects_malformed_and_mismatched_status_before_publication() {
    let ok = "{\"schema\":\"semaprax.agent-wasm-stage-outcome.v2\",\"kind\":\"returned\",\"value\":\"7\"}";
    let malformed_tail = format!("{ok}\n{{\"schema\":false}}\n");
    assert!(decode_node_outcomes(&malformed_tail, 2).is_err());

    let mismatch = "{\"schema\":\"semaprax.agent-wasm-stage-outcome.v2\",\"kind\":\"language_failure\",\"raw_status\":4,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.arithmetic.v1\",\"code\":5,\"class\":\"arithmetic\",\"retryable\":false}}\n";
    let error = decode_node_outcomes(mismatch, 1)
        .expect_err("raw and normalized status disagreement must not publish an outcome");
    assert!(error
        .message
        .contains("wasm_executor.outcome.status_mismatch"));

    let failure = "{\"schema\":\"semaprax.agent-wasm-stage-outcome.v2\",\"kind\":\"language_failure\",\"raw_status\":9,\"status\":{\"schema\":\"semaprax.status.v1\",\"domain_id\":\"semaprax.contract.v1\",\"code\":1,\"class\":\"contract\",\"retryable\":false}}";
    assert!(decode_node_outcomes(&format!("{failure}\n{ok}\n"), 2).is_err());
}

#[test]
fn owned_bytes_receipt_rejects_forged_omitted_and_duplicate_rows() {
    let tagged = "{\"schema\":\"semaprax.agent-wasm-stage-outcome.v2\",\"kind\":\"settled_owned_bytes\",\"value\":\"00ff\",\"byte_length\":2}";
    let scalar = "{\"schema\":\"semaprax.agent-wasm-stage-outcome.v2\",\"kind\":\"returned\",\"value\":\"00ff\"}";
    let NodeStageRun::Returned(mut values) =
        decode_node_outcomes(&format!("{tagged}\n"), 1).unwrap()
    else {
        panic!("owned observation must return")
    };
    let observed = values.pop().unwrap();
    observed.require_projection(true).unwrap();
    assert!(observed.require_projection(false).is_err());
    let NodeStageRun::Returned(mut omitted) =
        decode_node_outcomes(&format!("{scalar}\n"), 1).unwrap()
    else {
        panic!("scalar observation must return")
    };
    assert!(omitted.pop().unwrap().require_projection(true).is_err());
    assert!(decode_node_outcomes(&format!("{tagged}\n{tagged}\n"), 1).is_err());
    let NodeStageRun::Returned(duplicated) =
        decode_node_outcomes(&format!("{tagged}\n{tagged}\n"), 2).unwrap()
    else {
        panic!("two rows must decode before projection binding")
    };
    duplicated[0].require_projection(true).unwrap();
    assert!(duplicated[1].require_projection(false).is_err());
    for forged in [
        tagged.replace("\"byte_length\":2", "\"byte_length\":1"),
        tagged.replace("\"value\":\"00ff\"", "\"value\":\"00FG\""),
        tagged.replace("\"byte_length\":2", "\"byte_length\":2,\"settled\":true"),
    ] {
        assert!(decode_node_outcomes(&format!("{forged}\n"), 1).is_err());
    }
}

#[test]
fn wasm_record_owned_bytes_copy_out_is_observed_only_after_facade_settlement() {
    let Some(host) = node_host() else { return };
    const OWNED_SOURCE: &str = r#"module test.wasm_owned_receipt;
@id("test.wasm_owned_receipt.Output")
record Output {
    @id("test.wasm_owned_receipt.Output.first") first: Bytes,
    @id("test.wasm_owned_receipt.Output.marker") marker: i64,
    @id("test.wasm_owned_receipt.Output.second") second: Bytes,
}
@id("test.wasm_owned_receipt.make")
fn make(divisor: i64) -> Output {
    let first_seed = [0u8, 255u8];
    let second_seed = [7u8];
    Output {
        first: bytes_copy(array_as_slice(first_seed)),
        marker: 10 / divisor,
        second: bytes_copy(array_as_slice(second_seed)),
    }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let checked = crate::check(OWNED_SOURCE, Path::new("wasm-owned-receipt.spx")).unwrap();
    let program = hir::resolve(&checked).unwrap();
    hir::validate(&program).unwrap();
    let prepared = crate::interpreter::retained_call::prepare_retained_call(
        &program,
        "test.wasm_owned_receipt.make",
    )
    .unwrap();
    let actual = run(
        &host,
        OWNED_SOURCE,
        &program,
        &prepared,
        &[RetainedValue::I64(2)],
        100,
    )
    .unwrap();
    let expected = crate::interpreter::retained_call::evaluate_retained_call(
        &program,
        &prepared,
        &[RetainedValue::I64(2)],
        100,
    )
    .unwrap();
    assert_eq!(actual.outcome, expected.outcome);
    assert_eq!(actual.cleanup_events, expected.cleanup_events);
    assert_eq!(actual.cleanup_events.len(), 2);
    assert_eq!(
        actual.steps_used, 0,
        "Wasm instruction fuel remains unmeasured"
    );
    let failed = run(
        &host,
        OWNED_SOURCE,
        &program,
        &prepared,
        &[RetainedValue::I64(0)],
        100,
    )
    .unwrap();
    assert!(matches!(
        failed.outcome,
        RetainedCallOutcome::LanguageFailure(_)
    ));
    assert!(failed.cleanup_events.is_empty());
}

#[test]
fn direct_and_injected_paths_refuse_cancelled_or_invalid_budget_before_target_work() {
    let Some(host) = node_host() else { return };
    const INJECTED_SOURCE: &str = r#"module test.wasm_target_binding.admission;

@id("test.wasm_target_binding.admission.result")
record StageResult {
    @id("test.wasm_target_binding.admission.result.value") value: i64,
}

@id("test.wasm_target_binding.admission.wrap")
fn wrap(value: i64) -> StageResult { StageResult { value: value } }

@id("app.main")
fn main() -> i64 { 0 }
"#;

    let direct_program = program();
    let direct_prepared = prepared(&direct_program);
    let checked = crate::check(
        INJECTED_SOURCE,
        Path::new("wasm-target-process-admission-test.spx"),
    )
    .unwrap();
    let injected_program = hir::resolve(&checked).unwrap();
    hir::validate(&injected_program).unwrap();
    let injected_prepared = crate::interpreter::retained_call::prepare_retained_call(
        &injected_program,
        "test.wasm_target_binding.admission.wrap",
    )
    .unwrap();
    let cancellation = AgentCancellation::new();
    cancellation.cancel();

    for (source, program, prepared) in [
        (SOURCE, &direct_program, &direct_prepared),
        (INJECTED_SOURCE, &injected_program, &injected_prepared),
    ] {
        let cancelled = run_admitted(
            Some(&host),
            source,
            program,
            prepared,
            &[RetainedValue::I64(7)],
            100,
            Some(&cancellation),
        )
        .expect_err("cancellation must win before direct or injected target construction");
        assert!(cancelled
            .message
            .contains("wasm_executor.process.cancelled"));

        for max_steps in [0, 1_000_001] {
            let budget = run_admitted(
                Some(&host),
                source,
                program,
                prepared,
                &[RetainedValue::I64(7)],
                max_steps,
                None,
            )
            .expect_err("invalid stage budget must win before target construction");
            assert!(budget.message.contains("wasm_executor.max_steps"));
        }
    }
}

#[test]
fn node_process_cancellation_and_output_overflow_kill_reap_and_fail_closed() {
    let Some(host) = node_host() else { return };
    let mut workspace = WasmStageWorkspace::create().unwrap();
    workspace
        .write(Path::new("observe.mjs"), b"setInterval(() => {}, 1000);\n")
        .unwrap();
    let cancellation = AgentCancellation::new();
    let trigger = cancellation.clone();
    let canceller = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(25));
        trigger.cancel();
    });
    let cancelled = run_node_process(&host, &workspace, Some(&cancellation), 64)
        .expect_err("an in-flight local target must be killed and reaped on cancellation");
    canceller.join().unwrap();
    assert!(cancelled
        .message
        .contains("wasm_executor.process.cancelled"));

    workspace.cleanup().unwrap();
    let mut workspace = WasmStageWorkspace::create().unwrap();
    workspace
        .write(
            Path::new("observe.mjs"),
            b"const chunk = 'x'.repeat(4096); while (true) process.stdout.write(chunk);\n",
        )
        .unwrap();
    let overflow = run_node_process(&host, &workspace, None, 32)
        .expect_err("bounded capture must reject rather than retain oversized target output");
    assert!(overflow
        .message
        .contains("wasm_executor.process.output_budget"));
    workspace.cleanup().unwrap();
}

#[test]
fn node_envelope_preserves_compiler_owned_failure_and_fresh_process_recovers() {
    let Some(host) = node_host() else { return };
    let program = program();
    let prepared = crate::interpreter::retained_call::prepare_retained_call(
        &program,
        "test.wasm_target_binding.divide",
    )
    .expect("divide function prepares");
    let failed = run(
        &host,
        SOURCE,
        &program,
        &prepared,
        &[RetainedValue::I64(7), RetainedValue::I64(0)],
        100,
    )
    .expect("a checked language failure is an execution outcome");
    assert_eq!(
        failed.outcome,
        RetainedCallOutcome::LanguageFailure(crate::runtime_status::normalize_arithmetic(
            StatusCase::DivisionByZero
        ))
    );
    let healthy = run(
        &host,
        SOURCE,
        &program,
        &prepared,
        &[RetainedValue::I64(14), RetainedValue::I64(2)],
        100,
    )
    .expect("a fresh local target remains usable after the settled failure");
    assert_eq!(
        healthy.outcome,
        RetainedCallOutcome::Returned(RetainedValue::I64(7))
    );
}

#[test]
fn injected_aggregate_stops_after_one_checked_failure_before_arena_reuse() {
    let Some(host) = node_host() else { return };
    const AGGREGATE_SOURCE: &str = r#"module test.wasm_target_binding.aggregate;

@id("test.wasm_target_binding.aggregate.pair")
record Pair {
    @id("test.wasm_target_binding.aggregate.pair.left") left: i64,
    @id("test.wasm_target_binding.aggregate.pair.right") right: i64,
}

@id("test.wasm_target_binding.aggregate.divide")
fn divide(value: i64, divisor: i64) -> Pair {
    Pair { left: value / divisor, right: value }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;
    let checked = crate::check(
        AGGREGATE_SOURCE,
        Path::new("wasm-target-binding-aggregate.spx"),
    )
    .unwrap();
    let program = hir::resolve(&checked).unwrap();
    hir::validate(&program).unwrap();
    let prepared = crate::interpreter::retained_call::prepare_retained_call(
        &program,
        "test.wasm_target_binding.aggregate.divide",
    )
    .expect("aggregate divide prepares");
    let failed = run(
        &host,
        AGGREGATE_SOURCE,
        &program,
        &prepared,
        &[RetainedValue::I64(7), RetainedValue::I64(0)],
        100,
    )
    .expect("one checked failure settles the whole aggregate projection");
    assert_eq!(
        failed.outcome,
        RetainedCallOutcome::LanguageFailure(crate::runtime_status::normalize_arithmetic(
            StatusCase::DivisionByZero
        ))
    );
}

#[test]
fn scalar_output_reservation_covers_every_emitted_status_and_preserves_fixed_cap() {
    const SCHEMA: &str = "semaprax.agent-wasm-stage-outcome.v2";
    let mut rows = [
        i64::MIN.to_string(),
        i64::MAX.to_string(),
        u64::MAX.to_string(),
        "true".into(),
        "false".into(),
    ]
    .map(|value| serde_json::json!({"schema": SCHEMA, "kind": "returned", "value": value}))
    .to_vec();
    rows.push(serde_json::json!({"schema": SCHEMA, "kind": "call_depth_exceeded"}));
    for raw in 1..=10 {
        let arithmetic = raw <= 8;
        rows.push(serde_json::json!({"schema": SCHEMA, "kind": "language_failure", "raw_status": raw,
            "status": {"schema": "semaprax.status.v1", "domain_id": if arithmetic { "semaprax.arithmetic.v1" } else { "semaprax.contract.v1" },
            "code": if arithmetic { raw } else { raw - 8 }, "class": if arithmetic { "arithmetic" } else { "contract" }, "retryable": false}
        }));
    }
    for row in rows {
        let rendered = format!("{row}\n");
        assert!(
            rendered.len() <= MAX_NODE_SCALAR_OUTCOME_ROW_BYTES,
            "{rendered}"
        );
        decode_node_outcomes(&rendered, 1).expect("bounded row must retain exact parser admission");
    }
    assert_eq!(MAX_NODE_SCALAR_OUTCOME_ROW_BYTES, 512);
    assert_eq!(MAX_NODE_OUTCOME_ROW_BYTES, 4096);
    assert_eq!(MAX_SEMANTIC_ROW_BYTES, 2048);
    assert_eq!(node_output_budget(1, 0, false).unwrap(), 4096);
    assert_eq!(node_output_budget(1, 0, true).unwrap(), 512 + 2048);
    assert_eq!(node_output_budget(1, 1, true).unwrap(), 4096 + 2048);
    assert_eq!(
        node_output_budget(11, 3, true).unwrap(),
        8 * 512 + 3 * 4096 + 11 * 2048
    );
    for (calls, bytes) in [
        (0, 0),
        (1, 2),
        (usize::MAX, 0),
        (usize::MAX, usize::MAX),
        (16, 16),
    ] {
        assert!(node_output_budget(calls, bytes, true).is_err());
    }
    assert_eq!(
        node_output_budget(MAX_NODE_STDOUT_BYTES / 4096, 0, false).unwrap(),
        MAX_NODE_STDOUT_BYTES / 4096 * 4096
    );
    assert!(node_output_budget(MAX_NODE_STDOUT_BYTES / 4096 + 1, 0, false).is_err());
}
