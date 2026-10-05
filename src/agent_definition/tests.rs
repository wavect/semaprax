use super::{
    compilations_on_this_thread, compile_agent_definition, render_v1_definition_source,
    verify_agent_graph_bundle, verify_compiled_agent_graph,
};

/// The canonical fixture AgentDefinition, written out byte for byte so it is
/// independent of the renderer under test.
pub(crate) fn fixture_definition() -> String {
    concat!(
        "{\"schema\":\"semaprax.agent-definition.v1\",\"agent_id\":\"fixture.agent\",",
        "\"types\":[",
        "{\"role\":\"task\",\"stable_id\":\"fixture.agent.type.task\"},",
        "{\"role\":\"state\",\"stable_id\":\"fixture.agent.type.state\"},",
        "{\"role\":\"observation\",\"stable_id\":\"fixture.agent.type.observation\"},",
        "{\"role\":\"proposal\",\"stable_id\":\"fixture.agent.type.proposal\"},",
        "{\"role\":\"outcome\",\"stable_id\":\"fixture.agent.type.outcome\"},",
        "{\"role\":\"result\",\"stable_id\":\"fixture.agent.type.result\"}],",
        "\"operations\":[",
        "{\"role\":\"initialize\",\"stable_id\":\"fixture.agent.fn.initialize\",\"kind\":\"deterministic\"},",
        "{\"role\":\"observe\",\"stable_id\":\"fixture.agent.fn.observe\",\"kind\":\"deterministic\"},",
        "{\"role\":\"propose\",\"stable_id\":\"fixture.agent.fn.propose\",\"kind\":\"model\"},",
        "{\"role\":\"authorize\",\"stable_id\":\"fixture.agent.fn.authorize\",\"kind\":\"deterministic\"},",
        "{\"role\":\"execute\",\"stable_id\":\"fixture.agent.fn.execute\",\"kind\":\"effect\"},",
        "{\"role\":\"reduce\",\"stable_id\":\"fixture.agent.fn.reduce\",\"kind\":\"deterministic\"}],",
        "\"runtime_v1\":{",
        "\"models\":[{\"provider_id\":\"fake.local\",\"model_id\":\"fake-basic\",",
        "\"locality\":\"local\",\"quality_tier\":\"basic\",\"tokenizer_id\":\"fake.bytes-v1\",",
        "\"max_context_tokens\":4096,\"input_usd_microunits_per_million_tokens\":0,",
        "\"output_usd_microunits_per_million_tokens\":0,\"capabilities\":[\"text\"]}],",
        "\"tools\":[{\"tool_id\":\"fixture.read\",\"description\":\"Return one bounded fixture value.\",",
        "\"arguments_schema\":{\"type\":\"object\",\"fields\":[{\"name\":\"query\",\"type\":\"string\",\"required\":true,\"max_bytes\":64}],\"additional_properties\":false},",
        "\"result_schema\":{\"type\":\"object\",\"fields\":[{\"name\":\"value\",\"type\":\"string\",\"required\":true,\"max_bytes\":64}],\"additional_properties\":false},",
        "\"effects\":[\"read\"],\"required_capabilities\":[\"tool.read\"]}],",
        "\"policy\":{\"allowed_provider_ids\":[\"fake.local\"],\"allowed_model_ids\":[\"fake-basic\"],",
        "\"required_locality\":\"local_only\",\"minimum_quality_tier\":\"basic\",",
        "\"required_model_capabilities\":[\"text\"],\"granted_capabilities\":[\"tool.read\"],",
        "\"allowed_tool_ids\":[\"fixture.read\"]},",
        "\"limits\":{\"max_turns\":2,\"max_provider_attempts\":2,\"max_retries_per_turn\":1,",
        "\"max_concurrency\":1,\"max_elapsed_ms\":1000,\"max_provider_request_bytes\":65536,",
        "\"max_provider_response_bytes\":4096,\"max_stream_chunks\":64,",
        "\"max_total_provider_input_bytes\":131072,\"max_total_provider_output_bytes\":8192,",
        "\"max_reported_model_input_tokens\":131072,\"max_reported_model_output_tokens\":8192,",
        "\"max_usd_microunits\":0,\"max_tool_calls\":1,\"max_tool_arguments_bytes\":4096,",
        "\"max_tool_result_bytes\":4096,\"max_total_tool_bytes\":8192,",
        "\"max_retained_state_bytes\":131072,\"max_trace_events\":64,\"max_trace_bytes\":131072,",
        "\"max_evidence_bytes\":262144,\"max_builder_bytes\":1048576}}}\n"
    )
    .to_owned()
}

#[test]
fn standalone_graph_verification_compiles_its_own_definition() {
    let source = fixture_definition();
    let compiled = compile_agent_definition(&source).unwrap();
    let graph = compiled.graph().canonical_json().to_owned();
    let profile = compiled.runtime_v1_profile().to_owned();

    let before = compilations_on_this_thread();
    verify_agent_graph_bundle(&source, &profile, &graph).unwrap();
    assert_eq!(compilations_on_this_thread() - before, 1);

    // The reuse seam performs no compilation of its own and still compares
    // the exact bytes of the submitted graph.
    let before = compilations_on_this_thread();
    verify_compiled_agent_graph(&compiled, &graph).unwrap();
    let truncated = &graph[..graph.len() - 1];
    let error = verify_compiled_agent_graph(&compiled, truncated).unwrap_err();
    assert_eq!(error[0].code, "SPX-G503");
    assert_eq!(compilations_on_this_thread(), before);
}

#[test]
fn the_reuse_seam_keeps_the_agent_graph_size_guard() {
    let compiled = compile_agent_definition(&fixture_definition()).unwrap();
    // A graph one byte over the bound is refused before any byte comparison,
    // even when it begins with the exact compiled graph.
    let mut oversized = compiled.graph().canonical_json().to_owned();
    oversized.push_str(&" ".repeat(super::MAX_GRAPH_BYTES + 1 - oversized.len()));
    assert_eq!(oversized.len(), super::MAX_GRAPH_BYTES + 1);
    let error = verify_compiled_agent_graph(&compiled, &oversized).unwrap_err();
    assert_eq!(error[0].code, "SPX-G503");
}

/// Frozen known answers for the fixture, computed before the role-section
/// writers were shared and independent of any role table.
const DEFINITION_DIGEST: &str =
    "sha256:82ab9abbeca5e209c36224d9cab3b7b6a7cdffc3b2fce5db73123fa7425965a0";
const GRAPH_DIGEST: &str =
    "sha256:0dc7ce1d50d43077042577cf6ac3dcfb5d2a744fb3acd2ca6cea12a6e296ff61";

#[test]
fn every_type_and_operation_role_maps_to_its_admitted_identity() {
    let compiled = compile_agent_definition(&fixture_definition()).unwrap();
    let definition = compiled.definition();
    for (role, stable_id) in [
        ("task", "fixture.agent.type.task"),
        ("state", "fixture.agent.type.state"),
        ("observation", "fixture.agent.type.observation"),
        ("proposal", "fixture.agent.type.proposal"),
        ("outcome", "fixture.agent.type.outcome"),
        ("result", "fixture.agent.type.result"),
    ] {
        assert_eq!(
            definition.type_id(role),
            Some(stable_id),
            "type role {role}"
        );
        assert_eq!(
            definition.operation(role),
            None,
            "{role} is not an operation"
        );
    }
    for (role, stable_id, kind) in [
        ("initialize", "fixture.agent.fn.initialize", "deterministic"),
        ("observe", "fixture.agent.fn.observe", "deterministic"),
        ("propose", "fixture.agent.fn.propose", "model"),
        ("authorize", "fixture.agent.fn.authorize", "deterministic"),
        ("execute", "fixture.agent.fn.execute", "effect"),
        ("reduce", "fixture.agent.fn.reduce", "deterministic"),
    ] {
        assert_eq!(
            definition.operation(role),
            Some((stable_id, kind)),
            "operation role {role}"
        );
        assert_eq!(definition.type_id(role), None, "{role} is not a type");
    }
    assert_eq!(definition.proposal_type_id(), "fixture.agent.type.proposal");
    assert_eq!(
        definition.observation_type_id(),
        "fixture.agent.type.observation"
    );
    for unknown in ["", "Task", "proposal ", "agent"] {
        assert_eq!(definition.type_id(unknown), None);
        assert_eq!(definition.operation(unknown), None);
    }
}

#[test]
fn named_accessors_follow_identities_not_positions() {
    // Renaming the observation and proposal identities moves only the named
    // accessors that own those roles.
    let source = fixture_definition()
        .replacen("fixture.agent.type.observation", "fixture.agent.seen", 1)
        .replacen("fixture.agent.type.proposal", "fixture.agent.plan", 1);
    let compiled = compile_agent_definition(&source).unwrap();
    let definition = compiled.definition();
    assert_eq!(definition.observation_type_id(), "fixture.agent.seen");
    assert_eq!(definition.proposal_type_id(), "fixture.agent.plan");
    assert_eq!(
        definition.type_id("state"),
        Some("fixture.agent.type.state")
    );
    let graph = compiled.graph().canonical_json();
    assert!(graph.contains("\"kind\":\"opaque_authorized\",\"value_type\":\"fixture.agent.plan\""));
    assert!(graph.contains("\"proposal_contract\":{\"type_id\":\"fixture.agent.plan\""));
    assert!(graph.contains(
        "{\"from\":\"fixture.agent.fn.observe\",\"relationship\":\"returns\",\"to\":\"fixture.agent.seen\"}"
    ));
}

#[test]
fn shared_role_writers_keep_every_projection_byte_identical() {
    let source = fixture_definition();
    let compiled = compile_agent_definition(&source).unwrap();
    // Admission re-renders the definition and must reproduce it exactly.
    assert_eq!(compiled.definition().canonical_source(), source);
    assert_eq!(compiled.definition().digest(), DEFINITION_DIGEST);
    assert_eq!(compiled.graph().digest(), GRAPH_DIGEST);
    let graph = compiled.graph().canonical_json();
    assert!(graph.starts_with(concat!(
        "{\"schema\":\"semaprax.agent-graph.v1\",\"definition_digest\":",
        "\"sha256:82ab9abbeca5e209c36224d9cab3b7b6a7cdffc3b2fce5db73123fa7425965a0\",",
        "\"agent_id\":\"fixture.agent\",\"types\":[",
        "{\"role\":\"task\",\"stable_id\":\"fixture.agent.type.task\"},"
    )));
    assert!(graph.contains(concat!(
        "{\"role\":\"reduce\",\"stable_id\":\"fixture.agent.fn.reduce\",\"kind\":\"deterministic\"}],",
        "\"derived_types\":[{\"node_id\":\"@authorized_proposal\""
    )));

    // Definition assembly writes the same sections from caller-ordered rows.
    let value: serde_json::Value = serde_json::from_str(&source).unwrap();
    let ids = |key: &str| {
        value[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["stable_id"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>()
    };
    let (types, operations) = (ids("types"), ids("operations"));
    let assembled =
        render_v1_definition_source("fixture.agent", &types, &operations, &value["runtime_v1"])
            .unwrap();
    assert_eq!(assembled, source);
    assert!(render_v1_definition_source(
        "fixture.agent",
        &types[..5],
        &operations,
        &value["runtime_v1"]
    )
    .is_err());
    assert!(render_v1_definition_source(
        "fixture.agent",
        &types,
        &operations[1..],
        &value["runtime_v1"]
    )
    .is_err());
}

#[test]
fn missing_duplicated_reordered_and_wrong_kind_roles_stay_rejected() {
    let source = fixture_definition();
    let reject = |mutated: String| {
        let error = compile_agent_definition(&mutated).err().unwrap();
        assert_eq!(error.len(), 1);
        (error[0].code, error[0].message.clone())
    };
    let invariant = |field: &str| {
        (
            "SPX-G502",
            format!("AgentDefinition invariant failed: {field}"),
        )
    };
    let result_row = ",{\"role\":\"result\",\"stable_id\":\"fixture.agent.type.result\"}";
    assert_eq!(
        reject(source.replacen(result_row, "", 1)),
        invariant("types")
    );
    let reduce_row = concat!(
        ",{\"role\":\"reduce\",\"stable_id\":\"fixture.agent.fn.reduce\",",
        "\"kind\":\"deterministic\"}"
    );
    assert_eq!(
        reject(source.replacen(reduce_row, "", 1)),
        invariant("operations")
    );
    assert_eq!(
        reject(source.replacen("\"role\":\"state\"", "\"role\":\"task\"", 1)),
        invariant("types.roles")
    );
    assert_eq!(
        reject(source.replacen("\"role\":\"observe\"", "\"role\":\"initialize\"", 1)),
        invariant("operations.roles")
    );
    let reordered = source
        .replacen("\"role\":\"outcome\"", "\"role\":\"PLACEHOLDER\"", 1)
        .replacen("\"role\":\"result\"", "\"role\":\"outcome\"", 1)
        .replacen("\"role\":\"PLACEHOLDER\"", "\"role\":\"result\"", 1);
    assert_eq!(reject(reordered), invariant("types.roles"));
    assert_eq!(
        reject(source.replacen("\"kind\":\"effect\"", "\"kind\":\"deterministic\"", 1)),
        invariant("operations.roles")
    );
    assert_eq!(
        reject(source.replacen("fixture.agent.type.state", "fixture.agent.type.task", 1)),
        invariant("types.stable_ids")
    );
    assert_eq!(
        reject(source.replacen("fixture.agent.fn.reduce", "fixture.agent.fn.observe", 1)),
        invariant("operations.stable_ids")
    );
}
