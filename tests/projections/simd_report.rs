//! Executable evidence for Portable SIMD Eligibility Report v1
//! (`semaprax.simd-report.v1`).
//!
//! Pins canonical golden envelope digests, proves determinism, exercises
//! every function-admission exclusion reason and every per-expression
//! ineligibility reason against real verified programs, verifies lane-width
//! feasibility under the fixed portable lane model, proves byte-level
//! cross-consistency with the real resolved HIR nodes of the same program,
//! verifies fail-closed budget behavior, CLI exit codes, and tamper
//! rejection per digest field including forged-but-re-signed envelopes
//! caught by closed replay. No SIMD codegen or intrinsics are emitted, no
//! SPIR-V/WebGPU/GPU kernels are produced, no autovectorization is claimed,
//! and no target is executed.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use semaprax::hir::{ResolvedExpr, ResolvedExprKind, ResolvedFunction};
use semaprax::simd_report::{self, SimdReportOptions};
use semaprax::{graph, hir, parse, verify};
use sha2::{Digest as _, Sha256};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

const PAYLOAD_DIGEST_DOMAIN: &[u8] = b"semaprax.simd-report.payload.v1\0";
const REGION_DIGEST_DOMAIN: &[u8] = b"semaprax.simd-report.region.v1\0";

fn write_temp(source: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-simd-report-evidence-{}-{}.spx",
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
/// fixture is machine-independent.
#[test]
fn golden_calculator_envelope_digest_is_pinned() {
    let options = SimdReportOptions::default();
    let envelope = simd_report::generate(Path::new(CALCULATOR_PATH), &options).expect("envelope");
    assert!(envelope.contains("\"schema\":\"semaprax.simd-report.v1\""));
    assert!(envelope.contains("\"analysis_scope\":\"pure_straight_line_arithmetic_only\""));
    assert!(envelope.contains(
        "\"module\":{\"name\":\"examples.calculator\",\
\"functions_total\":7,\"functions_admitted\":7,\"functions_excluded\":0}"
    ));
    assert_eq!(
        sha256_hex(envelope.as_bytes()),
        "sha256:be87afb19898fbda7084f77dd947d2ce3b2ad310df4b61ce7fbb014ea229136d"
    );
}

/// A second pinned KAT over the minimal two-function example.
#[test]
fn golden_meaning_envelope_digest_is_pinned() {
    let options = SimdReportOptions::default();
    let envelope = simd_report::generate(Path::new(MEANING_PATH), &options).expect("envelope");
    assert!(envelope.contains(
        "\"module\":{\"name\":\"examples.meaning\",\
\"functions_total\":2,\"functions_admitted\":2,\"functions_excluded\":0}"
    ));
    assert_eq!(
        sha256_hex(envelope.as_bytes()),
        "sha256:d7b2bd4fc871aa9da5d5296c73614f4b304a869916d5d04767a9486b5e5bcf3e"
    );
}

#[test]
fn generation_is_deterministic() {
    let options = SimdReportOptions::default();
    let first = simd_report::generate(Path::new(CALCULATOR_PATH), &options).expect("first");
    let second = simd_report::generate(Path::new(CALCULATOR_PATH), &options).expect("second");
    assert_eq!(first, second);
}

/// The proposed lane width always respects the fixed portable lane model:
/// type ceilings (`i64`/`f64` never exceed 2, `i32`/`f32` reach 4, `u8`
/// reaches 8) and the largest-feasible-first scan over {2, 4, 8} against the
/// operator-plus-leaf element count.
#[test]
fn proposed_widths_respect_the_fixed_lane_model() {
    // u8 ceiling 8 with enough elements (5 operators + 6 leaves >= 8).
    let path = write_temp(
        "module test.probe;\n\
@id(\"app.main\")\n\
fn main() -> i64 { 0 }\n\
@id(\"probe.u8\")\n\
fn wide(a: u8, b: u8, c: u8, d: u8, e: u8, f: u8) -> u8 {\n\
    (a * b + c) * (d - e) + f\n\
}\n\
@id(\"probe.i32\")\n\
fn medium(a: i32, b: i32, c: i32) -> i32 {\n\
    a * b + c\n\
}\n\
@id(\"probe.i32.small\")\n\
fn small(a: i32, b: i32) -> i32 {\n\
    a + b\n\
}\n\
@id(\"probe.f64\")\n\
fn floating(x: f64, y: f64, z: f64) -> f64 {\n\
    x * y - z\n\
}\n\
",
    );
    let envelope = simd_report::generate(&path, &SimdReportOptions::default()).expect("envelope");
    let value: serde_json::Value = serde_json::from_str(&envelope).unwrap();
    let functions = value["payload"]["functions"].as_array().unwrap();
    let widths: Vec<(String, u64)> = functions
        .iter()
        .flat_map(|function| {
            function["regions"]
                .as_array()
                .unwrap()
                .iter()
                .map(move |region| {
                    (
                        format!(
                            "{}:{}",
                            function["stable_id"].as_str().unwrap(),
                            region["root"].as_str().unwrap()
                        ),
                        region["proposed_width"].as_u64().unwrap(),
                    )
                })
        })
        .collect();
    assert!(widths.contains(&(("probe.u8:(a * b + c) * (d - e) + f").to_owned(), 8)));
    assert!(widths.contains(&("probe.i32:a * b + c".to_owned(), 4)));
    assert!(widths.contains(&("probe.i32.small:a + b".to_owned(), 2)));
    // f64 ceiling is 2 even with plenty of elements.
    assert!(widths.contains(&("probe.f64:x * y - z".to_owned(), 2)));
    let report = simd_report::verify_envelope(&envelope).expect("verified");
    assert_eq!(report.functions.len(), 5);
    cleanup(&path);
}

#[test]
fn every_function_exclusion_reason_is_reachable() {
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
fn wrapped(value: i64) -> Wrapper { Wrapper { inner: value } }

fn helper(value: i64) -> i64 { value + 1 }

@id("app.main")
fn main() -> i64 { 0 }

@id("buffer.type")
resource Buffer {
    @id("buffer.type.drop")
    drop trivial;
}

@id("wrapper.type")
record Wrapper {
    @id("wrapper.inner") inner: i64,
}
"#;
    let path = write_temp(source);
    let envelope = simd_report::generate(&path, &SimdReportOptions::default()).expect("envelope");
    for reason in [
        "automatic_identity",
        "generic_function",
        "declared_effects",
        "unsupported_parameter_mode",
        "non_scalar_signature",
    ] {
        assert!(
            envelope.contains(&format!("\"reason\":\"{reason}\"")),
            "missing exclusion reason {reason}"
        );
    }
    assert!(envelope
        .contains("\"functions_total\":6,\"functions_admitted\":1,\"functions_excluded\":5"));
    let report = simd_report::verify_envelope(&envelope).expect("verified");
    assert_eq!(report.functions.len(), 1);
    assert_eq!(report.functions[0].stable_id, "app.main");

    // An out-of-vocabulary exclusion reason fails the closed-vocabulary
    // replay even when the outer digest was re-minted around the forgery.
    let foreign_reason =
        envelope.replace("\"reason\":\"generic_function\"", "\"reason\":\"magic\"");
    assert_ne!(foreign_reason, envelope);
    let error = simd_report::verify_envelope(&remint_digest(&foreign_reason))
        .expect_err("foreign reason must fail replay");
    assert_eq!(error.code, "SPX-V103");
    cleanup(&path);
}

/// Every per-expression ineligibility reason exercised by one real program.
#[test]
fn every_expression_ineligibility_reason_is_reachable() {
    let source = r#"
module test.probe;

@id("pair.type")
record Pair {
    @id("pair.left") left: i64,
    @id("pair.right") right: i64,
}

@id("probe.mix")
fn mix(a: i32, b: i32) -> i32 {
    let mut acc = a * 2i32;
    acc = a + b * 3i32;
    if acc > 0i32 { acc - 1i32 } else { 0i32 }
}

@id("probe.divide")
fn divide(left: i64, right: i64) -> i64
    requires right != 0
{
    left / right + left % 2
}

@id("probe.chars")
fn label(code: char) -> bool {
    code == 'a'
}

@id("probe.computed")
fn computed(value: i64) -> i64 {
    helper(value) + 1
}

@id("probe.helper")
fn helper(value: i64) -> i64 {
    value * 2
}

@id("probe.aggregates")
fn aggregates(p: i64) -> i64 {
    let pair = Pair { left: p, right: p };
    pair.left + 1
}

@id("app.main")
fn main() -> i64 { 0 }
"#;
    let path = write_temp(source);
    let envelope = simd_report::generate(&path, &SimdReportOptions::default()).expect("envelope");
    for reason in [
        "call",
        "contract",
        "division_remainder",
        "bool_mixing",
        "char_operation",
        "mutation_target",
        "computed_operand",
        "control_flow",
        "aggregate_operation",
        "scalar_leaf",
    ] {
        assert!(
            envelope.contains(&format!("\"reason\":\"{reason}\"")),
            "missing ineligibility reason {reason}"
        );
    }
    // Contract clauses are reported verbatim in canonical form, before any
    // body-derived entry.
    assert!(envelope.contains("\"reason\":\"contract\",\"expr\":\"right != 0\""));
    // Assignment stores are mutation targets and are never descended into.
    assert!(envelope.contains("\"reason\":\"mutation_target\",\"expr\":\"a + b * 3i32\""));
    let report = simd_report::verify_envelope(&envelope).expect("verified");
    let mix = report
        .functions
        .iter()
        .find(|function| function.stable_id == "probe.mix")
        .expect("mix analyzed");
    assert_eq!(mix.regions.len(), 2);
    cleanup(&path);
}

#[test]
fn verify_envelope_replays_and_returns_region_summaries() {
    let envelope = simd_report::generate(Path::new(MEANING_PATH), &SimdReportOptions::default())
        .expect("envelope");
    let report = simd_report::verify_envelope(&envelope).expect("verified");
    assert_eq!(report.functions.len(), 2);
    assert_eq!(report.functions[0].stable_id, "app.main");
    assert!(report.functions[0].regions.is_empty());
    assert_eq!(report.functions[1].stable_id, "math.add");
    assert_eq!(report.functions[1].regions.len(), 1);
    assert_eq!(report.functions[1].regions[0].root, "left + right");
    assert_eq!(report.functions[1].regions[0].proposed_width, 2);
    let replay = simd_report::verify_envelope(&envelope).expect("replay");
    assert_eq!(report, replay);

    // Per-region digests authenticate the exact rendered root text under a
    // dedicated domain, reproducible independently here.
    let value: serde_json::Value = serde_json::from_str(&envelope).unwrap();
    let region = &value["payload"]["functions"][1]["regions"][0];
    let mut hasher = Sha256::new();
    hasher.update(REGION_DIGEST_DOMAIN);
    let root = region["root"].as_str().unwrap();
    hasher.update((root.len() as u64).to_le_bytes());
    hasher.update(root.as_bytes());
    assert_eq!(
        region["root_sha256"],
        format!(
            "sha256:{:x}",
            semaprax::digest_hex::LowerHex(hasher.finalize())
        )
    );
}

#[test]
fn verify_envelope_rejects_every_digest_field_tamper() {
    let envelope = simd_report::generate(Path::new(MEANING_PATH), &SimdReportOptions::default())
        .expect("envelope");

    // Region-root text mutation first breaks the outer digest ...
    let tampered_root = envelope.replace("left + right", "left + right ");
    assert_ne!(tampered_root, envelope);
    assert!(simd_report::verify_envelope(&tampered_root).is_err());

    // ... and even a consistently re-signed envelope is caught by the inner
    // region-digest replay.
    let resigned = remint_digest(&tampered_root);
    let error = simd_report::verify_envelope(&resigned)
        .expect_err("re-signed region mutation must fail replay");
    assert_eq!(error.code, "SPX-V103");

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
    assert!(simd_report::verify_envelope(&tampered_outer).is_err());

    // Ineligibility text mutation breaks the exact payload bytes.
    let tampered_contract = envelope.replace("result == 42", "result == 43");
    assert_ne!(tampered_contract, envelope);
    assert!(simd_report::verify_envelope(&tampered_contract).is_err());

    // Spliced payload member invalidates the outer digest over exact bytes.
    let spliced = format!(
        "{}{}",
        &envelope[..envelope.len() - 1],
        ",\"injected\":true}"
    );
    assert!(simd_report::verify_envelope(&spliced).is_err());

    // Structural damage and foreign schemas.
    let truncated = envelope[..envelope.len() - 4].to_owned();
    assert!(simd_report::verify_envelope(&truncated).is_err());
    assert!(simd_report::verify_envelope("not json").is_err());
    assert!(simd_report::verify_envelope("[]").is_err());
    let foreign_schema = envelope.replace(
        "\"schema\":\"semaprax.simd-report.v1\"",
        "\"schema\":\"semaprax.foreign.v1\"",
    );
    assert_ne!(foreign_schema, envelope);
    assert!(simd_report::verify_envelope(&foreign_schema).is_err());
}

#[test]
fn re_signed_closed_section_forgeries_fail_replay() {
    let envelope = simd_report::generate(Path::new(MEANING_PATH), &SimdReportOptions::default())
        .expect("envelope");

    // A forged lane width cannot exceed the fixed model.
    let forged_width = envelope.replace("\"proposed_width\":2,", "\"proposed_width\":16,");
    assert_ne!(forged_width, envelope);
    let error = simd_report::verify_envelope(&remint_digest(&forged_width))
        .expect_err("infeasible width must fail replay");
    assert_eq!(error.code, "SPX-V103");

    // A forged module count cannot desynchronize counts from listings.
    let forged_count = envelope.replace("\"functions_admitted\":2,", "\"functions_admitted\":3,");
    assert_ne!(forged_count, envelope);
    assert!(simd_report::verify_envelope(&remint_digest(&forged_count)).is_err());

    // The portable-operation table is closed; a smuggled row fails even with
    // a consistent outer digest.
    let smuggled_operation =
        envelope.replace("\"operation_table\":[", "\"operation_table\":[{\"x\":1},");
    assert_ne!(smuggled_operation, envelope);
    assert!(simd_report::verify_envelope(&remint_digest(&smuggled_operation)).is_err());

    // A foreign portable operation inside a region is rejected too.
    let foreign_operation = envelope.replace(
        "\"operations\":[\"int_lane.add\"]",
        "\"operations\":[\"magic\"]",
    );
    assert_ne!(foreign_operation, envelope);
    assert!(simd_report::verify_envelope(&remint_digest(&foreign_operation)).is_err());

    // Declared effects cannot be smuggled into an admitted function.
    let forged_effects = envelope.replace(
        "\"declared_effects\":[],",
        "\"declared_effects\":[\"io.release\"],",
    );
    assert_ne!(forged_effects, envelope);
    assert!(simd_report::verify_envelope(&remint_digest(&forged_effects)).is_err());

    // Justification tokens outside the closed vocabulary fail.
    let foreign_token = envelope.replace(
        "\"no_call_expressions_in_body\"",
        "\"magically_effect_free\"",
    );
    assert_ne!(foreign_token, envelope);
    assert!(simd_report::verify_envelope(&remint_digest(&foreign_token)).is_err());

    // The nonclaims section is fixed; removing one claim fails.
    let dropped_claim = envelope.replace(",\"no_target_execution\"", "");
    assert_ne!(dropped_claim, envelope);
    assert!(simd_report::verify_envelope(&remint_digest(&dropped_claim)).is_err());

    // The lane model itself is fixed.
    let forged_ceiling = envelope.replace(
        "{\"element_type\":\"i64\",\"ceiling\":2}",
        "{\"element_type\":\"i64\",\"ceiling\":8}",
    );
    assert_ne!(forged_ceiling, envelope);
    assert!(simd_report::verify_envelope(&remint_digest(&forged_ceiling)).is_err());
}

/// Byte-level cross-consistency: every reported region and every
/// ineligibility entry corresponds to real resolved HIR nodes of the same
/// program. The test walks the actual `hir::resolve` output independently.
#[test]
fn report_matches_the_real_hir_nodes_of_the_same_program() {
    let path = Path::new(CALCULATOR_PATH);
    let source = std::fs::read_to_string(path).unwrap();
    let program = parse(&source, path).expect("parses");
    assert!(verify::verify(&program)
        .iter()
        .all(|item| !item.severity.is_error()));
    let resolved = hir::resolve(&program).expect("resolves");

    fn count_nodes(expr: &ResolvedExpr, operators: &mut usize, divisions: &mut usize) {
        match &expr.kind {
            ResolvedExprKind::Binary { op, left, right } => {
                if matches!(
                    op,
                    semaprax::ast::BinaryOp::Add
                        | semaprax::ast::BinaryOp::Sub
                        | semaprax::ast::BinaryOp::Mul
                ) {
                    *operators += 1;
                }
                if matches!(
                    op,
                    semaprax::ast::BinaryOp::Div | semaprax::ast::BinaryOp::Rem
                ) {
                    *divisions += 1;
                }
                count_nodes(left, operators, divisions);
                count_nodes(right, operators, divisions);
            }
            ResolvedExprKind::Unary { op, value } => {
                if matches!(op, semaprax::ast::UnaryOp::Neg) {
                    *operators += 1;
                }
                count_nodes(value, operators, divisions);
            }
            ResolvedExprKind::Block { statements, tail } => {
                for statement in statements {
                    count_nodes(statement.value(), operators, divisions);
                }
                count_nodes(tail, operators, divisions);
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                count_nodes(condition, operators, divisions);
                count_nodes(then_branch, operators, divisions);
                count_nodes(else_branch, operators, divisions);
            }
            ResolvedExprKind::Call { args, .. } => {
                for argument in args {
                    count_nodes(argument, operators, divisions);
                }
            }
            _ => {}
        }
    }

    let envelope = simd_report::generate(path, &SimdReportOptions::default()).expect("envelope");
    let value: serde_json::Value = serde_json::from_str(&envelope).unwrap();
    let functions = value["payload"]["functions"].as_array().unwrap();

    let find_hir = |stable_id: &str| -> &ResolvedFunction {
        resolved
            .functions
            .iter()
            .find(|candidate| candidate.id.as_str() == stable_id)
            .expect("monomorphic function exists in the resolved HIR")
    };

    for listed in functions {
        let stable_id = listed["stable_id"].as_str().unwrap();
        let function = find_hir(stable_id);
        let mut operators = 0usize;
        let mut divisions = 0usize;
        count_nodes(&function.body, &mut operators, &mut divisions);

        let regions = listed["regions"].as_array().unwrap();
        let region_operators: usize = regions
            .iter()
            .map(|region| region["operators"].as_u64().unwrap() as usize)
            .sum();
        assert_eq!(
            region_operators, operators,
            "region operators of `{stable_id}` must equal its real Add/Sub/Mul/Neg HIR nodes"
        );

        let ineligible = listed["ineligible"].as_array().unwrap();
        let division_entries: usize = ineligible
            .iter()
            .filter(|entry| entry["reason"] == "division_remainder")
            .count();
        assert_eq!(
            division_entries, divisions,
            "division entries of `{stable_id}` must equal its real Div/Rem HIR nodes"
        );

        // Each region root is rendered from the same element type as the
        // corresponding maximal arithmetic subtree root in the HIR body.
        for region in regions {
            let element_type = region["element_type"].as_str().unwrap();
            match function.body.ty {
                semaprax::hir::ResolvedType::I64 => assert_eq!(element_type, "i64"),
                semaprax::hir::ResolvedType::I32 => assert_eq!(element_type, "i32"),
                semaprax::hir::ResolvedType::U8 => assert_eq!(element_type, "u8"),
                semaprax::hir::ResolvedType::F32 => assert_eq!(element_type, "f32"),
                semaprax::hir::ResolvedType::F64 => assert_eq!(element_type, "f64"),
                _ => {}
            }
        }
    }

    // The whole-module inventory admits all seven calculator functions.
    assert_eq!(functions.len(), 7);
    assert_eq!(
        graph::revision(&program),
        value["payload"]["source"]["revision"].as_str().unwrap()
    );
}

#[test]
fn source_drift_between_generation_and_validation_fails_closed() {
    let path = write_temp(
        "module test.probe;\n@id(\"probe.one\")\nfn one(value: i64) -> i64 { value }\n@id(\"app.main\")\nfn main() -> i64 { 0 }\n",
    );
    let options = SimdReportOptions::default();
    let first = simd_report::generate(&path, &options).expect("envelope");
    std::fs::write(&path, "module test.probe;\n").unwrap();
    let outcome = simd_report::generate(&path, &options);
    assert!(
        outcome.is_err(),
        "drifted source must not reproduce the report"
    );
    assert_ne!(
        simd_report::generate(&path, &options).unwrap_or_default(),
        first
    );
    cleanup(&path);
}

#[test]
fn budget_exhaustion_fails_closed_without_truncation() {
    let path = write_temp(
        "module test.probe;\n@id(\"probe.one\")\nfn one(value: i64) -> i64 { value }\n@id(\"app.main\")\nfn main() -> i64 { 0 }\n",
    );
    let tiny = SimdReportOptions::new(semaprax::graph::MIN_AGENT_CONTEXT_BYTES).unwrap();
    let outcome = simd_report::generate(&path, &tiny);
    let errors = outcome.expect_err("tiny budgets must fail closed");
    assert!(
        errors.iter().any(|item| item.code == "SPX-V102"),
        "expected the byte-budget diagnostic"
    );
    cleanup(&path);
}

#[test]
fn unverified_source_fails_closed() {
    let path = write_temp("module test.probe;\nfn broken( { 0 }\n");
    let outcome = simd_report::generate(&path, &SimdReportOptions::default());
    assert!(outcome.is_err());
    cleanup(&path);
}

#[test]
fn cli_double_run_is_byte_identical() {
    let args = ["simd-report", CALCULATOR_PATH];
    let (first_code, first_out, _) = cli(&args);
    let (second_code, second_out, _) = cli(&args);
    assert_eq!(first_code, 0);
    assert_eq!(second_code, 0);
    assert_eq!(first_out, second_out);
    assert!(first_out.contains("\"schema\":\"semaprax.simd-report.v1\""));
    assert!(first_out.ends_with("}\n"));
}

#[test]
fn cli_rejects_bad_invocations() {
    // Missing file path.
    let (code, _, _) = cli(&["simd-report"]);
    assert_eq!(code, 2);
    // Unknown option.
    let (code, _, err) = cli(&["simd-report", CALCULATOR_PATH, "--lanes", "x"]);
    assert_eq!(code, 2);
    assert!(err.contains("unknown simd-report option"));
    // Duplicate option.
    let (code, _, err) = cli(&[
        "simd-report",
        CALCULATOR_PATH,
        "--max-bytes",
        "65536",
        "--max-bytes",
        "65536",
    ]);
    assert_eq!(code, 2);
    assert!(err.contains("duplicate"));
    // Malformed value.
    let (code, _, err) = cli(&["simd-report", CALCULATOR_PATH, "--max-bytes", "-3"]);
    assert_eq!(code, 2);
    assert!(err.contains("canonical nonnegative integer"));
    // Out-of-bounds value.
    let (code, _, err) = cli(&["simd-report", CALCULATOR_PATH, "--max-bytes", "512"]);
    assert_eq!(code, 2);
    assert!(err.contains("SPX-V101"));
    // Missing option value.
    let (code, _, _) = cli(&["simd-report", CALCULATOR_PATH, "--max-bytes"]);
    assert_eq!(code, 2);
    // Byte-budget exhaustion is fail-closed.
    let big = write_temp(
        "module app.big;\n\n@id(\"big.one\")\nfn one(value: i64) -> i64\n    requires value >= 0\n{ value + 1 }\n\n@id(\"app.main\")\nfn main() -> i64 { one(41) }\n",
    );
    let (code, _, err) = cli(&["simd-report", big.to_str().unwrap(), "--max-bytes", "2048"]);
    assert_eq!(code, 1);
    assert!(err.contains("SPX-V102"), "stderr was: {err}");
    cleanup(&big);
    // Unverifiable sources fail closed with exit code 1.
    let bad = write_temp("module test.probe;\nfn broken( { 0 }\n");
    let (code, _, _) = cli(&["simd-report", bad.to_str().unwrap()]);
    assert_eq!(code, 1);
    cleanup(&bad);
}
