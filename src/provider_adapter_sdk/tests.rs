//! End-to-end conformance tests: issue #181's own required evidence list,
//! one test per named criterion, each paired with the negative control that
//! proves the positive test was not vacuous.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use super::adapter::{AdapterEvent, AdapterInvocationCapability, AdapterRequest};
use super::capability::{RequiredCapabilities, StructuredOutputMode};
use super::conformance::{
    run_conformance_suite, DriveOutcome, ExpectedOutcome, CASE_CAPABILITY_NEGOTIATION,
    CASE_DRIVE_TO_SETTLEMENT, DRIVE_CONTRADICTORY_USAGE, DRIVE_DUPLICATE_COMPLETION,
    DRIVE_EVENT_AFTER_COMPLETION, DRIVE_LATE_AFTER_CANCEL, DRIVE_OVERSIZED,
};
use super::fixture_adapters::{
    usage, PanicsOnStartAdapter, RecordedReplayAdapter, ScriptedBatchAdapter,
    ScriptedStreamingAdapter,
};
use super::hostile;
use crate::live_invocation::model_invoke::ModelFailure;

fn cap() -> AdapterInvocationCapability {
    AdapterInvocationCapability::grant("provider-adapter-sdk-conformance-test")
}

fn permissive_requirement() -> RequiredCapabilities {
    RequiredCapabilities {
        require_streaming: false,
        require_structured_output_mode: None,
        max_request_bytes: 0,
        max_response_bytes: 0,
    }
}

fn request(max_response_bytes: usize) -> AdapterRequest {
    AdapterRequest {
        request_bytes: b"do-the-thing".to_vec(),
        max_response_bytes,
    }
}

// ---------------------------------------------------------------------
// "At least two provider adapters pass the same core conformance corpus."
// Materially different transport shapes (single-shot vs streaming), both
// offline fixtures — no live network, no real provider, no key, exactly as
// this task's scope requires.
// ---------------------------------------------------------------------

#[test]
fn a_non_streaming_batch_adapter_passes_the_full_conformance_suite() {
    let mut adapter = ScriptedBatchAdapter::new(b"the-answer".to_vec(), usage(10, 5, 100));
    let report = run_conformance_suite(
        &mut adapter,
        &cap(),
        &permissive_requirement(),
        &request(4096),
        None,
        &ExpectedOutcome::Settled {
            response_bytes: b"the-answer".to_vec(),
        },
    );
    assert!(report.all_passed(), "report: {report:?}");
    assert_eq!(report.cases.len(), 2);
}

#[test]
fn a_streaming_adapter_passes_the_same_conformance_suite() {
    let mut adapter = ScriptedStreamingAdapter::new(
        vec![b"the-".to_vec(), b"answer".to_vec()],
        b"the-answer".to_vec(),
        usage(10, 5, 100),
        true,
    );
    let mut required = permissive_requirement();
    required.require_streaming = true;
    let report = run_conformance_suite(
        &mut adapter,
        &cap(),
        &required,
        &request(4096),
        None,
        &ExpectedOutcome::Settled {
            response_bytes: b"the-answer".to_vec(),
        },
    );
    assert!(report.all_passed(), "report: {report:?}");
}

// ---------------------------------------------------------------------
// "Deterministic: the same adapter and same fixture inputs must produce
// byte-identical conformance results across runs." Also covers
// "Recording/replay reproduces event sequence and decoded Proposal without
// dispatch": two independently constructed replay adapters from the same
// recording, run through the suite twice, must agree byte for byte.
// ---------------------------------------------------------------------

fn sample_recording() -> (Vec<AdapterEvent>, super::adapter::AdapterSettlement) {
    (
        vec![
            AdapterEvent::Delta(b"re".to_vec()),
            AdapterEvent::Delta(b"corded".to_vec()),
            AdapterEvent::Usage {
                tokens_in: 4,
                tokens_out: 2,
                cost_micros: 10,
            },
            AdapterEvent::Completed,
        ],
        super::adapter::AdapterSettlement {
            response_bytes: b"recorded".to_vec(),
            usage: usage(4, 2, 10),
        },
    )
}

#[test]
fn the_same_recording_replayed_twice_produces_byte_identical_conformance_reports() {
    let caps_for = || super::fixture_adapters::base_capabilities("recorded-replay-adapter", true);
    let (events_a, settlement_a) = sample_recording();
    let (events_b, settlement_b) = sample_recording();
    let mut adapter_a = RecordedReplayAdapter::from_recording(caps_for(), events_a, settlement_a);
    let mut adapter_b = RecordedReplayAdapter::from_recording(caps_for(), events_b, settlement_b);

    let mut required = permissive_requirement();
    required.require_streaming = true;
    let expected = ExpectedOutcome::Settled {
        response_bytes: b"recorded".to_vec(),
    };

    let report_a = run_conformance_suite(
        &mut adapter_a,
        &cap(),
        &required,
        &request(4096),
        None,
        &expected,
    );
    let report_b = run_conformance_suite(
        &mut adapter_b,
        &cap(),
        &required,
        &request(4096),
        None,
        &expected,
    );

    assert!(report_a.all_passed());
    assert_eq!(
        report_a.render(),
        report_b.render(),
        "two independent replay instances of the same recording must produce byte-identical reports"
    );
    assert_eq!(report_a.digest(), report_b.digest());
    assert_eq!(adapter_a.start_calls(), 1);
    assert_eq!(adapter_b.start_calls(), 1);
}

// ---------------------------------------------------------------------
// "Capability negotiation rejects unsupported combinations before
// dispatch." `PanicsOnStartAdapter` proves "before dispatch" directly:
// if the suite ever called `start`, this test would fail on the panic,
// not on an assertion.
// ---------------------------------------------------------------------

#[test]
fn negotiation_refuses_a_streaming_requirement_the_adapter_does_not_support_before_any_dispatch() {
    let caps = super::fixture_adapters::base_capabilities("no-streaming-adapter", false);
    let mut adapter = PanicsOnStartAdapter::new(caps);
    let mut required = permissive_requirement();
    required.require_streaming = true;

    let report = run_conformance_suite(
        &mut adapter,
        &cap(),
        &required,
        &request(4096),
        None,
        &ExpectedOutcome::Settled {
            response_bytes: Vec::new(),
        },
    );

    assert!(!report.all_passed());
    assert_eq!(
        report.cases.len(),
        1,
        "no further case runs once negotiation refuses"
    );
    let negotiation = report.case(CASE_CAPABILITY_NEGOTIATION).unwrap();
    assert!(!negotiation.passed);
    assert!(negotiation.detail.contains("StreamingNotSupported"));
}

#[test]
fn negotiation_refuses_an_adapter_declared_ambient_endpoint_unconditionally() {
    let mut adapter = PanicsOnStartAdapter::new(hostile::ambient_endpoint_capabilities());
    let report = run_conformance_suite(
        &mut adapter,
        &cap(),
        &permissive_requirement(),
        &request(4096),
        None,
        &ExpectedOutcome::Settled {
            response_bytes: Vec::new(),
        },
    );
    assert!(!report.all_passed());
    let negotiation = report.case(CASE_CAPABILITY_NEGOTIATION).unwrap();
    assert!(negotiation.detail.contains("AmbientEndpointDeclared"));
}

#[test]
fn negotiation_refuses_an_adapter_that_declares_an_unsafe_class_as_retryable() {
    let mut adapter = PanicsOnStartAdapter::new(hostile::unsafe_retryable_capabilities());
    let report = run_conformance_suite(
        &mut adapter,
        &cap(),
        &permissive_requirement(),
        &request(4096),
        None,
        &ExpectedOutcome::Settled {
            response_bytes: Vec::new(),
        },
    );
    assert!(!report.all_passed());
    let negotiation = report.case(CASE_CAPABILITY_NEGOTIATION).unwrap();
    assert!(negotiation.detail.contains("UnsafeRetryableClassDeclared"));
}

// ---------------------------------------------------------------------
// "Write at least one deliberately broken adapter and show the suite
// rejecting it, with the specific conformance failure named." Six named
// hostilities, each caught by its own exact code, none by accident of
// another rule. Every pair below cross-checks that neighbouring codes are
// not confusable: the failing case's detail never contains a different
// hostility's own code string.
// ---------------------------------------------------------------------

fn assert_drive_refused_with(report: &super::report::ConformanceReport, code: &str) {
    assert!(
        !report.all_passed(),
        "report unexpectedly passed: {report:?}"
    );
    let negotiation = report.case(CASE_CAPABILITY_NEGOTIATION).unwrap();
    assert!(
        negotiation.passed,
        "capability declaration itself is conforming; only the drive should fail"
    );
    let drive = report
        .case(CASE_DRIVE_TO_SETTLEMENT)
        .expect("a negotiation-admitted adapter always reaches the drive case");
    assert!(!drive.passed);
    assert!(
        drive.detail.contains(code),
        "expected detail to name {code}, got: {}",
        drive.detail
    );
}

const ALL_HOSTILE_CODES: &[&str] = &[
    DRIVE_DUPLICATE_COMPLETION,
    DRIVE_CONTRADICTORY_USAGE,
    DRIVE_OVERSIZED,
    DRIVE_LATE_AFTER_CANCEL,
    DRIVE_EVENT_AFTER_COMPLETION,
];

fn assert_no_other_hostile_code_appears(detail: &str, own_code: &str) {
    for other in ALL_HOSTILE_CODES {
        if *other == own_code {
            continue;
        }
        assert!(
            !detail.contains(other),
            "detail for {own_code} must not also name {other}, got: {detail}"
        );
    }
}

#[test]
fn the_deliberately_broken_duplicate_completion_adapter_is_rejected_by_name() {
    let mut adapter = hostile::duplicate_completion_adapter();
    let report = run_conformance_suite(
        &mut adapter,
        &cap(),
        &permissive_requirement(),
        &request(4096),
        None,
        &ExpectedOutcome::Settled {
            response_bytes: b"partial".to_vec(),
        },
    );
    assert_drive_refused_with(&report, DRIVE_DUPLICATE_COMPLETION);
    let drive = report.case(CASE_DRIVE_TO_SETTLEMENT).unwrap();
    assert_no_other_hostile_code_appears(&drive.detail, DRIVE_DUPLICATE_COMPLETION);
}

#[test]
fn the_deliberately_broken_contradictory_usage_adapter_is_rejected_by_name() {
    let mut adapter = hostile::contradictory_usage_adapter();
    let report = run_conformance_suite(
        &mut adapter,
        &cap(),
        &permissive_requirement(),
        &request(4096),
        None,
        &ExpectedOutcome::Settled {
            response_bytes: b"ab".to_vec(),
        },
    );
    assert_drive_refused_with(&report, DRIVE_CONTRADICTORY_USAGE);
    let drive = report.case(CASE_DRIVE_TO_SETTLEMENT).unwrap();
    assert_no_other_hostile_code_appears(&drive.detail, DRIVE_CONTRADICTORY_USAGE);
}

#[test]
fn the_deliberately_broken_oversized_chunk_adapter_is_rejected_by_name() {
    let mut adapter = hostile::oversized_chunk_adapter(128);
    let report = run_conformance_suite(
        &mut adapter,
        &cap(),
        &permissive_requirement(),
        &request(64), // smaller than the adapter's 128-byte single chunk
        None,
        &ExpectedOutcome::Settled {
            response_bytes: vec![b'x'; 128],
        },
    );
    assert_drive_refused_with(&report, DRIVE_OVERSIZED);
    let drive = report.case(CASE_DRIVE_TO_SETTLEMENT).unwrap();
    assert_no_other_hostile_code_appears(&drive.detail, DRIVE_OVERSIZED);
}

#[test]
fn the_deliberately_broken_late_data_after_cancel_adapter_is_rejected_by_name() {
    let mut adapter = hostile::late_data_after_cancel_adapter();
    let report = run_conformance_suite(
        &mut adapter,
        &cap(),
        &permissive_requirement(),
        &request(4096),
        Some(1), // cancel after the first observed event
        &ExpectedOutcome::Settled {
            response_bytes: Vec::new(),
        },
    );
    assert_drive_refused_with(&report, DRIVE_LATE_AFTER_CANCEL);
    let drive = report.case(CASE_DRIVE_TO_SETTLEMENT).unwrap();
    assert_no_other_hostile_code_appears(&drive.detail, DRIVE_LATE_AFTER_CANCEL);
}

#[test]
fn the_deliberately_broken_malformed_event_order_adapter_is_rejected_by_name() {
    let mut adapter = hostile::malformed_order_delta_after_completed_adapter();
    let report = run_conformance_suite(
        &mut adapter,
        &cap(),
        &permissive_requirement(),
        &request(4096),
        None,
        &ExpectedOutcome::Settled {
            response_bytes: b"first".to_vec(),
        },
    );
    assert_drive_refused_with(&report, DRIVE_EVENT_AFTER_COMPLETION);
    let drive = report.case(CASE_DRIVE_TO_SETTLEMENT).unwrap();
    assert_no_other_hostile_code_appears(&drive.detail, DRIVE_EVENT_AFTER_COMPLETION);
}

/// A mid-stream disconnect is not itself a conformance violation: the
/// driver must surface it unchanged as a normalized [`ModelFailure`], never
/// lose, hide, or misclassify it. This is the negative control proving the
/// five tests above are not merely "any Failed/Refused counts as caught" —
/// this one is a legitimate outcome and the suite records it as such.
#[test]
fn a_mid_stream_disconnect_is_surfaced_as_a_normalized_provider_error_not_a_hostile_rejection() {
    let mut adapter = hostile::disconnect_mid_stream_adapter();
    let report = run_conformance_suite(
        &mut adapter,
        &cap(),
        &permissive_requirement(),
        &request(4096),
        None,
        &ExpectedOutcome::FailedWithClass(ModelFailure::ProviderError),
    );
    assert!(
        report.all_passed(),
        "a correctly-classified disconnect is expected, not a violation: {report:?}"
    );
    let drive = report.case(CASE_DRIVE_TO_SETTLEMENT).unwrap();
    assert!(drive.detail.starts_with("failed:provider_error:"));
}

// ---------------------------------------------------------------------
// "No credential, header, proxy, or ambient endpoint appears in receipts
// or source semantics" — this SDK's own analogue: nothing an adapter
// internally holds ever appears in the conformance report it produces.
// ---------------------------------------------------------------------

#[test]
fn a_credential_held_by_an_adapter_never_appears_in_its_conformance_report() {
    const SECRET: &str = "sk-live-conformance-canary-9f3a7e21";
    let mut adapter = hostile::CredentialHoldingAdapter::new(SECRET);
    assert_eq!(
        adapter.held_credential(),
        SECRET,
        "sanity: the secret really was set"
    );

    let report = run_conformance_suite(
        &mut adapter,
        &cap(),
        &permissive_requirement(),
        &request(4096),
        None,
        &ExpectedOutcome::Settled {
            response_bytes: b"ok".to_vec(),
        },
    );
    assert!(report.all_passed());
    let rendered = report.render();
    let debug_rendered = format!("{report:?}");
    assert!(
        !rendered.contains(SECRET),
        "rendered report leaked the held credential"
    );
    assert!(
        !debug_rendered.contains(SECRET),
        "Debug-formatted report leaked the held credential"
    );
}

// ---------------------------------------------------------------------
// Partial UTF-8 / slow trickle: chunk boundaries (including one splitting
// a multi-byte UTF-8 character, and one delivering a single byte at a
// time) must never change the assembled result. Proves the driver is
// chunk-boundary-agnostic rather than accidentally validating per-chunk
// UTF-8 in a way that would refuse a legally split character.
// ---------------------------------------------------------------------

#[test]
fn a_multi_byte_utf8_character_split_across_a_chunk_boundary_assembles_identically_to_one_chunk() {
    // "é" is the two-byte UTF-8 sequence 0xC3 0xA9; split the Delta stream
    // exactly between those two bytes.
    let whole = "caf\u{e9} \u{2713}".as_bytes().to_vec(); // "café ✓"
    let split_at = whole.len() - 3; // inside the multi-byte tail
    let (first, second) = whole.split_at(split_at);

    let mut split_adapter = ScriptedStreamingAdapter::new(
        vec![first.to_vec(), second.to_vec()],
        whole.clone(),
        usage(1, 1, 1),
        true,
    );
    let mut whole_adapter =
        ScriptedStreamingAdapter::new(vec![whole.clone()], whole.clone(), usage(1, 1, 1), true);

    let mut required = permissive_requirement();
    required.require_streaming = true;
    let expected = ExpectedOutcome::Settled {
        response_bytes: whole.clone(),
    };

    let split_report = run_conformance_suite(
        &mut split_adapter,
        &cap(),
        &required,
        &request(4096),
        None,
        &expected,
    );
    let whole_report = run_conformance_suite(
        &mut whole_adapter,
        &cap(),
        &required,
        &request(4096),
        None,
        &expected,
    );

    assert!(split_report.all_passed());
    assert!(whole_report.all_passed());
    assert_eq!(
        split_report.case(CASE_DRIVE_TO_SETTLEMENT).unwrap().detail,
        whole_report.case(CASE_DRIVE_TO_SETTLEMENT).unwrap().detail,
        "chunking must never change the assembled response"
    );
    assert!(std::str::from_utf8(&whole).is_ok());
}

#[test]
fn one_byte_at_a_time_delivery_assembles_identically_to_one_chunk() {
    let whole = b"the-answer-delivered-one-byte-at-a-time".to_vec();
    let trickle: Vec<Vec<u8>> = whole.iter().map(|byte| vec![*byte]).collect();

    let mut trickle_adapter =
        ScriptedStreamingAdapter::new(trickle, whole.clone(), usage(1, 1, 1), true);
    let mut required = permissive_requirement();
    required.require_streaming = true;
    let expected = ExpectedOutcome::Settled {
        response_bytes: whole.clone(),
    };
    let report = run_conformance_suite(
        &mut trickle_adapter,
        &cap(),
        &required,
        &request(4096),
        None,
        &expected,
    );
    assert!(report.all_passed());
}

// ---------------------------------------------------------------------
// "All adapter output still passes compiler-derived Proposal decoding":
// integration with the real, checked `agent_interaction_schema` decoder
// (read-only, reused as-is) — not this SDK's own byte-level rules. A
// legally split multi-byte character reassembles into bytes the real
// compiled decoder admits; a tampered field is refused by that same real
// decoder, not by anything this module invented.
// ---------------------------------------------------------------------

const SCHEMA_FIXTURE: &str = r#"
module test.provider_adapter_sdk;

@id("answer.type")
record Answer {
    @id("answer.note")
    note: string,
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

fn write_temp_schema_source() -> PathBuf {
    let unique = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "semaprax-provider-adapter-sdk-{}-{}-{unique}.spx",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, SCHEMA_FIXTURE).unwrap();
    path
}

fn compile_answer_schema() -> crate::agent_interaction_schema::CompiledInteractionSchema {
    let path = write_temp_schema_source();
    let compiled =
        crate::agent_interaction_schema::compile_agent_interaction_schema(&path, "answer.type")
            .expect("answer.type derivation succeeds");
    std::fs::remove_file(&path).ok();
    compiled
}

fn answer_document(
    schema: &crate::agent_interaction_schema::CompiledInteractionSchema,
    note: &str,
) -> Vec<u8> {
    format!(
        "{{\"schema\":\"semaprax.agent-interaction-value.v1\",\"root_type_id\":\"answer.type\",\"schema_digest\":{},\"value\":{{\"fields\":{{\"answer.note\":{}}}}}}}\n",
        crate::diagnostic::quote_json(schema.schema().digest()),
        crate::diagnostic::quote_json(note),
    )
    .into_bytes()
}

#[test]
fn an_adapter_response_assembled_from_a_split_multi_byte_character_decodes_via_the_real_compiled_schema(
) {
    let schema = compile_answer_schema();
    // "café ✓" again: a two-byte and a three-byte UTF-8 sequence, so the
    // note field genuinely exercises a multi-byte boundary once embedded
    // in the canonical document and split across two adapter Delta chunks.
    let document = answer_document(&schema, "caf\u{e9} \u{2713}");
    let split_at = document.len() / 2;
    let (first, second) = document.split_at(split_at);

    let mut adapter = ScriptedStreamingAdapter::new(
        vec![first.to_vec(), second.to_vec()],
        document.clone(),
        usage(1, 1, 1),
        true,
    );
    let capability = cap();
    let outcome =
        super::conformance::drive_to_settlement(&mut adapter, &capability, &request(65_536), None);
    let response_bytes = match outcome {
        DriveOutcome::Settled { response_bytes } => response_bytes,
        other => panic!("expected Settled, got {other:?}"),
    };
    assert_eq!(response_bytes, document);
    schema.decode(&response_bytes).expect(
        "a legally split multi-byte character must still decode via the real compiled schema",
    );
}

#[test]
fn a_field_tampered_after_reassembly_is_refused_by_the_real_compiled_schema_not_by_this_sdk() {
    let schema = compile_answer_schema();
    let valid = answer_document(&schema, "hi");
    // Structurally legal JSON, semantically wrong: an unknown field name,
    // exactly the kind of admission-rule violation only the real compiled
    // decoder (never this SDK's own byte-level rules) can detect.
    let tampered = String::from_utf8(valid.clone())
        .unwrap()
        .replace(
            "\"answer.note\":\"hi\"",
            "\"answer.note\":\"hi\",\"answer.bogus\":\"1\"",
        )
        .into_bytes();
    assert_ne!(valid, tampered);

    let mut adapter = ScriptedBatchAdapter::new(tampered.clone(), usage(1, 1, 1));
    let outcome =
        super::conformance::drive_to_settlement(&mut adapter, &cap(), &request(65_536), None);
    let response_bytes = match outcome {
        DriveOutcome::Settled { response_bytes } => response_bytes,
        other => panic!("expected Settled, got {other:?}"),
    };
    assert!(
        schema.decode(&response_bytes).is_err(),
        "an adapter-delivered but semantically invalid document must still be refused by the real decoder"
    );
}

#[test]
fn structured_output_mode_names_are_stable_and_distinct() {
    assert_ne!(
        StructuredOutputMode::JsonMode.as_str(),
        StructuredOutputMode::ToolCallShaped.as_str()
    );
    assert_ne!(
        StructuredOutputMode::RawText.as_str(),
        StructuredOutputMode::JsonMode.as_str()
    );
}
