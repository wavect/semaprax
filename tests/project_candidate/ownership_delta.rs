//! Whole-candidate ownership delta evidence, authored and intentionally unrun.
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
            "spx-ownership-delta-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let root = root.canonicalize().unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "ownership-delta"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "ownership.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["ownership.public"]
tests = ["ownership.tests"]
"#,
        )
        .unwrap();
        // Own-root Shared Loan Plan v1 without the independently closed
        // same-module owned-variant/Graph-v22 combination.
        for (path, source) in [
            (
                "src/core.spx",
                r#"module ownership.core;
@id("ownership.consume") fn consume(bytes:own Bytes)->i64 {7}
@id("ownership.forward") fn forward(bytes:own Bytes)->Bytes {bytes}
@id("ownership.select") fn select(left:own Bytes,right:own Bytes,flag:i64)->Bytes {if flag==0 {left}else{right}}
@id("ownership.call") fn call_select(input:borrow Slice<u8>)->Bytes {select(bytes_copy(input),bytes_copy(input),4/2)}
@id("ownership.loans") fn loans(input:borrow Slice<u8>)->i64 {
    let owned=bytes_copy(input);
    let parent=bytes_as_slice(owned);
    let child=byte_range(parent,1usize,byte_len(parent));
    let sibling=bytes_as_slice(owned);
    let observed=if byte_len(child)+byte_len(sibling)>0usize {1}else{0};
    consume(owned)+observed
}
@id("ownership.evaluate") fn evaluate()->i64 {let input=[7u8,8u8,9u8]; loans(array_as_slice(input))+consume(forward(call_select(array_as_slice(input))))}
@id("ownership.public") fn public_value(value:i64)->i64 {value}
@id("ownership.packet") record Packet {
    @id("ownership.packet.count") count: i64,
}
@id("ownership.packet.make") fn make_packet(value:i64)->Packet {Packet {count:value}}
@id("ownership.box") record Box<T> {
    @id("ownership.box.value") value: T,
}
@id("ownership.box.make") fn make_box(value:i64)->Box<i64> {Box<i64> {value:value}}
"#,
            ),
            (
                "src/app.spx",
                r#"module ownership.app;
use function @id("ownership.evaluate") from ownership.core as evaluate;
@id("ownership.main") fn main()->i64 {evaluate()}
"#,
            ),
            (
                "src/tests.spx",
                r#"module ownership.tests;
use function @id("ownership.evaluate") from ownership.core as evaluate;
@id("ownership.test") fn main()->i64 {if evaluate()==15 {0}else{1}}
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
fn place(name: &str) -> Value {
    json!({"kind":"place","name":name})
}
fn body(target: &str, body: Value) -> Value {
    json!({"kind":"replace_function_body","target":target,"body":body})
}
fn report(candidate: &ProjectCandidate) -> Value {
    serde_json::from_str(
        &candidate
            .ownership_delta(candidate.candidate_digest())
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
            .ownership_delta(restored.candidate_digest())
            .unwrap(),
        candidate
            .ownership_delta(candidate.candidate_digest())
            .unwrap()
    );
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
        .find(|f| f["id"] == id)
        .expect("ownership function missing")
}
fn type_row<'a>(report: &'a Value, id: &str) -> &'a Value {
    report["types"]
        .as_array()
        .unwrap()
        .iter()
        .find(|declaration| declaration["id"] == id)
        .expect("ownership type missing")
}
fn checked<'a>(candidate: &'a ProjectCandidate, id: &str) -> &'a semaprax::hir::ResolvedFunction {
    candidate
        .revision()
        .entry_program()
        .functions
        .iter()
        .find(|f| f.id.as_str() == id)
        .expect("reachable checked function missing")
}
fn fact_digest(value: &Value) -> String {
    use sha2::{Digest, Sha256};
    let bytes = canonical(value.clone()).into_bytes();
    let mut hash = Sha256::new();
    hash.update(b"semaprax.candidate-ownership-delta.fact.v1\0");
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
fn code<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    let errors = result.err().expect("hostile ownership report accepted");
    assert!(errors.iter().any(|e| e.code == expected), "{errors:?}");
}

#[test]
fn owning_nominal_transition_reports_exact_members_type_facts_and_cleanup_consequences() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let candidate = apply(
        &base,
        json!({
            "kind":"add_record_field",
            "target":"ownership.packet",
            "field":{
                "id":"ownership.packet.payload",
                "name":"payload",
                "type":"Bytes",
                "default":{"kind":"Bytes","values":[1,2,3]},
            },
        }),
    )
    .unwrap();
    let value = report(&candidate);
    let declaration = type_row(&value, "ownership.packet");
    assert_eq!(declaration["change"], "modified");
    assert_eq!(declaration["comparison"]["members_equal"], false);
    assert_eq!(declaration["comparison"]["type_facts_equal"], false);
    assert_eq!(
        declaration["comparison"]["type_facts_availability_equal"],
        true
    );
    assert_eq!(declaration["base"]["declaration_kind"], "record");
    assert_eq!(
        declaration["base"]["type_facts_availability"],
        "retained_checked"
    );
    assert_eq!(declaration["base"]["type_facts"]["copy"], true);
    assert_eq!(declaration["base"]["type_facts"]["needs_drop"], false);
    assert_eq!(declaration["candidate"]["type_facts"]["copy"], false);
    assert_eq!(declaration["candidate"]["type_facts"]["needs_drop"], true);
    assert_ne!(
        declaration["base"]["type_facts"]["layout_key"],
        declaration["candidate"]["type_facts"]["layout_key"]
    );
    let fields = declaration["candidate"]["members"]["fields"]
        .as_array()
        .unwrap();
    assert_eq!(fields[0]["id"], "ownership.packet.count");
    assert_eq!(fields[0]["index"], 0);
    assert_eq!(fields[1]["id"], "ownership.packet.payload");
    assert_eq!(fields[1]["index"], 1);
    assert_eq!(fields[1]["type_id"], "Bytes");
    assert_eq!(
        declaration["comparison"]["base_digest"],
        fact_digest(&declaration["base"])
    );
    assert_eq!(
        declaration["comparison"]["candidate_digest"],
        fact_digest(&declaration["candidate"])
    );

    let constructor = row(&value, "ownership.packet.make");
    assert_eq!(constructor["comparison"]["cleanup_inventory_equal"], false);
    assert!(
        constructor["candidate"]["cleanup_inventory"]["slots"]
            .as_array()
            .unwrap()
            .len()
            > constructor["base"]["cleanup_inventory"]["slots"]
                .as_array()
                .unwrap()
                .len()
    );
    replay(&candidate);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn generic_nominal_transition_keeps_uninstantiated_type_facts_explicitly_unavailable() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let candidate = apply(
        &base,
        json!({"kind":"rename_declaration","target":"ownership.box","name":"Crate"}),
    )
    .unwrap();
    let value = report(&candidate);
    let declaration = type_row(&value, "ownership.box");
    assert_eq!(declaration["change"], "modified");
    for side in ["base", "candidate"] {
        assert!(declaration[side]["type_facts"].is_null());
        assert_eq!(
            declaration[side]["type_facts_availability"],
            "generic_uninstantiated"
        );
        assert_eq!(declaration[side]["type_parameters"][0]["name"], "T");
    }
    assert!(declaration["comparison"]["type_facts_equal"].is_null());
    assert_eq!(
        declaration["comparison"]["type_facts_availability_equal"],
        true
    );
    assert_eq!(declaration["base"]["name"], "Box");
    assert_eq!(declaration["candidate"]["name"], "Crate");
    replay(&candidate);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn reordered_owned_parameters_and_caller_staging_retain_owned_modes_and_real_cleanup_plans() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let candidate=apply(&base,json!({"kind":"change_function_signature","target":"ownership.select","parameters":[{"from":"right"},{"from":"left"},{"from":"flag"}]})).unwrap();
    let value = report(&candidate);
    assert_eq!(
        value["schema"],
        "semaprax.project-candidate-ownership-delta.v1"
    );
    let selected = row(&value, "ownership.select");
    assert_eq!(selected["comparison"]["signature_equal"], false);
    for (side, names) in [
        ("base", vec!["left", "right", "flag"]),
        ("candidate", vec!["right", "left", "flag"]),
    ] {
        let parameters = selected[side]["signature"]["parameters"]
            .as_array()
            .unwrap();
        assert_eq!(
            parameters
                .iter()
                .map(|p| p["name"].as_str().unwrap())
                .collect::<Vec<_>>(),
            names
        );
        assert_eq!(
            parameters
                .iter()
                .map(|p| p["ownership"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["own", "own", "value"]
        );
        assert!(!selected[side]["cleanup_inventory"]["slots"]
            .as_array()
            .unwrap()
            .is_empty());
        assert_eq!(selected[side]["provenance"]["path"], "src/core.spx");
    }
    let caller = row(&value, "ownership.call");
    assert_eq!(caller["comparison"]["cleanup_plan_equal"], false);
    for (side, revision) in [("base", &base), ("candidate", &candidate)] {
        let actual = checked(revision, "ownership.call");
        let plan = &caller[side]["cleanup_plan"];
        assert_eq!(plan["schema"], actual.cleanup_plan.schema);
        assert_eq!(
            plan["slots"].as_array().unwrap().len(),
            actual.cleanup_plan.slots.len()
        );
        assert_eq!(
            plan["blocks"].as_array().unwrap().len(),
            actual.cleanup_plan.blocks.len()
        );
        assert_eq!(
            plan["edges"].as_array().unwrap().len(),
            actual.cleanup_plan.edges.len()
        );
        for (projected, slot) in plan["slots"]
            .as_array()
            .unwrap()
            .iter()
            .zip(&actual.cleanup_plan.slots)
        {
            assert_eq!(projected["id"], slot.id.0);
            assert_eq!(projected["storage_index"], slot.storage_index);
        }
    }
    replay(&candidate);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn owned_local_binding_changes_structural_inventory_without_claiming_runtime_destruction_counts() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let candidate = apply(
        &base,
        body(
            "ownership.forward",
            json!({"kind":"let","name":"held","value":place("bytes"),"body":place("held")}),
        ),
    )
    .unwrap();
    let value = report(&candidate);
    let changed = row(&value, "ownership.forward");
    assert_eq!(changed["comparison"]["cleanup_inventory_equal"], false);
    assert_eq!(changed["comparison"]["cleanup_plan_equal"], false);
    let before = &changed["base"]["cleanup_inventory"];
    let after = &changed["candidate"]["cleanup_inventory"];
    assert!(after["slots"].as_array().unwrap().len() > before["slots"].as_array().unwrap().len());
    for (side, owner) in [("base", &base), ("candidate", &candidate)] {
        let actual = checked(owner, "ownership.forward");
        let inventory = &changed[side]["cleanup_inventory"];
        assert_eq!(inventory["schema"], actual.cleanup.schema);
        assert_eq!(
            inventory["slots"].as_array().unwrap().len(),
            actual.cleanup.slots.len()
        );
        assert_eq!(
            inventory["flags"].as_array().unwrap().len(),
            actual.cleanup.flags.len()
        );
        assert_eq!(
            changed[side]["signature"]["parameters"][0]["ownership"],
            "own"
        );
    }
    replay(&candidate);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn nonempty_parent_and_sibling_loan_vectors_match_retained_order_and_disappear_after_body_replacement(
) {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let original = checked(&base, "ownership.loans");
    assert!(original.loan_plan.loans.len() >= 3);
    assert!(original
        .loan_plan
        .loans
        .iter()
        .any(|loan| loan.parent.is_some()));
    let candidate = apply(
        &base,
        body("ownership.loans", json!({"kind":"i64","value":0})),
    )
    .unwrap();
    let value = report(&candidate);
    let changed = row(&value, "ownership.loans");
    assert_eq!(changed["comparison"]["loan_plan_equal"], false);
    assert_eq!(
        changed["base"]["signature"]["parameters"][0]["ownership"],
        "borrow"
    );
    let plan = &changed["base"]["loan_plan"];
    assert_eq!(plan["schema"], semaprax::loan_plan::LOAN_PLAN_SCHEMA_V1);
    assert_eq!(
        plan["loans"].as_array().unwrap().len(),
        original.loan_plan.loans.len()
    );
    for (projected, loan) in plan["loans"]
        .as_array()
        .unwrap()
        .iter()
        .zip(&original.loan_plan.loans)
    {
        assert_eq!(projected["id"], loan.id.0);
        assert_eq!(projected["site"], loan.site.as_str());
        assert_eq!(projected["origin"]["root"], loan.origin.root.as_str());
        assert_eq!(
            projected["parent"],
            loan.parent.map(|id| json!(id.0)).unwrap_or(Value::Null)
        );
        assert_eq!(projected["end_edges"], json!(loan.end_edges));
    }
    assert_eq!(
        plan["endpoints"].as_array().unwrap().len(),
        original.loan_plan.endpoints.len()
    );
    assert_eq!(
        plan["edges"].as_array().unwrap().len(),
        original.loan_plan.edges.len()
    );
    for (projected, edge) in plan["edges"]
        .as_array()
        .unwrap()
        .iter()
        .zip(&original.loan_plan.edges)
    {
        assert_eq!(projected["from"], edge.from);
        assert_eq!(projected["to"], edge.to);
        assert_eq!(
            projected["live"],
            json!(edge.live.iter().map(|id| id.0).collect::<Vec<_>>())
        );
    }
    assert_eq!(changed["candidate"]["loan_plan"]["loans"], json!([]));
    assert_eq!(
        changed["candidate"]["cleanup_inventory"]["slots"],
        json!([])
    );
    replay(&candidate);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn added_owned_function_has_absent_base_and_reports_complete_source_provenance() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    assert_eq!(report(&base)["functions"], json!([]));
    let candidate=apply(&base,json!({"kind":"add_declaration","target":"ownership.forward","declaration":{"id":"ownership.added","name":"added","parameters":[{"name":"bytes","type":"Bytes","mode":"own"}],"return_type":"Bytes","effects":[],"requires":[],"ensures":[],"body":place("bytes")}})).unwrap();
    let value = report(&candidate);
    let added = row(&value, "ownership.added");
    assert_eq!(added["change"], "added");
    assert!(added["base"].is_null());
    assert_eq!(
        added["candidate"]["hir_availability"],
        "retained_checked_function"
    );
    assert_eq!(
        added["candidate"]["signature"]["parameters"][0]["ownership"],
        "own"
    );
    assert!(!added["candidate"]["cleanup_inventory"]["slots"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(
        value["inventory"]["candidate_functions"].as_u64().unwrap(),
        value["inventory"]["base_functions"].as_u64().unwrap() + 1
    );
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
fn reminted_reordered_loans_stale_reports_and_illegal_owned_reuse_cannot_change_candidate_or_source(
) {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let candidate = apply(
        &base,
        body("ownership.loans", json!({"kind":"i64","value":0})),
    )
    .unwrap();
    let bytes = candidate
        .ownership_delta(candidate.candidate_digest())
        .unwrap();
    candidate
        .verify_ownership_delta(candidate.candidate_digest(), bytes.as_bytes())
        .unwrap();
    let mut tampered: Value = serde_json::from_str(&bytes).unwrap();
    let changed = tampered["functions"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|f| f["id"] == "ownership.loans")
        .unwrap();
    changed["base"]["loan_plan"]["loans"]
        .as_array_mut()
        .unwrap()
        .swap(0, 1);
    changed["comparison"]["base_digest"] = json!(fact_digest(&changed["base"]));
    code(
        candidate
            .verify_ownership_delta(candidate.candidate_digest(), canonical(tampered).as_bytes()),
        "SPX-G330",
    );
    let noncanonical = format!("{bytes} ");
    code(
        candidate.verify_ownership_delta(candidate.candidate_digest(), noncanonical.as_bytes()),
        "SPX-G330",
    );
    assert!(candidate.ownership_delta(base.candidate_digest()).is_err());
    let original = base.ownership_delta(base.candidate_digest()).unwrap();
    let duplicate = json!({"kind":"let","name":"held","value":place("bytes"),"body":{"kind":"call","target":"ownership.select","arguments":[place("held"),place("held"),{"kind":"i64","value":0}]}});
    assert!(apply(&base, body("ownership.forward", duplicate)).is_err());
    assert!(apply(&base,json!({"kind":"change_function_signature","target":"ownership.select","parameters":[{"from":"left"},{"from":"flag"}]})).is_err());
    assert_eq!(
        base.ownership_delta(base.candidate_digest()).unwrap(),
        original
    );
    assert_eq!(
        candidate
            .ownership_delta(candidate.candidate_digest())
            .unwrap(),
        bytes
    );
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn generic_template_plans_remain_unavailable_while_actual_instance_plans_are_retained() {
    let fixture = Fixture::new();
    let path = fixture.0.join("src/core.spx");
    let core = std::fs::read_to_string(&path).unwrap()
        + r#"
@id("ownership.generic") fn generic<T>(value:T)->T {value}
@id("ownership.generic-call") fn generic_call(value:i64)->i64 {generic<i64>(value)}
"#;
    let parsed = semaprax::parse(&core, "src/core.spx").unwrap();
    std::fs::write(path, semaprax::format::canonical(&parsed)).unwrap();
    // Phase A retains the checked instance from this unselected source
    // function. Project-v8's executable owned-data closure does not admit
    // generic calls, so keep the entry/export roots unchanged; this test
    // checks report retention, not an expansion of target admission.
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let candidate = apply(
        &base,
        body("ownership.generic-call", json!({"kind":"i64","value":0})),
    )
    .unwrap();
    let value = report(&candidate);
    let template = row(&value, "ownership.generic");
    assert_eq!(template["comparison"]["instances_equal"], false);
    for side in ["base", "candidate"] {
        assert_eq!(
            template[side]["hir_availability"],
            "retained_checked_template"
        );
        assert!(template[side]["signature"].is_object());
        for plan in ["cleanup_inventory", "loan_plan", "cleanup_plan"] {
            assert!(template[side][plan].is_null());
        }
    }
    let instances = template["base"]["instances"].as_array().unwrap();
    assert_eq!(instances.len(), 1);
    let instance = &instances[0];
    assert!(instance["id"].is_string());
    assert_eq!(instance["template"], "ownership.generic");
    assert_eq!(instance["type_arguments"], json!(["i64"]));
    assert_eq!(instance["signature"]["parameters"][0]["ownership"], "value");
    assert_eq!(instance["signature"]["parameters"][0]["type_id"], "i64");
    assert_eq!(instance["cleanup_inventory"]["slots"], json!([]));
    assert_eq!(instance["loan_plan"]["loans"], json!([]));
    assert!(!instance["cleanup_plan"]["blocks"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(template["candidate"]["instances"], json!([]));
    assert_eq!(
        value["inventory"]["base_instances"].as_u64().unwrap(),
        value["inventory"]["candidate_instances"].as_u64().unwrap() + 1
    );
    replay(&candidate);
    assert_eq!(fixture.bytes(), disk);
}
