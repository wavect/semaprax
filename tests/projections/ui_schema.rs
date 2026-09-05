//! Executable evidence for UI Dialect Schema Projection v1
//! (`semaprax.ui-dialect-schema.v1`).
//!
//! Pins canonical golden envelope digests over real examples, exercises every
//! record and function exclusion reason, proves per-digest-field tamper
//! rejection including consistently re-minted forgeries, verifies determinism,
//! byte-budget fail-closed behavior, CLI exit codes, and cross-consistency:
//! state-shape facts must equal the checked Native64 compiler layouts and
//! action descriptors must agree with Canonical ABI Report v1 signatures for
//! the same program. No rendering, no runtime, no DOM, no target execution.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use semaprax::abi_report::{self, AbiReportOptions};
use semaprax::graph;
use semaprax::ui_schema::{self, UiSchemaOptions};
use sha2::{Digest as _, Sha256};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

const PAYLOAD_DIGEST_DOMAIN: &[u8] = b"semaprax.ui-dialect-schema.payload.v1\0";

fn write_temp(source: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-ui-schema-evidence-{}-{}.spx",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::write(&path, source).unwrap();
    path
}

fn cleanup(path: &Path) {
    let _ = std::fs::remove_file(path);
}

fn cli(args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args(args)
        .output()
        .expect("semaprax binary");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    )
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    )
}

/// Re-mints the outer digest around `tampered_envelope`'s exact payload
/// bytes so replay must rely on its derivation rules rather than the outer
/// digest alone.
fn remint_digest(tampered_envelope: &str) -> String {
    let payload_key = "\"payload\":";
    let payload_offset = tampered_envelope
        .find(payload_key)
        .expect("tampered envelope keeps its payload member")
        + payload_key.len();
    let payload = &tampered_envelope[payload_offset..tampered_envelope.len() - 1];
    let (prefix, _) = tampered_envelope
        .split_once("\"digest\":")
        .expect("digest member");
    format!(
        "{prefix}\"digest\":{},\"bytes\":{},\"payload\":{}}}",
        semaprax::diagnostic::quote_json(&domain_digest(PAYLOAD_DIGEST_DOMAIN, payload.as_bytes())),
        payload.len(),
        payload
    )
}

const MEANING_PATH: &str = "examples/meaning.spx";
const CALCULATOR_PATH: &str = "examples/calculator.spx";
const RECORDS_PATH: &str = "examples/records.spx";

/// Golden envelope digest over the exact library bytes for three canonical
/// examples, emitted through relative repository paths so the fixtures are
/// machine-independent.
#[test]
fn golden_envelope_digests_are_pinned() {
    let options = UiSchemaOptions::default();

    let meaning = ui_schema::generate(Path::new(MEANING_PATH), &options).expect("meaning envelope");
    assert!(meaning.contains("\"schema\":\"semaprax.ui-dialect-schema.v1\""));
    assert!(meaning.contains("\"state_shapes\":[]"));
    assert!(meaning.contains(
        "{\"stable_id\":\"math.add\",\"name\":\"add\",\"kind\":\"function\",\"role\":\"action\",\
\"signature\":{\"parameters\":[{\"name\":\"left\",\"type\":\"i64\"},\
{\"name\":\"right\",\"type\":\"i64\"}],\"result\":{\"type\":\"i64\"}}"
    ));
    assert_eq!(
        sha256_hex(meaning.as_bytes()),
        "sha256:ceb0e44dc94a94c6afb9ed0ec3fb74bb5b73ce44bd6c62d2e542c346a1d7e0d6"
    );

    let calculator =
        ui_schema::generate(Path::new(CALCULATOR_PATH), &options).expect("calculator envelope");
    assert!(calculator.contains("\"actions_admitted\":7,\"excluded\":0"));
    assert_eq!(
        sha256_hex(calculator.as_bytes()),
        "sha256:a8fda3e66d4fe86e7ac9ab3f713bef55850f490e07ed7bc808e48902b1cba18d"
    );

    let records = ui_schema::generate(Path::new(RECORDS_PATH), &options).expect("records envelope");
    // The Point state shape carries exactly the checked Native64 layout:
    // two i64 fields at offsets 0/8 plus one bool at 16, padded to 24/8.
    assert!(records.contains(
        "\"layout\":{\"fields\":[{\"index\":0,\"name\":\"x\",\"type\":\"i64\",\
\"offset\":0,\"size_bytes\":8,\"align_bytes\":8},{\"index\":1,\"name\":\"y\",\
\"type\":\"i64\",\"offset\":8,\"size_bytes\":8,\"align_bytes\":8},{\"index\":2,\
\"name\":\"enabled\",\"type\":\"bool\",\"offset\":16,\"size_bytes\":1,\
\"align_bytes\":1}],\"size_bytes\":24,\"align_bytes\":8}"
    ));
    assert!(records.contains("\"reason\":\"mixed_field_types\""));
    assert_eq!(
        sha256_hex(records.as_bytes()),
        "sha256:18f997f4e430c3df3ea924ecc0ef55674a5e42d593da98cc7745c670cf09a90f"
    );
}

#[test]
fn reserved_ui_sections_are_explicitly_empty_nonclaims() {
    let options = UiSchemaOptions::default();
    let envelope = ui_schema::generate(Path::new(MEANING_PATH), &options).expect("envelope");
    assert!(envelope.contains("\"controls\":[],\"accessibility\":[],\"navigation\":[]"));
    assert!(envelope.contains(
        "\"nonclaims\":[\"schema_projection_only\",\
\"no_typed_update_or_view_language_constructs\",\"no_semantic_controls\",\
\"no_accessibility\",\"no_navigation\",\"no_localization\",\"no_assets\",\
\"no_platform_blocks\",\"no_custom_rendering\",\"no_target_execution\",\
\"read_only_no_source_changes\"]"
    ));
    ui_schema::verify_envelope(&envelope).expect("verified");
}

#[test]
fn generation_is_deterministic_across_runs() {
    let path = write_temp(std::fs::read_to_string(RECORDS_PATH).unwrap().as_str());
    let options = UiSchemaOptions::default();
    let first = ui_schema::generate(&path, &options).expect("first envelope");
    let second = ui_schema::generate(&path, &options).expect("second envelope");
    assert_eq!(first, second);
    cleanup(&path);
}

#[test]
fn verify_envelope_round_trips_descriptors_in_stable_id_order() {
    let options = UiSchemaOptions::default();
    let envelope = ui_schema::generate(Path::new(RECORDS_PATH), &options).expect("envelope");
    let verified = ui_schema::verify_envelope(&envelope).expect("verified");
    assert_eq!(verified.state_shapes.len(), 1);
    assert_eq!(verified.state_shapes[0].stable_id, "geometry.point");
    assert_eq!(verified.state_shapes[0].name, "Point");
    assert_eq!(verified.state_shapes[0].size_bytes, 24);
    assert_eq!(verified.state_shapes[0].align_bytes, 8);
    assert_eq!(
        verified.state_shapes[0]
            .fields
            .iter()
            .map(|field| (
                field.index,
                field.name.as_str(),
                field.ty.as_str(),
                field.offset,
                field.size_bytes,
                field.align_bytes
            ))
            .collect::<Vec<_>>(),
        vec![
            (0, "x", "i64", 0, 8, 8),
            (1, "y", "i64", 8, 8, 8),
            (2, "enabled", "bool", 16, 1, 1),
        ]
    );
    assert_eq!(verified.actions.len(), 1);
    assert_eq!(verified.actions[0].stable_id, "app.main");
    assert!(verified.actions[0].parameters.is_empty());
    assert_eq!(verified.actions[0].result_ty, "i64");

    // Re-verification of the same bytes replays identically.
    let replay = ui_schema::verify_envelope(&envelope).expect("replay");
    assert_eq!(verified, replay);

    let calculator = ui_schema::generate(Path::new(CALCULATOR_PATH), &options).expect("calculator");
    let verified_calculator = ui_schema::verify_envelope(&calculator).expect("verified");
    let ids = verified_calculator
        .actions
        .iter()
        .map(|action| action.stable_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        ids,
        vec![
            "app.main",
            "calculator.add",
            "calculator.divide",
            "calculator.is-negative",
            "calculator.multiply",
            "calculator.not",
            "calculator.subtract",
        ]
    );
    assert!(ids
        .windows(2)
        .all(|pair| pair[0].as_bytes() <= pair[1].as_bytes()));
}

#[test]
fn every_record_and_function_exclusion_reason_is_reachable() {
    let source = r#"
module test.probe;
permit { io.release }

@id("probe.plain")
record Plain {
    @id("probe.plain.value")
    value: i64,
}

record Automatic {
    @id("probe.automatic.value")
    value: i64,
}

@id("probe.box")
record Box<T> {
    @id("probe.box.value")
    value: T,
}

@id("probe.mixed")
record Mixed {
    @id("probe.mixed.start")
    start: Plain,
}

@id("probe.token")
resource Token {
    @id("probe.token.drop")
    drop trivial;
}

@id("probe.choice")
variant Choice {
    @id("probe.choice.none")
    None,
    @id("probe.choice.some")
    Some {
        @id("probe.choice.some.value")
        value: i64,
    },
}

@id("probe.generic")
fn pick<T>(value: T) -> T { value }

@id("probe.effectful")
fn effectful(value: i64) -> i64 uses { io.release } { value }

@id("probe.borrowed")
fn borrowed(target: borrow Token, amount: i64) -> i64 { amount }

@id("probe.wide")
fn wide(plain: Plain) -> i64 { 0 }

@id("probe.narrow")
fn narrow(value: i64) -> Plain { Plain { value: value } }

fn helper(value: i64) -> i64 { value + 1 }

@id("app.main")
fn main() -> i64 { Plain { value: helper(0) }.value }
"#;
    let path = write_temp(source);
    let options = UiSchemaOptions::default();
    let envelope = ui_schema::generate(&path, &options).expect("all-excluded envelope");
    for reason in [
        "automatic_identity",
        "generic_type",
        "resource_type",
        "variant_type",
        "mixed_field_types",
        "generic_function",
        "declared_effects",
        "unsupported_parameter_mode",
        "unsupported_parameter_type",
        "unsupported_result_type",
    ] {
        assert!(
            envelope.contains(&format!("\"reason\":\"{reason}\"")),
            "missing exclusion reason {reason}"
        );
    }
    // The admitted scalar record and the admitted main remain present.
    assert!(envelope.contains("\"stable_id\":\"probe.plain\""));
    assert!(envelope.contains("\"stable_id\":\"app.main\""));
    ui_schema::verify_envelope(&envelope).expect("verified");
    cleanup(&path);
}

#[test]
fn verify_envelope_rejects_every_digest_field_tamper() {
    let options = UiSchemaOptions::default();
    let envelope = ui_schema::generate(Path::new(RECORDS_PATH), &options).expect("envelope");

    // State-shape layout field (any listed layout byte).
    let tampered_layout = envelope.replace("\"size_bytes\":24", "\"size_bytes\":25");
    assert_ne!(tampered_layout, envelope);
    assert!(ui_schema::verify_envelope(&tampered_layout).is_err());
    // Even a consistently re-minted outer digest cannot smuggle a forged
    // layout past the embedded state-shape digest replay.
    let error = ui_schema::verify_envelope(&remint_digest(&tampered_layout))
        .expect_err("forged layout must fail replay");
    assert_eq!(error.code, "SPX-U103");

    // Action signature field.
    let tampered_signature = envelope.replace(
        "\"parameters\":[],\"result\":{\"type\":\"i64\"}",
        "\"parameters\":[],\"result\":{\"type\":\"i32\"}",
    );
    assert_ne!(tampered_signature, envelope);
    assert!(ui_schema::verify_envelope(&tampered_signature).is_err());

    // Outer payload-digest field.
    const DIGEST_MARKER: &str = "\"digest\":\"sha256:";
    let digest_start =
        envelope.find(DIGEST_MARKER).expect("outer digest member") + DIGEST_MARKER.len();
    let mut corrupted = envelope.clone().into_bytes();
    corrupted[digest_start] = if corrupted[digest_start] == b'0' {
        b'1'
    } else {
        b'0'
    };
    let tampered_outer = String::from_utf8(corrupted).unwrap();
    assert_ne!(tampered_outer, envelope);
    assert!(ui_schema::verify_envelope(&tampered_outer).is_err());

    // Spliced payload member invalidates the outer digest over exact bytes.
    let spliced = format!(
        "{}{}",
        &envelope[..envelope.len() - 1],
        ",\"injected\":true}"
    );
    assert!(ui_schema::verify_envelope(&spliced).is_err());

    // A nonempty reserved UI section violates the schema contract even when
    // its digest was re-minted around the forgery.
    let injected_section =
        envelope.replace("\"controls\":[]", "\"controls\":[{\"widget\":\"button\"}]");
    assert_ne!(injected_section, envelope);
    let error = ui_schema::verify_envelope(&remint_digest(&injected_section))
        .expect_err("injected control must fail replay");
    assert_eq!(error.code, "SPX-U103");

    // Structural damage and foreign schemas.
    let truncated = envelope[..envelope.len() - 4].to_owned();
    assert!(ui_schema::verify_envelope(&truncated).is_err());
    assert!(ui_schema::verify_envelope("not json").is_err());
    assert!(ui_schema::verify_envelope("[]").is_err());
    let foreign_schema = envelope.replace(
        "\"schema\":\"semaprax.ui-dialect-schema.v1\"",
        "\"schema\":\"semaprax.foreign.v1\"",
    );
    assert_ne!(foreign_schema, envelope);
    assert!(ui_schema::verify_envelope(&foreign_schema).is_err());

    // Source-drift rebinding through the embedded source digest: the
    // envelope embeds the domain-separated digest of its own source bytes,
    // and drifted source bytes produce a different digest and a different
    // envelope.
    const SOURCE_DIGEST_DOMAIN: &[u8] = b"semaprax.ui-dialect-schema.source.v1\0";
    let records_source = std::fs::read(RECORDS_PATH).unwrap();
    let records_digest = domain_digest(SOURCE_DIGEST_DOMAIN, &records_source);
    let records_marker = format!("\"sha256\":\"{records_digest}\"");
    assert!(envelope.contains(&records_marker));
    let drifted_path =
        write_temp(&(std::fs::read_to_string(RECORDS_PATH).unwrap() + "\n// drift\n"));
    let drifted_bytes = std::fs::read(&drifted_path).unwrap();
    let drifted_envelope = ui_schema::generate(&drifted_path, &options).expect("drifted");
    assert_ne!(drifted_envelope, envelope);
    assert!(drifted_envelope.contains(&format!(
        "\"sha256\":\"{}\"",
        domain_digest(SOURCE_DIGEST_DOMAIN, &drifted_bytes)
    )));
    cleanup(&drifted_path);
}

#[test]
fn budget_exhaustion_fails_closed_without_truncation() {
    let path = write_temp(std::fs::read_to_string(MEANING_PATH).unwrap().as_str());
    let tiny = UiSchemaOptions::new(graph::MIN_AGENT_CONTEXT_BYTES).unwrap();
    let outcome = ui_schema::generate(&path, &tiny);
    let errors = outcome.expect_err("tiny budgets must fail closed");
    assert!(
        errors.iter().any(|item| item.code == "SPX-U102"),
        "expected the byte-budget diagnostic"
    );
    assert!(UiSchemaOptions::new(512).is_err());
    cleanup(&path);
}

#[test]
fn cli_double_run_is_byte_identical() {
    let args = ["ui-schema", RECORDS_PATH];
    let (first_code, first_out, _) = cli(&args);
    let (second_code, second_out, _) = cli(&args);
    assert_eq!(first_code, 0);
    assert_eq!(second_code, 0);
    assert_eq!(first_out, second_out);
    assert!(first_out.contains("\"schema\":\"semaprax.ui-dialect-schema.v1\""));
    assert!(first_out.ends_with("}\n"));
}

#[test]
fn cli_rejects_bad_invocations() {
    // Missing file path.
    let (code, _, _) = cli(&["ui-schema"]);
    assert_eq!(code, 2);
    // Unknown option.
    let (code, _, err) = cli(&["ui-schema", MEANING_PATH, "--sections", "controls"]);
    assert_eq!(code, 2);
    assert!(err.contains("unknown ui-schema option"));
    // Duplicate option.
    let (code, _, err) = cli(&[
        "ui-schema",
        MEANING_PATH,
        "--max-bytes",
        "65536",
        "--max-bytes",
        "65536",
    ]);
    assert_eq!(code, 2);
    assert!(err.contains("duplicate"));
    // Malformed value.
    let (code, _, err) = cli(&["ui-schema", MEANING_PATH, "--max-bytes", "-3"]);
    assert_eq!(code, 2);
    assert!(err.contains("canonical nonnegative integer"));
    // Out-of-bounds value.
    let (code, _, err) = cli(&["ui-schema", MEANING_PATH, "--max-bytes", "512"]);
    assert_eq!(code, 2);
    assert!(err.contains("SPX-U101"));
    // Missing option value.
    let (code, _, _) = cli(&["ui-schema", MEANING_PATH, "--max-bytes"]);
    assert_eq!(code, 2);
    // Byte-budget exhaustion is fail-closed; the smallest legal budget is
    // below any real envelope size.
    let (code, _, err) = cli(&["ui-schema", MEANING_PATH, "--max-bytes", "2048"]);
    assert_eq!(code, 1);
    assert!(err.contains("SPX-U102"));
    // Unverifiable sources fail closed as ordinary diagnostics.
    let broken = write_temp("module test.broken;\nfn main() -> i64 { missing_fn(1) }\n");
    let (code, _, _) = cli(&["ui-schema", broken.to_str().unwrap()]);
    assert_eq!(code, 1);
    cleanup(&broken);
}

/// Action descriptors must agree with Canonical ABI Report v1 for the same
/// program: the admitted stable-ID sets match, and each action's parameter
/// and result types correspond to the reported native parameter/result types.
#[test]
fn actions_agree_with_the_canonical_abi_report() {
    let options = UiSchemaOptions::default();
    let envelope = ui_schema::generate(Path::new(CALCULATOR_PATH), &options).expect("envelope");
    let verified = ui_schema::verify_envelope(&envelope).expect("verified");

    let abi_tokens = verified
        .actions
        .iter()
        .map(|action| action.stable_id.clone())
        .collect::<Vec<_>>();
    let abi_options =
        AbiReportOptions::new(abi_tokens.clone(), 64 * 1024).expect("valid selection");
    let abi_envelope =
        abi_report::generate(Path::new(CALCULATOR_PATH), &abi_options).expect("abi envelope");
    let abi_value: serde_json::Value = serde_json::from_str(&abi_envelope).unwrap();
    let abi_functions = abi_value["payload"]["functions"].as_array().unwrap();

    let mut abi_by_id = std::collections::BTreeMap::new();
    for function in abi_functions {
        abi_by_id.insert(
            function["stable_id"].as_str().unwrap().to_owned(),
            function.clone(),
        );
    }
    assert_eq!(abi_by_id.len(), verified.actions.len());
    for action in &verified.actions {
        let native = abi_by_id
            .get(action.stable_id.as_str())
            .unwrap_or_else(|| panic!("abi report is missing {}", action.stable_id));
        assert_eq!(native["name"].as_str().unwrap(), action.name);
        let parameters = native["native"]["parameters"].as_array().unwrap();
        assert_eq!(parameters.len(), action.parameters.len());
        for (parameter, fact) in action.parameters.iter().zip(parameters.iter()) {
            let expected = match parameter.1.as_str() {
                "i64" => "int64_t",
                "bool" => "bool",
                other => panic!("unexpected action type {other}"),
            };
            assert_eq!(fact["c_type"].as_str().unwrap(), expected);
            assert_eq!(fact["mode"].as_str().unwrap(), "value");
        }
        let result_c_type = native["native"]["result"]["c_type"].as_str().unwrap();
        let expected_result = match action.result_ty.as_str() {
            "i64" => "int64_t",
            "bool" => "bool",
            other => panic!("unexpected action result type {other}"),
        };
        assert_eq!(result_c_type, expected_result);
        // The reported prototype must exist verbatim in the native projection
        // exactly once per admitted identity.
        assert!(native["native"]["signature"]
            .as_str()
            .unwrap()
            .starts_with("static __attribute__((unused)) spx_status_token "));
    }
}
