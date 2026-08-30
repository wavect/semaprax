//! Candidate-wide static-conformance evidence, authored and intentionally unrun.
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
            "spx-interface-delta-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "interface-delta"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "iface.app"
sources = ["src/app.spx", "src/core.spx", "src/other.spx", "src/tests.spx"]
web_exports = ["iface.evaluate"]
tests = ["iface.tests"]
"#,
        )
        .unwrap();
        for (path, source) in [
            (
                "src/core.spx",
                r#"module iface.core;
use function @id("other.identity") from iface.other as other_identity;
@id("iface.counter") record Counter { @id("iface.counter.value") value: i64, }
@id("iface.readable") protocol Readable {
    @id("iface.readable.read") fn read(receiver: Self) -> i64;
    @id("iface.readable.positive") fn positive(receiver: Self) -> bool;
}
@id("iface.read") fn counter_read(receiver: Counter) -> i64 { other_identity(receiver.value) }
@id("iface.positive") fn counter_positive(receiver: Counter) -> bool { receiver.value > 0 }
@id("iface.evaluate") fn evaluate(value: i64) -> i64 { counter_read(Counter { value: value }) }
"#,
            ),
            (
                "src/other.spx",
                r#"module iface.other;
@id("other.identity") fn identity(value: i64) -> i64 { value }
@id("other.counter") record Counter { @id("other.counter.value") value: i64, }
@id("other.readable") protocol Readable { @id("other.readable.read") fn read(receiver: Self) -> i64; }
@id("other.read") fn counter_read(receiver: Counter) -> i64 { receiver.value }
@id("other.impl") impl "other.readable" for "other.counter" { "other.readable.read" = "other.read"; }
"#,
            ),
            (
                "src/app.spx",
                r#"module iface.app;
use function @id("iface.evaluate") from iface.core as evaluate;
@id("iface.main") fn main() -> i64 { evaluate(42) }
"#,
            ),
            (
                "src/tests.spx",
                r#"module iface.tests;
use function @id("iface.evaluate") from iface.core as evaluate;
@id("iface.test") fn evaluates() -> bool { evaluate(42) == 42 }
"#,
            ),
        ] {
            let program = semaprax::parse(source, path).unwrap();
            std::fs::write(root.join(path), semaprax::format::canonical(&program)).unwrap();
        }
        Self(root)
    }
    fn candidate(&self) -> ProjectCandidate {
        let revision = with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap();
        ProjectCandidate::open(Arc::clone(&revision), revision.project_revision()).unwrap()
    }
    fn sources(&self) -> Vec<Vec<u8>> {
        [
            "src/app.spx",
            "src/core.spx",
            "src/other.spx",
            "src/tests.spx",
        ]
        .iter()
        .map(|path| std::fs::read(self.0.join(path)).unwrap())
        .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn implementation() -> Value {
    json!({"kind":"implement_interface","target":"iface.counter","protocol":"iface.readable","id":"iface.impl","members":[
        {"method":"iface.readable.read","implementation":"iface.read"},
        {"method":"iface.readable.positive","implementation":"iface.positive"}
    ]})
}
fn apply(candidate: &ProjectCandidate, intent: Value) -> Result<ProjectCandidate, Vec<Diagnostic>> {
    candidate.apply(
        candidate.candidate_digest(),
        &SemanticChange::new(candidate.revision().project_revision(), &intent)?,
    )
}
fn report(candidate: &ProjectCandidate) -> Value {
    serde_json::from_str(
        &candidate
            .interface_delta(candidate.candidate_digest())
            .unwrap(),
    )
    .unwrap()
}
fn code<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    match result {
        Ok(_) => panic!("expected {expected}"),
        Err(errors) => assert!(
            errors.iter().any(|error| error.code == expected),
            "{errors:?}"
        ),
    }
}
fn implementation_row<'a>(value: &'a Value, id: &str) -> &'a Value {
    value["implementations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == id)
        .unwrap()
}
fn member_row<'a>(implementation: &'a Value, method: &str) -> &'a Value {
    implementation["members"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["method_id"] == method)
        .unwrap()
}

#[test]
fn added_binding_carries_complete_members_and_exact_independent_source_origins() {
    let fixture = Fixture::new();
    let untouched = fixture.sources();
    let base = fixture.candidate();
    let candidate = apply(&base, implementation()).unwrap();
    let value = report(&candidate);
    assert_eq!(
        value["schema"],
        "semaprax.project-candidate-interface-delta.v1"
    );
    assert_eq!(value["candidate_digest"], candidate.candidate_digest());
    assert_eq!(
        value["base_project_revision"],
        base.revision().project_revision()
    );
    assert_eq!(
        value["project_revision"],
        candidate.revision().project_revision()
    );
    assert_eq!(value["inventory"]["base_implementations"], 1);
    assert_eq!(value["inventory"]["candidate_implementations"], 2);
    assert_eq!(value["inventory"]["unchanged_implementations"], 1);
    let added = implementation_row(&value, "iface.impl");
    assert_eq!(added["comparison"]["change"], "added");
    assert!(added["comparison"]["base"].is_null());
    assert_eq!(added["members"].as_array().unwrap().len(), 2);
    for (method, function) in [
        ("iface.readable.read", "iface.read"),
        ("iface.readable.positive", "iface.positive"),
    ] {
        let member = member_row(added, method);
        assert_eq!(member["comparison"]["candidate"]["method_id"], method);
        assert_eq!(member["comparison"]["candidate"]["function_id"], function);
        assert!(member["comparison"]["base"].is_null());
    }
    let dependencies = member_row(added, "iface.readable.read")["comparison"]["candidate"]
        ["dependencies"]
        .as_array()
        .unwrap();
    assert!(dependencies
        .iter()
        .any(|row| row["id"] == "other.identity" && row["fact_digest"].is_string()));
    // An unrelated module uses identical display names, but cannot be joined
    // by spelling or be reported as changed merely because another module is.
    assert!(!value["implementations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["id"] == "other.impl"));
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
    let bytes = candidate
        .interface_delta(candidate.candidate_digest())
        .unwrap();
    assert!(bytes.ends_with('\n'));
    candidate
        .verify_interface_delta(candidate.candidate_digest(), bytes.as_bytes())
        .unwrap();
    assert_eq!(report(&candidate), value);
    assert_eq!(fixture.sources(), untouched);
}

#[test]
fn bound_function_edits_change_conformance_facts_without_dropping_unchanged_siblings() {
    let fixture = Fixture::new();
    let untouched = fixture.sources();
    let added = apply(&fixture.candidate(), implementation()).unwrap();
    // Start from an already-conforming immutable revision, so the comparison
    // isolates function changes rather than classifying the binding as added.
    let base = ProjectCandidate::open(
        Arc::clone(added.revision()),
        added.revision().project_revision(),
    )
    .unwrap();
    let unchanged = report(&base);
    assert_eq!(unchanged["implementations"], json!([]));
    assert_eq!(unchanged["protocols"], json!([]));
    assert_eq!(unchanged["inventory"]["unchanged_implementations"], 2);
    let renamed = apply(
        &base,
        json!({"kind":"rename_declaration","target":"iface.read","name":"read_again"}),
    )
    .unwrap();
    let replaced = apply(&renamed, json!({"kind":"replace_function_body","target":"iface.read","body":{"kind":"i64","value":41}})).unwrap();
    let candidate = apply(&replaced, json!({"kind":"add_contract","target":"iface.read","phase":"ensures","predicate":{"kind":"bool","value":true}})).unwrap();
    for changed in [&renamed, &replaced, &candidate] {
        let value = report(changed);
        let implementation = implementation_row(&value, "iface.impl");
        assert_eq!(implementation["comparison"]["change"], "modified");
        assert_eq!(implementation["members"].as_array().unwrap().len(), 2);
        let read = member_row(implementation, "iface.readable.read");
        assert_eq!(read["comparison"]["change"], "modified");
        assert_eq!(
            read["comparison"]["base"]["function"]["name"],
            "counter_read"
        );
        assert_eq!(
            read["comparison"]["candidate"]["function"]["name"],
            "read_again"
        );
        for side in ["base", "candidate"] {
            assert_eq!(read["comparison"][side]["function_id"], "iface.read");
        }
        let sibling = member_row(implementation, "iface.readable.positive");
        assert_eq!(
            sibling["comparison"]["projection_equal_without_provenance"],
            true
        );
        // Even an unchanged member retains complete values: absence would
        // falsely look like loss of a required protocol member.
        for side in ["base", "candidate"] {
            assert_eq!(sibling["comparison"][side]["function_id"], "iface.positive");
            assert!(sibling["comparison"][side]["function"].is_object());
        }
        assert_eq!(value["inventory"]["unchanged_implementations"], 1);
        assert!(!value["implementations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["id"] == "other.impl"));
    }
    let renamed_fact = report(&renamed);
    let final_fact = report(&candidate);
    let final_read = member_row(
        implementation_row(&final_fact, "iface.impl"),
        "iface.readable.read",
    );
    assert_ne!(
        final_read["comparison"]["base"]["function"]["body_digest"],
        final_read["comparison"]["candidate"]["function"]["body_digest"]
    );
    assert_eq!(
        final_read["comparison"]["base"]["function"]["ensures"],
        json!([])
    );
    assert_eq!(
        final_read["comparison"]["candidate"]["function"]["ensures"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        final_read["comparison"]["base"]["function"]["checked_signature"],
        final_read["comparison"]["candidate"]["function"]["checked_signature"]
    );
    assert_ne!(
        member_row(
            implementation_row(&renamed_fact, "iface.impl"),
            "iface.readable.read"
        )["comparison"]["candidate_digest"],
        member_row(
            implementation_row(&final_fact, "iface.impl"),
            "iface.readable.read"
        )["comparison"]["candidate_digest"]
    );
    assert_eq!(fixture.sources(), untouched);
}

#[test]
fn incompatible_member_changes_fail_before_a_new_delta_can_be_admitted() {
    let fixture = Fixture::new();
    let untouched = fixture.sources();
    let candidate = apply(&fixture.candidate(), implementation()).unwrap();
    let before = candidate
        .interface_delta(candidate.candidate_digest())
        .unwrap();
    code(
        apply(
            &candidate,
            json!({"kind":"add_contract","target":"iface.read","phase":"requires","predicate":{"kind":"bool","value":true}}),
        ),
        "SPX-Q107",
    );
    code(
        apply(
            &candidate,
            json!({"kind":"change_function_signature","target":"iface.read","append_parameters":[{"name":"offset","type":"i64","argument":{"kind":"i64","value":0}}]}),
        ),
        "SPX-Q107",
    );
    assert_eq!(
        candidate
            .interface_delta(candidate.candidate_digest())
            .unwrap(),
        before
    );
    assert_eq!(fixture.sources(), untouched);
}
fn canonical(mut value: Value) -> String {
    value.sort_all_objects();
    format!("{value}\n")
}
fn fact_digest(value: &Value) -> String {
    use sha2::{Digest, Sha256};
    let bytes = canonical(value.clone()).into_bytes();
    let mut hash = Sha256::new();
    hash.update(b"semaprax.candidate-interface-delta.fact.v1\0");
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    format!(
        "sha256:{}",
        hash.finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

#[test]
fn exact_replay_rejects_reminted_member_facts_provenance_and_stale_candidates() {
    let fixture = Fixture::new();
    let untouched = fixture.sources();
    let base = fixture.candidate();
    let candidate = apply(&base, implementation()).unwrap();
    let bytes = candidate
        .interface_delta(candidate.candidate_digest())
        .unwrap();
    let receipt: Value = serde_json::from_str(
        &candidate
            .verify_interface_delta(candidate.candidate_digest(), bytes.as_bytes())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(receipt["result"], "exact_recomputation");
    assert_eq!(receipt["execution"], false);
    assert_eq!(receipt["source_authority"], false);
    let mut forged: Value = serde_json::from_str(&bytes).unwrap();
    let row = forged["implementations"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|row| row["id"] == "iface.impl")
        .unwrap();
    let member_index = row["members"]
        .as_array()
        .unwrap()
        .iter()
        .position(|member| member["method_id"] == "iface.readable.read")
        .unwrap();
    row["members"][member_index]["comparison"]["candidate"]["requirement"]["return_type"] =
        json!("bool");
    let fake_member = row["members"][member_index]["comparison"]["candidate"].clone();
    row["members"][member_index]["comparison"]["candidate_digest"] =
        json!(fact_digest(&fake_member));
    let outer = row["comparison"]["candidate"]["members"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|member| member["method_id"] == "iface.readable.read")
        .unwrap();
    *outer = fake_member;
    row["comparison"]["candidate_digest"] = json!(fact_digest(&row["comparison"]["candidate"]));
    // Even internally consistent public fact hashes are not compiler evidence.
    code(
        candidate
            .verify_interface_delta(candidate.candidate_digest(), canonical(forged).as_bytes()),
        "SPX-G312",
    );
    let mut wrong_source: Value = serde_json::from_str(&bytes).unwrap();
    wrong_source["source_bindings"]["candidate"][0]["source_digest"] =
        json!(format!("sha256:{}", "0".repeat(64)));
    code(
        candidate.verify_interface_delta(
            candidate.candidate_digest(),
            canonical(wrong_source).as_bytes(),
        ),
        "SPX-G312",
    );
    code(
        candidate
            .verify_interface_delta(candidate.candidate_digest(), format!("{bytes} ").as_bytes()),
        "SPX-G312",
    );
    code(
        base.verify_interface_delta(base.candidate_digest(), bytes.as_bytes()),
        "SPX-G312",
    );
    assert!(candidate.interface_delta(base.candidate_digest()).is_err());
    assert!(candidate
        .verify_interface_delta(base.candidate_digest(), bytes.as_bytes())
        .is_err());
    assert_eq!(
        candidate
            .interface_delta(candidate.candidate_digest())
            .unwrap(),
        bytes
    );
    assert_eq!(fixture.sources(), untouched);
}

#[test]
fn imported_helper_body_change_reaches_binding_with_unchanged_local_source() {
    let fixture = Fixture::new();
    let added = apply(&fixture.candidate(), implementation()).unwrap();
    let base = ProjectCandidate::open(
        Arc::clone(added.revision()),
        added.revision().project_revision(),
    )
    .unwrap();
    let candidate = apply(&base, json!({"kind":"replace_function_body","target":"other.identity","body":{"kind":"i64","value":7}})).unwrap();
    let value = report(&candidate);
    let implementation = implementation_row(&value, "iface.impl");
    assert_eq!(implementation["comparison"]["change"], "modified");
    let read = member_row(implementation, "iface.readable.read");
    assert_eq!(read["comparison"]["change"], "modified");
    assert_eq!(
        read["comparison"]["base"]["function"],
        read["comparison"]["candidate"]["function"]
    );
    let dependency = |side: &str| {
        read["comparison"][side]["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == "other.identity")
            .unwrap()["fact_digest"]
            .clone()
    };
    assert_ne!(dependency("base"), dependency("candidate"));
    let source = |side: &str| {
        value["source_bindings"][side]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["path"] == "src/core.spx")
            .unwrap()
            .clone()
    };
    assert_eq!(source("base"), source("candidate"));
    assert_eq!(
        member_row(implementation, "iface.readable.positive")["comparison"]["change"],
        "unchanged"
    );
    let bytes = candidate
        .interface_delta(candidate.candidate_digest())
        .unwrap();
    candidate
        .verify_interface_delta(candidate.candidate_digest(), bytes.as_bytes())
        .unwrap();
}
