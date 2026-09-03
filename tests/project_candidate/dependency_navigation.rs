//! Candidate-bound compact dependency navigation; authored and intentionally unrun.
use semaprax::project::{
    with_authenticated_project, ImageDependencyPageOptions, ImageDependencyView, ProjectCandidate,
    ProjectRevision, ProjectSemanticImage, SemanticChange,
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
            "spx-candidate-dependency-navigation-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let root = root.canonicalize().unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "candidate-dependency-navigation"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "deps.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["deps.public"]
tests = ["deps.tests"]
"#,
        )
        .unwrap();
        for (path, text) in [
            (
                "src/core.spx",
                r#"module deps.core;
@id("deps.pair") record Pair { @id("deps.pair.x") x: i64, @id("deps.pair.y") y: i64, }
@id("deps.choose") fn choose(value: i64) -> i64 { value }
@id("deps.evaluate") fn evaluate(value: i64) -> i64
    requires (Pair { x: 0, y: 0 }).x == 0
{
    let mut pair = Pair { x: value, y: 0 };
    pair.x = pair.x + 1;
    choose(pair.x)
}
@id("deps.public") fn public_value(value: i64) -> i64 { value }
"#,
            ),
            (
                "src/app.spx",
                r#"module deps.app;
use type @id("deps.pair") from deps.core as Metric;
use function @id("deps.evaluate") from deps.core as evaluate;
@id("deps.main") fn main() -> i64 {
    let item = Metric { x: 41, y: 0 };
    evaluate(item.x)
}
"#,
            ),
            (
                "src/tests.spx",
                r#"module deps.tests;
use function @id("deps.evaluate") from deps.core as evaluate;
@id("deps.test") fn main() -> i64 { if evaluate(41) == 42 { 0 } else { 1 } }
"#,
            ),
        ] {
            let parsed = semaprax::parse(text, path).unwrap();
            std::fs::write(root.join(path), semaprax::format::canonical(&parsed)).unwrap();
        }
        Self(root)
    }
    fn revision(&self) -> Arc<ProjectRevision> {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
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

fn open(revision: &Arc<ProjectRevision>) -> ProjectCandidate {
    ProjectCandidate::open(Arc::clone(revision), revision.project_revision()).unwrap()
}
fn apply(candidate: &ProjectCandidate, intent: Value) -> ProjectCandidate {
    let change = SemanticChange::new(candidate.revision().project_revision(), &intent).unwrap();
    candidate
        .apply(candidate.candidate_digest(), &change)
        .unwrap()
}
fn introduced(candidate: &ProjectCandidate, id: &str) -> ProjectCandidate {
    apply(
        candidate,
        json!({"kind":"add_declaration","target":"deps.public","declaration":{
            "id":id,"name":"generated","parameters":[{"name":"value","type":"i64","mode":"value"}],
            "return_type":"i64","effects":[],"requires":[],"ensures":[],
            "body":{"kind":"call","target":"deps.choose","arguments":[{"kind":"place","name":"value"}]}
        }}),
    )
}
fn summary(candidate: &ProjectCandidate, target: &str) -> Value {
    serde_json::from_str(
        &candidate
            .dependency_summary(candidate.candidate_digest(), target)
            .unwrap(),
    )
    .unwrap()
}
fn facet(summary: &Value, view: ImageDependencyView) -> &Value {
    summary["facets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|facet| facet["view"] == view.name())
        .unwrap()
}
fn candidate_pages(
    candidate: &ProjectCandidate,
    target: &str,
    view: ImageDependencyView,
    size: usize,
    max_bytes: usize,
) -> Vec<Value> {
    let summary = summary(candidate, target);
    let handle = facet(&summary, view)["handle"].as_str().unwrap();
    let total = facet(&summary, view)["total_items"].as_u64().unwrap() as usize;
    let options = ImageDependencyPageOptions::new(size, max_bytes).unwrap();
    let mut cursor: Option<String> = None;
    let mut items = Vec::new();
    loop {
        let text = candidate
            .dependency_page(
                candidate.candidate_digest(),
                target,
                view,
                handle,
                cursor.as_deref(),
                options,
            )
            .unwrap();
        assert!(text.len() <= max_bytes);
        let page: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            page["schema"],
            "semaprax.project-candidate-dependency-page.v1"
        );
        assert_eq!(page["candidate_revision"], candidate.candidate_digest());
        assert_eq!(
            page["base_project_revision"],
            candidate.base_revision().project_revision()
        );
        assert_eq!(
            page["project_revision"],
            candidate.revision().project_revision()
        );
        assert_eq!(
            page["workspace_revision"],
            candidate.revision().workspace_revision()
        );
        assert_eq!(page["target"], target);
        assert_eq!(page["view"], view.name());
        assert_eq!(page["handle"], handle);
        assert_eq!(page["cursor"], json!(cursor));
        assert_eq!(page["offset"], items.len());
        assert_eq!(page["total_items"], total);
        assert_eq!(page["page_size"], size);
        assert_eq!(page["max_bytes"], max_bytes);
        assert_eq!(page["source_authority"], false);
        assert_eq!(page["candidate_retained"], false);
        assert_eq!(page["execution"], false);
        assert_eq!(page["publication_authority"], false);
        let rows = page["items"].as_array().unwrap();
        assert!(rows.len() <= size);
        items.extend(rows.iter().cloned());
        cursor = page["next_cursor"].as_str().map(str::to_owned);
        if cursor.is_none() {
            break;
        }
        assert!(!rows.is_empty(), "continuation must make progress");
    }
    assert_eq!(items.len(), total);
    items
}
fn image_pages(
    candidate: &ProjectCandidate,
    target: &str,
    view: ImageDependencyView,
    size: usize,
) -> Vec<Value> {
    let image = ProjectSemanticImage::derive(
        Arc::clone(candidate.revision()),
        candidate.revision().project_revision(),
    )
    .unwrap();
    let summary: Value = serde_json::from_str(
        &image
            .dependency_summary(image.image_digest(), target)
            .unwrap(),
    )
    .unwrap();
    let handle = facet(&summary, view)["handle"].as_str().unwrap();
    let options = ImageDependencyPageOptions::new(size, 65_536).unwrap();
    let mut cursor: Option<String> = None;
    let mut items = Vec::new();
    loop {
        let page: Value = serde_json::from_str(
            &image
                .dependency_page(
                    image.image_digest(),
                    target,
                    view,
                    handle,
                    cursor.as_deref(),
                    options,
                )
                .unwrap(),
        )
        .unwrap();
        items.extend(page["items"].as_array().unwrap().iter().cloned());
        cursor = page["next_cursor"].as_str().map(str::to_owned);
        if cursor.is_none() {
            return items;
        }
    }
}
fn failed<T>(result: Result<T, Vec<semaprax::diagnostic::Diagnostic>>, code: &str) {
    let diagnostics = result.err().expect("invalid candidate navigation accepted");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.code == code),
        "{diagnostics:?}"
    );
}

#[test]
fn changed_and_introduced_declarations_expose_exact_candidate_bound_views() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let base = open(&revision);
    let added = introduced(&base, "deps.generated");
    let candidate = apply(
        &added,
        json!({"kind":"rename_declaration","target":"deps.choose","name":"pick"}),
    );
    let candidate_image = ProjectSemanticImage::derive(
        Arc::clone(candidate.revision()),
        candidate.revision().project_revision(),
    )
    .unwrap();

    let changed = summary(&candidate, "deps.choose");
    assert_eq!(
        changed["schema"],
        "semaprax.project-candidate-dependency-summary.v1"
    );
    assert_eq!(changed["candidate_revision"], candidate.candidate_digest());
    assert_eq!(changed["image_digest"], candidate_image.image_digest());
    assert_eq!(
        changed["base_project_revision"],
        revision.project_revision()
    );
    assert_eq!(
        changed["project_revision"],
        candidate.revision().project_revision()
    );
    assert_eq!(changed["name"], "pick");
    assert_eq!(changed["source_authority"], false);
    assert_eq!(changed["candidate_retained"], false);
    assert_eq!(changed["execution"], false);
    assert_eq!(changed["publication_authority"], false);
    assert_eq!(changed["facets"].as_array().unwrap().len(), 4);
    let base_summary = summary(&base, "deps.choose");
    assert_eq!(base_summary["name"], "choose");
    assert_ne!(
        changed["source_binding"]["source_revision"],
        base_summary["source_binding"]["source_revision"]
    );

    let introduced = summary(&candidate, "deps.generated");
    assert_eq!(introduced["name"], "generated");
    assert_eq!(introduced["kind"], "function");
    assert_eq!(
        introduced["candidate_revision"],
        candidate.candidate_digest()
    );
    assert!(candidate_pages(
        &candidate,
        "deps.choose",
        ImageDependencyView::Calls,
        1,
        65_536
    )
    .iter()
    .any(|row| row["function_id"] == "deps.generated" && row["callee_id"] == "deps.choose"));
    assert!(candidate_pages(
        &candidate,
        "deps.choose",
        ImageDependencyView::Callers,
        1,
        65_536
    )
    .iter()
    .any(|row| row["id"] == "deps.generated"));
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn all_four_pages_equal_the_independently_derived_candidate_image_inventory() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let candidate = introduced(&open(&revision), "deps.generated");
    for view in ImageDependencyView::ALL {
        assert_eq!(
            candidate_pages(&candidate, "deps.pair", view, 1, 65_536),
            image_pages(&candidate, "deps.pair", view, 1),
            "{} view diverged from the candidate image",
            view.name()
        );
    }
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn sibling_and_earlier_history_handles_and_cursors_never_cross_candidate_identity() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let base = open(&revision);
    let first = introduced(&base, "deps.generated");
    let later = apply(
        &first,
        json!({"kind":"rename_declaration","target":"deps.choose","name":"pick"}),
    );
    let sibling = apply(
        &base,
        json!({"kind":"rename_declaration","target":"deps.choose","name":"select"}),
    );
    let first_summary = summary(&first, "deps.pair");
    let first_handle = facet(&first_summary, ImageDependencyView::Sites)["handle"]
        .as_str()
        .unwrap();
    let options = ImageDependencyPageOptions::new(1, 65_536).unwrap();
    let first_page: Value = serde_json::from_str(
        &first
            .dependency_page(
                first.candidate_digest(),
                "deps.pair",
                ImageDependencyView::Sites,
                first_handle,
                None,
                options,
            )
            .unwrap(),
    )
    .unwrap();
    let cursor = first_page["next_cursor"].as_str().unwrap();
    for other in [&later, &sibling] {
        failed(
            other.dependency_page(
                other.candidate_digest(),
                "deps.pair",
                ImageDependencyView::Sites,
                first_handle,
                None,
                options,
            ),
            "SPX-G324",
        );
        let own_handle = facet(&summary(other, "deps.pair"), ImageDependencyView::Sites)["handle"]
            .as_str()
            .unwrap()
            .to_owned();
        failed(
            other.dependency_page(
                other.candidate_digest(),
                "deps.pair",
                ImageDependencyView::Sites,
                &own_handle,
                Some(cursor),
                options,
            ),
            "SPX-G324",
        );
    }
    assert_ne!(
        facet(&summary(&later, "deps.pair"), ImageDependencyView::Sites)["handle"],
        facet(&summary(&sibling, "deps.pair"), ImageDependencyView::Sites)["handle"]
    );
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn selectors_targets_views_cursors_and_options_fail_closed_without_mutation() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let candidate = introduced(&open(&revision), "deps.generated");
    let before = candidate.to_json().to_owned();
    let summary = summary(&candidate, "deps.pair");
    let handle = facet(&summary, ImageDependencyView::Sites)["handle"]
        .as_str()
        .unwrap();
    let zeros = format!("sha256:{}", "0".repeat(64));
    let options = ImageDependencyPageOptions::new(1, 65_536).unwrap();
    let base_image =
        ProjectSemanticImage::derive(Arc::clone(&revision), revision.project_revision()).unwrap();
    let base_summary: Value = serde_json::from_str(
        &base_image
            .dependency_summary(base_image.image_digest(), "deps.pair")
            .unwrap(),
    )
    .unwrap();
    let base_handle = facet(&base_summary, ImageDependencyView::Sites)["handle"]
        .as_str()
        .unwrap();

    failed(
        candidate.dependency_summary(&zeros, "deps.pair"),
        "SPX-G224",
    );
    failed(
        candidate.dependency_summary(candidate.candidate_digest(), "missing.declaration"),
        "SPX-G320",
    );
    failed(
        candidate.dependency_page(
            candidate.candidate_digest(),
            "deps.pair",
            ImageDependencyView::Calls,
            handle,
            None,
            options,
        ),
        "SPX-G324",
    );
    failed(
        candidate.dependency_page(
            candidate.candidate_digest(),
            "deps.pair",
            ImageDependencyView::Sites,
            base_handle,
            None,
            options,
        ),
        "SPX-G324",
    );
    failed(
        candidate.dependency_page(
            candidate.candidate_digest(),
            "deps.pair",
            ImageDependencyView::Sites,
            handle,
            None,
            ImageDependencyPageOptions::new(1, 1024).unwrap(),
        ),
        "SPX-G323",
    );
    failed(
        candidate.dependency_page(
            candidate.candidate_digest(),
            "deps.pair.x",
            ImageDependencyView::Sites,
            handle,
            None,
            options,
        ),
        "SPX-G324",
    );
    for cursor in ["", "0", "01:sha256:bad", "-1:sha256:bad"] {
        failed(
            candidate.dependency_page(
                candidate.candidate_digest(),
                "deps.pair",
                ImageDependencyView::Sites,
                handle,
                Some(cursor),
                options,
            ),
            "SPX-G324",
        );
    }
    failed(ImageDependencyView::parse("edges"), "SPX-G322");
    for (size, bytes) in [(0, 65_536), (129, 65_536), (1, 1023), (1, 1_048_577)] {
        failed(ImageDependencyPageOptions::new(size, bytes), "SPX-G322");
    }
    assert_eq!(candidate.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn candidate_navigation_is_descriptive_and_never_retains_or_authorizes_another_state() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let base = open(&revision);
    let candidate = introduced(&base, "deps.generated");
    let base_json = base.to_json().to_owned();
    let candidate_json = candidate.to_json().to_owned();
    let report = summary(&candidate, "deps.generated");
    assert_eq!(report["source_authority"], false);
    assert_eq!(report["evidence_owner"], "retained_checked_hir");
    assert!(report["nonclaims"]
        .as_array()
        .unwrap()
        .iter()
        .any(|claim| claim == "no_execution_or_source_authority"));
    for view in ImageDependencyView::ALL {
        let _ = candidate_pages(&candidate, "deps.generated", view, 2, 65_536);
    }
    assert_eq!(base.to_json(), base_json);
    assert_eq!(candidate.to_json(), candidate_json);
    assert_eq!(fixture.bytes(), disk);
}
