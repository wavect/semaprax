//! Compact dependency navigation: authored tests, intentionally unrun.
use semaprax::project::{
    with_authenticated_project, ImageDependencyPageOptions, ImageDependencyView, ProjectRevision,
    ProjectSemanticImage,
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
            "spx-dependency-navigation-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let root = root.canonicalize().unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "declaration-dependencies"
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
@id("deps.choice") variant Choice { @id("deps.choice.some") Some { @id("deps.choice.some.value") value: i64, }, @id("deps.choice.none") None, }
@id("deps.choose") fn choose(value: i64) -> i64 {
    let choice = Choice::Some { value: value };
    match choice { Choice::Some { value: picked } => picked, Choice::None {} => 0, }
}
@id("deps.evaluate") fn evaluate(value: i64) -> i64
    requires (Pair { x: 0, y: 0 }).x == 0
{
    let mut pair = Pair { x: value, y: 0 };
    pair.x = pair.x + 1;
    let updated = pair with { y: 2 };
    match updated { Pair { x: picked, y: _ } => choose(picked), }
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
    let selected = match item { Metric { x: picked, y: _ } => picked, };
    evaluate(selected)
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
fn image(revision: &Arc<ProjectRevision>) -> ProjectSemanticImage {
    ProjectSemanticImage::derive(Arc::clone(revision), revision.project_revision()).unwrap()
}
fn report(image: &ProjectSemanticImage, target: &str) -> Value {
    serde_json::from_str(
        &image
            .declaration_dependencies(image.image_digest(), target)
            .unwrap(),
    )
    .unwrap()
}
fn summary(image: &ProjectSemanticImage, target: &str) -> Value {
    serde_json::from_str(
        &image
            .dependency_summary(image.image_digest(), target)
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
fn pages(
    image: &ProjectSemanticImage,
    target: &str,
    view: ImageDependencyView,
    size: usize,
) -> Vec<Value> {
    let summary = summary(image, target);
    let facet = facet(&summary, view);
    let handle = facet["handle"].as_str().unwrap();
    let total = facet["total_items"].as_u64().unwrap() as usize;
    let mut result = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let encoded = image
            .dependency_page(
                image.image_digest(),
                target,
                view,
                handle,
                cursor.as_deref(),
                ImageDependencyPageOptions::new(size, 65_536).unwrap(),
            )
            .unwrap();
        assert!(encoded.len() <= 65_536);
        let page: Value = serde_json::from_str(&encoded).unwrap();
        assert_eq!(page["schema"], "semaprax.image-dependency-page.v1");
        assert_eq!(page["view"], view.name());
        assert_eq!(page["handle"], handle);
        assert_eq!(page["cursor"], json!(cursor));
        assert_eq!(page["offset"], result.len());
        assert_eq!(page["total_items"], total);
        assert_eq!(page["page_size"], size);
        assert_eq!(page["max_bytes"], 65_536);
        assert_eq!(page["source_authority"], false);
        assert_eq!(page["image_digest"], image.image_digest());
        let items = page["items"].as_array().unwrap();
        assert!(items.len() <= size);
        result.extend(items.iter().cloned());
        assert!(result.len() <= total);
        cursor = page["next_cursor"].as_str().map(str::to_owned);
        if cursor.is_none() {
            break;
        }
        assert!(!items.is_empty(), "a continuation must make progress");
    }
    assert_eq!(result.len(), total);
    result
}
fn failed<T>(result: Result<T, Vec<semaprax::diagnostic::Diagnostic>>, code: &str) {
    let errors = result.err().expect("invalid navigation accepted");
    assert!(errors.iter().any(|error| error.code == code), "{errors:?}");
}

#[test]
fn all_four_views_reconstruct_the_full_report_in_order_with_exact_counts() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let image = image(&revision);
    let full = report(&image, "deps.pair");
    let compact = image
        .dependency_summary(image.image_digest(), "deps.pair")
        .unwrap();
    let summary: Value = serde_json::from_str(&compact).unwrap();
    assert_eq!(summary["schema"], "semaprax.image-dependency-summary.v1");
    assert_eq!(summary["facets"].as_array().unwrap().len(), 4);
    assert!(compact.len() < 16 * 1024);
    for key in [
        "items",
        "relationships",
        "direct_field_sites",
        "direct_call_sites",
    ] {
        assert!(
            summary.get(key).is_none(),
            "summary unexpectedly embeds {key}"
        );
    }
    let sites = pages(&image, "deps.pair", ImageDependencyView::Sites, 2);
    assert_eq!(json!(sites), full["relationships"]["direct_field_sites"]);
    let calls = pages(&image, "deps.pair", ImageDependencyView::Calls, 2);
    assert_eq!(json!(calls), full["direct_call_sites"]);
    let callers = pages(&image, "deps.pair", ImageDependencyView::Callers, 2);
    assert_eq!(
        json!(callers
            .iter()
            .map(|row| row["id"].clone())
            .collect::<Vec<_>>()),
        full["relationships"]["reverse_callable_closure"]
    );
    let members = pages(&image, "deps.pair", ImageDependencyView::Members, 2);
    assert_eq!(
        json!(members
            .iter()
            .map(|row| row["id"].clone())
            .collect::<Vec<_>>()),
        full["selected_declaration_ids"]
    );
    for row in members {
        assert_eq!(row["declaration"]["id"], row["id"]);
        assert_eq!(row["declaration"]["identity_origin"], "explicit");
    }
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn site_and_caller_pages_retain_source_provenance_and_static_reasons() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let image = image(&revision);
    let sites = pages(&image, "deps.pair.x", ImageDependencyView::Sites, 1);
    assert!(sites.iter().any(|site| site["phase"] == "requires"));
    assert!(sites.iter().any(|site| site["path"] == "src/app.spx"));
    for site in sites {
        let path = site["path"].as_str().unwrap();
        let source = revision
            .sources()
            .iter()
            .find(|source| source.path() == path)
            .unwrap();
        assert_eq!(site["source_revision"], source.source_revision());
        assert_eq!(site["source_digest"], source.source_digest());
        assert_eq!(site["reason"], site["access"]);
        assert_eq!(site["evidence_owner"], "retained_checked_hir");
    }
    let callers = pages(&image, "deps.pair.x", ImageDependencyView::Callers, 1);
    for (id, reason) in [
        ("deps.evaluate", "direct_site_user"),
        ("deps.main", "direct_site_user"),
        ("deps.test", "reverse_direct_caller"),
    ] {
        let row = callers.iter().find(|row| row["id"] == id).unwrap();
        assert_eq!(row["reason"], reason);
        let binding = &row["source_binding"];
        let source = revision
            .sources()
            .iter()
            .find(|source| source.path() == binding["path"].as_str().unwrap())
            .unwrap();
        assert_eq!(binding["source_revision"], source.source_revision());
        assert_eq!(binding["source_digest"], source.source_digest());
        assert_eq!(row["evidence_owner"], "retained_checked_hir");
    }
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn empty_site_and_call_views_terminate_without_fabricating_relevance() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let image = image(&revision);
    assert!(pages(&image, "deps.public", ImageDependencyView::Sites, 2).is_empty());
    assert!(pages(&image, "deps.public", ImageDependencyView::Calls, 2).is_empty());
    let callers = pages(&image, "deps.public", ImageDependencyView::Callers, 2);
    assert_eq!(callers.len(), 1);
    assert_eq!(callers[0]["id"], "deps.public");
    assert_eq!(callers[0]["reason"], "target");
    let members = pages(&image, "deps.public", ImageDependencyView::Members, 2);
    assert_eq!(members.len(), 1);
    assert_eq!(members[0]["id"], "deps.public");
    let full = report(&image, "deps.public");
    assert_eq!(full["relationships"]["test_reachable"], false);
    assert_eq!(full["relationships"]["coverage"], "not_inferred");
    assert_eq!(full["relationships"]["executed"], false);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn forged_cross_scope_and_noncanonical_cursors_and_options_fail_without_changing_the_image() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let image = image(&revision);
    let before = image.to_json().to_owned();
    let summary = summary(&image, "deps.pair");
    let handle = facet(&summary, ImageDependencyView::Sites)["handle"]
        .as_str()
        .unwrap();
    let page: Value = serde_json::from_str(
        &image
            .dependency_page(
                image.image_digest(),
                "deps.pair",
                ImageDependencyView::Sites,
                handle,
                None,
                ImageDependencyPageOptions::new(2, 65_536).unwrap(),
            )
            .unwrap(),
    )
    .unwrap();
    let cursor = page["next_cursor"]
        .as_str()
        .expect("fixture has multiple pages");
    let zeros = format!("sha256:{}", "0".repeat(64));
    failed(
        image.dependency_page(
            image.image_digest(),
            "deps.pair",
            ImageDependencyView::Sites,
            &zeros,
            None,
            ImageDependencyPageOptions::default(),
        ),
        "SPX-G324",
    );
    failed(
        image.dependency_page(
            image.image_digest(),
            "deps.pair",
            ImageDependencyView::Calls,
            handle,
            None,
            ImageDependencyPageOptions::default(),
        ),
        "SPX-G324",
    );
    failed(
        image.dependency_page(
            image.image_digest(),
            "deps.pair.x",
            ImageDependencyView::Sites,
            handle,
            None,
            ImageDependencyPageOptions::default(),
        ),
        "SPX-G324",
    );
    failed(
        image.dependency_page(
            image.image_digest(),
            "deps.pair",
            ImageDependencyView::Sites,
            handle,
            Some(cursor),
            ImageDependencyPageOptions::new(3, 65_536).unwrap(),
        ),
        "SPX-G324",
    );
    failed(
        image.dependency_page(
            image.image_digest(),
            "deps.pair",
            ImageDependencyView::Sites,
            handle,
            Some(cursor),
            ImageDependencyPageOptions::new(2, 32_768).unwrap(),
        ),
        "SPX-G324",
    );
    for cursor in [
        format!("0{cursor}"),
        format!("2:{zeros}"),
        "0".to_owned(),
        "-1".to_owned(),
        "".to_owned(),
    ] {
        failed(
            image.dependency_page(
                image.image_digest(),
                "deps.pair",
                ImageDependencyView::Sites,
                handle,
                Some(&cursor),
                ImageDependencyPageOptions::new(2, 65_536).unwrap(),
            ),
            "SPX-G324",
        );
    }
    for (size, bytes) in [(0, 65_536), (129, 65_536), (1, 1023), (1, 1024 * 1024 + 1)] {
        failed(ImageDependencyPageOptions::new(size, bytes), "SPX-G322");
    }
    assert!(image.dependency_summary(&zeros, "deps.pair").is_err());
    assert!(image
        .dependency_summary(image.image_digest(), "missing.type")
        .is_err());
    assert!(!pages(&image, "deps.pair", ImageDependencyView::Sites, 2).is_empty());
    assert_eq!(image.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn independent_roots_and_concurrent_queries_share_deterministic_handles_pages_and_image_identity() {
    let first = Fixture::new();
    let second = Fixture::new();
    let first_disk = first.bytes();
    let second_disk = second.bytes();
    let revision = first.revision();
    let independent_revision = second.revision();
    let image = Arc::new(image(&revision));
    let other = ProjectSemanticImage::derive(
        Arc::clone(&independent_revision),
        independent_revision.project_revision(),
    )
    .unwrap();
    let before = image.to_json().to_owned();
    let digest = image.image_digest().to_owned();
    assert_eq!(other.image_digest(), digest);
    assert_eq!(other.to_json(), before);
    let barrier = Arc::new(std::sync::Barrier::new(4));
    let threads = [
        ImageDependencyView::Sites,
        ImageDependencyView::Callers,
        ImageDependencyView::Calls,
        ImageDependencyView::Members,
    ]
    .into_iter()
    .map(|view| {
        let image = Arc::clone(&image);
        let barrier = Arc::clone(&barrier);
        std::thread::spawn(move || {
            barrier.wait();
            (
                view,
                image
                    .dependency_summary(image.image_digest(), "deps.pair")
                    .unwrap(),
                pages(&image, "deps.pair", view, 2),
            )
        })
    })
    .collect::<Vec<_>>();
    for thread in threads {
        let (view, summary, items) = thread.join().unwrap();
        assert_eq!(
            summary,
            other
                .dependency_summary(other.image_digest(), "deps.pair")
                .unwrap()
        );
        assert_eq!(items, pages(&other, "deps.pair", view, 2));
    }
    assert_eq!(image.image_digest(), digest);
    assert_eq!(image.to_json(), before);
    assert_eq!(first.bytes(), first_disk);
    assert_eq!(second.bytes(), second_disk);
}
