use super::*;
use semaprax::agent_lifecycle::{CheckpointStore, CheckpointStoreError};

#[derive(Default)]
struct Store {
    documents: Vec<String>,
    fail_kind: Option<&'static str>,
}
impl CheckpointStore for Store {
    fn commit(&mut self, _: u64, document: &str) -> Result<(), CheckpointStoreError> {
        self.documents.push(document.to_owned());
        let json: serde_json::Value = serde_json::from_str(document).unwrap();
        let last = json["entries"].as_array().unwrap().last().unwrap();
        if self
            .fail_kind
            .is_some_and(|kind| last["event"]["kind"] == kind)
        {
            return Err(CheckpointStoreError);
        }
        Ok(())
    }
}
#[test]
fn direct_runtime_v2_durable_store_replays_observations_and_rejects_uncertain_intent() {
    let fixture = typed_fixture();
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
        let proposals: Vec<_> = ["0", "1", "0"]
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
        let bind = |objective: &[u8]| {
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
                IterativeBudget::default(),
                EffectBudget {
                    max_calls: 3,
                    max_argument_bytes: 4096,
                    max_result_bytes: 4096,
                    max_total_bytes: 8192,
                },
            )
        };
        let mut handler = Handler {
            calls: Vec::new(),
            wrong: false,
        };
        let mut store = Store::default();
        let complete = bind(b"durable task")?
            .run_durable(
                &mut handler,
                &AgentCancellation::new(),
                None,
                &mut store,
                10_000_000,
            )
            .unwrap();
        assert_eq!(handler.calls.len(), 3);
        assert_eq!(
            complete.run().run().lifecycle().status(),
            IterativeStatus::Complete
        );
        assert_eq!(complete.run().usage().calls, 3);
        assert!(complete
            .evidence_root()
            .canonical_json()
            .contains(complete.run().checkpoint_digest()));
        let fuel = complete.run().usage().reserved_fuel;
        let mut never = Handler {
            calls: Vec::new(),
            wrong: false,
        };
        let resumed = bind(b"durable task")?
            .run_durable(
                &mut never,
                &AgentCancellation::new(),
                Some(complete.run().checkpoint()),
                &mut store,
                10_000_000,
            )
            .unwrap();
        assert!(never.calls.is_empty());
        assert_eq!(resumed.run().usage().calls, 3);
        assert!(resumed.run().usage().reserved_fuel > fuel);
        assert_eq!(
            resumed.run().run().lifecycle().status(),
            IterativeStatus::Complete
        );
        assert_ne!(complete.evidence_root(), resumed.evidence_root());
        let mut foreign_store = Store::default();
        assert!(bind(b"changed task")?
            .run_durable(
                &mut never,
                &AgentCancellation::new(),
                Some(complete.run().checkpoint()),
                &mut foreign_store,
                10_000_000
            )
            .is_err());
        assert!(foreign_store.documents.is_empty());
        for kind in ["intent", "observed"] {
            let mut interrupted_store = Store {
                fail_kind: Some(kind),
                ..Store::default()
            };
            let mut first = Handler {
                calls: Vec::new(),
                wrong: false,
            };
            let failure = bind(b"durable task")?
                .run_durable(
                    &mut first,
                    &AgentCancellation::new(),
                    None,
                    &mut interrupted_store,
                    10_000_000,
                )
                .err()
                .expect("lost acknowledgement must stop");
            assert_eq!(first.calls.len(), usize::from(kind == "observed"));
            let retained = interrupted_store.documents.last().unwrap().clone();
            assert_eq!(failure.checkpoint(), retained);
            let mut recovered_store = Store::default();
            let mut tail = Handler {
                calls: Vec::new(),
                wrong: false,
            };
            let recovered = bind(b"durable task")?.run_durable(
                &mut tail,
                &AgentCancellation::new(),
                Some(&retained),
                &mut recovered_store,
                10_000_000,
            );
            if kind == "intent" {
                assert!(recovered.is_err());
                assert!(tail.calls.is_empty());
                assert!(recovered_store.documents.is_empty());
            } else {
                let result = recovered.unwrap();
                assert_eq!(tail.calls, ["fixture.read.second", "fixture.read"]);
                assert_eq!(result.run().usage().calls, 3);
                assert_eq!(
                    result.run().run().lifecycle().status(),
                    IterativeStatus::Complete
                );
            }
        }
        Ok(())
    })
    .unwrap();
}
