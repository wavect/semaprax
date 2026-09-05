use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::agent_definition::compile_agent_definition;
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    render_project_lock, with_authenticated_project, ImageArtifactKind, InterfaceArtifactFacts,
    ProgramRootV2, SemanticWorkspaceRevision, MAX_IMAGE_ARTIFACT_BUILD_BYTES,
    PROGRAM_ROOT_DEPENDENCY_LOCK_ASSOCIATION_SCHEMA, PROGRAM_ROOT_SEGMENT_SCHEMA,
    PROGRAM_ROOT_V2_SCHEMA,
};
use serde_json::Value;
use sha2::{Digest, Sha256};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-program-root-v2-{label}-{}-{}",
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
        Self(root.canonicalize().unwrap())
    }

    fn manifest(&self) -> PathBuf {
        self.0.join("semaprax.toml")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn nonclaims() -> &'static str {
    r#"["no_compiler_determinism_from_model_output","no_model_output_authority","no_provider_identity_provenance_or_quality_truth","no_secret_input_or_secret_leakage_guarantee_for_caller_supplied_content","no_credential_prompt_state_trace_or_diagnostic_exposure","no_ambient_network_filesystem_process_home_or_environment_authority","no_write_apply_mutation_or_target_execution_tool_authority","no_capability_minting_delegation_or_self_approval","no_human_approval_ui_or_policy","no_semantic_prompt_injection_proof","no_forced_cancellation_or_preemption","no_exactly_once_provider_billing_or_retry","no_durable_memory_persistence_recovery_or_resume","no_crash_reboot_or_power_loss_durability","no_distributed_or_parallel_execution","no_model_quality_accuracy_or_completion_guarantee","no_live_price_or_cost_accuracy_guarantee","no_reusable_authorization_token","no_signature_attestation_or_authenticated_provenance","no_wallet_payment_signing_asset_or_economic_authority","no_privacy_compliance_or_data_residency_guarantee","no_general_formal_proof","no_new_language_graph_cleanup_backend_or_runtime_semantics","no_current_schema_api_or_kat_modification"]"#
}

pub(super) fn definition() -> semaprax::agent_definition::CompiledAgentDefinition {
    let profile = concat!(
        "{\"schema\":\"semaprax.agent-runtime-profile.v1\",\"agent_id\":\"root.agent\",",
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
    .replace("NONCLAIMS", nonclaims());
    let body = profile.strip_suffix('\n').unwrap();
    let members = body
        .strip_prefix(
            "{\"schema\":\"semaprax.agent-runtime-profile.v1\",\"agent_id\":\"root.agent\",",
        )
        .unwrap();
    let (runtime, _) = members.split_once(",\"nonclaims\":").unwrap();
    let source = concat!(
        "{\"schema\":\"semaprax.agent-definition.v1\",\"agent_id\":\"root.agent\",",
        "\"types\":[",
        "{\"role\":\"task\",\"stable_id\":\"root.agent.type.task\"},",
        "{\"role\":\"state\",\"stable_id\":\"root.agent.type.state\"},",
        "{\"role\":\"observation\",\"stable_id\":\"root.agent.type.observation\"},",
        "{\"role\":\"proposal\",\"stable_id\":\"root.agent.type.proposal\"},",
        "{\"role\":\"outcome\",\"stable_id\":\"root.agent.type.outcome\"},",
        "{\"role\":\"result\",\"stable_id\":\"root.agent.type.result\"}],",
        "\"operations\":[",
        "{\"role\":\"initialize\",\"stable_id\":\"root.agent.fn.initialize\",\"kind\":\"deterministic\"},",
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

fn framed(domain: &[u8], bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(digest.finalize())
    )
}

fn canonical(mut value: Value) -> String {
    value.sort_all_objects();
    serde_json::to_string(&value).unwrap() + "\n"
}

fn remint_v2(value: &mut Value) -> String {
    const DOMAIN: &[u8] = b"semaprax.program-root.digest.v2\0";
    value
        .as_object_mut()
        .unwrap()
        .remove("program_root_v2_digest");
    let identity = canonical(value.clone());
    let digest = framed(DOMAIN, identity.as_bytes());
    value["program_root_v2_digest"] = Value::String(digest);
    canonical(value.clone())
}

fn assert_code<T>(result: Result<T, Vec<Diagnostic>>, code: &str) {
    let errors = result.err().unwrap_or_else(|| panic!("expected {code}"));
    assert!(errors.iter().any(|error| error.code == code), "{errors:?}");
}

#[test]
fn v2_retains_v1_segments_and_appends_exact_typed_facts() {
    let fixture = Fixture::new("exact");
    with_authenticated_project(&fixture.manifest(), |snapshot| {
        let revision = snapshot.retain_revision();
        let base_workspace = snapshot.canonical_workspace_revision()?;
        let base_root = base_workspace.program_root()?;
        let lock = render_project_lock(snapshot)?;
        let lock_association = base_root.associate_dependency_lock(
            snapshot,
            base_root.program_root_digest(),
            &lock,
        )?;
        let facts = InterfaceArtifactFacts::derive(
            revision.clone(),
            revision.project_revision(),
            &[ImageArtifactKind::Web],
            MAX_IMAGE_ARTIFACT_BUILD_BYTES,
        )?;
        let agent = definition();
        let workspace = SemanticWorkspaceRevision::derive_with_agent_definitions(
            &revision,
            revision.project_revision(),
            &[&agent],
        )?;
        let semantic_root = workspace.program_root()?;
        let v2 = ProgramRootV2::derive(&workspace, &base_root, &facts, &lock_association)?;

        assert_ne!(
            v2.base_project_root_digest(),
            v2.semantic_workspace_root_digest()
        );
        assert_eq!(
            v2.semantic_workspace_revision(),
            workspace.workspace_revision()
        );
        assert_eq!(v2.segments().len(), 11);
        for (retained, original) in v2.segments()[..9].iter().zip(semantic_root.segments()) {
            assert_eq!(retained, original);
            assert_eq!(retained.to_json().as_bytes(), original.to_json().as_bytes());
        }
        assert_eq!(v2.segments()[9].kind(), "interface_artifact_facts");
        assert_eq!(v2.segments()[9].node_digest(), facts.digest());
        assert_eq!(v2.segments()[10].kind(), "project_lock_association");
        assert_eq!(
            v2.segments()[10].node_schema(),
            PROGRAM_ROOT_DEPENDENCY_LOCK_ASSOCIATION_SCHEMA
        );
        assert_eq!(
            v2.segments()[10].node_digest(),
            lock_association.association_digest()
        );
        assert!(v2
            .segments()
            .iter()
            .all(|segment| segment.to_json().contains(PROGRAM_ROOT_SEGMENT_SCHEMA)));
        assert_eq!(v2.relationships(), semantic_root.relationships());
        assert!(v2.relationships().iter().all(|relationship| {
            relationship.binding() == "unbound" && relationship.digest().is_none()
        }));
        let value: Value = serde_json::from_str(v2.to_json()).unwrap();
        assert_eq!(value["schema"], PROGRAM_ROOT_V2_SCHEMA);
        assert_eq!(
            value["base_project_root_digest"],
            base_root.program_root_digest()
        );
        assert_eq!(
            value["semantic_workspace_root_digest"],
            semantic_root.program_root_digest()
        );

        let replayed = ProgramRootV2::replay(
            &workspace,
            &base_root,
            &facts,
            &lock_association,
            v2.program_root_v2_digest(),
            v2.to_json().as_bytes(),
        )?;
        assert_eq!(replayed, v2);
        assert_eq!(snapshot.canonical_workspace_revision()?, base_workspace);
        assert_eq!(base_workspace.program_root()?, base_root);
        Ok(())
    })
    .unwrap();
}

#[test]
fn empty_agents_wrong_project_and_hostile_v2_wires_fail_closed() {
    let first = Fixture::new("first");
    let second = Fixture::new("second");
    let second_core = second.0.join("src/core.spx");
    let changed = std::fs::read_to_string(&second_core)
        .unwrap()
        .replace("left + right", "left + right + 1");
    std::fs::write(second_core, changed).unwrap();
    let mut second_facts = None;
    with_authenticated_project(&second.manifest(), |snapshot| {
        let revision = snapshot.retain_revision();
        second_facts = Some(InterfaceArtifactFacts::derive(
            revision.clone(),
            revision.project_revision(),
            &[ImageArtifactKind::Web],
            MAX_IMAGE_ARTIFACT_BUILD_BYTES,
        )?);
        Ok(())
    })
    .unwrap();

    with_authenticated_project(&first.manifest(), |snapshot| {
        let revision = snapshot.retain_revision();
        let base_workspace = snapshot.canonical_workspace_revision()?;
        let base_root = base_workspace.program_root()?;
        let lock = render_project_lock(snapshot)?;
        let lock_association = base_root.associate_dependency_lock(
            snapshot,
            base_root.program_root_digest(),
            &lock,
        )?;
        let facts = InterfaceArtifactFacts::derive(
            revision.clone(),
            revision.project_revision(),
            &[ImageArtifactKind::Web],
            MAX_IMAGE_ARTIFACT_BUILD_BYTES,
        )?;
        assert_code(
            ProgramRootV2::derive(&base_workspace, &base_root, &facts, &lock_association),
            "SPX-G550",
        );

        let agent = definition();
        let workspace = SemanticWorkspaceRevision::derive_with_agent_definitions(
            &revision,
            revision.project_revision(),
            &[&agent],
        )?;
        assert_code(
            ProgramRootV2::derive(
                &workspace,
                &base_root,
                second_facts.as_ref().unwrap(),
                &lock_association,
            ),
            "SPX-G551",
        );
        let v2 = ProgramRootV2::derive(&workspace, &base_root, &facts, &lock_association)?;

        let mut reordered: Value = serde_json::from_str(v2.to_json()).unwrap();
        reordered["segments"].as_array_mut().unwrap().swap(9, 10);
        let reordered = remint_v2(&mut reordered);
        let digest = serde_json::from_str::<Value>(&reordered).unwrap()["program_root_v2_digest"]
            .as_str()
            .unwrap()
            .to_owned();
        assert_code(
            ProgramRootV2::replay(
                &workspace,
                &base_root,
                &facts,
                &lock_association,
                &digest,
                reordered.as_bytes(),
            ),
            "SPX-G550",
        );

        let mut cycle: Value = serde_json::from_str(v2.to_json()).unwrap();
        cycle["relationships"][0]["binding"] = Value::String("bound".to_owned());
        cycle["relationships"][0]["digest"] = Value::String(v2.program_root_v2_digest().to_owned());
        let cycle = remint_v2(&mut cycle);
        let digest = serde_json::from_str::<Value>(&cycle).unwrap()["program_root_v2_digest"]
            .as_str()
            .unwrap()
            .to_owned();
        assert_code(
            ProgramRootV2::replay(
                &workspace,
                &base_root,
                &facts,
                &lock_association,
                &digest,
                cycle.as_bytes(),
            ),
            "SPX-G550",
        );
        Ok(())
    })
    .unwrap();
}
