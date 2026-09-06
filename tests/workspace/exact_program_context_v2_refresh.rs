//! Candidate-safe exact ProgramRoot-v3 refresh and rollback regressions.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    render_project_lock, with_authenticated_project, ExactProgramContext, ExactProgramContextV2,
    ImageArtifactKind, InterfaceArtifactFacts, ProgramRootV2, ProjectFrontendSource,
    ProjectManifest, SemanticServiceIndexQuery, SemanticWorkspaceRevision,
    SemanticWorkspaceService, MAX_IMAGE_ARTIFACT_BUILD_BYTES,
    SEMANTIC_WORKSPACE_SERVICE_REFRESH_SCHEMA,
};
use serde_json::Value;

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-exact-v3-refresh-{label}-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for file in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(source.join(file), root.join(file)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }

    fn replace_core(&self, before: &str, after: &str) {
        let path = self.0.join("src/core.spx");
        let source = std::fs::read_to_string(&path).unwrap();
        assert!(source.contains(before));
        let program = semaprax::parse(&source.replacen(before, after, 1), "src/core.spx").unwrap();
        std::fs::write(path, semaprax::format::canonical(&program)).unwrap();
    }

    fn context(&self) -> Arc<ExactProgramContextV2> {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            let revision = snapshot.retain_revision();
            let default_workspace = revision.canonical_workspace_revision()?;
            let base_root = default_workspace.program_root()?;
            let lock = render_project_lock(snapshot)?;
            let association = base_root.associate_dependency_lock(
                snapshot,
                base_root.program_root_digest(),
                &lock,
            )?;
            let workspace = SemanticWorkspaceRevision::derive_with_agent_definitions(
                &revision,
                revision.project_revision(),
                &[&super::program_root_v2::definition()],
            )?;
            let interface = InterfaceArtifactFacts::derive(
                Arc::clone(&revision),
                revision.project_revision(),
                &[ImageArtifactKind::Web],
                MAX_IMAGE_ARTIFACT_BUILD_BYTES,
            )?;
            let v2 = ProgramRootV2::derive(&workspace, &base_root, &interface, &association)?;
            let v2_digest = v2.program_root_v2_digest().to_owned();
            let context_v1 = ExactProgramContext::derive(
                Arc::clone(&revision),
                revision.project_revision(),
                workspace.clone(),
                workspace.workspace_revision(),
                interface,
                association,
                v2,
                &v2_digest,
            )?;
            ExactProgramContextV2::assemble(Arc::new(context_v1)).map(Arc::new)
        })
        .unwrap()
    }

    fn owned_inputs(&self) -> (ProjectManifest, Vec<ProjectFrontendSource>) {
        let context = self.context();
        let revision = context.exact_program_context_v1().revision();
        let sources = revision
            .sources()
            .iter()
            .map(|source| ProjectFrontendSource::new(source.path(), source.source()).unwrap())
            .collect();
        (revision.manifest().clone(), sources)
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

fn assert_code<T>(result: Result<T, Vec<Diagnostic>>, code: &str) {
    let errors = result.err().unwrap_or_else(|| panic!("expected {code}"));
    assert!(errors.iter().any(|error| error.code == code), "{errors:?}");
}

#[test]
fn exact_v3_refresh_matches_direct_candidate_replay_and_adopts_one_generation() {
    let current_fixture = Fixture::new("current");
    let candidate_fixture = Fixture::new("candidate");
    candidate_fixture.replace_core("left + right", "left - right");

    let current = current_fixture.context();
    let candidate = candidate_fixture.context();
    let (manifest, sources) = candidate_fixture.owned_inputs();
    let expected = ExactProgramContextV2::refresh_candidate(
        &current,
        candidate.exact_program_context_v1().revision(),
        Arc::clone(&candidate),
    )
    .unwrap();
    let old_workspace = current
        .exact_program_context_v1()
        .semantic_workspace()
        .workspace_revision()
        .to_owned();
    let old_v3 = current
        .program_root_v3()
        .program_root_v3_digest()
        .to_owned();
    let old_context_v1 = current.exact_program_context_v1().to_json().to_owned();
    let old_root_v1 = current
        .semantic_workspace_program_root_v1()
        .to_json()
        .to_owned();
    let old_root_v2 = current.program_root_v2().to_json().to_owned();
    let old_root_v3 = current.program_root_v3().to_json().to_owned();
    let before_disk = inventory(&candidate_fixture.0);

    let mut service = SemanticWorkspaceService::open_exact_v2(Arc::clone(&current)).unwrap();
    let old_snapshot = service.snapshot_exact_v2(&old_workspace, &old_v3).unwrap();
    let old_history = service
        .history_snapshot_exact_v2(&old_workspace, &old_v3)
        .unwrap();
    let receipt = service
        .refresh_owned_sources_exact_v2(
            &manifest,
            &sources,
            &old_workspace,
            &old_v3,
            Arc::clone(&candidate),
        )
        .unwrap();

    let new_workspace = expected
        .exact_program_context_v1()
        .semantic_workspace()
        .workspace_revision();
    let new_v3 = expected.program_root_v3().program_root_v3_digest();
    assert_eq!(receipt.old_workspace_revision(), old_workspace);
    assert_eq!(receipt.workspace_revision(), new_workspace);
    assert!(!receipt.generation_reused());
    let receipt_value: Value = serde_json::from_str(receipt.to_json()).unwrap();
    assert_eq!(
        receipt_value["schema"],
        SEMANTIC_WORKSPACE_SERVICE_REFRESH_SCHEMA
    );
    assert!(receipt_value.get("program_root_v3_digest").is_none());
    assert!(receipt_value.get("exact_program_context_v2").is_none());
    assert_eq!(
        service
            .active_generation()
            .exact_context_v2()
            .unwrap()
            .to_json(),
        expected.to_json()
    );
    assert_eq!(
        service.active_generation().program_root_v3(),
        Some(expected.program_root_v3())
    );
    assert!(service.snapshot_exact_v2(new_workspace, new_v3).is_ok());
    assert_code(
        service.snapshot_exact_v2(&old_workspace, &old_v3),
        "SPX-G577",
    );

    assert_eq!(old_snapshot.workspace_revision(), old_workspace);
    assert_eq!(
        old_snapshot.program_root_v3(),
        Some(current.program_root_v3())
    );
    assert!(old_history.is_empty());
    let history = service
        .history_snapshot_exact_v2(new_workspace, new_v3)
        .unwrap();
    assert_eq!(history.len(), 1);
    let history_query =
        semaprax::project::SemanticWorkspaceServiceHistoryQuery::new(new_workspace, 0, 8).unwrap();
    let history_result = history.query(&history_query).unwrap();
    let entry = &history_result.items()[0];
    assert_eq!(entry.kind(), "refresh");
    assert_eq!(entry.base_workspace_revision(), old_workspace);
    assert_eq!(entry.outcome_workspace_revision(), new_workspace);
    assert_eq!(
        entry.refresh_receipt_digest(),
        Some(receipt.receipt_digest())
    );

    assert_eq!(current.exact_program_context_v1().to_json(), old_context_v1);
    assert_eq!(
        current.semantic_workspace_program_root_v1().to_json(),
        old_root_v1
    );
    assert_eq!(current.program_root_v2().to_json(), old_root_v2);
    assert_eq!(current.program_root_v3().to_json(), old_root_v3);
    assert_eq!(inventory(&candidate_fixture.0), before_disk);

    let repeated = service
        .refresh_owned_sources_exact_v2(
            &manifest,
            &sources,
            new_workspace,
            new_v3,
            Arc::clone(&candidate),
        )
        .unwrap();
    assert!(repeated.generation_reused());
    let repeated_value: Value = serde_json::from_str(repeated.to_json()).unwrap();
    assert_eq!(
        repeated_value["frontend_work"]["work"]["modules_resolved"],
        0
    );
    assert_eq!(
        repeated_value["frontend_work"]["work"]["checked_HIR_reused"],
        3
    );
    assert_eq!(inventory(&candidate_fixture.0), before_disk);
}

#[test]
fn exact_v3_refresh_rejects_stale_cross_paired_and_invalid_candidates_atomically() {
    let current_fixture = Fixture::new("rollback-current");
    let candidate_fixture = Fixture::new("rollback-candidate");
    let cross_fixture = Fixture::new("rollback-cross");
    candidate_fixture.replace_core("left + right", "left - right");
    cross_fixture.replace_core("left + right", "left * right");

    let current = current_fixture.context();
    let candidate = candidate_fixture.context();
    let cross = cross_fixture.context();
    let (manifest, sources) = candidate_fixture.owned_inputs();
    let old_workspace = current
        .exact_program_context_v1()
        .semantic_workspace()
        .workspace_revision()
        .to_owned();
    let old_v3 = current
        .program_root_v3()
        .program_root_v3_digest()
        .to_owned();
    let stale = format!("sha256:{}", "0".repeat(64));
    let before_disk = inventory(&candidate_fixture.0);
    let mut service = SemanticWorkspaceService::open_exact_v2(Arc::clone(&current)).unwrap();
    let active = Arc::clone(service.active_generation());
    let index_query =
        SemanticServiceIndexQuery::tests_covering_declaration(&old_workspace, "calculator.add")
            .unwrap();
    let index_before = service
        .index_query(index_query.to_json().as_bytes())
        .unwrap()
        .to_json()
        .to_owned();

    assert_code(
        service.refresh_owned_sources_exact_v2(
            &manifest,
            &sources,
            &stale,
            &old_v3,
            Arc::clone(&candidate),
        ),
        "SPX-G577",
    );
    assert_code(
        service.refresh_owned_sources_exact_v2(
            &manifest,
            &sources,
            &old_workspace,
            &stale,
            Arc::clone(&candidate),
        ),
        "SPX-G577",
    );
    assert_code(
        service.refresh_owned_sources_exact_v2(
            &manifest,
            &sources,
            &old_workspace,
            &old_v3,
            Arc::clone(&cross),
        ),
        "SPX-G577",
    );

    let invalid_sources = sources
        .iter()
        .map(|source| {
            let text = if source.path() == "src/core.spx" {
                "module calculator.core; this is not valid source"
            } else {
                source.source()
            };
            ProjectFrontendSource::new(source.path(), text).unwrap()
        })
        .collect::<Vec<_>>();
    assert!(service
        .refresh_owned_sources_exact_v2(
            &manifest,
            &invalid_sources,
            &old_workspace,
            &old_v3,
            Arc::clone(&candidate),
        )
        .is_err());

    assert!(Arc::ptr_eq(service.active_generation(), &active));
    assert_eq!(
        service
            .index_query(index_query.to_json().as_bytes())
            .unwrap()
            .to_json(),
        index_before
    );
    assert_eq!(
        service
            .active_generation()
            .exact_context_v2()
            .unwrap()
            .to_json(),
        current.to_json()
    );
    assert!(service.snapshot_exact_v2(&old_workspace, &old_v3).is_ok());
    assert_eq!(
        service
            .history_snapshot_exact_v2(&old_workspace, &old_v3)
            .unwrap()
            .len(),
        0
    );
    assert_eq!(inventory(&candidate_fixture.0), before_disk);

    let receipt = service
        .refresh_owned_sources_exact_v2(&manifest, &sources, &old_workspace, &old_v3, candidate)
        .unwrap();
    let receipt_value: Value = serde_json::from_str(receipt.to_json()).unwrap();
    assert_eq!(
        receipt_value["frontend_work"]["work"]["modules_resolved"],
        1
    );
    assert_eq!(
        receipt_value["frontend_work"]["work"]["checked_HIR_reused"],
        2
    );
    assert_eq!(inventory(&candidate_fixture.0), before_disk);
}
