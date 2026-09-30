use super::*;
use semaprax::agent_runtime_v2::bind_agent_runtime_v2_live;
#[path = "model_wait/durable.rs"]
mod durable;

#[test]
fn model_wait_binding_obeys_exact_runtime_stage_ceiling() {
    let fixture = typed_fixture();
    let path = fixture.0.join("src/app.spx");
    let mut source = std::fs::read_to_string(&path).unwrap();
    source.push_str(
        r#"
@id("fixture.agent.fn.await_proposal")
fn await_proposal(observation: Observation) -> Proposal
    yields Observation -> Proposal
{
    yield observation
}
"#,
    );
    std::fs::write(&path, source).unwrap();
    with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        let project = snapshot.retain_revision();
        let root = project.program_root()?;
        let (_, deployment) = migrate_agent_definition_v1(
            project.agent_definitions()[0]
                .definition()
                .canonical_source(),
            "fixture.model_wait.runtime",
        )?;
        let runtime = bind_agent_runtime_v2_live(
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
                objective: b"wait fuel".to_vec(),
                budget: 12,
            },
            IterativeBudget {
                max_steps_per_stage: 1000,
                ..IterativeBudget::default()
            },
            EffectBudget {
                max_calls: 3,
                max_argument_bytes: 4096,
                max_result_bytes: 4096,
                max_total_bytes: 8192,
            },
        )?;
        assert_eq!(
            runtime
                .source_model_wait_binding("fixture.agent.fn.await_proposal", 1000)?
                .evaluation_fuel(),
            1000
        );
        let error = runtime
            .source_model_wait_binding("fixture.agent.fn.await_proposal", 1001)
            .unwrap_err();
        assert!(error[0].message.contains("source.model_wait_stage_fuel"));
        Ok(())
    })
    .unwrap();
}
