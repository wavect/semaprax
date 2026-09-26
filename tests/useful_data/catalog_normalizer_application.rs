//! Executes the actual catalog-normalizer source's named application tests.
//! The frozen oracle and its cases remain independent of this backend gate.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use semaprax::{
    codegen, format,
    interpreter::{
        evaluate_resolved_owned_data, OwnedDataEvaluationOutcome, OwnedDataValue, MAX_STEPS_LIMIT,
    },
    parse, project,
};

const MAX_REQUEST_BYTES: usize = 65_536;
const MAX_RECORDS: usize = 256;
const SUCCESS_ENVELOPE_MAX_BYTES: usize = 78;
const ENRICHED_FIELD_MAX_BYTES: usize = 16;
const OUTPUT_CAPACITY: usize =
    MAX_REQUEST_BYTES + SUCCESS_ENVELOPE_MAX_BYTES + MAX_RECORDS * ENRICHED_FIELD_MAX_BYTES;

const SOURCE_FILES: &[&str] = &[
    "app.spx",
    "batch.spx",
    "enrichment.spx",
    "limits.spx",
    "record.spx",
    "tests.spx",
];

const PUBLISHED_MANIFESTS: &[&str] = &[
    "basics.json",
    "enrichment.json",
    "errors.json",
    "limits.json",
];

struct PublishedCase {
    name: String,
    input: Vec<u8>,
    expected: Vec<u8>,
    enriched: bool,
}

struct ScratchRoot(PathBuf);

impl ScratchRoot {
    fn new() -> Self {
        #[cfg(windows)]
        let base = std::env::temp_dir();
        #[cfg(not(windows))]
        let base = std::env::temp_dir().canonicalize().unwrap();
        let root = base.join(format!(
            "semaprax-catalog-normalizer-batch-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        Self(root)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for ScratchRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/catalog-normalizer-project")
}

fn application_options() -> project::ProjectExecutionOptions {
    project::ProjectExecutionOptions::new(OUTPUT_CAPACITY, 2_000_000)
        .expect("catalog-normalizer's documented bounded interpreter envelope")
}

fn maximal_output_body() -> Vec<u8> {
    let mut body = Vec::with_capacity(MAX_REQUEST_BYTES);
    for record in 0..MAX_RECORDS {
        if record > 0 {
            body.push(b'\n');
        }
        let id = format!("item-{record:03}");
        let quantity = if record == 0 {
            i64::MAX.to_string()
        } else {
            "0".to_owned()
        };
        let target_line_bytes = if record == 0 { 256 } else { 255 };
        let fixed = format!("{{\"id\":\"{id}\",\"label\":\"\",\"quantity\":{quantity}}}");
        let label_bytes = target_line_bytes - fixed.len();
        let mut encoded_label = "\\u0001".repeat(label_bytes / 6);
        encoded_label.push_str(&"x".repeat(label_bytes % 6));
        let line =
            format!("{{\"id\":\"{id}\",\"label\":\"{encoded_label}\",\"quantity\":{quantity}}}");
        assert_eq!(line.len(), target_line_bytes);
        body.extend_from_slice(line.as_bytes());
    }
    assert_eq!(body.len(), MAX_REQUEST_BYTES);
    body
}

fn copy_fixture(root: &Path, destination: &Path) {
    std::fs::create_dir_all(destination.join("src")).unwrap();
    std::fs::copy(
        root.join("semaprax.toml"),
        destination.join("semaprax.toml"),
    )
    .unwrap();
    for source in SOURCE_FILES {
        std::fs::copy(
            root.join("src").join(source),
            destination.join("src").join(source),
        )
        .unwrap();
    }
}

fn assert_batch_mutant_rejected(
    root: &Path,
    scratch: &Path,
    name: &str,
    expected: &str,
    replacement: &str,
) {
    let mutant = scratch.join(name);
    copy_fixture(root, &mutant);
    let batch_path = mutant.join("src/batch.spx");
    let batch = std::fs::read_to_string(&batch_path).unwrap();
    let broken = batch.replace(expected, replacement);
    assert_ne!(
        broken, batch,
        "{name} negative-control mutation must be applied"
    );
    std::fs::write(&batch_path, broken).unwrap();
    project::with_authenticated_project(&mutant.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        let result = snapshot.execute_test(&application_options())?;
        assert!(
            matches!(result.outcome(), project::ProjectExecutionOutcome::Returned(code) if *code != 0),
            "{name} mutant must return a nonzero test status, not exhaust fuel: {:?}",
            result.outcome()
        );
        Ok(())
    })
    .unwrap();
}

fn run_oracle_with(input: &[u8], enriched: bool) -> Vec<u8> {
    let oracle_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/oracle/catalog_normalizer");
    let mut command = Command::new("python3");
    command.arg("oracle.py");
    if enriched {
        command
            .arg("--enrich")
            .arg("--fixture")
            .arg("fixtures/enrichment.json");
    }
    let mut child = command
        .current_dir(oracle_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start the independent catalog-normalizer oracle");
    child
        .stdin
        .take()
        .expect("piped oracle stdin")
        .write_all(input)
        .expect("write oracle input");
    let output = child.wait_with_output().expect("wait for oracle");
    assert!(
        output.status.success(),
        "oracle failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn run_oracle_bytes(input: &[u8]) -> Vec<u8> {
    run_oracle_with(input, false)
}

fn run_oracle(input: &[u8]) -> serde_json::Value {
    serde_json::from_slice(&run_oracle_bytes(input)).expect("oracle emits one JSON response")
}

fn decode_hex(input: &str) -> Vec<u8> {
    fn nibble(byte: u8) -> u8 {
        match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            b'A'..=b'F' => byte - b'A' + 10,
            _ => panic!("published input_hex contains a non-hex byte"),
        }
    }
    let bytes = input.as_bytes();
    assert_eq!(bytes.len() % 2, 0, "published input_hex has odd length");
    bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| (nibble(pair[0]) << 4) | nibble(pair[1]))
        .collect()
}

/// Loads only the frozen, reviewable published corpus. The hidden corpus is
/// deliberately neither named nor traversed here: it remains a held-back
/// acceptance control rather than implementation-visible test data.
fn published_cases() -> &'static [PublishedCase] {
    static CASES: OnceLock<Vec<PublishedCase>> = OnceLock::new();
    CASES
        .get_or_init(|| {
            let root = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/oracle/catalog_normalizer/cases/published");
            let mut cases = Vec::new();
            for manifest_name in PUBLISHED_MANIFESTS {
                let manifest_path = root.join(manifest_name);
                let manifest: serde_json::Value =
                    serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
                for case in manifest["cases"].as_array().expect("published cases array") {
                    let input = match (case.get("input"), case.get("input_hex")) {
                        (Some(value), None) => value.as_str().unwrap().as_bytes().to_vec(),
                        (None, Some(value)) => decode_hex(value.as_str().unwrap()),
                        _ => panic!("published case must contain exactly one input encoding"),
                    };
                    let expected = case["expected_output"]
                        .as_str()
                        .expect("published expected output")
                        .as_bytes()
                        .to_vec();
                    let enriched = case["enrich"].as_bool().expect("published enrich flag");
                    let name = case["name"]
                        .as_str()
                        .expect("published case name")
                        .to_owned();
                    assert_eq!(
                        run_oracle_with(&input, enriched),
                        expected,
                        "frozen published case {name} no longer matches the live independent oracle"
                    );
                    cases.push(PublishedCase {
                        name,
                        input,
                        expected,
                        enriched,
                    });
                }
            }
            assert_eq!(
                cases.len(),
                36,
                "published catalog-normalizer corpus inventory drifted"
            );
            cases
        })
        .as_slice()
}

#[test]
fn decoded_id_bounds_and_escaped_duplicates_match_the_independent_oracle() {
    let id64 = "x".repeat(64);
    let accepted = format!("{{\"id\":\"{id64}\",\"label\":\"l\",\"quantity\":1}}\n");
    assert_eq!(run_oracle(accepted.as_bytes())["status"], "ok");

    let id65 = "x".repeat(65);
    let oversized = format!("{{\"id\":\"{id65}\",\"label\":\"l\",\"quantity\":1}}\n");
    let oversized_response = run_oracle(oversized.as_bytes());
    assert_eq!(oversized_response["status"], "error");
    assert_eq!(oversized_response["category"], "oversized_input");

    let escaped_duplicate = b"{\"id\":\"dup\",\"label\":\"one\",\"quantity\":1}\n{\"id\":\"d\\u0075p\",\"label\":\"two\",\"quantity\":2}\n";
    let duplicate_response = run_oracle(escaped_duplicate);
    assert_eq!(duplicate_response["status"], "error");
    assert_eq!(duplicate_response["category"], "duplicate_id");
    assert_eq!(duplicate_response["record_index"], 1);
}

// These literals are also exercised by the candidate's named `test_record_parser`
// and `test_duplicate_batch` cases. Running the independent oracle here keeps
// their frozen category/position contract honest without pretending the current
// scalar parser helper already publishes a complete CNORM-042 response envelope.
#[test]
fn record_parser_categories_and_positions_match_the_independent_oracle() {
    for (input, category, record_index, byte_offset) in [
        (b"{\"id\":\"a\",\"label\":\"x\"}\n".as_slice(), "schema", 0, 0),
        (
            b"{\"id\":\"a\",\"label\":\"x\",\"quantity\":\"1\"}\n".as_slice(),
            "schema",
            0,
            33,
        ),
        (
            b"{\"\\u0069d\":\"a\",\"label\":\"x\",\"quantity\":1}\n".as_slice(),
            "schema",
            0,
            1,
        ),
        (b"{\"id\":\"a\n".as_slice(), "malformed_json", 0, 8),
        (
            b"{\"id\":\"a\",\"label\":\"\",\"quantity\":0}\n{\"id\":\"\\u0061\",\"label\":\"\",\"quantity\":0}\n"
                .as_slice(),
            "duplicate_id",
            1,
            6,
        ),
    ] {
        let output = run_oracle(input);
        assert_eq!(output["status"], "error", "input={input:?}");
        assert_eq!(output["category"], category, "input={input:?}");
        assert_eq!(output["record_index"], record_index, "input={input:?}");
        assert_eq!(output["byte_offset"], byte_offset, "input={input:?}");
    }
}

#[test]
fn every_published_input_matches_the_frozen_oracle_byte_for_byte() {
    let _ = published_cases();
}

#[test]
fn maximal_valid_outputs_fit_the_source_bound_exactly() {
    assert_eq!(OUTPUT_CAPACITY, 69_710);
    assert_eq!(
        OUTPUT_CAPACITY.checked_add(1),
        Some(69_711),
        "the +1 rejection boundary must not wrap"
    );
    let source = std::fs::read_to_string(fixture().join("src/app.spx")).unwrap();
    assert!(source.contains("fn output_capacity() -> usize\n{\n    69710usize\n}"));
    assert_eq!(source.matches("bytes_zeroed(69710usize)").count(), 1);
    assert!(source.contains("fn shared_success_response("));
    assert!(source.contains("shared_success_response(body, false)"));
    assert!(source.contains("shared_success_response(body, true)"));

    let body = maximal_output_body();
    let plain = run_oracle_with(&body, false);
    let enriched = run_oracle_with(&body, true);
    assert_eq!(plain.len(), MAX_REQUEST_BYTES + SUCCESS_ENVELOPE_MAX_BYTES);
    assert_eq!(enriched.len(), OUTPUT_CAPACITY);
    assert!(plain.len() <= OUTPUT_CAPACITY);
    assert!(enriched.len() <= OUTPUT_CAPACITY);
}

// Exact canonical lines retained in the Semaprax source's `test_canonical_responses`.
// This proves that its expected bytes are independently derived by the frozen oracle,
// while the three-backend project test below proves that the candidate reaches them.
#[test]
fn canonical_response_literals_match_the_independent_oracle() {
    assert_eq!(
        run_oracle_bytes(b"{\"id\":\"a\",\"label\":\" x \",\"quantity\":1}\n"),
        b"{\"status\":\"ok\",\"count\":1,\"total_quantity\":1,\"records\":[{\"id\":\"a\",\"label\":\"x\",\"quantity\":1}]}\n"
    );
    assert_eq!(
        run_oracle_bytes(
            b"{\"id\":\"a\",\"label\":\"\",\"quantity\":0}\n{\"id\":\"\\u0061\",\"label\":\"\",\"quantity\":0}\n"
        ),
        b"{\"status\":\"error\",\"category\":\"duplicate_id\",\"record_index\":1,\"byte_offset\":6}\n"
    );
}

#[test]
fn enriched_response_literals_match_the_independent_oracle() {
    assert_eq!(
        run_oracle_with(b"{\"id\":\"widget-1\",\"label\":\" l \",\"quantity\":1}\n", true),
        b"{\"status\":\"ok\",\"count\":1,\"total_quantity\":1,\"records\":[{\"id\":\"widget-1\",\"label\":\"l\",\"quantity\":1,\"category\":7}]}\n"
    );
    assert_eq!(
        run_oracle_with(b"{\"id\":\"blocked-vendor\",\"label\":\"l\",\"quantity\":1}\n", true),
        b"{\"status\":\"error\",\"category\":\"provider_denied\",\"record_index\":0,\"byte_offset\":6}\n"
    );
}

#[test]
fn published_256_record_boundary_agrees_with_independent_oracle() {
    let case = published_cases()
        .iter()
        .find(|case| case.name == "records-count-256-boundary-success")
        .expect("published 256-record boundary case");
    let records = case
        .input
        .split(|byte| *byte == b'\n')
        .filter(|record| !record.is_empty())
        .count();
    assert_eq!(records, 256);
    assert!(!case.enriched);
    let root = fixture();
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        let actual = evaluate_resolved_owned_data(
            snapshot.test_program(),
            "catalog_normalizer.app.normalize",
            &case.input,
            MAX_STEPS_LIMIT,
        )?;
        assert_eq!(
            actual.outcome,
            OwnedDataEvaluationOutcome::Returned(OwnedDataValue::Bytes(case.expected.clone())),
            "the 256-record boundary must match the independent oracle"
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn focused_r05_controls_agree_across_interpreter_native_and_core_wasm() {
    // Keep every production app and bundled std source unchanged. The full
    // published Project gate remains separate; this small tests module avoids
    // growing its already capacity-sensitive semantic graph.
    let cases = [
        (
            "canonical-control-escape",
            "{\"id\":\"one\",\"label\":\"\\u0001x\",\"quantity\":1}\n",
            "{\"status\":\"ok\",\"count\":1,\"total_quantity\":1,\"records\":[{\"id\":\"one\",\"label\":\"\\u0001x\",\"quantity\":1}]}\n",
            false,
        ),
        (
            "named-control-fallback",
            "{\"id\":\"one\",\"label\":\"x\\u000Ay\",\"quantity\":1}\n",
            "{\"status\":\"ok\",\"count\":1,\"total_quantity\":1,\"records\":[{\"id\":\"one\",\"label\":\"x\\ny\",\"quantity\":1}]}\n",
            false,
        ),
        (
            "uppercase-hex-fallback",
            "{\"id\":\"one\",\"label\":\"\\u001A\",\"quantity\":1}\n",
            "{\"status\":\"ok\",\"count\":1,\"total_quantity\":1,\"records\":[{\"id\":\"one\",\"label\":\"\\u001a\",\"quantity\":1}]}\n",
            false,
        ),
        (
            "high-nibble-boundary-10",
            "{\"id\":\"one\",\"label\":\"\\u0010\",\"quantity\":1}\n",
            "{\"status\":\"ok\",\"count\":1,\"total_quantity\":1,\"records\":[{\"id\":\"one\",\"label\":\"\\u0010\",\"quantity\":1}]}\n",
            false,
        ),
        (
            "high-nibble-boundary-1f",
            "{\"id\":\"one\",\"label\":\"\\u001f\",\"quantity\":1}\n",
            "{\"status\":\"ok\",\"count\":1,\"total_quantity\":1,\"records\":[{\"id\":\"one\",\"label\":\"\\u001f\",\"quantity\":1}]}\n",
            false,
        ),
        (
            "trimmed-empty-label",
            "{\"id\":\"one\",\"label\":\" \\t \",\"quantity\":1}\n",
            "{\"status\":\"ok\",\"count\":1,\"total_quantity\":1,\"records\":[{\"id\":\"one\",\"label\":\"\",\"quantity\":1}]}\n",
            false,
        ),
        (
            "reordered-field-fallback",
            "{\"quantity\":2,\"id\":\"one\",\"label\":\"l\"}\n",
            "{\"status\":\"ok\",\"count\":1,\"total_quantity\":2,\"records\":[{\"id\":\"one\",\"label\":\"l\",\"quantity\":2}]}\n",
            false,
        ),
        (
            "reordered-quantity-with-string-lookalike",
            concat!(r#"{"quantity":2,"id":"one","label":"look ,\"quantity\":123"}"#, "\n"),
            concat!(r#"{"status":"ok","count":1,"total_quantity":2,"records":[{"id":"one","label":"look ,\"quantity\":123","quantity":2}]}"#, "\n"),
            false,
        ),
        (
            "validated-quantity-19-digit-boundary",
            "{\"id\":\"maximum\",\"label\":\"l\",\"quantity\":9223372036854775807}\n",
            "{\"status\":\"ok\",\"count\":1,\"total_quantity\":9223372036854775807,\"records\":[{\"id\":\"maximum\",\"label\":\"l\",\"quantity\":9223372036854775807}]}\n",
            false,
        ),
        (
            "validated-quantity-zero",
            "{\"id\":\"zero\",\"label\":\"l\",\"quantity\":0}\n",
            "{\"status\":\"ok\",\"count\":1,\"total_quantity\":0,\"records\":[{\"id\":\"zero\",\"label\":\"l\",\"quantity\":0}]}\n",
            false,
        ),
        (
            "escaped-id-duplicate",
            "{\"id\":\"caf\\u00e9\",\"label\":\"l\",\"quantity\":1}\n{\"id\":\"café\",\"label\":\"l\",\"quantity\":1}\n",
            "{\"status\":\"error\",\"category\":\"duplicate_id\",\"record_index\":1,\"byte_offset\":6}\n",
            false,
        ),
        (
            "duplicate-before-overflow",
            "{\"id\":\"same\",\"label\":\"l\",\"quantity\":9223372036854775807}\n{\"id\":\"same\",\"label\":\"l\",\"quantity\":1}\n",
            "{\"status\":\"error\",\"category\":\"duplicate_id\",\"record_index\":1,\"byte_offset\":6}\n",
            false,
        ),
        (
            "enriched-shared-writer",
            "{\"id\":\"widget-1\",\"label\":\" l \",\"quantity\":1}\n",
            "{\"status\":\"ok\",\"count\":1,\"total_quantity\":1,\"records\":[{\"id\":\"widget-1\",\"label\":\"l\",\"quantity\":1,\"category\":7}]}\n",
            true,
        ),
        (
            "empty-plain-phases",
            "",
            "{\"status\":\"ok\",\"count\":0,\"total_quantity\":0,\"records\":[]}\n",
            false,
        ),
        (
            "empty-enriched-phases",
            "",
            "{\"status\":\"ok\",\"count\":0,\"total_quantity\":0,\"records\":[]}\n",
            true,
        ),
        (
            "two-record-phase-transition",
            "{\"id\":\"a\",\"label\":\"x\",\"quantity\":1}\n{\"id\":\"b\",\"label\":\"y\",\"quantity\":2}\n",
            "{\"status\":\"ok\",\"count\":2,\"total_quantity\":3,\"records\":[{\"id\":\"a\",\"label\":\"x\",\"quantity\":1},{\"id\":\"b\",\"label\":\"y\",\"quantity\":2}]}\n",
            false,
        ),
        (
            "mixed-canonical-escapes",
            concat!(r#"{"id":"one","label":"\u0001\n\\x","quantity":1}"#, "\n"),
            concat!(r#"{"status":"ok","count":1,"total_quantity":1,"records":[{"id":"one","label":"\u0001\n\\x","quantity":1}]}"#, "\n"),
            false,
        ),
        (
            "mixed-shrinking-escape-before-escaped-quote",
            concat!(r#"{"id":"one","label":"\u0001\/\"","quantity":1}"#, "\n"),
            concat!(r#"{"status":"ok","count":1,"total_quantity":1,"records":[{"id":"one","label":"\u0001/\"","quantity":1}]}"#, "\n"),
            false,
        ),
    ];
    let root = fixture();
    let scratch = ScratchRoot::new();
    for (group, group_cases) in cases.chunks(5).enumerate() {
        let focused = scratch.path().join(format!("focused-r05-project-{group}"));
        copy_fixture(&root, &focused);
        let mut tests_source = String::from(
        "module catalog_normalizer.tests;\n\
         use function @id(\"catalog_normalizer.app.normalizes-to\") from catalog_normalizer.app as normalizes_to;\n\
         use function @id(\"catalog_normalizer.app.normalizes-enriched-to\") from catalog_normalizer.app as normalizes_enriched_to;\n\
         @id(\"catalog_normalizer.tests.smoke\")\nfn smoke() -> bool { true }\n\
         @id(\"catalog_normalizer.tests.focused-r05\")\nfn test_focused_r05() -> i64\n{\n    let mut failed = 0;\n",
    );
        for (index, (name, input, expected, enriched)) in group_cases.iter().enumerate() {
            assert_eq!(
                run_oracle_with(input.as_bytes(), *enriched),
                expected.as_bytes(),
                "{name} frozen expectation differs from the independent oracle"
            );
            let function = if *enriched {
                "normalizes_enriched_to"
            } else {
                "normalizes_to"
            };
            tests_source.push_str(&format!(
            "    let input_{index} = {};\n    let expected_{index} = {};\n    let input_text_{index} = string_as_str(input_{index});\n    let expected_text_{index} = string_as_str(expected_{index});\n    failed = failed + if {function}(str_as_bytes(input_text_{index}), str_as_bytes(expected_text_{index})) {{ 0 }} else {{ {} }};\n",
            serde_json::to_string(input).unwrap(),
            serde_json::to_string(expected).unwrap(),
            1i64 << index,
        ));
        }
        tests_source.push_str(
        "    failed\n}\n@id(\"catalog_normalizer.tests.main\")\nfn main() -> i64 { test_focused_r05() }\n",
    );
        let tests_path = focused.join("src/tests.spx");
        let (parsed, comments) = semaprax::parse_with_comments(&tests_source, &tests_path).unwrap();
        let canonical = format::comments::canonical_with_comments(&parsed, &comments);
        std::fs::write(&tests_path, canonical).unwrap();
        project::with_authenticated_project(&focused.join("semaprax.toml"), |snapshot| {
            snapshot.check()?;
            for (name, input, expected, enriched) in group_cases {
                let function = if *enriched {
                    "catalog_normalizer.app.normalize-enriched"
                } else {
                    "catalog_normalizer.app.normalize"
                };
                let actual = evaluate_resolved_owned_data(
                    snapshot.test_program(),
                    function,
                    input.as_bytes(),
                    MAX_STEPS_LIMIT,
                )?;
                assert_eq!(
                    actual.outcome,
                    OwnedDataEvaluationOutcome::Returned(OwnedDataValue::Bytes(
                        expected.as_bytes().to_vec()
                    )),
                    "focused case {name} disagreed with the independent oracle on the interpreter"
                );
            }
            let result = snapshot.execute_test(&application_options())?;
            assert_eq!(
                result.outcome(),
                &project::ProjectExecutionOutcome::Returned(0)
            );
            let c = codegen::emit_hir_c(snapshot.test_program()).map_err(|e| vec![e])?;
            for optimization in ["-O0", "-O2"] {
                let c_path = scratch
                    .path()
                    .join(format!("focused-{group}-{optimization}.c"));
                let executable = scratch
                    .path()
                    .join(format!("focused-{group}-{optimization}"));
                std::fs::write(&c_path, &c).unwrap();
                let build = Command::new("clang")
                    .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
                    .arg(&c_path)
                    .arg("-o")
                    .arg(&executable)
                    .output()
                    .unwrap();
                assert!(
                    build.status.success(),
                    "{}",
                    String::from_utf8_lossy(&build.stderr)
                );
                let run = Command::new(&executable).output().unwrap();
                assert!(
                    run.status.success(),
                    "{}",
                    String::from_utf8_lossy(&run.stderr)
                );
                assert_eq!(run.stdout, b"0\n", "focused native {optimization}");
            }
            let wasm_path = scratch.path().join(format!("focused-{group}.wasm"));
            std::fs::write(&wasm_path, snapshot.test_wasm_module()?).unwrap();
            let script = scratch.path().join(format!("focused-{group}.mjs"));
            let mut host = std::fs::read_to_string(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/useful_data/environment_provider_fixture.mjs"),
            )
            .unwrap();
            host.push('\n');
            host.push_str(
                &std::fs::read_to_string(
                    Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("tests/useful_data/catalog_normalizer_application.mjs"),
                )
                .unwrap(),
            );
            std::fs::write(&script, host).unwrap();
            let node = Command::new("node")
                .arg(&script)
                .arg(&wasm_path)
                .output()
                .unwrap();
            assert!(
                node.status.success(),
                "{}",
                String::from_utf8_lossy(&node.stderr)
            );
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn batch_boundaries_and_string_normalization_agree_across_backends() {
    let root = fixture();
    for source in SOURCE_FILES {
        let path = root.join("src").join(source);
        let bytes = std::fs::read_to_string(&path).unwrap();
        let (program, comments) = semaprax::parse_with_comments(&bytes, &path).unwrap();
        assert_eq!(
            format::comments::canonical_with_comments(&program, &comments),
            bytes
        );
    }
    let tests_path = root.join("src/tests.spx");
    let tests_source = std::fs::read_to_string(&tests_path).unwrap();
    let tests = parse(&tests_source, &tests_path).unwrap();
    let named_cases = tests
        .functions
        .iter()
        .filter(|function| function.name.starts_with("test_"))
        .count();
    assert_eq!(
        named_cases, 14,
        "catalog-normalizer application case inventory drifted"
    );
    // This is deliberately one test: each backend lane runs serially against
    // one authenticated snapshot, so the capacity-sensitive graph is never
    // built concurrently merely because libtest has multiple worker threads.
    // It also owns direct application-to-oracle comparisons. Keeping them in
    // this snapshot avoids a second graph construction racing the backend
    // lanes, while checking the actual `normalize` entrypoints rather than
    // only source-authored expected literals.
    let published = published_cases();
    let maximal_body = maximal_output_body();
    let final_start = maximal_body
        .iter()
        .rposition(|byte| *byte == b'\n')
        .unwrap()
        + 1;
    assert!(
        final_start + 6 > 10_000 && maximal_body[final_start..].starts_with(b"{\"id\":\""),
        "maximal valid case must exercise a five-digit absolute id offset"
    );
    let maximal_plain = run_oracle_with(&maximal_body, false);
    let maximal_enriched = run_oracle_with(&maximal_body, true);
    assert_eq!(maximal_enriched.len(), OUTPUT_CAPACITY);
    assert_eq!(
        maximal_plain.len(),
        MAX_REQUEST_BYTES + SUCCESS_ENVELOPE_MAX_BYTES
    );
    let scratch = ScratchRoot::new();
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        for case in published {
            if case.name == "records-count-256-boundary-success" {
                let lines = case
                    .input
                    .split(|byte| *byte == b'\n')
                    .filter(|line| !line.is_empty())
                    .collect::<Vec<_>>();
                assert_eq!(lines.len(), 256, "published 256-record fixture shape");
                assert!(
                    lines.iter().all(|line| line.starts_with(b"{\"id\": ")),
                    "published 256-record fixture must exercise first-key id lookup"
                );
            }
            let function = if case.enriched {
                "catalog_normalizer.app.normalize-enriched"
            } else {
                "catalog_normalizer.app.normalize"
            };
            let actual = evaluate_resolved_owned_data(
                snapshot.test_program(),
                function,
                &case.input,
                MAX_STEPS_LIMIT,
            )?;
            assert_eq!(
                actual.outcome,
                OwnedDataEvaluationOutcome::Returned(OwnedDataValue::Bytes(case.expected.clone())),
                "{function} disagreed with the independent oracle for published case {}",
                case.name
            );
        }
        let single = b"{\"id\":\"one\",\"label\":\"l\",\"quantity\":1}\n";
        for (name, input, enriched) in [
            ("empty-enriched-terminal", &b""[..], true),
            ("one-plain-terminal", &single[..], false),
            ("one-enriched-terminal", &single[..], true),
            (
                "empty-label-plain-terminal",
                &b"{\"id\":\"one\",\"label\":\"\",\"quantity\":1}\n"[..],
                false,
            ),
            (
                "trimmed-empty-label-enriched-terminal",
                &b"{\"id\":\"one\",\"label\":\" \\t \",\"quantity\":1}\n"[..],
                true,
            ),
            (
                "escaped-label-enriched-terminal",
                &b"{\"id\":\"one\",\"label\":\"\\u0001x\",\"quantity\":1}\n"[..],
                true,
            ),
            (
                "named-escaped-label-plain-terminal",
                &b"{\"id\":\"one\",\"label\":\"line\\nnext\",\"quantity\":1}\n"[..],
                false,
            ),
            (
                "quote-backslash-label-plain-terminal",
                &b"{\"id\":\"one\",\"label\":\"q\\\"\\\\z\",\"quantity\":1}\n"[..],
                false,
            ),
            (
                "unicode-escaped-label-enriched-terminal",
                &b"{\"id\":\"one\",\"label\":\"caf\\u00e9\",\"quantity\":1}\n"[..],
                true,
            ),
            (
                "surrogate-label-enriched-terminal",
                &b"{\"id\":\"one\",\"label\":\"\\ud83d\\ude00\",\"quantity\":1}\n"[..],
                true,
            ),
            (
                "raw-utf8-label-plain-terminal",
                "{\"id\":\"one\",\"label\":\"café\",\"quantity\":1}\n".as_bytes(),
                false,
            ),
            (
                "reordered-quantity-fallback",
                &b"{\"quantity\":2,\"id\":\"one\",\"label\":\"l\"}\n"[..],
                false,
            ),
            (
                "spaced-quantity-suffix-fallback",
                &b"{\"id\":\"one\",\"label\":\"l\",\"quantity\": 2 }\n"[..],
                true,
            ),
        ] {
            let function = if enriched {
                "catalog_normalizer.app.normalize-enriched"
            } else {
                "catalog_normalizer.app.normalize"
            };
            let expected = run_oracle_with(input, enriched);
            let actual = evaluate_resolved_owned_data(
                snapshot.test_program(),
                function,
                input,
                MAX_STEPS_LIMIT,
            )?;
            assert_eq!(
                actual.outcome,
                OwnedDataEvaluationOutcome::Returned(OwnedDataValue::Bytes(expected)),
                "{name} disagreed with the independent oracle"
            );
        }
        // Exercise the duplicate walk at its late-byte boundary and through
        // both decoded escape paths. These use the same admitted application
        // entrypoint and independent oracle as the published corpus.
        let prefix = "x".repeat(63);
        let late_difference = format!(
            "{{\"id\":\"{prefix}a\",\"label\":\"l\",\"quantity\":1}}\n{{\"id\":\"{prefix}b\",\"label\":\"l\",\"quantity\":1}}\n"
        );
        let overlong_id = "x".repeat(65);
        let overlong_input = format!(
            "{{\"id\":\"{overlong_id}\",\"label\":\"l\",\"quantity\":1}}\n"
        );
        for (name, input, category) in [
            ("late-64-byte-difference", late_difference.into_bytes(), None),
            (
                "plain-65-byte-id-rejected",
                overlong_input.into_bytes(),
                Some("oversized_input"),
            ),
            (
                "unicode-escape-equals-raw",
                "{\"id\":\"caf\\u00e9\",\"label\":\"l\",\"quantity\":1}\n{\"id\":\"café\",\"label\":\"l\",\"quantity\":1}\n"
                    .as_bytes()
                    .to_vec(),
                Some("duplicate_id"),
            ),
            (
                "surrogate-pair-equals-raw",
                "{\"id\":\"\\uD83D\\uDE00\",\"label\":\"l\",\"quantity\":1}\n{\"id\":\"😀\",\"label\":\"l\",\"quantity\":1}\n"
                    .as_bytes()
                    .to_vec(),
                Some("duplicate_id"),
            ),
            (
                "reordered-spaced-id-fallback",
                "{ \"label\":\"l\", \"quantity\":1, \"id\":\"same\"}\n{\"id\":\"same\",\"label\":\"l\",\"quantity\":1}\n"
                    .as_bytes()
                    .to_vec(),
                Some("duplicate_id"),
            ),
            (
                "late-duplicate-multidigit-offset",
                "{\"id\":\"first\",\"label\":\"l\",\"quantity\":1}\n{\"id\":\"second\",\"label\":\"l\",\"quantity\":1}\n{\"id\":\"third\",\"label\":\"l\",\"quantity\":1}\n{\"id\":\"first\",\"label\":\"l\",\"quantity\":1}\n"
                    .as_bytes()
                    .to_vec(),
                Some("duplicate_id"),
            ),
            (
                "escaped-id-key-stays-schema",
                "{\"\\u0069d\":\"a\",\"label\":\"l\",\"quantity\":1}\n"
                    .as_bytes()
                    .to_vec(),
                Some("schema"),
            ),
            (
                "malformed-key-precedes-schema",
                "{\"i\\q\":\"a\",\"label\":\"l\",\"quantity\":1}\n"
                    .as_bytes()
                    .to_vec(),
                Some("malformed_json"),
            ),
            (
                "duplicate-quantity-key-precedes-suffix-lookup",
                "{\"id\":\"a\",\"quantity\":1,\"label\":\"l\",\"quantity\":2}\n"
                    .as_bytes()
                    .to_vec(),
                Some("schema"),
            ),
            (
                "duplicate-precedes-total-overflow",
                "{\"id\":\"same\",\"label\":\"l\",\"quantity\":9223372036854775807}\n{\"id\":\"same\",\"label\":\"l\",\"quantity\":1}\n"
                    .as_bytes()
                    .to_vec(),
                Some("duplicate_id"),
            ),
        ] {
            let expected = run_oracle_with(&input, false);
            if let Some(category) = category {
                let response: serde_json::Value = serde_json::from_slice(&expected).unwrap();
                assert_eq!(response["category"], category, "oracle category for {name}");
            }
            let actual = evaluate_resolved_owned_data(
                snapshot.test_program(),
                "catalog_normalizer.app.normalize",
                &input,
                MAX_STEPS_LIMIT,
            )?;
            assert_eq!(
                actual.outcome,
                OwnedDataEvaluationOutcome::Returned(OwnedDataValue::Bytes(expected)),
                "catalog-normalizer application disagreed with the oracle for {name}"
            );
        }
        let denied_input = b"{\"id\":\"widget-1\",\"label\":\"l\",\"quantity\":1}\n{\"id\":\"blocked-vendor\",\"label\":\"l\",\"quantity\":1}\n{\"id\":\"later\",\"label\":\"l\",\"quantity\":1}\n";
        let denied_expected = run_oracle_with(denied_input, true);
        let denied_response: serde_json::Value =
            serde_json::from_slice(&denied_expected).unwrap();
        assert_eq!(denied_response["category"], "provider_denied");
        assert_eq!(denied_response["record_index"], 1);
        let denied_actual = evaluate_resolved_owned_data(
            snapshot.test_program(),
            "catalog_normalizer.app.normalize-enriched",
            denied_input,
            MAX_STEPS_LIMIT,
        )?;
        assert_eq!(
            denied_actual.outcome,
            OwnedDataEvaluationOutcome::Returned(OwnedDataValue::Bytes(denied_expected)),
            "terminal provider failure after an accepted id must win before a later record"
        );
        for (function, expected) in [
            ("catalog_normalizer.app.normalize", maximal_plain),
            (
                "catalog_normalizer.app.normalize-enriched",
                maximal_enriched,
            ),
        ] {
            let actual = evaluate_resolved_owned_data(
                snapshot.test_program(),
                function,
                &maximal_body,
                MAX_STEPS_LIMIT,
            )?;
            assert_eq!(
                actual.outcome,
                OwnedDataEvaluationOutcome::Returned(OwnedDataValue::Bytes(expected)),
                "{function} did not emit its maximal valid response exactly"
            );
        }
        snapshot.check()?;
        {
            let result = snapshot.execute_test(&application_options())?;
            assert_eq!(
                result.outcome(),
                &project::ProjectExecutionOutcome::Returned(0),
                "catalog-normalizer application tests failed on the interpreter"
            );
        }

        {
            let c = codegen::emit_hir_c(snapshot.test_program()).map_err(|e| vec![e])?;
            for optimization in ["-O0", "-O2"] {
                let c_path = scratch.path().join(format!("tests-{optimization}.c"));
                let executable = scratch.path().join(format!("tests-{optimization}"));
                std::fs::write(&c_path, &c).unwrap();
                let build = Command::new("clang")
                    .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
                    .arg(&c_path)
                    .arg("-o")
                    .arg(&executable)
                    .output()
                    .unwrap();
                assert!(
                    build.status.success(),
                    "{}",
                    String::from_utf8_lossy(&build.stderr)
                );
                let run = Command::new(&executable).output().unwrap();
                assert!(
                    run.status.success(),
                    "{}",
                    String::from_utf8_lossy(&run.stderr)
                );
                assert_eq!(run.stdout, b"0\n");
            }
        }

        {
            let wasm_path = scratch.path().join("tests.wasm");
            let wasm = snapshot.test_wasm_module()?;
            std::fs::write(&wasm_path, &wasm).unwrap();
            drop(wasm);
            let script = scratch.path().join("tests.mjs");
            let mut host = std::fs::read_to_string(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/useful_data/environment_provider_fixture.mjs"),
            )
            .unwrap();
            host.push('\n');
            host.push_str(
                &std::fs::read_to_string(
                    Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("tests/useful_data/catalog_normalizer_application.mjs"),
                )
                .unwrap(),
            );
            std::fs::write(&script, host).unwrap();
            let node = Command::new("node")
                .arg(&script)
                .arg(&wasm_path)
                .output()
                .unwrap();
            assert!(
                node.status.success(),
                "{}",
                String::from_utf8_lossy(&node.stderr)
            );
        }
        Ok(())
    })
    .unwrap();

    // A candidate that counts the terminal LF as a record must fail the
    // frozen CNORM-001 case; success above cannot come from an inert harness.
    assert_batch_mutant_rejected(
        &root,
        scratch.path(),
        "terminal-lf-mutant",
        "if only_line { 0usize } else { count }",
        "count",
    );

    // CNORM-005 is not covered by the terminal-LF control. Deliberately
    // accepting every line must make the raw malformed-sequence case fail.
    assert_batch_mutant_rejected(
        &root,
        scratch.path(),
        "utf8-mutant",
        "line_utf8_end_range(body, start, end) == end - start",
        "true",
    );

    // CNORM-015's boundary is likewise independent of record splitting: a
    // checked-total predicate that never reports overflow must be caught.
    assert_batch_mutant_rejected(
        &root,
        scratch.path(),
        "total-mutant",
        "total > 9223372036854775807 - quantity",
        "false",
    );
}
