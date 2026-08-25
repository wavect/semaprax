//! Executable evidence for Region Structure Report v1
//! (`semaprax.region-report.v1`).
//!
//! Pins canonical golden envelope digests, proves determinism, exercises
//! every exclusion reason, cross-checks every reported binding identity
//! against the real resolved-HIR [`hir::ValueId`] inventory, verifies
//! fail-closed budget exhaustion, source-drift binding, tamper rejection per
//! digest field including forged-but-re-signed envelopes caught by closed
//! replay (canonical clustering, escape derivation, move facts, bulk-release
//! grouping, ordering, counts), and the CLI exit-code contract. No region
//! inference, annotation syntax, arena runtime, destructor change, or target
//! execution is involved.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use semaprax::region_report::{self, RegionReportOptions};
use semaprax::{hir, parse};
use sha2::{Digest as _, Sha256};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

const PAYLOAD_DIGEST_DOMAIN: &[u8] = b"semaprax.region-report.payload.v1\0";

fn write_temp(source: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-region-report-evidence-{}-{}.spx",
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
const CONTROL_FLOW_PATH: &str = "examples/control_flow.spx";
const MUTATION_PATH: &str = "examples/explicit_mutation.spx";

/// Golden envelope digest over the exact library bytes for the canonical
/// calculator example, emitted through the relative repository path so the
/// fixture is machine-independent. All seven functions are admitted.
#[test]
fn golden_calculator_envelope_digest_is_pinned() {
    let options = RegionReportOptions::default();
    let envelope = region_report::generate(Path::new(CALCULATOR_PATH), &options).expect("envelope");
    assert!(envelope.contains("\"schema\":\"semaprax.region-report.v1\""));
    assert!(envelope.contains(
        "\"module\":{\"name\":\"examples.calculator\",\
\"functions_total\":7,\"functions_admitted\":7,\"functions_excluded\":0}"
    ));
    assert_eq!(
        sha256_hex(envelope.as_bytes()),
        "sha256:cdde79b66a970e57cf86c13bfcac02cdd6782d5c1ceda7949270f344d80ee1e1"
    );
}

/// A second pinned KAT over the minimal two-function example.
#[test]
fn golden_meaning_envelope_digest_is_pinned() {
    let options = RegionReportOptions::default();
    let envelope = region_report::generate(Path::new(MEANING_PATH), &options).expect("envelope");
    assert!(envelope.contains(
        "\"module\":{\"name\":\"examples.meaning\",\
\"functions_total\":2,\"functions_admitted\":2,\"functions_excluded\":0}"
    ));
    assert_eq!(
        sha256_hex(envelope.as_bytes()),
        "sha256:b18fcfcad70e4d71a1de7cc472782af86d08cb15224662930c526b048c890946"
    );
}

#[test]
fn generation_is_deterministic() {
    let options = RegionReportOptions::default();
    for path in [
        Path::new(CALCULATOR_PATH),
        Path::new(MUTATION_PATH),
        Path::new(CONTROL_FLOW_PATH),
    ] {
        let first = region_report::generate(path, &options).expect("first");
        let second = region_report::generate(path, &options).expect("second");
        assert_eq!(first, second);
    }
}

/// Every reported binding identity is a real resolved-HIR `ValueId`, and the
/// reported inventory equals exactly that function's parameter/local/pattern
/// binding inventory.
#[test]
fn reported_binding_ids_equal_the_resolved_hir_inventory() {
    fn collect_pattern(pattern: &hir::ResolvedMatchPattern, ids: &mut Vec<String>) {
        match pattern {
            hir::ResolvedMatchPattern::Wildcard => {}
            hir::ResolvedMatchPattern::Variant { fields, .. } => {
                for field in fields {
                    ids.push(field.binding.id.as_str().to_owned());
                }
            }
            hir::ResolvedMatchPattern::Record { fields, .. } => {
                collect_record_fields(fields, ids);
            }
            // Refutable Match v1: binding arms carry a real identity;
            // literals and or-patterns carry none.
            hir::ResolvedMatchPattern::Binding(binding) => {
                ids.push(binding.id.as_str().to_owned());
            }
            hir::ResolvedMatchPattern::Literal(_) | hir::ResolvedMatchPattern::Or(_) => {}
        }
    }

    fn collect_record_fields(
        fields: &[hir::ResolvedRecordMatchPatternField],
        ids: &mut Vec<String>,
    ) {
        for field in fields {
            match &field.pattern {
                hir::ResolvedRecordMatchFieldPattern::Binding(binding) => {
                    ids.push(binding.id.as_str().to_owned());
                }
                hir::ResolvedRecordMatchFieldPattern::Wildcard => {}
                hir::ResolvedRecordMatchFieldPattern::Record { fields: nested, .. } => {
                    collect_record_fields(nested, ids)
                }
            }
        }
    }

    fn collect_expr(expression: &hir::ResolvedExpr, ids: &mut Vec<String>) {
        match &expression.kind {
            hir::ResolvedExprKind::Block { statements, tail } => {
                for statement in statements {
                    ids.push(statement.binding().id.as_str().to_owned());
                    collect_expr(statement.value(), ids);
                }
                collect_expr(tail, ids);
            }
            hir::ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                collect_expr(condition, ids);
                collect_expr(then_branch, ids);
                collect_expr(else_branch, ids);
            }
            hir::ResolvedExprKind::Match { scrutinee, arms } => {
                collect_expr(scrutinee, ids);
                for arm in arms {
                    collect_pattern(&arm.pattern, ids);
                    collect_expr(&arm.value, ids);
                }
            }
            hir::ResolvedExprKind::Call { args, .. } => {
                for argument in args {
                    collect_expr(argument, ids);
                }
            }
            hir::ResolvedExprKind::NativeRustImportCall(call) => {
                for argument in &call.args {
                    collect_expr(argument, ids);
                }
            }
            hir::ResolvedExprKind::HostCommandCall(call) => {
                for argument in &call.args {
                    collect_expr(argument, ids);
                }
            }
            hir::ResolvedExprKind::Unary { value, .. } => collect_expr(value, ids),
            hir::ResolvedExprKind::Binary { left, right, .. } => {
                collect_expr(left, ids);
                collect_expr(right, ids);
            }
            hir::ResolvedExprKind::ConstructRecord { fields, .. }
            | hir::ResolvedExprKind::ConstructVariant { fields, .. } => {
                for field in fields {
                    collect_expr(&field.value, ids);
                }
            }
            hir::ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                collect_expr(base, ids);
                for field in fields {
                    collect_expr(&field.value, ids);
                }
            }
            hir::ResolvedExprKind::Project { base, .. } => collect_expr(base, ids),
            hir::ResolvedExprKind::Upcast { source } => collect_expr(source, ids),
            hir::ResolvedExprKind::Try { operand, .. }
            | hir::ResolvedExprKind::TryOption { operand, .. } => collect_expr(operand, ids),
            hir::ResolvedExprKind::Int(_)
            | hir::ResolvedExprKind::Int32(_)
            | hir::ResolvedExprKind::Char(_)
            | hir::ResolvedExprKind::Uint8(_)
            | hir::ResolvedExprKind::Usize(_)
            | hir::ResolvedExprKind::ArrayU8(_)
            | hir::ResolvedExprKind::RepeatArrayU8 { .. }
            | hir::ResolvedExprKind::Float32(_)
            | hir::ResolvedExprKind::Float64(_)
            | hir::ResolvedExprKind::Bool(_)
            | hir::ResolvedExprKind::String(_)
            | hir::ResolvedExprKind::Place(_)
            | hir::ResolvedExprKind::BorrowPlace { .. } => {}
        }
    }

    for module_path in [
        Path::new(CALCULATOR_PATH),
        Path::new(MEANING_PATH),
        Path::new(CONTROL_FLOW_PATH),
        Path::new(MUTATION_PATH),
    ] {
        let source = std::fs::read_to_string(module_path).unwrap();
        let program = parse(&source, module_path).expect("parses");
        let resolved = hir::resolve(&program).expect("resolves");
        let options = RegionReportOptions::default();
        let envelope = region_report::generate(module_path, &options).expect("envelope");
        let report = region_report::verify_envelope(&envelope).expect("verified");

        for summary in &report.functions {
            let resolved_function = resolved
                .functions
                .iter()
                .find(|candidate| candidate.id.as_str() == summary.stable_id)
                .expect("resolved HIR has the monomorphic function");
            let mut expected: Vec<String> = resolved_function
                .params
                .iter()
                .map(|param| param.id.as_str().to_owned())
                .collect();
            collect_expr(&resolved_function.body, &mut expected);

            let envelope_value: serde_json::Value = serde_json::from_str(&envelope).unwrap();
            let listed = envelope_value["payload"]["functions"]
                .as_array()
                .unwrap()
                .iter()
                .find(|function| function["stable_id"].as_str() == Some(summary.stable_id.as_str()))
                .expect("listed function")["bindings"]
                .as_array()
                .unwrap()
                .iter()
                .map(|binding| binding["id"].as_str().unwrap().to_owned())
                .collect::<Vec<_>>();
            assert_eq!(
                BTreeSet::from_iter(listed.iter().cloned()),
                BTreeSet::from_iter(expected.iter().cloned()),
                "binding inventory mismatch for {}",
                summary.stable_id
            );
        }
    }
}

/// Pattern bindings carry their own identities and live inside the same
/// clustering rules as parameters and locals.
#[test]
fn match_pattern_bindings_are_reported_with_real_identities() {
    let source = r#"
module test.probe;

@id("app.main")
fn main() -> i64
{
    let maybe = Option<i64>::Some { value: 5 };
    match maybe {
        Option::Some { value } => value,
        Option::None {} => 0,
    }
}
"#;
    let path = write_temp(source);
    let envelope =
        region_report::generate(&path, &RegionReportOptions::default()).expect("envelope");
    let value: serde_json::Value = serde_json::from_str(&envelope).unwrap();
    let bindings = value["payload"]["functions"][0]["bindings"]
        .as_array()
        .unwrap();
    let kinds = bindings
        .iter()
        .map(|binding| {
            (
                binding["kind"].as_str().unwrap().to_owned(),
                binding["name"].as_str().unwrap().to_owned(),
            )
        })
        .collect::<Vec<_>>();
    assert!(kinds.contains(&("local".to_owned(), "maybe".to_owned())));
    assert!(kinds.contains(&("match_pattern".to_owned(), "value".to_owned())));

    // The unused-binding invariant: never-used means the live-range end is
    // the definition offset itself.
    let program = parse(source, &path).expect("parses");
    let _ = hir::resolve(&program).expect("resolves");
    let report = region_report::verify_envelope(&envelope).expect("verified");
    assert_eq!(report.functions[0].bindings_total, 2);
    assert_eq!(report.functions[0].regions_total, 2);
    cleanup(&path);
}

/// Co-dying bindings form one maximal bulk-release grouping candidate even
/// though their overlapping ranges force separate region clusters.
#[test]
fn overlapping_ranges_split_regions_and_coinciding_ends_group_releases() {
    let source = r#"
module test.probe;

@id("app.main")
fn main() -> i64
{
    let base = 40;
    let offset = 2;
    base + offset
}
"#;
    let path = write_temp(source);
    let envelope =
        region_report::generate(&path, &RegionReportOptions::default()).expect("envelope");
    let value: serde_json::Value = serde_json::from_str(&envelope).unwrap();
    let function = &value["payload"]["functions"][0];
    // base and offset both die at the tail statement: one region cluster each
    // (overlapping ranges) but exactly one release-group candidate covering
    // both.
    assert_eq!(function["regions_total"], 2);
    assert_eq!(function["release_groups"].as_array().unwrap().len(), 1);
    let group = &function["release_groups"][0];
    let members = group["binding_ids"].as_array().unwrap();
    assert_eq!(members.len(), 2);
    cleanup(&path);
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
        region_report::generate(&path, &RegionReportOptions::default()).expect("envelope");
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
    assert!(envelope
        .contains("\"functions_total\":7,\"functions_admitted\":0,\"functions_excluded\":7"));
    let report = region_report::verify_envelope(&envelope).expect("verified");
    assert!(report.functions.is_empty());

    // An out-of-vocabulary exclusion reason fails the closed-vocabulary
    // replay even when the outer digest was re-minted around the forgery.
    let foreign_reason =
        envelope.replace("\"reason\":\"generic_function\"", "\"reason\":\"magic\"");
    assert_ne!(foreign_reason, envelope);
    let error = region_report::verify_envelope(&remint_digest(&foreign_reason))
        .expect_err("foreign reason must fail replay");
    assert_eq!(error.code, "SPX-L103");
    cleanup(&path);
}

/// Escape facts are fully derived: under the admitted profile no parameter
/// view exists today, so every borrow count is zero, provably non-escaping,
/// and the enforcing ownership check is named verbatim.
#[test]
fn escape_facts_name_the_enforcing_check() {
    let envelope =
        region_report::generate(Path::new(MEANING_PATH), &RegionReportOptions::default())
            .expect("envelope");
    let value: serde_json::Value = serde_json::from_str(&envelope).unwrap();
    for function in value["payload"]["functions"].as_array().unwrap() {
        let escape = &function["escape"];
        assert_eq!(escape["borrowed_parameters"], 0);
        assert_eq!(escape["shared_parameters"], 0);
        assert_eq!(escape["borrows_total"], 0);
        assert_eq!(escape["non_escaping_borrows_total"], 0);
        assert_eq!(escape["all_borrows_provably_non_escaping"], true);
        assert_eq!(escape["enforcing_check"], "SPX-O104");
        assert_eq!(
            escape["enforcing_check_summary"],
            "return-position borrow escape is rejected: a function cannot return a borrowed or shared resource as owned"
        );
    }
    assert!(envelope.contains("\"no_region_inference_implementation\""));
    assert!(envelope.contains("\"no_arena_type\""));
    assert!(envelope.contains("\"no_destructor_changes\""));
}

#[test]
fn verify_envelope_rejects_every_digest_field_tamper() {
    let options = RegionReportOptions::default();
    let envelope = region_report::generate(Path::new(MEANING_PATH), &options).expect("envelope");

    // A payload byte mutation first breaks the outer digest ...
    let tampered_payload = envelope.replace("\"def_offset\":49,", "\"def_offset\":48,");
    assert_ne!(tampered_payload, envelope);
    assert!(region_report::verify_envelope(&tampered_payload).is_err());

    // ... and a spliced payload member invalidates it too.
    let spliced = format!(
        "{}{}",
        &envelope[..envelope.len() - 1],
        ",\"injected\":true}"
    );
    assert!(region_report::verify_envelope(&spliced).is_err());

    // Outer payload-digest field bit flip.
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
    assert!(region_report::verify_envelope(&tampered_outer).is_err());

    // Declared byte-count tamper.
    let payload_len = {
        let value: serde_json::Value = serde_json::from_str(&envelope).unwrap();
        value["bytes"].as_u64().unwrap()
    };
    let tampered_bytes = envelope.replace(
        &format!("\"bytes\":{payload_len},"),
        &format!("\"bytes\":{},", payload_len + 1),
    );
    assert_ne!(tampered_bytes, envelope);
    assert!(region_report::verify_envelope(&tampered_bytes).is_err());

    // Structural damage and foreign schemas.
    let truncated = envelope[..envelope.len() - 4].to_owned();
    assert!(region_report::verify_envelope(&truncated).is_err());
    assert!(region_report::verify_envelope("not json").is_err());
    assert!(region_report::verify_envelope("[]").is_err());
    let foreign_schema = envelope.replace(
        "\"schema\":\"semaprax.region-report.v1\"",
        "\"schema\":\"semaprax.foreign.v1\"",
    );
    assert_ne!(foreign_schema, envelope);
    assert!(region_report::verify_envelope(&foreign_schema).is_err());
}

/// Forged-but-consistently-re-signed envelopes must still fail closed
/// replay: canonical clustering, escape derivation, move facts,
/// bulk-release grouping, counts, and use/end agreement are all re-derived.
#[test]
fn re_signed_forgeries_fail_closed_replay() {
    let options = RegionReportOptions::default();
    let envelope = region_report::generate(Path::new(MEANING_PATH), &options).expect("envelope");

    // Escape-total forgery.
    let forged_escape =
        envelope.replace("\"borrowed_parameters\":0,", "\"borrowed_parameters\":1,");
    assert_ne!(forged_escape, envelope);
    let error = region_report::verify_envelope(&remint_digest(&forged_escape))
        .expect_err("forged escape totals must fail replay");
    assert_eq!(error.code, "SPX-L103");

    // Enforcing-check forgery.
    let forged_check = envelope.replace(
        "\"enforcing_check\":\"SPX-O104\"",
        "\"enforcing_check\":\"SPX-O999\"",
    );
    assert_ne!(forged_check, envelope);
    assert!(region_report::verify_envelope(&remint_digest(&forged_check)).is_err());

    // Count forgery fails counts-vs-listings replay.
    let forged_count = envelope.replace("\"functions_admitted\":2,", "\"functions_admitted\":3,");
    assert_ne!(forged_count, envelope);
    assert!(region_report::verify_envelope(&remint_digest(&forged_count)).is_err());

    // Release-group end forgery fails exact grouping re-derivation. The
    // math.add group holds both parameters at one shared statement end.
    let group_end = {
        let value: serde_json::Value = serde_json::from_str(&envelope).unwrap();
        value["payload"]["functions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|function| function["stable_id"].as_str() == Some("math.add"))
            .unwrap()["release_groups"][0]["end_offset"]
            .as_u64()
            .unwrap()
    };
    let forged_group = envelope.replace(
        &format!("\"end_offset\":{group_end}"),
        &format!("\"end_offset\":{}", group_end + 1),
    );
    assert_ne!(forged_group, envelope);
    let error = region_report::verify_envelope(&remint_digest(&forged_group))
        .expect_err("forged release group must fail replay");
    assert_eq!(error.code, "SPX-L103");

    // Move-fact forgery: an invented moved binding with no consumption site
    // disagrees with the derived list.
    let forged_moves = envelope.replace("\"moved_bindings\":[]", "\"moved_bindings\":[\"x\"]");
    assert_ne!(forged_moves, envelope);
    assert!(region_report::verify_envelope(&remint_digest(&forged_moves)).is_err());

    // Region-assignment forgery: swap two singleton region members of the
    // unused-parameter fixture. The partition stays conflict-free but is no
    // longer the canonical clustering, so exact re-derivation rejects it.
    let source = r#"
module test.probe;

@id("probe.take")
fn take(first: i64, second: i64) -> i64 { first }

@id("app.main")
fn main() -> i64 { take(0, 0) }
"#;
    let path = write_temp(source);
    let take_envelope =
        region_report::generate(&path, &RegionReportOptions::default()).expect("envelope");
    let value: serde_json::Value = serde_json::from_str(&take_envelope).unwrap();
    let take_function = value["payload"]["functions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|function| function["stable_id"].as_str() == Some("probe.take"))
        .unwrap()
        .clone();
    let regions = take_function["regions"].as_array().unwrap().clone();
    assert_eq!(regions.len(), 2);
    let first_id = regions[0]["binding_ids"][0].as_str().unwrap();
    let second_id = regions[1]["binding_ids"][0].as_str().unwrap();
    let swapped = take_envelope
        .replacen(first_id, "\u{0}A\u{0}", 1)
        .replacen(second_id, first_id, 1)
        .replace("\u{0}A\u{0}", second_id);
    assert_ne!(swapped, take_envelope);
    let error = region_report::verify_envelope(&remint_digest(&swapped))
        .expect_err("non-canonical region assignment must fail replay");
    assert_eq!(error.code, "SPX-L103");

    // Use/end disagreement: marking the never-used parameter as used without
    // moving its live-range end breaks the derivation invariant.
    let forged_use = take_envelope.replacen("\"use_count\":0", "\"use_count\":1", 1);
    assert_ne!(forged_use, take_envelope);
    assert!(region_report::verify_envelope(&remint_digest(&forged_use)).is_err());
    cleanup(&path);
}

#[test]
fn budget_exhaustion_fails_closed_without_truncation() {
    let path = write_temp(
        "module test.probe;\n@id(\"probe.one\")\nfn one(value: i64) -> i64 { value }\n@id(\"app.main\")\nfn main() -> i64 { 0 }\n",
    );
    let tiny = RegionReportOptions::new(semaprax::graph::MIN_AGENT_CONTEXT_BYTES).unwrap();
    let errors = region_report::generate(&path, &tiny).expect_err("tiny budgets must fail closed");
    assert!(
        errors.iter().any(|item| item.code == "SPX-L102"),
        "expected the byte-budget diagnostic"
    );

    // Out-of-bounds option values fail closed before any file access.
    let too_small = RegionReportOptions::new(512)
        .expect_err("below minimum")
        .code;
    assert_eq!(too_small, "SPX-L101");
    let too_large = RegionReportOptions::new(semaprax::graph::MAX_AGENT_CONTEXT_BYTES + 1)
        .expect_err("above maximum")
        .code;
    assert_eq!(too_large, "SPX-L101");
    cleanup(&path);
}

#[test]
fn source_drift_between_generation_and_validation_fails_closed() {
    let path = write_temp(
        "module test.probe;\n@id(\"probe.one\")\nfn one(value: i64) -> i64 { value }\n@id(\"app.main\")\nfn main() -> i64 { 0 }\n",
    );
    let options = RegionReportOptions::default();
    let envelope = region_report::generate(&path, &options).expect("envelope");
    region_report::verify_envelope_against_source(&envelope, &path)
        .expect("fresh report binds its source");

    std::fs::write(
        &path,
        "module test.probe;\n@id(\"app.main\")\nfn main() -> i64 { 7 }\n",
    )
    .unwrap();
    let error = region_report::verify_envelope_against_source(&envelope, &path)
        .expect_err("drifted source must fail closed");
    assert_eq!(error.code, "SPX-L103");
    cleanup(&path);
}

#[test]
fn unverified_source_fails_closed() {
    let path = write_temp("module test.probe;\nfn broken( { 0 }\n");
    let outcome = region_report::generate(&path, &RegionReportOptions::default());
    assert!(outcome.is_err());
    cleanup(&path);
}

#[test]
fn cli_double_run_is_byte_identical() {
    let args = ["region-report", CALCULATOR_PATH];
    let (first_code, first_out, _) = cli(&args);
    let (second_code, second_out, _) = cli(&args);
    assert_eq!(first_code, 0);
    assert_eq!(second_code, 0);
    assert_eq!(first_out, second_out);
    assert!(first_out.contains("\"schema\":\"semaprax.region-report.v1\""));
    assert!(first_out.ends_with("}\n"));
}

#[test]
fn cli_rejects_bad_invocations() {
    // Missing file path.
    let (code, _, _) = cli(&["region-report"]);
    assert_eq!(code, 2);
    // Unknown option.
    let (code, _, err) = cli(&["region-report", CALCULATOR_PATH, "--regions", "x"]);
    assert_eq!(code, 2);
    assert!(err.contains("unknown region-report option"));
    // Duplicate option.
    let (code, _, err) = cli(&[
        "region-report",
        CALCULATOR_PATH,
        "--max-bytes",
        "65536",
        "--max-bytes",
        "65536",
    ]);
    assert_eq!(code, 2);
    assert!(err.contains("duplicate"));
    // Malformed value.
    let (code, _, err) = cli(&["region-report", CALCULATOR_PATH, "--max-bytes", "-3"]);
    assert_eq!(code, 2);
    assert!(err.contains("canonical nonnegative integer"));
    // Out-of-bounds value.
    let (code, _, err) = cli(&["region-report", CALCULATOR_PATH, "--max-bytes", "512"]);
    assert_eq!(code, 2);
    assert!(err.contains("SPX-L101"));
    // Missing option value.
    let (code, _, _) = cli(&["region-report", CALCULATOR_PATH, "--max-bytes"]);
    assert_eq!(code, 2);
    // Byte-budget exhaustion is fail-closed.
    let big = write_temp(
        "module app.big;\n\n@id(\"big.one\")\nfn one(value: i64) -> i64\n    requires value >= 0\n    ensures result == value + 1\n{ value + 1 }\n\n@id(\"app.main\")\nfn main() -> i64 { one(41) }\n",
    );
    let (code, _, err) = cli(&[
        "region-report",
        big.to_str().unwrap(),
        "--max-bytes",
        "1024",
    ]);
    assert_eq!(code, 1);
    assert!(err.contains("SPX-L102"), "stderr was: {err}");
    cleanup(&big);
    // Unverifiable sources fail closed with exit code 1.
    let bad = write_temp("module test.probe;\nfn broken( { 0 }\n");
    let (code, _, _) = cli(&["region-report", bad.to_str().unwrap()]);
    assert_eq!(code, 1);
    cleanup(&bad);
}
