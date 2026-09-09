//! Imported migration roots retain the same Suspend and durable accounting rules.
use super::super::migration::durable;
use super::*;
use semaprax::agent_runtime_v2::{migrate_suspended_agent_runtime_v2, AgentRuntimeV2};
use semaprax::execution_revision::typed::resume_migrated_agent_runtime_v2;

fn link(fixture: &Fixture, migration: bool) {
    let path = fixture.0.join("src/app.spx");
    let source = std::fs::read_to_string(&path).unwrap();
    let mut app = semaprax::parse(&source, &path).unwrap();
    let mut provider = semaprax::parse(
        "module fixture.agent.support; @id(\"parse.anchor\") fn anchor()->i64{0}",
        std::path::Path::new("src/support.spx"),
    )
    .unwrap();
    provider.functions.clear();
    let mut imports = String::from("module fixture.agent.lifecycle;\n");
    let mut index = 0;
    while index < app.types.len() {
        let ty = &app.types[index];
        if ty.name.starts_with("State") || ty.name == "Observation" {
            imports.push_str(&format!(
                "use type @id(\"{}\") from fixture.agent.support as {};\n",
                ty.stable_id, ty.name
            ));
            provider.types.push(app.types.remove(index));
        } else {
            index += 1;
        }
    }
    for id in ["fixture.agent.fn.observe", "fixture.agent.fn.migrate_b"] {
        if let Some(index) = app.functions.iter().position(|f| f.stable_id == id) {
            let function = app.functions.remove(index);
            let alias = if id.ends_with("migrate_b") {
                "renamed_migration"
            } else {
                "observe"
            };
            imports.push_str(&format!(
                "use function @id(\"{id}\") from fixture.agent.support as {alias};\n"
            ));
            if id.ends_with("migrate_b") {
                let mut unimported = function.clone();
                unimported.stable_id = "fixture.agent.fn.unimported".into();
                unimported.name = "unimported".into();
                provider.functions.push(unimported);
            }
            provider.functions.push(function);
        }
    }
    assert_eq!(
        migration,
        provider
            .functions
            .iter()
            .any(|f| f.stable_id.ends_with("migrate_b"))
    );
    imports.push_str("@id(\"parse.anchor\") fn anchor()->i64{0}");
    app.module_uses.extend(
        semaprax::parse(&imports, std::path::Path::new("imports.spx"))
            .unwrap()
            .module_uses
            .clone(),
    );
    std::fs::write(path, semaprax::format::canonical(&app)).unwrap();
    std::fs::write(
        fixture.0.join("src/support.spx"),
        semaprax::format::canonical(&provider),
    )
    .unwrap();
    let path = fixture.0.join("semaprax.toml");
    let manifest = std::fs::read_to_string(&path).unwrap().replace(
        "sources = [\"src/app.spx\", \"src/tests.spx\"]",
        "sources = [\"src/app.spx\", \"src/support.spx\", \"src/tests.spx\"]",
    );
    std::fs::write(path, manifest).unwrap();
}
fn runtime(fixture: &Fixture) -> AgentRuntimeV2 {
    with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        let project = snapshot.retain_revision();
        let root = project.program_root()?;
        let schema = project.linked_agent_proposal_schema("src/app.spx", "fixture.agent")?;
        let (_, deployment) = migrate_agent_definition_v1(
            project.agent_definitions()[0]
                .definition()
                .canonical_source(),
            "fixture.linked.migration",
        )?;
        let proposals: Vec<_> = (0..9)
            .map(|index| {
                proposal(
                    schema.schema().digest(),
                    "5",
                    false,
                    if index % 3 == 1 { "1" } else { "0" },
                )
            })
            .collect();
        bind_linked_agent_runtime_v2(
            project,
            ProgramRootRef::V1(&root),
            root.program_root_digest(),
            "src/app.spx",
            "fixture.agent",
            "fixture.agent.type.step",
            "fixture.agent.type.proposal.sequence",
            operations(),
            &deployment,
            LifecycleTask {
                objective: b"linked migration payload".to_vec(),
                budget: 12,
            },
            &proposals,
            IterativeBudget {
                max_iterations: 9,
                max_stages: 96,
                ..IterativeBudget::default()
            },
            EffectBudget {
                max_calls: 9,
                max_argument_bytes: 4096,
                max_result_bytes: 4096,
                max_total_bytes: 16384,
            },
        )
    })
    .unwrap()
}
fn fixtures() -> (Fixture, Fixture) {
    let previous = durable::first();
    let destination = durable::successor(&previous, "State", "StateB", "b", &["marker"], false);
    link(&previous, false);
    link(&destination, true);
    (previous, destination)
}

#[test]
fn linked_agent_imported_migration_durable_recovery_preserves_state_and_usage() {
    let (a, b) = fixtures();
    let previous = runtime(&a);
    let before = previous.execution_revision().digest().to_owned();
    let suspended = runtime(&a)
        .run_durable(
            &mut durable::handler(),
            &AgentCancellation::new(),
            None,
            &mut durable::Store::default(),
            10_000_000,
        )
        .unwrap();
    assert_eq!(
        suspended.run().run().lifecycle().status(),
        IterativeStatus::Suspend
    );
    let previous_fuel = suspended.run().usage().reserved_fuel;
    let destination = runtime(&b);
    let after = destination.execution_revision().digest().to_owned();
    let migrated = migrate_suspended_agent_runtime_v2(
        previous,
        suspended,
        destination,
        &before,
        &after,
        "fixture.agent.fn.migrate_b",
        10_000,
        10_000_000,
    )
    .unwrap();
    let handoff = migrated.handoff_digest().unwrap();
    let mut store = durable::Store::default();
    let mut host = durable::handler();
    let complete = migrated
        .run_durable(&mut host, &AgentCancellation::new(), &mut store)
        .unwrap();
    assert_eq!(host.calls.len(), 3);
    assert_eq!(complete.run().usage().calls, 6);
    assert!(complete.run().usage().reserved_fuel >= previous_fuel + 20_000);
    assert_eq!(
        complete.run().run().lifecycle().status(),
        IterativeStatus::Complete
    );
    assert!(complete
        .run()
        .run()
        .lifecycle()
        .stages()
        .iter()
        .all(|s| s.role() != "initialize"));
    let Some(RetainedValue::Record(value)) = complete.run().run().lifecycle().value() else {
        panic!("missing Result")
    };
    assert!(value
        .fields
        .iter()
        .any(|f| f.value == RetainedValue::Bytes(b"linked migration payload".to_vec())));
    assert!(value
        .fields
        .iter()
        .any(|f| f.value == RetainedValue::I64(7)));
    let fuel = complete.run().usage().reserved_fuel;
    let checkpoint = store.document.clone();
    let resumed = resume_migrated_agent_runtime_v2(
        runtime(&a),
        runtime(&b),
        &checkpoint,
        &handoff,
        &before,
        &after,
    )
    .unwrap();
    let replay = resumed
        .run_durable(&mut host, &AgentCancellation::new(), &mut store)
        .unwrap();
    assert_eq!(host.calls.len(), 3);
    assert_eq!(replay.run().run().dispatched(), 0);
    assert!(replay.run().usage().reserved_fuel > fuel);
    // The destination is rebound from changed imported source, so supplying its
    // new expected execution digest cannot authorize the older handoff.
    let provider = b.0.join("src/support.spx");
    let original = std::fs::read_to_string(&provider).unwrap();
    let drifted = original.replace("marker: 7", "marker: 8");
    assert_ne!(drifted, original);
    std::fs::write(&provider, drifted).unwrap();
    let changed = runtime(&b);
    let changed_digest = changed.execution_revision().digest().to_owned();
    assert_ne!(changed_digest, after);
    assert!(resume_migrated_agent_runtime_v2(
        runtime(&a),
        changed,
        &checkpoint,
        &handoff,
        &before,
        &changed_digest,
    )
    .is_err());
    assert_eq!(host.calls.len(), 3);
}

#[test]
fn linked_agent_migration_selection_rejects_alias_and_unimported_stable_id() {
    let (a, b) = fixtures();
    for selection in ["renamed_migration", "fixture.agent.fn.unimported"] {
        let previous = runtime(&a);
        let before = previous.execution_revision().digest().to_owned();
        let suspended = runtime(&a)
            .run_durable(
                &mut durable::handler(),
                &AgentCancellation::new(),
                None,
                &mut durable::Store::default(),
                10_000_000,
            )
            .unwrap();
        let destination = runtime(&b);
        let after = destination.execution_revision().digest().to_owned();
        assert!(migrate_suspended_agent_runtime_v2(
            previous,
            suspended,
            destination,
            &before,
            &after,
            selection,
            10_000,
            10_000_000
        )
        .is_err());
    }
}
