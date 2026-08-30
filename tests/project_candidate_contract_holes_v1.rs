//! Contract-region hole evidence: authored and intentionally unrun.
use semaprax::diagnostic::Diagnostic;
use semaprax::digest_hex::LowerHex;
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectCandidateDraft, SemanticChange,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const TARGET: &str = "contract-holes.checked";
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-contract-holes-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let root = root.canonicalize().unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "contract-holes"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "contract_holes.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["contract-holes.public"]
tests = ["contract_holes.tests"]
"#,
        )
        .unwrap();
        for (path, source) in [
            (
                "src/core.spx",
                r#"module contract_holes.core;
@id("contract-holes.checked") fn checked(left:i64,right:i64)->i64
requires left >= 0 requires right != 0 ensures result >= left {let local = left + right; local}
@id("contract-holes.public") fn public_value(value:i64)->i64 {value}
"#,
            ),
            (
                "src/app.spx",
                r#"module contract_holes.app;
use function @id("contract-holes.checked") from contract_holes.core as checked;
@id("contract-holes.main") fn main()->i64 {checked(4,2)}
"#,
            ),
            (
                "src/tests.spx",
                r#"module contract_holes.tests;
use function @id("contract-holes.checked") from contract_holes.core as checked;
@id("contract-holes.test") fn main()->i64 {if checked(4,2) == 6 {0}else{1}}
"#,
            ),
        ] {
            let program = semaprax::parse(source, path).unwrap();
            std::fs::write(root.join(path), semaprax::format::canonical(&program)).unwrap();
        }
        Self(root)
    }
    fn candidate(&self) -> Arc<ProjectCandidate> {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
                .map(Arc::new)
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
        .map(|path| std::fs::read(self.0.join(path)).unwrap())
        .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn source(candidate: &ProjectCandidate) -> &str {
    candidate
        .revision()
        .sources()
        .iter()
        .find(|source| source.path() == "src/core.spx")
        .unwrap()
        .source()
}
fn selected(candidate: &ProjectCandidate, phase: &str, snippet: &str) -> String {
    let catalog: Value =
        serde_json::from_str(&candidate.contract_expression_catalog(TARGET).unwrap()).unwrap();
    let rows: Vec<_> = catalog["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| {
            let span = &row["source_span"];
            row["phase"] == phase
                && row["replaceable"] == true
                && source(candidate).get(
                    span["start"].as_u64().unwrap() as usize
                        ..span["end"].as_u64().unwrap() as usize,
                ) == Some(snippet)
        })
        .collect();
    assert_eq!(
        rows.len(),
        1,
        "unique authenticated selector for {phase}: {snippet}"
    );
    rows[0]["expression_id"].as_str().unwrap().to_owned()
}
fn open(
    base: &Arc<ProjectCandidate>,
    phase: &str,
    snippet: &str,
    hole: &str,
) -> ProjectCandidateDraft {
    let draft = ProjectCandidateDraft::open(Arc::clone(base)).unwrap();
    draft
        .with_contract_expression_hole(
            draft.draft_digest(),
            TARGET,
            &selected(base, phase, snippet),
            hole,
        )
        .unwrap()
}
fn context(draft: &ProjectCandidateDraft, hole: &str) -> Value {
    serde_json::from_str(&draft.hole_context(draft.draft_digest(), hole).unwrap()).unwrap()
}
fn place(name: &str) -> Value {
    json!({"kind":"place","name":name})
}
fn integer(value: i64) -> Value {
    json!({"kind":"i64","value":value})
}
fn apply(candidate: &ProjectCandidate, intent: Value) -> Result<ProjectCandidate, Vec<Diagnostic>> {
    candidate.apply(
        candidate.candidate_digest(),
        &SemanticChange::new(candidate.revision().project_revision(), &intent)?,
    )
}
fn restore(
    base: &ProjectCandidate,
    bytes: &[u8],
) -> Result<ProjectCandidateDraft, Vec<Diagnostic>> {
    ProjectCandidateDraft::restore(
        Arc::clone(base.base_revision()),
        base.base_revision().project_revision(),
        bytes,
    )
}
fn replay(candidate: &ProjectCandidate) {
    let capsule = candidate.recovery_capsule().unwrap();
    let restored = ProjectCandidate::restore(
        Arc::clone(candidate.base_revision()),
        candidate.base_revision().project_revision(),
        capsule.as_bytes(),
    )
    .unwrap();
    assert_eq!(candidate.to_json(), restored.to_json());
    assert_eq!(candidate.candidate_digest(), restored.candidate_digest());
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
fn canonical(mut value: Value) -> String {
    value.sort_all_objects();
    serde_json::to_string(&value).unwrap() + "\n"
}
fn remint(mut value: Value) -> String {
    value.as_object_mut().unwrap().remove("capsule_digest");
    let payload = canonical(value.clone());
    let mut hash = Sha256::new();
    hash.update(b"semaprax.project-candidate-draft-recovery.payload.v1\0");
    hash.update((payload.len() as u64).to_le_bytes());
    hash.update(payload.as_bytes());
    value["capsule_digest"] = json!(format!("sha256:{:x}", LowerHex(hash.finalize())));
    canonical(value)
}

#[test]
fn contract_context_authenticates_result_scope_without_changing_legacy_read_only_catalog() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let legacy: Value = serde_json::from_str(&base.expression_catalog(TARGET).unwrap()).unwrap();
    assert!(legacy["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["phase"] != "body")
        .all(|row| row["replaceable"] == false));
    let requires = open(&base, "requires", "left >= 0", "pre");
    let ensures = open(&base, "ensures", "result >= left", "post");
    for (draft, hole, phase, result_visible) in [
        (&requires, "pre", "requires", false),
        (&ensures, "post", "ensures", true),
    ] {
        let ctx = context(draft, hole);
        assert_eq!(
            ctx["schema"],
            "semaprax.project-candidate-contract-expression-hole-context.v1"
        );
        assert_eq!(ctx["selected_expression"]["phase"], phase);
        assert_eq!(ctx["expected_type_id"], "bool");
        let names: Vec<_> = ctx["scope"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"left") && names.contains(&"right"));
        assert_eq!(names.contains(&"result"), result_visible);
        assert!(!names.contains(&"local"));
        assert_eq!(ctx["materializable"], false);
        assert_eq!(ctx["source_authority"], false);
        code(draft.complete(draft.draft_digest()), "SPX-G232");
    }
    assert!(requires
        .fill_hole(
            requires.draft_digest(),
            "pre",
            &json!({"kind":"binary","op":">=","left":place("result"),"right":integer(0)})
        )
        .is_err());
    let filled = ensures.fill_hole(ensures.draft_digest(),"post",&json!({"kind":"binary","op":"==","left":place("result"),"right":{"kind":"binary","op":"+","left":place("left"),"right":place("right")}})).unwrap();
    let candidate = filled.complete(filled.draft_digest()).unwrap();
    assert!(source(&candidate).contains("ensures result == left + right"));
    replay(&candidate);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn scalar_subtree_type_and_contract_boolean_type_reject_without_mutating_draft() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let draft = open(&base, "ensures", "result", "scalar");
    let before = draft.to_json().to_owned();
    let capsule = draft.recovery_capsule().unwrap();
    assert_eq!(context(&draft, "scalar")["expected_type_id"], "i64");
    for replacement in [
        json!({"kind":"bool","value":true}),
        place("local"),
        place("missing"),
    ] {
        assert!(draft
            .fill_hole(draft.draft_digest(), "scalar", &replacement)
            .is_err());
        assert_eq!(draft.to_json(), before);
        assert_eq!(draft.recovery_capsule().unwrap(), capsule);
    }
    let boolean = open(&base, "requires", "left >= 0", "boolean");
    assert!(boolean
        .fill_hole(boolean.draft_digest(), "boolean", &integer(7))
        .is_err());
    let filled = draft
        .fill_hole(
            draft.draft_digest(),
            "scalar",
            &json!({"kind":"binary","op":"+","left":place("result"),"right":integer(1)}),
        )
        .unwrap();
    code(
        filled.fill_hole(draft.draft_digest(), "scalar", &integer(0)),
        "SPX-G232",
    );
    let final_candidate = filled.complete(filled.draft_digest()).unwrap();
    replay(&final_candidate);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn disjoint_contract_leaves_and_body_hole_remap_and_recover_partial_history() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let draft = open(&base, "ensures", "result", "a.result");
    let draft = draft
        .with_contract_expression_hole(
            draft.draft_digest(),
            TARGET,
            &selected(&base, "ensures", "left"),
            "b.left",
        )
        .unwrap();
    let draft = draft
        .with_contract_expression_hole(
            draft.draft_digest(),
            TARGET,
            &selected(&base, "requires", "right != 0"),
            "c.requires",
        )
        .unwrap();
    let draft = draft
        .with_body_hole(draft.draft_digest(), TARGET, "d.body")
        .unwrap();
    let recovered = restore(&base, draft.recovery_capsule().unwrap().as_bytes()).unwrap();
    assert_eq!(draft.to_json(), recovered.to_json());
    for hole in ["a.result", "b.left", "c.requires", "d.body"] {
        assert_eq!(context(&draft, hole), context(&recovered, hole));
    }
    let partial = recovered
        .fill_hole(
            recovered.draft_digest(),
            "d.body",
            &json!({"kind":"binary","op":"+","left":place("left"),"right":place("right")}),
        )
        .unwrap();
    let partial = partial
        .fill_hole(
            partial.draft_digest(),
            "a.result",
            &json!({"kind":"binary","op":"+","left":place("result"),"right":integer(1)}),
        )
        .unwrap();
    let partial = restore(&base, partial.recovery_capsule().unwrap().as_bytes()).unwrap();
    code(partial.complete(partial.draft_digest()), "SPX-G232");
    assert_eq!(
        context(&partial, "b.left")["selected_expression"]["phase"],
        "ensures"
    );
    let partial = partial
        .fill_hole(partial.draft_digest(), "b.left", &integer(0))
        .unwrap();
    let ready = partial
        .fill_hole(
            partial.draft_digest(),
            "c.requires",
            &json!({"kind":"bool","value":true}),
        )
        .unwrap();
    let recovered_ready = restore(&base, ready.recovery_capsule().unwrap().as_bytes()).unwrap();
    assert_eq!(ready.to_json(), recovered_ready.to_json());
    let final_candidate = recovered_ready
        .complete(recovered_ready.draft_digest())
        .unwrap();
    let program = semaprax::parse(source(&final_candidate), "src/core.spx").unwrap();
    let function = program
        .functions
        .iter()
        .find(|function| function.stable_id == TARGET)
        .unwrap();
    assert_eq!(function.requires.len(), 2);
    assert_eq!(function.ensures.len(), 1);
    assert!(source(&final_candidate).contains("ensures result + 1 >= 0"));
    assert!(!source(&final_candidate).contains("let local"));
    replay(&final_candidate);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn overlap_forged_selectors_and_reminted_recovery_cannot_release_pending_candidate() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let draft = open(&base, "ensures", "result", "leaf");
    assert!(draft
        .with_contract_expression_hole(
            draft.draft_digest(),
            TARGET,
            &selected(&base, "ensures", "result >= left"),
            "overlap"
        )
        .is_err());
    assert!(draft
        .with_contract_expression_hole(draft.draft_digest(), TARGET, "not-a-hir-id", "forged")
        .is_err());
    let capsule = draft.recovery_capsule().unwrap();
    let value: Value = serde_json::from_str(&capsule).unwrap();
    assert_eq!(value["holes"][0]["kind"], "contract_expression");
    let mut unknown = value.clone();
    unknown["untrusted"] = json!(true);
    assert!(restore(&base, remint(unknown).as_bytes()).is_err());
    let mut bad_selector = value.clone();
    bad_selector["holes"][0]["expression_id"] = json!("forged");
    assert!(restore(&base, remint(bad_selector).as_bytes()).is_err());
    let mut duplicate = value.clone();
    duplicate["holes"]
        .as_array_mut()
        .unwrap()
        .push(value["holes"][0].clone());
    code(restore(&base, remint(duplicate).as_bytes()), "SPX-G230");
    let mut wrong_digest = value.clone();
    wrong_digest["draft_digest"] = json!(format!("sha256:{}", "0".repeat(64)));
    code(restore(&base, remint(wrong_digest).as_bytes()), "SPX-G232");
    assert!(restore(
        &base,
        serde_json::to_string_pretty(&value).unwrap().as_bytes()
    )
    .is_err());
    let changed = apply(
        &base,
        json!({"kind":"replace_function_body","target":TARGET,"body":integer(6)}),
    )
    .unwrap();
    assert!(ProjectCandidateDraft::restore(
        Arc::clone(changed.revision()),
        changed.revision().project_revision(),
        capsule.as_bytes()
    )
    .is_err());
    assert_eq!(draft.recovery_capsule().unwrap(), capsule);
    code(draft.complete(draft.draft_digest()), "SPX-G232");
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn filled_contract_change_rebases_over_body_only_change_and_conflicts_on_contracts_or_signature() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let draft = open(&base, "ensures", "result", "result");
    let ready = draft
        .fill_hole(
            draft.draft_digest(),
            "result",
            &json!({"kind":"binary","op":"+","left":place("result"),"right":integer(1)}),
        )
        .unwrap();
    let own = ready.complete(ready.draft_digest()).unwrap();
    let body = apply(
        &base,
        json!({"kind":"replace_function_body","target":TARGET,"body":integer(6)}),
    )
    .unwrap();
    let rebased = own
        .rebase(
            own.candidate_digest(),
            Arc::clone(body.revision()),
            body.revision().project_revision(),
        )
        .unwrap();
    assert!(source(rebased.candidate()).contains("ensures result + 1 >= left"));
    assert!(source(rebased.candidate()).contains("    6\n"));
    replay(rebased.candidate());
    for intent in [
        json!({"kind":"add_contract","target":TARGET,"phase":"requires","predicate":{"kind":"bool","value":true}}),
        json!({"kind":"change_function_signature","target":TARGET,"append_parameters":[{"name":"extra","type":"i64","argument":integer(0)}]}),
    ] {
        let competing = apply(&base, intent).unwrap();
        code(
            own.rebase(
                own.candidate_digest(),
                Arc::clone(competing.revision()),
                competing.revision().project_revision(),
            ),
            "SPX-G235",
        );
    }
    // The ID spelling may be reused; staleness is the explicit revision binding.
    let stale_change = SemanticChange::new(
        base.revision().project_revision(),
        &json!({
            "kind":"replace_contract_expression","target":TARGET,
            "expression_id":selected(&base,"ensures","result"),"replacement":integer(0)
        }),
    )
    .unwrap();
    code(
        body.apply(body.candidate_digest(), &stale_change),
        "SPX-G224",
    );
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn contract_hole_rejects_effectful_call_despite_enclosing_declared_effect() {
    let fixture = Fixture::new();
    // Match the existing compiler SPX-C102 fixture: a declared effect is
    // sufficient; no clock intrinsic or runtime execution is needed.
    for path in ["src/core.spx", "src/app.spx", "src/tests.spx"] {
        let mut text = std::fs::read_to_string(fixture.0.join(path)).unwrap();
        if path == "src/core.spx" {
            text.push_str("\n@id(\"contract-holes.tick\") fn tick(value:i64)->i64 uses { clock.read } {value + 1}\n");
        }
        let mut program = semaprax::parse(&text, path).unwrap();
        program.permits.push("clock.read".to_owned());
        for function in &mut program.functions {
            if function.stable_id != "contract-holes.public" {
                function.effects = vec!["clock.read".to_owned()];
            }
        }
        std::fs::write(fixture.0.join(path), semaprax::format::canonical(&program)).unwrap();
    }
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let draft = open(&base, "ensures", "result", "pure-contract");
    let before = draft.recovery_capsule().unwrap();
    let ctx = context(&draft, "pure-contract");
    assert_eq!(ctx["effect_policy"]["allowed"], json!([]));
    assert_eq!(
        ctx["effect_policy"]["enclosing_declared_effects"],
        json!(["clock.read"])
    );
    let tick = ctx["accessible_calls"]
        .as_array()
        .unwrap()
        .iter()
        .find(|call| call["id"] == "contract-holes.tick")
        .unwrap();
    assert_eq!(tick["effects"], json!(["clock.read"]));
    assert_eq!(tick["within_effect_budget"], false);
    code(
        draft.fill_hole(
            draft.draft_digest(),
            "pure-contract",
            &json!({
                "kind":"call", "target":"contract-holes.tick", "arguments":[place("result")]
            }),
        ),
        "SPX-C102",
    );
    assert_eq!(draft.recovery_capsule().unwrap(), before);
    code(draft.complete(draft.draft_digest()), "SPX-G232");
    assert_eq!(fixture.bytes(), disk);
}
