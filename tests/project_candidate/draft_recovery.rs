//! Draft recovery evidence: authored and intentionally unrun.
use semaprax::digest_hex::LowerHex;
use semaprax::project::{with_authenticated_project, ProjectCandidate, ProjectCandidateDraft};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
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
            "spx-draft-recovery-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(example.join(path), root.join(path)).unwrap();
        }
        let path = root.join("src/core.spx");
        let mut source = std::fs::read_to_string(&path).unwrap();
        for index in 0..17 {
            source.push_str(&format!("\n@id(\"recovery.extra.{index}\") fn extra_{index}(value: i64) -> i64 {{ value }}\n"));
        }
        let parsed = semaprax::parse(&source, "src/core.spx").unwrap();
        std::fs::write(path, semaprax::format::canonical(&parsed)).unwrap();
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
fn selected(candidate: &ProjectCandidate, target: &str, snippet: &str) -> String {
    let catalog: Value =
        serde_json::from_str(&candidate.expression_catalog(target).unwrap()).unwrap();
    let source = candidate
        .revision()
        .sources()
        .iter()
        .find(|source| source.path() == "src/core.spx")
        .unwrap()
        .source();
    catalog["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| {
            let span = &row["source_span"];
            row["replaceable"] == true
                && source.get(
                    span["start"].as_u64().unwrap() as usize
                        ..span["end"].as_u64().unwrap() as usize,
                ) == Some(snippet)
        })
        .unwrap()["expression_id"]
        .as_str()
        .unwrap()
        .to_owned()
}
fn mixed(base: &Arc<ProjectCandidate>) -> ProjectCandidateDraft {
    let expression = selected(base, "calculator.subtract", "left - right");
    let draft = ProjectCandidateDraft::open(Arc::clone(base)).unwrap();
    let draft = draft
        .with_body_hole(draft.draft_digest(), "calculator.add", "z.add")
        .unwrap();
    let draft = draft
        .with_expression_hole(
            draft.draft_digest(),
            "calculator.subtract",
            &expression,
            "a.subtract",
        )
        .unwrap();
    draft
        .with_body_hole(draft.draft_digest(), "calculator.multiply", "m.multiply")
        .unwrap()
}
fn restore(
    base: &ProjectCandidate,
    bytes: &[u8],
) -> Result<ProjectCandidateDraft, Vec<semaprax::diagnostic::Diagnostic>> {
    ProjectCandidateDraft::restore(
        Arc::clone(base.base_revision()),
        base.base_revision().project_revision(),
        bytes,
    )
}
fn canonical(mut value: Value) -> String {
    value.sort_all_objects();
    let mut text = serde_json::to_string(&value).unwrap();
    text.push('\n');
    text
}
fn remint(mut value: Value) -> String {
    value.as_object_mut().unwrap().remove("capsule_digest");
    let payload = canonical(value.clone());
    let mut hasher = Sha256::new();
    hasher.update(b"semaprax.project-candidate-draft-recovery.payload.v1\0");
    hasher.update((payload.len() as u64).to_le_bytes());
    hasher.update(payload.as_bytes());
    value["capsule_digest"] = json!(format!("sha256:{:x}", LowerHex(hasher.finalize())));
    canonical(value)
}
fn integer(value: i64) -> Value {
    json!({"kind":"i64","value":value})
}
fn same_draft(left: &ProjectCandidateDraft, right: &ProjectCandidateDraft, holes: &[&str]) {
    assert_eq!(left.draft_digest(), right.draft_digest());
    assert_eq!(left.to_json(), right.to_json());
    assert_eq!(
        left.recovery_capsule().unwrap(),
        right.recovery_capsule().unwrap()
    );
    for hole in holes {
        assert_eq!(
            left.hole_context(left.draft_digest(), hole).unwrap(),
            right.hole_context(right.draft_digest(), hole).unwrap()
        );
    }
}

#[test]
fn mixed_body_and_expression_holes_recover_exact_contexts_without_materializing() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let draft = mixed(&base);
    let capsule = draft.recovery_capsule().unwrap();
    let value: Value = serde_json::from_str(&capsule).unwrap();
    assert_eq!(
        value["schema"],
        "semaprax.project-candidate-draft-recovery.v1"
    );
    assert_eq!(
        value["base_revision"],
        base.base_revision().project_revision()
    );
    assert_eq!(value["draft_digest"], draft.draft_digest());
    assert_eq!(
        value["candidate_recovery"]["schema"],
        "semaprax.project-candidate-recovery.v1"
    );
    assert_eq!(
        value["holes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|hole| hole["hole_id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["a.subtract", "m.multiply", "z.add"]
    );
    assert_eq!(value["holes"][0]["kind"], "expression");
    assert_eq!(value["holes"][1]["kind"], "function_body");
    let restored = restore(&base, capsule.as_bytes()).unwrap();
    same_draft(&draft, &restored, &["a.subtract", "m.multiply", "z.add"]);
    assert!(restored.complete(restored.draft_digest()).is_err());
    for hole in ["a.subtract", "m.multiply", "z.add"] {
        let context: Value = serde_json::from_str(
            &restored
                .hole_context(restored.draft_digest(), hole)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(context["materializable"], false);
        assert_eq!(context["source_authority"], false);
    }
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn partial_fill_history_and_remapped_expression_selectors_survive_recovery() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let draft = mixed(&base);
    let partial = draft
        .fill_hole(draft.draft_digest(), "z.add", &integer(99))
        .unwrap();
    assert!(partial
        .hole_context(partial.draft_digest(), "z.add")
        .is_err());
    assert!(partial.complete(partial.draft_digest()).is_err());
    let capsule = partial.recovery_capsule().unwrap();
    let value: Value = serde_json::from_str(&capsule).unwrap();
    assert_eq!(
        value["candidate_recovery"]["changes"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(value["holes"].as_array().unwrap().len(), 2);
    let restored = restore(&base, capsule.as_bytes()).unwrap();
    same_draft(&partial, &restored, &["a.subtract", "m.multiply"]);
    let before = restored.to_json().to_owned();
    assert!(restored
        .fill_hole(
            restored.draft_digest(),
            "a.subtract",
            &json!({"kind":"bool","value":true})
        )
        .is_err());
    assert_eq!(restored.to_json(), before);
    let first = restored
        .fill_hole(restored.draft_digest(), "a.subtract", &integer(7))
        .unwrap();
    assert!(first.complete(first.draft_digest()).is_err());
    let done = first
        .fill_hole(first.draft_digest(), "m.multiply", &integer(3))
        .unwrap();
    let candidate = done.complete(done.draft_digest()).unwrap();
    let original_first = partial
        .fill_hole(partial.draft_digest(), "a.subtract", &integer(7))
        .unwrap();
    let original_done = original_first
        .fill_hole(original_first.draft_digest(), "m.multiply", &integer(3))
        .unwrap();
    assert_eq!(
        candidate.to_json(),
        original_done
            .complete(original_done.draft_digest())
            .unwrap()
            .to_json()
    );
    let evidence: Value = serde_json::from_str(candidate.to_json()).unwrap();
    assert_eq!(evidence["changes"].as_array().unwrap().len(), 3);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn both_initially_ready_and_fully_filled_drafts_recover_the_exact_complete_candidate() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let initial = ProjectCandidateDraft::open(Arc::clone(&base)).unwrap();
    let restored = restore(&base, initial.recovery_capsule().unwrap().as_bytes()).unwrap();
    same_draft(&initial, &restored, &[]);
    assert_eq!(
        restored
            .complete(restored.draft_digest())
            .unwrap()
            .to_json(),
        base.to_json()
    );
    let pending = initial
        .with_body_hole(initial.draft_digest(), "calculator.add", "add")
        .unwrap();
    let done = pending
        .fill_hole(pending.draft_digest(), "add", &integer(21))
        .unwrap();
    let capsule = done.recovery_capsule().unwrap();
    let value: Value = serde_json::from_str(&capsule).unwrap();
    assert_eq!(value["holes"], json!([]));
    let restored = restore(&base, capsule.as_bytes()).unwrap();
    same_draft(&done, &restored, &[]);
    assert_eq!(
        restored
            .complete(restored.draft_digest())
            .unwrap()
            .to_json(),
        done.complete(done.draft_digest()).unwrap().to_json()
    );
    assert!(restored.complete(initial.draft_digest()).is_err());
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn tampering_canonicality_reminted_wrong_proofs_and_stale_bases_never_recover() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let draft = mixed(&base);
    let before = draft.to_json().to_owned();
    let capsule = draft.recovery_capsule().unwrap();
    let original: Value = serde_json::from_str(&capsule).unwrap();
    assert_eq!(remint(original.clone()), capsule);
    let mut unknown = original.clone();
    unknown["source_authority"] = json!(true);
    assert!(restore(&base, remint(unknown).as_bytes()).is_err());
    let mut digest = original.clone();
    digest["draft_digest"] = json!(format!("sha256:{}", "0".repeat(64)));
    assert!(restore(&base, remint(digest).as_bytes()).is_err());
    let mut wrong_candidate = original.clone();
    wrong_candidate["candidate_recovery"]["candidate_digest"] =
        json!(format!("sha256:{}", "0".repeat(64)));
    assert!(restore(&base, remint(wrong_candidate).as_bytes()).is_err());
    let mut altered = original.clone();
    altered["holes"][1]["target"] = json!("calculator.not");
    assert!(restore(&base, canonical(altered).as_bytes()).is_err());
    assert!(restore(&base, capsule.trim_end().as_bytes()).is_err());
    assert!(restore(&base, format!(" {capsule}").as_bytes()).is_err());
    let duplicate = format!(
        "{{\"schema\":\"semaprax.project-candidate-draft-recovery.v1\",{}",
        &capsule[1..]
    );
    assert!(restore(&base, duplicate.as_bytes()).is_err());
    let mut reordered = original.clone();
    reordered["holes"].as_array_mut().unwrap().reverse();
    assert!(restore(&base, remint(reordered).as_bytes()).is_err());
    assert!(ProjectCandidateDraft::restore(
        Arc::clone(base.base_revision()),
        &format!("sha256:{}", "0".repeat(64)),
        capsule.as_bytes()
    )
    .is_err());
    let pending = ProjectCandidateDraft::open(Arc::clone(&base)).unwrap();
    let pending = pending
        .with_body_hole(pending.draft_digest(), "calculator.add", "changed")
        .unwrap();
    let done = pending
        .fill_hole(pending.draft_digest(), "changed", &integer(17))
        .unwrap();
    let changed = done.complete(done.draft_digest()).unwrap();
    assert!(ProjectCandidateDraft::restore(
        Arc::clone(changed.revision()),
        changed.revision().project_revision(),
        capsule.as_bytes()
    )
    .is_err());
    assert_eq!(draft.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn hole_bounds_duplicates_overlap_and_invalid_authenticated_selectors_fail_without_mutation() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let mut draft = ProjectCandidateDraft::open(Arc::clone(&base)).unwrap();
    for index in 0..16 {
        draft = draft
            .with_body_hole(
                draft.draft_digest(),
                &format!("recovery.extra.{index}"),
                &format!("hole.{index:02}"),
            )
            .unwrap();
    }
    let capsule = draft.recovery_capsule().unwrap();
    let restored = restore(&base, capsule.as_bytes()).unwrap();
    assert_eq!(restored.draft_digest(), draft.draft_digest());
    let before = restored.to_json().to_owned();
    assert!(restored
        .with_body_hole(restored.draft_digest(), "recovery.extra.16", "hole.16")
        .is_err());
    let mut oversized: Value = serde_json::from_str(&capsule).unwrap();
    oversized["holes"]
        .as_array_mut()
        .unwrap()
        .push(json!({"kind":"function_body","hole_id":"hole.16","target":"recovery.extra.16"}));
    let errors = restore(&base, remint(oversized).as_bytes()).err().unwrap();
    assert!(
        errors.iter().any(|error| error.code == "SPX-G231"),
        "{errors:?}"
    );
    let mixed = mixed(&base);
    let original: Value = serde_json::from_str(&mixed.recovery_capsule().unwrap()).unwrap();
    let mut duplicate = original.clone();
    duplicate["holes"][1]["hole_id"] = duplicate["holes"][0]["hole_id"].clone();
    assert!(restore(&base, remint(duplicate).as_bytes()).is_err());
    let mut overlap = original.clone();
    overlap["holes"][1]["target"] = json!("calculator.subtract");
    assert!(restore(&base, remint(overlap).as_bytes()).is_err());
    let mut bad_selector = original.clone();
    bad_selector["holes"][0]["expression_id"] = json!("not-an-authenticated-expression");
    assert!(restore(&base, remint(bad_selector).as_bytes()).is_err());
    let mut wrong_target = original;
    wrong_target["holes"][0]["target"] = json!("calculator.add");
    assert!(restore(&base, remint(wrong_target).as_bytes()).is_err());
    assert_eq!(restored.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
}
