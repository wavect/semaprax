//! Source-backed draft archive evidence, authored and intentionally unrun.
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectCandidateDraft,
    ProjectCandidateDraftArchive, SemanticChange,
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
            "spx-draft-archive-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
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
            let parsed = semaprax::parse(source, path).unwrap();
            std::fs::write(root.join(path), semaprax::format::canonical(&parsed)).unwrap();
        }
        Self(root.canonicalize().unwrap())
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
fn selected(candidate: &ProjectCandidate, contract: bool, target: &str, snippet: &str) -> String {
    let text = if contract {
        candidate.contract_expression_catalog(target)
    } else {
        candidate.expression_catalog(target)
    }
    .unwrap();
    let catalog: Value = serde_json::from_str(&text).unwrap();
    let source = candidate
        .revision()
        .sources()
        .iter()
        .find(|source| source.path() == "src/core.spx")
        .unwrap()
        .source();
    let rows: Vec<_> = catalog["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| {
            let span = &row["source_span"];
            row["replaceable"] == true
                && source.get(
                    span["start"].as_u64().unwrap() as usize
                        ..span["end"].as_u64().unwrap() as usize,
                ) == Some(snippet)
        })
        .collect();
    assert_eq!(rows.len(), 1);
    rows[0]["expression_id"].as_str().unwrap().to_owned()
}
fn mixed(candidate: &Arc<ProjectCandidate>) -> ProjectCandidateDraft {
    let draft = ProjectCandidateDraft::open(Arc::clone(candidate)).unwrap();
    let draft = draft
        .with_body_hole(draft.draft_digest(), TARGET, "a.body")
        .unwrap();
    let draft = draft
        .with_expression_hole(
            draft.draft_digest(),
            "contract-holes.public",
            &selected(candidate, false, "contract-holes.public", "value"),
            "b.expression",
        )
        .unwrap();
    draft
        .with_contract_expression_hole(
            draft.draft_digest(),
            TARGET,
            &selected(candidate, true, TARGET, "right != 0"),
            "c.contract",
        )
        .unwrap()
}
fn archive(draft: &ProjectCandidateDraft) -> ProjectCandidateDraftArchive {
    ProjectCandidateDraftArchive::prepare(draft, draft.draft_digest()).unwrap()
}
fn restore(saved: &ProjectCandidateDraftArchive) -> ProjectCandidateDraft {
    ProjectCandidateDraftArchive::restore(
        saved.to_json().as_bytes(),
        saved.archive_digest(),
        saved.draft_digest(),
    )
    .unwrap()
}
fn same(left: &ProjectCandidateDraft, right: &ProjectCandidateDraft, holes: &[&str]) {
    assert_eq!(left.to_json(), right.to_json());
    assert_eq!(left.draft_digest(), right.draft_digest());
    assert_eq!(
        left.recovery_capsule().unwrap(),
        right.recovery_capsule().unwrap()
    );
    assert_eq!(archive(left).to_json(), archive(right).to_json());
    for hole in holes {
        assert_eq!(
            left.hole_context(left.draft_digest(), hole).unwrap(),
            right.hole_context(right.draft_digest(), hole).unwrap()
        );
    }
}
fn canonical(mut value: Value) -> String {
    value.sort_all_objects();
    format!("{value}\n")
}
fn remint(mut value: Value, field: &str, domain: &[u8]) -> (String, String) {
    value.as_object_mut().unwrap().remove(field);
    let bytes = canonical(value.clone());
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes.as_bytes());
    let digest = format!(
        "sha256:{}",
        hash.finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    value[field] = json!(digest);
    (canonical(value), digest)
}
fn mint(value: Value) -> (String, String) {
    remint(
        value,
        "archive_digest",
        b"semaprax.project-candidate-draft-archive.payload.v1\0",
    )
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

#[test]
fn deleted_sources_rebuild_all_three_hole_kinds_and_partially_filled_history() {
    let fixture = Fixture::new();
    let base = fixture.candidate();
    let draft = mixed(&base);
    let saved = archive(&draft);
    let partial = draft.fill_hole(draft.draft_digest(), "a.body", &json!({"kind":"binary","op":"+","left":{"kind":"place","name":"left"},"right":{"kind":"place","name":"right"}})).unwrap();
    let saved_partial = archive(&partial);
    let outer: Value = serde_json::from_str(saved_partial.to_json()).unwrap();
    assert_eq!(
        outer["schema"],
        "semaprax.project-candidate-draft-archive.v1"
    );
    assert_eq!(
        saved_partial.base_revision(),
        base.base_revision().project_revision()
    );
    for field in ["source_authority", "approval_authority", "trusted_hir"] {
        assert_eq!(outer[field], false);
    }
    let recovery: Value =
        serde_json::from_str(outer["draft_recovery_capsule"].as_str().unwrap()).unwrap();
    assert_eq!(
        recovery["candidate_recovery"]["changes"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(recovery["holes"].as_array().unwrap().len(), 2);
    let candidate_archive: Value =
        serde_json::from_str(outer["candidate_archive"].as_str().unwrap()).unwrap();
    assert_eq!(
        candidate_archive["schema"],
        "semaprax.project-candidate-archive.v1"
    );
    assert_eq!(
        candidate_archive["archive_digest"],
        outer["candidate_archive_digest"]
    );
    assert_eq!(
        candidate_archive["candidate_digest"],
        outer["candidate_digest"]
    );
    std::fs::remove_dir_all(&fixture.0).unwrap();
    same(
        &draft,
        &restore(&saved),
        &["a.body", "b.expression", "c.contract"],
    );
    let restored = restore(&saved_partial);
    same(&partial, &restored, &["b.expression", "c.contract"]);
    code(restored.complete(restored.draft_digest()), "SPX-G232");
    assert!(!fixture.0.exists());
}

#[test]
fn ready_archive_completes_and_continues_without_overwriting_changed_original_files() {
    let fixture = Fixture::new();
    let base = fixture.candidate();
    let initial = ProjectCandidateDraft::open(Arc::clone(&base)).unwrap();
    let ready = restore(&archive(&initial));
    assert_eq!(
        ready.complete(ready.draft_digest()).unwrap().to_json(),
        base.to_json()
    );
    let draft = mixed(&base);
    let draft = draft
        .fill_hole(
            draft.draft_digest(),
            "a.body",
            &json!({"kind":"i64","value":6}),
        )
        .unwrap();
    let draft = draft
        .fill_hole(
            draft.draft_digest(),
            "b.expression",
            &json!({"kind":"i64","value":7}),
        )
        .unwrap();
    let ready = draft
        .fill_hole(
            draft.draft_digest(),
            "c.contract",
            &json!({"kind":"bool","value":true}),
        )
        .unwrap();
    let saved = archive(&ready);
    std::fs::write(
        fixture.0.join("src/core.spx"),
        "unrelated replacement bytes\n",
    )
    .unwrap();
    let disk = fixture.bytes();
    let restored = restore(&saved);
    same(&ready, &restored, &[]);
    let complete = restored.complete(restored.draft_digest()).unwrap();
    assert_eq!(
        complete.to_json(),
        ready.complete(ready.draft_digest()).unwrap().to_json()
    );
    let change = SemanticChange::new(complete.revision().project_revision(), &json!({"kind":"rename_declaration","target":"contract-holes.public","name":"published_value"})).unwrap();
    let continued = complete
        .apply(complete.candidate_digest(), &change)
        .unwrap();
    assert_ne!(continued.candidate_digest(), complete.candidate_digest());
    let continued = ProjectCandidateDraft::open(Arc::new(continued)).unwrap();
    same(&continued, &restore(&archive(&continued)), &[]);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn selectors_canonical_bytes_unknown_fields_and_claimed_authority_fail_closed() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let draft = mixed(&base);
    let saved = archive(&draft);
    let wrong = format!("sha256:{}", "0".repeat(64));
    code(
        ProjectCandidateDraftArchive::prepare(&draft, &wrong),
        "SPX-G342",
    );
    code(
        ProjectCandidateDraftArchive::restore(
            saved.to_json().as_bytes(),
            &wrong,
            saved.draft_digest(),
        ),
        "SPX-G342",
    );
    code(
        ProjectCandidateDraftArchive::restore(
            saved.to_json().as_bytes(),
            saved.archive_digest(),
            &wrong,
        ),
        "SPX-G342",
    );
    let original: Value = serde_json::from_str(saved.to_json()).unwrap();
    assert_eq!(mint(original.clone()).0, saved.to_json());
    for bytes in [
        serde_json::to_string_pretty(&original).unwrap(),
        format!("{}\n", saved.to_json()),
        saved.to_json().trim_end().to_owned(),
    ] {
        code(
            ProjectCandidateDraftArchive::restore(
                bytes.as_bytes(),
                saved.archive_digest(),
                saved.draft_digest(),
            ),
            "SPX-G340",
        );
    }
    for field in [
        "source_authority",
        "approval_authority",
        "trusted_hir",
        "unknown",
    ] {
        let mut changed = original.clone();
        changed[field] = json!(true);
        let (bytes, digest) = mint(changed);
        code(
            ProjectCandidateDraftArchive::restore(bytes.as_bytes(), &digest, saved.draft_digest()),
            "SPX-G340",
        );
    }
    same(
        &draft,
        &restore(&saved),
        &["a.body", "b.expression", "c.contract"],
    );
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn reminted_nested_source_and_hole_facts_cannot_bypass_independent_reconstruction() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let draft = mixed(&fixture.candidate());
    let saved = archive(&draft);
    let original: Value = serde_json::from_str(saved.to_json()).unwrap();
    let mut changed = original.clone();
    let mut nested: Value =
        serde_json::from_str(changed["candidate_archive"].as_str().unwrap()).unwrap();
    nested["sources"][0]["source"] = json!("module invalid;\n");
    let (nested_bytes, nested_digest) = remint(
        nested,
        "archive_digest",
        b"semaprax.project-candidate-archive.payload.v1\0",
    );
    changed["candidate_archive"] = json!(nested_bytes);
    changed["candidate_archive_digest"] = json!(nested_digest);
    let (bytes, digest) = mint(changed);
    assert!(
        ProjectCandidateDraftArchive::restore(bytes.as_bytes(), &digest, saved.draft_digest())
            .is_err()
    );
    for field in ["target", "expression_id"] {
        let mut changed = original.clone();
        let mut nested: Value =
            serde_json::from_str(changed["draft_recovery_capsule"].as_str().unwrap()).unwrap();
        nested["holes"][1][field] = json!("does.not.exist");
        let (nested_bytes, _) = remint(
            nested,
            "capsule_digest",
            b"semaprax.project-candidate-draft-recovery.payload.v1\0",
        );
        changed["draft_recovery_capsule"] = json!(nested_bytes);
        let (bytes, digest) = mint(changed);
        assert!(ProjectCandidateDraftArchive::restore(
            bytes.as_bytes(),
            &digest,
            saved.draft_digest()
        )
        .is_err());
    }
    let mut changed = original;
    changed["base_revision"] = json!(format!("sha256:{}", "0".repeat(64)));
    let (bytes, digest) = mint(changed);
    code(
        ProjectCandidateDraftArchive::restore(bytes.as_bytes(), &digest, saved.draft_digest()),
        "SPX-G342",
    );
    let deep = format!("{}0{}", "[".repeat(129), "]".repeat(129));
    code(
        ProjectCandidateDraftArchive::restore(
            deep.as_bytes(),
            saved.archive_digest(),
            saved.draft_digest(),
        ),
        "SPX-G341",
    );
    same(
        &draft,
        &restore(&saved),
        &["a.body", "b.expression", "c.contract"],
    );
    assert_eq!(fixture.bytes(), disk);
}
