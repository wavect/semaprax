//! Iterative and typed service-bound runtime adapter regressions.

use std::sync::Arc;

use semaprax::agent_deployment::migrate_agent_definition_v1;
use semaprax::agent_lifecycle::iterative::effects::EffectBudget;
use semaprax::agent_lifecycle::iterative::{
    compile_source_agent_lifecycle_v2, IterativeBudget, IterativeStatus,
};
use semaprax::agent_lifecycle::{
    CheckpointStore, CheckpointStoreError, FixtureRead, LifecycleTask,
};
use semaprax::agent_runtime::AgentCancellation;
use semaprax::project::{
    SemanticWorkspaceService, WorkspaceExecutionBinding, WorkspaceExecutionRootVersion,
};

use super::super::iterative;
use super::super::typed::{operations, typed_fixture, Handler};
use super::{assert_refused, proposal};

#[derive(Default)]
struct MemoryStore {
    documents: Vec<String>,
}
impl CheckpointStore for MemoryStore {
    fn commit(&mut self, _: u64, document: &str) -> Result<(), CheckpointStoreError> {
        self.documents.push(document.to_owned());
        Ok(())
    }
}

fn iterative_inputs(binding: &WorkspaceExecutionBinding) -> (String, Vec<String>) {
    let source = &binding.project_revision().sources()[0];
    let lifecycle = compile_source_agent_lifecycle_v2(
        source.source(),
        source.path(),
        "fixture.agent",
        "fixture.agent.type.step",
    )
    .unwrap();
    let (_, deployment) = migrate_agent_definition_v1(
        binding.project_revision().agent_definitions()[0]
            .definition()
            .canonical_source(),
        "fixture.workspace.iterative",
    )
    .unwrap();
    let proposals = vec![
        proposal(
            lifecycle.proposal_schema().schema().digest(),
            "5",
            false,
            "1",
        );
        3
    ];
    (deployment, proposals)
}

fn typed_inputs(binding: &WorkspaceExecutionBinding) -> (String, Vec<String>) {
    let source = &binding.project_revision().sources()[0];
    let lifecycle = compile_source_agent_lifecycle_v2(
        source.source(),
        source.path(),
        "fixture.agent",
        "fixture.agent.type.step",
    )
    .unwrap();
    let (_, deployment) = migrate_agent_definition_v1(
        binding.project_revision().agent_definitions()[0]
            .definition()
            .canonical_source(),
        "fixture.workspace.typed",
    )
    .unwrap();
    let proposals = ["0", "1", "0"]
        .into_iter()
        .map(|selector| {
            proposal(
                lifecycle.proposal_schema().schema().digest(),
                "5",
                false,
                selector,
            )
        })
        .collect();
    (deployment, proposals)
}

fn typed_binding(
    binding: &WorkspaceExecutionBinding,
    deployment: &str,
    proposals: &[String],
) -> semaprax::project::WorkspaceExecution<semaprax::agent_runtime_v2::AgentRuntimeV2> {
    binding
        .bind_typed(
            "src/app.spx",
            "fixture.agent",
            "fixture.agent.type.step",
            "fixture.agent.type.proposal.sequence",
            operations(),
            deployment,
            LifecycleTask {
                objective: b"workspace-typed-secret".to_vec(),
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
fn workspace_iterative_binding_runs_actual_multiturn_producer() {
    let fixture = iterative::fixture();
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
        let (deployment, proposals) = iterative_inputs(&binding);
        let execution = binding.bind_iterative(
            "src/app.spx",
            "fixture.agent",
            "fixture.agent.type.step",
            &deployment,
            LifecycleTask {
                objective: b"workspace-iterative-secret".to_vec(),
                budget: 12,
            },
            &proposals,
            IterativeBudget::default(),
        )?;
        assert!(!execution
            .association_root()
            .canonical_json()
            .contains("workspace-iterative-secret"));
        let mut handler = FixtureRead::new(b"observed".to_vec());
        let evidence = execution.run(&mut handler, &AgentCancellation::new())?;
        assert_eq!(
            evidence.evidence().run().status(),
            IterativeStatus::Complete
        );
        assert_eq!(
            (evidence.evidence().run().iterations(), handler.calls()),
            (3, 3)
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
fn workspace_typed_binding_replays_durably_and_refuses_stale_execution_before_host() {
    let fixture = typed_fixture();
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
        let (deployment, proposals) = typed_inputs(&binding);
        assert_refused(binding.bind_typed(
            "src/missing.spx",
            "fixture.agent",
            "fixture.agent.type.step",
            "fixture.agent.type.proposal.sequence",
            operations(),
            &deployment,
            LifecycleTask {
                objective: b"foreign-source".to_vec(),
                budget: 12,
            },
            &proposals,
            IterativeBudget::default(),
            EffectBudget {
                max_calls: 3,
                max_argument_bytes: 4096,
                max_result_bytes: 4096,
                max_total_bytes: 8192,
            },
        ));

        let mut handler = Handler {
            calls: Vec::new(),
            wrong: false,
        };
        let evidence = typed_binding(&binding, &deployment, &proposals).run_current(
            &service,
            &mut handler,
            &AgentCancellation::new(),
        )?;
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

        let mut first = Handler {
            calls: Vec::new(),
            wrong: false,
        };
        let mut first_store = MemoryStore::default();
        let durable = typed_binding(&binding, &deployment, &proposals)
            .run_durable(
                &mut first,
                &AgentCancellation::new(),
                None,
                &mut first_store,
                10_000_000,
            )
            .unwrap();
        assert_eq!(first.calls.len(), 3);
        assert_eq!(
            durable.evidence().run().run().lifecycle().status(),
            IterativeStatus::Complete
        );
        let checkpoint = durable.evidence().run().checkpoint().to_owned();
        let mut resumed_handler = Handler {
            calls: Vec::new(),
            wrong: false,
        };
        let mut trusted_new_store = MemoryStore::default();
        let resumed = typed_binding(&binding, &deployment, &proposals)
            .run_durable(
                &mut resumed_handler,
                &AgentCancellation::new(),
                Some(&checkpoint),
                &mut trusted_new_store,
                10_000_000,
            )
            .unwrap();
        assert!(resumed_handler.calls.is_empty());
        assert_eq!(
            resumed.evidence().run().run().lifecycle().status(),
            IterativeStatus::Complete
        );

        let stale = typed_binding(&binding, &deployment, &proposals);
        let candidate_fixture = typed_fixture();
        let candidate_path = candidate_fixture.0.join("src/tests.spx");
        let candidate_source = std::fs::read_to_string(&candidate_path).unwrap();
        let changed = candidate_source.replacen("    0\n}\n", "    1\n}\n", 1);
        assert_ne!(changed, candidate_source);
        std::fs::write(&candidate_path, changed).unwrap();
        let (candidate_manifest, candidate_sources) =
            semaprax::project::with_authenticated_project(
                &candidate_fixture.0.join("semaprax.toml"),
                |candidate| {
                    let revision = candidate.retain_revision();
                    let sources = revision
                        .sources()
                        .iter()
                        .map(|source| {
                            semaprax::project::ProjectFrontendSource::new(
                                source.path(),
                                source.source(),
                            )
                            .unwrap()
                        })
                        .collect::<Vec<_>>();
                    Ok((revision.manifest().clone(), sources))
                },
            )?;
        service.refresh_owned_sources(
            &candidate_manifest,
            &candidate_sources,
            canonical.workspace_revision(),
        )?;
        let mut never = Handler {
            calls: Vec::new(),
            wrong: false,
        };
        assert_refused(stale.run_current(&service, &mut never, &AgentCancellation::new()));
        assert!(never.calls.is_empty());
        Ok(())
    })
    .unwrap();
}
