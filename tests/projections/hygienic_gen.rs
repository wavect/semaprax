use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use semaprax::digest_hex::LowerHex;
use semaprax::hygienic::{self, HygienicGenOptions, Template};
use serde_json::Value;
use sha2::{Digest as _, Sha256};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn write_source(name: &str, source: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-hygienic-gen-v1-{}-{}-{name}.spx",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::write(&path, source).unwrap();
    path
}

fn generate_value(source_path: &Path, options: &HygienicGenOptions) -> Value {
    let report = hygienic::generate(source_path, options).unwrap();
    serde_json::from_str(&report).unwrap()
}

/// Independently recompute the domain-separated outer digest from the
/// reported bytes. The digest covers the full payload object including its
/// final brace; the envelope trims that brace before appending the suffix.
fn verify_outer_digest(report_text: &str) {
    // Suffix: `,"outer_sha256":"` + `sha256:<64 hex>` + `"}` = 90 bytes.
    const SUFFIX_BYTES: usize = 17 + 71 + 2;
    let cut = report_text.len() - SUFFIX_BYTES;
    let mut hasher = Sha256::new();
    hasher.update(b"semaprax.hygienic-gen.v1:outer-digest.v1\0");
    hasher.update(&report_text.as_bytes()[..cut]);
    hasher.update(b"}");
    let expected = format!("sha256:{:x}", LowerHex(hasher.finalize()));
    let embedded = &report_text[cut + 17..report_text.len() - 2];
    assert_eq!(embedded, expected);
}

const GOLDEN_SOURCE: &str = r#"
module test.gen;

@id("gen.point")
record Point {
    @id("gen.point.x")
    x: i64,
    @id("gen.point.flag")
    flag: bool,
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

#[test]
fn golden_known_answer_pins_names_identities_and_digests() {
    let path = write_source("golden.spx", GOLDEN_SOURCE);
    let text = hygienic::generate(&path, &HygienicGenOptions::default()).unwrap();
    verify_outer_digest(&text);
    let report: Value = serde_json::from_str(&text).unwrap();

    assert_eq!(report["schema"], "semaprax.hygienic-gen.v1");
    assert_eq!(
        report["registry"],
        serde_json::json!(["default-constructor", "field-accessors"])
    );
    assert_eq!(
        report["templates"],
        serde_json::json!(["default-constructor", "field-accessors"])
    );
    assert_eq!(report["limits"]["max_bytes"], 65536);

    // The base revision binds the canonical source; source.revision repeats it
    // and combined.base_revision must agree.
    assert_eq!(
        report["source"]["revision"], report["combined"]["base_revision"],
        "base revision must bind the unmodified input"
    );
    assert!(report["source"]["sha256"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));

    assert_eq!(
        report["types"],
        serde_json::json!({
            "total": 1,
            "admitted": [{
                "name": "Point",
                "stable_id": "gen.point",
                "fields": [
                    {"name": "x", "type": "i64"},
                    {"name": "flag", "type": "bool"},
                ],
            }],
            "excluded": [],
        })
    );
    assert_eq!(
        report["functions"],
        serde_json::json!({"total": 1, "admitted": 1, "excluded": []})
    );

    let generated = report["generated"].as_array().unwrap();
    let expected: &[(&str, &str, &str, &str, Value)] = &[
        (
            "default-constructor",
            "__gen_91827de5_default",
            "auto:test.gen.__gen_91827de5_default",
            "sha256:eefc39956e0469697ad365bfe009dc645a8ddee4cb0f8beca7117067e8809c31",
            serde_json::json!({"params": 0, "result": "Point", "tail": "construct_record"}),
        ),
        (
            "field-accessors",
            "__gen_91827de5_get_x",
            "auto:test.gen.__gen_91827de5_get_x",
            "sha256:94cbb78526eca1f476f9a5546c14fa5e1a823d764fe50d18b79e5f4648dbf6af",
            serde_json::json!({"params": 1, "result": "i64", "tail": "project"}),
        ),
        (
            "field-accessors",
            "__gen_91827de5_get_flag",
            "auto:test.gen.__gen_91827de5_get_flag",
            "sha256:44c5fc8e2f6227ba52992c89785dbad01bb10a181d39a6d95a20cd41e372c1f4",
            serde_json::json!({"params": 1, "result": "bool", "tail": "project"}),
        ),
    ];
    assert_eq!(
        generated.len(),
        expected.len(),
        "one artifact per template instance"
    );
    for (entry, (template, name, identity, digest, ast)) in generated.iter().zip(expected) {
        assert_eq!(entry["template"], *template);
        assert_eq!(entry["record"], "Point");
        assert_eq!(entry["record_stable_id"], "gen.point");
        if entry["template"] == "default-constructor" {
            assert_eq!(entry["field"], Value::Null);
        } else {
            assert!(entry["field"].is_string());
        }
        assert_eq!(entry["name"], *name, "derived names are pinned");
        assert_eq!(
            entry["resolved_id"], *identity,
            "resolved identities are pinned"
        );
        assert_eq!(
            entry["formatted_sha256"], *digest,
            "formatted digests are pinned"
        );
        assert_eq!(entry["ast"], *ast);
    }

    assert_eq!(
        report["budget"],
        serde_json::json!({"generated_total": 3, "generated_emitted": 3})
    );
    assert_eq!(
        report["truncation"],
        serde_json::json!({"truncated": false, "reasons": [], "omitted_generated": 0})
    );
    assert_eq!(
        report["nonclaims"],
        serde_json::json!([
            "no_unrestricted_textual_rewriting",
            "no_macro_system",
            "no_cross_file_scope",
            "read_only_no_source_mutation",
            "no_persistent_artifacts",
            "no_target_execution",
        ])
    );
    assert_eq!(
        report["combined"]["base_graph_schema"],
        "semaprax.graph.v10"
    );
    assert_eq!(report["combined"]["graph_schema"], "semaprax.graph.v10");
    assert_eq!(report["combined"]["base_function_nodes"], 1);
    assert_eq!(report["combined"]["function_nodes"], 4);
    assert_ne!(
        report["combined"]["revision"], report["combined"]["base_revision"],
        "the combined revision must differ once generated code exists"
    );
}

#[test]
fn generation_is_deterministic_across_runs() {
    let path = write_source("determinism.spx", GOLDEN_SOURCE);
    let options = HygienicGenOptions::default();
    let first = hygienic::generate(&path, &options).unwrap();
    let second = hygienic::generate(&path, &options).unwrap();
    assert_eq!(first, second, "repeated generation must be byte-identical");
}

#[test]
fn rename_with_same_stable_id_preserves_derived_names() {
    let renamed = GOLDEN_SOURCE.replace("record Point {", "record Pt {");
    assert_ne!(renamed, GOLDEN_SOURCE);
    let baseline_path = write_source("baseline.spx", GOLDEN_SOURCE);
    let renamed_path = write_source("renamed.spx", &renamed);
    let baseline = generate_value(&baseline_path, &HygienicGenOptions::default());
    let renamed_report = generate_value(&renamed_path, &HygienicGenOptions::default());

    let names = |report: &Value| -> Vec<String> {
        report["generated"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["name"].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(names(&baseline), names(&renamed_report));

    // The formatted text legitimately embeds the display type name, so those
    // digests move while identities stay put.
    let digests = |report: &Value| -> Vec<String> {
        report["generated"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["formatted_sha256"].as_str().unwrap().to_owned())
            .collect()
    };
    assert_ne!(digests(&baseline), digests(&renamed_report));
}

#[test]
fn moving_code_and_comments_preserves_all_artifact_digests() {
    let moved = r#"
// a leading comment that canonical formatting must erase

module test.gen;

@id("app.main")
fn main() -> i64 { 0 }

@id("gen.point") // an inline comment on the record
record Point {
    @id("gen.point.x")
    x: i64,
    @id("gen.point.flag")
    flag: bool,
}
"#;
    let golden_path = write_source("golden2.spx", GOLDEN_SOURCE);
    let moved_path = write_source("moved.spx", moved);
    let golden = generate_value(&golden_path, &HygienicGenOptions::default());
    let moved_report = generate_value(&moved_path, &HygienicGenOptions::default());

    // Canonical formatting erases movement and comments, so every digest is
    // stable even though field order changed in the source text.
    let signature = |report: &Value| -> Vec<(String, String, String)> {
        report["generated"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| {
                (
                    entry["name"].as_str().unwrap().to_owned(),
                    entry["resolved_id"].as_str().unwrap().to_owned(),
                    entry["formatted_sha256"].as_str().unwrap().to_owned(),
                )
            })
            .collect()
    };
    assert_eq!(signature(&golden), signature(&moved_report));

    // Swapping field order keeps names and identities but legitimately moves
    // the constructor's formatted digest, because construction order is
    // semantic in SPX.
    let swapped = r#"
module test.gen;

@id("app.main")
fn main() -> i64 { 0 }

@id("gen.point")
record Point {
    @id("gen.point.flag")
    flag: bool,
    @id("gen.point.x")
    x: i64,
}
"#;
    let swapped_path = write_source("swapped.spx", swapped);
    let swapped_report = generate_value(&swapped_path, &HygienicGenOptions::default());
    let golden_names = signature(&golden)
        .into_iter()
        .map(|(name, identity, _)| (name, identity))
        .collect::<std::collections::BTreeSet<_>>();
    let swapped_names = signature(&swapped_report)
        .into_iter()
        .map(|(name, identity, _)| (name, identity))
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(golden_names, swapped_names);
    let constructor_digest = |report: &Value| -> String {
        signature(report)
            .iter()
            .find(|(name, _, _)| name.ends_with("_default"))
            .unwrap()
            .2
            .clone()
    };
    assert_ne!(
        constructor_digest(&golden),
        constructor_digest(&swapped_report),
        "construction order is semantic; the digest must move"
    );
}

#[test]
fn user_symbol_collision_fails_closed_with_spx_y102() {
    let derived = hygienic::default_constructor_name("gen.point");
    let source = format!(
        r#"
module test.gen;

@id("gen.point")
record Point {{
    @id("gen.point.x")
    x: i64,
}}

@id("user.clash")
fn {derived}() -> i64 {{ 0 }}

@id("app.main")
fn main() -> i64 {{ 0 }}
"#
    );
    let path = write_source("clash.spx", &source);
    let errors = hygienic::generate(&path, &HygienicGenOptions::default()).unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, "SPX-Y102");
    assert!(errors[0].message.contains(&derived));

    // The source file must be untouched by the failed run.
    assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
}

#[test]
fn reserved_prefix_preemption_fails_closed_with_spx_y103() {
    let source = r#"
module test.gen;

@id("gen.point")
record Point {
    @id("gen.point.x")
    x: i64,
}

@id("user.reserved")
fn __gen_user_helper(value: i64) -> i64 { value }

@id("app.main")
fn main() -> i64 { 0 }
"#;
    let path = write_source("reserved.spx", source);
    let errors = hygienic::generate(&path, &HygienicGenOptions::default()).unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, "SPX-Y103");
}

#[test]
fn base_program_diagnostics_pass_through_unchanged() {
    let source = r#"
module test.gen;

@id("gen.point")
record Point {
    @id("gen.point.x")
    x: i64,
}

@id("app.broken")
fn broken() -> i64 { true }
"#;
    let path = write_source("broken.spx", source);
    let errors = hygienic::generate(&path, &HygienicGenOptions::default()).unwrap_err();
    assert!(
        !errors.is_empty(),
        "verifier diagnostics must surface verbatim"
    );
    assert!(
        errors.iter().all(|error| !error.code.starts_with("SPX-Y")),
        "no hygienic-generation codes may wrap verifier diagnostics"
    );
}

#[test]
fn excluded_inventory_reports_closed_reasons() {
    let source = r#"
module test.probe;

permit { clock.read }

variant Choice {
    None,
    Number { value: i64, },
}

record Wrapped<T> {
    inner: T,
}

record Empty {}

record Pair {
    left: i64,
    right: bool,
}

// A record-typed field makes the record non-scalar without being an error.
record Holder {
    pair: Pair,
}

@id("probe.generic")
fn generic<T>(value: T) -> T { value }

@id("probe.effectful")
fn effectful(value: i64) -> i64
    uses { clock.read }
{ value + 1 }

@id("probe.constructs")
fn constructs(value: i64) -> i64 {
    let pair = Pair { left: value, right: true };
    pair.left
}

@id("app.main")
fn main() -> i64 { 0 }
"#;
    let path = write_source("excluded.spx", source);
    let report = generate_value(&path, &HygienicGenOptions::default());

    let types = &report["types"];
    assert_eq!(types["total"], 5);
    let excluded = types["excluded"].as_array().unwrap();
    let reason_of = |name: &str| -> &str {
        excluded
            .iter()
            .find(|entry| entry["name"] == name)
            .unwrap_or_else(|| panic!("missing exclusion for {name}"))["reason"]
            .as_str()
            .unwrap()
    };
    assert_eq!(reason_of("Choice"), "variant_declaration");
    assert_eq!(reason_of("Wrapped"), "generic_record");
    assert_eq!(reason_of("Empty"), "empty_record");
    assert_eq!(reason_of("Holder"), "non_scalar_field");

    // Only the fully scalar record generates anything.
    let admitted: Vec<&str> = types["admitted"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["name"].as_str().unwrap())
        .collect();
    assert_eq!(admitted, vec!["Pair"]);

    let functions = &report["functions"];
    assert_eq!(functions["total"], 4);
    assert_eq!(functions["admitted"], 1, "only app.main survives the scan");
    let excluded_functions = functions["excluded"].as_array().unwrap();
    let function_reason_of = |stable_id: &str| -> &str {
        excluded_functions
            .iter()
            .find(|entry| entry["stable_id"] == stable_id)
            .unwrap_or_else(|| panic!("missing function exclusion for {stable_id}"))["reason"]
            .as_str()
            .unwrap()
    };
    assert_eq!(function_reason_of("probe.generic"), "generic_function");
    assert_eq!(function_reason_of("probe.effectful"), "declared_effects");
    assert_eq!(
        function_reason_of("probe.constructs"),
        "record_construction"
    );

    let generated = report["generated"].as_array().unwrap();
    assert_eq!(generated.len(), 3);
    assert_eq!(report["budget"]["generated_total"], 3);
}

#[test]
fn resources_and_interfaces_exclude_with_closed_reasons() {
    let lifecycle = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/lifecycle.spx");
    let report = generate_value(&lifecycle, &HygienicGenOptions::default());
    let excluded = report["types"]["excluded"].as_array().unwrap();
    let reason_of = |name: &str| -> String {
        excluded
            .iter()
            .find(|entry| entry["name"] == name)
            .unwrap_or_else(|| panic!("missing exclusion for {name}"))["reason"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    assert_eq!(reason_of("Token"), "resource_declaration");
    assert_eq!(reason_of("TokenHost"), "interface_declaration");
}

#[test]
fn template_selection_limits_generation_and_validates_input() {
    let path = write_source("templates.spx", GOLDEN_SOURCE);
    let only_accessors = HygienicGenOptions::new(&[Template::FieldAccessors], 64 * 1024).unwrap();
    let report = generate_value(&path, &only_accessors);
    assert_eq!(report["templates"], serde_json::json!(["field-accessors"]));
    let generated = report["generated"].as_array().unwrap();
    assert_eq!(generated.len(), 2);
    assert!(
        generated
            .iter()
            .all(|entry| entry["template"] == "field-accessors"),
        "constructor entries must vanish under a narrower selection"
    );
    assert_eq!(
        report["registry"],
        serde_json::json!(["default-constructor", "field-accessors"])
    );

    let duplicate = HygienicGenOptions::new(
        &[Template::FieldAccessors, Template::FieldAccessors],
        64 * 1024,
    )
    .unwrap_err();
    assert_eq!(duplicate.code, "SPX-Y100");
    let empty = HygienicGenOptions::new(&[], 64 * 1024).unwrap_err();
    assert_eq!(empty.code, "SPX-Y100");
    let too_small = HygienicGenOptions::new(&[Template::DefaultConstructor], 16).unwrap_err();
    assert_eq!(too_small.code, "SPX-Y100");
    assert_eq!(Template::from_id("mystery"), None);
}

#[test]
fn byte_budget_truncates_prefix_and_keeps_json_valid() {
    let mut records = String::new();
    for index in 1..=6 {
        records.push_str(&format!(
            "\n@id(\"gen.r{index}\")\nrecord R{index} {{\n    @id(\"gen.r{index}.x\")\n    x: i64,\n    @id(\"gen.r{index}.flag\")\n    flag: bool,\n}}\n"
        ));
    }
    let source =
        format!("module test.wide;\n{records}\n@id(\"app.main\")\nfn main() -> i64 {{ 0 }}\n");
    let path = write_source("wide.spx", &source);

    let truncated = generate_value(
        &path,
        &HygienicGenOptions::new(&[Template::DefaultConstructor], 6200).unwrap(),
    );
    assert_eq!(truncated["budget"]["generated_total"], 6);
    assert_eq!(truncated["budget"]["generated_emitted"], 2);
    assert_eq!(truncated["truncation"]["truncated"], true);
    assert_eq!(
        truncated["truncation"]["reasons"],
        serde_json::json!(["byte_budget"])
    );
    assert_eq!(truncated["truncation"]["omitted_generated"], 4);
    let names: Vec<&str> = truncated["generated"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names.len(),
        2,
        "canonical prefix order must survive truncation"
    );

    let roomier = generate_value(
        &path,
        &HygienicGenOptions::new(&[Template::DefaultConstructor], 10000).unwrap(),
    );
    assert_eq!(roomier["budget"]["generated_emitted"], 6);
    assert_eq!(roomier["truncation"]["truncated"], false);
    assert_eq!(roomier["truncation"]["omitted_generated"], 0);
}

#[test]
fn envelope_floor_fails_closed_with_spx_y105() {
    let path = write_source("floor.spx", GOLDEN_SOURCE);
    let options = HygienicGenOptions::new(
        &[Template::DefaultConstructor, Template::FieldAccessors],
        2048,
    )
    .unwrap();
    let errors = hygienic::generate(&path, &options).unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, "SPX-Y105");
    assert!(errors[0].message.contains("2048"));
}

#[test]
fn generation_never_mutates_the_source_file() {
    let path = write_source("readonly.spx", GOLDEN_SOURCE);
    let before = std::fs::read(&path).unwrap();
    let _ = generate_value(&path, &HygienicGenOptions::default());
    let after = std::fs::read(&path).unwrap();
    assert_eq!(before, after, "generation must be read-only");
    assert_eq!(
        hygienic::NONCLAIMS,
        [
            "no_unrestricted_textual_rewriting",
            "no_macro_system",
            "no_cross_file_scope",
            "read_only_no_source_mutation",
            "no_persistent_artifacts",
            "no_target_execution",
        ]
    );
}

#[test]
fn cli_rejects_unknown_duplicate_and_bad_template_options() {
    let golden = write_source("cli-golden.spx", GOLDEN_SOURCE);
    let golden_path = golden.to_string_lossy().into_owned();
    let cli = |arguments: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .args(arguments)
            .output()
            .unwrap()
    };
    let missing = cli(&["hygienic-gen"]);
    assert_eq!(missing.status.code(), Some(2));
    let unknown = cli(&["hygienic-gen", &golden_path, "--seed", "1"]);
    assert_eq!(unknown.status.code(), Some(2));
    let duplicate = cli(&[
        "hygienic-gen",
        &golden_path,
        "--max-bytes",
        "4096",
        "--max-bytes",
        "8192",
    ]);
    assert_eq!(duplicate.status.code(), Some(2));
    let bad_template = cli(&["hygienic-gen", &golden_path, "--templates", "mystery"]);
    assert_eq!(bad_template.status.code(), Some(2));
    let duplicate_template = cli(&[
        "hygienic-gen",
        &golden_path,
        "--templates",
        "field-accessors,field-accessors",
    ]);
    assert_eq!(duplicate_template.status.code(), Some(2));
    let missing_value = cli(&["hygienic-gen", &golden_path, "--max-bytes"]);
    assert_eq!(missing_value.status.code(), Some(2));

    let success = cli(&["hygienic-gen", &golden_path]);
    assert!(
        success.status.success(),
        "a clean program must generate: {}",
        String::from_utf8_lossy(&success.stderr)
    );
}
