//! Shared declaration dependency index: authored tests, intentionally unrun.
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectRevision, ProjectSemanticImage,
    SemanticChange,
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
            "spx-declaration-dependencies-{}-{}",
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

#[test]
fn concurrent_queries_share_deterministic_results_without_changing_image_bytes() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let image = Arc::new(image(&revision));
    let before = image.to_json().to_owned();
    let digest = image.image_digest().to_owned();
    // All threads start against a cold lazy index. No test is executed locally.
    let barrier = Arc::new(std::sync::Barrier::new(4));
    let threads = (0..4)
        .map(|_| {
            let image = Arc::clone(&image);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                image
                    .declaration_dependencies(image.image_digest(), "deps.pair.x")
                    .unwrap()
            })
        })
        .collect::<Vec<_>>();
    let results = threads
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect::<Vec<_>>();
    assert!(results.iter().all(|result| result == &results[0]));
    assert_eq!(image.to_json(), before);
    assert_eq!(image.image_digest(), digest);
    let independent =
        ProjectSemanticImage::derive(Arc::clone(&revision), revision.project_revision()).unwrap();
    assert_eq!(independent.to_json(), before);
    assert_eq!(independent.image_digest(), digest);
    assert_eq!(
        independent
            .declaration_dependencies(independent.image_digest(), "deps.pair.x")
            .unwrap(),
        results[0]
    );
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn stale_image_and_unknown_targets_are_rejected_without_poisoning_subsequent_reads() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let image = image(&revision);
    let before = image.to_json().to_owned();
    let digest = image.image_digest().to_owned();
    let wrong = format!("sha256:{}", "0".repeat(64));
    assert!(image
        .declaration_dependencies(&wrong, "deps.pair.x")
        .is_err());
    for target in ["", "missing.field", "deps.pair\0x"] {
        assert!(image
            .declaration_dependencies(image.image_digest(), target)
            .is_err());
    }
    let first = image
        .declaration_dependencies(image.image_digest(), "deps.pair.x")
        .unwrap();
    assert_eq!(
        first,
        image
            .declaration_dependencies(image.image_digest(), "deps.pair.x")
            .unwrap()
    );
    assert_eq!(image.to_json(), before);
    assert_eq!(image.image_digest(), digest);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn record_field_sites_bind_cross_source_contract_body_and_declared_test_provenance() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let image = image(&revision);
    let field = report(&image, "deps.pair.x");
    assert_eq!(
        field["schema"],
        "semaprax.image-declaration-dependencies.v1"
    );
    assert_eq!(field["image_digest"], image.image_digest());
    assert_eq!(field["target"], "deps.pair.x");
    assert_eq!(field["source_binding"]["path"], "src/core.spx");
    assert_eq!(field["selected_declaration_ids"], json!(["deps.pair.x"]));
    let relations = &field["relationships"];
    let sites = relations["direct_field_sites"].as_array().unwrap();
    assert!(sites
        .iter()
        .all(|site| site["field_or_type_id"] == "deps.pair.x"));
    for access in ["initialize", "in_place_write", "pattern_bind"] {
        assert!(
            sites.iter().any(|site| site["access"] == access),
            "missing {access}: {sites:?}"
        );
    }
    assert!(sites
        .iter()
        .any(|site| site["phase"] == "requires" && site["function_id"] == "deps.evaluate"));
    assert!(sites
        .iter()
        .any(|site| site["path"] == "src/app.spx" && site["function_id"] == "deps.main"));
    assert!(sites
        .iter()
        .any(|site| site["access"] == "read_or_move" || site["access"] == "projection_read"));
    for site in sites {
        let path = site["path"].as_str().unwrap();
        let source = revision
            .sources()
            .iter()
            .find(|source| source.path() == path)
            .unwrap();
        assert_eq!(site["source_revision"], source.source_revision());
        assert_eq!(site["source_digest"], source.source_digest());
        assert_eq!(site["evidence_owner"], "retained_checked_hir");
        assert_eq!(site["reason"], site["access"]);
        assert!(site["expression_id"]
            .as_str()
            .is_some_and(|id| !id.is_empty()));
        assert!(site["span"]["start"].as_u64().unwrap() <= site["span"]["end"].as_u64().unwrap());
        assert!(site["span"]["end"].as_u64().unwrap() <= source.source().len() as u64);
    }
    assert_eq!(relations["declared_test_root"], "deps.test");
    assert_eq!(relations["test_reachable"], true);
    assert_eq!(relations["coverage"], "not_inferred");
    assert_eq!(relations["executed"], false);
    for caller in ["deps.evaluate", "deps.main", "deps.test"] {
        assert!(relations["reverse_callable_closure"]
            .as_array()
            .unwrap()
            .contains(&json!(caller)));
    }
    let whole = report(&image, "deps.pair");
    assert_eq!(
        whole["selected_declaration_ids"],
        json!(["deps.pair", "deps.pair.x", "deps.pair.y"])
    );
    let whole_sites = whole["relationships"]["direct_field_sites"]
        .as_array()
        .unwrap();
    assert!(whole_sites.iter().any(
        |site| site["field_or_type_id"] == "deps.pair" && site["access"] == "construct_record"
    ));
    assert!(whole_sites
        .iter()
        .any(|site| site["field_or_type_id"] == "deps.pair.y"
            && site["access"] == "update_result_field"));
    assert!(whole_sites.iter().any(
        |site| site["field_or_type_id"] == "deps.pair.y" && site["access"] == "pattern_ignore"
    ));
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn variant_owner_case_and_payload_queries_preserve_distinct_identity_sites_and_local_callers() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let image = image(&revision);
    let owner = report(&image, "deps.choice");
    assert_eq!(
        owner["selected_declaration_ids"],
        json!([
            "deps.choice",
            "deps.choice.none",
            "deps.choice.some",
            "deps.choice.some.value"
        ])
    );
    for (target, accesses) in [
        ("deps.choice.some", ["construct_case", "case_pattern"]),
        ("deps.choice.some.value", ["initialize", "pattern_bind"]),
    ] {
        let selected = report(&image, target);
        let sites = selected["relationships"]["direct_field_sites"]
            .as_array()
            .unwrap();
        assert!(sites
            .iter()
            .all(|site| site["function_id"] == "deps.choose"));
        for access in accesses {
            assert!(sites
                .iter()
                .any(|site| site["field_or_type_id"] == target && site["access"] == access));
        }
        for caller in ["deps.choose", "deps.evaluate", "deps.main", "deps.test"] {
            assert!(selected["relationships"]["reverse_callable_closure"]
                .as_array()
                .unwrap()
                .contains(&json!(caller)));
        }
        assert_eq!(selected["relationships"]["test_reachable"], true);
    }
    let none = report(&image, "deps.choice.none");
    let sites = none["relationships"]["direct_field_sites"]
        .as_array()
        .unwrap();
    assert!(sites.iter().any(|site| site["access"] == "case_pattern"));
    assert!(!sites.iter().any(|site| site["access"] == "construct_case"));
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn shared_index_matches_the_legacy_delta_projection_without_changing_image_identity() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let revision = fixture.revision();
    let base = ProjectCandidate::open(Arc::clone(&revision), revision.project_revision()).unwrap();
    let change = SemanticChange::new(base.revision().project_revision(), &json!({"kind":"add_record_field","target":"deps.pair","field":{"id":"deps.pair.flag","name":"flag","type":"bool","default":{"kind":"bool","value":false}}})).unwrap();
    let candidate = base.apply(base.candidate_digest(), &change).unwrap();
    let image = image(candidate.revision());
    let before = image.to_json().to_owned();
    let public = report(&image, "deps.pair.flag");
    let delta: Value = serde_json::from_str(
        &candidate
            .semantic_delta(candidate.candidate_digest(), "deps.pair.flag")
            .unwrap(),
    )
    .unwrap();
    let legacy = &delta["facets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|facet| facet["facet"] == "reverse_field_and_call_relationships")
        .unwrap()["candidate"];
    let mut projected = public["relationships"].clone();
    for site in projected["direct_field_sites"].as_array_mut().unwrap() {
        site.as_object_mut().unwrap().retain(|key, _| {
            [
                "field_or_type_id",
                "function_id",
                "path",
                "phase",
                "expression_id",
                "access",
            ]
            .contains(&key.as_str())
        });
    }
    assert_eq!(&projected, legacy);
    assert_eq!(image.to_json(), before);
    assert!(public["relationships"]["direct_field_sites"]
        .as_array()
        .unwrap()
        .iter()
        .any(|site| site["access"] == "pattern_ignore"));
    assert_eq!(fixture.bytes(), disk);
}
