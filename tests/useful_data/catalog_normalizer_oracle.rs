//! Runs the independent Python oracle for the catalog-normalizer acceptance
//! application (SPX-AI-018, GitHub issue #117) and proves, from the Rust
//! side, that it executes, that it passes its own frozen known-answer
//! corpus, and that a deliberately wrong candidate output is REJECTED by the
//! acceptance comparison rather than silently accepted.
//!
//! This module owns no catalog-normalizer application logic of its own.
//! `catalog-normalizer` is not implemented in Semaprax here; that is a
//! separate, later issue (SPX-AI-025, GitHub issue #124). This module exists
//! only to prove the frozen oracle under `tests/oracle/catalog_normalizer/`
//! is real, executable, and discriminating, per the worker contract's
//! negative-control requirement.
//! `docs/CATALOG-NORMALIZER-ORACLE-V1.md` is the normative specification the
//! oracle implements; that document and `tests/oracle/catalog_normalizer/`
//! are frozen and outside this repository's implementation-agent write
//! authority (see the oracle directory's own README for the exact policy).

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn oracle_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/oracle/catalog_normalizer")
}

fn run_oracle(input: &[u8], buggy: Option<&str>, enrich: bool) -> Vec<u8> {
    let mut command = Command::new("python3");
    command.arg("oracle.py");
    if enrich {
        command.arg("--enrich");
    }
    if let Some(mode) = buggy {
        command.args(["--buggy", mode]);
    }
    let mut child = command
        .current_dir(oracle_dir())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect(
            "python3 must be on PATH to run the catalog-normalizer oracle \
             (tests/oracle/catalog_normalizer/oracle.py); this repository's \
             own CI/MSRV harness already requires python3 for scripts/ci-msrv.py",
        );
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(input)
        .expect("write oracle stdin");
    let output = child.wait_with_output().expect("wait on oracle.py");
    assert!(
        output.status.success(),
        "oracle.py exited nonzero: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn load_case(manifest_rel: &str, name: &str) -> (Vec<u8>, Vec<u8>, bool) {
    let raw = std::fs::read_to_string(oracle_dir().join(manifest_rel))
        .unwrap_or_else(|error| panic!("read {manifest_rel}: {error}"));
    let value: serde_json::Value =
        serde_json::from_str(&raw).unwrap_or_else(|error| panic!("parse {manifest_rel}: {error}"));
    let case = value["cases"]
        .as_array()
        .expect("manifest has a cases array")
        .iter()
        .find(|c| c["name"] == name)
        .unwrap_or_else(|| panic!("case {name} must exist in {manifest_rel}"));
    let input: Vec<u8> = if let Some(hex) = case.get("input_hex").and_then(|v| v.as_str()) {
        decode_hex(hex)
    } else {
        case["input"]
            .as_str()
            .unwrap_or_else(|| panic!("case {name} has neither input nor input_hex"))
            .as_bytes()
            .to_vec()
    };
    let expected = case["expected_output"]
        .as_str()
        .unwrap_or_else(|| panic!("case {name} has no expected_output"))
        .as_bytes()
        .to_vec();
    let enrich = case
        .get("enrich")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    (input, expected, enrich)
}

fn decode_hex(hex: &str) -> Vec<u8> {
    assert_eq!(hex.len() % 2, 0, "odd-length hex string");
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("valid hex byte"))
        .collect()
}

/// The oracle's own `--self-test` mode walks the whole frozen corpus
/// (`cases/published/**` and `cases/hidden/**`) plus every entry in
/// `cases/negative_controls.json`, and fails if any known-answer case
/// mismatches or any negative control's buggy output happens to match the
/// correct output. This is the single strongest piece of evidence that the
/// oracle is both correct against its own frozen corpus and discriminating
/// against the five documented anti-pattern bugs from issue #117's
/// checklist. A "0 tests run" result is not evidence, so this test asserts
/// nonzero, substantial counts rather than only a zero exit code.
#[test]
fn oracle_self_test_passes_the_frozen_corpus_and_every_negative_control() {
    let output = Command::new("python3")
        .arg("oracle.py")
        .arg("--self-test")
        .current_dir(oracle_dir())
        .output()
        .expect("python3 must be on PATH to run oracle.py --self-test");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(
        output.status.success(),
        "oracle self-test failed:\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.contains("self-test: OK"),
        "unexpected self-test stdout: {stdout}"
    );

    let open = stdout.find('(').expect("summary reports counts in parens");
    let close = stdout.find(')').expect("summary reports counts in parens");
    let mut numbers = stdout[open + 1..close]
        .split_whitespace()
        .filter_map(|token| token.parse::<u32>().ok());
    let known_answers = numbers.next().expect("known-answer case count");
    let controls = numbers.next().expect("negative-control count");

    assert!(
        known_answers >= 40,
        "expected a substantial frozen known-answer corpus (published + hidden), \
         got {known_answers}"
    );
    assert!(
        controls >= 6,
        "expected one negative control per documented anti-pattern from issue #117 \
         plus the hard-coded-example control, got {controls}"
    );
}

/// Exercises the oracle's single-invocation CLI directly (not through
/// `--self-test`), over one published case, to prove the documented
/// interface (`stdin` in, canonical response bytes on `stdout`, exit 0) is
/// real and not only reachable from the self-test's in-process calls.
#[test]
fn oracle_cli_reproduces_the_frozen_published_example() {
    let (input, expected, enrich) = load_case("cases/published/basics.json", "basic-single-record");
    let actual = run_oracle(&input, None, enrich);
    assert_eq!(actual, expected);
}

/// The negative-control requirement, proven directly at the Rust level
/// rather than only inside the Python self-test: run the SAME input through
/// both the correct oracle and a deliberately wrong candidate
/// (`oracle.py --buggy MODE`, a real runnable implementation of one of
/// issue #117's five named anti-patterns), and require their output bytes
/// to differ. An acceptance harness that compared candidate output against
/// the oracle byte for byte would therefore reject the wrong candidate.
#[test]
fn a_deliberately_wrong_candidate_is_rejected_by_byte_exact_comparison() {
    let (input, expected, enrich) =
        load_case("cases/published/errors.json", "overflow-total-rejected");
    let correct = run_oracle(&input, None, enrich);
    assert_eq!(
        correct, expected,
        "the oracle must match its own frozen answer"
    );
    let buggy = run_oracle(&input, Some("unchecked-total"), enrich);
    assert_ne!(
        correct, buggy,
        "a candidate with an unchecked total-quantity sum must be rejected: \
         its output must differ from the oracle's correct, checked-overflow \
         output"
    );

    let (dup_input, dup_expected, dup_enrich) = load_case(
        "cases/published/errors.json",
        "duplicate-required-key-rejected",
    );
    let dup_correct = run_oracle(&dup_input, None, dup_enrich);
    assert_eq!(dup_correct, dup_expected);
    let dup_buggy = run_oracle(&dup_input, Some("accept-duplicate-keys"), dup_enrich);
    assert_ne!(
        dup_correct, dup_buggy,
        "a candidate that silently accepts a duplicate object key must be rejected"
    );
}

/// A candidate that hard-codes the one published example cannot pass a
/// hidden input variant it has never seen: the "cannot pass by matching
/// only the visible example" acceptance criterion, proven against a real
/// hidden-zone case.
#[test]
fn hardcoding_the_published_example_fails_a_hidden_case() {
    let (hidden_input, hidden_expected, hidden_enrich) = load_case(
        "cases/hidden/boundaries.json",
        "records-count-257-boundary-oversized-rejected",
    );
    let correct = run_oracle(&hidden_input, None, hidden_enrich);
    assert_eq!(correct, hidden_expected);
    let hardcoded = run_oracle(&hidden_input, Some("hardcode-example"), hidden_enrich);
    assert_ne!(
        correct, hardcoded,
        "a candidate that always returns the visible published example's output \
         must be rejected on this hidden input"
    );
}
