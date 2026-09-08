//! Workspace-bound durable typed migration retains both selected generations.

use std::sync::Arc;

use semaprax::agent_deployment::migrate_agent_definition_v1;
use semaprax::agent_lifecycle::iterative::IterativeStatus;
use semaprax::agent_lifecycle::iterative::{
    compile_source_agent_lifecycle_v2, effects::EffectBudget, IterativeBudget,
};
use semaprax::agent_lifecycle::LifecycleTask;
use semaprax::agent_runtime::AgentCancellation;
use serde_json::Value;

use semaprax::project::{
    prepare_workspace_migration, resume_workspace_migration, SemanticWorkspaceService,
    WorkspaceExecutionBinding, WorkspaceExecutionRootVersion, WorkspaceSuspensionEvidence,
};

use super::super::proposal;
use super::super::typed::migration::durable::{first, handler, successor, Store};
use super::super::typed::operations;
use super::remint;

fn binding(
    fixture: &super::super::Fixture,
) -> (SemanticWorkspaceService, WorkspaceExecutionBinding) {
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
        Ok((service, binding))
    })
    .unwrap()
}

fn migration_inputs(binding: &WorkspaceExecutionBinding) -> (String, Vec<String>) {
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
        "fixture.workspace.migration",
    )
    .unwrap();
    let proposals = (0..9)
        .map(|index| {
            proposal(
                lifecycle.proposal_schema().schema().digest(),
                "5",
                false,
                if index % 3 == 1 { "1" } else { "0" },
            )
        })
        .collect();
    (deployment, proposals)
}

fn migration_binding(
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
                objective: b"workspace-migration".to_vec(),
                budget: 12,
            },
            proposals,
            IterativeBudget {
                max_iterations: 9,
                max_stages: 96,
                ..IterativeBudget::default()
            },
            EffectBudget {
                max_calls: 9,
                max_argument_bytes: 4096,
                max_result_bytes: 4096,
                max_total_bytes: 16_384,
            },
        )
        .unwrap()
}

#[test]
fn workspace_migration_replays_suspended_middle_generation_and_preserves_cumulative_usage() {
    let a = first();
    let b = successor(&a, "State", "StateB", "b", &["marker"], true);
    let c = successor(&b, "StateB", "StateC", "c", &["marker", "stamp"], false);
    let (_a_service, a_binding) = binding(&a);
    let (_b_service, b_binding) = binding(&b);
    let (_c_service, c_binding) = binding(&c);
    let (a_deployment, a_proposals) = migration_inputs(&a_binding);
    let (b_deployment, b_proposals) = migration_inputs(&b_binding);
    let (c_deployment, c_proposals) = migration_inputs(&c_binding);

    let mut a_host = handler();
    let mut a_store = Store::default();
    let suspended_a = migration_binding(&a_binding, &a_deployment, &a_proposals)
        .run_durable(
            &mut a_host,
            &AgentCancellation::new(),
            None,
            &mut a_store,
            10_000_000,
        )
        .unwrap();
    assert_eq!(a_host.calls.len(), 3);
    assert_eq!(
        suspended_a.evidence().run().run().lifecycle().status(),
        IterativeStatus::Suspend
    );

    let b_migration = prepare_workspace_migration(
        migration_binding(&a_binding, &a_deployment, &a_proposals),
        WorkspaceSuspensionEvidence::Ordinary(suspended_a),
        migration_binding(&b_binding, &b_deployment, &b_proposals),
        "fixture.agent.fn.migrate_b",
        10_000,
        10_000_000,
    )
    .unwrap();
    let b_handoff = b_migration.handoff_digest().unwrap();
    let b_association = b_migration.association_root().clone();
    assert!(b_association
        .canonical_json()
        .contains(a_binding.association_root().digest()));
    assert!(b_association
        .canonical_json()
        .contains(b_binding.association_root().digest()));
    assert!(!b_association.canonical_json().contains("State"));

    let mut b_host = handler();
    let mut b_store = Store::default();
    let b_suspended = b_migration
        .run_durable(&mut b_host, &AgentCancellation::new(), &mut b_store)
        .unwrap();
    assert_eq!(b_host.calls.len(), 3);
    assert_eq!(
        b_suspended.evidence().run().run().lifecycle().status(),
        IterativeStatus::Suspend
    );
    assert_eq!(b_suspended.evidence().run().usage().calls, 6);
    let retained = b_store.document.clone();
    let mut forged: Value = serde_json::from_str(b_association.canonical_json()).unwrap();
    forged["facts"]["migration_root"] = Value::String("sha256:forged".to_owned());
    let (forged_digest, forged_receipt) = remint(forged);
    assert!(resume_workspace_migration(
        migration_binding(&a_binding, &a_deployment, &a_proposals),
        migration_binding(&b_binding, &b_deployment, &b_proposals),
        &retained,
        &b_handoff,
        &forged_digest,
        &forged_receipt,
    )
    .is_err());

    let recovered = resume_workspace_migration(
        migration_binding(&a_binding, &a_deployment, &a_proposals),
        migration_binding(&b_binding, &b_deployment, &b_proposals),
        &retained,
        &b_handoff,
        b_association.digest(),
        b_association.canonical_json(),
    )
    .unwrap();
    let mut recovered_host = handler();
    let mut recovered_store = Store::default();
    let recovered_suspend = recovered
        .run_durable(
            &mut recovered_host,
            &AgentCancellation::new(),
            &mut recovered_store,
        )
        .unwrap();
    assert!(recovered_host.calls.is_empty());
    assert_eq!(
        recovered_suspend
            .evidence()
            .run()
            .run()
            .lifecycle()
            .status(),
        IterativeStatus::Suspend
    );
    assert_eq!(recovered_suspend.evidence().run().usage().calls, 6);

    let c_migration = prepare_workspace_migration(
        migration_binding(&b_binding, &b_deployment, &b_proposals),
        WorkspaceSuspensionEvidence::Migrated(recovered_suspend),
        migration_binding(&c_binding, &c_deployment, &c_proposals),
        "fixture.agent.fn.migrate_c",
        10_000,
        10_000_000,
    )
    .unwrap();
    let mut c_host = handler();
    let mut c_store = Store::default();
    let complete = c_migration
        .run_durable(&mut c_host, &AgentCancellation::new(), &mut c_store)
        .unwrap();
    assert_eq!(c_host.calls.len(), 3);
    assert_eq!(
        complete.evidence().run().run().lifecycle().status(),
        IterativeStatus::Complete
    );
    assert_eq!(complete.evidence().run().usage().calls, 9);
    assert_eq!(complete.evidence().run().iterations(), 9);
    assert!(complete.evidence().run().usage().reserved_fuel > 0);
}

fn refreshed_candidate() -> (
    semaprax::project::ProjectManifest,
    Vec<semaprax::project::ProjectFrontendSource>,
) {
    let fixture = super::super::typed::typed_fixture();
    let path = fixture.0.join("src/tests.spx");
    let source = std::fs::read_to_string(&path).unwrap();
    let changed = source.replacen("    0\n}\n", "    1\n}\n", 1);
    assert_ne!(changed, source);
    std::fs::write(&path, changed).unwrap();
    semaprax::project::with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        let revision = snapshot.retain_revision();
        let sources = revision
            .sources()
            .iter()
            .map(|source| {
                semaprax::project::ProjectFrontendSource::new(source.path(), source.source())
                    .unwrap()
            })
            .collect();
        Ok((revision.manifest().clone(), sources))
    })
    .unwrap()
}

#[test]
fn workspace_migration_current_guard_refuses_stale_destination_before_host_or_store() {
    let a = first();
    let b = successor(&a, "State", "StateB", "b", &["marker"], false);
    let (_a_service, a_binding) = binding(&a);
    let (mut b_service, b_binding) = binding(&b);
    let (a_deployment, a_proposals) = migration_inputs(&a_binding);
    let (b_deployment, b_proposals) = migration_inputs(&b_binding);
    let mut source_host = handler();
    let mut source_store = Store::default();
    let suspended = migration_binding(&a_binding, &a_deployment, &a_proposals)
        .run_durable(
            &mut source_host,
            &AgentCancellation::new(),
            None,
            &mut source_store,
            10_000_000,
        )
        .unwrap();
    let migration = prepare_workspace_migration(
        migration_binding(&a_binding, &a_deployment, &a_proposals),
        WorkspaceSuspensionEvidence::Ordinary(suspended),
        migration_binding(&b_binding, &b_deployment, &b_proposals),
        "fixture.agent.fn.migrate_b",
        10_000,
        10_000_000,
    )
    .unwrap();
    let (manifest, sources) = refreshed_candidate();
    b_service
        .refresh_owned_sources(&manifest, &sources, b_binding.workspace_revision())
        .unwrap();
    let mut never = handler();
    let mut untouched = Store::default();
    let failure = migration
        .run_durable_current(
            &b_service,
            &mut never,
            &AgentCancellation::new(),
            &mut untouched,
        )
        .err()
        .expect("stale destination ran");
    assert!(failure
        .diagnostics()
        .iter()
        .any(|error| error.code == "SPX-G583"));
    assert!(never.calls.is_empty());
    assert_eq!(untouched.commits, 0);
}

#[test]
fn workspace_migration_preserves_terminal_durable_failure_after_store_lost_acknowledgement() {
    let a = first();
    let b = successor(&a, "State", "StateB", "b", &["marker"], false);
    let (_a_service, a_binding) = binding(&a);
    let (_b_service, b_binding) = binding(&b);
    let (a_deployment, a_proposals) = migration_inputs(&a_binding);
    let (b_deployment, b_proposals) = migration_inputs(&b_binding);
    let mut source_host = handler();
    let mut source_store = Store::default();
    let suspended = migration_binding(&a_binding, &a_deployment, &a_proposals)
        .run_durable(
            &mut source_host,
            &AgentCancellation::new(),
            None,
            &mut source_store,
            10_000_000,
        )
        .unwrap();
    let mut mismatch_host = handler();
    let mut mismatch_store = Store::default();
    let mismatch_suspension = migration_binding(&a_binding, &a_deployment, &a_proposals)
        .run_durable(
            &mut mismatch_host,
            &AgentCancellation::new(),
            None,
            &mut mismatch_store,
            10_000_000,
        )
        .unwrap();
    let mismatch = prepare_workspace_migration(
        migration_binding(&b_binding, &b_deployment, &b_proposals),
        WorkspaceSuspensionEvidence::Ordinary(mismatch_suspension),
        migration_binding(&b_binding, &b_deployment, &b_proposals),
        "fixture.agent.fn.migrate_b",
        10_000,
        10_000_000,
    )
    .err()
    .expect("mismatched prior binding accepted");
    assert!(matches!(
        mismatch,
        semaprax::project::WorkspaceMigrationFailure::Association(_)
    ));

    let migration = prepare_workspace_migration(
        migration_binding(&a_binding, &a_deployment, &a_proposals),
        WorkspaceSuspensionEvidence::Ordinary(suspended),
        migration_binding(&b_binding, &b_deployment, &b_proposals),
        "fixture.agent.fn.migrate_b",
        10_000,
        10_000_000,
    )
    .unwrap();
    let mut destination_host = handler();
    let mut fault = Store {
        fail: Some("terminal"),
        ..Store::default()
    };
    let failure = migration
        .run_durable(&mut destination_host, &AgentCancellation::new(), &mut fault)
        .err()
        .expect("lost terminal acknowledgement was accepted");
    match failure {
        semaprax::project::WorkspaceMigrationFailure::Durable(rich) => {
            assert_eq!(rich.terminal().unwrap().status(), IterativeStatus::Complete);
            assert_eq!(rich.checkpoint(), fault.document);
            assert!(!rich.diagnostics().is_empty());
        }
        other => panic!("durable migration failure was flattened: {other:?}"),
    }
    assert_eq!(destination_host.calls.len(), 3);
    assert!(fault.commits > 0);
}
