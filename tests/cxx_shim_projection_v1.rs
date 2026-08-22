//! Executable evidence for C++ Shim Projection v1 (`semaprax.cxx-shim.v1`).
//!
//! Pins canonical golden bytes, exercises the CLI contract and exit codes,
//! proves include-guard stability rules, rejects independent tampering of
//! every digest field, and verifies fail-closed behavior. No C++ compiler,
//! subprocess toolchain, or target execution is involved.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use semaprax::cxx_shim::{self, CxxShimOptions};
use sha2::{Digest as _, Sha256};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn write_temp(source: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-cxx-shim-evidence-{}-{}.spx",
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

const MEANING_PATH: &str = "examples/meaning.spx";

/// Golden envelope digest over the exact library bytes for the canonical
/// example, selected by stable ID, emitted through the relative repository
/// path so the fixture is machine-independent.
#[test]
fn golden_envelope_digest_is_pinned() {
    let options =
        CxxShimOptions::new(vec!["math.add".to_owned()], 64 * 1024).expect("valid options");
    let envelope = cxx_shim::generate(Path::new(MEANING_PATH), &options).expect("envelope");
    assert_eq!(
        sha256_hex(envelope.as_bytes()),
        "sha256:45a4dd13d1d39381c61c87cb5a25f9885602e02ff595978912e903efd4e18c58"
    );
}

/// The bare fragment bytes are path-independent: the same program emits the
/// same `extern "C"` fragment regardless of where the source lives, and the
/// bytes match the pinned golden digest for the two admitted functions.
#[test]
fn golden_fragment_digest_is_pinned_and_path_independent() {
    let options = CxxShimOptions::new(
        vec!["math.add".to_owned(), "app.main".to_owned()],
        64 * 1024,
    )
    .expect("valid options");
    let from_examples =
        cxx_shim::fragment_text(Path::new(MEANING_PATH), &options).expect("fragment");

    let copied = write_temp(&std::fs::read_to_string(MEANING_PATH).unwrap());
    let from_temp = cxx_shim::fragment_text(&copied, &options).expect("fragment");
    cleanup(&copied);

    assert_eq!(from_examples, from_temp);
    assert_eq!(
        sha256_hex(from_examples.as_bytes()),
        "sha256:e6ef7400120ffb350f21b5a21a9221081d1c82745a0eb0478464622e3b7c7cd6"
    );
    assert!(from_examples.contains("extern \"C\" {"));
    assert!(from_examples.ends_with("\n}\n\n#endif\n"));
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    )
}

fn payload_digest(payload: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"semaprax.cxx-shim.payload.v1\0");
    hasher.update((payload.len() as u64).to_le_bytes());
    hasher.update(payload.as_bytes());
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    )
}

/// Replace the first occurrence of `needle`, failing the test when absent,
/// and return the mutated envelope.
fn spliced(envelope: &str, needle: &str, replacement: &str) -> String {
    let offset = envelope
        .find(needle)
        .unwrap_or_else(|| panic!("anchor `{needle}` must be present"));
    let mut tampered = String::with_capacity(envelope.len() + replacement.len());
    tampered.push_str(&envelope[..offset]);
    tampered.push_str(replacement);
    tampered.push_str(&envelope[offset + needle.len()..]);
    assert_ne!(tampered, envelope);
    tampered
}

/// Flip the first hex digit of the digest value that follows `marker`.
fn flip_hex_after(envelope: &str, marker: &str) -> String {
    let offset = envelope.find(marker).expect("marker present") + marker.len();
    let replacement = if envelope[offset..].starts_with('0') {
        "1"
    } else {
        "0"
    };
    let mut tampered = String::with_capacity(envelope.len());
    tampered.push_str(&envelope[..offset]);
    tampered.push_str(replacement);
    tampered.push_str(&envelope[offset + 1..]);
    assert_ne!(tampered.as_str(), envelope);
    tampered
}

/// Re-sign an envelope whose payload was mutated: recompute the outer byte
/// count and domain-separated payload digest so verification can only fail
/// through the independently replayed inner checks.
fn resign(mut envelope: String) -> String {
    let marker = "\"payload\":";
    let offset = envelope.find(marker).expect("payload member");
    let payload_len = envelope.len() - 1 - (offset + marker.len());
    let digest = payload_digest(&envelope[offset + marker.len()..envelope.len() - 1]);
    let start = envelope.find("\"digest\":\"").expect("digest member");
    let rest = &envelope[start + "\"digest\":\"".len()..];
    let end = rest.find('"').expect("digest value end") + start + "\"digest\":\"".len();
    envelope.replace_range(start..end, &format!("\"digest\":\"{digest}\""));
    let bytes_start = envelope.find("\"bytes\":").expect("bytes member");
    let bytes_rest = &envelope[bytes_start + "\"bytes\":".len()..];
    let bytes_end = bytes_rest.find(',').expect("bytes comma") + bytes_start + "\"bytes\":".len();
    envelope.replace_range(bytes_start..bytes_end, &format!("\"bytes\":{payload_len}"));
    envelope
}

#[test]
fn cli_double_run_is_byte_identical() {
    let args = ["cxx-shim", MEANING_PATH, "--function", "app.main"];
    let (first_code, first_out, _) = cli(&args);
    let (second_code, second_out, _) = cli(&args);
    assert_eq!(first_code, 0);
    assert_eq!(second_code, 0);
    assert_eq!(first_out, second_out);
    assert!(first_out.contains("\"schema\":\"semaprax.cxx-shim.v1\""));
}

#[test]
fn cli_emit_fragment_prints_bare_fragment_bytes() {
    let args = [
        "cxx-shim",
        MEANING_PATH,
        "--function",
        "math.add",
        "--emit-fragment",
    ];
    let (code, out, _) = cli(&args);
    assert_eq!(code, 0);
    assert!(out.starts_with("/*\n"));
    assert!(out.contains("#ifndef SPX_CXX_SHIM_"));
    assert!(out.contains("extern \"C\" {"));
    assert!(out.ends_with("\n}\n\n#endif\n"));
    assert!(out.contains("spx_decl_6d6174682e616464"));
}

#[test]
fn cli_rejects_bad_invocations() {
    // Missing required --function.
    let (code, _, _) = cli(&["cxx-shim", MEANING_PATH]);
    assert_eq!(code, 2);
    // Unknown option.
    let (code, _, _) = cli(&["cxx-shim", MEANING_PATH, "--functions", "x"]);
    assert_eq!(code, 2);
    // Unknown selection target fails after admission as a diagnostic.
    let (code, _, _) = cli(&["cxx-shim", MEANING_PATH, "--function", "nope"]);
    assert_eq!(code, 1);
    // Byte-budget exhaustion is fail-closed.
    let (code, _, err) = cli(&[
        "cxx-shim",
        MEANING_PATH,
        "--function",
        "math.add",
        "--max-bytes",
        "1024",
    ]);
    assert_eq!(code, 1);
    assert!(err.contains("SPX-X103"));
}

#[test]
fn every_requested_function_gets_an_admission_or_exclusion_record() {
    let mixed = CxxShimOptions::new(
        vec![
            "buffer.consume".to_owned(),
            "buffer.inspect".to_owned(),
            "buffer.pipeline".to_owned(),
            "missing.one".to_owned(),
        ],
        64 * 1024,
    )
    .unwrap();
    // Unknown targets are hard errors even alongside valid ones.
    let errors = cxx_shim::generate(Path::new("examples/ownership.spx"), &mixed)
        .expect_err("unknown selection must fail closed");
    assert!(errors.iter().any(|item| item.code == "SPX-X102"));

    let owned_only = CxxShimOptions::new(
        vec![
            "buffer.consume".to_owned(),
            "buffer.inspect".to_owned(),
            "buffer.pipeline".to_owned(),
        ],
        64 * 1024,
    )
    .unwrap();
    let envelope =
        cxx_shim::generate(Path::new("examples/ownership.spx"), &owned_only).expect("envelope");
    assert!(envelope.contains("\"admitted\":0,\"excluded\":3"));
    assert!(envelope.contains("\"reason\":\"unsupported_parameter_mode\""));
    let fragment = cxx_shim::verify_envelope(&envelope).expect("verified");
    assert!(fragment.contains("extern \"C\" {"));
    assert!(!fragment.contains("static __attribute__((unused))"));
}

#[test]
fn every_exclusion_reason_is_reachable_in_one_program() {
    let source = r#"
module test.probe;
permit { io.release }

@id("probe.generic")
fn pick<T>(value: T) -> T { value }

@id("probe.effectful")
fn effectful(value: i64) -> i64 uses { io.release } { value }

@id("probe.borrowed")
fn borrowed(target: borrow Buffer, amount: i64) -> i64 { amount }

@id("probe.wide")
fn wide(ratio: f64) -> f64 { ratio }

@id("probe.narrow")
fn narrow(ratio: i64) -> char { 'x' }

@id("app.main")
fn main() -> i64
    ensures result == 7
{
    7
}

@id("buffer.type")
resource Buffer {
    @id("buffer.type.drop")
    drop trivial;
}
"#;
    let helper = r#"
module test.probe;

fn helper(value: i64) -> i64 { value + 1 }

@id("app.main")
fn main() -> i64
    ensures result == 1
{
    helper(0)
}
"#;
    let path = write_temp(source);
    let options = CxxShimOptions::new(
        vec![
            "probe.generic".to_owned(),
            "probe.effectful".to_owned(),
            "probe.borrowed".to_owned(),
            "probe.wide".to_owned(),
            "probe.narrow".to_owned(),
        ],
        64 * 1024,
    )
    .unwrap();
    let envelope = generate_verified(&path, &options);
    assert!(envelope.contains("\"reason\":\"generic_function\""));
    assert!(envelope.contains("\"reason\":\"declared_effects\""));
    assert!(envelope.contains("\"reason\":\"unsupported_parameter_mode\""));
    assert!(envelope.contains("\"reason\":\"unsupported_parameter_type\""));
    assert!(envelope.contains("\"reason\":\"unsupported_result_type\""));
    assert!(envelope.contains("\"admitted\":0,\"excluded\":5"));
    let fragment = cxx_shim::verify_envelope(&envelope).expect("verified");
    assert!(fragment.contains("extern \"C\" {"));
    assert!(!fragment.contains("static __attribute__((unused))"));
    cleanup(&path);

    let private_path = write_temp(helper);
    let private_options = CxxShimOptions::new(vec!["helper".to_owned()], 64 * 1024).unwrap();
    let private_envelope = generate_verified(&private_path, &private_options);
    assert!(private_envelope.contains("\"reason\":\"automatic_identity\""));
    cleanup(&private_path);
}

fn generate_verified(path: &Path, options: &CxxShimOptions) -> String {
    match cxx_shim::generate(path, options) {
        Ok(envelope) => envelope,
        Err(errors) => panic!(
            "generation must succeed: {:?}",
            errors
                .iter()
                .map(|item| item.message.clone())
                .collect::<Vec<_>>()
        ),
    }
}

#[test]
fn verify_envelope_rejects_each_digest_field_tamper_separately() {
    let options = CxxShimOptions::new(vec!["math.add".to_owned()], 64 * 1024).expect("options");
    let envelope = cxx_shim::generate(Path::new(MEANING_PATH), &options).expect("envelope");

    // Outer wrapper fields.
    let outer_digest = flip_hex_after(&envelope, "\"digest\":\"sha256:");
    assert!(cxx_shim::verify_envelope(&outer_digest).is_err());
    let bytes_marker = "\"bytes\":";
    let bytes_offset = envelope.find(bytes_marker).expect("bytes") + bytes_marker.len();
    let digits = envelope[bytes_offset..].find(',').expect("bytes comma");
    let byte_count = envelope[bytes_offset..bytes_offset + digits]
        .parse::<u64>()
        .unwrap();
    let bytes_tampered = spliced(
        &envelope,
        &format!("\"bytes\":{byte_count}"),
        &format!("\"bytes\":{}", byte_count + 1),
    );
    assert!(cxx_shim::verify_envelope(&bytes_tampered).is_err());
    let schema_tampered = spliced(
        &envelope,
        "\"schema\":\"semaprax.cxx-shim.v1\"",
        "\"schema\":\"semaprax.cxx-shim.v0\"",
    );
    assert!(cxx_shim::verify_envelope(&schema_tampered).is_err());

    // Payload members: any mutation invalidates the authenticated payload.
    let matches_native = spliced(
        &envelope,
        "\"matches_native\":true",
        "\"matches_native\":false",
    );
    assert!(cxx_shim::verify_envelope(&matches_native).is_err());
    // First `"sha256":"` inside the payload is the source snapshot digest.
    let source_digest_tampered = flip_hex_after(&envelope, "\"sha256\":\"sha256:");
    assert!(cxx_shim::verify_envelope(&source_digest_tampered).is_err());
    let declaration_tampered = flip_hex_after(&envelope, "\"declaration_sha256\":\"sha256:");
    assert!(cxx_shim::verify_envelope(&declaration_tampered).is_err());
    let signature_tampered = spliced(&envelope, "spx_decl_", "spx_declX");
    assert!(cxx_shim::verify_envelope(&signature_tampered).is_err());
    let fragment_text_tampered = spliced(
        &envelope,
        "Generated by SEMAPRAX C++ Shim Projection v1",
        "Generated by SEMAPRAX C++ Shim Projection vX",
    );
    assert!(cxx_shim::verify_envelope(&fragment_text_tampered).is_err());

    // Inner layer fires independently: with the outer digest honestly
    // recomputed over the mutated payload, a forged fragment digest or a
    // spliced fragment text still fails replay.
    let forged_inner_digest = resign(flip_hex_after(&envelope, "\"fragment_sha256\":\"sha256:"));
    assert!(cxx_shim::verify_envelope(&forged_inner_digest).is_err());
    let forged_fragment_text = resign(spliced(&envelope, "SPX_CXX_SHIM_", "SPX_CXX_SHIX_"));
    assert!(cxx_shim::verify_envelope(&forged_fragment_text).is_err());
    // The untouched envelope still verifies after all that surgery.
    assert!(cxx_shim::verify_envelope(&envelope).is_ok());

    // Structural truncation and non-JSON input fail closed too.
    let truncated = envelope[..envelope.len() - 4].to_owned();
    assert!(cxx_shim::verify_envelope(&truncated).is_err());
    assert!(cxx_shim::verify_envelope("not json").is_err());
}

#[test]
fn include_guard_follows_the_documented_stability_rules() {
    let base = r#"
module test.guard;

@id("guard.add")
fn add(left: i64, right: i64) -> i64 { left + right }

@id("app.main")
fn main() -> i64 { add(1, 2) }
"#;

    fn guard_of(source: &str, selection: &str) -> String {
        let path = write_temp(source);
        let options = CxxShimOptions::new(vec![selection.to_owned()], 64 * 1024).expect("options");
        let fragment = cxx_shim::fragment_text(&path, &options).expect("fragment");
        cleanup(&path);
        fragment
            .lines()
            .find(|line| line.starts_with("#ifndef "))
            .expect("guard line")
            .to_owned()
    }

    let baseline = guard_of(base, "guard.add");
    // Formatting-only drift keeps the guard byte-identical.
    assert_eq!(
        baseline,
        guard_of(&format!("{base}\n// formatting note\n"), "guard.add")
    );
    // Display-name-only renames keep the guard because identities are stable.
    let renamed_display = base
        .replace("fn add(", "fn sum(")
        .replace("add(1, 2)", "sum(1, 2)");
    assert!(renamed_display.contains("fn sum("));
    assert_eq!(baseline, guard_of(&renamed_display, "guard.add"));
    // Renames that change the admitted stable identity change the guard.
    let renamed_identity = base.replace("@id(\"guard.add\")", "@id(\"guard.sum\")");
    assert_ne!(baseline, guard_of(&renamed_identity, "guard.sum"));
}
