//! Semantic delta regressions authored without running tests or compilers.
use semaprax::project::{with_authenticated_project, ProjectCandidate, SemanticChange};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-semantic-delta-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in [
            "semaprax.toml",
            "src/core.spx",
            "src/app.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(example.join(path), root.join(path)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn open(&self) -> ProjectCandidate {
        let revision = with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap();
        ProjectCandidate::open(Arc::clone(&revision), revision.project_revision()).unwrap()
    }
    fn record_fixture() -> Self {
        let fixture = Self::new();
        let manifest = fixture.0.join("semaprax.toml");
        let original = std::fs::read_to_string(&manifest).unwrap();
        std::fs::write(
            manifest,
            original
                .replace("semaprax.project.v1", "semaprax.project.v8")
                .replace(
                    "name = \"calculator\"",
                    "name = \"calculator\"\nversion = \"1.0.0\"\nprofile = \"owned-data-api.v1\"",
                )
                .replace("\"calculator.divide\", ", ""),
        )
        .unwrap();
        let core = fixture.0.join("src/core.spx");
        let source = format!(
            r#"{}
@id("delta.pair") record Pair {{ @id("delta.pair.value") value: i64, }}
@id("delta.record-value") fn record_value(input: i64) -> i64 {{
    let mut pair = Pair {{ value: input }};
    pair.value = pair.value + 1;
    match pair {{ Pair {{ value: picked }} => picked, }}
}}
"#,
            std::fs::read_to_string(&core).unwrap()
        );
        let parsed = semaprax::parse(&source, Path::new("src/core.spx")).unwrap();
        std::fs::write(core, semaprax::format::canonical(&parsed)).unwrap();
        for (path, module, body) in [
            ("src/app.spx", "calculator.app", "record_value(41)"),
            (
                "src/tests.spx",
                "calculator.tests",
                "if record_value(41) == 42 { 0 } else { 1 }",
            ),
        ] {
            let source=format!("module {module};\nuse function @id(\"delta.record-value\") from calculator.core as record_value;\n@id(\"{module}.main\") fn main() -> i64 {{ {body} }}\n");
            let parsed = semaprax::parse(&source, Path::new(path)).unwrap();
            std::fs::write(fixture.0.join(path), semaprax::format::canonical(&parsed)).unwrap();
        }
        fixture
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn apply(candidate: &ProjectCandidate, intent: Value) -> ProjectCandidate {
    candidate
        .apply(
            candidate.candidate_digest(),
            &SemanticChange::new(candidate.revision().project_revision(), &intent).unwrap(),
        )
        .unwrap()
}
fn delta(candidate: &ProjectCandidate, target: &str) -> Value {
    serde_json::from_str(
        &candidate
            .semantic_delta(candidate.candidate_digest(), target)
            .unwrap(),
    )
    .unwrap()
}
fn facet<'a>(delta: &'a Value, name: &str) -> &'a Value {
    delta["facets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["facet"] == name)
        .unwrap()
}

#[test]
fn signature_delta_is_bound_recomputable_and_does_not_embed_unchanged_graphs() {
    let fixture = Fixture::new();
    let root = fixture.open();
    let before = std::fs::read(fixture.0.join("src/core.spx")).unwrap();
    let candidate = apply(
        &root,
        json!({"kind":"change_function_signature","target":"calculator.add","append_parameters":[{"name":"offset","type":"i64","argument":{"kind":"i64","value":0}}]}),
    );
    let report = candidate
        .semantic_delta(candidate.candidate_digest(), "calculator.add")
        .unwrap();
    let value: Value = serde_json::from_str(&report).unwrap();
    assert_eq!(value["candidate_digest"], candidate.candidate_digest());
    assert_eq!(value["presence"], "modified");
    assert_eq!(facet(&value, "signature")["change"], "modified");
    assert!(facet(&value, "signature")["candidate"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["name"] == "offset"));
    assert!(value.get("project_graph").is_none());
    let contracts = facet(&value, "contracts");
    assert_eq!(contracts["projection_equal_without_provenance"], true);
    assert!(contracts.get("base").is_none());
    assert_eq!(
        value["target_artifacts"]["facet"],
        "complete_entry_and_test_target_artifacts"
    );
    assert!(candidate
        .verify_semantic_delta(
            candidate.candidate_digest(),
            "calculator.add",
            report.as_bytes()
        )
        .is_ok());
    let mut tampered = report.into_bytes();
    tampered.push(b' ');
    let errors = candidate
        .verify_semantic_delta(candidate.candidate_digest(), "calculator.add", &tampered)
        .unwrap_err();
    assert!(errors.iter().any(|error| error.code == "SPX-G254"));
    assert!(candidate
        .semantic_delta(root.candidate_digest(), "calculator.add")
        .is_err());
    assert_eq!(
        std::fs::read(fixture.0.join("src/core.spx")).unwrap(),
        before
    );
}

#[test]
fn added_function_has_an_absent_base_and_contract_fact_payload() {
    let fixture = Fixture::new();
    let root = fixture.open();
    let candidate = apply(
        &root,
        json!({"kind":"add_declaration","target":"calculator.add","declaration":{
        "id":"delta.identity","name":"identity","parameters":[{"name":"value","type":"i64","mode":"value"}],
        "return_type":"i64","effects":[],"requires":[],"ensures":[{"kind":"bool","value":true}],
        "body":{"kind":"place","name":"value"}}}),
    );
    let report = delta(&candidate, "delta.identity");
    assert_eq!(report["presence"], "added");
    assert!(report["source_bindings"]["base"].is_null());
    assert!(facet(&report, "contracts")["base"].is_null());
    assert_eq!(
        facet(&report, "contracts")["candidate"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let catalog: Value = serde_json::from_str(
        &candidate
            .semantic_delta_catalog(candidate.candidate_digest())
            .unwrap(),
    )
    .unwrap();
    assert!(catalog["roots"]
        .as_array()
        .unwrap()
        .iter()
        .any(|root| root["target"] == "delta.identity" && root["change"] == "added"));
    assert!(!catalog["roots"]
        .as_array()
        .unwrap()
        .iter()
        .any(|root| root["target"] == "calculator.multiply"));
}

#[test]
fn field_and_record_deltas_include_real_reads_writes_patterns_and_test_callers() {
    let fixture = Fixture::record_fixture();
    let root = fixture.open();
    let candidate = apply(
        &root,
        json!({"kind":"add_record_field","target":"delta.pair","field":{"id":"delta.pair.flag","name":"flag","type":"bool","default":{"kind":"bool","value":false}}}),
    );
    let field = delta(&candidate, "delta.pair.flag");
    assert_eq!(field["presence"], "added");
    let relationships = &facet(&field, "reverse_field_and_call_relationships")["candidate"];
    assert_eq!(relationships["test_reachable"], true);
    assert!(relationships["direct_field_sites"]
        .as_array()
        .unwrap()
        .iter()
        .any(|site| site["access"] == "initialize" && site["function_id"] == "delta.record-value"));
    assert!(relationships["direct_field_sites"]
        .as_array()
        .unwrap()
        .iter()
        .any(|site| site["access"] == "pattern_ignore"));
    let record = delta(&candidate, "delta.pair");
    assert_eq!(facet(&record, "typed_declaration")["change"], "modified");
    assert_eq!(
        facet(&record, "typed_declaration")["candidate"]["fields"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let old_field = delta(&root, "delta.pair.value");
    // Unchanged payloads are intentionally omitted; query the new field above
    // proves the reverse index, while a changed declaration exposes old/new sites.
    assert_eq!(
        facet(&old_field, "reverse_field_and_call_relationships")["exact_equal"],
        true
    );
    let relations = &facet(&record, "reverse_field_and_call_relationships")["candidate"];
    assert!(relations["direct_field_sites"]
        .as_array()
        .unwrap()
        .iter()
        .any(|site| site["access"] == "in_place_write"
            && site["field_or_type_id"] == "delta.pair.value"));
    assert!(relations["direct_field_sites"]
        .as_array()
        .unwrap()
        .iter()
        .any(|site| site["access"] == "read_or_move" || site["access"] == "projection_read"));
}

#[test]
fn moved_identity_keeps_before_and_after_source_origins() {
    let fixture = Fixture::new();
    let exported = fixture.open();
    let intent = json!({"kind":"move_declaration","target":"calculator.add","destination":"calculator.app.main"});
    let change = SemanticChange::new(exported.revision().project_revision(), &intent).unwrap();
    let errors = exported
        .apply(exported.candidate_digest(), &change)
        .err()
        .unwrap();
    assert!(errors.iter().any(|error| error.code == "SPX-G225"));
    let manifest = fixture.0.join("semaprax.toml");
    let source = std::fs::read_to_string(&manifest)
        .unwrap()
        .replace("\"calculator.add\", ", "")
        .replace(
            "\"src/tests.spx\"]",
            "\"src/support.spx\", \"src/tests.spx\"]",
        );
    std::fs::write(manifest, source).unwrap();
    std::fs::write(
        fixture.0.join("src/support.spx"),
        semaprax::format::canonical(&semaprax::parse(
            "module calculator.support;\n@id(\"calculator.support.anchor\") fn anchor() -> i64 { 0 }\n",
            "src/support.spx",
        ).unwrap()),
    )
    .unwrap();
    let root = fixture.open();
    let candidate = apply(
        &root,
        json!({"kind":"move_declaration","target":"calculator.add","destination":"calculator.support.anchor"}),
    );
    let report = delta(&candidate, "calculator.add");
    assert_eq!(report["source_bindings"]["base"]["path"], "src/core.spx");
    assert_eq!(
        report["source_bindings"]["candidate"]["path"],
        "src/support.spx"
    );
    let catalog: Value = serde_json::from_str(
        &candidate
            .semantic_delta_catalog(candidate.candidate_digest())
            .unwrap(),
    )
    .unwrap();
    assert!(catalog["roots"]
        .as_array()
        .unwrap()
        .iter()
        .any(|root| root["target"] == "calculator.add" && root["change"] == "moved"));
}
