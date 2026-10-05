use super::{
    compilations_on_this_thread, compile_agent_definition, verify_agent_graph_bundle,
    verify_compiled_agent_graph,
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
