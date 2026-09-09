//! Workspace bindings retain imported Agent roles and reject stale helpers.

use std::sync::Arc;

use semaprax::agent_deployment::migrate_agent_definition_v1;
use semaprax::agent_lifecycle::iterative::effects::EffectBudget;
use semaprax::agent_lifecycle::iterative::{IterativeBudget, IterativeStatus};
use semaprax::agent_lifecycle::LifecycleTask;
use semaprax::agent_runtime::AgentCancellation;
use semaprax::project::{
    ProjectFrontendSource, SemanticWorkspaceService, WorkspaceExecutionBinding,
    WorkspaceExecutionRootVersion,
};

use super::*;

fn linked_inputs(binding: &WorkspaceExecutionBinding) -> (String, Vec<String>) {
    let schema = binding
        .project_revision()
        .linked_agent_proposal_schema("src/app.spx", "fixture.agent")
        .unwrap();
    let (_, deployment) = migrate_agent_definition_v1(
        binding.project_revision().agent_definitions()[0]
            .definition()
            .canonical_source(),
        "fixture.workspace.linked",
    )
    .unwrap();
    let proposals = ["0", "1", "0"]
        .into_iter()
        .map(|selector| proposal(schema.schema().digest(), "5", false, selector))
        .collect::<Vec<_>>();
    (deployment, proposals)
}

fn linked_binding(
    binding: &WorkspaceExecutionBinding,
    deployment: &str,
    proposals: &[String],
) -> semaprax::project::WorkspaceExecution<semaprax::agent_runtime_v2::AgentRuntimeV2> {
    binding
        .bind_linked_typed(
            "src/app.spx",
            "fixture.agent",
            "fixture.agent.type.step",
            "fixture.agent.type.proposal.sequence",
            operations(),
            deployment,
            LifecycleTask {
                objective: b"workspace-linked-secret".to_vec(),
                budget: 12,
            },
            proposals,
            IterativeBudget::default(),
            EffectBudget {
                max_calls: 3,
                max_argument_bytes: 4096,
                max_result_bytes: 4096,
                max_total_bytes: 8192,
            },
        )
        .unwrap()
}

#[test]
fn workspace_linked_typed_runs_imported_roles_across_three_turns() {
    let (fixture, _) = linked_fixture();
    semaprax::project::with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        let project = snapshot.retain_revision();
        let canonical = project.canonical_workspace_revision()?;
        let root = canonical.program_root()?;
        let service = SemanticWorkspaceService::open(Arc::clone(&project))?;
        let binding = WorkspaceExecutionBinding::select(
            &service,
            WorkspaceExecutionRootVersion::V1,
            canonical.workspace_revision(),
            root.program_root_digest(),
        )?;
        let (deployment, proposals) = linked_inputs(&binding);
        let execution = linked_binding(&binding, &deployment, &proposals);
        assert!(!execution
            .association_root()
            .canonical_json()
            .contains("workspace-linked-secret"));

        let mut handler = Handler {
            calls: Vec::new(),
            wrong: false,
        };
        let evidence = execution.run_current(&service, &mut handler, &AgentCancellation::new())?;
        assert_eq!(
            evidence.evidence().run().lifecycle().status(),
            IterativeStatus::Complete
        );
        assert_eq!(
            handler.calls,
            ["fixture.read", "fixture.read.second", "fixture.read"]
        );
        assert!(evidence
            .execution_association_root()
            .canonical_json()
            .contains(binding.association_root().digest()));
        assert!(evidence
            .association_root()
            .canonical_json()
            .contains(evidence.evidence().evidence_root().digest()));
        Ok(())
    })
    .unwrap();
}

#[test]
fn workspace_linked_typed_refuses_changed_imported_helper_before_host_call() {
    let (fixture, _) = linked_fixture();
    semaprax::project::with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        let project = snapshot.retain_revision();
        let canonical = project.canonical_workspace_revision()?;
        let root = canonical.program_root()?;
        let mut service = SemanticWorkspaceService::open(Arc::clone(&project))?;
        let binding = WorkspaceExecutionBinding::select(
            &service,
            WorkspaceExecutionRootVersion::V1,
            canonical.workspace_revision(),
            root.program_root_digest(),
        )?;
        let (deployment, proposals) = linked_inputs(&binding);
        let stale = linked_binding(&binding, &deployment, &proposals);

        let manifest = project.manifest().clone();
        let sources = project
            .sources()
            .iter()
            .map(|source| {
                let text = if source.path() == "src/support.spx" {
                    let changed = source
                        .source()
                        .replace("epoch: state.epoch", "epoch: state.epoch + 1");
                    assert_ne!(source.source(), changed);
                    changed
                } else {
                    source.source().to_owned()
                };
                ProjectFrontendSource::new(source.path(), &text).unwrap()
            })
            .collect::<Vec<_>>();
        service.refresh_owned_sources(&manifest, &sources, canonical.workspace_revision())?;

        let mut never = Handler {
            calls: Vec::new(),
            wrong: false,
        };
        assert!(stale
            .run_current(&service, &mut never, &AgentCancellation::new())
            .is_err());
        assert!(never.calls.is_empty());
        Ok(())
    })
    .unwrap();
}
