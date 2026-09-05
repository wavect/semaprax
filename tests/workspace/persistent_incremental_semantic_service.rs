use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, ProjectFrontendSource, ProjectManifest, ProjectRevision,
    SemanticServiceIndexItemKind, SemanticServiceIndexQuery, SemanticTransaction,
    SemanticTransactionRenameDisplayName, SemanticWorkspaceService,
    SemanticWorkspaceServiceHistoryQuery,
};
use semaprax::workspace_analysis::{
    WorkspaceAnalysisTargetKind, WorkspaceContextOptions, WorkspaceImpactOptions,
};
use serde_json::{json, Value};

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
            "spx-persistent-incremental-semantic-service-v1-{}-{}",
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
        let program = semaprax::parse(
            "module calculator.spare; @id(\"calculator.spare.value\") fn value() -> i64 { 3 }",
            "src/spare.spx",
        )
        .unwrap();
        std::fs::write(
            fixture.0.join("src/spare.spx"),
            semaprax::format::canonical(&program),
        )
        .unwrap();
        fixture
    }

    fn read(&self, path: &str) -> String {
        std::fs::read_to_string(self.0.join(path)).unwrap()
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

    fn revision(&self) -> Arc<ProjectRevision> {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn inventory(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, current: &Path, entries: &mut BTreeMap<PathBuf, Vec<u8>>) {
        let mut paths = std::fs::read_dir(current)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        paths.sort();
        for path in paths {
            let relative = path.strip_prefix(root).unwrap().to_owned();
            if path.is_dir() {
                entries.insert(relative, Vec::new());
                visit(root, &path, entries);
            } else {
                entries.insert(relative, std::fs::read(path).unwrap());
            }
        }
    }
    let mut entries = BTreeMap::new();
    visit(root, root, &mut entries);
    entries
}

fn replace_source(
    sources: &[ProjectFrontendSource],
    selected: &str,
    before: &str,
    after: &str,
) -> Vec<ProjectFrontendSource> {
    sources
        .iter()
        .map(|source| {
            let text = if source.path() == selected {
                assert!(source.source().contains(before));
                let changed = source.source().replace(before, after);
                let program = semaprax::parse(&changed, selected).unwrap();
                semaprax::format::canonical(&program)
            } else {
                source.source().to_owned()
            };
            ProjectFrontendSource::new(source.path(), &text).unwrap()
        })
        .collect()
}

fn cold(manifest: &ProjectManifest, sources: &[ProjectFrontendSource]) -> Arc<ProjectRevision> {
    let mut cache = semaprax::project::ProjectFrontendCache::new_with_semantic_cache();
    cache.build(manifest, sources).unwrap().into_revision()
}

fn assert_code<T>(result: Result<T, Vec<Diagnostic>>, code: &str) {
    let errors = result.err().unwrap_or_else(|| panic!("expected {code}"));
    assert!(errors.iter().any(|error| error.code == code), "{errors:?}");
}

fn work(source: &str, resolved: u64, reused: u64) -> Value {
    let value: Value = serde_json::from_str(source).unwrap();
    assert_eq!(
        value["schema"],
        "semaprax.semantic-workspace-service-work.v1"
    );
    frontend_work(&value["frontend_work"], resolved, reused);
    value
}

fn frontend_work(value: &Value, resolved: u64, reused: u64) {
    assert_eq!(value["schema"], "semaprax.project-semantic-cache-work.v1");
    assert_eq!(value["work"]["modules_resolved"], resolved);
    assert_eq!(value["work"]["checked_HIR_reused"], reused);
    assert_eq!(value["work"]["full_source_verification"], true);
    assert_eq!(value["work"]["full_HIR_validation"], true);
    assert_eq!(value["work"]["full_link_and_profile_admission"], true);
}

fn ids(result: &semaprax::project::SemanticServiceIndexResult) -> Vec<&str> {
    result.items().iter().map(|item| item.stable_id()).collect()
}

#[test]
fn retained_indexes_answer_exact_canonical_coverage_and_effect_queries() {
    let fixture = Fixture::new();
    let service = SemanticWorkspaceService::open(fixture.revision()).unwrap();
    let revision = service.active_generation().workspace_revision();

    let coverage =
        SemanticServiceIndexQuery::tests_covering_declaration(revision, "calculator.add").unwrap();
    assert_eq!(
        SemanticServiceIndexQuery::from_json(coverage.to_json().as_bytes()).unwrap(),
        coverage
    );
    let first = service.index_query(coverage.to_json().as_bytes()).unwrap();
    let second = service.index_query(coverage.to_json().as_bytes()).unwrap();
    assert_eq!(first.to_json(), second.to_json());
    assert_eq!(first.result_digest(), second.result_digest());
    assert_eq!(ids(&first), ["calculator.tests.main"]);
    assert_eq!(
        first.items()[0].kind(),
        SemanticServiceIndexItemKind::TestMain
    );
    assert_eq!(first.query_digest(), coverage.query_digest());
    assert_eq!(first.workspace_revision(), revision);
    let snapshot = service.snapshot(revision).unwrap();
    let replayed = SemanticServiceIndexQuery::replay(
        &snapshot,
        coverage.to_json().as_bytes(),
        first.result_digest(),
        first.to_json().as_bytes(),
    )
    .unwrap();
    assert_eq!(replayed.to_json(), first.to_json());

    let no_effect =
        SemanticServiceIndexQuery::functions_reaching_effect(revision, "process.stdout.write")
            .unwrap();
    assert!(service
        .index_query(no_effect.to_json().as_bytes())
        .unwrap()
        .items()
        .is_empty());
    assert_code(
        SemanticServiceIndexQuery::from_json(coverage.to_json().trim_end().as_bytes()),
        "SPX-G528",
    );
    let unknown =
        SemanticServiceIndexQuery::tests_covering_declaration(revision, "unknown").unwrap();
    assert_code(
        service.index_query(unknown.to_json().as_bytes()),
        "SPX-G528",
    );

    let manifest = fixture.manifest();
    let mut case_sources = fixture.sources();
    let test_source = case_sources
        .iter_mut()
        .find(|source| source.path() == "src/tests.spx")
        .unwrap();
    let with_case = format!(
        "{}\n@id(\"calculator.tests.test_add\") fn test_add() -> i64 {{ if add(1, 2) == 3 {{ 0 }} else {{ 1 }} }}\n",
        test_source.source()
    );
    let canonical =
        semaprax::format::canonical(&semaprax::parse(&with_case, "src/tests.spx").unwrap());
    *test_source = ProjectFrontendSource::new("src/tests.spx", &canonical).unwrap();
    let case_service = SemanticWorkspaceService::open(cold(&manifest, &case_sources)).unwrap();
    let case_query = SemanticServiceIndexQuery::tests_covering_declaration(
        case_service.active_generation().workspace_revision(),
        "calculator.add",
    )
    .unwrap();
    let case_result = case_service
        .index_query(case_query.to_json().as_bytes())
        .unwrap();
    assert_eq!(
        ids(&case_result),
        ["calculator.tests.main", "calculator.tests.test_add"]
    );
    assert_eq!(
        case_result.items()[1].kind(),
        SemanticServiceIndexItemKind::TestCase
    );

    let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/spxgrep-project");
    let manifest =
        ProjectManifest::parse(&std::fs::read_to_string(sample.join("semaprax.toml")).unwrap())
            .unwrap();
    let sources = ["src/app.spx", "src/tests.spx"]
        .iter()
        .map(|path| {
            ProjectFrontendSource::new(path, &std::fs::read_to_string(sample.join(path)).unwrap())
                .unwrap()
        })
        .collect::<Vec<_>>();
    let effect_service = SemanticWorkspaceService::open(cold(&manifest, &sources)).unwrap();
    let query = SemanticServiceIndexQuery::functions_reaching_effect(
        effect_service.active_generation().workspace_revision(),
        "process.stdout.write",
    )
    .unwrap();
    let result = effect_service
        .index_query(query.to_json().as_bytes())
        .unwrap();
    assert_eq!(ids(&result), ["spxgrep.contains"]);
    assert_eq!(
        result.items()[0].kind(),
        SemanticServiceIndexItemKind::Function
    );
}

#[test]
fn index_refresh_is_atomic_and_old_queries_stale_while_snapshots_remain_exact() {
    let fixture = Fixture::new();
    let manifest = fixture.manifest();
    let sources = fixture.sources();
    let mut service = SemanticWorkspaceService::open(fixture.revision()).unwrap();
    let initial = service.active_generation().workspace_revision().to_owned();
    let old_snapshot = service.snapshot(&initial).unwrap();
    let query =
        SemanticServiceIndexQuery::tests_covering_declaration(&initial, "calculator.add").unwrap();
    assert_eq!(
        ids(&old_snapshot.index_query(&query).unwrap()),
        ["calculator.tests.main"]
    );

    let changed = replace_source(&sources, "src/tests.spx", "add(19, 23) == 42 && ", "");
    service
        .refresh_owned_sources(&manifest, &changed, &initial)
        .unwrap();
    assert_code(service.index_query(query.to_json().as_bytes()), "SPX-G530");
    assert_eq!(
        ids(&old_snapshot.index_query(&query).unwrap()),
        ["calculator.tests.main"]
    );

    let current = service.active_generation().workspace_revision();
    let current_query =
        SemanticServiceIndexQuery::tests_covering_declaration(current, "calculator.add").unwrap();
    assert!(service
        .index_query(current_query.to_json().as_bytes())
        .unwrap()
        .items()
        .is_empty());
}

#[test]
fn cold_open_snapshot_and_revision_bound_queries_are_exact_and_deterministic() {
    let fixture = Fixture::new();
    let before = inventory(&fixture.0);
    let revision = fixture.revision();
    let service = SemanticWorkspaceService::open(Arc::clone(&revision)).unwrap();
    let explicit = SemanticWorkspaceService::open_with_semantic_cache(
        Arc::clone(&revision),
        semaprax::project::ProjectFrontendCache::new_with_semantic_cache(),
    )
    .unwrap();
    let opened = work(service.open_work().to_json(), 4, 0);
    assert_eq!(
        opened,
        serde_json::from_str::<Value>(service.open_work().to_json()).unwrap()
    );
    assert_eq!(
        explicit.open_work().to_json(),
        service.open_work().to_json()
    );
    assert_eq!(
        explicit.open_work().receipt_digest(),
        service.open_work().receipt_digest()
    );
    let expected = service.active_generation().workspace_revision().to_owned();
    let snapshot = service.snapshot(&expected).unwrap();
    assert_eq!(snapshot.workspace_revision(), expected);
    assert_eq!(
        snapshot.generation().revision().project_revision(),
        revision.project_revision()
    );
    assert_eq!(
        snapshot.generation().canonical().to_json(),
        revision.canonical_workspace_revision().unwrap().to_json()
    );

    let symbol: Value = serde_json::from_str(&snapshot.symbol("calculator.add").unwrap()).unwrap();
    assert_eq!(symbol["symbol"]["id"], "calculator.add");
    let kind = WorkspaceAnalysisTargetKind::Declaration;
    assert_eq!(
        snapshot
            .context(kind, "calculator.add", WorkspaceContextOptions::default())
            .unwrap(),
        revision
            .semantic_context(kind, "calculator.add", WorkspaceContextOptions::default())
            .unwrap()
    );
    assert_eq!(
        snapshot
            .impact(kind, "calculator.add", WorkspaceImpactOptions::default())
            .unwrap(),
        revision
            .semantic_impact(kind, "calculator.add", WorkspaceImpactOptions::default())
            .unwrap()
    );
    assert_code(
        service.snapshot(&format!("sha256:{}", "0".repeat(64))),
        "SPX-G530",
    );
    assert_eq!(inventory(&fixture.0), before);
}

#[test]
fn refresh_is_incremental_cold_equivalent_and_rolls_back_stale_or_failed_work() {
    let fixture = Fixture::new();
    let before = inventory(&fixture.0);
    let manifest = fixture.manifest();
    let sources = fixture.sources();
    let mut service = SemanticWorkspaceService::open(fixture.revision()).unwrap();
    let initial = service.active_generation().workspace_revision().to_owned();
    let initial_project = service
        .active_generation()
        .revision()
        .project_revision()
        .to_owned();
    let old_snapshot = service.snapshot(&initial).unwrap();

    let unchanged = service
        .refresh_owned_sources(&manifest, &sources, &initial)
        .unwrap();
    let unchanged_value: Value = serde_json::from_str(unchanged.to_json()).unwrap();
    assert_eq!(
        unchanged_value["schema"],
        "semaprax.semantic-workspace-service-refresh.v1"
    );
    assert_eq!(unchanged.old_workspace_revision(), initial);
    assert_eq!(unchanged.workspace_revision(), initial);
    assert!(unchanged.generation_reused());
    frontend_work(&unchanged_value["frontend_work"], 0, 4);
    let repeated = service
        .refresh_owned_sources(&manifest, &sources, &initial)
        .unwrap();
    assert_eq!(repeated.to_json(), unchanged.to_json());
    assert_eq!(repeated.receipt_digest(), unchanged.receipt_digest());

    let changed = replace_source(&sources, "src/app.spx", "multiply(6, 7)", "multiply(6, 8)");
    let refreshed = service
        .refresh_owned_sources(&manifest, &changed, &initial)
        .unwrap();
    let refreshed_value: Value = serde_json::from_str(refreshed.to_json()).unwrap();
    assert!(!refreshed.generation_reused());
    assert_eq!(refreshed.old_workspace_revision(), initial);
    assert_eq!(
        refreshed.workspace_revision(),
        service.active_generation().workspace_revision()
    );
    assert_eq!(old_snapshot.workspace_revision(), initial);
    assert_eq!(
        old_snapshot.generation().revision().project_revision(),
        initial_project
    );
    frontend_work(&refreshed_value["frontend_work"], 1, 3);
    let independent = cold(&manifest, &changed);
    let snapshot = service
        .snapshot(service.active_generation().workspace_revision())
        .unwrap();
    assert_eq!(
        snapshot.generation().revision().project_revision(),
        independent.project_revision()
    );
    assert_eq!(
        snapshot.generation().revision().semantic_graph(),
        independent.semantic_graph()
    );
    assert_eq!(
        snapshot.generation().canonical().to_json(),
        independent
            .canonical_workspace_revision()
            .unwrap()
            .to_json()
    );

    let retained = service.active_generation().workspace_revision().to_owned();
    assert_code(
        service.refresh_owned_sources(&manifest, &sources, &initial),
        "SPX-G530",
    );
    assert_eq!(service.active_generation().workspace_revision(), retained);
    let mut invalid = replace_source(&changed, "src/app.spx", "multiply(6, 8)", "multiply(6, 9)");
    invalid[0] = ProjectFrontendSource::new("src/app.spx", "invalid source").unwrap();
    assert!(service
        .refresh_owned_sources(&manifest, &invalid, &retained)
        .is_err());
    assert_eq!(service.active_generation().workspace_revision(), retained);
    let warm = service
        .refresh_owned_sources(&manifest, &changed, &retained)
        .unwrap();
    let warm_value: Value = serde_json::from_str(warm.to_json()).unwrap();
    frontend_work(&warm_value["frontend_work"], 0, 4);
    assert_eq!(inventory(&fixture.0), before);
}

#[test]
fn transaction_validation_is_direct_exact_read_only_and_stales_after_refresh() {
    let fixture = Fixture::new();
    let before = inventory(&fixture.0);
    let manifest = fixture.manifest();
    let sources = fixture.sources();
    let base = fixture.revision();
    let mut service = SemanticWorkspaceService::open(Arc::clone(&base)).unwrap();
    let transaction = SemanticTransaction::rename_display_name(
        service.active_generation().workspace_revision(),
        SemanticTransactionRenameDisplayName::new("calculator.add", "add", "sum"),
    )
    .unwrap();
    let direct = transaction.validate(Arc::clone(&base)).unwrap();
    let through_service = service
        .validate_transaction(transaction.to_json().as_bytes())
        .unwrap();
    assert_eq!(through_service.impact(), direct.impact());
    assert_eq!(through_service.review(), direct.review());
    assert_eq!(through_service.result(), direct.result());
    assert_eq!(through_service.evidence(), direct.evidence());
    let unchanged = service.active_generation().workspace_revision().to_owned();
    assert_eq!(
        service
            .snapshot(&unchanged)
            .unwrap()
            .generation()
            .revision()
            .project_revision(),
        base.project_revision()
    );

    let changed = replace_source(&sources, "src/app.spx", "multiply(6, 7)", "multiply(6, 8)");
    service
        .refresh_owned_sources(&manifest, &changed, &unchanged)
        .unwrap();
    assert_code(
        service.validate_transaction(transaction.to_json().as_bytes()),
        "SPX-G530",
    );
    assert_eq!(inventory(&fixture.0), before);
}

#[test]
fn successful_history_is_digest_bound_revision_paged_and_failure_atomic() {
    let fixture = Fixture::new();
    let manifest = fixture.manifest();
    let sources = fixture.sources();
    let base = fixture.revision();
    let mut service = SemanticWorkspaceService::open(Arc::clone(&base)).unwrap();
    let initial = service.active_generation().workspace_revision().to_owned();
    let empty = service.history_snapshot(&initial).unwrap();
    assert!(empty.is_empty());

    let transaction = SemanticTransaction::rename_display_name(
        &initial,
        SemanticTransactionRenameDisplayName::new("calculator.add", "add", "sum"),
    )
    .unwrap();
    let artifacts = service
        .validate_transaction(transaction.to_json().as_bytes())
        .unwrap();
    let first_snapshot = service.history_snapshot(&initial).unwrap();
    assert_eq!(first_snapshot.len(), 1);

    let stale_old = SemanticTransaction::rename_display_name(
        &initial,
        SemanticTransactionRenameDisplayName::new("calculator.add", "wrong", "sum"),
    )
    .unwrap();
    assert!(service
        .validate_transaction(stale_old.to_json().as_bytes())
        .is_err());
    assert_eq!(service.history_snapshot(&initial).unwrap().len(), 1);

    let changed = replace_source(&sources, "src/app.spx", "multiply(6, 7)", "multiply(6, 8)");
    let refresh = service
        .refresh_owned_sources(&manifest, &changed, &initial)
        .unwrap();
    let current = service.active_generation().workspace_revision().to_owned();
    assert_ne!(current, initial);
    assert_eq!(first_snapshot.len(), 1);
    let first_query = SemanticWorkspaceServiceHistoryQuery::new(&initial, 0, 1).unwrap();
    assert_eq!(first_snapshot.query(&first_query).unwrap().items().len(), 1);

    let query = SemanticWorkspaceServiceHistoryQuery::new(&current, 0, 1).unwrap();
    assert_eq!(
        SemanticWorkspaceServiceHistoryQuery::from_json(query.to_json().as_bytes()).unwrap(),
        query
    );
    let page = service.history_query(query.to_json().as_bytes()).unwrap();
    assert_eq!(page.history_length(), 2);
    assert_eq!(page.items().len(), 1);
    assert_eq!(page.next_offset(), Some(1));
    let transaction_entry = &page.items()[0];
    assert_eq!(transaction_entry.kind(), "transaction_validation");
    assert_eq!(
        transaction_entry.transaction_digest(),
        Some(transaction.digest())
    );
    assert_eq!(
        transaction_entry.result_digest(),
        Some(artifacts.result_digest())
    );
    assert_eq!(transaction_entry.refresh_receipt_digest(), None);

    let second_query = SemanticWorkspaceServiceHistoryQuery::new(&current, 1, 1).unwrap();
    let second = service
        .history_query(second_query.to_json().as_bytes())
        .unwrap();
    let refresh_entry = &second.items()[0];
    assert_eq!(refresh_entry.kind(), "refresh");
    assert_eq!(
        refresh_entry.refresh_receipt_digest(),
        Some(refresh.receipt_digest())
    );
    assert_eq!(refresh_entry.base_workspace_revision(), initial);
    assert_eq!(refresh_entry.outcome_workspace_revision(), current);

    let snapshot = service.history_snapshot(&current).unwrap();
    let replay = SemanticWorkspaceServiceHistoryQuery::replay(
        &snapshot,
        second_query.to_json().as_bytes(),
        second.result_digest(),
        second.to_json().as_bytes(),
    )
    .unwrap();
    assert_eq!(replay.to_json(), second.to_json());
    assert_code(
        service.refresh_owned_sources(&manifest, &sources, &initial),
        "SPX-G530",
    );
    let invalid = changed
        .iter()
        .map(|source| {
            if source.path() == "src/app.spx" {
                ProjectFrontendSource::new(source.path(), "invalid source")
            } else {
                ProjectFrontendSource::new(source.path(), source.source())
            }
        })
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert!(service
        .refresh_owned_sources(&manifest, &invalid, &current)
        .is_err());
    assert_eq!(service.active_generation().workspace_revision(), current);
    assert_eq!(service.history_snapshot(&current).unwrap().len(), 2);

    let mut tampered: Value = serde_json::from_str(second.to_json()).unwrap();
    tampered["authority"] = json!(true);
    let mut tampered = serde_json::to_string(&tampered).unwrap();
    tampered.push('\n');
    assert_code(
        SemanticWorkspaceServiceHistoryQuery::replay(
            &snapshot,
            second_query.to_json().as_bytes(),
            second.result_digest(),
            tampered.as_bytes(),
        ),
        "SPX-G530",
    );
}
