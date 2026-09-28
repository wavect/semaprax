//! Metadata-only scalar Agent fixture; no provider or tool dispatch.
fn nonclaims() -> &'static str {
    r#"["no_compiler_determinism_from_model_output","no_model_output_authority","no_provider_identity_provenance_or_quality_truth","no_secret_input_or_secret_leakage_guarantee_for_caller_supplied_content","no_credential_prompt_state_trace_or_diagnostic_exposure","no_ambient_network_filesystem_process_home_or_environment_authority","no_write_apply_mutation_or_target_execution_tool_authority","no_capability_minting_delegation_or_self_approval","no_human_approval_ui_or_policy","no_semantic_prompt_injection_proof","no_forced_cancellation_or_preemption","no_exactly_once_provider_billing_or_retry","no_durable_memory_persistence_recovery_or_resume","no_crash_reboot_or_power_loss_durability","no_distributed_or_parallel_execution","no_model_quality_accuracy_or_completion_guarantee","no_live_price_or_cost_accuracy_guarantee","no_reusable_authorization_token","no_signature_attestation_or_authenticated_provenance","no_wallet_payment_signing_asset_or_economic_authority","no_privacy_compliance_or_data_residency_guarantee","no_general_formal_proof","no_new_language_graph_cleanup_backend_or_runtime_semantics","no_current_schema_api_or_kat_modification"]"#
}

fn profile(agent_id: &str) -> String {
    concat!(
        "{\"schema\":\"semaprax.agent-runtime-profile.v1\",\"agent_id\":\"AGENT\",",
        "\"models\":[{\"provider_id\":\"fake.local\",\"model_id\":\"fake-basic\",",
        "\"locality\":\"local\",\"quality_tier\":\"basic\",\"tokenizer_id\":\"fake.bytes-v1\",",
        "\"max_context_tokens\":4096,\"input_usd_microunits_per_million_tokens\":0,",
        "\"output_usd_microunits_per_million_tokens\":0,\"capabilities\":[\"text\"]}],",
        "\"tools\":[{\"tool_id\":\"fixture.read\",\"description\":\"Read.\",",
        "\"arguments_schema\":{\"type\":\"object\",\"fields\":[],\"additional_properties\":false},",
        "\"result_schema\":{\"type\":\"object\",\"fields\":[],\"additional_properties\":false},",
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
        "\"max_evidence_bytes\":262144,\"max_builder_bytes\":1048576},\"nonclaims\":NONCLAIMS}\n"
    )
    .replace("AGENT", agent_id)
    .replace("NONCLAIMS", nonclaims())
}

pub(super) fn source(embedded: bool, wait: bool) -> String {
    let profile = profile("fixture.agent");
    let runtime = profile
        .strip_prefix(
            "{\"schema\":\"semaprax.agent-runtime-profile.v1\",\"agent_id\":\"fixture.agent\",",
        )
        .unwrap()
        .split_once(",\"nonclaims\":")
        .unwrap()
        .0;
    let runtime = format!("{{{runtime}}}");
    let mut text =
        String::from("module fixture.app;\n@id(\"fixture.agent\") agent FixtureAgent { types {\n");
    for role in [
        "task",
        "state",
        "observation",
        "proposal",
        "outcome",
        "result",
    ] {
        text.push_str(&format!(
            "@id(\"fixture.agent.type.{role}\") type {role};\n"
        ));
    }
    text.push_str("} operations {\n");
    for role in [
        "initialize",
        "observe",
        "propose",
        "authorize",
        "execute",
        "reduce",
    ] {
        text.push_str(&format!("@id(\"fixture.agent.fn.{role}\") "));
        match role {
            "propose" => text.push_str("model fn propose;\n"),
            "execute" => text.push_str("effect fn execute;\n"),
            _ if embedded => text.push_str(&format!("fn {role}(value:i64)->i64 {{ value + 1 }}\n")),
            _ => text.push_str(&format!("fn {role};\n")),
        }
    }
    text.push_str("}\n");
    if wait {
        text.push_str("model_wait_v1 { propose = \"fixture.wait\"; }\n");
    }
    text.push_str(&format!(
        "runtime_v1 {{ canonical_json {}; }} }}\n",
        serde_json::to_string(&runtime).unwrap()
    ));
    if !embedded {
        for role in ["initialize", "observe", "authorize", "reduce"] {
            text.push_str(&format!(
                "@id(\"fixture.agent.fn.{role}\") fn {role}(value:i64)->i64 {{ value + 1 }}\n"
            ));
        }
    }
    // Scalar helpers intentionally carry no model-wait runtime/owned-State claim.
    text.push_str("@id(\"fixture.wait\") fn wait(value:i64)->i64 {value}\n@id(\"fixture.wait.other\") fn other_wait(value:i64)->i64 {value}\n@id(\"fixture.main\") fn main()->i64 {observe(41)}\n@id(\"fixture.public\") fn published()->i64 {0}\n");
    text
}

pub(super) fn protocol(source: &str, follows: bool) -> String {
    let protocol = include_str!("../../src/session_protocol/tests/fixtures/follows.spx")
        .split_once("\n")
        .unwrap()
        .1
        .replace("fn main()", "fn protocol_main()");
    let protocol = if follows {
        protocol
    } else {
        protocol.replace(
            "    follows session protocol \"fixture.follows.protocol\"\n",
            "",
        )
    };
    format!("{source}\n{protocol}")
}

/// The Project SDK additionally derives genuine Proposal/Observation schemas.
/// Keep the package's scalar metadata-only fixture unchanged.
pub(super) fn workspace_source(embedded: bool, wait: bool) -> String {
    let source = source(embedded, wait);
    let mut declarations = String::new();
    for (role, name) in [
        ("task", "Task"),
        ("state", "State"),
        ("observation", "Observation"),
        ("proposal", "Proposal"),
        ("outcome", "Outcome"),
        ("result", "Result"),
    ] {
        declarations.push_str(&format!(
            "@id(\"fixture.agent.type.{role}\") record {name} {{ @id(\"fixture.agent.type.{role}.value\") value:i64; }}\n"
        ));
    }
    source.replacen(
        "module fixture.app;\n",
        &format!("module fixture.app;\n{declarations}"),
        1,
    )
}
