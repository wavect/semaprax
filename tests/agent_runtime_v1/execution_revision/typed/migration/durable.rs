use super::*;
use semaprax::execution_revision::typed::resume_migrated_agent_runtime_v2;

const COMPLETE: &str =
    "Step::Complete { summary: state.objective, budget: state.budget, status: state.epoch }";
const SUSPEND: &str =
    "Step::Suspend { objective: state.objective, budget: state.budget, epoch: state.epoch }";

fn first() -> Fixture {
    let fixture = typed_fixture();
    let path = fixture.0.join("src/app.spx");
    let original = std::fs::read_to_string(&path).unwrap();
    let source = original.replace(COMPLETE, SUSPEND);
    assert_ne!(source, original);
    write(&fixture, &source);
    fixture
}
fn write(fixture: &Fixture, source: &str) {
    let path = fixture.0.join("src/app.spx");
    std::fs::write(
        &path,
        semaprax::format::canonical(&semaprax::parse(source, &path).unwrap()),
    )
    .unwrap();
}
/// Preserve all preceding State declarations; replace only current role bodies
/// and Step, then append the exact successor record and pure migration.
fn successor(
    previous: &Fixture,
    from: &str,
    to: &str,
    letter: &str,
    extras: &[&str],
    suspend: bool,
) -> Fixture {
    let fixture = typed_fixture();
    let source = std::fs::read_to_string(previous.0.join("src/app.spx"))
        .unwrap()
        .replace(
            &format!("fn observe(state: borrow {from})"),
            &format!("fn observe(state: borrow {to})"),
        )
        .replace(
            &format!("fn authorize(state: borrow {from},"),
            &format!("fn authorize(state: borrow {to},"),
        );
    let mut program = semaprax::parse(&source, "src/app.spx").unwrap();
    program.agents[0]
        .types
        .iter_mut()
        .find(|r| r.role == semaprax::ast::AgentTypeRole::State)
        .unwrap()
        .stable_id = format!("fixture.agent.type.state_{letter}");
    let calls = if letter == "b" { 6 } else { 9 };
    for key in ["max_turns", "max_tool_calls"] {
        for before in [3, 6] {
            program.agents[0].runtime_v1_json = program.agents[0].runtime_v1_json.replace(
                &format!("\"{key}\":{before}"),
                &format!("\"{key}\":{calls}"),
            );
        }
    }
    let record_fields = extras
        .iter()
        .map(|name| format!("@id(\"fixture.agent.type.state_{letter}.{name}\") {name}: i64,"))
        .collect::<String>();
    let init_extra = extras
        .iter()
        .map(|name| format!(", {name}: 100"))
        .collect::<String>();
    let carried = extras
        .iter()
        .map(|name| format!(", {name}: state.{name}"))
        .collect::<String>();
    let migration_extra = extras
        .iter()
        .enumerate()
        .map(|(index, name)| {
            if index + 1 == extras.len() {
                format!(", {name}: {}", if letter == "b" { 7 } else { 9 })
            } else {
                format!(", {name}: old.{name}")
            }
        })
        .collect::<String>();
    let case_fields = |case: &str| {
        extras
            .iter()
            .map(|name| format!("@id(\"fixture.agent.step.{case}.{name}\") {name}: i64,"))
            .collect::<String>()
    };
    let result_status = extras
        .iter()
        .map(|name| format!("state.{name}"))
        .collect::<Vec<_>>()
        .join(" + ");
    let terminal = if suspend {
        format!("Step::Suspend {{ objective: state.objective, budget: state.budget, epoch: state.epoch{carried} }}")
    } else {
        format!("Step::Complete {{ summary: state.objective, budget: state.budget, status: {result_status} }}")
    };
    let addition = format!(
        r#"module fixture.agent.lifecycle;
@id("fixture.agent.type.state_{letter}")
record {to} {{
    @id("fixture.agent.type.state_{letter}.objective") objective: Bytes,
    @id("fixture.agent.type.state_{letter}.budget") budget: i64,
    @id("fixture.agent.type.state_{letter}.epoch") epoch: i64,
    {record_fields}
}}
@id("fixture.agent.type.step")
variant Step {{
    @id("fixture.agent.step.continue") Continue {{
        @id("fixture.agent.step.continue.objective") objective: Bytes,
        @id("fixture.agent.step.continue.budget") budget: i64,
        @id("fixture.agent.step.continue.epoch") epoch: i64,
        {continue_fields}
    }},
    @id("fixture.agent.step.complete") Complete {{
        @id("fixture.agent.step.complete.summary") summary: Bytes,
        @id("fixture.agent.step.complete.budget") budget: i64,
        @id("fixture.agent.step.complete.status") status: i64,
    }},
    @id("fixture.agent.step.suspend") Suspend {{
        @id("fixture.agent.step.suspend.objective") objective: Bytes,
        @id("fixture.agent.step.suspend.budget") budget: i64,
        @id("fixture.agent.step.suspend.epoch") epoch: i64,
        {suspend_fields}
    }},
    @id("fixture.agent.step.fail") Fail {{ @id("fixture.agent.step.fail.code") code: i64, }},
}}
@id("fixture.agent.fn.initialize")
fn initialize(task: own Task) -> {to} {{
    {to} {{ objective: task.objective, budget: task.budget, epoch: 1{init_extra} }}
}}
@id("fixture.agent.fn.reduce")
fn reduce(state: own {to}, budget: i64, urgent: bool, sequence: usize, outcome: own Outcome) -> Step {{
    if state.epoch < 3 {{
        Step::Continue {{ objective: state.objective, budget: state.budget, epoch: state.epoch + 1{carried} }}
    }} else {{ {terminal} }}
}}
@id("fixture.agent.fn.migrate_{letter}")
fn migrate_{letter}(old: own {from}) -> {to} {{
    {to} {{ objective: old.objective, budget: old.budget, epoch: 1{migration_extra} }}
}}
"#,
        continue_fields = case_fields("continue"),
        suspend_fields = case_fields("suspend")
    );
    let mut addition = semaprax::parse(&addition, "src/app.spx").unwrap();
    for ty in std::mem::take(&mut addition.types) {
        if let Some(index) = program
            .types
            .iter()
            .position(|old| old.stable_id == ty.stable_id)
        {
            program.types[index] = ty;
        } else {
            program.types.push(ty);
        }
    }
    for function in std::mem::take(&mut addition.functions) {
        if let Some(index) = program
            .functions
            .iter()
            .position(|old| old.stable_id == function.stable_id)
        {
            program.functions[index] = function;
        } else {
            program.functions.push(function);
        }
    }
    write(&fixture, &semaprax::format::canonical(&program));
    fixture
}
fn effects() -> EffectBudget {
    EffectBudget {
        max_calls: 9,
        max_argument_bytes: 4096,
        max_result_bytes: 4096,
        max_total_bytes: 16384,
    }
}
fn bind(fixture: &Fixture, objective: &[u8]) -> AgentRuntimeV2 {
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
        let proposals: Vec<_> = (0..9)
            .map(|index| {
                proposal(
                    lifecycle.proposal_schema().schema().digest(),
                    "5",
                    false,
                    if index % 3 == 1 { "1" } else { "0" },
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
                max_iterations: 9,
                max_stages: 96,
                ..IterativeBudget::default()
            },
            effects(),
        )
    })
    .unwrap()
}
fn handler() -> Handler {
    Handler {
        calls: Vec::new(),
        wrong: false,
    }
}
#[derive(Default)]
struct Store {
    document: String,
    commits: usize,
    fail: Option<&'static str>,
}
impl CheckpointStore for Store {
    fn commit(&mut self, generation: u64, document: &str) -> Result<(), CheckpointStoreError> {
        self.commits += 1;
        self.document = document.into();
        let outer: serde_json::Value = serde_json::from_str(document).unwrap();
        let checkpoint = outer
            .get("checkpoint")
            .and_then(serde_json::Value::as_str)
            .map(|s| serde_json::from_str::<serde_json::Value>(s).unwrap());
        let inner = checkpoint.as_ref().unwrap_or(&outer);
        let last = inner["entries"].as_array().and_then(|rows| rows.last());
        let kind = last.and_then(|row| row["event"]["kind"].as_str());
        let terminal = last.and_then(|row| row["event"]["transition"].as_str());
        let fail = self.fail.is_some_and(|wanted| {
            if wanted == "genesis" {
                generation == 0
            } else if wanted == "terminal" {
                kind == Some("transition") && matches!(terminal, Some("Complete" | "Suspend"))
            } else {
                kind == Some(wanted)
            }
        });
        if fail {
            self.fail = None;
            Err(CheckpointStoreError)
        } else {
            Ok(())
        }
    }
}
fn migrated(
    a: &Fixture,
    b: &Fixture,
) -> (
    semaprax::execution_revision::typed::MigratedAgentRuntimeV2,
    String,
    String,
) {
    let previous = bind(a, b"chain payload");
    let before = previous.execution_revision().digest().to_owned();
    let suspended = bind(a, b"chain payload")
        .run_durable(
            &mut handler(),
            &AgentCancellation::new(),
            None,
            &mut Store::default(),
            10_000_000,
        )
        .unwrap();
    assert_eq!(
        suspended.run().run().lifecycle().status(),
        IterativeStatus::Suspend
    );
    let destination = bind(b, b"destination input ignored");
    let after = destination.execution_revision().digest().to_owned();
    (
        migrate_suspended_agent_runtime_v2(
            previous,
            suspended,
            destination,
            &before,
            &after,
            "fixture.agent.fn.migrate_b",
            10_000,
            10_000_000,
        )
        .unwrap(),
        before,
        after,
    )
}

#[test]
fn migrated_durable_complete_and_full_replay_preserve_payload_and_charges() {
    let a = first();
    let b = successor(&a, "State", "StateB", "b", &["marker"], false);
    let (migration, before, after) = migrated(&a, &b);
    let handoff = migration.handoff_digest().unwrap();
    let mut host = handler();
    let mut store = Store::default();
    let complete = migration
        .run_durable(&mut host, &AgentCancellation::new(), &mut store)
        .unwrap();
    assert_eq!(
        complete.run().run().lifecycle().status(),
        IterativeStatus::Complete
    );
    assert_eq!(host.calls.len(), 3);
    assert_eq!(
        (
            complete.run().usage().calls,
            complete.run().iterations(),
            complete.run().stages()
        ),
        (6, 6, 19)
    );
    assert_eq!(complete.migration_handoff_digest(), Some(handoff.as_str()));
    assert_eq!(complete.checkpoint(), store.document);
    assert!(complete
        .run()
        .run()
        .lifecycle()
        .stages()
        .iter()
        .all(|s| s.role() != "initialize"));
    let Some(RetainedValue::Record(value)) = complete.run().run().lifecycle().value() else {
        panic!("missingResult")
    };
    assert!(value
        .fields
        .iter()
        .any(|f| f.value == RetainedValue::Bytes(b"chain payload".to_vec())));
    assert!(value
        .fields
        .iter()
        .any(|f| f.value == RetainedValue::I64(7)));
    let retained = store.document.clone();
    let fuel = complete.run().usage().reserved_fuel;
    let resumed = resume_migrated_agent_runtime_v2(
        bind(&a, b"chain payload"),
        bind(&b, b"destination input ignored"),
        &retained,
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
    assert_eq!(replay.run().iterations(), 6);
    assert_eq!(replay.run().stages(), 28);
    assert!(replay.run().usage().reserved_fuel > fuel);
    let commits = store.commits;
    let wrong = format!("sha256:{}", "0".repeat(64));
    let error = resume_migrated_agent_runtime_v2(
        bind(&a, b"chain payload"),
        bind(&b, b"destination input ignored"),
        &retained,
        &wrong,
        &before,
        &after,
    )
    .err()
    .expect("wronghandoffaccepted");
    assert!(error.iter().any(|e| e.code == "SPX-G583"));
    assert_eq!(store.commits, commits);
    for swap in [false, true] {
        let previous = if swap {
            bind(&b, b"destination input ignored")
        } else {
            bind(&a, b"changed task")
        };
        let destination = if swap {
            bind(&a, b"chain payload")
        } else {
            bind(&b, b"destination input ignored")
        };
        let previous_revision = previous.execution_revision().digest().to_owned();
        let destination_revision = destination.execution_revision().digest().to_owned();
        let error = resume_migrated_agent_runtime_v2(
            previous,
            destination,
            &retained,
            &handoff,
            &previous_revision,
            &destination_revision,
        )
        .err()
        .expect("changed live runtime accepted the stored handoff");
        assert!(
            error.iter().any(|error| error.message.contains("binding")),
            "{error:?}"
        );
    }
    let mut changed: serde_json::Value = serde_json::from_str(&retained).unwrap();
    let mut saved: serde_json::Value =
        serde_json::from_str(changed["handoff"].as_str().unwrap()).unwrap();
    saved["usage"]["calls"] = serde_json::json!("0");
    changed["handoff"] = serde_json::json!(format!("{saved}\n"));
    assert!(resume_migrated_agent_runtime_v2(
        bind(&a, b"chain payload"),
        bind(&b, b"destination input ignored"),
        &format!("{changed}\n"),
        &handoff,
        &before,
        &after
    )
    .is_err());
    assert_eq!(host.calls.len(), 3);
    assert_eq!(store.commits, commits);
}

#[test]
fn migrated_genesis_and_effect_lost_acknowledgements_fail_closed_and_recover() {
    let a = first();
    let b = successor(&a, "State", "StateB", "b", &["marker"], false);
    for boundary in ["genesis", "intent", "observed", "terminal"] {
        let (migration, before, after) = migrated(&a, &b);
        let handoff = migration.handoff_digest().unwrap();
        let mut host = handler();
        let mut store = Store {
            fail: Some(boundary),
            ..Default::default()
        };
        let failure = migration
            .run_durable(&mut host, &AgentCancellation::new(), &mut store)
            .err()
            .expect("lostackignored");
        assert!(!failure.diagnostics().is_empty());
        if boundary == "genesis" {
            assert_eq!(host.calls.len(), 0);
            assert_eq!(store.commits, 1);
        }
        if boundary == "terminal" {
            assert_eq!(
                failure.terminal().unwrap().status(),
                IterativeStatus::Complete
            );
        }
        let retained = store.document.clone();
        let old_calls = host.calls.len();
        let resumed = resume_migrated_agent_runtime_v2(
            bind(&a, b"chain payload"),
            bind(&b, b"destination input ignored"),
            &retained,
            &handoff,
            &before,
            &after,
        );
        if boundary == "intent" {
            match resumed {
                Err(errors) => assert!(errors.iter().any(|e| e.message.contains("intent"))),
                Ok(resumed) => assert!(resumed
                    .run_durable(&mut host, &AgentCancellation::new(), &mut store)
                    .is_err()),
            }
            assert_eq!(host.calls.len(), old_calls);
        } else {
            let done = resumed
                .unwrap()
                .run_durable(&mut host, &AgentCancellation::new(), &mut store)
                .unwrap();
            assert_eq!(
                done.run().run().lifecycle().status(),
                IterativeStatus::Complete
            );
            assert_eq!(host.calls.len(), 3);
            assert_eq!(done.run().usage().calls, 6);
        }
    }
}

#[test]
fn three_revision_durable_chain_preserves_cumulative_usage_and_predecessor_handoff() {
    for replayed_suspend in [false, true] {
        chain(replayed_suspend);
    }
}

fn chain(replayed_suspend: bool) {
    let a = first();
    let b = successor(&a, "State", "StateB", "b", &["marker"], true);
    let c = successor(&b, "StateB", "StateC", "c", &["marker", "stamp"], false);
    let (migration, a_revision, b_revision) = migrated(&a, &b);
    let b_handoff = migration.handoff_digest().unwrap();
    let mut b_host = handler();
    let mut b_store = Store::default();
    let suspended = migration
        .run_durable(&mut b_host, &AgentCancellation::new(), &mut b_store)
        .unwrap();
    assert_eq!(
        suspended.run().run().lifecycle().status(),
        IterativeStatus::Suspend
    );
    assert_eq!(
        (
            suspended.run().usage().calls,
            suspended.run().iterations(),
            suspended.run().stages()
        ),
        (6, 6, 19)
    );
    let suspended = if replayed_suspend {
        let retained = b_store.document.clone();
        let before_fuel = suspended.run().usage().reserved_fuel;
        let resumed = resume_migrated_agent_runtime_v2(
            bind(&a, b"chain payload"),
            bind(&b, b"destination input ignored"),
            &retained,
            &b_handoff,
            &a_revision,
            &b_revision,
        )
        .unwrap();
        let replay = resumed
            .run_durable(&mut b_host, &AgentCancellation::new(), &mut b_store)
            .unwrap();
        assert_eq!(
            replay.run().run().lifecycle().status(),
            IterativeStatus::Suspend
        );
        assert_eq!(b_host.calls.len(), 3);
        assert_eq!(replay.run().run().dispatched(), 0);
        assert_eq!(
            (
                replay.run().usage().calls,
                replay.run().iterations(),
                replay.run().stages()
            ),
            (6, 6, 28)
        );
        assert!(replay.run().usage().reserved_fuel > before_fuel);
        assert_eq!(replay.migration_handoff_digest(), Some(b_handoff.as_str()));
        replay
    } else {
        suspended
    };
    let prior_fuel = suspended.run().usage().reserved_fuel;
    let destination = bind(&c, b"third input ignored");
    let c_revision = destination.execution_revision().digest().to_owned();
    let migration = migrate_suspended_agent_runtime_v2(
        bind(&b, b"destination input ignored"),
        suspended,
        destination,
        &b_revision,
        &c_revision,
        "fixture.agent.fn.migrate_c",
        10_000,
        10_000_000,
    )
    .unwrap();
    assert!(migration
        .migration_root()
        .canonical_json()
        .contains(&b_handoff));
    let c_handoff = migration.handoff_digest().unwrap();
    assert_ne!(c_handoff, b_handoff);
    let mut c_host = handler();
    let complete = migration
        .run_durable(
            &mut c_host,
            &AgentCancellation::new(),
            &mut Store::default(),
        )
        .unwrap();
    assert_eq!(
        complete.run().run().lifecycle().status(),
        IterativeStatus::Complete
    );
    assert_eq!(c_host.calls.len(), 3);
    assert_eq!(
        (
            complete.run().usage().calls,
            complete.run().iterations(),
            complete.run().stages()
        ),
        (9, 9, if replayed_suspend { 37 } else { 28 })
    );
    assert_eq!(
        complete.run().usage().reserved_fuel,
        prior_fuel + 20_000 + 900_000
    );
    let Some(RetainedValue::Record(value)) = complete.run().run().lifecycle().value() else {
        panic!("missingResult")
    };
    assert!(value
        .fields
        .iter()
        .any(|f| f.value == RetainedValue::Bytes(b"chain payload".to_vec())));
    assert!(value
        .fields
        .iter()
        .any(|f| f.value == RetainedValue::I64(16)));
    assert_eq!(
        complete.migration_handoff_digest(),
        Some(c_handoff.as_str())
    );
}
