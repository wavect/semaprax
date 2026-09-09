use super::*;
use semaprax::agent_lifecycle::iterative::effects::{
    EffectArgument, EffectBudget, EffectOperation, EffectResult, EffectScalar, TypedEffectHandler,
    TypedEffectRequest,
};
use semaprax::agent_lifecycle::iterative::{
    compile_source_agent_lifecycle_v2, IterativeBudget, IterativeStatus,
};
use semaprax::agent_runtime_v2::bind_agent_runtime_v2;
use semaprax::interpreter::retained_call::RetainedValue;

pub(super) fn operations() -> Vec<EffectOperation> {
    ["fixture.read", "fixture.read.second"]
        .into_iter()
        .map(|id| EffectOperation {
            operation_id: id.into(),
            effect_id: "read".into(),
            arguments: vec![EffectArgument {
                argument_id: "query".into(),
                proposal_field_id: "fixture.agent.type.proposal.budget".into(),
                kind: EffectScalar::I64,
            }],
            results: vec![EffectResult {
                result_id: "value".into(),
                kind: EffectScalar::I64,
            }],
        })
        .collect()
}
pub(super) struct Handler {
    pub(super) calls: Vec<String>,
    pub(super) wrong: bool,
}
impl TypedEffectHandler for Handler {
    fn execute(
        &mut self,
        request: &TypedEffectRequest<'_>,
    ) -> Option<Vec<(String, RetainedValue)>> {
        self.calls.push(request.operation_id().into());
        assert_eq!(request.effect_id(), "read");
        assert_eq!(
            request.arguments(),
            &[("query".into(), RetainedValue::I64(5))]
        );
        Some(vec![(
            "value".into(),
            if self.wrong {
                RetainedValue::Bool(true)
            } else {
                RetainedValue::I64(8)
            },
        )])
    }
}
pub(super) fn typed_fixture() -> Fixture {
    let fixture = super::iterative::fixture();
    let path = fixture.0.join("src/app.spx");
    let source = std::fs::read_to_string(&path)
        .unwrap()
        .replace("sequence > 0usize", "sequence <= 1usize");
    let mut program = semaprax::parse(&source, &path).unwrap();
    let runtime = &mut program.agents[0].runtime_v1_json;
    *runtime = runtime.replace(
        "\"type\":\"string\",\"required\":true,\"max_bytes\":64",
        "\"type\":\"integer\",\"required\":true,\"max_bytes\":20",
    );
    let start = runtime.find("\"tools\":[").unwrap() + "\"tools\":[".len();
    let end = runtime.find("],\"policy\":").unwrap();
    let second = runtime[start..end].replace(
        "\"tool_id\":\"fixture.read\"",
        "\"tool_id\":\"fixture.read.second\"",
    );
    runtime.insert_str(end, &format!(",{second}"));
    *runtime = runtime.replace(
        "\"allowed_tool_ids\":[\"fixture.read\"]",
        "\"allowed_tool_ids\":[\"fixture.read\",\"fixture.read.second\"]",
    );
    std::fs::write(&path, semaprax::format::canonical(&program)).unwrap();
    fixture
}

#[test]
fn direct_runtime_v2_consumes_typed_product_and_binds_actual_producer() {
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
        let budgets = EffectBudget {
            max_calls: 3,
            max_argument_bytes: 4096,
            max_result_bytes: 4096,
            max_total_bytes: 8192,
        };
        let bind = |operations, selector, budgets| {
            bind_agent_runtime_v2(
                project.clone(),
                ProgramRootRef::V1(&root),
                root.program_root_digest(),
                source.path(),
                "fixture.agent",
                "fixture.agent.type.step",
                selector,
                operations,
                &deployment,
                LifecycleTask {
                    objective: b"typed task".to_vec(),
                    budget: 12,
                },
                &proposals,
                IterativeBudget::default(),
                budgets,
            )
        };
        let runtime = bind(
            operations(),
            "fixture.agent.type.proposal.sequence",
            budgets,
        )?;
        let same = bind(
            operations(),
            "fixture.agent.type.proposal.sequence",
            budgets,
        )?;
        assert_eq!(runtime.execution_revision(), same.execution_revision());
        let mut reversed = operations();
        reversed.reverse();
        let reordered = bind(reversed, "fixture.agent.type.proposal.sequence", budgets)?;
        assert_ne!(runtime.deployment_root(), reordered.deployment_root());
        let narrowed = bind(
            operations(),
            "fixture.agent.type.proposal.sequence",
            EffectBudget {
                max_total_bytes: 1,
                ..budgets
            },
        )?;
        assert_ne!(runtime.instance_root(), narrowed.instance_root());
        assert!(bind(operations(), "fixture.agent.type.proposal.budget", budgets).is_err());
        let revision = runtime.execution_revision().clone();
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
        assert_eq!(evidence.execution_revision(), &revision);
        assert!(evidence
            .evidence_root()
            .canonical_json()
            .contains(evidence.run().evidence_digest()));
        let mut wrong = Handler {
            calls: Vec::new(),
            wrong: true,
        };
        let failure = same.run(&mut wrong, &AgentCancellation::new())?;
        assert_eq!(
            failure.run().lifecycle().status(),
            IterativeStatus::EffectFailed
        );
        assert_eq!(wrong.calls.len(), 1);
        assert!(failure.run().result_bytes() > 0);
        assert_ne!(evidence.evidence_root(), failure.evidence_root());
        let mut never = Handler {
            calls: Vec::new(),
            wrong: false,
        };
        let stopped = narrowed.run(&mut never, &AgentCancellation::new())?;
        assert!(never.calls.is_empty());
        assert!(stopped.run().failure().is_some());
        Ok(())
    })
    .unwrap();
}

#[path = "typed/durable.rs"]
mod durable;

#[path = "typed/migration.rs"]
pub(in crate::execution_revision) mod migration;

#[path = "typed/linked.rs"]
mod linked;
