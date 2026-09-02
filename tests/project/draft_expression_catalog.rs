//! Draft expression catalogue evidence, authored and intentionally unrun.
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectCandidateDraft, SemanticChange,
    MAX_PROJECT_DRAFT_EXPRESSION_CATALOG_BYTES, PROJECT_DRAFT_EXPRESSION_CATALOG_SCHEMA,
};
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
            "spx-draft-expression-catalog-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(sample.join(path), root.join(path)).unwrap();
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
fn code<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    let errors = result.err().expect("expected rejection");
    assert!(
        errors.iter().any(|error| error.code == expected),
        "{errors:?}"
    );
}
fn body_catalog(draft: &ProjectCandidateDraft, target: &str) -> Value {
    let text = draft
        .expression_catalog(draft.draft_digest(), target)
        .unwrap();
    assert!(text.len() <= MAX_PROJECT_DRAFT_EXPRESSION_CATALOG_BYTES);
    serde_json::from_str(&text).unwrap()
}
fn assert_envelope(
    report: &Value,
    draft: &ProjectCandidateDraft,
    candidate: &ProjectCandidate,
    target: &str,
    region: &str,
) {
    let expected = [
        "schema",
        "draft_revision",
        "last_valid_revision",
        "last_valid_candidate_digest",
        "target",
        "region",
        "source",
        "declared_effect_budget",
        "expressions",
        "limits",
        "materializable",
        "source_authority",
        "validation",
        "evidence_class",
        "selection_admission",
        "nonclaims",
    ]
    .into_iter()
    .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        report
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>(),
        expected
    );
    assert_eq!(report["schema"], PROJECT_DRAFT_EXPRESSION_CATALOG_SCHEMA);
    assert_eq!(report["draft_revision"], draft.draft_digest());
    assert_eq!(
        report["last_valid_revision"],
        candidate.revision().project_revision()
    );
    assert_eq!(
        report["last_valid_candidate_digest"],
        candidate.candidate_digest()
    );
    assert_eq!(report["target"], target);
    assert_eq!(report["region"], region);
    assert_eq!(report["materializable"], false);
    assert_eq!(report["source_authority"], false);
    assert_eq!(report["validation"], "pending_fill_full_source_replay");
    assert_eq!(
        report["selection_admission"],
        "requires_hole_open_validation"
    );
    assert_eq!(
        report["evidence_class"],
        "last_valid_expression_inventory_not_draft_validation"
    );
    assert!(report.get("candidate_revision").is_none());
    assert!(report.get("candidate_digest").is_none());
    let source: Value = serde_json::from_str(&if region == "body" {
        candidate.expression_catalog(target).unwrap()
    } else {
        candidate.contract_expression_catalog(target).unwrap()
    })
    .unwrap();
    let expected_rows = source["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| {
            if region == "body" {
                row["phase"] == "body"
            } else {
                row["phase"] == "requires" || row["phase"] == "ensures"
            }
        })
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(report["expressions"], json!(expected_rows));
    for field in ["source", "declared_effect_budget", "limits"] {
        assert_eq!(report[field], source[field]);
    }
}
fn first_replaceable(report: &Value) -> &str {
    report["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["replaceable"] == true)
        .unwrap()["expression_id"]
        .as_str()
        .unwrap()
}

#[test]
fn partial_fill_discovers_new_current_scope_and_allows_another_hole_without_completion() {
    let fixture = Fixture::new();
    let bytes = fixture.bytes();
    let base = fixture.candidate();
    let draft = ProjectCandidateDraft::open(Arc::clone(&base)).unwrap();
    let draft = draft
        .with_body_hole(draft.draft_digest(), "calculator.add", "first")
        .unwrap();
    let draft = draft
        .with_body_hole(draft.draft_digest(), "calculator.multiply", "pending")
        .unwrap();
    let original_report = body_catalog(&draft, "calculator.add");
    assert_envelope(&original_report, &draft, &base, "calculator.add", "body");
    let replacement = json!({"kind":"let","name":"fresh", "value":{"kind":"binary","op":"+",
        "left":{"kind":"place","name":"left"},"right":{"kind":"place","name":"right"}},
        "body":{"kind":"binary","op":"+","left":{"kind":"place","name":"fresh"},"right":{"kind":"i64","value":1}}});
    let change = SemanticChange::new(
        base.revision().project_revision(),
        &json!({"kind":"replace_function_body","target":"calculator.add","body":replacement}),
    )
    .unwrap();
    let independently_checked = base.apply(base.candidate_digest(), &change).unwrap();
    let filled = draft
        .fill_hole(draft.draft_digest(), "first", &replacement)
        .unwrap();
    let report = body_catalog(&filled, "calculator.add");
    assert_envelope(
        &report,
        &filled,
        &independently_checked,
        "calculator.add",
        "body",
    );
    assert_ne!(
        report["last_valid_revision"],
        original_report["last_valid_revision"]
    );
    let selected = report["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| {
            row["replaceable"] == true
                && row["kind"] == "binary"
                && row["scope"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|binding| binding["name"] == "fresh")
        })
        .expect("new body tail carries the newly introduced lexical scope")["expression_id"]
        .as_str()
        .unwrap();
    let extended = filled
        .with_expression_hole(
            filled.draft_digest(),
            "calculator.add",
            selected,
            "new-current-expression",
        )
        .unwrap();
    let context: Value = serde_json::from_str(
        &extended
            .hole_context(extended.draft_digest(), "new-current-expression")
            .unwrap(),
    )
    .unwrap();
    assert_eq!(context["expression_id"], selected);
    assert_eq!(
        context["last_valid_revision"],
        report["last_valid_revision"]
    );
    code(extended.complete(extended.draft_digest()), "SPX-G232");
    assert_eq!(body_catalog(&draft, "calculator.add"), original_report);
    code(
        filled.expression_catalog(draft.draft_digest(), "calculator.add"),
        "SPX-G232",
    );
    assert_eq!(fixture.bytes(), bytes);
}

#[test]
fn contract_inventory_preserves_actual_rows_and_does_not_bypass_overlap_checks() {
    let fixture = Fixture::new();
    let bytes = fixture.bytes();
    let base = fixture.candidate();
    let draft = ProjectCandidateDraft::open(Arc::clone(&base)).unwrap();
    let text = draft
        .contract_expression_catalog(draft.draft_digest(), "calculator.divide")
        .unwrap();
    let report: Value = serde_json::from_str(&text).unwrap();
    assert_envelope(&report, &draft, &base, "calculator.divide", "contract");
    assert!(!report["expressions"].as_array().unwrap().is_empty());
    let selected = first_replaceable(&report);
    let pending = draft
        .with_contract_expression_hole(
            draft.draft_digest(),
            "calculator.divide",
            selected,
            "contract",
        )
        .unwrap();
    let again: Value = serde_json::from_str(
        &pending
            .contract_expression_catalog(pending.draft_digest(), "calculator.divide")
            .unwrap(),
    )
    .unwrap();
    assert_eq!(again["expressions"], report["expressions"]);
    code(
        pending.with_contract_expression_hole(
            pending.draft_digest(),
            "calculator.divide",
            selected,
            "overlapping",
        ),
        "SPX-G230",
    );
    code(pending.complete(pending.draft_digest()), "SPX-G232");
    let body = pending
        .with_body_hole(pending.draft_digest(), "calculator.multiply", "whole-body")
        .unwrap();
    let body_report = body_catalog(&body, "calculator.multiply");
    code(
        body.with_expression_hole(
            body.draft_digest(),
            "calculator.multiply",
            first_replaceable(&body_report),
            "overlap-body",
        ),
        "SPX-G230",
    );
    assert_eq!(fixture.bytes(), bytes);
}

#[test]
fn draft_authentication_precedes_target_lookup_and_pure_reports_ignore_ambient_source() {
    let fixture = Fixture::new();
    let base = fixture.candidate();
    let draft = ProjectCandidateDraft::open(base).unwrap();
    let sibling = draft
        .with_body_hole(draft.draft_digest(), "calculator.add", "sibling")
        .unwrap();
    code(
        draft.expression_catalog(sibling.draft_digest(), "not.a.function"),
        "SPX-G232",
    );
    code(
        draft.contract_expression_catalog("not-a-digest", "not.a.function"),
        "SPX-G232",
    );
    code(
        draft.expression_catalog(&"x".repeat(72), "not.a.function"),
        "SPX-G231",
    );
    code(
        draft.expression_catalog(draft.draft_digest(), "not.a.function"),
        "SPX-G225",
    );
    let before = draft
        .expression_catalog(draft.draft_digest(), "calculator.add")
        .unwrap();
    std::fs::write(fixture.0.join("src/core.spx"), "not source").unwrap();
    assert_eq!(
        draft
            .expression_catalog(draft.draft_digest(), "calculator.add")
            .unwrap(),
        before
    );
    assert_eq!(
        std::fs::read(fixture.0.join("src/core.spx")).unwrap(),
        b"not source"
    );
}
