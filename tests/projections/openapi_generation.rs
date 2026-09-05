use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use semaprax::openapi::{self, OpenApiOptions};
use serde_json::Value;
use sha2::{Digest as _, Sha256};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn write_source(name: &str, source: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-openapi-v1-{}-{}-{name}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    let path = if name.ends_with(".json") {
        path.with_extension("json")
    } else {
        path.with_extension("spx")
    };
    std::fs::write(&path, source).unwrap();
    path
}

fn generate(source_path: &Path, selections: &[&str], options: &OpenApiOptions) -> String {
    let owned: Vec<String> = selections.iter().map(|name| name.to_string()).collect();
    openapi::generate(source_path, &owned, options).unwrap()
}

fn first_error_code(
    source_path: &Path,
    selections: &[&str],
    options: &OpenApiOptions,
) -> &'static str {
    let owned: Vec<String> = selections.iter().map(|name| name.to_string()).collect();
    let errors = openapi::generate(source_path, &owned, options).unwrap_err();
    assert!(
        errors.iter().all(|error| error.severity.is_error()),
        "every openapi diagnostic must be an error"
    );
    errors[0].code
}

/// Pinned known-answer fixture: one selected function with declared-order
/// parameters and a requires clause.
const KAT_SINGLE_SOURCE: &str = r#"module test.openapi;

@id("api.echo")
fn echo(zeta: i64, alpha: bool) -> i64
    requires zeta >= 0
{
    zeta
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

const KAT_DOCUMENT_PAYLOAD: &str = r##"{"components":{"schemas":{"Semaprax.Status.v1":{"additionalProperties":false,"description":"Normalized SEMAPRAX failure status (semaprax.status.v1). Compiler-owned domains: semaprax.arithmetic.v1 codes 1 add_overflow, 2 sub_overflow, 3 mul_overflow, 4 division_by_zero, 5 division_overflow, 6 remainder_by_zero, 7 remainder_overflow, 8 negation_overflow (class arithmetic); semaprax.contract.v1 codes 1 requires-false and 2 ensures-false (class contract). All listed statuses are non-retryable.","properties":{"class":{"enum":["arithmetic","contract"],"type":"string"},"code":{"format":"int32","minimum":1,"type":"integer"},"domain_id":{"type":"string"},"retryable":{"type":"boolean"},"schema":{"type":"string"}},"required":["schema","domain_id","code","class","retryable"],"type":"object"},"api_echo.Request":{"additionalProperties":false,"properties":{"alpha":{"description":"Canonical true/false boolean.","type":"boolean"},"zeta":{"description":"Signed 64-bit two's-complement integer; range [-9223372036854775808, 9223372036854775807]; little-endian byte order in SEMAPRAX target ABIs.","format":"int64","type":"integer"}},"required":["zeta","alpha"],"type":"object"},"api_echo.Result":{"description":"Signed 64-bit two's-complement integer; range [-9223372036854775808, 9223372036854775807]; little-endian byte order in SEMAPRAX target ABIs.","format":"int64","type":"integer"}}},"info":{"description":"Deterministic SEMAPRAX OpenAPI Schema Generation v1 projection of the verified module test.openapi; integer range and byte-order notes are static descriptions derived from declared types.","title":"test.openapi OpenAPI schema","version":"sha256:6ab6f2c32c1ac64d17745ff4e9be0eb483c16e3fed19b209961829f050196298"},"openapi":"3.1.0","paths":{"/api.echo":{"post":{"description":"SEMAPRAX function echo. requires zeta >= 0; A violated requires clause selects the compiler-owned failure domain semaprax.contract.v1 code 1; a violated ensures clause selects code 2. Checked i64 arithmetic failures select the compiler-owned failure domain semaprax.arithmetic.v1 codes 1 add_overflow, 2 sub_overflow, 3 mul_overflow, 4 division_by_zero, 5 division_overflow, 6 remainder_by_zero, 7 remainder_overflow, 8 negation_overflow.","operationId":"api_echo","requestBody":{"content":{"application/json":{"schema":{"$ref":"#/components/schemas/api_echo.Request"}}},"required":true},"responses":{"200":{"content":{"application/json":{"schema":{"$ref":"#/components/schemas/api_echo.Result"}}},"description":"Success."},"default":{"content":{"application/json":{"schema":{"$ref":"#/components/schemas/Semaprax.Status.v1"}}},"description":"Compiler-owned SEMAPRAX failure status."}},"x-stable-id":"api.echo"}}}}"##;

const KAT_DOCUMENT_SHA256: &str =
    "sha256:aa0fe83b1e10bc817d850ec15b1489c8aef3aeb33a999c6d69495bd164136441";

const KAT_SOURCE_REVISION: &str =
    "sha256:6ab6f2c32c1ac64d17745ff4e9be0eb483c16e3fed19b209961829f050196298";

const KAT_BASE_SOURCE: &str = r#"module test.openapi;

@id("api.add")
fn add(left: i64, right: i64) -> i64
    requires left >= 0
{
    left + right
}

@id("api.get")
fn get(flag: bool) -> bool { flag }

@id("app.main")
fn main() -> i64 { 0 }
"#;

const KAT_CANDIDATE_SOURCE: &str = r#"module test.openapi;

@id("api.add")
fn add(left: i64, right: bool) -> bool
    requires left > 0
{
    right
}

@id("api.ping")
fn ping(flag: bool) -> bool { flag }

@id("app.main")
fn main() -> i64 { 0 }
"#;

const KAT_INPUT_SHA256: &str =
    "sha256:c7b8649b8f876e0c48583282f3043de6f2eca671db15fd3821f983e94557493a";

#[test]
fn generation_matches_pinned_document_kat() {
    let source_path = write_source("kat-single.spx", KAT_SINGLE_SOURCE);
    let report = generate(&source_path, &["api.echo"], &OpenApiOptions::default());
    let envelope: Value = serde_json::from_str(&report).unwrap();

    assert_eq!(envelope["schema"], "semaprax.openapi.v1");
    assert_eq!(envelope["operations"], 1);
    assert_eq!(envelope["limits"]["max_functions"], 32);
    assert_eq!(envelope["limits"]["max_bytes"], 65536);
    assert_eq!(envelope["source"]["revision"], KAT_SOURCE_REVISION);
    assert_eq!(
        envelope["nonclaims"],
        serde_json::json!([
            "no_protobuf_grpc_graphql_sql",
            "no_schema_import_parsing",
            "no_live_conformance_fixtures",
            "no_registry_server_or_hosting",
            "no_target_execution",
            "read_only_no_source_changes"
        ])
    );

    // Exact payload bytes are pinned, so the digest must be independently
    // replayable from those bytes using the documented domain-separated scheme.
    assert_eq!(envelope["document"].to_string(), KAT_DOCUMENT_PAYLOAD);
    let mut hasher = Sha256::new();
    hasher.update(b"semaprax.openapi.document.v1\0");
    hasher.update((KAT_DOCUMENT_PAYLOAD.len() as u64).to_le_bytes());
    hasher.update(KAT_DOCUMENT_PAYLOAD.as_bytes());
    let recomputed = format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    );
    assert_eq!(recomputed, KAT_DOCUMENT_SHA256);
    assert_eq!(envelope["sha256"], KAT_DOCUMENT_SHA256);

    let operation = &envelope["document"]["paths"]["/api.echo"]["post"];
    assert_eq!(operation["x-stable-id"], "api.echo");
    assert_eq!(operation["operationId"], "api_echo");
    assert_eq!(
        operation["responses"]["default"]["content"]["application/json"]["schema"]["$ref"],
        "#/components/schemas/Semaprax.Status.v1"
    );
    assert_eq!(
        operation["requestBody"]["content"]["application/json"]["schema"]["$ref"],
        "#/components/schemas/api_echo.Request"
    );
    // Declared parameter order is preserved in `required` even though object
    // members themselves sort alphabetically.
    assert_eq!(
        envelope["document"]["components"]["schemas"]["api_echo.Request"]["required"],
        serde_json::json!(["zeta", "alpha"])
    );

    std::fs::remove_file(&source_path).unwrap();
}

#[test]
fn generation_is_deterministic_across_runs() {
    let source_path = write_source(
        "deterministic.spx",
        "module test.det;\n\n@id(\"det.f\")\nfn f(value: i64) -> i64 { value }\n\n@id(\"app.main\")\nfn main() -> i64 { 0 }\n",
    );
    let first = generate(&source_path, &["det.f"], &OpenApiOptions::default());
    let second = generate(&source_path, &["det.f"], &OpenApiOptions::default());
    assert_eq!(first, second, "repeated generation must be byte-identical");

    let by_name = generate(&source_path, &["f"], &OpenApiOptions::default());
    assert_eq!(
        by_name, first,
        "plain-name selection must resolve to stable id"
    );

    std::fs::remove_file(&source_path).unwrap();
}

#[test]
fn selection_limits_are_rejected_with_stable_codes() {
    let source_path = write_source("limits.spx", KAT_SINGLE_SOURCE);

    assert_eq!(
        first_error_code(&source_path, &[], &OpenApiOptions::default()),
        "SPX-OA101",
        "an empty selection set must fail closed"
    );

    let mut many =
        String::from("module test.many;\n\n@id(\"app.main\")\nfn main() -> i64 { 0 }\n\n");
    for index in 0..33 {
        many.push_str(&format!(
            "@id(\"m.f{index}\")\nfn f{index}(value: i64) -> i64 {{ value }}\n\n"
        ));
    }
    let many_path = write_source("many.spx", &many);
    let selections: Vec<String> = (0..33).map(|index| format!("m.f{index}")).collect();
    let errors =
        openapi::generate(&many_path, &selections, &OpenApiOptions::default()).unwrap_err();
    assert_eq!(
        errors[0].code, "SPX-OA101",
        "more than 32 selections must fail closed"
    );

    let duplicate = openapi::generate(
        &source_path,
        &["api.echo".to_owned(), "api.echo".to_owned()],
        &OpenApiOptions::default(),
    )
    .unwrap_err();
    assert_eq!(
        duplicate[0].code, "SPX-OA101",
        "duplicate selections must fail closed"
    );

    std::fs::remove_file(&source_path).unwrap();
    std::fs::remove_file(&many_path).unwrap();
}

#[test]
fn unknown_selection_fails_with_stable_code() {
    let source_path = write_source("unknown.spx", KAT_SINGLE_SOURCE);
    assert_eq!(
        first_error_code(&source_path, &["api.nope"], &OpenApiOptions::default()),
        "SPX-OA102"
    );
    std::fs::remove_file(&source_path).unwrap();
}

#[test]
fn excluded_constructs_report_closed_reasons() {
    let source = r#"module test.excluded;

permit { host.echo }

@id("ex.buffer.type")
resource Buffer {
    @id("ex.buffer.type.drop")
    drop trivial;
}

@id("ex.poly")
fn poly<T>(value: i64) -> i64 { value }

@id("ex.effect")
fn effect(value: i64) -> i64 uses { host.echo } { value }

@id("ex.string")
fn string(text: string) -> i64 { 0 }

@id("ex.outcome")
fn outcome(flag: bool) -> Result<bool, bool> { Result<bool, bool>::Ok { value: flag } }

@id("ex.lent")
fn lent(buffer: borrow Buffer) -> i64 { 1 }

@id("app.main")
fn main() -> i64 { 0 }
"#;
    let expected_reasons = [
        ("ex.poly", "generic_function"),
        ("ex.effect", "declared_effects"),
        ("ex.string", "unsupported_parameter_type"),
        ("ex.outcome", "unsupported_result_type"),
        ("ex.lent", "unsupported_parameter_mode"),
    ];
    let source_path = write_source("excluded.spx", source);
    for (selection, reason) in expected_reasons {
        let owned = vec![selection.to_owned()];
        let errors =
            openapi::generate(&source_path, &owned, &OpenApiOptions::default()).unwrap_err();
        assert_eq!(
            errors[0].code, "SPX-OA103",
            "selection {selection} must be excluded"
        );
        assert!(
            errors[0].message.contains(reason),
            "the exclusion message for {selection} must carry the stable reason {reason}"
        );
    }
    std::fs::remove_file(&source_path).unwrap();
}

#[test]
fn status_schema_tracks_contracts_and_signature() {
    let bool_only = write_source(
        "bool-only.spx",
        "module test.boolonly;\n\n@id(\"api.pure\")\nfn pure(flag: bool) -> bool { flag }\n\n@id(\"app.main\")\nfn main() -> i64 { 0 }\n",
    );
    let report = generate(&bool_only, &["api.pure"], &OpenApiOptions::default());
    let envelope: Value = serde_json::from_str(&report).unwrap();
    let operation = &envelope["document"]["paths"]["/api.pure"]["post"];
    assert!(
        operation["responses"].get("default").is_none(),
        "a bool-only signature without contracts has no compiler-owned failure surface"
    );
    assert!(
        envelope["document"]["components"]["schemas"]
            .get("Semaprax.Status.v1")
            .is_none(),
        "the shared status schema must not be emitted when unused"
    );
    std::fs::remove_file(&bool_only).unwrap();

    let contract_only = write_source(
        "contract-only.spx",
        "module test.contractonly;\n\n@id(\"api.guarded\")\nfn guarded(flag: bool) -> bool\n    ensures result == false || flag\n{\n    flag\n}\n\n@id(\"app.main\")\nfn main() -> i64 { 0 }\n",
    );
    let report = generate(&contract_only, &["api.guarded"], &OpenApiOptions::default());
    let envelope: Value = serde_json::from_str(&report).unwrap();
    let operation = &envelope["document"]["paths"]["/api.guarded"]["post"];
    assert!(
        operation["responses"].get("default").is_some(),
        "declared contracts alone must produce the default failure response"
    );
    std::fs::remove_file(&contract_only).unwrap();
}

#[test]
fn budget_exhaustion_fails_closed_on_generation() {
    let source_path = write_source("budget.spx", KAT_SINGLE_SOURCE);
    let options = OpenApiOptions::new(2048).unwrap();
    let errors = openapi::generate(&source_path, &["api.echo".to_owned()], &options).unwrap_err();
    assert_eq!(errors[0].code, "SPX-OA105");
    assert!(
        errors[0].message.contains("bounded-output budget"),
        "the budget diagnostic must name the bounded-output budget"
    );
    std::fs::remove_file(&source_path).unwrap();
}

fn compat_report(base_envelope: &str, candidate_envelope: &str) -> String {
    let base_path = write_source("compat-base.json", base_envelope);
    let candidate_path = write_source("compat-candidate.json", candidate_envelope);
    let report =
        openapi::compatibility(&base_path, &candidate_path, &OpenApiOptions::default()).unwrap();
    std::fs::remove_file(&base_path).unwrap();
    std::fs::remove_file(&candidate_path).unwrap();
    report
}

#[test]
fn compat_classifies_all_finding_families() {
    let base_path = write_source("kat-base.spx", KAT_BASE_SOURCE);
    let candidate_path = write_source("kat-candidate.spx", KAT_CANDIDATE_SOURCE);
    let base_envelope = generate(
        &base_path,
        &["api.add", "api.get"],
        &OpenApiOptions::default(),
    );
    let candidate_envelope = generate(
        &candidate_path,
        &["api.add", "api.ping"],
        &OpenApiOptions::default(),
    );

    let report = compat_report(&base_envelope, &candidate_envelope);
    let again = compat_report(&base_envelope, &candidate_envelope);
    assert_eq!(
        report, again,
        "the compatibility report must be deterministic"
    );

    let parsed: Value = serde_json::from_str(&report).unwrap();
    assert_eq!(parsed["schema"], "semaprax.openapi-compat.v1");
    assert_eq!(parsed["verdict"], "breaking");
    assert_eq!(
        parsed["summary"],
        serde_json::json!({"breaking": 3, "informational": 2, "non_breaking": 1})
    );
    assert_eq!(parsed["input_sha256"], KAT_INPUT_SHA256);
    assert_eq!(parsed["migration"]["major_version_bump_required"], true);

    let findings = parsed["findings"].as_array().unwrap();
    let codes: Vec<&str> = findings
        .iter()
        .map(|finding| finding["code"].as_str().unwrap())
        .collect();
    assert_eq!(
        codes,
        vec![
            "OAC-B003", "OAC-B005", "OAC-I001", "OAC-B001", "OAC-N001", "OAC-I002"
        ],
        "finding order is deterministic: shared operations first, then removals, additions, revision"
    );

    // The input binding must be replayable from the two envelope digests using
    // the documented domain-separated scheme.
    let mut hasher = Sha256::new();
    hasher.update(b"semaprax.openapi-compat.inputs.v1\0");
    for envelope in [&base_envelope, &candidate_envelope] {
        let bound: Value = serde_json::from_str(envelope).unwrap();
        let document_sha = bound["sha256"].as_str().unwrap();
        hasher.update((document_sha.len() as u64).to_le_bytes());
        hasher.update(document_sha.as_bytes());
    }
    let recomputed = format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    );
    assert_eq!(recomputed, KAT_INPUT_SHA256);

    std::fs::remove_file(&base_path).unwrap();
    std::fs::remove_file(&candidate_path).unwrap();
}

#[test]
fn compat_flags_parameter_removals_and_additions() {
    let base_source = "module test.shape;\n\n@id(\"shape.shrink\")\nfn shrink(left: i64, right: i64) -> i64 { left }\n\n@id(\"shape.grow\")\nfn grow() -> i64 { 0 }\n\n@id(\"app.main\")\nfn main() -> i64 { 0 }\n";
    let candidate_source = "module test.shape;\n\n@id(\"shape.shrink\")\nfn shrink(left: i64) -> i64 { left }\n\n@id(\"shape.grow\")\nfn grow(extra: bool) -> i64 { 0 }\n\n@id(\"app.main\")\nfn main() -> i64 { 0 }\n";
    let base_path = write_source("shape-base.spx", base_source);
    let candidate_path = write_source("shape-candidate.spx", candidate_source);
    let base_envelope = generate(
        &base_path,
        &["shape.shrink", "shape.grow"],
        &OpenApiOptions::default(),
    );
    let candidate_envelope = generate(
        &candidate_path,
        &["shape.shrink", "shape.grow"],
        &OpenApiOptions::default(),
    );

    let report = compat_report(&base_envelope, &candidate_envelope);
    let parsed: Value = serde_json::from_str(&report).unwrap();
    assert_eq!(parsed["verdict"], "breaking");
    let findings = parsed["findings"].as_array().unwrap();
    let flagged: Vec<(&str, &str)> = findings
        .iter()
        .map(|finding| {
            (
                finding["code"].as_str().unwrap(),
                finding["location"].as_str().unwrap(),
            )
        })
        .collect();
    assert!(
        flagged.contains(&("OAC-B002", "/shape.shrink:right")),
        "`right` removal must be reported as breaking: {flagged:?}"
    );
    assert!(
        flagged.contains(&("OAC-B004", "/shape.grow:extra")),
        "`extra` addition must be reported as breaking: {flagged:?}"
    );

    std::fs::remove_file(&base_path).unwrap();
    std::fs::remove_file(&candidate_path).unwrap();
}

#[test]
fn compat_identical_documents_are_compatible() {
    let source_path = write_source(
        "same.spx",
        "module test.same;\n\n@id(\"api.same\")\nfn same(value: i64) -> i64 { value }\n\n@id(\"app.main\")\nfn main() -> i64 { 0 }\n",
    );
    let envelope = generate(&source_path, &["api.same"], &OpenApiOptions::default());
    let report = compat_report(&envelope, &envelope);
    let parsed: Value = serde_json::from_str(&report).unwrap();
    assert_eq!(parsed["verdict"], "compatible");
    assert_eq!(
        parsed["summary"],
        serde_json::json!({"breaking": 0, "informational": 0, "non_breaking": 0})
    );
    assert_eq!(parsed["findings"], serde_json::json!([]));
    assert_eq!(parsed["migration"]["major_version_bump_required"], false);
    std::fs::remove_file(&source_path).unwrap();
}

#[test]
fn compat_rejects_tampered_and_foreign_inputs() {
    let source_path = write_source("tamper.spx", KAT_SINGLE_SOURCE);
    let envelope = generate(&source_path, &["api.echo"], &OpenApiOptions::default());
    std::fs::remove_file(&source_path).unwrap();

    let tampered = envelope.replace("\"int64\"", "\"int63\"");
    assert_ne!(
        envelope, tampered,
        "the tampering must actually change bytes"
    );
    let tamper_base = write_source("tampered-base.json", &envelope);
    let tamper_candidate = write_source("tampered-candidate.json", &tampered);
    let errors =
        openapi::compatibility(&tamper_base, &tamper_candidate, &OpenApiOptions::default())
            .unwrap_err();
    assert_eq!(
        errors[0].code, "SPX-OA104",
        "a digest mismatch must fail authentication"
    );
    std::fs::remove_file(&tamper_base).unwrap();
    std::fs::remove_file(&tamper_candidate).unwrap();

    for foreign in ["{}", "not json at all"] {
        let foreign_base = write_source("foreign-base.json", &envelope);
        let foreign_candidate = write_source("foreign-candidate.json", foreign);
        let errors = openapi::compatibility(
            &foreign_base,
            &foreign_candidate,
            &OpenApiOptions::default(),
        )
        .unwrap_err();
        assert_eq!(
            errors[0].code, "SPX-OA104",
            "foreign JSON must fail authentication"
        );
        std::fs::remove_file(&foreign_base).unwrap();
        std::fs::remove_file(&foreign_candidate).unwrap();
    }
}

#[test]
fn cli_rejects_unknown_options_and_missing_arity() {
    let cli = |arguments: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .args(arguments)
            .output()
            .unwrap()
    };
    let missing_function = cli(&["openapi", "examples/meaning.spx"]);
    assert_eq!(missing_function.status.code(), Some(2));
    let unknown = cli(&["openapi", "examples/meaning.spx", "--wat", "1"]);
    assert_eq!(unknown.status.code(), Some(2));
    let duplicate = cli(&[
        "openapi",
        "examples/meaning.spx",
        "--function",
        "math.add",
        "--max-bytes",
        "4096",
        "--max-bytes",
        "8192",
    ]);
    assert_eq!(duplicate.status.code(), Some(2));
    let noncanonical = cli(&[
        "openapi",
        "examples/meaning.spx",
        "--function",
        "math.add",
        "--max-bytes",
        "0100",
    ]);
    assert_eq!(noncanonical.status.code(), Some(2));
    let arity = cli(&["openapi-compat", "only-one-argument"]);
    assert_eq!(arity.status.code(), Some(2));
}
