//! Record field migration evidence authored without executing local gates.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectExecutionOptions, SemanticChange,
};
use serde_json::{json, Value};

#[allow(dead_code)]
#[path = "../support/project_product.rs"]
mod support;

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-record-field-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "record-field"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "field.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["field.unrelated"]
tests = ["field.tests"]
"#,
        )
        .unwrap();
        for (path, source) in [
            (
                "src/core.spx",
                r#"module field.core;
@id("field.pair") record Pair {
    @id("field.pair.left") left: i64,
    @id("field.pair.right") right: i64,
}
@id("field.envelope") record Envelope {
    @id("field.envelope.pair") pair: Pair,
    @id("field.envelope.marker") marker: bool,
}
@id("field.evaluate") fn evaluate(input: i64) -> i64
    requires (Pair { right: 0, left: 0 }).left == 0
{
    let pair = Pair { right: 10, left: input };
    let updated = pair with { left: pair.left + 1 };
    let outer = Envelope { pair: updated, marker: true };
    match outer { Envelope { pair: Pair { left: picked, right: other }, marker: _ } => picked + other, }
}
@id("field.unrelated") fn unrelated() -> i64 { 7 }
"#,
            ),
            (
                "src/app.spx",
                r#"module field.app;
use type @id("field.pair") from field.core as Metric;
use function @id("field.evaluate") from field.core as evaluate;
@id("field.app.main") fn main() -> i64 {
    let item = Metric { right: 3, left: 2 };
    let selected = match item { Metric { left: picked, right: _ } => picked, };
    evaluate(selected)
}
"#,
            ),
            (
                "src/tests.spx",
                r#"module field.tests;
use function @id("field.evaluate") from field.core as evaluate;
@id("field.tests.main") fn main() -> i64 { if evaluate(2) == 13 { 0 } else { 1 } }
"#,
            ),
        ] {
            let program = semaprax::parse(source, Path::new(path)).unwrap();
            std::fs::write(root.join(path), semaprax::format::canonical(&program)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn candidate(&self) -> ProjectCandidate {
        let revision = with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap();
        ProjectCandidate::open(Arc::clone(&revision), revision.project_revision()).unwrap()
    }
    fn bytes(&self) -> BTreeMap<String, Vec<u8>> {
        [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ]
        .into_iter()
        .map(|path| (path.to_owned(), std::fs::read(self.0.join(path)).unwrap()))
        .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn request() -> Value {
    json!({"kind":"add_record_field","target":"field.pair","field":{"id":"field.pair.tag","name":"tag","type":"i64","default":{"kind":"i64","value":9}}})
}
fn apply(
    candidate: &ProjectCandidate,
    intent: &Value,
) -> Result<ProjectCandidate, Vec<Diagnostic>> {
    candidate.apply(
        candidate.candidate_digest(),
        &SemanticChange::new(candidate.revision().project_revision(), intent)?,
    )
}
fn source<'a>(candidate: &'a ProjectCandidate, path: &str) -> &'a str {
    candidate
        .revision()
        .sources()
        .iter()
        .find(|source| source.path() == path)
        .unwrap()
        .source()
}
fn diagnostic<T>(result: Result<T, Vec<Diagnostic>>, code: &str) {
    match result {
        Ok(_) => panic!("expected {code}"),
        Err(errors) => assert!(errors.iter().any(|error| error.code == code), "{errors:?}"),
    }
}
fn same_outcome(before: &ProjectCandidate, after: &ProjectCandidate) {
    let options = ProjectExecutionOptions::default();
    for (label, candidate) in [
        ("record-field-before", before),
        ("record-field-after", after),
    ] {
        // Copy-record construction/update is outside the interpreter's closed profile.
        diagnostic(candidate.revision().execute_entry(&options), "SPX-F102");
        let program = candidate.revision().entry_program();
        let native = semaprax::codegen::emit_hir_c(program).unwrap();
        support::run_native_c(&native, label, "13", &["-O0", "-O2"]);
        let wasm = semaprax::wasm::emit_resolved_module(program).unwrap();
        support::run_core_wasm(&wasm, label, "13");
    }
}

#[test]
fn all_alias_constructors_contracts_and_nested_patterns_migrate_with_exact_replay() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let root = fixture.candidate();
    let change = SemanticChange::new(root.revision().project_revision(), &request()).unwrap();
    let candidate = root.apply(root.candidate_digest(), &change).unwrap();
    let core = source(&candidate, "src/core.spx");
    let app = source(&candidate, "src/app.spx");
    assert!(core.contains("Pair { right: 0, left: 0, tag: 9 }"));
    assert!(core.contains("Pair { right: 10, left: input, tag: 9 }"));
    assert!(core.contains("pair with { left: pair.left + 1 }"));
    assert!(core.contains("Pair { left: picked, right: other, tag: _ }"));
    assert!(app.contains("Metric { right: 3, left: 2, tag: 9 }"));
    assert!(app.contains("Metric { left: picked, right: _, tag: _ }"));
    let graph: Value = serde_json::from_str(candidate.revision().semantic_graph()).unwrap();
    let addition = graph["declarations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == "field.pair.tag")
        .unwrap();
    assert_eq!(addition["kind"], "field");
    assert_eq!(addition["owner"], "field.pair");
    assert_eq!(addition["path"], "src/core.spx");
    assert_eq!(addition["identity_origin"], "explicit");
    same_outcome(&root, &candidate);
    let replay = ProjectCandidate::replay(
        Arc::clone(root.base_revision()),
        root.base_revision().project_revision(),
        std::slice::from_ref(&change),
        candidate.to_json().as_bytes(),
    )
    .unwrap();
    assert_eq!(replay.to_json(), candidate.to_json());
    let capsule = candidate.recovery_capsule().unwrap();
    let restored = ProjectCandidate::restore(
        Arc::clone(root.base_revision()),
        root.base_revision().project_revision(),
        capsule.as_bytes(),
    )
    .unwrap();
    assert_eq!(restored.to_json(), candidate.to_json());
    diagnostic(
        candidate.apply(candidate.candidate_digest(), &change),
        "SPX-G224",
    );
    let mut tampered = candidate.to_json().as_bytes().to_vec();
    tampered.push(b' ');
    diagnostic(
        ProjectCandidate::replay(
            Arc::clone(root.base_revision()),
            root.base_revision().project_revision(),
            &[change],
            &tampered,
        ),
        "SPX-G224",
    );
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn existing_binding_order_and_lazy_failure_are_preserved() {
    let fixture = Fixture::new();
    let path = fixture.0.join("src/app.spx");
    let original = std::fs::read_to_string(&path).unwrap();
    let changed=original.replace("Metric { right: 3, left: 2 }","if true { Metric { right: 3, left: 2 } } else { Metric { right: 1 / 0, left: 9223372036854775807 + 1 } }");
    let parsed = semaprax::parse(&changed, Path::new("src/app.spx")).unwrap();
    std::fs::write(path, semaprax::format::canonical(&parsed)).unwrap();
    let root = fixture.candidate();
    let candidate = apply(&root, &request()).unwrap();
    assert!(source(&candidate, "src/app.spx")
        .contains("Metric { right: 1 / 0, left: 9223372036854775807 + 1, tag: 9 }"));
    same_outcome(&root, &candidate);
}

#[test]
fn default_grammar_collisions_and_nonrecord_targets_fail_without_writes() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let root = fixture.candidate();
    for mutation in 0..6 {
        let mut intent = request();
        match mutation {
            0 => {
                intent["field"]["default"] =
                    json!({"kind":"call","target":"field.unrelated","arguments":[]})
            }
            1 => intent["field"]["default"] = json!({"kind":"bool","value":true}),
            2 => intent["field"]["id"] = json!("field.pair.left"),
            3 => intent["field"]["name"] = json!("left"),
            4 => intent["target"] = json!("field.evaluate"),
            _ => intent["field"]["default"]["unknown"] = json!(0),
        }
        diagnostic(apply(&root, &intent), "SPX-G225");
    }
    let mut boolean = request();
    boolean["field"]["type"] = json!("bool");
    boolean["field"]["default"] = json!({"kind":"bool","value":false});
    let candidate = apply(&root, &boolean).unwrap();
    assert!(source(&candidate, "src/app.spx").contains("tag: false"));
    same_outcome(&root, &candidate);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn generic_records_reject_while_unused_flat_owned_records_accept_scalar_fields() {
    let fixture = Fixture::new();
    let path = fixture.0.join("src/core.spx");
    let original = std::fs::read_to_string(&path).unwrap();
    let changed = format!(
        r#"{original}
@id("field.generic") record Generic<T> {{ @id("field.generic.value") value: T, }}
@id("field.owned") record Owned {{ @id("field.owned.bytes") bytes: Bytes, }}
"#
    );
    let program = semaprax::parse(&changed, Path::new("src/core.spx")).unwrap();
    std::fs::write(path, semaprax::format::canonical(&program)).unwrap();
    let root = fixture.candidate();
    let disk = fixture.bytes();
    let mut generic = request();
    generic["target"] = json!("field.generic");
    diagnostic(apply(&root, &generic), "SPX-G225");

    // No function mentions Owned: selection reconstructs its checked facts
    // without inventing a source consumer or relying on retained instances.
    let mut owned = request();
    owned["target"] = json!("field.owned");
    owned["field"]["id"] = json!("field.owned.tag");
    let candidate = apply(&root, &owned).unwrap();
    let parsed = semaprax::parse(
        source(&candidate, "src/core.spx"),
        Path::new("src/core.spx"),
    )
    .unwrap();
    let declaration = parsed
        .types
        .iter()
        .find(|ty| ty.stable_id == "field.owned")
        .unwrap();
    let semaprax::ast::TypeDeclarationKind::Record { fields } = &declaration.kind else {
        panic!("owned record disappeared")
    };
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].stable_id, "field.owned.bytes");
    assert_eq!(fields[0].name, "bytes");
    assert_eq!(fields[0].ty, semaprax::ast::Type::Bytes);
    assert_eq!(fields[1].stable_id, "field.owned.tag");
    assert_eq!(fields[1].name, "tag");
    assert_eq!(fields[1].ty, semaprax::ast::Type::I64);
    let restored = ProjectCandidate::restore(
        Arc::clone(candidate.base_revision()),
        candidate.base_revision().project_revision(),
        candidate.recovery_capsule().unwrap().as_bytes(),
    )
    .unwrap();
    assert_eq!(restored.to_json(), candidate.to_json());
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn merge_replays_field_migration_after_unrelated_rename_and_rejects_competing_shape() {
    let fixture = Fixture::new();
    let root = fixture.candidate();
    let left = apply(&root, &request()).unwrap();
    let right = apply(
        &root,
        &json!({"kind":"rename_declaration","target":"field.unrelated","name":"different"}),
    )
    .unwrap();
    let merged = left
        .merge(left.candidate_digest(), &right, right.candidate_digest())
        .unwrap();
    assert!(source(merged.candidate(), "src/core.spx").contains("fn different("));
    assert!(source(merged.candidate(), "src/core.spx").contains("tag: i64"));
    same_outcome(&root, merged.candidate());
    let mut conflict = request();
    conflict["field"]["id"] = json!("field.pair.other");
    conflict["field"]["name"] = json!("other");
    let conflict = apply(&root, &conflict).unwrap();
    diagnostic(
        left.merge(
            left.candidate_digest(),
            &conflict,
            conflict.candidate_digest(),
        ),
        "SPX-G235",
    );
}

fn add_private_owned_field_fixture(fixture: &Fixture) {
    let path = fixture.0.join("src/core.spx");
    let original = std::fs::read_to_string(&path).unwrap();
    let changed = format!(
        r#"{original}
@id("field.seed") record Seed {{
    @id("field.seed.number") number: i64,
}}
@id("field.preowned") record Preowned {{
    @id("field.preowned.bytes") bytes: Bytes,
}}
@id("field.seed.consume") fn consume_seed(input: i64) -> i64 {{
    let seed = Seed {{ number: input }};
    let updated = seed with {{ number: input + 1 }};
    let other = Seed {{ number: updated.number }};
    other.number
}}
"#
    );
    let program = semaprax::parse(&changed, Path::new("src/core.spx")).unwrap();
    std::fs::write(path, semaprax::format::canonical(&program)).unwrap();
}

fn owning_request(field_type: &str) -> Value {
    let default = match field_type {
        "string" => json!({"kind":"string","value":"agent-owned"}),
        "Bytes" => json!({"kind":"Bytes","values":[0,1,127,255]}),
        _ => unreachable!(),
    };
    json!({
        "kind":"add_record_field",
        "target":"field.seed",
        "field":{
            "id":format!("field.seed.{}", field_type.to_ascii_lowercase()),
            "name":format!("{}_payload", field_type.to_ascii_lowercase()),
            "type":field_type,
            "default":default,
        },
    })
}

#[test]
fn private_copy_record_constructors_gain_fresh_string_and_bytes_owners() {
    for field_type in ["string", "Bytes"] {
        let fixture = Fixture::new();
        add_private_owned_field_fixture(&fixture);
        let disk = fixture.bytes();
        let root = fixture.candidate();
        let change = SemanticChange::new(
            root.revision().project_revision(),
            &owning_request(field_type),
        )
        .unwrap();
        let candidate = root.apply(root.candidate_digest(), &change).unwrap();
        let core = source(&candidate, "src/core.spx");
        if field_type == "string" {
            assert!(core.contains("string_payload: string"));
            assert_eq!(core.matches("string_payload: \"agent-owned\"").count(), 2);
        } else {
            assert!(core.contains("bytes_payload: Bytes"));
            assert_eq!(
                core.matches("bytes_payload: bytes_copy(array_as_slice([0u8, 1u8, 127u8, 255u8]))")
                    .count(),
                2
            );
        }
        assert!(core.contains("let updated = seed with { number: input + 1 };"));
        let graph: Value = serde_json::from_str(candidate.revision().semantic_graph()).unwrap();
        assert!(graph["declarations"].as_array().unwrap().iter().any(|row| {
            row["id"] == format!("field.seed.{}", field_type.to_ascii_lowercase())
                && row["owner"] == "field.seed"
        }));
        let ownership: Value = serde_json::from_str(
            &candidate
                .ownership_delta(candidate.candidate_digest())
                .unwrap(),
        )
        .unwrap();
        let changed = ownership["functions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == "field.seed.consume")
            .unwrap();
        assert_eq!(changed["comparison"]["cleanup_inventory_equal"], false);
        assert!(!changed["candidate"]["cleanup_inventory"]["slots"]
            .as_array()
            .unwrap()
            .is_empty());
        let replay = ProjectCandidate::replay(
            Arc::clone(root.base_revision()),
            root.base_revision().project_revision(),
            &[change],
            candidate.to_json().as_bytes(),
        )
        .unwrap();
        assert_eq!(replay.to_json(), candidate.to_json());
        assert_eq!(fixture.bytes(), disk);
    }

    let fixture = Fixture::new();
    add_private_owned_field_fixture(&fixture);
    let root = fixture.candidate();
    let mut empty = owning_request("string");
    empty["field"]["default"]["value"] = json!("");
    let candidate = apply(&root, &empty).unwrap();
    assert!(source(&candidate, "src/core.spx").contains("string_payload: \"\""));
}

#[test]
fn owning_field_lane_rejects_patterns_existing_owners_and_unbounded_defaults() {
    let fixture = Fixture::new();
    let root = fixture.candidate();
    let mut pattern = request();
    pattern["field"]["type"] = json!("string");
    pattern["field"]["default"] = json!({"kind":"string","value":"x"});
    diagnostic(apply(&root, &pattern), "SPX-G225");

    add_private_owned_field_fixture(&fixture);
    let root = fixture.candidate();
    let mut oversized = owning_request("Bytes");
    oversized["field"]["default"]["values"] = json!(vec![0u8; 4094]);
    diagnostic(apply(&root, &oversized), "SPX-G226");

    let mut oversized_string = owning_request("string");
    oversized_string["field"]["default"]["value"] = json!("é".repeat(4097));
    diagnostic(apply(&root, &oversized_string), "SPX-G226");

    let mut mismatch = owning_request("Bytes");
    mismatch["field"]["default"] = json!({"kind":"string","value":"x"});
    diagnostic(apply(&root, &mismatch), "SPX-G225");

    let mut already_owned = owning_request("string");
    already_owned["target"] = json!("field.preowned");
    already_owned["field"]["id"] = json!("field.preowned.text");
    diagnostic(apply(&root, &already_owned), "SPX-G225");
}

#[test]
fn bytes_default_history_reserves_its_implicit_builtin_spellings() {
    for name in ["bytes_copy", "array_as_slice"] {
        let fixture = Fixture::new();
        add_private_owned_field_fixture(&fixture);
        let root = fixture.candidate();
        let candidate = apply(&root, &owning_request("Bytes")).unwrap();
        diagnostic(
            apply(
                &candidate,
                &json!({
                    "kind":"rename_declaration",
                    "target":"field.unrelated",
                    "name":name
                }),
            ),
            "SPX-G225",
        );
    }
}

#[test]
fn bytes_default_rebase_rejects_concurrent_implicit_builtin_spelling() {
    let fixture = Fixture::new();
    add_private_owned_field_fixture(&fixture);
    let root = fixture.candidate();
    let candidate = apply(&root, &owning_request("Bytes")).unwrap();
    let path = fixture.0.join("src/core.spx");
    let source = std::fs::read_to_string(&path)
        .unwrap()
        .replace("fn unrelated()", "fn bytes_copy()");
    let parsed = semaprax::parse(&source, Path::new("src/core.spx")).unwrap();
    std::fs::write(&path, semaprax::format::canonical(&parsed)).unwrap();
    let concurrent = with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        Ok(snapshot.retain_revision())
    })
    .unwrap();
    diagnostic(
        candidate.rebase(
            candidate.candidate_digest(),
            Arc::clone(&concurrent),
            concurrent.project_revision(),
        ),
        "SPX-G235",
    );
}

#[test]
fn rebase_binds_resolved_nominal_field_identity_beneath_unchanged_alias_spelling() {
    let fixture = Fixture::new();
    let core_path = fixture.0.join("src/core.spx");
    let core = format!(
        "{}\n{}",
        std::fs::read_to_string(&core_path).unwrap(),
        r#"@id("field.pair2") record Pair2 {
    @id("field.pair2.left") left: i64,
    @id("field.pair2.right") right: i64,
}"#
    );
    let core = semaprax::parse(&core, Path::new("src/core.spx")).unwrap();
    std::fs::write(&core_path, semaprax::format::canonical(&core)).unwrap();
    let app_path = fixture.0.join("src/app.spx");
    let app = format!(
        "{}\n{}",
        std::fs::read_to_string(&app_path).unwrap(),
        r#"@id("field.wrapper") record Wrapper {
    @id("field.wrapper.child") child: Metric,
}
@id("field.wrapper.read") fn read_wrapper() -> i64 {
    let wrapper = Wrapper { child: Metric { right: 2, left: 1 } };
    wrapper.child.left
}"#
    );
    let app = semaprax::parse(&app, Path::new("src/app.spx")).unwrap();
    std::fs::write(&app_path, semaprax::format::canonical(&app)).unwrap();
    let root = fixture.candidate();
    let intent = json!({
        "kind":"add_record_field",
        "target":"field.wrapper",
        "field":{"id":"field.wrapper.tag","name":"tag","type":"i64","default":{"kind":"i64","value":7}}
    });
    let candidate = apply(&root, &intent).unwrap();

    let rebound = std::fs::read_to_string(&app_path).unwrap().replacen(
        "use type @id(\"field.pair\") from field.core as Metric;",
        "use type @id(\"field.pair2\") from field.core as Metric;",
        1,
    );
    let rebound = semaprax::parse(&rebound, Path::new("src/app.spx")).unwrap();
    std::fs::write(&app_path, semaprax::format::canonical(&rebound)).unwrap();
    let concurrent = with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        Ok(snapshot.retain_revision())
    })
    .unwrap();
    diagnostic(
        candidate.rebase(
            candidate.candidate_digest(),
            Arc::clone(&concurrent),
            concurrent.project_revision(),
        ),
        "SPX-G235",
    );
}
