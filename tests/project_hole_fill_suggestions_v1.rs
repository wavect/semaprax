//! Compiler-replayed bounded fill suggestions: authored regressions, unrun.
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{with_authenticated_project, ProjectCandidate, ProjectCandidateDraft};
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
            "spx-fill-suggestions-{}-{}",
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
    fn append(&self, extra: &str) {
        let path = self.0.join("src/core.spx");
        let text = format!("{}\n{extra}\n", std::fs::read_to_string(&path).unwrap());
        let program = semaprax::parse(&text, "src/core.spx").unwrap();
        std::fs::write(path, semaprax::format::canonical(&program)).unwrap();
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
fn select(base: &ProjectCandidate, target: &str, contract: bool, snippet: &str) -> String {
    let catalog: Value = serde_json::from_str(
        &if contract {
            base.contract_expression_catalog(target)
        } else {
            base.expression_catalog(target)
        }
        .unwrap(),
    )
    .unwrap();
    let source = base
        .revision()
        .sources()
        .iter()
        .find(|s| s.path() == "src/core.spx")
        .unwrap()
        .source();
    let rows = catalog["expressions"]
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
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 1, "{target}: {snippet}");
    rows[0]["expression_id"].as_str().unwrap().to_owned()
}
fn code<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    let errors = result.err().expect("expected rejection");
    assert!(
        errors.iter().any(|error| error.code == expected),
        "{errors:?}"
    );
}
fn checked_report(draft: &ProjectCandidateDraft, hole: &str) -> Value {
    let before = draft.to_json().to_owned();
    let recovery = draft.recovery_capsule().unwrap();
    let full = draft.hole_context(draft.draft_digest(), hole).unwrap();
    let context: Value = serde_json::from_str(&full).unwrap();
    let summary: Value =
        serde_json::from_str(&draft.hole_summary(draft.draft_digest(), hole).unwrap()).unwrap();
    let text = draft
        .hole_fill_suggestions(draft.draft_digest(), hole)
        .unwrap();
    assert!(text.len() <= 65536);
    assert_eq!(
        draft
            .hole_fill_suggestions(draft.draft_digest(), hole)
            .unwrap(),
        text
    );
    let report: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        report["schema"],
        "semaprax.project-hole-fill-suggestions.v1"
    );
    assert_eq!(report.as_object().unwrap().len(), 15);
    assert_eq!(report["draft_revision"], draft.draft_digest());
    assert_eq!(report["hole_id"], hole);
    assert_eq!(report["context_revision"], summary["context_revision"]);
    assert_eq!(
        report["last_valid_revision"],
        context["last_valid_revision"]
    );
    assert_eq!(report["expected_type_id"], context["expected_type_id"]);
    assert_eq!(report["validation"], "ordinary_fill_source_replay");
    assert_eq!(report["tests"], "not_run");
    assert_eq!(report["source_authority"], false);
    assert_eq!(report["draft_retained"], false);
    assert_eq!(
        report["nonclaims"],
        json!([
            "not_intent_correctness",
            "not_runtime_contract_proof",
            "not_complete_expression_search",
            "not_liveness_inference"
        ])
    );
    let considered = report["considered"].as_u64().unwrap();
    let rejected = report["rejected"].as_u64().unwrap();
    assert!(considered <= 32);
    let suggestions = report["suggestions"].as_array().unwrap();
    assert_eq!(considered, rejected + suggestions.len() as u64);
    for row in suggestions {
        assert_eq!(row.as_object().unwrap().len(), 2);
        let expression = &row["expression"];
        match expression["kind"].as_str().unwrap() {
            "place" => assert_eq!(expression.as_object().unwrap().len(), 2),
            "call" => {
                assert_eq!(expression.as_object().unwrap().len(), 3);
                assert_ne!(expression["target"], context["target"]);
                let callee = context["accessible_calls"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|call| call["id"] == expression["target"])
                    .unwrap();
                assert_eq!(callee["within_effect_budget"], true);
                assert_eq!(callee["return_type_id"], context["expected_type_id"]);
                for argument in expression["arguments"].as_array().unwrap() {
                    assert_eq!(argument["kind"], "place");
                    assert_eq!(argument.as_object().unwrap().len(), 2);
                }
            }
            other => panic!("enumeration must not invent literals/builtins/nesting: {other}"),
        }
        let preview = draft
            .fill_hole(draft.draft_digest(), hole, expression)
            .unwrap();
        assert_eq!(
            preview.draft_digest(),
            row["preview_draft_revision"].as_str().unwrap()
        );
        assert_ne!(preview.draft_digest(), draft.draft_digest());
    }
    assert_eq!(draft.to_json(), before);
    assert_eq!(draft.recovery_capsule().unwrap(), recovery);
    assert_eq!(
        draft.hole_context(draft.draft_digest(), hole).unwrap(),
        full
    );
    report
}

#[test]
fn mixed_holes_replay_every_suggestion_without_filling_the_parent_or_proving_contracts() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let expression = select(&base, "calculator.is-negative", false, "value");
    let contract = select(&base, "calculator.divide", true, "right != 0");
    let draft = ProjectCandidateDraft::open(base).unwrap();
    let draft = draft
        .with_body_hole(draft.draft_digest(), "calculator.add", "body")
        .unwrap();
    let draft = draft
        .with_expression_hole(
            draft.draft_digest(),
            "calculator.is-negative",
            &expression,
            "expression",
        )
        .unwrap();
    let draft = draft
        .with_contract_expression_hole(
            draft.draft_digest(),
            "calculator.divide",
            &contract,
            "contract",
        )
        .unwrap();
    for hole in ["body", "expression", "contract"] {
        let report = checked_report(&draft, hole);
        assert!(
            !report["suggestions"].as_array().unwrap().is_empty(),
            "{hole}: {report}"
        );
        code(draft.complete(draft.draft_digest()), "SPX-G232");
    }
    let report = checked_report(&draft, "body");
    assert_eq!(
        report["suggestions"][0]["expression"],
        json!({"kind":"place","name":"left"})
    );
    assert_eq!(
        report["suggestions"][1]["expression"],
        json!({"kind":"place","name":"right"})
    );
    let partial = draft
        .fill_hole(
            draft.draft_digest(),
            "body",
            &report["suggestions"][0]["expression"],
        )
        .unwrap();
    code(
        partial.hole_fill_suggestions(draft.draft_digest(), "expression"),
        "SPX-G232",
    );
    code(
        draft.hole_fill_suggestions(draft.draft_digest(), "missing"),
        "SPX-G230",
    );
    let refreshed = checked_report(&partial, "expression");
    assert_ne!(refreshed["draft_revision"], report["draft_revision"]);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn finite_enumeration_exhaustion_is_distinct_from_global_attempt_limit_and_no_literal_is_invented()
{
    let fixture = Fixture::new();
    let params = (0..35)
        .map(|i| format!("p{i}:i64"))
        .collect::<Vec<_>>()
        .join(",");
    fixture.append(&format!("@id(\"calculator.wide\") fn wide({params})->i64 {{p0}}\n@id(\"calculator.nothing\") fn nothing()->bool {{true}}"));
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let empty = ProjectCandidateDraft::open(Arc::clone(&base)).unwrap();
    let wide = empty
        .with_body_hole(empty.draft_digest(), "calculator.wide", "wide")
        .unwrap();
    let report = checked_report(&wide, "wide");
    assert_eq!(report["considered"], 32);
    assert_eq!(report["rejected"], 0);
    assert_eq!(report["search_exhausted"], false);
    assert_eq!(report["suggestions"].as_array().unwrap().len(), 32);
    for (index, row) in report["suggestions"].as_array().unwrap().iter().enumerate() {
        assert_eq!(
            row["expression"],
            json!({"kind":"place","name":format!("p{index}")})
        );
    }
    let nothing = empty
        .with_body_hole(empty.draft_digest(), "calculator.nothing", "nothing")
        .unwrap();
    let report = checked_report(&nothing, "nothing");
    assert_eq!(report["considered"], 0);
    assert_eq!(report["suggestions"], json!([]));
    assert_eq!(report["search_exhausted"], true);
    // The original literal true would fill this hole, but literals lie outside
    // this deliberately finite grammar. Exhaustion is not an impossibility proof.
    nothing
        .fill_hole(
            nothing.draft_digest(),
            "nothing",
            &json!({"kind":"bool","value":true}),
        )
        .unwrap();
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn repeated_owned_arguments_are_rejected_by_real_fill_admission_not_inferred_liveness() {
    let fixture = Fixture::new();
    std::fs::write(fixture.0.join("semaprax.toml"),r#"schema = "semaprax.project.v8"
name = "fill-owned"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "calculator.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["calculator.add", "calculator.divide", "calculator.is-negative", "calculator.multiply", "calculator.not", "calculator.subtract"]
tests = ["calculator.tests"]
"#).unwrap();
    fixture.append("@id(\"calculator.owned-target\") fn owned_target(input:own Bytes)->Bytes {input}\n@id(\"calculator.owned-pair\") fn owned_pair(left:own Bytes,right:own Bytes)->Bytes {left}");
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let empty = ProjectCandidateDraft::open(base).unwrap();
    let draft = empty
        .with_body_hole(empty.draft_digest(), "calculator.owned-target", "owned")
        .unwrap();
    let duplicate = json!({"kind":"call","target":"calculator.owned-pair","arguments":[{"kind":"place","name":"input"},{"kind":"place","name":"input"}]});
    assert!(draft
        .fill_hole(draft.draft_digest(), "owned", &duplicate)
        .is_err());
    let report = checked_report(&draft, "owned");
    assert_eq!(report["search_exhausted"], true);
    assert!(report["rejected"].as_u64().unwrap() >= 1);
    assert!(report["suggestions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["expression"] == json!({"kind":"place","name":"input"})));
    assert!(!report["suggestions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["expression"] == duplicate));
    code(draft.complete(draft.draft_digest()), "SPX-G232");
    assert_eq!(fixture.bytes(), disk);
}
