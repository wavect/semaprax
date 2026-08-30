//! Whole-candidate contract delta evidence, authored and intentionally unrun.
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{with_authenticated_project, ProjectCandidate, SemanticChange};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-contract-delta-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let root = root.canonicalize().unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "contract-delta"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "contracts.app"
sources = ["src/app.spx", "src/core.spx", "src/helper.spx", "src/tests.spx"]
web_exports = ["contracts.public"]
tests = ["contracts.tests"]
"#,
        )
        .unwrap();
        for (path, source) in [
            (
                "src/core.spx",
                r#"module contracts.core;
use function @id("contracts.predicate") from contracts.helper as imported_predicate;
@id("contracts.checked") fn checked(value:i64)->i64 requires imported_predicate(value) ensures result >= 0 {value}
@id("contracts.sibling") fn sibling(value:i64)->i64 requires value >= 0 {value}
@id("contracts.public") fn public_value(value:i64)->i64 {value}
"#,
            ),
            (
                "src/helper.spx",
                r#"module contracts.helper;
@id("contracts.leaf") fn leaf(value:i64)->bool {value >= 0}
@id("contracts.predicate") fn predicate(value:i64)->bool {leaf(value)}
"#,
            ),
            (
                "src/app.spx",
                r#"module contracts.app;
use function @id("contracts.checked") from contracts.core as checked;
@id("contracts.main") fn main()->i64 {checked(42)}
"#,
            ),
            (
                "src/tests.spx",
                r#"module contracts.tests;
use function @id("contracts.checked") from contracts.core as imported_checked;
@id("contracts.other-checked") fn checked(value:i64)->i64 requires value >= 0 {value}
@id("contracts.test") fn main()->i64 {if imported_checked(42) == checked(42) {0}else{1}}
"#,
            ),
        ] {
            let program = semaprax::parse(source, path).unwrap();
            std::fs::write(root.join(path), semaprax::format::canonical(&program)).unwrap();
        }
        Self(root)
    }
    fn candidate(&self) -> ProjectCandidate {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
        })
        .unwrap()
    }
    fn bytes(&self) -> Vec<Vec<u8>> {
        [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/helper.spx",
            "src/tests.spx",
        ]
        .iter()
        .map(|p| std::fs::read(self.0.join(p)).unwrap())
        .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn apply(base: &ProjectCandidate, intent: Value) -> Result<ProjectCandidate, Vec<Diagnostic>> {
    base.apply(
        base.candidate_digest(),
        &SemanticChange::new(base.revision().project_revision(), &intent)?,
    )
}
fn add(phase: &str, predicate: Value) -> Value {
    json!({"kind":"add_contract","target":"contracts.checked","phase":phase,"predicate":predicate})
}
fn place(name: &str) -> Value {
    json!({"kind":"place","name":name})
}
fn report(candidate: &ProjectCandidate) -> Value {
    serde_json::from_str(
        &candidate
            .contract_delta(candidate.candidate_digest())
            .unwrap(),
    )
    .unwrap()
}
fn replay(candidate: &ProjectCandidate) {
    let capsule = candidate.recovery_capsule().unwrap();
    let restored = ProjectCandidate::restore(
        Arc::clone(candidate.base_revision()),
        candidate.base_revision().project_revision(),
        capsule.as_bytes(),
    )
    .unwrap();
    assert_eq!(restored.to_json(), candidate.to_json());
    assert_eq!(
        restored
            .contract_delta(restored.candidate_digest())
            .unwrap(),
        candidate
            .contract_delta(candidate.candidate_digest())
            .unwrap()
    );
}
fn code<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    let errors = result.err().expect("invalid contract delta input accepted");
    assert!(errors.iter().any(|e| e.code == expected), "{errors:?}");
}
fn canonical(mut value: Value) -> String {
    value.sort_all_objects();
    format!("{value}\n")
}
fn row<'a>(report: &'a Value, id: &str) -> &'a Value {
    report["functions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == id)
        .expect("affected function missing")
}
fn predicate<'a>(side: &'a Value, phase: &str, index: u64) -> &'a Value {
    side["predicates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["phase"] == phase && p["index"] == index)
        .expect("contract slot missing")
}
fn fact_digest(value: &Value) -> String {
    use sha2::{Digest, Sha256};
    let bytes = canonical(value.clone()).into_bytes();
    let mut hash = Sha256::new();
    hash.update(b"semaprax.candidate-contract-delta.fact.v1\0");
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    format!(
        "sha256:{}",
        hash.finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}

#[test]
fn appended_pre_and_postconditions_retain_complete_ordered_original_predicates() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let first=apply(&base,add("requires",json!({"kind":"binary","op":"<=","left":place("value"),"right":{"kind":"i64","value":100}}))).unwrap();
    let candidate = apply(
        &first,
        add(
            "ensures",
            json!({"kind":"binary","op":"==","left":place("result"),"right":place("value")}),
        ),
    )
    .unwrap();
    let value = report(&candidate);
    assert_eq!(
        value["schema"],
        "semaprax.project-candidate-contract-delta.v1"
    );
    assert_eq!(value["candidate_digest"], candidate.candidate_digest());
    let changed = row(&value, "contracts.checked");
    assert_eq!(changed["comparison"]["predicate_projection_equal"], false);
    let before = &changed["base"];
    let after = &changed["candidate"];
    assert_eq!(before["predicates"].as_array().unwrap().len(), 2);
    assert_eq!(after["predicates"].as_array().unwrap().len(), 4);
    let slots = after["predicates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| (p["phase"].as_str().unwrap(), p["index"].as_u64().unwrap()))
        .collect::<Vec<_>>();
    assert_eq!(
        slots,
        vec![
            ("requires", 0),
            ("requires", 1),
            ("ensures", 0),
            ("ensures", 1)
        ]
    );
    for phase in ["requires", "ensures"] {
        assert_eq!(
            predicate(before, phase, 0)["projection_digest"],
            predicate(after, phase, 0)["projection_digest"]
        );
        assert_eq!(
            predicate(before, phase, 0)["source_fragment"],
            predicate(after, phase, 0)["source_fragment"]
        );
    }
    assert_eq!(
        predicate(after, "requires", 1)["source_fragment"],
        "value <= 100"
    );
    assert_eq!(
        predicate(after, "ensures", 1)["source_fragment"],
        "result == value"
    );
    assert!(!value["functions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["id"] == "contracts.other-checked"));
    let bytes = candidate
        .contract_delta(candidate.candidate_digest())
        .unwrap();
    candidate
        .verify_contract_delta(candidate.candidate_digest(), bytes.as_bytes())
        .unwrap();
    replay(&candidate);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn transitive_cross_source_predicate_dependency_edits_are_visible_without_fabricating_predicate_edits(
) {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let candidate=apply(&base,json!({"kind":"replace_function_body","target":"contracts.leaf","body":{"kind":"binary","op":">","left":place("value"),"right":{"kind":"i64","value":0}}})).unwrap();
    let value = report(&candidate);
    let changed = row(&value, "contracts.checked");
    assert_eq!(changed["comparison"]["predicate_projection_equal"], true);
    assert_eq!(changed["comparison"]["dependency_equal"], false);
    assert!(changed["comparison"]["reasons"]
        .as_array()
        .unwrap()
        .contains(&json!("dependency_changed")));
    let before = predicate(&changed["base"], "requires", 0);
    let after = predicate(&changed["candidate"], "requires", 0);
    assert_eq!(before["expression"], after["expression"]);
    assert_eq!(before["source_fragment"], after["source_fragment"]);
    for (id, reason) in [
        ("contracts.predicate", "contract_direct_callee"),
        ("contracts.leaf", "transitive_contract_callee"),
    ] {
        let dependencies = after["dependencies"].as_array().unwrap();
        let dependency = dependencies.iter().find(|d| d["id"] == id).unwrap();
        assert_eq!(dependency["reason"], reason);
        assert_eq!(dependency["fact_availability"], "retained_source_callable");
        assert_eq!(dependency["provenance"]["path"], "src/helper.spx");
        assert_eq!(
            dependency["evidence_owner"],
            "validated_workspace_HIR_and_canonical_source"
        );
        assert!(dependency["fact_digest"].is_string());
    }
    let dependency = |predicate: &Value| {
        predicate["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["id"] == "contracts.leaf")
            .unwrap()["fact_digest"]
            .clone()
    };
    assert_ne!(dependency(before), dependency(after));
    // Same display name in tests is a different stable function and source.
    assert!(!value["functions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["id"] == "contracts.other-checked"));
    replay(&candidate);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn same_source_display_shift_is_separate_from_predicate_and_dependency_changes() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let candidate = apply(
        &base,
        json!({"kind":"rename_declaration","target":"contracts.sibling","name":"renamed_sibling"}),
    )
    .unwrap();
    let value = report(&candidate);
    let checked = row(&value, "contracts.checked");
    assert_eq!(checked["comparison"]["predicate_projection_equal"], true);
    assert_eq!(checked["comparison"]["dependency_equal"], true);
    assert_eq!(checked["comparison"]["exact_equal"], false);
    assert_eq!(checked["change"], "provenance_only");
    assert_ne!(
        checked["base"]["provenance"]["source_digest"],
        checked["candidate"]["provenance"]["source_digest"]
    );
    assert_eq!(checked["base"]["provenance"]["path"], "src/core.spx");
    assert_eq!(checked["candidate"]["provenance"]["path"], "src/core.spx");
    assert!(!value["functions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["id"] == "contracts.other-checked"));
    replay(&candidate);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn added_contract_owner_has_absent_base_while_empty_functions_do_not_invent_predicates() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let mut candidate=apply(&base,json!({"kind":"add_declaration","target":"contracts.checked","declaration":{"id":"contracts.added","name":"added","parameters":[{"name":"value","type":"i64","mode":"value"}],"return_type":"i64","effects":[],"requires":[],"ensures":[{"kind":"bool","value":true}],"body":place("value")}})).unwrap();
    candidate=apply(&candidate,json!({"kind":"add_declaration","target":"contracts.checked","declaration":{"id":"contracts.empty","name":"empty","parameters":[],"return_type":"i64","effects":[],"requires":[],"ensures":[],"body":{"kind":"i64","value":0}}})).unwrap();
    let value = report(&candidate);
    assert_eq!(
        value["inventory"]["candidate_functions"].as_u64().unwrap(),
        value["inventory"]["base_functions"].as_u64().unwrap() + 2
    );
    assert_eq!(
        value["inventory"]["candidate_functions_with_contracts"]
            .as_u64()
            .unwrap(),
        value["inventory"]["base_functions_with_contracts"]
            .as_u64()
            .unwrap()
            + 1
    );
    assert_eq!(
        value["inventory"]["candidate_predicates"].as_u64().unwrap(),
        value["inventory"]["base_predicates"].as_u64().unwrap() + 1
    );
    let added = row(&value, "contracts.added");
    assert_eq!(added["change"], "added");
    assert!(added["base"].is_null());
    assert_eq!(
        added["candidate"]["predicates"].as_array().unwrap().len(),
        1
    );
    assert_eq!(
        predicate(&added["candidate"], "ensures", 0)["expression"],
        json!({"kind":"bool","value":true})
    );
    assert!(!value["functions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["id"] == "contracts.empty"));
    for (side, revision) in [
        ("base", base.revision()),
        ("candidate", candidate.revision()),
    ] {
        let bindings = value["source_bindings"][side].as_array().unwrap();
        assert_eq!(bindings.len(), revision.sources().len());
        for (binding, source) in bindings.iter().zip(revision.sources()) {
            assert_eq!(binding["path"], source.path());
            assert_eq!(binding["source_revision"], source.source_revision());
            assert_eq!(binding["source_digest"], source.source_digest());
        }
    }
    replay(&candidate);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn exact_replay_rejects_reminted_facts_noncanonical_bytes_stale_handles_and_failed_changes() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let candidate = apply(&base, add("ensures", json!({"kind":"bool","value":true}))).unwrap();
    let bytes = candidate
        .contract_delta(candidate.candidate_digest())
        .unwrap();
    assert!(bytes.ends_with('\n'));
    let mut reminted: Value = serde_json::from_str(&bytes).unwrap();
    let changed = reminted["functions"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|f| f["id"] == "contracts.checked")
        .unwrap();
    changed["candidate"]["predicates"][0]["source_fragment"] = json!("false");
    changed["comparison"]["candidate_digest"] = json!(fact_digest(&changed["candidate"]));
    code(
        candidate
            .verify_contract_delta(candidate.candidate_digest(), canonical(reminted).as_bytes()),
        "SPX-G327",
    );
    let mut noncanonical = bytes.clone();
    noncanonical.push(' ');
    code(
        candidate.verify_contract_delta(candidate.candidate_digest(), noncanonical.as_bytes()),
        "SPX-G327",
    );
    let mut bad_provenance: Value = serde_json::from_str(&bytes).unwrap();
    bad_provenance["source_bindings"]["candidate"][0]["path"] = json!("src/forged.spx");
    code(
        candidate.verify_contract_delta(
            candidate.candidate_digest(),
            canonical(bad_provenance).as_bytes(),
        ),
        "SPX-G327",
    );
    assert!(candidate.contract_delta(base.candidate_digest()).is_err());
    assert!(base
        .verify_contract_delta(base.candidate_digest(), bytes.as_bytes())
        .is_err());
    code(
        apply(&candidate, add("requires", place("result"))),
        "SPX-G225",
    );
    assert!(apply(&candidate, add("ensures", json!({"kind":"i64","value":1}))).is_err());
    assert_eq!(
        candidate
            .contract_delta(candidate.candidate_digest())
            .unwrap(),
        bytes
    );
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn first_contract_on_existing_function_preserves_present_empty_base_inventory() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let candidate = apply(
        &base,
        json!({"kind":"add_contract","target":"contracts.leaf","phase":"ensures","predicate":{"kind":"bool","value":true}}),
    )
    .unwrap();
    let value = report(&candidate);
    let changed = row(&value, "contracts.leaf");
    assert_eq!(changed["change"], "modified");
    assert_eq!(changed["base"]["id"], "contracts.leaf");
    assert_eq!(changed["base"]["predicates"], json!([]));
    assert_eq!(changed["base"]["provenance"]["path"], "src/helper.spx");
    assert_eq!(changed["candidate"]["id"], "contracts.leaf");
    assert_eq!(
        changed["candidate"]["predicates"].as_array().unwrap().len(),
        1
    );
    assert_eq!(
        predicate(&changed["candidate"], "ensures", 0)["source_fragment"],
        "true"
    );
    assert_eq!(
        value["inventory"]["base_functions"],
        value["inventory"]["candidate_functions"]
    );
    assert_eq!(
        value["inventory"]["candidate_functions_with_contracts"]
            .as_u64()
            .unwrap(),
        value["inventory"]["base_functions_with_contracts"]
            .as_u64()
            .unwrap()
            + 1
    );
    assert!(!changed["comparison"]["reasons"]
        .as_array()
        .unwrap()
        .contains(&json!("added")));
    replay(&candidate);
    assert_eq!(fixture.bytes(), disk);
}
