//! Compiler-derived field-borrow repairs; authored without running local gates.
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectCandidateAttempt,
    ProjectCandidateAttemptOutcome, SemanticChange,
};
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
            "spx-field-borrow-repair-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "field-borrow-repair"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "repair.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["repair.public"]
tests = ["repair.tests"]
"#,
        )
        .unwrap();
        for (path, source) in [
            (
                "src/core.spx",
                r#"module repair.core;
@id("repair.packet") record Packet { @id("repair.packet.left") left:Bytes, @id("repair.packet.right") right:Bytes, }
@id("repair.other") record Other { @id("repair.other.left") left:Bytes, }
@id("repair.make") fn make(input:borrow Slice<u8>)->Packet {Packet {left:bytes_copy(input),right:bytes_copy(input)}}
@id("repair.consume") fn consume(bytes:own Bytes)->i64 {7}
@id("repair.take") fn take(packet:own Packet)->i64 {7}
@id("repair.inspect") fn inspect(packet:own Packet)->usize {let view=bytes_as_slice(packet.left);let sibling=consume(packet.right);byte_len(view)}
@id("repair.borrowed") fn borrowed(packet:borrow Packet)->usize {0usize}
@id("repair.input") fn inspect_input(input:borrow Slice<u8>)->usize {0usize}
@id("repair.public") fn public_value(value:i64)->i64 {value}
@id("repair.evaluate") fn evaluate()->i64 {let input=[7u8,8u8];if inspect(make(array_as_slice(input)))==2usize {42}else{0}}
"#,
            ),
            (
                "src/app.spx",
                r#"module repair.app;
use function @id("repair.evaluate") from repair.core as evaluate;
@id("repair.main") fn main()->i64 {evaluate()}
"#,
            ),
            (
                "src/tests.spx",
                r#"module repair.tests;
use function @id("repair.evaluate") from repair.core as evaluate;
@id("repair.test") fn main()->i64 {if evaluate()==42 {0}else{1}}
"#,
            ),
        ] {
            let program = semaprax::parse(source, path).unwrap();
            std::fs::write(root.join(path), semaprax::format::canonical(&program)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn candidate(&self) -> Arc<ProjectCandidate> {
        Arc::new(
            with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
                ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
            })
            .unwrap(),
        )
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
fn place(name: &str) -> Value {
    json!({"kind":"place","name":name})
}
fn field(target: &str, root: &str) -> Value {
    json!({"kind":"field_place","target":target,"root":root})
}
fn project(target: &str, base: Value) -> Value {
    json!({"kind":"project","target":target,"base":base})
}
fn builtin(target: &str, args: Vec<Value>) -> Value {
    json!({"kind":"builtin_call","target":target,"arguments":args})
}
fn call(target: &str, args: Vec<Value>) -> Value {
    json!({"kind":"call","target":target,"arguments":args})
}
fn binding(name: &str, value: Value, body: Value) -> Value {
    json!({"kind":"let","name":name,"value":value,"body":body})
}
fn view(value: Value) -> Value {
    builtin("core.bytes.as-slice", vec![value])
}
fn length(value: Value) -> Value {
    builtin("core.bytes.len", vec![value])
}
fn intent(target: &str, body: Value) -> Value {
    json!({"kind":"replace_function_body","target":target,"body":body})
}
fn nested(repaired: bool) -> Value {
    let value = if repaired {
        field("repair.packet.left", "packet")
    } else {
        project("repair.packet.left", place("packet"))
    };
    binding(
        "view",
        view(value),
        binding(
            "sibling",
            call(
                "repair.consume",
                vec![field("repair.packet.right", "packet")],
            ),
            length(place("view")),
        ),
    )
}
fn apply(base: &ProjectCandidate, intent: &Value) -> Result<ProjectCandidate, Vec<Diagnostic>> {
    base.apply(
        base.candidate_digest(),
        &SemanticChange::new(base.revision().project_revision(), intent)?,
    )
}
fn rejected(base: &Arc<ProjectCandidate>, intent: &Value) -> Arc<ProjectCandidateAttempt> {
    match ProjectCandidateAttempt::apply(Arc::clone(base), base.candidate_digest(), intent).unwrap()
    {
        ProjectCandidateAttemptOutcome::Rejected(attempt) => attempt,
        ProjectCandidateAttemptOutcome::Accepted(_) => panic!("expected rejected candidate"),
    }
}
fn catalog(attempt: &ProjectCandidateAttempt) -> Value {
    serde_json::from_str(&attempt.repair_catalog(attempt.attempt_digest()).unwrap()).unwrap()
}
fn code<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    let errors = result.err().expect("expected rejection");
    assert!(errors.iter().any(|e| e.code == expected), "{errors:?}");
}

#[test]
fn actual_t266_nested_projection_has_one_exact_fully_admitted_field_place_repair() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let before = base.to_json().to_owned();
    let failed = intent("repair.inspect", nested(false));
    code(apply(&base, &failed), "SPX-T266");
    let attempt = rejected(&base, &failed);
    let report: Value = serde_json::from_str(attempt.to_json()).unwrap();
    assert_eq!(report["change"]["intent"], failed);
    assert!(report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["code"] == "SPX-T266"));
    assert_eq!(report["checked_image"], false);
    assert_eq!(report["materializable"], false);
    let discovered = catalog(&attempt);
    let repairs = discovered["repairs"].as_array().unwrap();
    assert_eq!(repairs.len(), 1);
    let repair = &repairs[0];
    assert_eq!(repair["class"], "borrow_owned_byte_field_without_staging");
    assert_eq!(repair["target"], "repair.inspect");
    assert_eq!(repair["diagnostic_code"], "SPX-T266");
    assert_eq!(repair["replacement_count"], 1);
    assert_eq!(
        repair["replacements"],
        json!([{"field":"repair.packet.left","root":"packet"}])
    );
    assert_eq!(
        repair["change"]["intent"],
        intent("repair.inspect", nested(true))
    );
    assert_eq!(repair["validation"], "normal_full_candidate_apply");
    assert_eq!(repair["source_authority"], false);
    assert_eq!(repair["tests"], "not_run");
    let selected = attempt
        .repair_diagnostic(
            attempt.attempt_digest(),
            repair["repair_id"].as_str().unwrap(),
        )
        .unwrap();
    let ordinary = apply(&base, &intent("repair.inspect", nested(true))).unwrap();
    assert_eq!(selected.to_json(), ordinary.to_json());
    assert_eq!(
        repair["validated_candidate_revision"],
        selected.candidate_digest()
    );
    assert_eq!(catalog(&attempt), discovered);
    assert_eq!(base.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn nested_branch_repairs_keep_each_field_selector_and_root_exact() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let branches = |fixed| json!({"kind":"if","condition":{"kind":"bool","value":true},"then":length(view(if fixed {field("repair.packet.left","packet")} else {project("repair.packet.left",place("packet"))})),"else":length(view(if fixed {field("repair.packet.right","packet")} else {project("repair.packet.right",place("packet"))}))});
    let failed = intent("repair.inspect", branches(false));
    code(apply(&base, &failed), "SPX-T266");
    let attempt = rejected(&base, &failed);
    let discovered = catalog(&attempt);
    assert_eq!(discovered["repairs"].as_array().unwrap().len(), 1);
    assert_eq!(discovered["repairs"][0]["replacement_count"], 2);
    assert_eq!(
        discovered["repairs"][0]["replacements"],
        json!([{"field":"repair.packet.left","root":"packet"},{"field":"repair.packet.right","root":"packet"}])
    );
    assert_eq!(
        discovered["repairs"][0]["change"]["intent"],
        intent("repair.inspect", branches(true))
    );
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn nonplace_roots_wrong_owners_and_remaining_ownership_errors_offer_no_repair() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let before = base.to_json().to_owned();
    let nonplace = intent(
        "repair.input",
        length(view(project(
            "repair.packet.left",
            call("repair.make", vec![place("input")]),
        ))),
    );
    let wrong_owner = intent(
        "repair.inspect",
        length(view(project("repair.other.left", place("packet")))),
    );
    let borrowed = intent(
        "repair.borrowed",
        length(view(project("repair.packet.left", place("packet")))),
    );
    let still_invalid = intent(
        "repair.inspect",
        binding(
            "view",
            view(project("repair.packet.left", place("packet"))),
            binding(
                "moved",
                call("repair.take", vec![place("packet")]),
                length(place("view")),
            ),
        ),
    );
    code(apply(&base, &still_invalid), "SPX-T266");
    let invalid_after_rewrite = intent(
        "repair.inspect",
        binding(
            "view",
            view(field("repair.packet.left", "packet")),
            binding(
                "moved",
                call("repair.take", vec![place("packet")]),
                length(place("view")),
            ),
        ),
    );
    code(apply(&base, &invalid_after_rewrite), "SPX-T265");
    let mut untrusted_effect = intent("repair.inspect", nested(false));
    untrusted_effect["body"]["value"]["effects"] = json!(["network"]);
    for failed in [
        nonplace,
        wrong_owner,
        borrowed,
        still_invalid,
        untrusted_effect,
    ] {
        let attempt = rejected(&base, &failed);
        assert!(catalog(&attempt)["repairs"].as_array().unwrap().is_empty());
        assert_eq!(base.to_json(), before);
    }
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn repair_history_rederives_exact_recovery_and_rejects_tamper_stale_predecessors_and_rebase() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let attempt = rejected(&base, &intent("repair.inspect", nested(false)));
    let discovered = catalog(&attempt);
    let wire = discovered["repairs"][0]["semantic_change_intent"].clone();
    let change = SemanticChange::new(base.revision().project_revision(), &wire).unwrap();
    let candidate = base.apply(base.candidate_digest(), &change).unwrap();
    let report: Value = serde_json::from_str(candidate.to_json()).unwrap();
    assert_eq!(report["operations"][0]["kind"], "repair_diagnostic");
    let capsule = candidate.recovery_capsule().unwrap();
    let capsule_value: Value = serde_json::from_str(&capsule).unwrap();
    assert_eq!(capsule_value["changes"][0]["intent"], wire);
    let restored = ProjectCandidate::restore(
        Arc::clone(base.base_revision()),
        base.base_revision().project_revision(),
        capsule.as_bytes(),
    )
    .unwrap();
    assert_eq!(restored.to_json(), candidate.to_json());
    let replayed = ProjectCandidate::replay(
        Arc::clone(base.base_revision()),
        base.base_revision().project_revision(),
        &[change],
        candidate.to_json().as_bytes(),
    )
    .unwrap();
    assert_eq!(replayed.to_json(), candidate.to_json());
    assert_eq!(
        replayed.revision().semantic_graph(),
        candidate.revision().semantic_graph()
    );
    let mut changed_root = wire.clone();
    changed_root["rejected_intent"]["body"]["value"]["arguments"][0]["base"]["name"] =
        json!("missing");
    let mut changed_selector = wire.clone();
    changed_selector["repair_id"] = json!(format!("sha256:{}", "0".repeat(64)));
    let mut offered = wire.clone();
    offered["replacement"] = intent("repair.inspect", nested(true));
    for (tampered, diagnostic) in [
        (changed_root, "SPX-G270"),
        (changed_selector, "SPX-G270"),
        (offered, "SPX-G268"),
    ] {
        code(apply(&base, &tampered), diagnostic);
    }
    let renamed = apply(
        &base,
        &json!({"kind":"rename_declaration","target":"repair.public","name":"renamed_public"}),
    )
    .unwrap();
    code(apply(&renamed, &wire), "SPX-G270");
    code(
        candidate.rebase(
            candidate.candidate_digest(),
            Arc::clone(renamed.revision()),
            renamed.revision().project_revision(),
        ),
        "SPX-G271",
    );
    code(
        attempt.repair_catalog(&format!("sha256:{}", "0".repeat(64))),
        "SPX-G243",
    );
    assert_eq!(fixture.bytes(), disk);
}
