use super::*;
use semaprax::agent_lifecycle::iterative::{
    compile_source_agent_lifecycle_v2, IterativeBudget, IterativeStatus,
};
use semaprax::agent_lifecycle::{CheckpointStore, CheckpointStoreError};
use semaprax::agent_runtime_v2::{
    bind_agent_runtime_v2, migrate_suspended_agent_runtime_v2, AgentRuntimeV2,
};
use semaprax::execution_revision::ProgramRootRef;

#[derive(Default)]
struct MigrationStore {
    documents: Vec<String>,
}
impl CheckpointStore for MigrationStore {
    fn commit(&mut self, _: u64, document: &str) -> Result<(), CheckpointStoreError> {
        self.documents.push(document.to_owned());
        Ok(())
    }
}

fn runtime(
    fixture: &Fixture,
    objective: &[u8],
    effects: EffectBudget,
) -> std::result::Result<AgentRuntimeV2, Vec<semaprax::diagnostic::Diagnostic>> {
    with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        let project = snapshot.retain_revision();
        let root = project.program_root()?;
        let source = &project.sources()[0];
        let lifecycle = compile_source_agent_lifecycle_v2(
            source.source(),
            source.path(),
            "fixture.agent",
            "fixture.agent.type.step",
        )?;
        let (_, deployment) = migrate_agent_definition_v1(
            project.agent_definitions()[0]
                .definition()
                .canonical_source(),
            "fixture.runtime.v2",
        )?;
        let proposals: Vec<_> = ["0", "1", "0", "0", "0", "0"]
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
        bind_agent_runtime_v2(
            project.clone(),
            ProgramRootRef::V1(&root),
            root.program_root_digest(),
            source.path(),
            "fixture.agent",
            "fixture.agent.type.step",
            "fixture.agent.type.proposal.sequence",
            operations(),
            &deployment,
            LifecycleTask {
                objective: objective.to_vec(),
                budget: 12,
            },
            &proposals,
            IterativeBudget {
                max_iterations: 6,
                max_stages: 64,
                ..IterativeBudget::default()
            },
            effects,
        )
    })
}

#[test]
fn prepared_migration_consumes_actual_suspend_and_charges_old_and_new_work() {
    let old_fixture = typed_fixture();
    let old_path = old_fixture.0.join("src/app.spx");
    let old_before = std::fs::read_to_string(&old_path).unwrap();
    let old_source = old_before.replace(
        "Step::Complete { summary: state.objective, budget: state.budget, status: state.epoch }",
        "Step::Suspend { objective: state.objective, budget: state.budget, epoch: state.epoch }",
    );
    assert_ne!(old_before, old_source);
    std::fs::write(
        &old_path,
        semaprax::format::canonical(&semaprax::parse(&old_source, &old_path).unwrap()),
    )
    .unwrap();

    let new_fixture = typed_fixture();
    let new_path = new_fixture.0.join("src/app.spx");
    let mut new_source = std::fs::read_to_string(&new_path).unwrap();
    let before = new_source.clone();
    new_source = new_source
        .replace(
            "@id(\"fixture.agent.type.state\")\n        type state;",
            "@id(\"fixture.agent.type.new_state\")\n        type state;",
        )
        .replace(
            "fn initialize(task: own Task) -> State",
            "fn initialize(task: own Task) -> NewState",
        )
        .replace(
            "fn observe(state: borrow State)",
            "fn observe(state: borrow NewState)",
        )
        .replace(
            "fn authorize(state: borrow State",
            "fn authorize(state: borrow NewState",
        )
        .replace(
            "fn reduce(state: own State",
            "fn reduce(state: own NewState",
        )
        .replace(
            "    State { objective: task.objective",
            "    NewState { objective: task.objective",
        );
    new_source.push_str(
        r#"
@id("fixture.agent.type.new_state")
record NewState {
    @id("fixture.agent.type.new_state.objective") objective: Bytes,
    @id("fixture.agent.type.new_state.budget") budget: i64,
    @id("fixture.agent.type.new_state.epoch") epoch: i64,
}
@id("fixture.agent.fn.migrate")
fn migrate(old: own State) -> NewState {
    NewState { objective: old.objective, budget: old.budget, epoch: 1 }
}
"#,
    );
    assert_ne!(before, new_source);
    new_source = new_source
        .replace(r#"\"max_turns\":3"#, r#"\"max_turns\":6"#)
        .replace(r#"\"max_tool_calls\":3"#, r#"\"max_tool_calls\":6"#);
    std::fs::write(
        &new_path,
        semaprax::format::canonical(&semaprax::parse(&new_source, &new_path).unwrap()),
    )
    .unwrap();

    let effects = EffectBudget {
        max_calls: 6,
        max_argument_bytes: 4096,
        max_result_bytes: 4096,
        max_total_bytes: 16_384,
    };
    let old_for_run = runtime(&old_fixture, b"migration task", effects).unwrap();
    let old_revision = old_for_run.execution_revision().digest().to_owned();
    let old = runtime(&old_fixture, b"migration task", effects).unwrap();
    let mut old_handler = Handler {
        calls: Vec::new(),
        wrong: false,
    };
    let mut old_store = MigrationStore::default();
    let suspended = old_for_run
        .run_durable(
            &mut old_handler,
            &AgentCancellation::new(),
            None,
            &mut old_store,
            10_000_000,
        )
        .unwrap();
    assert_eq!(
        suspended.run().run().lifecycle().status(),
        IterativeStatus::Suspend
    );
    assert_eq!(old_handler.calls.len(), 3);
    let prior_usage = suspended.run().usage();
    let prior_stages = suspended.run().run().lifecycle().stages().len();

    let destination = runtime(&new_fixture, b"fresh destination input", effects).unwrap();
    let destination_revision = destination.execution_revision().digest().to_owned();
    assert_ne!(old_revision, destination_revision);
    let migration = migrate_suspended_agent_runtime_v2(
        old,
        suspended,
        destination,
        &old_revision,
        &destination_revision,
        "fixture.agent.fn.migrate",
        10_000,
        10_000_000,
    )
    .unwrap();
    assert!(migration
        .migration_root()
        .canonical_json()
        .contains("fixture.agent.fn.migrate"));

    let mut new_handler = Handler {
        calls: Vec::new(),
        wrong: false,
    };
    let evidence = migration
        .run(&mut new_handler, &AgentCancellation::new())
        .unwrap();
    assert_eq!(
        evidence.run().lifecycle().status(),
        IterativeStatus::Complete
    );
    assert_eq!(new_handler.calls.len(), 3);
    assert_eq!(evidence.usage().calls, 6);
    assert!(evidence.usage().argument_bytes > 0);
    assert!(evidence.usage().result_bytes > 0);
    assert!(evidence.usage().reserved_fuel > prior_usage.reserved_fuel);
    assert_eq!(evidence.iterations(), 6);
    assert_eq!(evidence.stages(), prior_stages + 9);
    assert_eq!(
        evidence.usage().reserved_fuel,
        prior_usage.reserved_fuel + 20_000 + 900_000
    );
    assert!(evidence
        .run()
        .lifecycle()
        .stages()
        .iter()
        .all(|stage| stage.role() != "initialize"));
    let Some(RetainedValue::Record(result)) = evidence.run().lifecycle().value() else {
        panic!("expected completed Result");
    };
    assert!(result
        .fields
        .iter()
        .any(|field| field.value == RetainedValue::Bytes(b"migration task".to_vec())));

    // A stale expected revision is rejected before the destination can run.
    let stale_old_fixture = typed_fixture();
    let stale_path = stale_old_fixture.0.join("src/app.spx");
    let stale_source = std::fs::read_to_string(&stale_path).unwrap().replace(
        "Step::Complete { summary: state.objective, budget: state.budget, status: state.epoch }",
        "Step::Suspend { objective: state.objective, budget: state.budget, epoch: state.epoch }",
    );
    std::fs::write(
        &stale_path,
        semaprax::format::canonical(&semaprax::parse(&stale_source, &stale_path).unwrap()),
    )
    .unwrap();
    let stale_runtime_for_run = runtime(&stale_old_fixture, b"migration task", effects).unwrap();
    let stale_revision = stale_runtime_for_run
        .execution_revision()
        .digest()
        .to_owned();
    let stale_runtime = runtime(&stale_old_fixture, b"migration task", effects).unwrap();
    let mut stale_store = MigrationStore::default();
    let mut stale_handler = Handler {
        calls: Vec::new(),
        wrong: false,
    };
    let stale_evidence = stale_runtime_for_run
        .run_durable(
            &mut stale_handler,
            &AgentCancellation::new(),
            None,
            &mut stale_store,
            10_000_000,
        )
        .unwrap();
    let stale_destination = runtime(&new_fixture, b"migration task", effects).unwrap();
    let stale_destination_revision = stale_destination.execution_revision().digest().to_owned();
    let error = migrate_suspended_agent_runtime_v2(
        stale_runtime,
        stale_evidence,
        stale_destination,
        "sha256:wrong",
        &stale_destination_revision,
        "fixture.agent.fn.migrate",
        10_000,
        10_000_000,
    )
    .err()
    .unwrap();
    assert!(error
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.message.contains("stale")));
    assert_eq!(stale_handler.calls.len(), 3);
    assert_eq!(stale_revision.len(), 71);

    // Previous calls already consume the destination call ceiling. The refusal
    // happens before the destination lifecycle can reach its injected host.
    let limited_old_for_run = runtime(&old_fixture, b"migration task", effects).unwrap();
    let limited_old = runtime(&old_fixture, b"migration task", effects).unwrap();
    let mut limited_store = MigrationStore::default();
    let mut limited_old_handler = Handler {
        calls: Vec::new(),
        wrong: false,
    };
    let limited_evidence = limited_old_for_run
        .run_durable(
            &mut limited_old_handler,
            &AgentCancellation::new(),
            None,
            &mut limited_store,
            10_000_000,
        )
        .unwrap();
    let limited_destination = runtime(
        &new_fixture,
        b"migration task",
        EffectBudget {
            max_calls: 3,
            ..effects
        },
    )
    .unwrap();
    let limited_revision = limited_destination.execution_revision().digest().to_owned();
    let limited = migrate_suspended_agent_runtime_v2(
        limited_old,
        limited_evidence,
        limited_destination,
        &old_revision,
        &limited_revision,
        "fixture.agent.fn.migrate",
        10_000,
        10_000_000,
    )
    .err()
    .unwrap();
    assert!(limited
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.message.contains("exhausts")));
    let failed_old = runtime(&old_fixture, b"migration task", effects).unwrap();
    let mut failed_handler = Handler {
        calls: Vec::new(),
        wrong: false,
    };
    let mut failed_store = MigrationStore::default();
    let failed_suspend = runtime(&old_fixture, b"migration task", effects)
        .unwrap()
        .run_durable(
            &mut failed_handler,
            &AgentCancellation::new(),
            None,
            &mut failed_store,
            10_000_000,
        )
        .unwrap();
    let before = failed_suspend.run().usage();
    let failed_destination = runtime(&new_fixture, b"migration task", effects).unwrap();
    let failed_revision = failed_destination.execution_revision().digest().to_owned();
    let failure = migrate_suspended_agent_runtime_v2(
        failed_old,
        failed_suspend,
        failed_destination,
        &old_revision,
        &failed_revision,
        "fixture.agent.fn.migrate",
        1,
        10_000_000,
    )
    .err()
    .expect("migration fuel must exhaust");
    assert_eq!(failure.usage().calls, before.calls);
    assert_eq!(failure.usage().reserved_fuel, before.reserved_fuel + 2);
}

#[path = "migration/durable.rs"]
pub(in crate::execution_revision) mod durable;
