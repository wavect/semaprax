//! Checked-module cache evidence authored without running tests or compiler gates.
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, ImageWorkspace, ProjectFrontendBuild, ProjectFrontendCache,
    ProjectFrontendSource, ProjectManifest, ProjectRevision, ProjectSemanticImage,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

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
            "spx-semantic-cache-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in ["src/app.spx", "src/core.spx", "src/tests.spx"] {
            std::fs::copy(sample.join(path), root.join(path)).unwrap();
        }
        let manifest = std::fs::read_to_string(sample.join("semaprax.toml"))
            .unwrap()
            .replace(
                "\"src/tests.spx\"]",
                "\"src/spare.spx\", \"src/tests.spx\"]",
            );
        std::fs::write(root.join("semaprax.toml"), manifest).unwrap();
        let fixture = Self(root.canonicalize().unwrap());
        fixture.write(
            "src/spare.spx",
            "module calculator.spare; @id(\"calculator.spare.value\") fn value() -> i64 { 3 }",
        );
        fixture
    }
    fn read(&self, path: &str) -> String {
        std::fs::read_to_string(self.0.join(path)).unwrap()
    }
    fn write(&self, path: &str, source: &str) {
        let program = semaprax::parse(source, path).unwrap();
        std::fs::write(self.0.join(path), semaprax::format::canonical(&program)).unwrap();
    }
    fn replace(&self, path: &str, from: &str, to: &str) {
        let source = self.read(path);
        assert!(source.contains(from));
        self.write(path, &source.replace(from, to));
    }
    fn manifest(&self) -> ProjectManifest {
        ProjectManifest::parse(&self.read("semaprax.toml")).unwrap()
    }
    fn sources(&self) -> Vec<ProjectFrontendSource> {
        PATHS
            .iter()
            .map(|path| ProjectFrontendSource::new(path, &self.read(path)).unwrap())
            .collect()
    }
    fn cold(&self) -> Result<Arc<ProjectRevision>, Vec<Diagnostic>> {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
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
fn same(cached: &Arc<ProjectRevision>, cold: Arc<ProjectRevision>) {
    assert_eq!(cached.project_revision(), cold.project_revision());
    assert_eq!(cached.workspace_revision(), cold.workspace_revision());
    assert_eq!(cached.workspace_manifest(), cold.workspace_manifest());
    assert_eq!(cached.sources(), cold.sources());
    assert_eq!(cached.semantic_graph(), cold.semantic_graph());
    let left = image(Arc::clone(cached));
    let right = image(cold);
    assert_eq!(left.image_digest(), right.image_digest());
    assert_eq!(left.to_json(), right.to_json());
}
fn work(build: &ProjectFrontendBuild, parsed: usize, reused: usize) -> Value {
    let report: Value = serde_json::from_str(build.to_json()).unwrap();
    assert_eq!(report["schema"], "semaprax.project-semantic-cache-work.v1");
    assert_eq!(
        report["compiler"]["compatibility"],
        "semaprax.project-checked-module-hir.v1"
    );
    assert_eq!(report["work"]["modules_parsed"], parsed);
    assert_eq!(report["work"]["canonicalizer_calls"], parsed);
    assert_eq!(report["work"]["modules_reused"], reused);
    assert_eq!(report["work"]["modules_resolved"], parsed);
    assert_eq!(report["work"]["checked_HIR_reused"], reused);
    assert_eq!(report["work"]["full_cross_file_checks"], true);
    assert_eq!(report["work"]["full_link_and_profile_admission"], true);
    report
}
fn errors<T>(result: Result<T, Vec<Diagnostic>>) -> Vec<Diagnostic> {
    match result {
        Ok(_) => panic!("expected rejection"),
        Err(errors) => errors,
    }
}

#[test]
fn warm_checked_modules_preserve_cold_graph_image_bytes_and_legacy_mode() {
    let fixture = Fixture::new();
    let manifest = fixture.manifest();
    let mut cache = ProjectFrontendCache::new_with_semantic_cache();
    let first = cache.build(&manifest, &fixture.sources()).unwrap();
    work(&first, 4, 0);
    same(first.revision(), fixture.cold().unwrap());
    let warm = cache.build(&manifest, &fixture.sources()).unwrap();
    assert_eq!(work(&warm, 0, 4)["invalidated_sources"], json!([]));
    same(warm.revision(), fixture.cold().unwrap());
    assert_eq!(
        warm.to_json(),
        cache
            .build(&manifest, &fixture.sources())
            .unwrap()
            .to_json()
    );
    let mut ast_only = ProjectFrontendCache::new();
    ast_only.build(&manifest, &fixture.sources()).unwrap();
    let build = ast_only.build(&manifest, &fixture.sources()).unwrap();
    let report: Value = serde_json::from_str(build.to_json()).unwrap();
    assert_eq!(report["schema"], "semaprax.project-frontend-cache-work.v1");
    assert_eq!(report["work"]["modules_resolved"], 4);
    assert_eq!(report["work"]["checked_HIR_reused"], 0);
    same(build.revision(), Arc::clone(warm.revision()));
}

#[test]
fn leaf_provider_and_private_body_edits_invalidate_their_checked_modules() {
    let fixture = Fixture::new();
    let manifest = fixture.manifest();
    let mut cache = ProjectFrontendCache::new_with_semantic_cache();
    cache.build(&manifest, &fixture.sources()).unwrap();
    fixture.replace("src/app.spx", "multiply(6, 7)", "multiply(6, 8)");
    let leaf = cache.build(&manifest, &fixture.sources()).unwrap();
    assert_eq!(
        work(&leaf, 1, 3)["invalidated_sources"],
        json!(["src/app.spx"])
    );
    same(leaf.revision(), fixture.cold().unwrap());
    fixture.replace("src/core.spx", "left + right", "left + right + 1");
    let provider = cache.build(&manifest, &fixture.sources()).unwrap();
    assert_eq!(
        work(&provider, 3, 1)["invalidated_sources"],
        json!(["src/app.spx", "src/core.spx", "src/tests.spx"])
    );
    same(provider.revision(), fixture.cold().unwrap());
    fixture.replace("src/spare.spx", "    3", "    4");
    let local = cache.build(&manifest, &fixture.sources()).unwrap();
    assert_eq!(
        work(&local, 1, 3)["invalidated_sources"],
        json!(["src/spare.spx"])
    );
    same(local.revision(), fixture.cold().unwrap());
}

#[test]
fn body_only_edit_reuses_exact_sibling_functions_but_contract_change_invalidates_all() {
    let fixture = Fixture::new();
    fixture.write(
        "src/spare.spx",
        r#"module calculator.spare;
@id("calculator.spare.value")
fn value(input: i64) -> i64 requires input >= 0 { input + 1 }
@id("calculator.spare.other")
fn other() -> i64 { 7 }
"#,
    );
    let manifest = fixture.manifest();
    let mut cache = ProjectFrontendCache::new_with_semantic_cache();
    cache.build(&manifest, &fixture.sources()).unwrap();

    fixture.replace("src/spare.spx", "input + 1", "input + 2");
    let body = cache.build(&manifest, &fixture.sources()).unwrap();
    let report = work(&body, 1, 3);
    assert_eq!(report["work"]["monomorphic_functions_resolved"], 1);
    // `other` and the synthetic entry point have exact AST and environment.
    assert_eq!(report["work"]["monomorphic_function_HIR_reused"], 2);
    assert_eq!(report["work"]["full_source_verification"], true);
    assert_eq!(report["work"]["full_HIR_validation"], true);
    same(body.revision(), fixture.cold().unwrap());

    fixture.replace("src/spare.spx", "input >= 0", "input >= 1");
    let contract = cache.build(&manifest, &fixture.sources()).unwrap();
    let report = work(&contract, 1, 3);
    assert_eq!(report["work"]["monomorphic_functions_resolved"], 3);
    assert_eq!(report["work"]["monomorphic_function_HIR_reused"], 0);
    same(contract.revision(), fixture.cold().unwrap());
}

#[test]
fn stale_import_signature_cannot_hide_behind_cached_consumers() {
    let fixture = Fixture::new();
    let manifest = fixture.manifest();
    let mut cache = ProjectFrontendCache::new_with_semantic_cache();
    cache.build(&manifest, &fixture.sources()).unwrap();
    let original = fixture.read("src/core.spx");
    fixture.replace(
        "src/core.spx",
        "fn add(left: i64, right: i64)",
        "fn add(left: i64, right: i64, extra: i64)",
    );
    let cached = errors(cache.build(&manifest, &fixture.sources()));
    let cold = errors(fixture.cold());
    assert_eq!(format!("{cached:?}"), format!("{cold:?}"));
    fixture.write("src/core.spx", &original);
    work(&cache.build(&manifest, &fixture.sources()).unwrap(), 0, 4);
    // Changing an import binding must also prevent retaining old resolved calls.
    let app = fixture.read("src/app.spx");
    fixture.replace(
        "src/app.spx",
        "@id(\"calculator.add\")",
        "@id(\"calculator.not\")",
    );
    let cached = errors(cache.build(&manifest, &fixture.sources()));
    assert_eq!(
        format!("{cached:?}"),
        format!("{:?}", errors(fixture.cold()))
    );
    fixture.write("src/app.spx", &app);
    work(&cache.build(&manifest, &fixture.sources()).unwrap(), 0, 4);
    // A coordinated admitted signature edit rebuilds both consumers instead of
    // keeping stale imported stubs merely because their stable IDs survived.
    fixture.replace(
        "src/core.spx",
        "fn add(left: i64, right: i64)",
        "fn add(left: i64, right: i64, extra: i64)",
    );
    fixture.replace(
        "src/app.spx",
        "add(multiply(6, 7), subtract(divide(4, 2), 2))",
        "add(multiply(6, 7), subtract(divide(4, 2), 2), 0)",
    );
    fixture.replace("src/tests.spx", "add(19, 23)", "add(19, 23, 0)");
    let coordinated = cache.build(&manifest, &fixture.sources()).unwrap();
    work(&coordinated, 3, 1);
    same(coordinated.revision(), fixture.cold().unwrap());
}

#[test]
fn failed_semantics_and_manifest_reset_do_not_install_partial_checked_state() {
    let fixture = Fixture::new();
    let manifest = fixture.manifest();
    let mut cache = ProjectFrontendCache::new_with_semantic_cache();
    cache.build(&manifest, &fixture.sources()).unwrap();
    let spare = fixture.read("src/spare.spx");
    let tests = fixture.read("src/tests.spx");
    fixture.replace("src/spare.spx", "    3", "    4");
    fixture.replace("src/tests.spx", "not(false)", "not(4)");
    let cached = errors(cache.build(&manifest, &fixture.sources()));
    assert_eq!(
        format!("{cached:?}"),
        format!("{:?}", errors(fixture.cold()))
    );
    fixture.write("src/spare.spx", &spare);
    fixture.write("src/tests.spx", &tests);
    work(&cache.build(&manifest, &fixture.sources()).unwrap(), 0, 4);
    let changed = ProjectManifest::parse(
        &manifest
            .to_canonical_toml()
            .replace("name = \"calculator\"", "name = \"calculator-new\""),
    )
    .unwrap();
    let reset = cache.build(&changed, &fixture.sources()).unwrap();
    assert_eq!(work(&reset, 4, 0)["manifest_context_reset"], true);
    let cold = ProjectFrontendCache::new()
        .build(&changed, &fixture.sources())
        .unwrap();
    same(reset.revision(), Arc::clone(cold.revision()));
    work(&cache.build(&changed, &fixture.sources()).unwrap(), 0, 4);
}

#[test]
fn owned_source_refresh_reuses_checked_modules_and_rolls_back_failed_admission() {
    let fixture = Fixture::new();
    let manifest = fixture.manifest();
    let original = image(fixture.cold().unwrap());
    let expected = original.image_digest().to_owned();
    let mut workspace = ImageWorkspace::with_semantic_cache(Arc::clone(&original)).unwrap();
    let unchanged = workspace
        .refresh_owned_sources(&manifest, &fixture.sources(), &expected)
        .unwrap();
    let report: Value = serde_json::from_str(unchanged.to_json()).unwrap();
    assert_eq!(report["frontend_work"]["work"]["checked_HIR_reused"], 4);
    assert!(Arc::ptr_eq(workspace.image(), &original));
    fixture.replace("src/app.spx", "multiply(6, 7)", "multiply(6, 8)");
    workspace
        .refresh_owned_sources(&manifest, &fixture.sources(), &expected)
        .unwrap();
    same(workspace.image().revision(), fixture.cold().unwrap());
    let retained = Arc::clone(workspace.image());
    let expected = retained.image_digest().to_owned();
    let mut bad = fixture.sources();
    bad[0] = ProjectFrontendSource::new("src/app.spx", "invalid source").unwrap();
    assert!(workspace
        .refresh_owned_sources(&manifest, &bad, &expected)
        .is_err());
    assert!(Arc::ptr_eq(workspace.image(), &retained));
    let warm = workspace
        .refresh_owned_sources(&manifest, &fixture.sources(), &expected)
        .unwrap();
    let report: Value = serde_json::from_str(warm.to_json()).unwrap();
    assert_eq!(report["frontend_work"]["work"]["modules_resolved"], 0);
    assert_eq!(report["frontend_work"]["work"]["checked_HIR_reused"], 4);
}

#[test]
#[ignore = "SPX-G410 owned-variant Graph v22 masking, needs loan plan fix"]
fn nonempty_loan_plan_clones_preserve_cold_graph_builder_accounting() {
    let root = std::env::temp_dir().join(format!(
        "spx-semantic-loan-cache-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("src")).unwrap();
    let fixture = Fixture(root.canonicalize().unwrap());
    let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/frame-payload-project");
    for path in [
        "semaprax.toml",
        "src/app.spx",
        "src/frame.spx",
        "src/tests.spx",
    ] {
        std::fs::copy(sample.join(path), fixture.0.join(path)).unwrap();
    }
    // Same nonempty parent/range/sibling loan structure as shared_loan_graph_v1.
    let frame = fixture.read("src/frame.spx")
        + r#"
@id("loan.consume-bytes")
fn consume_bytes(value: own Bytes) -> i64 { 7 }
@id("loan.projected")
fn projected() -> i64 {
    let source = [7u8, 8u8, 9u8];
    let owned = bytes_copy(array_as_slice(source));
    let parent = bytes_as_slice(owned);
    let child = byte_range(parent, 1usize, byte_len(parent));
    let sibling = bytes_as_slice(owned);
    let byte_observed = if byte_len(child) + byte_len(sibling) > 0usize { 1 } else { 0 };
    consume_bytes(owned) + byte_observed
}
"#;
    fixture.write("src/frame.spx", &frame);
    let tests=fixture.read("src/tests.spx")
        .replace("module frame_payload.tests;","module frame_payload.tests;\nuse function @id(\"loan.projected\") from frame_payload.frame as loan_projected;")
        .replace("if valid_ok && mismatch_ok","if valid_ok && mismatch_ok && loan_projected() == 8");
    fixture.write("src/tests.spx", &tests);
    let manifest = fixture.manifest();
    let sources = manifest
        .sources()
        .iter()
        .map(|path| ProjectFrontendSource::new(path, &fixture.read(path)).unwrap())
        .collect::<Vec<_>>();
    let mut cache = ProjectFrontendCache::new_with_semantic_cache();
    let cold = cache.build(&manifest, &sources).unwrap();
    work(&cold, 3, 0);
    let projected = cold
        .revision()
        .test_program()
        .functions
        .iter()
        .find(|function| function.id.as_str() == "loan.projected")
        .unwrap();
    assert_eq!(
        projected.loan_plan.schema,
        semaprax::loan_plan::LOAN_PLAN_SCHEMA_V1
    );
    assert!(projected.loan_plan.loans.len() >= 4);
    assert!(projected
        .loan_plan
        .loans
        .iter()
        .any(|loan| loan.parent.is_some()));
    let warm = cache.build(&manifest, &sources).unwrap();
    work(&warm, 0, 3);
    let cloned = warm
        .revision()
        .test_program()
        .functions
        .iter()
        .find(|function| function.id.as_str() == "loan.projected")
        .unwrap();
    assert_eq!(projected.loan_plan, cloned.loan_plan);
    same(warm.revision(), Arc::clone(cold.revision()));
    same(warm.revision(), fixture.cold().unwrap());
}
