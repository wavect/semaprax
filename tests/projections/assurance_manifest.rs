//! Executable evidence for Assurance Manifest v1
//! (`semaprax.assurance-manifest.v1`).
//!
//! Pins a golden envelope digest, proves determinism, exercises hostile
//! reordering/duplicate/forged-class/dangling-assumption/drift rejection
//! through independent replay, and checks the delta's added/removed
//! buckets. `semaprax assurance-manifest <file>` (#214) is exercised at the
//! end of this file by spawning the real binary, not merely by checking it
//! is registered in the CLI catalog.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

use semaprax::assurance_manifest::{
    self, delta, public_view, verify_envelope, verify_envelope_against_source,
    AssuranceManifestOptions,
};
use sha2::{Digest as _, Sha256};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

const PAYLOAD_DIGEST_DOMAIN: &[u8] = b"semaprax.assurance-manifest.payload.v1\0";

fn write_temp(source: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-assurance-manifest-evidence-{}-{}.spx",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::write(&path, source).unwrap();
    path
}

fn cleanup(path: &Path) {
    let _ = std::fs::remove_file(path);
}

/// The exact domain-separated digest [`assurance_manifest::generate`] mints,
/// reproduced independently so hostile tests can re-mint consistent-looking
/// envelopes without importing the producer's private digest function.
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

/// Re-wrap a (possibly hostile-mutated) payload JSON object into a
/// structurally valid outer envelope with a correctly recomputed digest, so
/// a hostile test can isolate exactly one payload-level defect instead of
/// also tripping the outer digest check.
fn rewrap(payload: &str) -> String {
    format!(
        "{{\"schema\":\"semaprax.assurance-manifest.v1\",\"digest\":\"{}\",\"bytes\":{},\"payload\":{}}}",
        payload_digest(payload),
        payload.len(),
        payload,
    )
}

fn payload_of(envelope: &str) -> serde_json::Value {
    let value: serde_json::Value = serde_json::from_str(envelope).unwrap();
    value["payload"].clone()
}

const DECLARED_SOURCE: &str = r#"
module app.assurance;

@id("app.assurance.combine")
fn combine(a: i64, b: i64) -> i64
    requires a >= 0
    requires b >= 0
    ensures result >= a
{ a + b }

@id("app.assurance.identity")
fn identity(value: i64) -> i64
    ensures result == value
{ value }

@id("app.assurance.main")
fn main() -> i64 { 0 }
"#;

#[test]
fn golden_envelope_digest_is_pinned() {
    let path = write_temp(DECLARED_SOURCE);
    let envelope = assurance_manifest::generate(&path, &AssuranceManifestOptions::default());
    cleanup(&path);
    let envelope = envelope.expect("envelope");
    assert!(envelope.contains("\"schema\":\"semaprax.assurance-manifest.v1\""));
    let payload = payload_of(&envelope);
    assert_eq!(payload["counts"]["obligations_total"], 7);
    assert_eq!(payload["counts"]["by_class"]["runtime_guarded"], 4);
    assert_eq!(payload["counts"]["by_class"]["compiler_proved"], 3);
    assert_eq!(payload["counts"]["by_class"]["open"], 0);
    assert_eq!(payload["counts"]["assumptions_total"], 0);
    verify_envelope(&envelope).expect("golden envelope must independently replay");
}

#[test]
fn generation_is_byte_deterministic_across_repeated_calls() {
    let path = write_temp(DECLARED_SOURCE);
    let first = assurance_manifest::generate(&path, &AssuranceManifestOptions::default()).unwrap();
    let second = assurance_manifest::generate(&path, &AssuranceManifestOptions::default()).unwrap();
    cleanup(&path);
    assert_eq!(first, second);
}

#[test]
fn precondition_and_postcondition_obligations_are_runtime_guarded() {
    let path = write_temp(DECLARED_SOURCE);
    let envelope =
        assurance_manifest::generate(&path, &AssuranceManifestOptions::default()).unwrap();
    cleanup(&path);
    let payload = payload_of(&envelope);
    let obligations = payload["obligations"].as_array().unwrap();
    for obligation in obligations {
        let kind = obligation["kind"].as_str().unwrap();
        let classification = obligation["classification"].as_str().unwrap();
        match kind {
            "precondition" | "postcondition" => assert_eq!(classification, "runtime_guarded"),
            "ownership_parameter" => assert_eq!(classification, "compiler_proved"),
            other => panic!("unexpected kind {other}"),
        }
    }
}

#[test]
fn obligation_ids_are_stable_across_a_pure_formatting_change() {
    let compact = "module app.assurance;\n@id(\"app.assurance.identity\")\nfn identity(value: i64) -> i64 ensures result == value { value }\n@id(\"app.assurance.main\")\nfn main() -> i64 { 0 }\n";
    let spaced = "module app.assurance;\n\n\n@id(\"app.assurance.identity\")\nfn identity(value: i64) -> i64\n    ensures result == value\n{\n    value\n}\n\n@id(\"app.assurance.main\")\nfn main() -> i64 {\n    0\n}\n";
    let compact_path = write_temp(compact);
    let spaced_path = write_temp(spaced);
    let compact_envelope =
        assurance_manifest::generate(&compact_path, &AssuranceManifestOptions::default()).unwrap();
    let spaced_envelope =
        assurance_manifest::generate(&spaced_path, &AssuranceManifestOptions::default()).unwrap();
    cleanup(&compact_path);
    cleanup(&spaced_path);
    let compact_ids: Vec<String> = payload_of(&compact_envelope)["obligations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["id"].as_str().unwrap().to_owned())
        .collect();
    let spaced_ids: Vec<String> = payload_of(&spaced_envelope)["obligations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["id"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(compact_ids, spaced_ids);
}

#[test]
fn hostile_reordered_obligations_are_rejected() {
    let path = write_temp(DECLARED_SOURCE);
    let envelope =
        assurance_manifest::generate(&path, &AssuranceManifestOptions::default()).unwrap();
    cleanup(&path);
    let mut payload = payload_of(&envelope);
    let obligations = payload["obligations"].as_array_mut().unwrap();
    obligations.reverse();
    let tampered = rewrap(&serde_json::to_string(&payload).unwrap());
    let error = verify_envelope(&tampered).expect_err("reordered obligations must fail closed");
    assert_eq!(error.code, "SPX-Z103");
}

#[test]
fn hostile_duplicate_obligation_ids_are_rejected() {
    let path = write_temp(DECLARED_SOURCE);
    let envelope =
        assurance_manifest::generate(&path, &AssuranceManifestOptions::default()).unwrap();
    cleanup(&path);
    let mut payload = payload_of(&envelope);
    let obligations = payload["obligations"].as_array_mut().unwrap();
    let first = obligations[0].clone();
    obligations.insert(0, first);
    let tampered = rewrap(&serde_json::to_string(&payload).unwrap());
    let error = verify_envelope(&tampered).expect_err("duplicate obligation ids must fail closed");
    assert_eq!(error.code, "SPX-Z103");
}

#[test]
fn hostile_forged_classification_is_rejected() {
    let path = write_temp(DECLARED_SOURCE);
    let envelope =
        assurance_manifest::generate(&path, &AssuranceManifestOptions::default()).unwrap();
    cleanup(&path);
    let mut payload = payload_of(&envelope);
    let obligations = payload["obligations"].as_array_mut().unwrap();
    // The obligation's own methods are `runtime_guarded`; claim
    // `theorem_proved` instead, a class no method here supports.
    obligations[0]["classification"] = serde_json::Value::String("theorem_proved".to_owned());
    let tampered = rewrap(&serde_json::to_string(&payload).unwrap());
    let error = verify_envelope(&tampered).expect_err("a forged classification must fail closed");
    assert_eq!(error.code, "SPX-Z103");
}

#[test]
fn hostile_dangling_assumption_reference_is_rejected() {
    let path = write_temp(DECLARED_SOURCE);
    let envelope =
        assurance_manifest::generate(&path, &AssuranceManifestOptions::default()).unwrap();
    cleanup(&path);
    let mut payload = payload_of(&envelope);
    let obligations = payload["obligations"].as_array_mut().unwrap();
    obligations[0]["assumption_ids"] = serde_json::json!(["ghost-assumption"]);
    obligations[0]["methods"][0]["assumption_ids"] = serde_json::json!(["ghost-assumption"]);
    let tampered = rewrap(&serde_json::to_string(&payload).unwrap());
    let error =
        verify_envelope(&tampered).expect_err("a dangling assumption reference must fail closed");
    assert_eq!(error.code, "SPX-Z103");
}

#[test]
fn hostile_timed_out_attempt_forged_as_proved_is_rejected_by_the_producer() {
    use assurance_manifest::{
        AssuranceClass, ExternalRecords, MethodRecord, Obligation, ObligationKind,
    };
    let path = write_temp(DECLARED_SOURCE);
    let mut inconclusive =
        MethodRecord::new(AssuranceClass::AttemptInconclusive, "smt-stub", "0.0.0");
    inconclusive.proof_ref = Some("forged-proof".to_owned());
    let options = AssuranceManifestOptions::default().with_external_records(ExternalRecords {
        obligations: vec![Obligation::new(
            ObligationKind::Effect,
            "app.assurance.combine",
            "effect:network",
        )
        .with_method(inconclusive)],
        assumptions: Vec::new(),
    });
    let result = assurance_manifest::generate(&path, &options);
    cleanup(&path);
    let diagnostics = result.expect_err("an inconclusive attempt cannot carry a proof reference");
    assert_eq!(diagnostics[0].code, "SPX-Z101");
}

#[test]
fn hostile_source_drift_is_rejected() {
    let path = write_temp(DECLARED_SOURCE);
    let envelope =
        assurance_manifest::generate(&path, &AssuranceManifestOptions::default()).unwrap();
    // Mutate the source after the manifest was generated.
    std::fs::write(&path, format!("{DECLARED_SOURCE}\n// drifted\n")).unwrap();
    let error = verify_envelope_against_source(&envelope, &path)
        .expect_err("post-generation source drift must fail closed");
    cleanup(&path);
    assert_eq!(error.code, "SPX-Z104");
}

#[test]
fn hostile_wrong_source_path_is_rejected() {
    let path = write_temp(DECLARED_SOURCE);
    let other_path =
        write_temp("module app.other;\n@id(\"app.other.main\")\nfn main() -> i64 { 1 }\n");
    let envelope =
        assurance_manifest::generate(&path, &AssuranceManifestOptions::default()).unwrap();
    cleanup(&path);
    let error = verify_envelope_against_source(&envelope, &other_path)
        .expect_err("binding to an unrelated file's bytes must fail closed");
    cleanup(&other_path);
    assert_eq!(error.code, "SPX-Z104");
}

#[test]
fn delta_reports_added_and_removed_obligations() {
    let base_source = "module app.assurance;\n@id(\"app.assurance.a\")\nfn a(x: i64) -> i64 { x }\n@id(\"app.assurance.b\")\nfn b(x: i64) -> i64 { x }\n@id(\"app.assurance.main\")\nfn main() -> i64 { 0 }\n";
    let candidate_source = "module app.assurance;\n@id(\"app.assurance.a\")\nfn a(x: i64) -> i64 { x }\n@id(\"app.assurance.c\")\nfn c(x: i64) -> i64 { x }\n@id(\"app.assurance.main\")\nfn main() -> i64 { 0 }\n";
    let base_path = write_temp(base_source);
    let candidate_path = write_temp(candidate_source);
    let base =
        assurance_manifest::generate(&base_path, &AssuranceManifestOptions::default()).unwrap();
    let candidate =
        assurance_manifest::generate(&candidate_path, &AssuranceManifestOptions::default())
            .unwrap();
    cleanup(&base_path);
    cleanup(&candidate_path);
    let result = delta(&base, &candidate, None).expect("delta");
    let value: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(value["schema"], "semaprax.assurance-manifest-delta.v1");
    let added: Vec<&str> = value["added"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    let removed: Vec<&str> = value["removed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(added.iter().any(|id| id.contains("app.assurance.c")));
    assert!(removed.iter().any(|id| id.contains("app.assurance.b")));
}

#[test]
fn public_view_redacts_free_text_and_path_but_keeps_classification() {
    let path = write_temp(DECLARED_SOURCE);
    let envelope =
        assurance_manifest::generate(&path, &AssuranceManifestOptions::default()).unwrap();
    let path_text = path.display().to_string();
    cleanup(&path);
    let view = public_view(&envelope).expect("public view");
    let value: serde_json::Value = serde_json::from_str(&view).unwrap();
    assert_eq!(value["schema"], "semaprax.assurance-manifest.v1");
    assert!(value["source"].get("path").is_none() || value["source"]["path"].is_null());
    assert!(!view.contains(path_text.as_str()));
    for obligation in value["obligations"].as_array().unwrap() {
        for method in obligation["methods"].as_array().unwrap() {
            assert!(method["detail"].is_null());
        }
    }
}

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .arg("assurance-manifest")
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn cli_subcommand_prints_the_same_envelope_the_library_generates() {
    let path = write_temp(DECLARED_SOURCE);
    let library_envelope =
        assurance_manifest::generate(&path, &AssuranceManifestOptions::default()).unwrap();
    let output = cli(&[path.to_str().unwrap()]);
    cleanup(&path);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let cli_stdout = String::from_utf8(output.stdout).unwrap();
    // The CLI prints the envelope followed by the process's own trailing
    // newline from `println!`; the library call returns the bytes exactly.
    assert_eq!(cli_stdout, format!("{library_envelope}\n"));
    verify_envelope(&library_envelope).expect("CLI-produced envelope must independently replay");
}

#[test]
fn cli_subcommand_rejects_an_unknown_option() {
    let path = write_temp(DECLARED_SOURCE);
    let output = cli(&[path.to_str().unwrap(), "--bogus"]);
    cleanup(&path);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "unknown assurance-manifest option `--bogus`\n\
         hint: run `semaprax assurance-manifest --help` for usage\n"
    );
}

#[test]
fn cli_subcommand_honors_max_obligations_and_fails_closed_over_budget() {
    let path = write_temp(DECLARED_SOURCE);
    // `DECLARED_SOURCE` derives 7 obligations (see
    // `golden_envelope_digest_is_pinned`); a budget of 1 must be refused
    // with the producer's own budget diagnostic, not silently truncated.
    let output = cli(&[path.to_str().unwrap(), "--max-obligations", "1"]);
    cleanup(&path);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("SPX-Z102"), "{stderr}");
}
