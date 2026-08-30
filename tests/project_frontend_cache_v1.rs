//! Authored incremental frontend evidence; intentionally not run in this change.
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, ImageWorkspace, ProjectFrontendBuild, ProjectFrontendCache,
    ProjectFrontendSource, ProjectManifest, ProjectRevision, ProjectSemanticImage,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

static SERIAL: AtomicU64 = AtomicU64::new(0);
const PATHS: [&str; 4] = [
    "src/app.spx",
    "src/core.spx",
    "src/spare.spx",
    "src/tests.spx",
];

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-frontend-cache-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in ["src/app.spx", "src/core.spx", "src/tests.spx"] {
            std::fs::copy(example.join(path), root.join(path)).unwrap();
        }
        let manifest = std::fs::read_to_string(example.join("semaprax.toml"))
            .unwrap()
            .replace(
                "\"src/tests.spx\"]",
                "\"src/spare.spx\", \"src/tests.spx\"]",
            );
        std::fs::write(root.join("semaprax.toml"), manifest).unwrap();
        let fixture = Self(root.canonicalize().unwrap());
        fixture.write_canonical(
            "src/spare.spx",
            "module calculator.spare;\n@id(\"calculator.spare.value\")\nfn value() -> i64 { 3 }\n",
        );
        fixture
    }
    fn manifest(&self) -> ProjectManifest {
        ProjectManifest::parse(&std::fs::read_to_string(self.0.join("semaprax.toml")).unwrap())
            .unwrap()
    }
    fn sources(&self) -> Vec<ProjectFrontendSource> {
        PATHS
            .iter()
            .map(|path| {
                ProjectFrontendSource::new(
                    path,
                    &std::fs::read_to_string(self.0.join(path)).unwrap(),
                )
                .unwrap()
            })
            .collect()
    }
    fn revision(&self) -> Result<Arc<ProjectRevision>, Vec<Diagnostic>> {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
    }
    fn write_canonical(&self, path: &str, source: &str) {
        let program = semaprax::parse(source, path).unwrap();
        std::fs::write(self.0.join(path), semaprax::format::canonical(&program)).unwrap();
    }
    fn replace(&self, path: &str, from: &str, to: &str) {
        let original = std::fs::read_to_string(self.0.join(path)).unwrap();
        assert!(original.contains(from));
        self.write_canonical(path, &original.replace(from, to));
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn image(revision: Arc<ProjectRevision>) -> Arc<ProjectSemanticImage> {
    let expected = revision.project_revision().to_owned();
    Arc::new(ProjectSemanticImage::derive(revision, &expected).unwrap())
}
fn work(build: &ProjectFrontendBuild, parsed: usize, reused: usize) -> Value {
    let report: Value = serde_json::from_str(build.to_json()).unwrap();
    assert_eq!(report["work"]["modules_parsed"], parsed);
    assert_eq!(report["work"]["canonicalizer_calls"], parsed);
    assert_eq!(report["work"]["modules_reused"], reused);
    assert_eq!(report["work"]["cached_AST_clones"], reused);
    assert_eq!(report["work"]["modules_resolved"], 4);
    assert_eq!(report["work"]["checked_HIR_reused"], 0);
    assert_eq!(report["work"]["full_link_and_profile_admission"], true);
    report
}
fn same(cached: &Arc<ProjectRevision>, cold: Arc<ProjectRevision>) {
    assert_eq!(cached.project_revision(), cold.project_revision());
    assert_eq!(cached.workspace_manifest(), cold.workspace_manifest());
    assert_eq!(cached.workspace_revision(), cold.workspace_revision());
    assert_eq!(cached.sources(), cold.sources());
    assert_eq!(cached.semantic_graph(), cold.semantic_graph());
    assert_eq!(image(Arc::clone(cached)).to_json(), image(cold).to_json());
}
fn errors<T>(result: Result<T, Vec<Diagnostic>>) -> Vec<Diagnostic> {
    match result {
        Ok(_) => panic!("expected rejection"),
        Err(errors) => errors,
    }
}

#[test]
fn warm_sources_skip_actual_parsing_and_formatting_without_changing_cold_bytes() {
    let fixture = Fixture::new();
    let manifest = fixture.manifest();
    let mut cache = ProjectFrontendCache::new();
    let cold = cache.build(&manifest, &fixture.sources()).unwrap();
    work(&cold, 4, 0);
    same(cold.revision(), fixture.revision().unwrap());
    let warm = cache.build(&manifest, &fixture.sources()).unwrap();
    let report = work(&warm, 0, 4);
    assert_eq!(report["invalidated_sources"], json!([]));
    assert_eq!(report["manifest_context_reset"], false);
    same(warm.revision(), fixture.revision().unwrap());
    let repeated = cache.build(&manifest, &fixture.sources()).unwrap();
    assert_eq!(warm.to_json(), repeated.to_json());
}

#[test]
fn leaf_and_provider_changes_invalidate_exact_old_reverse_import_closure() {
    let fixture = Fixture::new();
    let manifest = fixture.manifest();
    let mut cache = ProjectFrontendCache::new();
    cache.build(&manifest, &fixture.sources()).unwrap();
    fixture.replace("src/app.spx", "multiply(6, 7)", "multiply(6, 8)");
    let leaf = cache.build(&manifest, &fixture.sources()).unwrap();
    let report = work(&leaf, 1, 3);
    assert_eq!(report["invalidated_sources"], json!(["src/app.spx"]));
    same(leaf.revision(), fixture.revision().unwrap());
    fixture.replace("src/core.spx", "left + right", "left + right + 1");
    let provider = cache.build(&manifest, &fixture.sources()).unwrap();
    let report = work(&provider, 3, 1);
    assert_eq!(
        report["invalidated_sources"],
        json!(["src/app.spx", "src/core.spx", "src/tests.spx"])
    );
    same(provider.revision(), fixture.revision().unwrap());
}

#[test]
fn changed_import_signature_is_rechecked_and_failed_build_does_not_poison_cache() {
    let fixture = Fixture::new();
    let manifest = fixture.manifest();
    let mut cache = ProjectFrontendCache::new();
    cache.build(&manifest, &fixture.sources()).unwrap();
    let original = std::fs::read_to_string(fixture.0.join("src/core.spx")).unwrap();
    fixture.replace(
        "src/core.spx",
        "fn add(left: i64, right: i64)",
        "fn add(left: i64, right: i64, extra: i64)",
    );
    let warm_errors = errors(cache.build(&manifest, &fixture.sources()));
    let cold_errors = errors(fixture.revision());
    assert_eq!(format!("{warm_errors:?}"), format!("{cold_errors:?}"));
    std::fs::write(fixture.0.join("src/core.spx"), original).unwrap();
    work(&cache.build(&manifest, &fixture.sources()).unwrap(), 0, 4);
    let mut noncanonical = fixture.sources();
    noncanonical[0] = ProjectFrontendSource::new(
        noncanonical[0].path(),
        &format!("{}\n", noncanonical[0].source()),
    )
    .unwrap();
    assert!(errors(cache.build(&manifest, &noncanonical))
        .iter()
        .any(|d| d.code == "SPX-G170"));
    work(&cache.build(&manifest, &fixture.sources()).unwrap(), 0, 4);
}

#[test]
fn exact_manifest_context_resets_every_module_and_duplicate_paths_reject() {
    let fixture = Fixture::new();
    let mut cache = ProjectFrontendCache::new();
    let manifest = fixture.manifest();
    cache.build(&manifest, &fixture.sources()).unwrap();
    let changed = ProjectManifest::parse(
        &manifest
            .to_canonical_toml()
            .replace("name = \"calculator\"", "name = \"calculator-new\""),
    )
    .unwrap();
    let build = cache.build(&changed, &fixture.sources()).unwrap();
    assert_eq!(work(&build, 4, 0)["manifest_context_reset"], true);
    let mut duplicate = fixture.sources();
    duplicate.push(ProjectFrontendSource::new(duplicate[0].path(), duplicate[0].source()).unwrap());
    assert!(errors(cache.build(&changed, &duplicate))
        .iter()
        .any(|d| d.code == "SPX-G255"));
    work(&cache.build(&changed, &fixture.sources()).unwrap(), 0, 4);
}

#[test]
fn image_refresh_admits_owned_changes_without_a_prebuilt_changed_revision() {
    let fixture = Fixture::new();
    let manifest = fixture.manifest();
    let original = image(fixture.revision().unwrap());
    let expected = original.image_digest().to_owned();
    let mut workspace = ImageWorkspace::with_frontend_cache(Arc::clone(&original)).unwrap();
    let repeated = workspace
        .refresh_owned_sources(&manifest, &fixture.sources(), &expected)
        .unwrap();
    assert!(repeated.image_reused());
    assert!(Arc::ptr_eq(workspace.image(), &original));
    let report: Value = serde_json::from_str(repeated.to_json()).unwrap();
    assert_eq!(report["frontend_work"]["work"]["modules_parsed"], 0);
    assert_eq!(report["frontend_work"]["work"]["modules_resolved"], 4);
    fixture.replace("src/app.spx", "multiply(6, 7)", "multiply(6, 8)");
    // No changed ProjectRevision has been built before this call.
    let changed = workspace
        .refresh_owned_sources(&manifest, &fixture.sources(), &expected)
        .unwrap();
    let report: Value = serde_json::from_str(changed.to_json()).unwrap();
    assert_eq!(report["frontend_work"]["work"]["modules_parsed"], 1);
    assert_eq!(report["frontend_work"]["work"]["modules_reused"], 3);
    assert!(!changed.image_reused());
    same(workspace.image().revision(), fixture.revision().unwrap());
    let retained = Arc::clone(workspace.image());
    assert!(
        errors(workspace.refresh_owned_sources(&manifest, &fixture.sources(), &expected))
            .iter()
            .any(|d| d.code == "SPX-G251")
    );
    assert!(Arc::ptr_eq(workspace.image(), &retained));
    let expected = retained.image_digest().to_owned();
    let mut invalid = fixture.sources();
    invalid[0] = ProjectFrontendSource::new("src/app.spx", "invalid").unwrap();
    assert!(workspace
        .refresh_owned_sources(&manifest, &invalid, &expected)
        .is_err());
    assert!(Arc::ptr_eq(workspace.image(), &retained));
    let report = workspace
        .refresh_owned_sources(&manifest, &fixture.sources(), &expected)
        .unwrap();
    let report: Value = serde_json::from_str(report.to_json()).unwrap();
    assert_eq!(report["frontend_work"]["work"]["modules_reused"], 4);
    let mut legacy = ImageWorkspace::new(retained);
    assert!(
        errors(legacy.refresh_owned_sources(&manifest, &fixture.sources(), &expected))
            .iter()
            .any(|d| d.code == "SPX-G255")
    );
}
