//! Cross-module callable-contract reconciliation.
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectRevision, SemanticChange,
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
            "spx-package-dependency-rebase-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for file in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(sample.join(file), root.join(file)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }

    fn revision(&self) -> Arc<ProjectRevision> {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap()
    }

    fn candidate(&self) -> ProjectCandidate {
        let revision = self.revision();
        ProjectCandidate::open(Arc::clone(&revision), revision.project_revision()).unwrap()
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

/// `calculator.app.main` is a main function, so the candidate catalogue exposes
/// expression replacement rather than whole-body replacement. Select the exact
/// authenticated imported call and rebind its arguments.
fn imported_call(candidate: &ProjectCandidate) -> Value {
    let catalog: Value =
        serde_json::from_str(&candidate.expression_catalog("calculator.app.main").unwrap())
            .unwrap();
    let caller = source(candidate, "src/app.spx");
    let selected = catalog["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| {
            let span = &entry["source_span"];
            caller.get(
                span["start"].as_u64().unwrap() as usize..span["end"].as_u64().unwrap() as usize,
            ) == Some("divide(4, 2)")
        })
        .expect("the authenticated imported divide call");
    json!({
        "kind":"replace_expression",
        "target":"calculator.app.main",
        "expression_id":selected["expression_id"],
        "replacement":{
            "kind":"call",
            "target":"calculator.divide",
            "arguments":[
                {"kind":"i64","value":84},
                {"kind":"i64","value":2}
            ]
        }
    })
}

fn added_provider_contract() -> Value {
    json!({
        "kind":"add_contract",
        "target":"calculator.divide",
        "phase":"ensures",
        "predicate":{"kind":"bool","value":true}
    })
}

fn provider_body(value: i64) -> Value {
    json!({
        "kind":"replace_function_body",
        "target":"calculator.divide",
        "body":{"kind":"i64","value":value}
    })
}

fn source<'a>(candidate: &'a ProjectCandidate, path: &str) -> &'a str {
    candidate
        .revision()
        .sources()
        .iter()
        .find(|source| source.path() == path)
        .unwrap()
        .source()
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
fn imported_provider_contract_drift_conflicts_before_rebase_or_merge() {
    let fixture = Fixture::new();
    let root = fixture.candidate();
    let caller = apply(&root, imported_call(&root));
    let provider = apply(&root, added_provider_contract());
    let caller_bytes = caller.to_json().to_owned();
    let provider_bytes = provider.to_json().to_owned();

    code(
        caller.rebase(
            caller.candidate_digest(),
            Arc::clone(provider.revision()),
            provider.revision().project_revision(),
        ),
        "SPX-G235",
    );
    code(
        caller.merge(
            caller.candidate_digest(),
            &provider,
            provider.candidate_digest(),
        ),
        "SPX-G235",
    );

    assert_eq!(caller.to_json(), caller_bytes);
    assert_eq!(provider.to_json(), provider_bytes);
    assert!(!source(&caller, "src/core.spx").contains("ensures true"));
    assert!(!source(&provider, "src/app.spx").contains("divide(84, 2)"));
}

#[test]
fn imported_provider_body_only_change_merges_through_complete_canonical_replay() {
    let fixture = Fixture::new();
    let root = fixture.candidate();
    let caller = apply(&root, imported_call(&root));
    let provider = apply(&root, provider_body(21));

    let merged = caller
        .merge(
            caller.candidate_digest(),
            &provider,
            provider.candidate_digest(),
        )
        .unwrap();
    assert!(source(merged.candidate(), "src/app.spx").contains("divide(84, 2)"));
    assert!(source(merged.candidate(), "src/core.spx").contains("    21\n"));
    let report: Value = serde_json::from_str(merged.to_json()).unwrap();
    assert_eq!(report["validation"], "complete_candidate_source_replay");
    assert_eq!(report["source_authority"], false);

    let replayed = ProjectCandidate::restore(
        Arc::clone(merged.candidate().base_revision()),
        merged.candidate().base_revision().project_revision(),
        merged.candidate().recovery_capsule().unwrap().as_bytes(),
    )
    .unwrap();
    assert_eq!(
        replayed.revision().project_revision(),
        merged.candidate().revision().project_revision()
    );
    assert_eq!(replayed.to_json(), merged.candidate().to_json());
    assert_eq!(
        replayed.revision().semantic_graph(),
        merged.candidate().revision().semantic_graph()
    );
    for retained in merged.candidate().revision().sources() {
        let parsed = semaprax::parse(retained.source(), retained.path()).unwrap();
        assert_eq!(
            semaprax::format::canonical(&parsed),
            retained.source(),
            "{} was not canonical",
            retained.path()
        );
    }
}
