use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use semaprax::agent_definition::{compile_agent_definition, CompiledAgentDefinition};
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, ProgramRoot, ProjectRevision, SemanticWorkspaceRevision,
    MAX_SEMANTIC_WORKSPACE_AGENT_DEFINITIONS,
};
use serde_json::Value;

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-agent-definition-association-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for file in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(example.join(file), root.join(file)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }

    fn revision(&self) -> Arc<ProjectRevision> {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn inventory(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut entries = BTreeMap::new();
    for path in [
        "semaprax.toml",
        "src/app.spx",
        "src/core.spx",
        "src/tests.spx",
    ] {
        entries.insert(PathBuf::from(path), std::fs::read(root.join(path)).unwrap());
    }
    entries
}

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

fn definition(agent_id: &str) -> CompiledAgentDefinition {
    let profile = profile(agent_id);
    let body = profile.strip_suffix('\n').unwrap();
    let members = body
        .strip_prefix(&format!(
            "{{\"schema\":\"semaprax.agent-runtime-profile.v1\",\"agent_id\":\"{agent_id}\","
        ))
        .unwrap();
    let (runtime, _) = members.split_once(",\"nonclaims\":").unwrap();
    let source = concat!(
        "{\"schema\":\"semaprax.agent-definition.v1\",\"agent_id\":\"AGENT\",",
        "\"types\":[",
        "{\"role\":\"task\",\"stable_id\":\"AGENT.type.task\"},",
        "{\"role\":\"state\",\"stable_id\":\"AGENT.type.state\"},",
        "{\"role\":\"observation\",\"stable_id\":\"AGENT.type.observation\"},",
        "{\"role\":\"proposal\",\"stable_id\":\"AGENT.type.proposal\"},",
        "{\"role\":\"outcome\",\"stable_id\":\"AGENT.type.outcome\"},",
        "{\"role\":\"result\",\"stable_id\":\"AGENT.type.result\"}],",
        "\"operations\":[",
        "{\"role\":\"initialize\",\"stable_id\":\"AGENT.fn.initialize\",\"kind\":\"deterministic\"},",
        "{\"role\":\"observe\",\"stable_id\":\"AGENT.fn.observe\",\"kind\":\"deterministic\"},",
        "{\"role\":\"propose\",\"stable_id\":\"AGENT.fn.propose\",\"kind\":\"model\"},",
        "{\"role\":\"authorize\",\"stable_id\":\"AGENT.fn.authorize\",\"kind\":\"deterministic\"},",
        "{\"role\":\"execute\",\"stable_id\":\"AGENT.fn.execute\",\"kind\":\"effect\"},",
        "{\"role\":\"reduce\",\"stable_id\":\"AGENT.fn.reduce\",\"kind\":\"deterministic\"}],",
        "\"runtime_v1\":RUNTIME}\n"
    )
    .replace("AGENT", agent_id)
    .replace("RUNTIME", &format!("{{{runtime}}}"));
    compile_agent_definition(&source).unwrap()
}

fn assert_code<T>(result: Result<T, Vec<Diagnostic>>, code: &str) {
    match result {
        Ok(_) => panic!("expected {code}"),
        Err(errors) => assert!(errors.iter().any(|error| error.code == code), "{errors:?}"),
    }
}

#[test]
fn explicit_agent_definitions_populate_workspace_and_program_root_with_exact_replay() {
    let fixture = Fixture::new();
    let before = inventory(&fixture.0);
    let revision = fixture.revision();
    let legacy_before = SemanticWorkspaceRevision::derive(&revision).unwrap();
    let first = definition("alpha.agent");
    let second = definition("beta.agent");
    assert_code(
        SemanticWorkspaceRevision::derive_with_agent_definitions(
            &revision,
            revision.project_revision(),
            &[],
        ),
        "SPX-G222",
    );
    let workspace = SemanticWorkspaceRevision::derive_with_agent_definitions(
        &revision,
        revision.project_revision(),
        &[&first, &second],
    )
    .unwrap();
    let repeated = SemanticWorkspaceRevision::derive_with_agent_definitions(
        &revision,
        revision.project_revision(),
        &[&first, &second],
    )
    .unwrap();
    assert_eq!(workspace, repeated);
    let agents: Value = serde_json::from_str(workspace.agent_definitions().to_json()).unwrap();
    assert_eq!(
        agents["payload"]["integration"],
        "explicit_compiler_admitted_association_input"
    );
    assert_eq!(
        agents["payload"]["expected_project_revision"],
        revision.project_revision()
    );
    assert_eq!(
        agents["payload"]["definitions"][0]["agent_id"],
        "alpha.agent"
    );
    assert_eq!(
        agents["payload"]["definitions"][0]["agent_definition"],
        first.definition().canonical_source()
    );
    assert_eq!(
        agents["payload"]["definitions"][0]["agent_graph"],
        first.graph().canonical_json()
    );
    let replay = SemanticWorkspaceRevision::replay_with_agent_definitions(
        &revision,
        revision.project_revision(),
        &[&first, &second],
        workspace.workspace_revision(),
        workspace.to_json().as_bytes(),
    )
    .unwrap();
    assert_eq!(replay, workspace);
    let root = ProgramRoot::derive(&workspace).unwrap();
    let agent_segment = root
        .segments()
        .iter()
        .find(|segment| segment.kind() == "agent_definitions")
        .unwrap();
    assert_eq!(
        agent_segment.node_digest(),
        workspace.agent_definitions().digest()
    );
    let legacy_after = SemanticWorkspaceRevision::derive(&revision).unwrap();
    assert_eq!(legacy_after.to_json(), legacy_before.to_json());
    assert_eq!(
        legacy_after.workspace_revision(),
        legacy_before.workspace_revision()
    );
    assert_eq!(inventory(&fixture.0), before);
}

#[test]
fn association_rejects_stale_unordered_duplicate_and_cross_paired_inputs() {
    let fixture = Fixture::new();
    let revision = fixture.revision();
    let first = definition("alpha.agent");
    let second = definition("beta.agent");
    assert_code(
        SemanticWorkspaceRevision::derive_with_agent_definitions(
            &revision,
            &format!("sha256:{}", "0".repeat(64)),
            &[&first],
        ),
        "SPX-G223",
    );
    assert_code(
        SemanticWorkspaceRevision::derive_with_agent_definitions(
            &revision,
            revision.project_revision(),
            &[&second, &first],
        ),
        "SPX-G222",
    );
    assert_code(
        SemanticWorkspaceRevision::derive_with_agent_definitions(
            &revision,
            revision.project_revision(),
            &[&first, &first],
        ),
        "SPX-G222",
    );
    let workspace = SemanticWorkspaceRevision::derive_with_agent_definitions(
        &revision,
        revision.project_revision(),
        &[&first],
    )
    .unwrap();
    assert_code(
        SemanticWorkspaceRevision::replay_with_agent_definitions(
            &revision,
            revision.project_revision(),
            &[&second],
            workspace.workspace_revision(),
            workspace.to_json().as_bytes(),
        ),
        "SPX-G223",
    );
    let too_many = (0..=MAX_SEMANTIC_WORKSPACE_AGENT_DEFINITIONS)
        .map(|_| &first)
        .collect::<Vec<_>>();
    assert_code(
        SemanticWorkspaceRevision::derive_with_agent_definitions(
            &revision,
            revision.project_revision(),
            &too_many,
        ),
        "SPX-G222",
    );
}

#[test]
fn default_empty_derivation_and_replay_bytes_remain_unchanged() {
    let fixture = Fixture::new();
    let revision = fixture.revision();
    let via_project = revision.canonical_workspace_revision().unwrap();
    let direct = SemanticWorkspaceRevision::derive(&revision).unwrap();
    assert_eq!(via_project.to_json(), direct.to_json());
    let agents: Value = serde_json::from_str(direct.agent_definitions().to_json()).unwrap();
    assert_eq!(agents["payload"]["definitions"], Value::Array(Vec::new()));
    assert_eq!(
        agents["payload"]["integration"],
        "no_project_agent_definition_declarations"
    );
    assert_eq!(
        SemanticWorkspaceRevision::replay(
            &revision,
            direct.workspace_revision(),
            direct.to_json().as_bytes(),
        )
        .unwrap(),
        direct
    );
}
