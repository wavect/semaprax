//! Executable evidence for Build Capability Manifest v1
//! (`semaprax.capability-manifest.v1`).
//!
//! Pins canonical golden envelope digests, proves the declared-effect
//! inventory and the empty-by-default ambient authority assertion, exercises
//! hostile injection/tamper/drift rejection through independent replay,
//! verifies determinism, byte-budget fail-closed behavior, and CLI exit
//! codes. No sandbox is enforced, no dependency is resolved, no target runs.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use semaprax::capability_manifest::{self, CapabilityManifestOptions};
use sha2::{Digest as _, Sha256};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

const PAYLOAD_DIGEST_DOMAIN: &[u8] = b"semaprax.capability-manifest.payload.v1\0";
const SOURCE_DIGEST_DOMAIN: &[u8] = b"semaprax.capability-manifest.source.v1\0";

fn write_temp(source: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-capability-manifest-evidence-{}-{}.spx",
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

const DECLARED_SOURCE: &str = r#"
module app.capabilities;

permit { filesystem.write, network.read }

@id("app.fetch")
fn fetch(seed: i64) -> i64 uses { network.read } { seed }

@id("app.save")
fn save(value: i64) -> i64 uses { filesystem.write } { value }

@id("app.main")
fn main() -> i64
    ensures result == 7
{ 7 }
"#;

/// Golden envelope digest over the exact library bytes for the canonical
/// effect-free example, emitted through the relative repository path so the
/// fixture is machine-independent.
#[test]
fn golden_envelope_digest_is_pinned() {
    let options = CapabilityManifestOptions::default();
    let envelope =
        capability_manifest::generate(Path::new(MEANING_PATH), &options).expect("envelope");
    assert!(envelope.contains("\"schema\":\"semaprax.capability-manifest.v1\""));
    // A completely effect-free module asserts zero ambient authority.
    assert!(envelope.contains("\"module_permits\":[]"));
    assert!(envelope.contains(
        "\"ambient_authority\":{\"filesystem\":\"none\",\"home\":\"none\",\
\"network\":\"none\",\"process\":\"none\",\"secrets\":\"none\"}"
    ));
    assert_eq!(
        sha256_hex(envelope.as_bytes()),
        "sha256:96fc31f923b6f37207bf5a8593d780b49a7df5e51ac7e4bd8c0af3dd83da1b38"
    );
}

#[test]
fn declared_effects_are_listed_exactly_and_drive_the_ambient_assertion() {
    let path = write_temp(DECLARED_SOURCE);
    let options = CapabilityManifestOptions::default();
    let envelope = capability_manifest::generate(&path, &options).expect("envelope");
    assert!(
        envelope.contains("\"module_permits\":[\"filesystem.write\",\"network.read\"]"),
        "permit inventory must be sorted and exact"
    );
    assert!(envelope.contains(
        "{\"stable_id\":\"app.fetch\",\"name\":\"fetch\",\
\"effects\":[\"network.read\"]}"
    ));
    assert!(envelope.contains(
        "{\"stable_id\":\"app.save\",\"name\":\"save\",\
\"effects\":[\"filesystem.write\"]}"
    ));
    assert!(envelope.contains("{\"stable_id\":\"app.main\",\"name\":\"main\",\"effects\":[]}"));
    assert!(envelope.contains("\"permits_total\":2,\"functions_total\":3,\"imports_total\":0"));
    assert!(
        envelope.contains(
            "\"ambient_authority\":{\"filesystem\":\"declared\",\"home\":\"none\",\
\"network\":\"declared\",\"process\":\"none\",\"secrets\":\"none\"}"
        ),
        "exactly the declared domains may flip to declared"
    );
    capability_manifest::verify_envelope(&envelope).expect("verified");
    cleanup(&path);
}

#[test]
fn generation_is_deterministic() {
    let path = write_temp(DECLARED_SOURCE);
    let options = CapabilityManifestOptions::default();
    let first = capability_manifest::generate(&path, &options).expect("first envelope");
    let second = capability_manifest::generate(&path, &options).expect("second envelope");
    assert_eq!(first, second);
    cleanup(&path);
}

#[test]
fn verify_envelope_accepts_only_genuine_envelopes() {
    let path = write_temp(DECLARED_SOURCE);
    let options = CapabilityManifestOptions::default();
    let envelope = capability_manifest::generate(&path, &options).expect("envelope");
    capability_manifest::verify_envelope(&envelope).expect("genuine envelope verifies");
    capability_manifest::verify_envelope_against_source(&envelope, &path)
        .expect("source binding holds while bytes are unchanged");
    cleanup(&path);

    assert!(capability_manifest::verify_envelope("not json").is_err());
    assert!(capability_manifest::verify_envelope("[]").is_err());
    assert!(capability_manifest::verify_envelope(&format!(
        "{}{}",
        &envelope[..envelope.len() - 1],
        ",\"injected\":true}"
    ))
    .is_err());
    let foreign_schema = envelope.replace(
        "\"schema\":\"semaprax.capability-manifest.v1\"",
        "\"schema\":\"semaprax.foreign.v1\"",
    );
    assert_ne!(foreign_schema, envelope);
    assert!(capability_manifest::verify_envelope(&foreign_schema).is_err());
}

#[test]
fn undeclared_capability_injection_is_rejected() {
    let path = write_temp(DECLARED_SOURCE);
    let options = CapabilityManifestOptions::default();
    let envelope = capability_manifest::generate(&path, &options).expect("envelope");

    let injected = envelope.replace(
        "\"module_permits\":[\"filesystem.write\",\"network.read\"]",
        "\"module_permits\":[\"filesystem.write\",\"network.read\",\"secrets.read\"]",
    );
    assert_ne!(injected, envelope);
    assert!(capability_manifest::verify_envelope(&injected).is_err());

    // Even a consistently re-minted digest cannot smuggle an undeclared
    // capability past the ambient-authority derivation replay: the injected
    // token would have to flip `secrets` to declared as well.
    let reminted = remint_digest(&injected);
    let error = capability_manifest::verify_envelope(&reminted)
        .expect_err("stale ambient section must fail replay");
    assert_eq!(error.code, "SPX-K204");
    cleanup(&path);
}

#[test]
fn out_of_vocabulary_tokens_fail_closed_at_generation_and_replay() {
    let hostile = r#"
module app.hostile;

permit { audit.write }

@id("app.main")
fn main() -> i64 { 7 }
"#;
    let path = write_temp(hostile);
    let options = CapabilityManifestOptions::default();
    let errors = capability_manifest::generate(&path, &options)
        .expect_err("out-of-vocabulary permits must abort generation");
    assert!(
        errors.iter().any(|item| item.code == "SPX-K202"),
        "expected the closed-vocabulary diagnostic"
    );

    // A forged envelope carrying an out-of-vocabulary token fails replay even
    // when its digest was re-minted consistently around the forgery.
    let genuine_path = write_temp(DECLARED_SOURCE);
    let genuine = capability_manifest::generate(&genuine_path, &options).expect("envelope");
    let forged = genuine
        .replace("network.read", "audit.write")
        .replace("\"network\":\"declared\"", "\"network\":\"none\"");
    assert_ne!(forged, genuine);
    let error = capability_manifest::verify_envelope(&remint_digest(&forged))
        .expect_err("forged vocabulary must fail replay");
    assert_eq!(error.code, "SPX-K204");
    cleanup(&genuine_path);

    let tampered = genuine.replace(
        "sha256:",
        "sha256:00000000000000000000000000000000000000000000000000000000000000",
    );
    assert!(capability_manifest::verify_envelope(&tampered).is_err());
    cleanup(&path);
}

#[test]
fn source_drift_after_generation_is_detected_via_the_source_binding() {
    let path = write_temp(DECLARED_SOURCE);
    let options = CapabilityManifestOptions::default();
    let envelope = capability_manifest::generate(&path, &options).expect("envelope");

    // The embedded source digest must equal an independent domain-separated
    // computation over the exact source bytes.
    let source_bytes = std::fs::read(&path).unwrap();
    let mut hasher = Sha256::new();
    hasher.update(SOURCE_DIGEST_DOMAIN);
    hasher.update((source_bytes.len() as u64).to_le_bytes());
    hasher.update(&source_bytes);
    let expected = format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    );
    assert!(envelope.contains(&format!("\"sha256\":\"{expected}\"")));

    let drifted = format!("{DECLARED_SOURCE}\n// drift comment\n");
    std::fs::write(&path, drifted).unwrap();
    let error = capability_manifest::verify_envelope_against_source(&envelope, &path)
        .expect_err("drifted source must fail the binding check");
    assert_eq!(error.code, "SPX-K204");
    cleanup(&path);
}

#[test]
fn byte_budget_exhaustion_fails_closed_without_truncation() {
    let path = write_temp(DECLARED_SOURCE);
    let tiny = CapabilityManifestOptions::new(semaprax::graph::MIN_AGENT_CONTEXT_BYTES).unwrap();
    let outcome = capability_manifest::generate(&path, &tiny);
    let errors = outcome.expect_err("tiny budgets must fail closed");
    assert!(
        errors.iter().any(|item| item.code == "SPX-K203"),
        "expected the byte-budget diagnostic"
    );
    cleanup(&path);
}

#[test]
fn cli_double_run_is_byte_identical() {
    let args = ["capability-manifest", MEANING_PATH];
    let (first_code, first_out, _) = cli(&args);
    let (second_code, second_out, _) = cli(&args);
    assert_eq!(first_code, 0);
    assert_eq!(second_code, 0);
    assert_eq!(first_out, second_out);
    assert!(first_out.ends_with('\n'));
}

#[test]
fn cli_rejects_bad_invocations() {
    // Missing file path.
    let (code, _, _) = cli(&["capability-manifest"]);
    assert_eq!(code, 2);
    // Unknown option.
    let (code, _, err) = cli(&["capability-manifest", MEANING_PATH, "--permits", "x"]);
    assert_eq!(code, 2);
    assert!(err.contains("unknown capability-manifest option"));
    // Duplicate option.
    let (code, _, err) = cli(&[
        "capability-manifest",
        MEANING_PATH,
        "--max-bytes",
        "65536",
        "--max-bytes",
        "65536",
    ]);
    assert_eq!(code, 2);
    assert!(err.contains("duplicate"));
    // Malformed value.
    let (code, _, err) = cli(&["capability-manifest", MEANING_PATH, "--max-bytes", "-3"]);
    assert_eq!(code, 2);
    assert!(err.contains("canonical nonnegative integer"));
    // Out-of-bounds value.
    let (code, _, err) = cli(&["capability-manifest", MEANING_PATH, "--max-bytes", "512"]);
    assert_eq!(code, 2);
    assert!(err.contains("SPX-K201"));
    // Missing option value.
    let (code, _, _) = cli(&["capability-manifest", MEANING_PATH, "--max-bytes"]);
    assert_eq!(code, 2);
    // Byte-budget exhaustion is fail-closed; the smallest legal budget is
    // below the declared-effects envelope size.
    let big = write_temp(DECLARED_SOURCE);
    let (code, _, err) = cli(&[
        "capability-manifest",
        big.to_str().unwrap(),
        "--max-bytes",
        "2048",
    ]);
    assert_eq!(code, 1);
    assert!(err.contains("SPX-K203"));
    cleanup(&big);
    // Out-of-vocabulary module capabilities are fail-closed.
    let hostile = write_temp(
        "module app.hostile;\n\npermit { audit.write }\n\n@id(\"app.main\")\nfn main() -> i64 { 7 }\n",
    );
    let (code, _, err) = cli(&["capability-manifest", hostile.to_str().unwrap()]);
    assert_eq!(code, 1);
    assert!(err.contains("SPX-K202"), "stderr was: {err}");
    cleanup(&hostile);
}
