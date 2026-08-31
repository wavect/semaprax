//! Candidate-bound semantic-impact navigation; authored and intentionally unrun.
use semaprax::project::{
    with_authenticated_project, CandidateImpactPageOptions, CandidateImpactView, ProjectCandidate,
    ProjectRevision, SemanticChange, MAX_PROJECT_CANDIDATE_IMPACT_PAGE_BYTES,
    MAX_PROJECT_CANDIDATE_IMPACT_SUMMARY_BYTES, PROJECT_CANDIDATE_IMPACT_ITEM_SCHEMA,
    PROJECT_CANDIDATE_IMPACT_PAGE_SCHEMA, PROJECT_CANDIDATE_IMPACT_SUMMARY_SCHEMA,
};
use semaprax::workspace_analysis::{WorkspaceAnalysisTargetKind, WorkspaceImpactOptions};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const TARGET: &str = "calculator.add";
const FILES: [&str; 4] = [
    "semaprax.toml",
    "src/app.spx",
    "src/core.spx",
    "src/tests.spx",
];

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-candidate-impact-navigation-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in FILES {
            std::fs::copy(source.join(path), root.join(path)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }

    fn revision(&self) -> Arc<ProjectRevision> {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap()
    }

    fn bytes(&self) -> Vec<Vec<u8>> {
        FILES
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

fn open(revision: &Arc<ProjectRevision>) -> ProjectCandidate {
    ProjectCandidate::open(Arc::clone(revision), revision.project_revision()).unwrap()
}

fn introduce(candidate: &ProjectCandidate, id: &str) -> ProjectCandidate {
    let intent = json!({"kind":"add_declaration","target":TARGET,"declaration":{
        "id":id,"name":"generated_caller",
        "parameters":[{"name":"left","type":"i64","mode":"value"},{"name":"right","type":"i64","mode":"value"}],
        "return_type":"i64","effects":[],"requires":[],"ensures":[],
        "body":{"kind":"call","target":TARGET,"arguments":[
            {"kind":"place","name":"left"},{"kind":"place","name":"right"}
        ]}
    }});
    let change = SemanticChange::new(candidate.revision().project_revision(), &intent).unwrap();
    candidate
        .apply(candidate.candidate_digest(), &change)
        .unwrap()
}

fn artifact(candidate: &ProjectCandidate, options: WorkspaceImpactOptions) -> Value {
    serde_json::from_str(
        &candidate
            .revision()
            .semantic_impact(WorkspaceAnalysisTargetKind::Declaration, TARGET, options)
            .unwrap(),
    )
    .unwrap()
}

fn summary(candidate: &ProjectCandidate, options: WorkspaceImpactOptions) -> (String, Value) {
    let text = candidate
        .impact_summary(candidate.candidate_digest(), TARGET, options)
        .unwrap();
    assert!(text.len() <= MAX_PROJECT_CANDIDATE_IMPACT_SUMMARY_BYTES);
    assert!(!text.ends_with('\n'));
    let value = serde_json::from_str(&text).unwrap();
    (text, value)
}

fn facet<'a>(summary: &'a Value, view: CandidateImpactView) -> &'a Value {
    let matches = summary["facets"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["view"] == view.name())
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1);
    matches[0]
}

fn owner_items(items: Vec<Value>) -> Vec<Value> {
    items
        .into_iter()
        .map(|item| {
            let mut envelope = item.as_object().unwrap().clone();
            assert_eq!(
                envelope.remove("schema"),
                Some(json!(PROJECT_CANDIDATE_IMPACT_ITEM_SCHEMA))
            );
            assert_eq!(envelope.len(), 1);
            envelope.remove("value").unwrap()
        })
        .collect()
}

fn pages(
    candidate: &ProjectCandidate,
    impact_options: WorkspaceImpactOptions,
    summary: &Value,
    view: CandidateImpactView,
    page_size: usize,
) -> Vec<Value> {
    let expected_handle = facet(summary, view)["handle"].as_str().unwrap();
    let total = facet(summary, view)["total_items"].as_u64().unwrap() as usize;
    let page_options = CandidateImpactPageOptions::new(page_size, 65_536).unwrap();
    let mut cursor: Option<String> = None;
    let mut items = Vec::new();
    loop {
        let text = candidate
            .impact_page(
                candidate.candidate_digest(),
                TARGET,
                impact_options,
                view,
                expected_handle,
                cursor.as_deref(),
                page_options,
            )
            .unwrap();
        assert!(text.len() <= page_options.max_bytes());
        assert!(!text.ends_with('\n'));
        let page: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(page["schema"], PROJECT_CANDIDATE_IMPACT_PAGE_SCHEMA);
        assert_eq!(page["candidate_revision"], candidate.candidate_digest());
        assert_eq!(
            page["base_project_revision"],
            candidate.base_revision().project_revision()
        );
        assert_eq!(page.as_object().unwrap().len(), 27);
        assert_eq!(page["project_schema"], "semaprax.project.v1");
        assert_eq!(page["project"], "calculator");
        assert_eq!(
            page["project_revision"],
            candidate.revision().project_revision()
        );
        assert_eq!(
            page["workspace_revision"],
            candidate.revision().workspace_revision()
        );
        assert_eq!(
            page["project_graph_digest"],
            candidate.revision().semantic_graph_digest()
        );
        assert_eq!(page["target"]["id"], TARGET);
        assert_eq!(page["view"], view.name());
        assert_eq!(page["handle"], expected_handle);
        assert_eq!(page["cursor"], json!(cursor));
        assert_eq!(page["offset"], items.len());
        assert_eq!(page["total_items"], total);
        assert_eq!(page["page_size"], page_size);
        assert_eq!(page["max_bytes"], page_options.max_bytes());
        for field in [
            "source_authority",
            "execution",
            "publication_authority",
            "candidate_retained",
        ] {
            assert_eq!(page[field], false);
        }
        assert_eq!(page["nonclaims"].as_array().unwrap().len(), 7);
        assert!(page["nonclaims"].as_array().unwrap().contains(&json!(
            "bounded_or_truncated_inventory_is_not_complete_impact"
        )));
        let rows = page["items"].as_array().unwrap();
        assert!(rows.len() <= page_size);
        items.extend(rows.iter().cloned());
        cursor = page["next_cursor"].as_str().map(str::to_owned);
        if cursor.is_none() {
            break;
        }
        assert!(!rows.is_empty(), "continuation must advance");
    }
    assert_eq!(items.len(), total);
    items
}

fn failed<T>(result: Result<T, Vec<semaprax::diagnostic::Diagnostic>>, code: &str) {
    let diagnostics = result.err().expect("invalid impact navigation accepted");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.code == code),
        "{diagnostics:?}"
    );
}

#[test]
fn summary_and_pages_preserve_the_exact_candidate_impact_artifact_and_compiler_order() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let base = open(&revision);
    let candidate = introduce(&base, "impact.generated");
    let base_json = base.to_json().to_owned();
    let candidate_json = candidate.to_json().to_owned();
    let options = WorkspaceImpactOptions::default();
    let underlying = artifact(&candidate, options);
    let (_, compact) = summary(&candidate, options);

    assert_eq!(compact["schema"], PROJECT_CANDIDATE_IMPACT_SUMMARY_SCHEMA);
    assert_eq!(compact.as_object().unwrap().len(), 19);
    assert_eq!(compact["candidate_revision"], candidate.candidate_digest());
    assert_eq!(
        compact["base_project_revision"],
        revision.project_revision()
    );
    assert_eq!(compact["project_schema"], "semaprax.project.v1");
    assert_eq!(compact["project"], "calculator");
    assert_eq!(compact["project_schema"], underlying["project_schema"]);
    assert_eq!(compact["project"], underlying["project"]);
    assert_eq!(
        compact["project_revision"],
        candidate.revision().project_revision()
    );
    assert_eq!(
        compact["workspace_revision"],
        candidate.revision().workspace_revision()
    );
    assert_eq!(
        compact["project_graph_digest"],
        candidate.revision().semantic_graph_digest()
    );
    assert_eq!(compact["target"], underlying["target"]);
    assert_eq!(compact["artifact_digest"], underlying["artifact_digest"]);
    assert_eq!(compact["query"], underlying["query"]);
    assert_eq!(compact["truncation"], underlying["truncation"]);
    assert_eq!(compact["budget"], underlying["budget"]);
    assert_eq!(compact["facets"].as_array().unwrap().len(), 3);
    for view in CandidateImpactView::ALL {
        let expected = underlying[view.name()].as_array().unwrap();
        assert_eq!(facet(&compact, view)["total_items"], expected.len());
        let actual = owner_items(pages(&candidate, options, &compact, view, 1));
        assert_eq!(actual, *expected, "{} order changed", view.name());
    }
    assert!(underlying["affected"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["id"] == "impact.generated"));
    for field in [
        "source_authority",
        "execution",
        "publication_authority",
        "candidate_retained",
    ] {
        assert_eq!(compact[field], false);
    }
    assert!(compact["nonclaims"].as_array().unwrap().contains(&json!(
        "not_a_candidate_semantic_delta_or_behavioral_change"
    )));
    assert_eq!(compact["nonclaims"].as_array().unwrap().len(), 7);
    assert!(compact["nonclaims"].as_array().unwrap().contains(&json!(
        "not_runtime_liveness_test_coverage_or_external_consumer_compatibility"
    )));
    assert!(compact["nonclaims"].as_array().unwrap().contains(&json!(
        "bounded_or_truncated_inventory_is_not_complete_impact"
    )));
    assert_eq!(base.to_json(), base_json);
    assert_eq!(candidate.to_json(), candidate_json);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn underlying_truncation_is_preserved_and_navigation_never_claims_omitted_impact() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let candidate = introduce(&open(&revision), "impact.truncated");
    let options = WorkspaceImpactOptions::new(0, 65_536, 128).unwrap();
    let underlying = artifact(&candidate, options);
    assert_eq!(underlying["truncation"]["truncated"], true);
    assert!(!underlying["frontier"].as_array().unwrap().is_empty());
    let (_, compact) = summary(&candidate, options);
    assert_eq!(compact["query"], underlying["query"]);
    assert_eq!(compact["truncation"], underlying["truncation"]);
    assert_eq!(compact["budget"], underlying["budget"]);
    for view in CandidateImpactView::ALL {
        assert_eq!(
            owner_items(pages(&candidate, options, &compact, view, 2)),
            *underlying[view.name()].as_array().unwrap()
        );
    }
    assert!(compact["nonclaims"].as_array().unwrap().contains(&json!(
        "potential_reverse_dependencies_over_the_existing_six_edge_families_only"
    )));
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn candidate_target_query_view_handles_cursors_and_options_fail_closed() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let base = open(&revision);
    let left = introduce(&base, "impact.left");
    let right = introduce(&base, "impact.right");
    let left_json = left.to_json().to_owned();
    let right_json = right.to_json().to_owned();
    let options = WorkspaceImpactOptions::default();
    let (_, left_summary) = summary(&left, options);
    let (_, right_summary) = summary(&right, options);
    let view = CandidateImpactView::Affected;
    let left_handle = facet(&left_summary, view)["handle"].as_str().unwrap();
    let right_handle = facet(&right_summary, view)["handle"].as_str().unwrap();
    let page_options = CandidateImpactPageOptions::new(1, 65_536).unwrap();
    let first: Value = serde_json::from_str(
        &left
            .impact_page(
                left.candidate_digest(),
                TARGET,
                options,
                view,
                left_handle,
                None,
                page_options,
            )
            .unwrap(),
    )
    .unwrap();
    let cursor = first["next_cursor"]
        .as_str()
        .expect("calculator impact must have more than one affected row");

    failed(
        left.impact_summary(right.candidate_digest(), TARGET, options),
        "SPX-G224",
    );
    failed(
        left.impact_summary("not-a-digest", TARGET, options),
        "SPX-G222",
    );
    failed(
        left.impact_page(
            left.candidate_digest(),
            TARGET,
            options,
            view,
            right_handle,
            None,
            page_options,
        ),
        "SPX-G335",
    );
    failed(
        right.impact_page(
            right.candidate_digest(),
            TARGET,
            options,
            view,
            left_handle,
            None,
            page_options,
        ),
        "SPX-G335",
    );
    failed(
        left.impact_page(
            left.candidate_digest(),
            TARGET,
            options,
            CandidateImpactView::DependencyEdges,
            left_handle,
            None,
            page_options,
        ),
        "SPX-G335",
    );
    for malformed in ["", "0", "01:sha256:bad", "-1:sha256:bad"] {
        failed(
            left.impact_page(
                left.candidate_digest(),
                TARGET,
                options,
                view,
                left_handle,
                Some(malformed),
                page_options,
            ),
            "SPX-G335",
        );
    }
    failed(
        left.impact_page(
            left.candidate_digest(),
            TARGET,
            options,
            view,
            left_handle,
            Some(cursor),
            CandidateImpactPageOptions::new(2, 65_536).unwrap(),
        ),
        "SPX-G335",
    );
    let different_query = WorkspaceImpactOptions::new(0, 65_536, 128).unwrap();
    failed(
        left.impact_page(
            left.candidate_digest(),
            TARGET,
            different_query,
            view,
            left_handle,
            None,
            page_options,
        ),
        "SPX-G335",
    );
    failed(CandidateImpactView::parse("callers"), "SPX-G333");
    for (size, bytes) in [(0, 65_536), (129, 65_536), (1, 1023), (1, 1_048_577)] {
        failed(CandidateImpactPageOptions::new(size, bytes), "SPX-G333");
    }
    assert_eq!(MAX_PROJECT_CANDIDATE_IMPACT_PAGE_BYTES, 1_048_576);
    assert_eq!(left.to_json(), left_json);
    assert_eq!(right.to_json(), right_json);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn sibling_histories_have_distinct_artifacts_and_repeated_reads_retain_nothing() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let base = open(&revision);
    let left = introduce(&base, "impact.sibling-left");
    let right = introduce(&base, "impact.sibling-right");
    let base_json = base.to_json().to_owned();
    let left_json = left.to_json().to_owned();
    let right_json = right.to_json().to_owned();
    let options = WorkspaceImpactOptions::default();
    let (first, left_summary) = summary(&left, options);
    let (again, _) = summary(&left, options);
    let (_, right_summary) = summary(&right, options);

    assert_eq!(first, again);
    assert_ne!(left.candidate_digest(), right.candidate_digest());
    assert_ne!(
        left_summary["artifact_digest"],
        right_summary["artifact_digest"]
    );
    for view in CandidateImpactView::ALL {
        assert_ne!(
            facet(&left_summary, view)["handle"],
            facet(&right_summary, view)["handle"]
        );
    }
    assert_eq!(base.to_json(), base_json);
    assert_eq!(left.to_json(), left_json);
    assert_eq!(right.to_json(), right_json);
    assert_eq!(fixture.bytes(), disk);
}
