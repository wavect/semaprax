//! Executable evidence for Plugin Manifest Projection v1
//! (`semaprax.plugin-manifest.v1`).
//!
//! Pins canonical golden envelope digests, exercises every admission and
//! exclusion reason, proves cross-consistency with both
//! `semaprax capability-manifest` (identical required-capabilities
//! derivation) and `semaprax abi-report` (byte-equal native symbols and
//! signatures), verifies determinism, fail-closed budget behavior, CLI exit
//! codes, source-drift rejection, and tamper rejection per digest field
//! including forged-but-re-signed envelopes caught by closed replay. No
//! Component Model runtime or packaging, no host loading or lifecycle, no
//! versioning negotiation, no resource-limit enforcement, no hostile-plugin
//! execution testing, and no target execution is involved.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use semaprax::abi_report::AbiReportOptions;
use semaprax::capability_manifest::CapabilityManifestOptions;
use semaprax::plugin_manifest::{self, PluginManifestOptions};
use semaprax::{abi_report, capability_manifest};
use sha2::{Digest as _, Sha256};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

const PAYLOAD_DIGEST_DOMAIN: &[u8] = b"semaprax.plugin-manifest.payload.v1\0";
const EXPORT_SIGNATURE_DIGEST_DOMAIN: &[u8] = b"semaprax.plugin-manifest.export-signature.v1\0";

fn write_temp(source: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-plugin-manifest-evidence-{}-{}.spx",
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

/// A module exercising every nontrivial descriptor input at once: module
/// permits, declared function effects, an interface-import effect, and an
/// unconsumed interface permit that stays checked-but-not-required exactly
/// like Build Capability Manifest v1 treats it. Every function is excluded
/// from the export inventory, so the module needs no native projection.
const CAPABILITY_SOURCE: &str = r#"
module test.gadget;

permit { filesystem.write, network.read }

@id("gadget.buffer")
resource Buffer {
    @id("gadget.buffer.drop")
    drop import "gadget.buffer.finalize";
}

@id("gadget.host")
interface GadgetHost permits { filesystem.write, secrets.token } {
    @id("gadget.buffer.finalize")
    import fn finalize(buffer: own Buffer) -> unit
        effects { filesystem.write }
        failure infallible
        consumes buffer always;
}

@id("gadget.fetch")
fn fetch(value: i64) -> i64 uses { network.read } { value }

fn main() -> i64 { 0 }
"#;

const MEANING_PATH: &str = "examples/meaning.spx";
const CALCULATOR_PATH: &str = "examples/calculator.spx";

/// Golden envelope digest over the exact library bytes for the minimal
/// two-function example, emitted through the relative repository path so
/// the fixture is machine-independent.
#[test]
fn golden_meaning_envelope_digest_is_pinned() {
    let options = PluginManifestOptions::default();
    let envelope = plugin_manifest::generate(Path::new(MEANING_PATH), &options).expect("envelope");
    assert!(envelope.contains("\"schema\":\"semaprax.plugin-manifest.v1\""));
    assert!(envelope.contains(
        "\"plugin\":{\"name\":\"examples.meaning\",\
\"identity\":\"sha256:2a806a2bffbd7996ef65403daa8011af87a1a268b6069b53ff0499980e828fd8\",\
\"version\":\"2a806a2bffbd7996\"}"
    ));
    assert!(envelope.contains(
        "\"descriptor\":{\"functions_total\":2,\"exports_admitted\":2,\
\"exports_excluded\":0}"
    ));
    assert_eq!(
        sha256_hex(envelope.as_bytes()),
        "sha256:135e4320d5a777eee4167ee43eb5dc75802d0460b8fefd94ff4602e7c5a105f9"
    );
}

/// A second pinned KAT over the whole-module inventory of the calculator
/// example.
#[test]
fn golden_calculator_envelope_digest_is_pinned() {
    let options = PluginManifestOptions::default();
    let envelope =
        plugin_manifest::generate(Path::new(CALCULATOR_PATH), &options).expect("envelope");
    assert!(envelope.contains(
        "\"descriptor\":{\"functions_total\":7,\"exports_admitted\":7,\"exports_excluded\":0}"
    ));
    assert!(envelope.contains(
        "\"required_capabilities\":{\"filesystem\":\"none\",\"home\":\"none\",\
\"network\":\"none\",\"process\":\"none\",\"secrets\":\"none\"}"
    ));
    assert_eq!(
        sha256_hex(envelope.as_bytes()),
        "sha256:5b70733d21c171280c236377e1c30bdd02b7aeda4a70e5ca6cf940cfb447f957"
    );
}

#[test]
fn generation_is_deterministic() {
    let options = PluginManifestOptions::default();
    let first = plugin_manifest::generate(Path::new(CALCULATOR_PATH), &options).expect("first");
    let second = plugin_manifest::generate(Path::new(CALCULATOR_PATH), &options).expect("second");
    assert_eq!(first, second);
}

#[test]
fn exports_carry_interface_facts_and_exact_native_signatures() {
    let options = PluginManifestOptions::default();
    let envelope = plugin_manifest::generate(Path::new(MEANING_PATH), &options).expect("envelope");
    let value: serde_json::Value = serde_json::from_str(&envelope).unwrap();
    let exports = value["payload"]["exports"].as_array().unwrap();
    assert_eq!(exports.len(), 2);
    assert_eq!(exports[0]["stable_id"], "app.main");
    assert_eq!(exports[0]["parameters"].as_array().unwrap().len(), 0);
    assert_eq!(exports[0]["result"], "i64");
    assert_eq!(exports[0]["requires"].as_array().unwrap().len(), 0);
    assert_eq!(exports[0]["ensures"], serde_json::json!(["result == 42"]));
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

    let verified = plugin_manifest::verify_envelope(&envelope).expect("verified");
    assert_eq!(verified.name, "examples.meaning");
    assert_eq!(verified.exports.len(), 2);
    assert_eq!(verified.exports[1].stable_id, "math.add");
    let replay = plugin_manifest::verify_envelope(&envelope).expect("replay");
    assert_eq!(verified, replay);
    plugin_manifest::verify_envelope_against_source(&envelope, Path::new(MEANING_PATH))
        .expect("source binding holds while bytes are unchanged");
}

#[test]
fn plugin_identity_fields_follow_the_module_metadata_conventions() {
    let options = PluginManifestOptions::default();
    let envelope =
        plugin_manifest::generate(Path::new(CALCULATOR_PATH), &options).expect("envelope");
    let value: serde_json::Value = serde_json::from_str(&envelope).unwrap();
    let plugin = &value["payload"]["plugin"];
    // The name comes from the module declaration convention.
    assert_eq!(plugin["name"], "examples.calculator");
    // The identity is the domain-separated source digest...
    let identity = plugin["identity"].as_str().unwrap();
    assert!(identity.starts_with("sha256:"));
    // ...and the version is exactly its leading hex characters.
    assert_eq!(
        plugin["version"].as_str().unwrap(),
        &identity[7..7 + semaprax::plugin_manifest::VERSION_HEX_CHARS]
    );
}

#[test]
fn required_capabilities_mirror_the_capability_manifest_derivation() {
    let path = write_temp(CAPABILITY_SOURCE);
    let options = PluginManifestOptions::default();
    let envelope = plugin_manifest::generate(&path, &options).expect("plugin envelope");

    // Exactly the module permits plus declared function/import effects drive
    // the section; tokens are sorted and deduplicated.
    assert!(
        envelope.contains("\"capability_tokens\":[\"filesystem.write\",\"network.read\"]"),
        "unexpected capability inventory"
    );
    assert!(envelope.contains(
        "\"required_capabilities\":{\"filesystem\":\"declared\",\"home\":\"none\",\
\"network\":\"declared\",\"process\":\"none\",\"secrets\":\"none\"}"
    ));

    // The section equals the ambient-authority section that Build Capability
    // Manifest v1 embeds for the same program.
    let capability_envelope =
        capability_manifest::generate(&path, &CapabilityManifestOptions::default())
            .expect("capability envelope");
    let plugin_value: serde_json::Value = serde_json::from_str(&envelope).unwrap();
    let capability_value: serde_json::Value = serde_json::from_str(&capability_envelope).unwrap();
    assert_eq!(
        plugin_value["payload"]["required_capabilities"],
        capability_value["payload"]["ambient_authority"]
    );

    // An unconsumed interface permit (`secrets.token`) stays checked but
    // never flips its domain, exactly like Build Capability Manifest v1.
    assert_eq!(
        capability_value["payload"]["ambient_authority"]["secrets"],
        "none"
    );

    plugin_manifest::verify_envelope(&envelope).expect("verified");
    cleanup(&path);
}

#[test]
fn out_of_vocabulary_capabilities_fail_generation_closed() {
    let path = write_temp(
        "module test.probe;\npermit { audit.log }\n\n@id(\"probe.one\")\nfn one(value: i64) -> i64\n    uses { audit.log }\n{ value }\n\nfn main() -> i64 uses { audit.log } { one(0) }\n",
    );
    let outcome = plugin_manifest::generate(&path, &PluginManifestOptions::default());
    let errors = outcome.expect_err("out-of-vocabulary capabilities must fail closed");
    assert!(
        errors.iter().any(|item| item.code == "SPX-N102"),
        "expected the closed-vocabulary diagnostic"
    );
    cleanup(&path);
}

#[test]
fn every_admission_and_exclusion_reason_is_reachable() {
    let source = r#"
module test.probe;
permit { network.read }

@id("probe.generic")
fn pick<T>(value: T) -> T { value }

@id("probe.effectful")
fn effectful(value: i64) -> i64 uses { network.read } { value }

@id("probe.borrowed")
fn borrowed(target: borrow Buffer, amount: i64) -> i64 { amount }

@id("probe.wide")
fn wide(ratio: f64) -> f64 { ratio }

@id("probe.narrow")
fn narrow(value: i64) -> f64 { 1.0 }

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
        plugin_manifest::generate(&path, &PluginManifestOptions::default()).expect("envelope");
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
    let verified = plugin_manifest::verify_envelope(&envelope).expect("verified");
    assert!(verified.exports.is_empty());

    // An out-of-vocabulary exclusion reason fails the closed-vocabulary
    // replay even when the outer digest was re-minted around the forgery.
    let foreign_reason =
        envelope.replace("\"reason\":\"generic_function\"", "\"reason\":\"magic\"");
    assert_ne!(foreign_reason, envelope);
    let error = plugin_manifest::verify_envelope(&remint_digest(&foreign_reason))
        .expect_err("foreign reason must fail replay");
    assert_eq!(error.code, "SPX-N104");
    cleanup(&path);
}

#[test]
fn verify_envelope_rejects_every_digest_field_tamper() {
    let options = PluginManifestOptions::default();
    let envelope = plugin_manifest::generate(Path::new(MEANING_PATH), &options).expect("envelope");

    // Export-signature digest field: a signature-text mutation first breaks
    // the outer digest ...
    let tampered_signature = envelope.replace("*spx_result_out);", "*spx_result_out );");
    assert_ne!(tampered_signature, envelope);
    assert!(plugin_manifest::verify_envelope(&tampered_signature).is_err());

    // ... and even a consistently re-signed envelope is caught by the inner
    // export-signature digest replay.
    let resigned = remint_digest(&tampered_signature);
    let error = plugin_manifest::verify_envelope(&resigned)
        .expect_err("re-signed signature mutation must fail replay");
    assert_eq!(error.code, "SPX-N104");

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
    assert!(plugin_manifest::verify_envelope(&tampered_outer).is_err());

    // Plugin version field: a raw mutation breaks the outer digest ...
    const VERSION_KEY: &str = "\"version\":\"2a806a2bffbd7996\"";
    assert_eq!(envelope.matches(VERSION_KEY).count(), 1);
    let forged_version = envelope.replace(VERSION_KEY, "\"version\":\"ffffffffffffffff\"");
    assert_ne!(forged_version, envelope);
    assert!(plugin_manifest::verify_envelope(&forged_version).is_err());

    // ... and a re-signed version forgery is caught by the internal
    // identity/version consistency replay.
    let error = plugin_manifest::verify_envelope(&remint_digest(&forged_version))
        .expect_err("re-signed version forgery must fail replay");
    assert_eq!(error.code, "SPX-N104");

    // Contract text mutation breaks the exact payload bytes.
    let tampered_contract = envelope.replace("left >= 0", "left >= 1");
    assert_ne!(tampered_contract, envelope);
    assert!(plugin_manifest::verify_envelope(&tampered_contract).is_err());

    // Spliced payload member invalidates the outer digest over exact bytes.
    let spliced = format!(
        "{}{}",
        &envelope[..envelope.len() - 1],
        ",\"injected\":true}"
    );
    assert!(plugin_manifest::verify_envelope(&spliced).is_err());

    // Structural damage and foreign schemas.
    let truncated = envelope[..envelope.len() - 4].to_owned();
    assert!(plugin_manifest::verify_envelope(&truncated).is_err());
    assert!(plugin_manifest::verify_envelope("not json").is_err());
    assert!(plugin_manifest::verify_envelope("[]").is_err());
    let foreign_schema = envelope.replace(
        "\"schema\":\"semaprax.plugin-manifest.v1\"",
        "\"schema\":\"semaprax.foreign.v1\"",
    );
    assert_ne!(foreign_schema, envelope);
    assert!(plugin_manifest::verify_envelope(&foreign_schema).is_err());
}

#[test]
fn re_signed_closed_section_forgeries_fail_replay() {
    let options = PluginManifestOptions::default();
    let envelope = plugin_manifest::generate(Path::new(MEANING_PATH), &options).expect("envelope");

    // A forged capability flip cannot claim an undeclared domain even with a
    // consistent outer digest: the section must equal its re-derivation from
    // the listed tokens.
    let forged_capability =
        envelope.replace("\"filesystem\":\"none\"", "\"filesystem\":\"declared\"");
    assert_ne!(forged_capability, envelope);
    let error = plugin_manifest::verify_envelope(&remint_digest(&forged_capability))
        .expect_err("undeclared capability flip must fail replay");
    assert_eq!(error.code, "SPX-N104");

    // An out-of-vocabulary token cannot be smuggled into the closed
    // inventory either.
    let smuggled_token = envelope.replace(
        "\"capability_tokens\":[]",
        "\"capability_tokens\":[\"audit.write\"]",
    );
    assert_ne!(smuggled_token, envelope);
    assert!(plugin_manifest::verify_envelope(&remint_digest(&smuggled_token)).is_err());

    // The resource-limits section is canonical; any declared limit or
    // changed shape fails replay.
    let forged_limit = envelope
        .replace("\"fuel\":null", "\"fuel\":1024")
        .replace("\"memory_bytes\":null", "\"memory_bytes\":4096");
    assert_ne!(forged_limit, envelope);
    assert!(plugin_manifest::verify_envelope(&remint_digest(&forged_limit)).is_err());

    // The unavailable-section list is closed; removing or adding entries
    // fails even with a consistent outer digest.
    let dropped_section = envelope.replace(",\"versioning_negotiation\"", "");
    assert_ne!(dropped_section, envelope);
    assert!(plugin_manifest::verify_envelope(&remint_digest(&dropped_section)).is_err());
    let added_section = envelope.replace(
        "\"unavailable_sections\":[",
        "\"unavailable_sections\":[\"sandboxing\",",
    );
    assert_ne!(added_section, envelope);
    assert!(plugin_manifest::verify_envelope(&remint_digest(&added_section)).is_err());

    // Count forgery fails the counts-vs-listings replay.
    let forged_count = envelope.replace("\"exports_admitted\":2,", "\"exports_admitted\":3,");
    assert_ne!(forged_count, envelope);
    assert!(plugin_manifest::verify_envelope(&remint_digest(&forged_count)).is_err());
}

#[test]
fn listed_exports_equal_what_abi_report_admits() {
    let options = PluginManifestOptions::default();
    let envelope =
        plugin_manifest::generate(Path::new(CALCULATOR_PATH), &options).expect("envelope");
    let verified = plugin_manifest::verify_envelope(&envelope).expect("verified");
    let mut tokens: Vec<String> = verified
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
    for export in &verified.exports {
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
fn source_drift_between_generation_and_validation_fails_closed() {
    let path = write_temp(
        "module test.probe;\n@id(\"probe.one\")\nfn one(value: i64) -> i64 { value }\n@id(\"app.main\")\nfn main() -> i64 { 0 }\n",
    );
    let options = PluginManifestOptions::default();
    let first = plugin_manifest::generate(&path, &options).expect("envelope");
    plugin_manifest::verify_envelope_against_source(&first, &path)
        .expect("binding holds while unchanged");
    std::fs::write(&path, "module test.probe;\n").unwrap();

    // The drifted source no longer reproduces the manifest...
    let outcome = plugin_manifest::generate(&path, &options);
    assert!(
        outcome.is_err(),
        "drifted source must not reproduce the manifest"
    );

    // ...and both embedded source digests reject the old envelope against
    // the new bytes.
    let error = plugin_manifest::verify_envelope_against_source(&first, &path)
        .expect_err("drift must fail the source binding");
    assert_eq!(error.code, "SPX-N104");
    cleanup(&path);
}

#[test]
fn budget_exhaustion_fails_closed_without_truncation() {
    let path = write_temp(
        "module test.probe;\n@id(\"probe.one\")\nfn one(value: i64) -> i64 { value }\n@id(\"app.main\")\nfn main() -> i64 { 0 }\n",
    );
    let tiny = PluginManifestOptions::new(semaprax::graph::MIN_AGENT_CONTEXT_BYTES).unwrap();
    let outcome = plugin_manifest::generate(&path, &tiny);
    let errors = outcome.expect_err("tiny budgets must fail closed");
    assert!(
        errors.iter().any(|item| item.code == "SPX-N103"),
        "expected the byte-budget diagnostic"
    );
    cleanup(&path);
}

#[test]
fn unverified_source_fails_closed() {
    let path = write_temp("module test.probe;\nfn broken( { 0 }\n");
    let outcome = plugin_manifest::generate(&path, &PluginManifestOptions::default());
    assert!(outcome.is_err());
    cleanup(&path);
}

#[test]
fn cli_double_run_is_byte_identical() {
    let args = ["plugin-manifest", CALCULATOR_PATH];
    let (first_code, first_out, _) = cli(&args);
    let (second_code, second_out, _) = cli(&args);
    assert_eq!(first_code, 0);
    assert_eq!(second_code, 0);
    assert_eq!(first_out, second_out);
    assert!(first_out.contains("\"schema\":\"semaprax.plugin-manifest.v1\""));
    assert!(first_out.ends_with("}\n"));
}

#[test]
fn cli_rejects_bad_invocations() {
    // Missing file path.
    let (code, _, _) = cli(&["plugin-manifest"]);
    assert_eq!(code, 2);
    // Unknown option.
    let (code, _, err) = cli(&["plugin-manifest", CALCULATOR_PATH, "--functions", "x"]);
    assert_eq!(code, 2);
    assert!(err.contains("unknown plugin-manifest option"));
    // Duplicate option.
    let (code, _, err) = cli(&[
        "plugin-manifest",
        CALCULATOR_PATH,
        "--max-bytes",
        "65536",
        "--max-bytes",
        "65536",
    ]);
    assert_eq!(code, 2);
    assert!(err.contains("duplicate"));
    // Malformed value.
    let (code, _, err) = cli(&["plugin-manifest", CALCULATOR_PATH, "--max-bytes", "-3"]);
    assert_eq!(code, 2);
    assert!(err.contains("canonical nonnegative integer"));
    // Out-of-bounds value.
    let (code, _, err) = cli(&["plugin-manifest", CALCULATOR_PATH, "--max-bytes", "512"]);
    assert_eq!(code, 2);
    assert!(err.contains("SPX-N101"));
    // Missing option value.
    let (code, _, _) = cli(&["plugin-manifest", CALCULATOR_PATH, "--max-bytes"]);
    assert_eq!(code, 2);
    // Byte-budget exhaustion is fail-closed.
    let big = write_temp(
        "module app.big;\n\n@id(\"big.one\")\nfn one(value: i64) -> i64\n    requires value >= 0\n    ensures result == value + 1\n{ value + 1 }\n\n@id(\"app.main\")\nfn main() -> i64 { one(41) }\n",
    );
    let (code, _, err) = cli(&[
        "plugin-manifest",
        big.to_str().unwrap(),
        "--max-bytes",
        "1024",
    ]);
    assert_eq!(code, 1);
    assert!(err.contains("SPX-N103"), "stderr was: {err}");
    cleanup(&big);
    // Unverifiable sources fail closed with exit code 1.
    let bad = write_temp("module test.probe;\nfn broken( { 0 }\n");
    let (code, _, _) = cli(&["plugin-manifest", bad.to_str().unwrap()]);
    assert_eq!(code, 1);
    cleanup(&bad);
}
