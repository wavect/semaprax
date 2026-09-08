//! Workspace-selected execution roots retain one immutable generation.

use std::sync::Arc;

use semaprax::agent_deployment::migrate_agent_definition_v1;
use semaprax::agent_lifecycle::{
    compile_source_agent_lifecycle, FixtureRead, LifecycleBudget, LifecycleStatus, LifecycleTask,
};
use semaprax::agent_runtime::AgentCancellation;
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    render_project_lock, with_authenticated_project, ExactProgramContext, ExactProgramContextV2,
    ImageArtifactKind, InterfaceArtifactFacts, ProgramRootV2, ProjectFrontendSource,
    ProjectRevision, SemanticWorkspaceService, WorkspaceExecutionBinding,
    WorkspaceExecutionRootVersion, MAX_IMAGE_ARTIFACT_BUILD_BYTES,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::{proposal, Fixture};

fn assert_refused<T>(result: Result<T, Vec<Diagnostic>>) {
    let errors = result
        .err()
        .expect("expected workspace association refusal");
    assert!(
        errors.iter().any(|error| error.code == "SPX-G583"),
        "{errors:?}"
    );
}

fn exact_contexts(
    fixture: &Fixture,
) -> (
    Arc<ProjectRevision>,
    Arc<ExactProgramContext>,
    Arc<ExactProgramContextV2>,
) {
    with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        let revision = snapshot.retain_revision();
        let workspace = revision.canonical_workspace_revision()?;
        let root = workspace.program_root()?;
        let lock = render_project_lock(snapshot)?;
        let association =
            root.associate_dependency_lock(snapshot, root.program_root_digest(), &lock)?;
        let interface = InterfaceArtifactFacts::derive(
            Arc::clone(&revision),
            revision.project_revision(),
            &[ImageArtifactKind::Npm],
            MAX_IMAGE_ARTIFACT_BUILD_BYTES,
        )?;
        let root_v2 = ProgramRootV2::derive(&workspace, &root, &interface, &association)?;
        let digest = root_v2.program_root_v2_digest().to_owned();
        let context_v1 = Arc::new(ExactProgramContext::derive(
            Arc::clone(&revision),
            revision.project_revision(),
            workspace.clone(),
            workspace.workspace_revision(),
            interface,
            association,
            root_v2,
            &digest,
        )?);
        let context_v2 = Arc::new(ExactProgramContextV2::assemble(Arc::clone(&context_v1))?);
        Ok((revision, context_v1, context_v2))
    })
    .unwrap()
}

fn runtime_inputs(project: &Arc<ProjectRevision>) -> (String, String) {
    let source = &project.sources()[0];
    let lifecycle =
        compile_source_agent_lifecycle(source.source(), source.path(), "fixture.agent").unwrap();
    let (_, deployment) = migrate_agent_definition_v1(
        project.agent_definitions()[0]
            .definition()
            .canonical_source(),
        "fixture.workspace.deployment",
    )
    .unwrap();
    let proposal = proposal(
        lifecycle.proposal_schema().schema().digest(),
        "5",
        false,
        "1",
    );
    (deployment, proposal)
}

fn run_once(binding: &WorkspaceExecutionBinding, deployment: &str, proposal: &str) {
    let execution = binding
        .bind_once(
            "src/app.spx",
            "fixture.agent",
            deployment,
            LifecycleTask {
                objective: b"workspace-invocation-secret".to_vec(),
                budget: 12,
            },
            proposal,
            LifecycleBudget::default(),
        )
        .unwrap();
    let execution_receipt = execution.association_root().canonical_json();
    assert!(!execution_receipt.contains("workspace-invocation-secret"));
    assert!(!execution_receipt.contains(proposal));
    let mut handler = FixtureRead::new(b"observed".to_vec());
    let evidence = execution
        .run(&mut handler, &AgentCancellation::new())
        .unwrap();
    assert_eq!(
        evidence.evidence().run().status(),
        LifecycleStatus::Completed
    );
    assert_eq!(handler.calls(), 1);
    for root in [
        evidence.association_root(),
        evidence.execution_association_root(),
        evidence.evidence().evidence_root(),
    ] {
        assert!(!root
            .canonical_json()
            .contains("workspace-invocation-secret"));
        assert!(!root.canonical_json().contains(proposal));
    }
}

/// This reproduces the closed `ExecutionRoot` digest construction. It proves
/// receipt replay rejects a self-consistent attacker remint, rather than only a
/// malformed JSON edit.
fn remint(mut receipt: Value) -> (String, String) {
    let schema = receipt["schema"].as_str().unwrap().to_owned();
    let facts = receipt["facts"].take();
    let mut identity = json!({"schema": schema, "facts": facts});
    let bytes = format!("{}\n", serde_json::to_string(&identity).unwrap());
    let mut hash = Sha256::new();
    hash.update(schema.as_bytes());
    hash.update([0]);
    hash.update(bytes.as_bytes());
    let digest = format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hash.finalize())
    );
    identity["digest"] = Value::String(digest.clone());
    (
        digest,
        format!("{}\n", serde_json::to_string(&identity).unwrap()),
    )
}

#[test]
fn workspace_execution_selects_replays_and_runs_each_program_root_version() {
    let fixture = Fixture::new();
    let (project, context_v1, context_v2) = exact_contexts(&fixture);
    let (deployment, proposed) = runtime_inputs(&project);

    let v1_service = SemanticWorkspaceService::open(Arc::clone(&project)).unwrap();
    let v1_canonical = project.canonical_workspace_revision().unwrap();
    let v1_workspace = v1_canonical.workspace_revision().to_owned();
    let v1_root = v1_canonical
        .program_root()
        .unwrap()
        .program_root_digest()
        .to_owned();
    let v2_service = SemanticWorkspaceService::open_exact(Arc::clone(&context_v1)).unwrap();
    let v2_workspace = context_v1
        .semantic_workspace()
        .workspace_revision()
        .to_owned();
    let v2_root = context_v1
        .program_root_v2()
        .program_root_v2_digest()
        .to_owned();
    let v3_service = SemanticWorkspaceService::open_exact_v2(Arc::clone(&context_v2)).unwrap();
    let v3_workspace = context_v2
        .exact_program_context_v1()
        .semantic_workspace()
        .workspace_revision()
        .to_owned();
    let v3_root = context_v2
        .program_root_v3()
        .program_root_v3_digest()
        .to_owned();

    for (service, version, workspace, root) in [
        (
            &v1_service,
            WorkspaceExecutionRootVersion::V1,
            &v1_workspace,
            &v1_root,
        ),
        (
            &v2_service,
            WorkspaceExecutionRootVersion::V2,
            &v2_workspace,
            &v2_root,
        ),
        (
            &v3_service,
            WorkspaceExecutionRootVersion::V3,
            &v3_workspace,
            &v3_root,
        ),
    ] {
        let binding = WorkspaceExecutionBinding::select(service, version, workspace, root).unwrap();
        assert_eq!(binding.root_version(), version);
        assert_eq!(binding.workspace_revision(), workspace);
        assert_eq!(binding.program_root().digest(), root);
        assert_eq!(
            binding.project_revision().project_revision(),
            project.project_revision()
        );
        assert!(!binding.image_digest().is_empty());
        assert!(binding
            .association_root()
            .canonical_json()
            .contains("\"authority\":false"));

        let receipt = binding.association_root().canonical_json().to_owned();
        let replay = WorkspaceExecutionBinding::replay(
            service,
            version,
            workspace,
            root,
            binding.association_root().digest(),
            &receipt,
        )
        .unwrap();
        assert_eq!(replay.association_root(), binding.association_root());
        run_once(&binding, &deployment, &proposed);

        assert_refused(WorkspaceExecutionBinding::select(
            service,
            version,
            workspace,
            "sha256:stale",
        ));
        let mutated = receipt.replacen("\"authority\":false", "\"authority\":true", 1);
        assert_ne!(mutated, receipt);
        assert_refused(WorkspaceExecutionBinding::replay(
            service,
            version,
            workspace,
            root,
            binding.association_root().digest(),
            &mutated,
        ));
        let mut forged: Value = serde_json::from_str(&receipt).unwrap();
        forged["facts"]["semantic_image"] = Value::String("sha256:forged".to_owned());
        let (forged_digest, forged_receipt) = remint(forged);
        assert_refused(WorkspaceExecutionBinding::replay(
            service,
            version,
            workspace,
            root,
            &forged_digest,
            &forged_receipt,
        ));
    }

    let restarted = SemanticWorkspaceService::open_exact_v2(Arc::clone(&context_v2)).unwrap();
    let binding = WorkspaceExecutionBinding::select(
        &v3_service,
        WorkspaceExecutionRootVersion::V3,
        &v3_workspace,
        &v3_root,
    )
    .unwrap();
    let replayed = WorkspaceExecutionBinding::replay(
        &restarted,
        WorkspaceExecutionRootVersion::V3,
        &v3_workspace,
        &v3_root,
        binding.association_root().digest(),
        binding.association_root().canonical_json(),
    )
    .unwrap();
    assert_eq!(replayed.association_root(), binding.association_root());
}

#[test]
fn workspace_execution_retains_historical_generation_across_exact_refresh() {
    let current_fixture = Fixture::new();
    let candidate_fixture = Fixture::new();
    let candidate_path = candidate_fixture.0.join("src/tests.spx");
    let candidate_source = std::fs::read_to_string(&candidate_path).unwrap();
    let changed = candidate_source.replacen("    0\n}\n", "    1\n}\n", 1);
    assert_ne!(changed, candidate_source);
    std::fs::write(&candidate_path, changed).unwrap();

    let (current_project, _, current_context) = exact_contexts(&current_fixture);
    let (_, _, candidate_context) = exact_contexts(&candidate_fixture);
    let (deployment, proposed) = runtime_inputs(&current_project);
    let old_workspace = current_context
        .exact_program_context_v1()
        .semantic_workspace()
        .workspace_revision()
        .to_owned();
    let old_root = current_context
        .program_root_v3()
        .program_root_v3_digest()
        .to_owned();
    let mut service =
        SemanticWorkspaceService::open_exact_v2(Arc::clone(&current_context)).unwrap();
    let historical = WorkspaceExecutionBinding::select(
        &service,
        WorkspaceExecutionRootVersion::V3,
        &old_workspace,
        &old_root,
    )
    .unwrap();

    let current_sources = current_project
        .sources()
        .iter()
        .map(|source| ProjectFrontendSource::new(source.path(), source.source()).unwrap())
        .collect::<Vec<_>>();
    let same = service
        .refresh_owned_sources_exact_v2(
            current_project.manifest(),
            &current_sources,
            &old_workspace,
            &old_root,
            Arc::clone(&current_context),
        )
        .unwrap();
    assert!(same.generation_reused());
    historical.require_current(&service).unwrap();

    let candidate_project = candidate_context.exact_program_context_v1().revision();
    let candidate_sources = candidate_project
        .sources()
        .iter()
        .map(|source| ProjectFrontendSource::new(source.path(), source.source()).unwrap())
        .collect::<Vec<_>>();
    let refresh = service
        .refresh_owned_sources_exact_v2(
            candidate_project.manifest(),
            &candidate_sources,
            &old_workspace,
            &old_root,
            Arc::clone(&candidate_context),
        )
        .unwrap();
    assert!(!refresh.generation_reused());
    assert_refused(historical.require_current(&service));
    assert_refused(WorkspaceExecutionBinding::select(
        &service,
        WorkspaceExecutionRootVersion::V3,
        &old_workspace,
        &old_root,
    ));

    let new_workspace = candidate_context
        .exact_program_context_v1()
        .semantic_workspace()
        .workspace_revision();
    let new_root = candidate_context.program_root_v3().program_root_v3_digest();
    let current = WorkspaceExecutionBinding::select(
        &service,
        WorkspaceExecutionRootVersion::V3,
        new_workspace,
        new_root,
    )
    .unwrap();
    assert_ne!(historical.association_root(), current.association_root());
    assert_ne!(
        historical.program_root().digest(),
        current.program_root().digest()
    );

    // A binding owns its selected immutable generation, so it still invokes the
    // old admitted lifecycle even after the active service generation changes.
    run_once(&historical, &deployment, &proposed);
}

#[path = "workspace/typed.rs"]
mod typed;
