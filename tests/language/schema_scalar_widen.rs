//! Executable evidence for the Schema Scalar Widening v1 tranche: the three
//! read-only schema/manifest projections (`semaprax.openapi.v1`,
//! `semaprax.package-report.v1`, `semaprax.ui-dialect-schema.v1`) admit the
//! full Copy-scalar surface (`i64`, `i32`, `u8`, `f32`, `f64`, `char`, `bool`;
//! mixed signatures allowed) with unchanged envelope schemas, canonical
//! rendering rules, and digest authentication.
//!
//! Pins golden KATs per widened family over repository-relative fixtures,
//! proves determinism and independent digest replay, exercises still-closed
//! exclusions beside widened admissions, verifies budget fail-closed behavior
//! and CLI exit codes, checks layout cross-consistency against the checked
//! Native64 compiler layouts (pinned facts; the in-crate module tests compare
//! against `aggregate_layout` directly), and proves cross-consistency between
//! package-report exports and openapi operations over the same widened
//! program. No target execution is involved.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use semaprax::graph;
use semaprax::openapi::{self, OpenApiOptions};
use semaprax::package_report::{self, PackageReportOptions};
use semaprax::ui_schema::{self, UiSchemaOptions};
use serde_json::Value;
use sha2::{Digest as _, Sha256};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

const MIXED_PATH: &str = "tests/schema-widen-fixtures/mixed.spx";
const STATE_PATH: &str = "tests/schema-widen-fixtures/state.spx";

const OPENAPI_DOCUMENT_DIGEST_DOMAIN: &[u8] = b"semaprax.openapi.document.v1\0";
const PACKAGE_PAYLOAD_DIGEST_DOMAIN: &[u8] = b"semaprax.package-report.payload.v1\0";
const PACKAGE_EXPORT_SIGNATURE_DIGEST_DOMAIN: &[u8] =
    b"semaprax.package-report.export-signature.v1\0";
const UI_PAYLOAD_DIGEST_DOMAIN: &[u8] = b"semaprax.ui-dialect-schema.payload.v1\0";
const UI_STATE_SHAPE_DIGEST_DOMAIN: &[u8] = b"semaprax.ui-dialect-schema.state-shape.v1\0";
const UI_ACTION_SIGNATURE_DIGEST_DOMAIN: &[u8] =
    b"semaprax.ui-dialect-schema.action-signature.v1\0";

fn write_temp(source: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-schema-scalar-widen-{}-{}.spx",
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

fn domain_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
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

/// Re-mints an outer digest around `tampered_envelope`'s exact payload bytes
/// so replay must rely on its derivation rules rather than the outer digest
/// alone. Works for both compact wrapper envelopes (package-report,
/// ui-schema).
fn remint_wrapper_digest(tampered_envelope: &str) -> String {
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
        semaprax::diagnostic::quote_json(&domain_digest(
            PACKAGE_PAYLOAD_DIGEST_DOMAIN,
            payload.as_bytes()
        )),
        payload.len(),
        payload
    )
}

// ---------------------------------------------------------------------------
// OpenAPI: widened schema objects, pinned document KAT, compat classification.
// ---------------------------------------------------------------------------

const WIDEN_MIX_REQUEST_SCHEMA: &str = r#"{"additionalProperties":false,"properties":{"a":{"description":"Signed 32-bit two's-complement integer; range [-2147483648, 2147483647]; little-endian byte order in SEMAPRAX target ABIs.","format":"int32","type":"integer"},"b":{"description":"Unsigned 8-bit integer; range [0, 255]; little-endian byte order in SEMAPRAX target ABIs.","format":"int32","maximum":255,"minimum":0,"type":"integer"}},"required":["a","b"],"type":"object"}"#;

const WIDEN_MIX_RESULT_SCHEMA: &str = r#"{"description":"IEEE-754 double-precision binary floating-point value; total arithmetic with no compiler-owned failure statuses; IEEE-754 binary interchange encoding in SEMAPRAX target ABIs.","format":"double","type":"number"}"#;

const WIDEN_RATIO_RESULT_SCHEMA: &str = r#"{"description":"Exactly one Unicode scalar value; compared by scalar-value ordering with no arithmetic and no compiler-owned failure statuses; carried as one UTF-8 code point.","maxLength":1,"minLength":1,"type":"string"}"#;

const WIDEN_COUNT_RESULT_SCHEMA: &str = r#"{"description":"Unsigned 8-bit integer; range [0, 255]; little-endian byte order in SEMAPRAX target ABIs.","format":"int32","maximum":255,"minimum":0,"type":"integer"}"#;

const WIDEN_DOCUMENT_SHA256: &str =
    "sha256:c512aba565d883abc9db99acc1914c9a6d10bf020ca9692877df48eb882e30e1";

#[test]
fn openapi_widened_document_kat_is_pinned() {
    let selections = [
        "widen.mix".to_owned(),
        "widen.ratio".to_owned(),
        "widen.count".to_owned(),
        "app.main".to_owned(),
    ];
    let report = openapi::generate(
        Path::new(MIXED_PATH),
        &selections,
        &OpenApiOptions::default(),
    )
    .expect("envelope");
    let envelope: Value = serde_json::from_str(&report).unwrap();
    assert_eq!(envelope["schema"], "semaprax.openapi.v1");
    assert_eq!(envelope["operations"], 4);

    // Per-family schema objects are pinned exactly as canonical bytes.
    let schemas = &envelope["document"]["components"]["schemas"];
    assert_eq!(
        schemas["widen_mix.Request"].to_string(),
        WIDEN_MIX_REQUEST_SCHEMA
    );
    assert_eq!(
        schemas["widen_mix.Result"].to_string(),
        WIDEN_MIX_RESULT_SCHEMA
    );
    assert_eq!(
        schemas["widen_ratio.Result"].to_string(),
        WIDEN_RATIO_RESULT_SCHEMA
    );
    assert_eq!(
        schemas["widen_count.Result"].to_string(),
        WIDEN_COUNT_RESULT_SCHEMA
    );

    // Failure-surface rules survive widening: checked i32/u8 positions keep
    // the default response while total f32->char signatures stay without one.
    assert!(
        envelope["document"]["paths"]["/widen.mix"]["post"]["responses"]
            .get("default")
            .is_some()
    );
    assert!(
        envelope["document"]["paths"]["/widen.ratio"]["post"]["responses"]
            .get("default")
            .is_none()
    );

    // The pinned document digest is independently replayable from the exact
    // embedded document bytes using the documented domain-separated scheme.
    let document_bytes = envelope["document"].to_string();
    assert_eq!(
        domain_digest(OPENAPI_DOCUMENT_DIGEST_DOMAIN, document_bytes.as_bytes()),
        WIDEN_DOCUMENT_SHA256
    );
    assert_eq!(envelope["sha256"], WIDEN_DOCUMENT_SHA256);
}

#[test]
fn openapi_widened_generation_is_deterministic() {
    let selections = ["widen.mix".to_owned(), "widen.count".to_owned()];
    let first = openapi::generate(
        Path::new(MIXED_PATH),
        &selections,
        &OpenApiOptions::default(),
    )
    .expect("first");
    let second = openapi::generate(
        Path::new(MIXED_PATH),
        &selections,
        &OpenApiOptions::default(),
    )
    .expect("second");
    assert_eq!(first, second);
}

#[test]
fn openapi_widened_context_keeps_still_closed_exclusions_fail_closed() {
    let source = r#"
module test.widen.excluded;

permit { host.echo }

@id("wex.poly")
fn poly<T>(value: T) -> T { value }

@id("wex.effect")
fn effect(ratio: f32) -> f32 uses { host.echo } { ratio }

@id("wex.string")
fn string(text: string) -> u8 { 0u8 }

@id("app.main")
fn main() -> i64 { 0 }
"#;
    let path = write_temp(source);
    for (selection, reason) in [
        ("wex.poly", "generic_function"),
        ("wex.effect", "declared_effects"),
        ("wex.string", "unsupported_parameter_type"),
    ] {
        let errors = openapi::generate(&path, &[selection.to_owned()], &OpenApiOptions::default())
            .unwrap_err();
        assert_eq!(errors[0].code, "SPX-OA103", "selection {selection}");
        assert!(
            errors[0].message.contains(reason),
            "{selection} must carry reason {reason}"
        );
    }

    // Budget exhaustion stays fail-closed over widened envelopes.
    let tiny = OpenApiOptions::new(graph::MIN_AGENT_CONTEXT_BYTES).unwrap();
    let errors = openapi::generate(&path, &["app.main".to_owned()], &tiny).unwrap_err();
    assert_eq!(errors[0].code, "SPX-OA105");
    cleanup(&path);
}

/// Re-mints an openapi envelope's embedded-document digest around tampered
/// document bytes, simulating a forged-but-consistently-signed envelope.
fn remint_openapi_document_digest(tampered_envelope: &str) -> String {
    let envelope: Value = serde_json::from_str(tampered_envelope).unwrap();
    let mut fixed = envelope.clone();
    fixed["sha256"] = Value::String(domain_digest(
        OPENAPI_DOCUMENT_DIGEST_DOMAIN,
        envelope["document"].to_string().as_bytes(),
    ));
    fixed.to_string()
}

#[test]
fn openapi_widened_compat_classifies_width_and_result_changes() {
    let base_source = "module test.widen.compat;\n\n@id(\"wcompat.f\")\nfn f(a: i32, b: u8) -> f64 { 1.5 }\n\n@id(\"app.main\")\nfn main() -> i64 { 0 }\n";
    // Same operation, but the byte width changed (u8 -> i32 shares the
    // integer:int32 rendering) and the result narrowed (f64 -> i64).
    let candidate_source = "module test.widen.compat;\n\n@id(\"wcompat.f\")\nfn f(a: i32, b: i32) -> i64 { 0 }\n\n@id(\"app.main\")\nfn main() -> i64 { 0 }\n";
    let base_path = write_temp(base_source);
    let candidate_path = write_temp(candidate_source);
    let selections = ["wcompat.f".to_owned()];
    let base =
        openapi::generate(&base_path, &selections, &OpenApiOptions::default()).expect("base");
    let candidate = openapi::generate(&candidate_path, &selections, &OpenApiOptions::default())
        .expect("candidate");

    let base_file = write_temp("base");
    let candidate_file = write_temp("candidate");
    std::fs::write(&base_file, &base).unwrap();
    std::fs::write(&candidate_file, &candidate).unwrap();
    let report = openapi::compatibility(&base_file, &candidate_file, &OpenApiOptions::default())
        .expect("report");
    let parsed: Value = serde_json::from_str(&report).unwrap();
    assert_eq!(parsed["verdict"], "breaking");
    let findings: Vec<&str> = parsed["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| finding["code"].as_str().unwrap())
        .collect();
    assert!(
        findings.contains(&"OAC-B003"),
        "the u8 -> i32 width change must be breaking: {findings:?}"
    );
    assert!(
        findings.contains(&"OAC-B005"),
        "the f64 -> i64 result change must be breaking: {findings:?}"
    );

    // A forged-but-re-signed input authenticates (the envelope binds its own
    // document) and is then classified honestly instead of silently passing
    // as unchanged. The forgery narrows the base result rendering.
    let forged = base.replace("\"format\":\"double\"", "\"format\":\"float\"");
    assert_ne!(forged, base);
    let re_signed = remint_openapi_document_digest(&forged);
    std::fs::write(&base_file, &re_signed).unwrap();
    let report = openapi::compatibility(&base_file, &candidate_file, &OpenApiOptions::default())
        .expect("re-signed inputs authenticate");
    let parsed: Value = serde_json::from_str(&report).unwrap();
    let findings: Vec<&str> = parsed["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| finding["code"].as_str().unwrap())
        .collect();
    assert!(
        findings.contains(&"OAC-B005"),
        "a re-signed float-width forgery must still be flagged breaking: {findings:?}"
    );

    // Tampering without re-minting fails authentication closed.
    std::fs::write(&base_file, &forged).unwrap();
    let errors = openapi::compatibility(&base_file, &candidate_file, &OpenApiOptions::default())
        .unwrap_err();
    assert_eq!(errors[0].code, "SPX-OA104");

    cleanup(&base_path);
    cleanup(&candidate_path);
    cleanup(&base_file);
    cleanup(&candidate_file);
}

#[test]
fn openapi_widened_cli_exit_codes_hold() {
    let (code, out, _) = cli(&[
        "openapi",
        MIXED_PATH,
        "--function",
        "widen.mix",
        "--function",
        "widen.count",
    ]);
    assert_eq!(code, 0);
    assert!(out.contains("\"format\":\"double\""));
    assert!(out.contains("\"maximum\":255"));
    assert!(out.ends_with("}\n"));

    let (code, _, _) = cli(&["openapi", MIXED_PATH]);
    assert_eq!(code, 2, "missing --function selections must fail closed");

    let (code, _, err) = cli(&[
        "openapi",
        MIXED_PATH,
        "--function",
        "widen.mix",
        "--max-bytes",
        "2048",
    ]);
    assert_eq!(code, 1);
    assert!(err.contains("SPX-OA105"), "stderr was: {err}");
}

// ---------------------------------------------------------------------------
// Package report: widened inventory, verbatim native prototypes, replay.
// ---------------------------------------------------------------------------

const WIDEN_MIX_NATIVE_SIGNATURE: &str = "static __attribute__((unused)) spx_status_token spx_decl_776964656e2e6d6978(struct spx_context *spx_ctx, int32_t, uint8_t, double *spx_result_out);";
const WIDEN_RATIO_NATIVE_SIGNATURE: &str = "static __attribute__((unused)) spx_status_token spx_decl_776964656e2e726174696f(struct spx_context *spx_ctx, float, uint32_t *spx_result_out);";

const WIDEN_PACKAGE_ENVELOPE_SHA256: &str =
    "sha256:daf92661846ed9e940ead3bcf4e97a19e88a9ff7300d7454c671f77d6ff16ced";

#[test]
fn package_report_widened_golden_envelope_kat_is_pinned() {
    let options = PackageReportOptions::default();
    let envelope = package_report::generate(Path::new(MIXED_PATH), &options).expect("envelope");
    assert!(envelope.contains("\"schema\":\"semaprax.package-report.v1\""));
    assert!(envelope.contains("\"exports_admitted\":4,\"exports_excluded\":0"));
    // Widened language-type vocabulary in the export inventory.
    assert!(envelope.contains("\"parameters\":[\"i32\",\"u8\"],\"result\":\"f64\""));
    assert!(envelope.contains("\"parameters\":[\"f32\"],\"result\":\"char\""));
    assert!(envelope.contains("\"parameters\":[\"char\",\"bool\"],\"result\":\"u8\""));
    // The whole-envelope digest is pinned, so any rendering change breaks
    // this KAT loudly.
    assert_eq!(
        sha256_hex(envelope.as_bytes()),
        WIDEN_PACKAGE_ENVELOPE_SHA256
    );
    package_report::verify_envelope(&envelope).expect("verified");
}

#[test]
fn package_report_widened_exports_carry_verbatim_native_prototypes() {
    let options = PackageReportOptions::default();
    let envelope = package_report::generate(Path::new(MIXED_PATH), &options).expect("envelope");
    let value: Value = serde_json::from_str(&envelope).unwrap();
    let exports = value["payload"]["exports"].as_array().unwrap().clone();
    assert_eq!(exports.len(), 4);
    let by_id: std::collections::BTreeMap<String, &Value> = exports
        .iter()
        .map(|export| (export["stable_id"].as_str().unwrap().to_owned(), export))
        .collect();

    // Prototypes are extracted verbatim from the production native projection
    // with compiler-exact C types per widened family.
    assert_eq!(
        by_id["widen.mix"]["native64"]["signature"]
            .as_str()
            .unwrap(),
        WIDEN_MIX_NATIVE_SIGNATURE
    );
    assert_eq!(
        by_id["widen.ratio"]["native64"]["signature"]
            .as_str()
            .unwrap(),
        WIDEN_RATIO_NATIVE_SIGNATURE
    );
    assert!(by_id["widen.count"]["native64"]["signature"]
        .as_str()
        .unwrap()
        .ends_with("uint32_t, bool, uint8_t *spx_result_out);"));

    // Every embedded signature digest equals an independent domain-separated
    // recomputation, and full replay returns identical summaries.
    for export in exports.iter() {
        let signature = export["native64"]["signature"].as_str().unwrap();
        assert_eq!(
            export["native64"]["signature_sha256"].as_str().unwrap(),
            domain_digest(PACKAGE_EXPORT_SIGNATURE_DIGEST_DOMAIN, signature.as_bytes())
        );
    }
    let verified = package_report::verify_envelope(&envelope).expect("verified");
    let replay = package_report::verify_envelope(&envelope).expect("replay");
    assert_eq!(verified, replay);
    assert_eq!(verified.exports.len(), 4);
    let mix = verified
        .exports
        .iter()
        .find(|export| export.stable_id == "widen.mix")
        .expect("mix export");
    assert_eq!(mix.native_signature, WIDEN_MIX_NATIVE_SIGNATURE);
}

#[test]
fn package_report_widened_forgeries_still_fail_replay() {
    let options = PackageReportOptions::default();
    let envelope = package_report::generate(Path::new(MIXED_PATH), &options).expect("envelope");

    // A signature-text mutation breaks the outer digest ...
    let tampered = envelope.replace("*spx_result_out);", "*spx_result_out );");
    assert_ne!(tampered, envelope);
    assert!(package_report::verify_envelope(&tampered).is_err());
    // ... and even a consistently re-minted envelope is caught by the inner
    // export-signature digest replay.
    let error = package_report::verify_envelope(&remint_wrapper_digest(&tampered))
        .expect_err("re-signed signature mutation must fail replay");
    assert_eq!(error.code, "SPX-P303");

    // The closed target matrix stays closed over widened envelopes: a demoted
    // target fails replay even when the outer digest was re-minted around the
    // forgery.
    let forged_target = envelope.replace(
        "\"target\":\"wasm32\",\"available\":true",
        "\"target\":\"wasm32\",\"available\":false",
    );
    assert_ne!(forged_target, envelope);
    let error = package_report::verify_envelope(&remint_wrapper_digest(&forged_target))
        .expect_err("demoted target must fail replay");
    assert_eq!(error.code, "SPX-P303");
}

#[test]
fn package_report_widened_exclusions_budget_and_determinism() {
    // By-value resource modes stay excluded beside widened admissions (the
    // unsupported_parameter_mode reason is exercised against real programs by
    // tests/offline_package/report.rs); here the widened inventory admits five
    // mixed-scalar exports while string parameters stay excluded.
    let source = r#"
module test.widen.pr;

@id("wpr.ok")
fn ok(a: i32, b: u8) -> f64 { 1.5 }

@id("wpr.string")
fn string(text: string) -> i32 { 0i32 }

@id("wpr.extra.one")
fn extra_one(ratio: f32) -> char { 'x' }

@id("wpr.extra.two")
fn extra_two(c: char, flag: bool) -> u8 { 7u8 }

@id("wpr.extra.three")
fn extra_three(a: i32, b: u8, r: f32) -> f64
    requires b >= 0u8
{ 2.5 }

@id("wpr.extra.four")
fn extra_four(a: i64, b: i32) -> char { 'y' }

fn main() -> i64 { 0 }
"#;
    let path = write_temp(source);
    let options = PackageReportOptions::default();
    let envelope = package_report::generate(&path, &options).expect("envelope");
    assert!(envelope.contains("\"reason\":\"unsupported_parameter_type\""));
    assert!(envelope.contains("\"reason\":\"automatic_identity\""));
    assert!(
        envelope.contains("\"functions_total\":7,\"exports_admitted\":5,\"exports_excluded\":2")
    );
    let again = package_report::generate(&path, &options).expect("again");
    assert_eq!(envelope, again, "generation must be deterministic");

    let tiny = PackageReportOptions::new(graph::MIN_AGENT_CONTEXT_BYTES).unwrap();
    let errors = package_report::generate(&path, &tiny).expect_err("tiny budgets must fail closed");
    assert!(errors.iter().any(|item| item.code == "SPX-P302"));
    cleanup(&path);
}

#[test]
fn package_report_widened_exports_equal_openapi_operations() {
    let options = PackageReportOptions::default();
    let envelope = package_report::generate(Path::new(MIXED_PATH), &options).expect("envelope");
    let value: Value = serde_json::from_str(&envelope).unwrap();
    let mut package_ids: Vec<String> = value["payload"]["exports"]
        .as_array()
        .unwrap()
        .iter()
        .map(|export| export["stable_id"].as_str().unwrap().to_owned())
        .collect();

    let mut selections = package_ids.clone();
    selections.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    let openapi_envelope = openapi::generate(
        Path::new(MIXED_PATH),
        &selections,
        &openapi::OpenApiOptions::default(),
    )
    .expect("openapi envelope");
    let openapi_value: Value = serde_json::from_str(&openapi_envelope).unwrap();
    let mut openapi_ids: Vec<String> = openapi_value["document"]["paths"]
        .as_object()
        .unwrap()
        .keys()
        .map(|path| path.trim_start_matches('/').to_owned())
        .collect();
    package_ids.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    openapi_ids.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    assert!(!package_ids.is_empty());
    assert_eq!(
        package_ids, openapi_ids,
        "widened exports and widened operations must describe the same surface"
    );

    // The interface types must also agree per export between the two
    // projections: package-report language types versus openapi JSON types.
    for id in &package_ids {
        assert!(openapi_envelope.contains(&format!("\"x-stable-id\":\"{id}\"")));
    }
    let export_types: std::collections::BTreeMap<String, (Vec<String>, String)> = value["payload"]
        ["exports"]
        .as_array()
        .unwrap()
        .iter()
        .map(|export| {
            (
                export["stable_id"].as_str().unwrap().to_owned(),
                (
                    export["parameters"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|item| item.as_str().unwrap().to_owned())
                        .collect::<Vec<_>>(),
                    export["result"].as_str().unwrap().to_owned(),
                ),
            )
        })
        .collect();
    let expected_pairs = [
        ("widen.mix", vec!["integer:int32", "integer:int32[0,255]"]),
        ("widen.ratio", vec!["number:float"]),
    ];
    for (stable_id, parameter_labels) in expected_pairs {
        let operation = &openapi_value["document"]["paths"][format!("/{stable_id}")]["post"];
        let request_ref = operation["requestBody"]["content"]["application/json"]["schema"]["$ref"]
            .as_str()
            .unwrap();
        let component = request_ref.strip_prefix("#/components/schemas/").unwrap();
        let request = &openapi_value["document"]["components"]["schemas"][component];
        let properties = request["properties"].as_object().unwrap();
        let mut labels: Vec<String> = properties
            .values()
            .map(SchemaShapeLabel::of)
            .map(|shape| shape.render())
            .collect();
        labels.sort();
        let (package_parameters, _) = &export_types[stable_id];
        assert_eq!(
            labels.len(),
            package_parameters.len(),
            "{stable_id} parameter arity"
        );
        assert_eq!(
            labels, parameter_labels,
            "{stable_id} parameters must agree across projections"
        );
        let result_ref = operation["responses"]["200"]["content"]["application/json"]["schema"]
            ["$ref"]
            .as_str()
            .unwrap();
        let result_component = result_ref.strip_prefix("#/components/schemas/").unwrap();
        let result_label_actual = SchemaShapeLabel::of(
            &openapi_value["document"]["components"]["schemas"][result_component],
        )
        .render();
        let (_, package_result) = &export_types[stable_id];
        // The package-report language type must map to exactly the JSON
        // schema shape the openapi document renders.
        let expected_result_label: &str = match package_result.as_str() {
            "i64" => "integer:int64",
            "i32" => "integer:int32",
            "u8" => "integer:int32[0,255]",
            "f32" => "number:float",
            "f64" => "number:double",
            "char" => "string:- length[1,1]",
            "bool" => "boolean:-",
            other => panic!("unexpected package-report result type {other}"),
        };
        assert_eq!(
            result_label_actual, expected_result_label,
            "{stable_id} result must agree across projections"
        );
    }
}

/// Minimal mirror of the openapi module's shape label rendering used for
/// cross-projection type agreement checks.
struct SchemaShapeLabel {
    ty: String,
    format: Option<String>,
    minimum: Option<i64>,
    maximum: Option<i64>,
    min_length: Option<u64>,
    max_length: Option<u64>,
}

impl SchemaShapeLabel {
    fn of(schema: &Value) -> Self {
        Self {
            ty: schema["type"].as_str().unwrap_or_default().to_owned(),
            format: schema["format"].as_str().map(str::to_owned),
            minimum: schema["minimum"].as_i64(),
            maximum: schema["maximum"].as_i64(),
            min_length: schema["minLength"].as_u64(),
            max_length: schema["maxLength"].as_u64(),
        }
    }

    fn render(&self) -> String {
        let mut label = format!("{}:{}", self.ty, self.format.as_deref().unwrap_or("-"));
        if self.minimum.is_some() || self.maximum.is_some() {
            label.push_str(&format!(
                "[{:?},{}]",
                self.minimum.unwrap_or(-1),
                self.maximum.unwrap_or(-1)
            ));
        }
        if self.min_length.is_some() || self.max_length.is_some() {
            label.push_str(&format!(
                " length[{},{}]",
                self.min_length.unwrap_or(0),
                self.max_length.unwrap_or(0)
            ));
        }
        label
    }
}

#[test]
fn package_report_widened_cli_exit_codes_hold() {
    let args = ["package-report", MIXED_PATH];
    let (first_code, first_out, _) = cli(&args);
    let (second_code, second_out, _) = cli(&args);
    assert_eq!(first_code, 0);
    assert_eq!(second_code, 0);
    assert_eq!(first_out, second_out);
    assert!(first_out.contains("\"result\":\"char\""));
    assert!(first_out.ends_with("}\n"));

    let (code, _, err) = cli(&["package-report", MIXED_PATH, "--max-bytes", "2048"]);
    assert_eq!(code, 1);
    assert!(err.contains("SPX-P302"), "stderr was: {err}");
}

// ---------------------------------------------------------------------------
// UI schema: widened state shapes from checked layouts, widened actions.
// ---------------------------------------------------------------------------

const WIDEN_TENSOR_LAYOUT_JSON: &str = "{\"fields\":[{\"index\":0,\"name\":\"a\",\"type\":\"i64\",\"offset\":0,\"size_bytes\":8,\"align_bytes\":8},{\"index\":1,\"name\":\"b\",\"type\":\"i32\",\"offset\":8,\"size_bytes\":4,\"align_bytes\":4},{\"index\":2,\"name\":\"c\",\"type\":\"u8\",\"offset\":12,\"size_bytes\":1,\"align_bytes\":1},{\"index\":3,\"name\":\"d\",\"type\":\"f32\",\"offset\":16,\"size_bytes\":4,\"align_bytes\":4},{\"index\":4,\"name\":\"e\",\"type\":\"f64\",\"offset\":24,\"size_bytes\":8,\"align_bytes\":8},{\"index\":5,\"name\":\"g\",\"type\":\"char\",\"offset\":32,\"size_bytes\":4,\"align_bytes\":4},{\"index\":6,\"name\":\"h\",\"type\":\"bool\",\"offset\":36,\"size_bytes\":1,\"align_bytes\":1}],\"size_bytes\":40,\"align_bytes\":8}";

const WIDEN_ACTION_ENTRY_JSON: &str = "{\"stable_id\":\"widen.action\",\"name\":\"action\",\"kind\":\"function\",\"role\":\"action\",\"signature\":{\"parameters\":[{\"name\":\"a\",\"type\":\"i32\"},{\"name\":\"b\",\"type\":\"u8\"}],\"result\":{\"type\":\"f64\"}}";

const WIDEN_UI_ENVELOPE_SHA256: &str =
    "sha256:d4fd47f64b1aed2c5f959b847081a7a530401754fb779d12e1b4e17b847997c0";

#[test]
fn ui_schema_widened_golden_envelope_kat_is_pinned() {
    let options = UiSchemaOptions::default();
    let envelope = ui_schema::generate(Path::new(STATE_PATH), &options).expect("envelope");
    assert!(envelope.contains("\"schema\":\"semaprax.ui-dialect-schema.v1\""));
    assert!(envelope.contains(
        "\"inventory\":{\"state_shapes_admitted\":1,\"actions_admitted\":2,\"excluded\":0}"
    ));
    // The Tensor state shape carries exactly the checked Native64 layout for
    // every widened scalar kind, including padding to the record alignment.
    assert!(envelope.contains(WIDEN_TENSOR_LAYOUT_JSON));
    // The widened action descriptor mirrors the widened abi-report profile.
    assert!(envelope.contains(WIDEN_ACTION_ENTRY_JSON));
    // The whole-envelope digest is pinned, so any rendering change breaks
    // this KAT loudly.
    assert_eq!(sha256_hex(envelope.as_bytes()), WIDEN_UI_ENVELOPE_SHA256);
    ui_schema::verify_envelope(&envelope).expect("verified");
}

#[test]
fn ui_schema_widened_descriptors_match_the_checked_layout_facts() {
    let options = UiSchemaOptions::default();
    let envelope = ui_schema::generate(Path::new(STATE_PATH), &options).expect("envelope");
    let verified = ui_schema::verify_envelope(&envelope).expect("verified");
    assert_eq!(verified.state_shapes.len(), 1);
    let tensor = &verified.state_shapes[0];
    assert_eq!(tensor.stable_id, "widen.tensor");
    assert_eq!(tensor.name, "Tensor");
    // These facts are exactly the aggregate_layout Native64 outputs (unit
    // tests inside the crate compare project_state_shape against
    // aggregate_layout directly for this record).
    assert_eq!(
        tensor
            .fields
            .iter()
            .map(|field| (
                field.name.as_str(),
                field.ty.as_str(),
                field.offset,
                field.size_bytes,
                field.align_bytes
            ))
            .collect::<Vec<_>>(),
        vec![
            ("a", "i64", 0, 8, 8),
            ("b", "i32", 8, 4, 4),
            ("c", "u8", 12, 1, 1),
            ("d", "f32", 16, 4, 4),
            ("e", "f64", 24, 8, 8),
            ("g", "char", 32, 4, 4),
            ("h", "bool", 36, 1, 1),
        ]
    );
    assert_eq!((tensor.size_bytes, tensor.align_bytes), (40, 8));

    // Widened action descriptors round-trip with their exact types.
    let action = verified
        .actions
        .iter()
        .find(|action| action.stable_id == "widen.action")
        .expect("widened action");
    assert_eq!(
        action.parameters,
        vec![
            ("a".to_owned(), "i32".to_owned()),
            ("b".to_owned(), "u8".to_owned()),
        ]
    );
    assert_eq!(action.result_ty, "f64");
}

#[test]
fn ui_schema_widened_embedded_digests_replay_from_listed_values() {
    let options = UiSchemaOptions::default();
    let envelope = ui_schema::generate(Path::new(STATE_PATH), &options).expect("envelope");
    let value: Value = serde_json::from_str(&envelope).unwrap();
    let shape = &value["payload"]["state_shapes"][0];

    // Independently rebuild the canonical layout text from the listed values
    // and compare against the embedded state-shape digest.
    let fields = shape["layout"]["fields"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(index, field)| {
            format!(
                "{{\"index\":{},\"name\":{},\"type\":{},\"offset\":{},\
\"size_bytes\":{},\"align_bytes\":{}}}",
                index,
                semaprax::diagnostic::quote_json(field["name"].as_str().unwrap()),
                semaprax::diagnostic::quote_json(field["type"].as_str().unwrap()),
                field["offset"].as_u64().unwrap(),
                field["size_bytes"].as_u64().unwrap(),
                field["align_bytes"].as_u64().unwrap(),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let rebuilt_layout = format!(
        "{{\"fields\":[{}],\"size_bytes\":{},\"align_bytes\":{}}}",
        fields,
        shape["layout"]["size_bytes"].as_u64().unwrap(),
        shape["layout"]["align_bytes"].as_u64().unwrap(),
    );
    assert_eq!(
        shape["layout_sha256"].as_str().unwrap(),
        domain_digest(UI_STATE_SHAPE_DIGEST_DOMAIN, rebuilt_layout.as_bytes())
    );

    let action = &value["payload"]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|action| action["stable_id"] == "widen.action")
        .cloned()
        .expect("widened action");
    let parameters = action["signature"]["parameters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|parameter| {
            format!(
                "{{\"name\":{},\"type\":{}}}",
                semaprax::diagnostic::quote_json(parameter["name"].as_str().unwrap()),
                semaprax::diagnostic::quote_json(parameter["type"].as_str().unwrap()),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let rebuilt_signature = format!(
        "{{\"parameters\":[{}],\"result\":{{\"type\":{}}}}}",
        parameters,
        semaprax::diagnostic::quote_json(action["signature"]["result"]["type"].as_str().unwrap()),
    );
    assert_eq!(
        action["signature_sha256"].as_str().unwrap(),
        domain_digest(
            UI_ACTION_SIGNATURE_DIGEST_DOMAIN,
            rebuilt_signature.as_bytes()
        )
    );
}

#[test]
fn ui_schema_widened_tamper_and_remint_are_rejected() {
    let options = UiSchemaOptions::default();
    let envelope = ui_schema::generate(Path::new(STATE_PATH), &options).expect("envelope");

    // Any listed layout byte mutation first breaks the outer digest ...
    let tampered_layout = envelope.replace("\"size_bytes\":40", "\"size_bytes\":41");
    assert_ne!(tampered_layout, envelope);
    assert!(ui_schema::verify_envelope(&tampered_layout).is_err());
    // ... and a consistently re-minted forgery is caught by the embedded
    // state-shape digest replay.
    let error = ui_schema::verify_envelope(&remint_ui_digest(&tampered_layout))
        .expect_err("forged layout must fail replay");
    assert_eq!(error.code, "SPX-U103");

    // Widened action signature mutations fail the same way.
    let tampered_signature = envelope.replace(
        "{\"name\":\"a\",\"type\":\"i32\"}",
        "{\"name\":\"a\",\"type\":\"i64\"}",
    );
    assert_ne!(tampered_signature, envelope);
    assert!(ui_schema::verify_envelope(&tampered_signature).is_err());
    let error = ui_schema::verify_envelope(&remint_ui_digest(&tampered_signature))
        .expect_err("forged action signature must fail replay");
    assert_eq!(error.code, "SPX-U103");

    // Foreign schemas stay rejected over widened envelopes.
    let foreign_schema = envelope.replace(
        "\"schema\":\"semaprax.ui-dialect-schema.v1\"",
        "\"schema\":\"semaprax.foreign.v1\"",
    );
    assert_ne!(foreign_schema, envelope);
    assert!(ui_schema::verify_envelope(&foreign_schema).is_err());
}

/// Ui-dialect variant of the wrapper re-mint helper; only the payload digest
/// domain differs from the package-report one.
fn remint_ui_digest(tampered_envelope: &str) -> String {
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
        semaprax::diagnostic::quote_json(&domain_digest(
            UI_PAYLOAD_DIGEST_DOMAIN,
            payload.as_bytes()
        )),
        payload.len(),
        payload
    )
}

#[test]
fn ui_schema_widened_exclusions_determinism_budget_and_cli() {
    let source = r#"
module test.widen.us;

@id("wus.tensor")
record Tensor {
    @id("wus.tensor.a")
    a: f64,

    @id("wus.tensor.text")
    text: string,
}

@id("wus.generic")
record Wrapper<T> {
    @id("wus.generic.value")
    value: T,
}

@id("wus.action")
fn action(a: i32, b: char) -> bool { b == 'x' }

@id("wus.string")
fn string(text: string) -> i64 { 0 }

fn helper(value: i64) -> i64 { value + 1 }

@id("app.main")
fn main() -> i64 { 0 }
"#;
    let path = write_temp(source);
    let options = UiSchemaOptions::default();
    let envelope = ui_schema::generate(&path, &options).expect("envelope");
    assert!(envelope.contains("\"reason\":\"mixed_field_types\""));
    assert!(envelope.contains("\"reason\":\"generic_type\""));
    assert!(envelope.contains("\"reason\":\"automatic_identity\""));
    assert!(envelope.contains("\"reason\":\"unsupported_parameter_type\""));
    // The widened record and widened actions remain admitted.
    assert!(envelope.contains("\"stable_id\":\"wus.tensor\""));
    assert!(envelope.contains("\"stable_id\":\"wus.action\""));
    let again = ui_schema::generate(&path, &options).expect("again");
    assert_eq!(envelope, again, "generation must be deterministic");
    ui_schema::verify_envelope(&envelope).expect("verified");

    let tiny = UiSchemaOptions::new(graph::MIN_AGENT_CONTEXT_BYTES).unwrap();
    let errors = ui_schema::generate(&path, &tiny).expect_err("tiny budgets must fail closed");
    assert!(errors.iter().any(|item| item.code == "SPX-U102"));

    cleanup(&path);

    let args = ["ui-schema", STATE_PATH];
    let (first_code, first_out, _) = cli(&args);
    let (second_code, second_out, _) = cli(&args);
    assert_eq!(first_code, 0);
    assert_eq!(second_code, 0);
    assert_eq!(first_out, second_out);
    assert!(first_out.contains("\"type\":\"char\""));
    assert!(first_out.ends_with("}\n"));

    let (code, _, err) = cli(&["ui-schema", STATE_PATH, "--max-bytes", "2048"]);
    assert_eq!(code, 1);
    assert!(err.contains("SPX-U102"), "stderr was: {err}");
}
