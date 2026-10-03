use super::*;

fn source() -> String {
    let source = include_str!("../../../../../examples/offline-repair-project/src/app.spx");
    format!(
        "{}\n{}",
        source.replace(
            "    runtime_v1 {",
            "    model_wait_v1 { propose = \"fixture.agent.fn.park\"; }\n    runtime_v1 {"
        ),
        r#"
@id("fixture.agent.fn.park")
fn park(state: own State, observation: Observation) -> State yields Observation -> Proposal {
    let proposal = yield observation;
    state
}
"#
    )
}
fn bind(source: &str) -> Result<CheckedOwnedAgentWaitBindingV8, Vec<Diagnostic>> {
    compile_owned_agent_wait_v8(
        source,
        Path::new("owned-agent-binding.spx"),
        "fixture.agent",
        "fixture.agent.type.step",
    )
}
#[test]
fn owned_agent_binding_uses_real_roles_shared_proofs_and_full_source_graph() {
    let source = source();
    let b = bind(&source).unwrap_or_else(|e| panic!("{e:?}"));
    assert!(b.helper().same_helper(b.observe().helper()));
    assert!(b.helper().same_helper(b.authorize().helper()));
    assert_eq!(
        b.observe().function().id.as_str(),
        "fixture.agent.fn.observe"
    );
    assert_eq!(
        b.authorize().function().id.as_str(),
        "fixture.agent.fn.authorize"
    );
    assert_eq!(b.agent().as_str(), "fixture.agent");
    assert_eq!(b.signature().as_object().unwrap().len(), 7);
    assert_eq!(b.signature()["parameters"][0]["mode"], "own");
    assert_eq!(b.signature()["parameters"][1]["mode"], "copy");
    let original = crate::check(&source, Path::new("owned-agent-binding.spx")).unwrap();
    let graph: Value = serde_json::from_str(&crate::graph::to_json(&original).unwrap()).unwrap();
    assert_eq!(graph["schema"], "semaprax.graph.v50");
    let expected = super::super::super::codec::fact_digest(
        b"semaprax.source-owned-frame-plan.v2\0",
        &json!({"source_revision":b.lifecycle().source_revision(),"agent":"fixture.agent",
            "model_operation":"fixture.agent.fn.propose","helper":"fixture.agent.fn.park",
            "signature":b.signature(),"checked_graph":graph,
            "cleanup_plan_digest":b.cleanup_digest()}),
    );
    assert_eq!(b.binding(), expected);
    let canonical = crate::format::canonical(&original);
    assert_eq!(b.binding(), bind(&canonical).unwrap().binding());
    assert_eq!(
        b.binding(),
        bind(&format!("// comment\n{source}")).unwrap().binding()
    );
    let changed = source.replace("sequence <= 1usize", "sequence <= 0usize");
    assert_ne!(b.binding(), bind(&changed).unwrap().binding());
    let changed = source.replace("fixture.agent.fn.propose", "fixture.agent.fn.other_propose");
    assert_ne!(b.binding(), bind(&changed).unwrap().binding());
}
#[test]
fn owned_agent_binding_refuses_unassociated_helper_and_wrong_role_carriers() {
    let source = source();
    let missing = source.replace(
        "    model_wait_v1 { propose = \"fixture.agent.fn.park\"; }\n",
        "",
    );
    assert_eq!(bind(&missing).err().unwrap()[0].code, "SPX-G583");
    let changed = source.replace("fn park(state: own State, observation: Observation) -> State yields Observation -> Proposal",
        "fn park(state: own State, observation: OtherObservation) -> State yields OtherObservation -> Proposal");
    let changed = format!("{changed}\n@id(\"other.observation\") record OtherObservation {{ @id(\"other.budget\") budget:i64, }}");
    assert_eq!(bind(&changed).err().unwrap()[0].code, "SPX-G583");
    assert!(compile_owned_agent_wait_v8(
        &source,
        Path::new("owned-agent-binding.spx"),
        "other.agent",
        "fixture.agent.type.step"
    )
    .is_err());
}

mod backend_refusal;
