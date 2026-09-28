use super::*;

const WRAPPER: &str = r#"
@id("fixture.agent.fn.await_proposal")
fn await_proposal(observation: Observation) -> Proposal
    yields Observation -> Proposal
{
    yield observation
}
"#;

fn source(wrapper: &str) -> String {
    // A distinct model-wait revision of the real standalone Agent fixture;
    // the shared original and its native/graph snapshots remain unchanged.
    format!(
        "{}{}",
        include_str!("../../../../examples/offline-repair-project/src/app.spx"),
        wrapper
    )
}

fn fixture(wrapper: &str) -> CompiledIterativeLifecycle {
    compile_source_agent_lifecycle_v2(
        &source(wrapper),
        "model-wait-fixture.spx",
        "fixture.agent",
        "fixture.agent.type.step",
    )
    .unwrap()
}

#[test]
fn checked_identity_wrapper_binds_nominal_copy_channel_and_canonical_graph() {
    let compiled = fixture(WRAPPER);
    let binding = compiled
        .model_wait_binding("fixture.agent.fn.await_proposal", 1000)
        .unwrap();
    assert!(binding.matches(&compiled));
    assert_eq!(
        binding.observation.as_str(),
        "fixture.agent.type.observation"
    );
    assert_eq!(binding.proposal.as_str(), "fixture.agent.type.proposal");
    assert_eq!(binding.evaluation_fuel(), 1000);
    assert_eq!(binding.digest().len(), 71);
    assert!(binding.digest().starts_with("sha256:"));
    assert!(binding.digest()[7..]
        .bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
    assert_eq!(
        binding,
        compiled
            .model_wait_binding("fixture.agent.fn.await_proposal", 1000)
            .unwrap()
    );
    assert_ne!(
        binding.digest(),
        compiled
            .model_wait_binding("fixture.agent.fn.await_proposal", 999)
            .unwrap()
            .digest()
    );
    let original = crate::check(&source(WRAPPER), "model-wait-fixture.spx").unwrap();
    let canonical = crate::format::canonical(&original);
    let checked = crate::check(&canonical, "model-wait-fixture.spx").unwrap();
    let reparsed = crate::parse(&canonical, "model-wait-fixture.spx").unwrap();
    assert_eq!(canonical, crate::format::canonical(&reparsed));
    assert_eq!(
        crate::graph::to_json(&checked).unwrap(),
        crate::graph::to_json(&original).unwrap()
    );
    let graph: serde_json::Value =
        serde_json::from_str(&crate::graph::to_json(&checked).unwrap()).unwrap();
    let wrapper = graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == "fixture.agent.fn.await_proposal")
        .unwrap();
    let ty = |id| {
        hir::ResolvedType::Nominal {
            declaration: hir::DeclarationId::new(id),
            arguments: vec![],
        }
        .identity_key()
    };
    assert_eq!(wrapper["persistent"], true);
    assert_eq!(
        wrapper["params"][0]["type_id"],
        ty("fixture.agent.type.observation")
    );
    assert_eq!(wrapper["params"][0]["ownership_mode"], "value");
    assert_eq!(wrapper["return_type_id"], ty("fixture.agent.type.proposal"));
    assert_eq!(wrapper["body"]["statements"].as_array().unwrap().len(), 0);
    assert_eq!(wrapper["body"]["tail"]["kind"], "yield");
    assert_eq!(
        wrapper["body"]["tail"]["request_type_id"],
        ty("fixture.agent.type.observation")
    );
    assert_eq!(
        wrapper["body"]["tail"]["type_id"],
        ty("fixture.agent.type.proposal")
    );
}

#[test]
fn wrapper_selection_fuel_type_and_identity_transformations_refuse() {
    let compiled = fixture(WRAPPER);
    for (id, fuel) in [
        ("missing", 1000),
        ("fixture.agent.fn.observe", 1000),
        ("fixture.agent.fn.await_proposal", 0),
        (
            "fixture.agent.fn.await_proposal",
            crate::interpreter::MAX_STEPS_LIMIT + 1,
        ),
    ] {
        let error = compiled.model_wait_binding(id, fuel).unwrap_err();
        assert_eq!(error[0].code, "SPX-G582");
        assert!(error[0].message.contains("source.model_wait_binding"));
    }
    let alternatives = [
        WRAPPER.replace("observation: Observation", "observation: i64")
            .replace("-> Proposal", "-> i64").replace("yields Observation", "yields i64"),
        WRAPPER.replace("yield observation", "yield Observation { budget: observation.budget, epoch: observation.epoch }"),
        WRAPPER.replace("yield observation", "let answer = yield observation;\n    answer"),
        format!("{}\n@id(\"fixture.agent.fn.identity_observation\")\nfn identity_observation(observation: Observation) -> Observation {{ observation }}\n",
            WRAPPER.replace("yield observation", "yield identity_observation(observation)")),
    ];
    for wrapper in alternatives {
        let compiled = fixture(&wrapper);
        let error = compiled
            .model_wait_binding("fixture.agent.fn.await_proposal", 1000)
            .unwrap_err();
        assert_eq!(error[0].code, "SPX-G582");
    }
    // Ownership cannot be smuggled into an otherwise checked compiler product.
    let mut tampered = fixture(WRAPPER);
    tampered
        .inner
        .program
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "fixture.agent.fn.await_proposal")
        .unwrap()
        .params[0]
        .ownership = hir::OwnershipMode::Own;
    assert_eq!(
        tampered
            .model_wait_binding("fixture.agent.fn.await_proposal", 1000)
            .unwrap_err()[0]
            .code,
        "SPX-G582"
    );
}

#[test]
fn already_decoded_proposal_projects_exact_order_and_rejects_other_schema() {
    let compiled = fixture(WRAPPER);
    let schema = compiled.proposal_schema();
    let document = format!("{{\"schema\":\"semaprax.agent-proposal.v1\",\"agent_id\":\"fixture.agent\",\"proposal_schema_digest\":\"{}\",\"value\":{{\"fields\":{{\"fixture.agent.type.proposal.budget\":\"7\",\"fixture.agent.type.proposal.urgent\":true,\"fixture.agent.type.proposal.sequence\":\"1\"}}}}}}\n", schema.schema().digest());
    let numeric = document.replace("\"7\"", "7");
    assert_eq!(schema.decode(&numeric).unwrap_err()[0].code, "SPX-G551");
    let decoded = schema.decode(&document).unwrap();
    assert_eq!(
        schema.model_wait_carrier(&decoded),
        Some(ResumableChannelValue::Record {
            declaration: hir::DeclarationId::new("fixture.agent.type.proposal"),
            fields: vec![
                ArgumentValue::Int(7),
                ArgumentValue::Bool(true),
                ArgumentValue::Usize(1)
            ],
        })
    );
    let renamed = source(WRAPPER).replace("fixture.agent\"", "fixture.other\"");
    let other = compile_source_agent_lifecycle_v2(
        &renamed,
        "model-wait-other.spx",
        "fixture.other",
        "fixture.agent.type.step",
    )
    .unwrap();
    assert!(other
        .proposal_schema()
        .model_wait_carrier(&decoded)
        .is_none());
}
