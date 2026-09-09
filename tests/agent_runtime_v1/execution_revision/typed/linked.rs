//! Imported deterministic roles are retained independently of ordinary main.
use super::*;
use semaprax::agent_runtime_v2::bind_linked_agent_runtime_v2;

fn linked_fixture() -> (Fixture, String) {
    let fixture = typed_fixture();
    let path = fixture.0.join("src/app.spx");
    let original = std::fs::read_to_string(&path).unwrap();
    let lifecycle = compile_source_agent_lifecycle_v2(
        &original,
        &path,
        "fixture.agent",
        "fixture.agent.type.step",
    )
    .unwrap();
    let schema = lifecycle.proposal_schema().schema().digest().to_owned();
    let mut app = semaprax::parse(&original, &path).unwrap();
    let mut provider = semaprax::parse(
        "module fixture.agent.support;\n@id(\"fixture.support.main\") fn main()->i64{0}\n",
        std::path::Path::new("src/support.spx"),
    )
    .unwrap();
    provider.functions.clear();
    for id in ["fixture.agent.type.state", "fixture.agent.type.observation"] {
        let index = app.types.iter().position(|ty| ty.stable_id == id).unwrap();
        provider.types.push(app.types.remove(index));
    }
    let index = app
        .functions
        .iter()
        .position(|function| function.stable_id == "fixture.agent.fn.observe")
        .unwrap();
    provider.functions.push(app.functions.remove(index));
    let imports = semaprax::parse(
        r#"module fixture.agent.lifecycle;
use type @id("fixture.agent.type.state") from fixture.agent.support as State;
use type @id("fixture.agent.type.observation") from fixture.agent.support as Observation;
use function @id("fixture.agent.fn.observe") from fixture.agent.support as observe;
@id("unused.parse.anchor") fn main()->i64{0}
"#,
        std::path::Path::new("imports.spx"),
    )
    .unwrap();
    app.module_uses.extend(imports.module_uses.clone());
    std::fs::write(&path, semaprax::format::canonical(&app)).unwrap();
    std::fs::write(
        fixture.0.join("src/support.spx"),
        semaprax::format::canonical(&provider),
    )
    .unwrap();
    let manifest = fixture.0.join("semaprax.toml");
    let text = std::fs::read_to_string(&manifest).unwrap().replace(
        "sources = [\"src/app.spx\", \"src/tests.spx\"]",
        "sources = [\"src/app.spx\", \"src/support.spx\", \"src/tests.spx\"]",
    );
    std::fs::write(manifest, text).unwrap();
    (fixture, schema)
}

fn bind(
    project: std::sync::Arc<semaprax::project::ProjectRevision>,
    root: &semaprax::project::ProgramRoot,
    schema: &str,
    agent: &str,
) -> Result<semaprax::agent_runtime_v2::AgentRuntimeV2, Vec<semaprax::diagnostic::Diagnostic>> {
    let (_, deployment) = migrate_agent_definition_v1(
        project.agent_definitions()[0]
            .definition()
            .canonical_source(),
        "fixture.linked.runtime",
    )?;
    let proposals = ["0", "1", "0"].map(|selector| proposal(schema, "5", false, selector));
    bind_linked_agent_runtime_v2(
        project,
        ProgramRootRef::V1(root),
        root.program_root_digest(),
        "src/app.spx",
        agent,
        "fixture.agent.type.step",
        "fixture.agent.type.proposal.sequence",
        operations(),
        &deployment,
        LifecycleTask {
            objective: b"linked task".to_vec(),
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
    )
}

#[test]
fn linked_agent_roles_execute_imported_observe_across_three_turns() {
    let (fixture, schema) = linked_fixture();
    with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        let project = snapshot.retain_revision();
        let root = project.program_root()?;
        assert!(!project
            .entry_program()
            .functions
            .iter()
            .any(|function| function.id.as_str() == "fixture.agent.fn.observe"));
        let runtime = bind(project.clone(), &root, &schema, "fixture.agent")?;
        let repeated = bind(project.clone(), &root, &schema, "fixture.agent")?;
        assert_eq!(runtime.execution_revision(), repeated.execution_revision());
        assert!(bind(project, &root, &schema, "foreign.agent").is_err());
        let mut handler = Handler {
            calls: Vec::new(),
            wrong: false,
        };
        let evidence = runtime.run(&mut handler, &AgentCancellation::new())?;
        assert_eq!(
            evidence.run().lifecycle().status(),
            IterativeStatus::Complete
        );
        assert_eq!(
            handler.calls,
            ["fixture.read", "fixture.read.second", "fixture.read"]
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn linked_agent_imported_body_drift_changes_roots_and_rejects_stale_pair() {
    let (fixture, schema) = linked_fixture();
    let previous = with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        Ok(snapshot.retain_revision())
    })
    .unwrap();
    let previous_root = previous.program_root().unwrap();
    let before = bind(previous.clone(), &previous_root, &schema, "fixture.agent").unwrap();
    let path = fixture.0.join("src/support.spx");
    let text = std::fs::read_to_string(&path).unwrap();
    let changed = text.replace("epoch: state.epoch", "epoch: state.epoch + 1");
    assert_ne!(text, changed);
    std::fs::write(path, changed).unwrap();
    with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        let current = snapshot.retain_revision();
        let root = current.program_root()?;
        assert!(bind(current.clone(), &previous_root, &schema, "fixture.agent").is_err());
        let after = bind(current, &root, &schema, "fixture.agent")?;
        assert_ne!(before.deployment_root(), after.deployment_root());
        // Historical immutable selection remains usable, but never substitutes
        // for a different retained Project's current source-owned segments.
        assert!(bind(previous.clone(), &previous_root, &schema, "fixture.agent").is_ok());
        Ok(())
    })
    .unwrap();
}

#[test]
fn linked_agent_standard_package_roles_execute_inside_typed_runtime() {
    let fixture = typed_fixture();
    let path = fixture.0.join("src/app.spx");
    let original = std::fs::read_to_string(&path).unwrap();
    let mut app = semaprax::parse(&original, &path).unwrap();
    app.types.retain(|ty| {
        ![
            "fixture.agent.type.task",
            "fixture.agent.type.state",
            "fixture.agent.type.observation",
            "fixture.agent.type.outcome",
        ]
        .contains(&ty.stable_id.as_str())
    });
    app.functions.retain(|function| {
        !["fixture.agent.fn.initialize", "fixture.agent.fn.observe"]
            .contains(&function.stable_id.as_str())
    });
    let imports = semaprax::parse(
        r#"module fixture.agent.lifecycle;
use type @id("std.agent.task") from std.agent as Task;
use type @id("std.agent.context") from std.agent as State;
use type @id("std.agent.observation") from std.agent as Observation;
use type @id("std.agent.outcome") from std.agent as Outcome;
use function @id("std.agent.initialize") from std.agent as initialize;
use function @id("std.agent.observe") from std.agent as observe;
use function @id("std.agent.advance") from std.agent as advance;
use function @id("std.agent.outcome-bytes") from std.agent as outcome_bytes;
use function @id("std.agent.outcome-status") from std.agent as outcome_status;
@id("unused.parse.anchor") fn main()->i64{0}
"#,
        std::path::Path::new("imports.spx"),
    )
    .unwrap();
    app.module_uses.extend(imports.module_uses.clone());
    let mut reducer = semaprax::parse(r#"module fixture.agent.lifecycle;
@id("fixture.agent.fn.reduce")
fn reduce(state: own State, budget: i64, urgent: bool, sequence: usize, outcome: own Outcome)->Step {
 let status = outcome_status(outcome);
 let payload = outcome_bytes(outcome);
 let next = advance(state);
 if next.epoch < 3 {
  Step::Continue {objective:next.objective,budget:next.budget,epoch:next.epoch}
 } else {
  Step::Complete {summary:next.objective,budget:next.budget,status:next.epoch}
 }
}
"#, std::path::Path::new("reducer.spx")).unwrap();
    let position = app
        .functions
        .iter()
        .position(|function| function.stable_id == "fixture.agent.fn.reduce")
        .unwrap();
    app.functions[position] = reducer.functions.remove(0);
    let mut source = semaprax::format::canonical(&app);
    for (old, new) in [
        ("fixture.agent.type.task", "std.agent.task"),
        ("fixture.agent.type.state", "std.agent.context"),
        ("fixture.agent.type.observation", "std.agent.observation"),
        ("fixture.agent.type.outcome", "std.agent.outcome"),
        ("fixture.agent.fn.initialize", "std.agent.initialize"),
        ("fixture.agent.fn.observe", "std.agent.observe"),
    ] {
        source = source.replace(old, new);
    }
    let parsed = semaprax::parse(&source, &path).unwrap();
    std::fs::write(&path, semaprax::format::canonical(&parsed)).unwrap();
    std::fs::write(
        fixture.0.join("semaprax.toml"),
        r#"schema = "semaprax.manifest.v1"

[package]
name = "fixture"
version = "1.0.0"
profile = "nested-owned-record-api.v1"

[modules]
entry = "fixture.agent.lifecycle"
sources = ["src/app.spx", "src/tests.spx"]
tests = ["fixture.tests"]

[exports]
web = ["fixture.export.build"]

[dependencies]
std.agent = "=0.1.0"
"#,
    )
    .unwrap();
    with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        let project = snapshot.retain_revision();
        let root = project.program_root()?;
        let schema = project.linked_agent_proposal_schema("src/app.spx", "fixture.agent")?;
        let runtime = bind(project, &root, schema.schema().digest(), "fixture.agent")?;
        let mut handler = Handler {
            calls: Vec::new(),
            wrong: false,
        };
        let evidence = runtime.run(&mut handler, &AgentCancellation::new())?;
        assert_eq!(
            evidence.run().lifecycle().status(),
            IterativeStatus::Complete
        );
        assert_eq!(
            handler.calls,
            ["fixture.read", "fixture.read.second", "fixture.read"]
        );
        let stages = evidence.run().lifecycle().stages();
        assert!(stages
            .iter()
            .any(|stage| stage.function_id() == "std.agent.initialize"));
        assert_eq!(
            stages
                .iter()
                .filter(|stage| stage.function_id() == "std.agent.observe")
                .count(),
            3
        );
        Ok(())
    })
    .unwrap();
}

#[derive(Default)]
struct LinkedStore {
    documents: Vec<String>,
    fail_kind: Option<&'static str>,
}
impl semaprax::agent_lifecycle::CheckpointStore for LinkedStore {
    fn commit(
        &mut self,
        _: u64,
        document: &str,
    ) -> Result<(), semaprax::agent_lifecycle::CheckpointStoreError> {
        self.documents.push(document.to_owned());
        let value: serde_json::Value = serde_json::from_str(document).unwrap();
        let last = value["entries"].as_array().unwrap().last().unwrap();
        if self
            .fail_kind
            .is_some_and(|kind| last["event"]["kind"] == kind)
        {
            Err(semaprax::agent_lifecycle::CheckpointStoreError)
        } else {
            Ok(())
        }
    }
}

#[test]
fn linked_agent_checkpoints_preserve_intent_and_observation_recovery() {
    let (fixture, schema) = linked_fixture();
    with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        let project = snapshot.retain_revision();
        let root = project.program_root()?;
        for kind in ["intent", "observed"] {
            let mut store = LinkedStore {
                fail_kind: Some(kind),
                ..Default::default()
            };
            let mut handler = Handler {
                calls: Vec::new(),
                wrong: false,
            };
            let failure = bind(project.clone(), &root, &schema, "fixture.agent")?
                .run_durable(
                    &mut handler,
                    &AgentCancellation::new(),
                    None,
                    &mut store,
                    10_000_000,
                )
                .err()
                .expect("lost acknowledgement stops the producer");
            assert_eq!(handler.calls.len(), usize::from(kind == "observed"));
            let trusted = store.documents.last().unwrap().clone();
            assert_eq!(failure.checkpoint(), trusted);
            let mut recovered_store = LinkedStore::default();
            let mut tail = Handler {
                calls: Vec::new(),
                wrong: false,
            };
            let recovered = bind(project.clone(), &root, &schema, "fixture.agent")?.run_durable(
                &mut tail,
                &AgentCancellation::new(),
                Some(&trusted),
                &mut recovered_store,
                10_000_000,
            );
            if kind == "intent" {
                assert!(recovered.is_err());
                assert!(tail.calls.is_empty() && recovered_store.documents.is_empty());
            } else {
                let recovered = recovered.unwrap();
                assert_eq!(tail.calls, ["fixture.read.second", "fixture.read"]);
                assert_eq!(recovered.run().usage().calls, 3);
                assert_eq!(
                    recovered.run().run().lifecycle().status(),
                    IterativeStatus::Complete
                );
                let mut replay_store = LinkedStore::default();
                let mut never = Handler {
                    calls: Vec::new(),
                    wrong: false,
                };
                let replayed = bind(project.clone(), &root, &schema, "fixture.agent")?
                    .run_durable(
                        &mut never,
                        &AgentCancellation::new(),
                        Some(recovered.checkpoint()),
                        &mut replay_store,
                        10_000_000,
                    )
                    .unwrap();
                assert!(never.calls.is_empty());
                assert_eq!(replayed.run().usage().calls, 3);
                assert!(
                    replayed.run().usage().reserved_fuel > recovered.run().usage().reserved_fuel
                );
            }
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn linked_agent_suspend_is_terminal_data_and_replays_without_dispatch() {
    let (fixture, schema) = linked_fixture();
    let path = fixture.0.join("src/app.spx");
    let text = std::fs::read_to_string(&path).unwrap();
    let changed = text.replace("Step::Continue", "Step::Suspend");
    assert_ne!(text, changed);
    std::fs::write(path, changed).unwrap();
    with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        let project = snapshot.retain_revision();
        let root = project.program_root()?;
        let mut store = LinkedStore::default();
        let mut first = Handler {
            calls: Vec::new(),
            wrong: false,
        };
        let suspended = bind(project.clone(), &root, &schema, "fixture.agent")?
            .run_durable(
                &mut first,
                &AgentCancellation::new(),
                None,
                &mut store,
                10_000_000,
            )
            .unwrap();
        assert_eq!(first.calls, ["fixture.read"]);
        assert_eq!(
            suspended.run().run().lifecycle().status(),
            IterativeStatus::Suspend
        );
        let mut never = Handler {
            calls: Vec::new(),
            wrong: false,
        };
        let replayed = bind(project, &root, &schema, "fixture.agent")?
            .run_durable(
                &mut never,
                &AgentCancellation::new(),
                Some(suspended.checkpoint()),
                &mut store,
                10_000_000,
            )
            .unwrap();
        assert!(never.calls.is_empty());
        assert_eq!(
            replayed.run().run().lifecycle().status(),
            IterativeStatus::Suspend
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn linked_agent_migration_cannot_fall_back_to_isolated_source() {
    let (old_fixture, schema) = linked_fixture();
    let old_path = old_fixture.0.join("src/app.spx");
    let source = std::fs::read_to_string(&old_path).unwrap();
    std::fs::write(old_path, source.replace("Step::Continue", "Step::Suspend")).unwrap();
    let old = with_authenticated_project(&old_fixture.0.join("semaprax.toml"), |snapshot| {
        Ok(snapshot.retain_revision())
    })
    .unwrap();
    let old_root = old.program_root().unwrap();
    let previous = bind(old.clone(), &old_root, &schema, "fixture.agent").unwrap();
    let previous_revision = previous.execution_revision().digest().to_owned();
    let mut handler = Handler {
        calls: Vec::new(),
        wrong: false,
    };
    let suspended = bind(old, &old_root, &schema, "fixture.agent")
        .unwrap()
        .run_durable(
            &mut handler,
            &AgentCancellation::new(),
            None,
            &mut LinkedStore::default(),
            10_000_000,
        )
        .unwrap();
    let usage = suspended.run().usage();
    assert_eq!(
        suspended.run().run().lifecycle().status(),
        IterativeStatus::Suspend
    );
    let (new_fixture, new_schema) = linked_fixture();
    let new = with_authenticated_project(&new_fixture.0.join("semaprax.toml"), |snapshot| {
        Ok(snapshot.retain_revision())
    })
    .unwrap();
    let new_root = new.program_root().unwrap();
    assert_ne!(
        old_root.program_root_digest(),
        new_root.program_root_digest()
    );
    let destination = bind(new, &new_root, &new_schema, "fixture.agent").unwrap();
    let destination_revision = destination.execution_revision().digest().to_owned();
    let failure = semaprax::agent_runtime_v2::migrate_suspended_agent_runtime_v2(
        previous,
        suspended,
        destination,
        &previous_revision,
        &destination_revision,
        "fixture.agent.fn.migrate",
        10_000,
        10_000_000,
    )
    .err()
    .expect("linked migration requires a separately authenticated migration root");
    assert!(failure.diagnostics().iter().any(|diagnostic| diagnostic
        .message
        .contains("linked Agent migration function is not declared or explicitly imported by selected source")));
    assert_eq!(failure.usage(), usage);
    assert_eq!(handler.calls, ["fixture.read"]);
}

#[path = "linked/workspace.rs"]
mod workspace;

#[path = "linked/migration.rs"]
mod linked_migration;
