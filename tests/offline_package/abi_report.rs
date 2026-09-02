//! Executable evidence for Canonical ABI Report v1 (`semaprax.abi-report.v1`).
//!
//! Pins canonical golden bytes, exercises the CLI contract and exit codes,
//! proves byte-level agreement with both real backend projections (native C11
//! prototypes and Core-Wasm scalar-export signatures), covers every exclusion
//! reason, and verifies fail-closed behavior including per-digest-field tamper
//! rejection. No C compiler, Node runtime, or target execution is involved.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use semaprax::abi_report::{self, AbiReportOptions};
use semaprax::{codegen, graph, parse, verify, wasm};
use sha2::{Digest as _, Sha256};
use wasmparser::{ExternalKind, Parser, Payload, TypeRef, ValType};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn write_temp(source: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-abi-report-evidence-{}-{}.spx",
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

const MEANING_PATH: &str = "examples/meaning.spx";
const CALCULATOR_PATH: &str = "examples/calculator.spx";

/// Golden envelope digest over the exact library bytes for the canonical
/// calculator example, selecting the two functions that exercise every
/// portable mapping cell (`i64` parameters plus `bool` parameter and result),
/// emitted through the relative repository path so the fixture is
/// machine-independent.
#[test]
fn golden_envelope_digest_is_pinned() {
    let options = AbiReportOptions::new(
        vec![
            "calculator.not".to_owned(),
            "calculator.is-negative".to_owned(),
        ],
        64 * 1024,
    )
    .expect("valid options");
    let envelope = abi_report::generate(Path::new(CALCULATOR_PATH), &options).expect("envelope");
    assert_eq!(
        sha256_hex(envelope.as_bytes()),
        "sha256:3834116477d6641e2e9c07d9e0e38432509e3e7cddd4391a9b0d8b080166636a"
    );
}

/// A second pinned KAT over the minimal single-function example.
#[test]
fn golden_meaning_envelope_digest_is_pinned() {
    let options =
        AbiReportOptions::new(vec!["math.add".to_owned()], 64 * 1024).expect("valid options");
    let envelope = abi_report::generate(Path::new(MEANING_PATH), &options).expect("envelope");
    assert_eq!(
        sha256_hex(envelope.as_bytes()),
        "sha256:353ec3be6d7618538694bac6a7878ccd27c6aee152d78232dff745c4243592dd"
    );
}

#[test]
fn cli_double_run_is_byte_identical() {
    let args = [
        "abi-report",
        CALCULATOR_PATH,
        "--function",
        "calculator.not",
    ];
    let (first_code, first_out, _) = cli(&args);
    let (second_code, second_out, _) = cli(&args);
    assert_eq!(first_code, 0);
    assert_eq!(second_code, 0);
    assert_eq!(first_out, second_out);
    assert!(first_out.contains("\"schema\":\"semaprax.abi-report.v1\""));
    assert!(first_out.ends_with("}\n"));
}

#[test]
fn cli_rejects_bad_invocations() {
    // Missing required --function.
    let (code, _, _) = cli(&["abi-report", CALCULATOR_PATH]);
    assert_eq!(code, 2);
    // Unknown option.
    let (code, _, _) = cli(&["abi-report", CALCULATOR_PATH, "--functions", "x"]);
    assert_eq!(code, 2);
    // Empty selection token.
    let (code, _, _) = cli(&["abi-report", CALCULATOR_PATH, "--function", ""]);
    assert_eq!(code, 2);
    // Duplicate --max-bytes.
    let (code, _, _) = cli(&[
        "abi-report",
        CALCULATOR_PATH,
        "--max-bytes",
        "4096",
        "--max-bytes",
        "4096",
    ]);
    assert_eq!(code, 2);
    // Unknown selection target fails after admission as a diagnostic.
    let (code, _, _) = cli(&["abi-report", CALCULATOR_PATH, "--function", "nope"]);
    assert_eq!(code, 1);
    // Byte-budget exhaustion is fail-closed.
    let (code, _, err) = cli(&[
        "abi-report",
        CALCULATOR_PATH,
        "--function",
        "calculator.not",
        "--max-bytes",
        "1024",
    ]);
    assert_eq!(code, 1);
    assert!(err.contains("SPX-A203"));
}

#[test]
fn every_requested_function_gets_an_admission_or_exclusion_record() {
    let owned_only = AbiReportOptions::new(
        vec![
            "buffer.consume".to_owned(),
            "buffer.inspect".to_owned(),
            "buffer.pipeline".to_owned(),
        ],
        64 * 1024,
    )
    .unwrap();
    let envelope =
        abi_report::generate(Path::new("examples/ownership.spx"), &owned_only).expect("envelope");
    assert!(envelope.contains("\"admitted\":0,\"excluded\":3"));
    assert!(envelope.contains("\"reason\":\"unsupported_parameter_mode\""));
    assert!(envelope.contains("\"functions\":[]"));
    abi_report::verify_envelope(&envelope).expect("verified");

    // Unknown targets are hard errors even alongside valid ones.
    let mixed = AbiReportOptions::new(
        vec!["buffer.consume".to_owned(), "missing.one".to_owned()],
        64 * 1024,
    )
    .unwrap();
    let errors = abi_report::generate(Path::new("examples/ownership.spx"), &mixed)
        .expect_err("unknown selection must fail closed");
    assert!(errors.iter().any(|item| item.code == "SPX-A202"));
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

@id("probe.wide")
fn wide(label: string) -> string { label }

@id("probe.narrow")
fn narrow(value: i64) -> string { "x" }

@id("app.main")
fn main() -> i64 { 7 }

@id("buffer.type")
resource Buffer {
    @id("buffer.type.drop")
    drop trivial;
}
"#;
    let path = write_temp(source);
    let options = AbiReportOptions::new(
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
    let envelope = abi_report::generate(&path, &options).expect("all-excluded envelope");
    assert!(envelope.contains("\"reason\":\"generic_function\""));
    assert!(envelope.contains("\"reason\":\"declared_effects\""));
    assert!(envelope.contains("\"reason\":\"unsupported_parameter_mode\""));
    assert!(envelope.contains("\"reason\":\"unsupported_parameter_type\""));
    assert!(envelope.contains("\"reason\":\"unsupported_result_type\""));
    assert!(envelope.contains("\"admitted\":0,\"excluded\":5"));
    cleanup(&path);
}

#[test]
fn automatic_identity_functions_are_excluded() {
    let source = r#"
module test.probe;

fn helper(value: i64) -> i64 { value + 1 }

@id("app.main")
fn main() -> i64 { helper(0) }
"#;
    let path = write_temp(source);
    let options = AbiReportOptions::new(vec!["helper".to_owned()], 64 * 1024).unwrap();
    let envelope = abi_report::generate(&path, &options).expect("envelope");
    assert!(envelope.contains("\"reason\":\"automatic_identity\""));
    cleanup(&path);
}

/// The portable mapping in the report must equal the signatures the Wasm
/// backend actually emits for the same program and the same selected stable
/// identities, down to the raw export symbol names.
#[test]
fn canonical_mapping_matches_the_emitted_wasm_module() {
    const SELECTED: &[&str] = &[
        "calculator.divide",
        "calculator.is-negative",
        "calculator.multiply",
        "calculator.not",
        "calculator.subtract",
        "calculator.add",
    ];
    let mut tokens: Vec<String> = SELECTED.iter().map(|item| (*item).to_owned()).collect();
    tokens.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    let options = AbiReportOptions::new(tokens.clone(), 64 * 1024).unwrap();
    let envelope = abi_report::generate(Path::new(CALCULATOR_PATH), &options).expect("envelope");
    let report = abi_report::verify_envelope(&envelope).expect("verified");
    assert_eq!(report.functions.len(), 6);
    assert!(report
        .functions
        .windows(2)
        .all(|pair| pair[0].stable_id.as_bytes() <= pair[1].stable_id.as_bytes()));

    let source = std::fs::read_to_string(CALCULATOR_PATH).unwrap();
    let program = parse(&source, Path::new(CALCULATOR_PATH)).unwrap();
    assert!(verify::verify(&program).is_empty());
    let module = wasm::emit_module_with_scalar_exports(&program, &tokens).expect("module");

    let mut types: Vec<(Vec<ValType>, Vec<ValType>)> = Vec::new();
    let mut imported_functions = 0usize;
    let mut function_types: Vec<u32> = Vec::new();
    let mut exports: Vec<(String, u32)> = Vec::new();
    for payload in Parser::new(0).parse_all(&module) {
        match payload.unwrap() {
            Payload::TypeSection(section) => {
                types.extend(section.into_iter_err_on_gc_types().map(|ty| {
                    let ty = ty.unwrap();
                    (ty.params().to_vec(), ty.results().to_vec())
                }))
            }
            Payload::ImportSection(section) => {
                imported_functions += section
                    .into_imports()
                    .filter_map(|import| match import.unwrap().ty {
                        TypeRef::Func(_) => Some(()),
                        _ => None,
                    })
                    .count();
            }
            Payload::FunctionSection(section) => {
                function_types.extend(section.into_iter().map(Result::unwrap));
            }
            Payload::ExportSection(section) => exports.extend(
                section
                    .into_iter()
                    .map(Result::unwrap)
                    .filter(|export| export.kind == ExternalKind::Func)
                    .map(|export| (export.name.to_owned(), export.index)),
            ),
            _ => {}
        }
    }
    let mut export_index = std::collections::BTreeMap::new();
    for (name, index) in exports {
        export_index.insert(name, index);
    }

    fn text(value: ValType) -> &'static str {
        match value {
            ValType::I64 => "i64",
            ValType::I32 => "i32",
            other => panic!("unexpected core value type: {other:?}"),
        }
    }

    for function in &report.functions {
        let index = *export_index
            .get(function.wasm_export.as_str())
            .unwrap_or_else(|| panic!("module is missing export {}", function.wasm_export));
        let type_index = function_types[(index - imported_functions as u32) as usize];
        let (params, results) = types[type_index as usize].clone();
        let observed_params: Vec<&'static str> = params.iter().map(|value| text(*value)).collect();
        let observed_results: Vec<String> = results
            .iter()
            .map(|value| text(*value).to_owned())
            .collect();
        assert_eq!(observed_params, function.wasm_parameters);
        assert_eq!(observed_results, vec![function.wasm_result.clone()]);
    }
}

/// Reported native sizes, alignments, and prototype lines must equal the
/// checked compiler layouts and the exact native projection bytes.
#[test]
fn native_facts_match_the_checked_layouts_and_native_projection() {
    let options = AbiReportOptions::new(
        vec![
            "calculator.not".to_owned(),
            "calculator.is-negative".to_owned(),
            "app.main".to_owned(),
        ],
        64 * 1024,
    )
    .unwrap();
    let envelope = abi_report::generate(Path::new(CALCULATOR_PATH), &options).expect("envelope");
    assert!(envelope.contains(
        "\"type\":\"i64\",\"c_type\":\"int64_t\",\"size_bytes\":8,\"align_bytes\":8,\"mode\":\"value\""
    ));
    assert!(envelope.contains(
        "\"result\":{\"type\":\"bool\",\"c_type\":\"bool\",\"size_bytes\":1,\"align_bytes\":1,\"mode\":\"value\"}"
    ));
    assert!(envelope.contains("\"target\":\"Native64\""));
    assert!(envelope.contains("\"parameter_passing\":\"by-value copy\""));
    assert!(envelope.contains("\"returns\":\"spx_status_token\""));
    assert!(envelope.contains("\"context_parameter\":\"struct spx_context *spx_ctx\""));
    assert!(envelope.contains("\",\"result_written_at\":\"final success commit\""));
    assert!(envelope.contains("\"bool_boundary\":{\"parameters\":\"trap_unless_canonical_0_or_1\",\"result\":\"trap_unless_canonical_0_or_1\"},\"copy_behavior\":\"copy\"}"));

    let source = std::fs::read_to_string(CALCULATOR_PATH).unwrap();
    let program = parse(&source, Path::new(CALCULATOR_PATH)).unwrap();
    let native = codegen::emit_c(&program).expect("native projection");
    let report = abi_report::verify_envelope(&envelope).expect("verified");
    for function in &report.functions {
        assert!(
            native
                .lines()
                .any(|line| line.trim_end() == function.native_signature),
            "reported prototype must appear verbatim in the native projection"
        );
    }
}

#[test]
fn verify_envelope_round_trips_function_summaries() {
    let options = AbiReportOptions::new(
        vec![
            "calculator.not".to_owned(),
            "calculator.is-negative".to_owned(),
        ],
        64 * 1024,
    )
    .unwrap();
    let envelope = abi_report::generate(Path::new(CALCULATOR_PATH), &options).expect("envelope");
    let report = abi_report::verify_envelope(&envelope).expect("verified");
    assert_eq!(report.functions.len(), 2);
    assert_eq!(report.functions[0].stable_id, "calculator.is-negative");
    assert_eq!(
        report.functions[0].native_symbol,
        "spx_decl_63616c63756c61746f722e69732d6e65676174697665"
    );
    assert_eq!(
        report.functions[0].native_signature,
        "static __attribute__((unused)) spx_status_token \
spx_decl_63616c63756c61746f722e69732d6e65676174697665(struct spx_context *spx_ctx, int64_t, \
bool *spx_result_out);"
    );
    assert_eq!(
        report.functions[0].wasm_export,
        "spx_scalar_63616c63756c61746f722e69732d6e65676174697665"
    );
    assert_eq!(report.functions[0].wasm_parameters, vec!["i64"]);
    assert_eq!(report.functions[0].wasm_result, "i32");
    assert_eq!(report.functions[1].stable_id, "calculator.not");
    assert_eq!(report.functions[1].wasm_parameters, vec!["i32"]);
    assert_eq!(report.functions[1].wasm_result, "i32");

    // Re-verification of the same bytes replays identically.
    let replay = abi_report::verify_envelope(&envelope).expect("replay");
    assert_eq!(report, replay);
}

#[test]
fn verify_envelope_rejects_every_digest_field_tamper() {
    let options =
        AbiReportOptions::new(vec!["calculator.not".to_owned()], 64 * 1024).expect("options");
    let envelope = abi_report::generate(Path::new(CALCULATOR_PATH), &options).expect("envelope");

    // Native-signature digest field.
    let tampered_native = envelope.replace(
        "(struct spx_context *spx_ctx, bool,",
        "(struct spx_context *spx_ctx, bool ,",
    );
    assert_ne!(tampered_native, envelope);
    assert!(abi_report::verify_envelope(&tampered_native).is_err());

    // Canonical-mapping digest field (any mapping byte).
    let tampered_canonical = envelope.replace("\"results\":[\"i32\"]", "\"results\":[\"i33\"]");
    assert_ne!(tampered_canonical, envelope);
    assert!(abi_report::verify_envelope(&tampered_canonical).is_err());

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
    assert!(abi_report::verify_envelope(&tampered_outer).is_err());

    // Spliced payload member invalidates the outer digest over exact bytes.
    let spliced = format!(
        "{}{}",
        &envelope[..envelope.len() - 1],
        ",\"injected\":true}"
    );
    assert!(abi_report::verify_envelope(&spliced).is_err());

    // Structural damage.
    let truncated = envelope[..envelope.len() - 4].to_owned();
    assert!(abi_report::verify_envelope(&truncated).is_err());
    assert!(abi_report::verify_envelope("not json").is_err());
    assert!(abi_report::verify_envelope("[]").is_err());
}

#[test]
fn budget_exhaustion_fails_closed_without_truncation() {
    let path = write_temp(
        "module test.probe;\n@id(\"probe.one\")\nfn one(value: i64) -> i64 { value }\n@id(\"probe.two\")\nfn two(flag: bool) -> bool { flag }\n@id(\"app.main\")\nfn main() -> i64 { 0 }\n",
    );
    let options = AbiReportOptions::new(
        vec!["probe.one".to_owned(), "probe.two".to_owned()],
        graph::MIN_AGENT_CONTEXT_BYTES,
    )
    .unwrap();
    let outcome = abi_report::generate(&path, &options);
    let errors = outcome.expect_err("tiny budgets must fail closed");
    assert!(
        errors.iter().any(|item| item.code == "SPX-A203"),
        "expected the byte-budget diagnostic"
    );
    cleanup(&path);
}
