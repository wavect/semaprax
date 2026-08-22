//! Executable evidence for C Header Emission v1 (`semaprax.c-header.v1`).
//!
//! Pins canonical golden bytes, exercises the CLI contract and exit codes,
//! proves include-guard stability rules, and verifies fail-closed behavior.
//! No C compiler, subprocess toolchain, or target execution is involved.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use semaprax::c_header::{self, CHeaderOptions};
use sha2::{Digest as _, Sha256};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn write_temp(source: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-c-header-evidence-{}-{}.spx",
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
        CHeaderOptions::new(vec!["math.add".to_owned()], 64 * 1024).expect("valid options");
    let envelope = c_header::generate(Path::new(MEANING_PATH), &options).expect("envelope");
    let digest = sha256_hex(envelope.as_bytes());
    assert_eq!(
        digest,
        "sha256:db9a7d7ac9525174e03d8b4f5e7d18513805fa18a33325eb71fa391529ea7d85"
    );
}

/// The bare header bytes are path-independent: the same program emits the
/// same header regardless of where the source lives, and the bytes match the
/// pinned golden digest for the two admitted functions.
#[test]
fn golden_header_digest_is_pinned_and_path_independent() {
    let options = CHeaderOptions::new(
        vec!["math.add".to_owned(), "app.main".to_owned()],
        64 * 1024,
    )
    .expect("valid options");
    let from_examples = c_header::header_text(Path::new(MEANING_PATH), &options).expect("header");

    let copied = write_temp(&std::fs::read_to_string(MEANING_PATH).unwrap());
    let from_temp = c_header::header_text(&copied, &options).expect("header");
    cleanup(&copied);

    assert_eq!(from_examples, from_temp);
    assert_eq!(
        sha256_hex(from_examples.as_bytes()),
        "sha256:a172cbf2f1912a4834873dfc6bf13475805b13b045895e1de131d2ddc979ac6a"
    );
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    )
}

#[test]
fn cli_double_run_is_byte_identical() {
    let args = ["c-header", MEANING_PATH, "--function", "app.main"];
    let (first_code, first_out, _) = cli(&args);
    let (second_code, second_out, _) = cli(&args);
    assert_eq!(first_code, 0);
    assert_eq!(second_code, 0);
    assert_eq!(first_out, second_out);
    assert!(first_out.contains("\"schema\":\"semaprax.c-header.v1\""));
}

#[test]
fn cli_emit_header_prints_bare_header_bytes() {
    let args = [
        "c-header",
        MEANING_PATH,
        "--function",
        "math.add",
        "--emit-header",
    ];
    let (code, out, _) = cli(&args);
    assert_eq!(code, 0);
    assert!(out.starts_with("/*\n"));
    assert!(out.ends_with("#endif\n"));
    assert!(out.contains("spx_decl_6d6174682e616464"));
}

#[test]
fn cli_rejects_bad_invocations() {
    // Missing required --function.
    let (code, _, _) = cli(&["c-header", MEANING_PATH]);
    assert_eq!(code, 2);
    // Unknown option.
    let (code, _, _) = cli(&["c-header", MEANING_PATH, "--functions", "x"]);
    assert_eq!(code, 2);
    // Unknown selection target fails after admission as a diagnostic.
    let (code, _, _) = cli(&["c-header", MEANING_PATH, "--function", "nope"]);
    assert_eq!(code, 1);
    // Byte-budget exhaustion is fail-closed.
    let (code, _, err) = cli(&[
        "c-header",
        MEANING_PATH,
        "--function",
        "math.add",
        "--max-bytes",
        "1024",
    ]);
    assert_eq!(code, 1);
    assert!(err.contains("SPX-D103"));
}

#[test]
fn every_requested_function_gets_an_admission_or_exclusion_record() {
    let mixed = CHeaderOptions::new(
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
    let errors = c_header::generate(Path::new("examples/ownership.spx"), &mixed)
        .expect_err("unknown selection must fail closed");
    assert!(errors.iter().any(|item| item.code == "SPX-D102"));

    let owned_only = CHeaderOptions::new(
        vec![
            "buffer.consume".to_owned(),
            "buffer.inspect".to_owned(),
            "buffer.pipeline".to_owned(),
        ],
        64 * 1024,
    )
    .unwrap();
    let envelope =
        c_header::generate(Path::new("examples/ownership.spx"), &owned_only).expect("envelope");
    assert!(envelope.contains("\"admitted\":0,\"excluded\":3"));
    assert!(envelope.contains("\"reason\":\"unsupported_parameter_mode\""));
    let header = c_header::verify_envelope(&envelope).expect("verified");
    assert!(!header.contains("static __attribute__((unused))"));
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
        let options = CHeaderOptions::new(vec![selection.to_owned()], 64 * 1024).expect("options");
        let header = c_header::header_text(&path, &options).expect("header");
        cleanup(&path);
        header
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

#[test]
fn verify_envelope_rejects_tampered_payloads() {
    let options = CHeaderOptions::new(vec!["math.add".to_owned()], 64 * 1024).expect("options");
    let envelope = c_header::generate(Path::new(MEANING_PATH), &options).expect("envelope");
    let spliced = format!(
        "{}{}",
        &envelope[..envelope.len() - 1],
        ",\"injected\":true}"
    );
    assert!(c_header::verify_envelope(&spliced).is_err());
}
