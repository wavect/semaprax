//! Candidate-safe exact ProgramRoot-v3 refresh regressions.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use semaprax::agent_definition::{compile_agent_definition, CompiledAgentDefinition};
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    render_project_lock, with_authenticated_project, ExactProgramContext, ExactProgramContextV2,
    ImageArtifactKind, InterfaceArtifactFacts, ProgramRootV2, ProjectRevision,
    SemanticWorkspaceRevision, MAX_IMAGE_ARTIFACT_BUILD_BYTES,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new(label: &str, operator: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-candidate-exact-v3-{label}-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed),
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
        if operator != "+" {
            let path = root.join("src/core.spx");
            let source = std::fs::read_to_string(&path).unwrap();
            let source = source.replacen("left + right", &format!("left {operator} right"), 1);
            let parsed = semaprax::parse(&source, &path).unwrap();
            std::fs::write(&path, semaprax::format::canonical(&parsed)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }

    fn revision(&self) -> Arc<ProjectRevision> {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap()
    }

    fn exact_context(&self) -> Arc<ExactProgramContextV2> {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            let revision = snapshot.retain_revision();
            let default_workspace = snapshot.canonical_workspace_revision()?;
            let base_root = default_workspace.program_root()?;
            let lock = render_project_lock(snapshot)?;
            let association = base_root.associate_dependency_lock(
                snapshot,
                base_root.program_root_digest(),
                &lock,
            )?;
            let definition = definition();
            let workspace = SemanticWorkspaceRevision::derive_with_agent_definitions(
                &revision,
                revision.project_revision(),
                &[&definition],
            )?;
            let interface = InterfaceArtifactFacts::derive(
                Arc::clone(&revision),
                revision.project_revision(),
                &[ImageArtifactKind::Web],
                MAX_IMAGE_ARTIFACT_BUILD_BYTES,
            )?;
            let root_v2 = ProgramRootV2::derive(&workspace, &base_root, &interface, &association)?;
            let root_v2_digest = root_v2.program_root_v2_digest().to_owned();
            let context_v1 = ExactProgramContext::derive(
                revision,
                snapshot.project_revision(),
                workspace.clone(),
                workspace.workspace_revision(),
                interface,
                association,
                root_v2,
                &root_v2_digest,
            )?;
            ExactProgramContextV2::assemble(Arc::new(context_v1)).map(Arc::new)
        })
        .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn definition() -> CompiledAgentDefinition {
    let nonclaims = r#"["no_compiler_determinism_from_model_output","no_model_output_authority","no_provider_identity_provenance_or_quality_truth","no_secret_input_or_secret_leakage_guarantee_for_caller_supplied_content","no_credential_prompt_state_trace_or_diagnostic_exposure","no_ambient_network_filesystem_process_home_or_environment_authority","no_write_apply_mutation_or_target_execution_tool_authority","no_capability_minting_delegation_or_self_approval","no_human_approval_ui_or_policy","no_semantic_prompt_injection_proof","no_forced_cancellation_or_preemption","no_exactly_once_provider_billing_or_retry","no_durable_memory_persistence_recovery_or_resume","no_crash_reboot_or_power_loss_durability","no_distributed_or_parallel_execution","no_model_quality_accuracy_or_completion_guarantee","no_live_price_or_cost_accuracy_guarantee","no_reusable_authorization_token","no_signature_attestation_or_authenticated_provenance","no_wallet_payment_signing_asset_or_economic_authority","no_privacy_compliance_or_data_residency_guarantee","no_general_formal_proof","no_new_language_graph_cleanup_backend_or_runtime_semantics","no_current_schema_api_or_kat_modification"]"#;
    let profile = concat!(
        "{\"schema\":\"semaprax.agent-runtime-profile.v1\",\"agent_id\":\"root.agent\",",
        "\"models\":[{\"provider_id\":\"fake.local\",\"model_id\":\"fake-basic\",",
        "\"locality\":\"local\",\"quality_tier\":\"basic\",\"tokenizer_id\":\"fake.bytes-v1\",",
        "\"max_context_tokens\":4096,\"input_usd_microunits_per_million_tokens\":0,",
        "\"output_usd_microunits_per_million_tokens\":0,\"capabilities\":[\"text\"]}],",
        "\"tools\":[],\"policy\":{\"allowed_provider_ids\":[\"fake.local\"],",
        "\"allowed_model_ids\":[\"fake-basic\"],\"required_locality\":\"local_only\",",
        "\"minimum_quality_tier\":\"basic\",\"required_model_capabilities\":[\"text\"],",
        "\"granted_capabilities\":[],\"allowed_tool_ids\":[]},",
        "\"limits\":{\"max_turns\":2,\"max_provider_attempts\":2,\"max_retries_per_turn\":1,",
        "\"max_concurrency\":1,\"max_elapsed_ms\":1000,\"max_provider_request_bytes\":65536,",
        "\"max_provider_response_bytes\":4096,\"max_stream_chunks\":64,",
        "\"max_total_provider_input_bytes\":131072,\"max_total_provider_output_bytes\":8192,",
        "\"max_reported_model_input_tokens\":131072,\"max_reported_model_output_tokens\":8192,",
        "\"max_usd_microunits\":0,\"max_tool_calls\":0,\"max_tool_arguments_bytes\":4096,",
        "\"max_tool_result_bytes\":4096,\"max_total_tool_bytes\":8192,",
        "\"max_retained_state_bytes\":131072,\"max_trace_events\":64,\"max_trace_bytes\":131072,",
        "\"max_evidence_bytes\":262144,\"max_builder_bytes\":1048576},\"nonclaims\":NONCLAIMS}\n"
    )
    .replace("NONCLAIMS", nonclaims);
    let body = profile.strip_suffix('\n').unwrap();
    let members = body
        .strip_prefix(
            "{\"schema\":\"semaprax.agent-runtime-profile.v1\",\"agent_id\":\"root.agent\",",
        )
        .unwrap();
    let (runtime, _) = members.split_once(",\"nonclaims\":").unwrap();
    let source = concat!(
        "{\"schema\":\"semaprax.agent-definition.v1\",\"agent_id\":\"root.agent\",",
        "\"types\":[{\"role\":\"task\",\"stable_id\":\"root.agent.type.task\"},",
        "{\"role\":\"state\",\"stable_id\":\"root.agent.type.state\"},",
        "{\"role\":\"observation\",\"stable_id\":\"root.agent.type.observation\"},",
        "{\"role\":\"proposal\",\"stable_id\":\"root.agent.type.proposal\"},",
        "{\"role\":\"outcome\",\"stable_id\":\"root.agent.type.outcome\"},",
        "{\"role\":\"result\",\"stable_id\":\"root.agent.type.result\"}],",
        "\"operations\":[{\"role\":\"initialize\",\"stable_id\":\"root.agent.fn.initialize\",\"kind\":\"deterministic\"},",
        "{\"role\":\"observe\",\"stable_id\":\"root.agent.fn.observe\",\"kind\":\"deterministic\"},",
        "{\"role\":\"propose\",\"stable_id\":\"root.agent.fn.propose\",\"kind\":\"model\"},",
        "{\"role\":\"authorize\",\"stable_id\":\"root.agent.fn.authorize\",\"kind\":\"deterministic\"},",
        "{\"role\":\"execute\",\"stable_id\":\"root.agent.fn.execute\",\"kind\":\"effect\"},",
        "{\"role\":\"reduce\",\"stable_id\":\"root.agent.fn.reduce\",\"kind\":\"deterministic\"}],",
        "\"runtime_v1\":RUNTIME}\n"
    )
    .replace("RUNTIME", &format!("{{{runtime}}}"));
    compile_agent_definition(&source).unwrap()
}

fn assert_code<T>(result: Result<T, Vec<Diagnostic>>, code: &str) {
    let errors = result.err().unwrap_or_else(|| panic!("expected {code}"));
    assert!(errors.iter().any(|error| error.code == code), "{errors:?}");
}

#[test]
fn candidate_context_is_replayed_and_matches_independently_admitted_revision() {
    let base = Fixture::new("base", "+");
    let candidate = Fixture::new("candidate", "-");
    let current = base.exact_context();
    let proposed = candidate.exact_context();
    let independently_admitted = candidate.revision();
    let current_bytes = current.to_json().to_owned();
    let proposed_bytes = proposed.to_json().to_owned();

    let refreshed = ExactProgramContextV2::refresh_candidate(
        &current,
        &independently_admitted,
        Arc::clone(&proposed),
    )
    .unwrap();

    assert_eq!(current.to_json(), current_bytes);
    assert_eq!(refreshed.to_json(), proposed_bytes);
    assert_eq!(
        refreshed.program_root_v3().to_json(),
        proposed.program_root_v3().to_json()
    );
    assert_eq!(
        refreshed.contracts_and_tests_facts().project_revision(),
        independently_admitted.project_revision()
    );
    assert_ne!(
        refreshed.exact_program_context_v1().project_revision(),
        current.exact_program_context_v1().project_revision()
    );
}

#[test]
fn cross_candidate_context_fails_closed_without_changing_retained_contexts() {
    let base = Fixture::new("base-stale", "+");
    let candidate = Fixture::new("candidate-stale", "-");
    let foreign = Fixture::new("foreign-stale", "*");
    let current = base.exact_context();
    let proposed = candidate.exact_context();
    let foreign_revision = foreign.revision();
    let current_bytes = current.to_json().to_owned();
    let proposed_bytes = proposed.to_json().to_owned();

    assert_code(
        ExactProgramContextV2::refresh_candidate(
            &current,
            &foreign_revision,
            Arc::clone(&proposed),
        ),
        "SPX-G577",
    );
    assert_eq!(current.to_json(), current_bytes);
    assert_eq!(proposed.to_json(), proposed_bytes);
}
