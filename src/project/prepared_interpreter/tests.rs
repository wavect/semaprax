use std::collections::BTreeMap;
use std::ops::Range;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use sha2::{Digest as _, Sha256};

use crate::{interpreter, project::ProjectRevision};

use super::*;

const TRACE_PAYLOAD_DOMAIN: &[u8] = b"semaprax.project-source-trace.payload.v1\0";
static REAL_PREPARE_SERIAL: Mutex<()> = Mutex::new(());

fn real_prepare_serial() -> MutexGuard<'static, ()> {
    REAL_PREPARE_SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn revision() -> Arc<ProjectRevision> {
    let manifest =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project/semaprax.toml");
    crate::project::load_snapshot(&manifest)
        .expect("calculator Project must load")
        .retain_revision()
}

fn remint_payload(envelope: &str, mutate: impl FnOnce(&str) -> String) -> String {
    let (_, payload_and_close) = envelope
        .split_once(",\"payload\":")
        .expect("trace wrapper must carry payload");
    let payload = payload_and_close
        .strip_suffix('}')
        .expect("trace wrapper must close exactly once");
    let payload = mutate(payload);
    let mut hasher = Sha256::new();
    hasher.update(TRACE_PAYLOAD_DOMAIN);
    hasher.update((payload.len() as u64).to_le_bytes());
    hasher.update(payload.as_bytes());
    let digest = format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    );
    format!(
        "{{\"schema\":{},\"digest\":{},\"bytes\":{},\"payload\":{}}}",
        crate::diagnostic::quote_json(PROJECT_SOURCE_TRACE_SCHEMA),
        crate::diagnostic::quote_json(&digest),
        payload.len(),
        payload,
    )
}

fn first_event_range(payload: &str) -> Range<usize> {
    let start = payload
        .find("\"events\":[{")
        .expect("trace must carry at least one event")
        + "\"events\":[".len();
    let mut depth = 0usize;
    let mut quoted = false;
    let mut escaped = false;
    for (offset, byte) in payload.as_bytes()[start..].iter().copied().enumerate() {
        if quoted {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                quoted = false;
            }
            continue;
        }
        match byte {
            b'"' => quoted = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return start..start + offset + 1;
                }
            }
            _ => {}
        }
    }
    panic!("first trace event must be a complete object")
}

#[test]
fn one_prepared_worker_repeats_exact_entry_and_test_with_replayable_origins() {
    let _serial = real_prepare_serial();
    let revision = revision();
    let prepared = revision
        .prepare_interpreter(PreparedProjectInterpreterOptions::default())
        .unwrap();
    let cancellation = ProjectExecutionCancellation::new();
    let options = PreparedProjectExecutionOptions::default();
    let first = prepared.execute_entry(&options, &cancellation).unwrap();
    let second = prepared.execute_entry(&options, &cancellation).unwrap();
    assert_eq!(
        first.outcome(),
        &ProjectPreparedExecutionOutcome::Returned(42)
    );
    assert_eq!(first.trace().envelope(), second.trace().envelope());
    verify_project_source_trace(first.trace().envelope()).unwrap();
    verify_project_source_trace_against_revision(&revision, first.trace().envelope()).unwrap();

    let test = prepared.execute_test(&options, &cancellation).unwrap();
    assert_eq!(
        test.outcome(),
        &ProjectPreparedExecutionOutcome::Returned(0)
    );
    verify_project_source_trace_against_revision(&revision, test.trace().envelope()).unwrap();
}

#[test]
fn cancellation_is_a_replayable_zero_step_outcome_and_trace_saturation_is_explicit() {
    let _serial = real_prepare_serial();
    let revision = revision();
    let prepared = revision
        .prepare_interpreter(PreparedProjectInterpreterOptions::default())
        .unwrap();
    let cancellation = ProjectExecutionCancellation::new();
    cancellation.cancel();
    let cancelled = prepared
        .execute_entry(&PreparedProjectExecutionOptions::default(), &cancellation)
        .unwrap();
    assert_eq!(
        cancelled.outcome(),
        &ProjectPreparedExecutionOutcome::Cancelled { before_step: 1 }
    );
    assert_eq!(cancelled.steps_used(), 0);
    assert_eq!(cancelled.trace().recorded_events(), 0);
    verify_project_source_trace_against_revision(&revision, cancelled.trace().envelope()).unwrap();

    let one_event = PreparedProjectExecutionOptions::new(
        interpreter::DEFAULT_MAX_STEPS,
        DEFAULT_PROJECT_SOURCE_TRACE_BYTES,
        1,
    )
    .unwrap();
    let completed = prepared
        .execute_entry(&one_event, &ProjectExecutionCancellation::new())
        .unwrap();
    assert_eq!(completed.trace().recorded_events(), 1);
    assert!(completed.trace().truncated());
}

#[test]
fn options_and_resigned_origin_mutation_fail_closed() {
    let _serial = real_prepare_serial();
    assert_eq!(
        PreparedProjectExecutionOptions::new(0, MIN_PROJECT_SOURCE_TRACE_BYTES, 1)
            .unwrap_err()
            .code,
        "SPX-F108"
    );
    let revision = revision();
    let prepared = revision
        .prepare_interpreter(PreparedProjectInterpreterOptions::default())
        .unwrap();
    let execution = prepared
        .execute_entry(
            &PreparedProjectExecutionOptions::default(),
            &ProjectExecutionCancellation::new(),
        )
        .unwrap();
    let hostile = execution
        .trace()
        .envelope()
        .replacen("src/app.spx", "src/bad.spx", 1);
    assert_eq!(
        verify_project_source_trace(&hostile).unwrap_err().code,
        "SPX-F110"
    );
}

#[test]
fn worker_permit_is_bounded_and_released_exactly_once() {
    static ACTIVE: AtomicUsize = AtomicUsize::new(0);
    let first = PreparedWorkerPermit::acquire(&ACTIVE, 2).unwrap();
    let second = PreparedWorkerPermit::acquire(&ACTIVE, 2).unwrap();
    assert_eq!(
        PreparedWorkerPermit::acquire(&ACTIVE, 2)
            .unwrap_err()
            .first()
            .unwrap()
            .code,
        "SPX-F107"
    );
    drop(first);
    let replacement = PreparedWorkerPermit::acquire(&ACTIVE, 2).unwrap();
    drop(second);
    drop(replacement);
    assert_eq!(ACTIVE.load(Ordering::Acquire), 0);
}

#[test]
fn production_worker_bound_is_exact_and_real_workers_release_their_permits() {
    let _serial = real_prepare_serial();
    assert_eq!(
        ACTIVE_PREPARED_PROJECT_INTERPRETER_WORKERS.load(Ordering::Acquire),
        0
    );
    let revision = revision();
    let mut workers = Vec::new();
    for _ in 0..MAX_PREPARED_PROJECT_INTERPRETER_WORKERS {
        workers.push(
            prepare_project_interpreter(
                Arc::clone(&revision),
                PreparedProjectInterpreterOptions::default(),
            )
            .unwrap(),
        );
    }
    assert_eq!(
        ACTIVE_PREPARED_PROJECT_INTERPRETER_WORKERS.load(Ordering::Acquire),
        MAX_PREPARED_PROJECT_INTERPRETER_WORKERS
    );
    let refusal = match prepare_project_interpreter(
        Arc::clone(&revision),
        PreparedProjectInterpreterOptions::default(),
    ) {
        Ok(worker) => {
            drop(worker);
            panic!("the ninth prepared worker must be rejected")
        }
        Err(diagnostics) => diagnostics,
    };
    assert_eq!(refusal.first().unwrap().code, "SPX-F107");

    drop(workers);
    assert_eq!(
        ACTIVE_PREPARED_PROJECT_INTERPRETER_WORKERS.load(Ordering::Acquire),
        0
    );
    let replacement =
        prepare_project_interpreter(revision, PreparedProjectInterpreterOptions::default())
            .unwrap();
    drop(replacement);
    assert_eq!(
        ACTIVE_PREPARED_PROJECT_INTERPRETER_WORKERS.load(Ordering::Acquire),
        0
    );
}

#[test]
fn execution_admission_rejects_concurrent_work_and_reopens_after_release() {
    let executing = AtomicBool::new(false);
    let first = ExecutionAdmission::acquire(&executing).unwrap();
    assert_eq!(
        ExecutionAdmission::acquire(&executing)
            .unwrap_err()
            .first()
            .unwrap()
            .code,
        "SPX-F109"
    );
    drop(first);
    drop(ExecutionAdmission::acquire(&executing).unwrap());
}

#[test]
fn duplicate_function_origin_must_be_exact_and_is_counted_once() {
    let origin = FunctionOrigin {
        path: "src/app.spx".to_owned(),
        source_revision: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            .to_owned(),
        source_digest: "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
            .to_owned(),
        source_bytes: 128,
    };
    let mut origins = BTreeMap::new();
    let mut bytes = 0;
    insert_origin(&mut origins, "app.main", origin.clone(), &mut bytes).unwrap();
    let exact_bytes = bytes;
    insert_origin(&mut origins, "app.main", origin.clone(), &mut bytes).unwrap();
    assert_eq!(bytes, exact_bytes);

    let mut drifted = origin;
    drifted.source_bytes += 1;
    assert_eq!(
        insert_origin(&mut origins, "app.main", drifted, &mut bytes)
            .unwrap_err()
            .first()
            .unwrap()
            .code,
        "SPX-F107"
    );
}

#[test]
fn canonical_remints_cannot_change_structural_phase_or_escape_the_exact_closure() {
    let _serial = real_prepare_serial();
    let revision = revision();
    let prepared = revision
        .prepare_interpreter(PreparedProjectInterpreterOptions::default())
        .unwrap();
    let execution = prepared
        .execute_entry(
            &PreparedProjectExecutionOptions::default(),
            &ProjectExecutionCancellation::new(),
        )
        .unwrap();

    let wrong_phase = remint_payload(execution.trace().envelope(), |payload| {
        payload.replacen("\"phase\":\"body\"", "\"phase\":\"ensures\"", 1)
    });
    verify_project_source_trace(&wrong_phase).unwrap();
    assert_eq!(
        verify_project_source_trace_against_revision(&revision, &wrong_phase)
            .unwrap_err()
            .code,
        "SPX-F110"
    );

    let target = revision
        .entry_program()
        .functions
        .iter()
        .find(|function| function.id.as_str() == "calculator.is-negative")
        .expect("fixture must retain one entry-unreachable export");
    let source_path = &revision
        .semantic
        .rename_function(target.id.as_str())
        .expect("fixture export must have a semantic origin")
        .path;
    let source = revision
        .sources()
        .iter()
        .find(|source| source.path() == source_path)
        .expect("fixture export source must be retained");
    let root: serde_json::Value = serde_json::from_str(execution.trace().envelope()).unwrap();
    let original = &root["payload"]["events"][0];
    let replacement = format!(
        "{{\"index\":0,\"step\":{},\"depth\":{},\"phase\":\"body\",\"function_id\":{},\"expression_id\":{},\"source\":{{\"path\":{},\"revision\":{},\"digest\":{}}},\"span\":{{\"start\":{},\"end\":{},\"line\":{},\"column\":{}}}}}",
        original["step"].as_u64().unwrap(),
        original["depth"].as_u64().unwrap(),
        crate::diagnostic::quote_json(target.id.as_str()),
        crate::diagnostic::quote_json(target.body.id.as_str()),
        crate::diagnostic::quote_json(source.path()),
        crate::diagnostic::quote_json(source.source_revision()),
        crate::diagnostic::quote_json(source.source_digest()),
        target.body.span.start,
        target.body.span.end,
        target.body.span.line,
        target.body.span.column,
    );
    let unreachable = remint_payload(execution.trace().envelope(), |payload| {
        let range = first_event_range(payload);
        format!(
            "{}{}{}",
            &payload[..range.start],
            replacement,
            &payload[range.end..]
        )
    });
    verify_project_source_trace(&unreachable).unwrap();
    assert_eq!(
        verify_project_source_trace_against_revision(&revision, &unreachable)
            .unwrap_err()
            .code,
        "SPX-F110"
    );
}

#[test]
fn canonical_remints_reject_impossible_cancellation_and_drop_accounting() {
    let _serial = real_prepare_serial();
    let revision = revision();
    let prepared = revision
        .prepare_interpreter(PreparedProjectInterpreterOptions::default())
        .unwrap();
    let cancellation = ProjectExecutionCancellation::new();
    cancellation.cancel();
    let execution = prepared
        .execute_entry(&PreparedProjectExecutionOptions::default(), &cancellation)
        .unwrap();

    let exhausted_cancellation = remint_payload(execution.trace().envelope(), |payload| {
        payload
            .replacen("\"steps_used\":0", "\"steps_used\":1000000", 1)
            .replacen("\"before_step\":1", "\"before_step\":1000001", 1)
    });
    assert_eq!(
        verify_project_source_trace(&exhausted_cancellation)
            .unwrap_err()
            .code,
        "SPX-F110"
    );

    let impossible_drop = remint_payload(execution.trace().envelope(), |payload| {
        payload.replacen(
            "\"recorded_events\":0,\"dropped_events\":0,\"truncated\":false",
            "\"recorded_events\":0,\"dropped_events\":1,\"truncated\":true",
            1,
        )
    });
    assert_eq!(
        verify_project_source_trace(&impossible_drop)
            .unwrap_err()
            .code,
        "SPX-F110"
    );
}

#[test]
fn byte_range_language_status_is_in_the_exact_trace_vocabulary() {
    let status = crate::conformance::NormalizedStatus::try_new(
        crate::byte_ops::RANGE_STATUS_DOMAIN,
        crate::byte_ops::RANGE_END_OUT_OF_BOUNDS_CODE,
        crate::conformance::StatusClass::Adapter,
        crate::conformance::Retryability::Known(false),
    )
    .unwrap();
    let value = serde_json::from_str(&status.to_json()).unwrap();
    assert_eq!(trace::parse_status(&value).unwrap(), status);
}
