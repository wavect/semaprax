//! Executable evidence for Interface Package Report v1
//! (`semaprax.package-report.v1`).
//!
//! Pins canonical golden envelope digests, exercises every exclusion reason,
//! proves byte-level cross-consistency with both `semaprax abi-report` and
//! `semaprax openapi` for the same program, verifies determinism,
//! fail-closed budget behavior, CLI exit codes, and tamper rejection per
//! digest field including forged-but-re-signed envelopes caught by closed
//! replay. No resolver, lockfile, registry, compatibility engine, conformance
//! suite, or target execution is involved.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use semaprax::abi_report::AbiReportOptions;
use semaprax::package_report::{self, PackageReportOptions};
use semaprax::{abi_report, openapi};
use sha2::{Digest as _, Sha256};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

const PAYLOAD_DIGEST_DOMAIN: &[u8] = b"semaprax.package-report.payload.v1\0";
const EXPORT_SIGNATURE_DIGEST_DOMAIN: &[u8] = b"semaprax.package-report.export-signature.v1\0";

fn write_temp(source: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-package-report-evidence-{}-{}.spx",
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

/// The exact domain-separated digest minted by the generator, reproduced
/// independently so hostile tests can re-mint consistent-looking envelopes.
fn payload_digest(payload: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(PAYLOAD_DIGEST_DOMAIN);
    hasher.update((payload.len() as u64).to_le_bytes());
    hasher.update(payload.as_bytes());
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    )
}

/// Re-mints the outer digest around `tampered_envelope`'s exact payload
/// bytes so replay must rely on its derivation rules rather than the digest
/// alone.
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
        semaprax::diagnostic::quote_json(&payload_digest(payload)),
        payload.len(),
        payload
    )
}

const MEANING_PATH: &str = "examples/meaning.spx";
const CALCULATOR_PATH: &str = "examples/calculator.spx";

/// Golden envelope digest over the exact library bytes for the canonical
/// calculator example, emitted through the relative repository path so the
/// fixture is machine-independent. The whole-module inventory admits all six
/// calculator functions plus `app.main`.
#[test]
fn golden_calculator_envelope_digest_is_pinned() {
    let options = PackageReportOptions::default();
    let envelope =
        package_report::generate(Path::new(CALCULATOR_PATH), &options).expect("envelope");
    assert!(envelope.contains("\"schema\":\"semaprax.package-report.v1\""));
    assert!(envelope.contains(
        "\"targets\":[{\"target\":\"native64\",\"available\":true},{\"target\":\"wasm32\",\"available\":true}]"
    ));
    assert!(envelope.contains("\"exports_admitted\":7,\"exports_excluded\":0"));
    assert_eq!(
        sha256_hex(envelope.as_bytes()),
        "sha256:19e31c65f46cd73cc8f948f17d2266d36a6779ee9a3179cc5cd7bdbc29fef748"
    );
}

/// A second pinned KAT over the minimal two-function example.
#[test]
fn golden_meaning_envelope_digest_is_pinned() {
    let options = PackageReportOptions::default();
    let envelope = package_report::generate(Path::new(MEANING_PATH), &options).expect("envelope");
    assert!(envelope.contains(
        "\"package\":{\"name\":\"examples.meaning\",\
\"functions_total\":2,\"exports_admitted\":2,\"exports_excluded\":0}"
    ));
    assert_eq!(
        sha256_hex(envelope.as_bytes()),
        "sha256:97bcde287804d9311f343157058926fb0648e66282461ede138e98824aac06f2"
    );
}

#[test]
fn generation_is_deterministic() {
    let options = PackageReportOptions::default();
    let first = package_report::generate(Path::new(CALCULATOR_PATH), &options).expect("first");
    let second = package_report::generate(Path::new(CALCULATOR_PATH), &options).expect("second");
    assert_eq!(first, second);
}

#[test]
fn exports_carry_interface_facts_and_exact_native_signatures() {
    let options = PackageReportOptions::default();
    let envelope = package_report::generate(Path::new(MEANING_PATH), &options).expect("envelope");
    let value: serde_json::Value = serde_json::from_str(&envelope).unwrap();
    let exports = value["payload"]["exports"].as_array().unwrap();
    assert_eq!(exports.len(), 2);
    assert_eq!(exports[0]["stable_id"], "app.main");
    assert_eq!(exports[0]["parameters"].as_array().unwrap().len(), 0);
    assert_eq!(exports[0]["result"], "i64");
    assert_eq!(exports[0]["requires"].as_array().unwrap().len(), 0);
    assert_eq!(exports[0]["ensures"], serde_json::json!(["result == 42"]));
    assert_eq!(exports[0]["effects"].as_array().unwrap().len(), 0);
    assert_eq!(exports[1]["stable_id"], "math.add");
    assert_eq!(exports[1]["parameters"], serde_json::json!(["i64", "i64"]));
    assert_eq!(
        exports[1]["requires"],
        serde_json::json!(["left >= 0", "right >= 0"])
    );
    assert_eq!(
        exports[1]["native64"]["symbol"],
        "spx_decl_6d6174682e616464"
    );
    assert!(exports[1]["native64"]["signature"]
        .as_str()
        .unwrap()
        .starts_with("static __attribute__((unused)) spx_status_token spx_decl_6d6174682e616464("));
    assert!(exports[1]["native64"]["signature"]
        .as_str()
        .unwrap()
        .ends_with("int64_t *spx_result_out);"));

    // The embedded export-signature digest equals an independent
    // domain-separated recomputation over the exact signature bytes.
    let signature = exports[1]["native64"]["signature"].as_str().unwrap();
    let mut hasher = Sha256::new();
    hasher.update(EXPORT_SIGNATURE_DIGEST_DOMAIN);
    hasher.update((signature.len() as u64).to_le_bytes());
    hasher.update(signature.as_bytes());
    assert_eq!(
        exports[1]["native64"]["signature_sha256"],
        format!(
            "sha256:{:x}",
            semaprax::digest_hex::LowerHex(hasher.finalize())
        )
    );

    let report = package_report::verify_envelope(&envelope).expect("verified");
    assert_eq!(report.exports.len(), 2);
    assert_eq!(report.exports[1].stable_id, "math.add");
    let replay = package_report::verify_envelope(&envelope).expect("replay");
    assert_eq!(report, replay);
}

#[test]
fn every_exclusion_reason_is_reachable() {
    let source = r#"
module test.probe;
permit { io.release }

@id("probe.generic")
fn pick<T>(value: T) -> T { value }

@id("probe.effectful")
fn effectful(value: i64) -> i64 uses { io.release } { value }

@id("probe.borrowed")
fn borrowed(target: borrow Buffer, amount: i64) -> i64 { amount }

@id("probe.box")
record Box {
    @id("probe.box.value")
    value: i64,
}

@id("probe.wide")
fn wide(box: Box) -> Box { box }

@id("probe.narrow")
fn narrow(value: i64) -> Box { Box { value: value } }

fn helper(value: i64) -> i64 { value + 1 }

fn main() -> i64 { helper(0) }

@id("buffer.type")
resource Buffer {
    @id("buffer.type.drop")
    drop trivial;
}
"#;
    let path = write_temp(source);
    let envelope =
        package_report::generate(&path, &PackageReportOptions::default()).expect("envelope");
    for reason in [
        "automatic_identity",
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
    assert!(
        envelope.contains("\"functions_total\":7,\"exports_admitted\":0,\"exports_excluded\":7")
    );
    let report = package_report::verify_envelope(&envelope).expect("verified");
    assert!(report.exports.is_empty());

    // An out-of-vocabulary exclusion reason fails the closed-vocabulary
    // replay even when the outer digest was re-minted around the forgery.
    let foreign_reason =
        envelope.replace("\"reason\":\"generic_function\"", "\"reason\":\"magic\"");
    assert_ne!(foreign_reason, envelope);
    let error = package_report::verify_envelope(&remint_digest(&foreign_reason))
        .expect_err("foreign reason must fail replay");
    assert_eq!(error.code, "SPX-P303");
    cleanup(&path);
}

#[test]
fn verify_envelope_rejects_every_digest_field_tamper() {
    let options = PackageReportOptions::default();
    let envelope = package_report::generate(Path::new(MEANING_PATH), &options).expect("envelope");

    // Export-signature digest field: a signature-text mutation first breaks
    // the outer digest ...
    let tampered_signature = envelope.replace("*spx_result_out);", "*spx_result_out );");
    assert_ne!(tampered_signature, envelope);
    assert!(package_report::verify_envelope(&tampered_signature).is_err());

    // ... and even a consistently re-signed envelope is caught by the inner
    // export-signature digest replay.
    let resigned = remint_digest(&tampered_signature);
    let error = package_report::verify_envelope(&resigned)
        .expect_err("re-signed signature mutation must fail replay");
    assert_eq!(error.code, "SPX-P303");

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
    assert!(package_report::verify_envelope(&tampered_outer).is_err());

    // Contract text mutation breaks the exact payload bytes.
    let tampered_contract = envelope.replace("left >= 0", "left >= 1");
    assert_ne!(tampered_contract, envelope);
    assert!(package_report::verify_envelope(&tampered_contract).is_err());

    // Spliced payload member invalidates the outer digest over exact bytes.
    let spliced = format!(
        "{}{}",
        &envelope[..envelope.len() - 1],
        ",\"injected\":true}"
    );
    assert!(package_report::verify_envelope(&spliced).is_err());

    // Structural damage and foreign schemas.
    let truncated = envelope[..envelope.len() - 4].to_owned();
    assert!(package_report::verify_envelope(&truncated).is_err());
    assert!(package_report::verify_envelope("not json").is_err());
    assert!(package_report::verify_envelope("[]").is_err());
    let foreign_schema = envelope.replace(
        "\"schema\":\"semaprax.package-report.v1\"",
        "\"schema\":\"semaprax.foreign.v1\"",
    );
    assert_ne!(foreign_schema, envelope);
    assert!(package_report::verify_envelope(&foreign_schema).is_err());
}

#[test]
fn re_signed_closed_section_forgeries_fail_replay() {
    let options = PackageReportOptions::default();
    let envelope = package_report::generate(Path::new(MEANING_PATH), &options).expect("envelope");

    // A forged target matrix cannot demote an admitted target.
    let forged_target = envelope.replace(
        "\"target\":\"wasm32\",\"available\":true",
        "\"target\":\"wasm32\",\"available\":false",
    );
    assert_ne!(forged_target, envelope);
    let error = package_report::verify_envelope(&remint_digest(&forged_target))
        .expect_err("demoted target must fail replay");
    assert_eq!(error.code, "SPX-P303");

    // A third target cannot be smuggled into the matrix either.
    let smuggled_target = envelope.replace(
        "\"targets\":[",
        "\"targets\":[{\"target\":\"wasi\",\"available\":true},",
    );
    assert_ne!(smuggled_target, envelope);
    assert!(package_report::verify_envelope(&remint_digest(&smuggled_target)).is_err());

    // The unavailable-capability list is closed; removing or adding entries
    // fails even with a consistent outer digest.
    let dropped_capability = envelope.replace(",\"resolver\"", "");
    assert_ne!(dropped_capability, envelope);
    assert!(package_report::verify_envelope(&remint_digest(&dropped_capability)).is_err());
    let added_capability = envelope.replace(
        "\"unavailable_capabilities\":[",
        "\"unavailable_capabilities\":[\"sandboxing\",",
    );
    assert_ne!(added_capability, envelope);
    assert!(package_report::verify_envelope(&remint_digest(&added_capability)).is_err());

    // Count forgery fails the counts-vs-listings replay.
    let forged_count = envelope.replace("\"exports_admitted\":2,", "\"exports_admitted\":3,");
    assert_ne!(forged_count, envelope);
    assert!(package_report::verify_envelope(&remint_digest(&forged_count)).is_err());
}

#[test]
fn listed_exports_equal_what_abi_report_admits() {
    let options = PackageReportOptions::default();
    let envelope =
        package_report::generate(Path::new(CALCULATOR_PATH), &options).expect("envelope");
    let report = package_report::verify_envelope(&envelope).expect("verified");
    let mut tokens: Vec<String> = report
        .exports
        .iter()
        .map(|export| export.stable_id.clone())
        .collect();

    let abi_options = AbiReportOptions::new(tokens.clone(), 64 * 1024).expect("valid options");
    let abi_envelope =
        abi_report::generate(Path::new(CALCULATOR_PATH), &abi_options).expect("abi envelope");
    let abi_report_value = abi_report::verify_envelope(&abi_envelope).expect("abi verified");
    let mut abi_ids: Vec<String> = abi_report_value
        .functions
        .iter()
        .map(|function| function.stable_id.clone())
        .collect();
    tokens.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    abi_ids.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    assert!(!tokens.is_empty());
    assert_eq!(tokens, abi_ids);

    // Both reports quote the identical verbatim native prototype per export.
    for export in &report.exports {
        let function = abi_report_value
            .functions
            .iter()
            .find(|function| function.stable_id == export.stable_id)
            .expect("abi report lists the same export");
        assert_eq!(export.native_symbol, function.native_symbol);
        assert_eq!(export.native_signature, function.native_signature);
        assert!(
            export.native_signature.ends_with("*spx_result_out);"),
            "signatures come from the same production native projection"
        );
    }
}

#[test]
fn listed_exports_equal_the_openapi_operations() {
    let options = PackageReportOptions::default();
    let envelope =
        package_report::generate(Path::new(CALCULATOR_PATH), &options).expect("envelope");
    let value: serde_json::Value = serde_json::from_str(&envelope).unwrap();
    let mut package_ids: Vec<String> = value["payload"]["exports"]
        .as_array()
        .unwrap()
        .iter()
        .map(|export| export["stable_id"].as_str().unwrap().to_owned())
        .collect();

    package_report::verify_envelope(&envelope).expect("verified");
    let mut selections = package_ids.clone();
    selections.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    let openapi_envelope = openapi::generate(
        Path::new(CALCULATOR_PATH),
        &selections,
        &openapi::OpenApiOptions::default(),
    )
    .expect("openapi envelope");
    let openapi_value: serde_json::Value = serde_json::from_str(&openapi_envelope).unwrap();
    let mut openapi_ids: Vec<String> = openapi_value["document"]["paths"]
        .as_object()
        .unwrap()
        .keys()
        .map(|path| path.trim_start_matches('/').to_owned())
        .collect();
    package_ids.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    openapi_ids.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    assert!(!openapi_ids.is_empty());
    assert_eq!(package_ids, openapi_ids);

    // Every admitted export also appears with its exact x-stable-id marker.
    for id in &package_ids {
        assert!(openapi_envelope.contains(&format!("\"x-stable-id\":\"{id}\"")));
    }
}

#[test]
fn source_drift_between_generation_and_validation_fails_closed() {
    let path = write_temp(
        "module test.probe;\n@id(\"probe.one\")\nfn one(value: i64) -> i64 { value }\n@id(\"app.main\")\nfn main() -> i64 { 0 }\n",
    );
    let options = PackageReportOptions::default();
    let first = package_report::generate(&path, &options).expect("envelope");
    std::fs::write(&path, "module test.probe;\n").unwrap();
    let outcome = package_report::generate(&path, &options);
    assert!(
        outcome.is_err(),
        "drifted source must not reproduce the report"
    );
    assert_ne!(
        package_report::generate(&path, &options).unwrap_or_default(),
        first
    );
    cleanup(&path);
}

#[test]
fn budget_exhaustion_fails_closed_without_truncation() {
    let path = write_temp(
        "module test.probe;\n@id(\"probe.one\")\nfn one(value: i64) -> i64 { value }\n@id(\"app.main\")\nfn main() -> i64 { 0 }\n",
    );
    let tiny = PackageReportOptions::new(semaprax::graph::MIN_AGENT_CONTEXT_BYTES).unwrap();
    let outcome = package_report::generate(&path, &tiny);
    let errors = outcome.expect_err("tiny budgets must fail closed");
    assert!(
        errors.iter().any(|item| item.code == "SPX-P302"),
        "expected the byte-budget diagnostic"
    );
    cleanup(&path);
}

#[test]
fn unverified_source_fails_closed() {
    let path = write_temp("module test.probe;\nfn broken( { 0 }\n");
    let outcome = package_report::generate(&path, &PackageReportOptions::default());
    assert!(outcome.is_err());
    cleanup(&path);
}

#[test]
fn cli_double_run_is_byte_identical() {
    let args = ["package-report", CALCULATOR_PATH];
    let (first_code, first_out, _) = cli(&args);
    let (second_code, second_out, _) = cli(&args);
    assert_eq!(first_code, 0);
    assert_eq!(second_code, 0);
    assert_eq!(first_out, second_out);
    assert!(first_out.contains("\"schema\":\"semaprax.package-report.v1\""));
    assert!(first_out.ends_with("}\n"));
}

#[test]
fn cli_rejects_bad_invocations() {
    // Missing file path.
    let (code, _, _) = cli(&["package-report"]);
    assert_eq!(code, 2);
    // Unknown option.
    let (code, _, err) = cli(&["package-report", CALCULATOR_PATH, "--functions", "x"]);
    assert_eq!(code, 2);
    assert!(err.contains("unknown package-report option"));
    // Duplicate option.
    let (code, _, err) = cli(&[
        "package-report",
        CALCULATOR_PATH,
        "--max-bytes",
        "65536",
        "--max-bytes",
        "65536",
    ]);
    assert_eq!(code, 2);
    assert!(err.contains("duplicate"));
    // Malformed value.
    let (code, _, err) = cli(&["package-report", CALCULATOR_PATH, "--max-bytes", "-3"]);
    assert_eq!(code, 2);
    assert!(err.contains("canonical nonnegative integer"));
    // Out-of-bounds value.
    let (code, _, err) = cli(&["package-report", CALCULATOR_PATH, "--max-bytes", "512"]);
    assert_eq!(code, 2);
    assert!(err.contains("SPX-P301"));
    // Missing option value.
    let (code, _, _) = cli(&["package-report", CALCULATOR_PATH, "--max-bytes"]);
    assert_eq!(code, 2);
    // Byte-budget exhaustion is fail-closed.
    let big = write_temp(
        "module app.big;\n\n@id(\"big.one\")\nfn one(value: i64) -> i64\n    requires value >= 0\n    ensures result == value + 1\n{ value + 1 }\n\n@id(\"app.main\")\nfn main() -> i64 { one(41) }\n",
    );
    let (code, _, err) = cli(&[
        "package-report",
        big.to_str().unwrap(),
        "--max-bytes",
        "2048",
    ]);
    assert_eq!(code, 1);
    assert!(err.contains("SPX-P302"), "stderr was: {err}");
    cleanup(&big);
    // Unverifiable sources fail closed with exit code 1.
    let bad = write_temp("module test.probe;\nfn broken( { 0 }\n");
    let (code, _, _) = cli(&["package-report", bad.to_str().unwrap()]);
    assert_eq!(code, 1);
    cleanup(&bad);
}
