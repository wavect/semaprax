use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use semaprax::agent_runtime::{
    Agent, AgentBoundaryProbe, AgentCancellation, AgentHost, AgentProviderAttempt,
    AgentProviderDisposition, AgentProviderSink, AgentProviderUsage, AgentRunStatus,
    AgentToolResultSink,
};
use sha2::{Digest, Sha256};

const TRACE_DOMAIN: &[u8] = b"semaprax.agent-runtime.trace-digest.v1\0";
const EVIDENCE_DOMAIN: &[u8] = b"semaprax.agent-runtime.evidence-digest.v1\0";

fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(bytes);
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(digest.finalize())
    )
}

fn raw_sha(source: &str) -> String {
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(source.as_bytes()))
    )
}

fn nonclaims() -> &'static str {
    r#"["no_compiler_determinism_from_model_output","no_model_output_authority","no_provider_identity_provenance_or_quality_truth","no_secret_input_or_secret_leakage_guarantee_for_caller_supplied_content","no_credential_prompt_state_trace_or_diagnostic_exposure","no_ambient_network_filesystem_process_home_or_environment_authority","no_write_apply_mutation_or_target_execution_tool_authority","no_capability_minting_delegation_or_self_approval","no_human_approval_ui_or_policy","no_semantic_prompt_injection_proof","no_forced_cancellation_or_preemption","no_exactly_once_provider_billing_or_retry","no_durable_memory_persistence_recovery_or_resume","no_crash_reboot_or_power_loss_durability","no_distributed_or_parallel_execution","no_model_quality_accuracy_or_completion_guarantee","no_live_price_or_cost_accuracy_guarantee","no_reusable_authorization_token","no_signature_attestation_or_authenticated_provenance","no_wallet_payment_signing_asset_or_economic_authority","no_privacy_compliance_or_data_residency_guarantee","no_general_formal_proof","no_new_language_graph_cleanup_backend_or_runtime_semantics","no_current_schema_api_or_kat_modification"]"#
}

fn profile() -> String {
    concat!(
            "{\"schema\":\"semaprax.agent-runtime-profile.v1\",\"agent_id\":\"fixture.agent\",",
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
            "\"max_evidence_bytes\":262144,\"max_builder_bytes\":1048576},\"nonclaims\":NONCLAIMS",
            "}\n"
    )
    .replace("NONCLAIMS", nonclaims())
}

fn task() -> String {
    format!(
        "{{\"schema\":\"semaprax.agent-runtime-task.v1\",\"nonce\":\"{}\",\"objective\":\"Return one bounded answer.\",\"context\":[{{\"label\":\"input\",\"provenance\":\"caller_untrusted\",\"content\":\"alpha\"}}]}}\n",
        "0".repeat(64)
    )
}

#[derive(Clone)]
struct Probe {
    epoch: Arc<AtomicU64>,
    elapsed: Arc<AtomicU64>,
}

impl AgentBoundaryProbe for Probe {
    fn policy_epoch(&self) -> u64 {
        self.epoch.load(Ordering::Acquire)
    }

    fn elapsed_ms(&self) -> u64 {
        self.elapsed.load(Ordering::Acquire)
    }
}

struct Host {
    probe: Probe,
    provider_calls: Arc<AtomicUsize>,
    tool_calls: Arc<AtomicUsize>,
    retry_first: bool,
    retry_to_final: bool,
    uncertain_first: bool,
    tokenize_none: bool,
    tool_succeeds: bool,
    provider_overflow: bool,
    cancellation: Option<AgentCancellation>,
    cancel_provider_after_first: bool,
    cancel_tool_before_push: bool,
    final_message: String,
    secret: &'static str,
}

impl Host {
    fn new() -> Self {
        Self {
            probe: Probe {
                epoch: Arc::new(AtomicU64::new(7)),
                elapsed: Arc::new(AtomicU64::new(0)),
            },
            provider_calls: Arc::new(AtomicUsize::new(0)),
            tool_calls: Arc::new(AtomicUsize::new(0)),
            retry_first: false,
            retry_to_final: false,
            uncertain_first: false,
            tokenize_none: false,
            tool_succeeds: true,
            provider_overflow: false,
            cancellation: None,
            cancel_provider_after_first: false,
            cancel_tool_before_push: false,
            final_message: "done".to_owned(),
            secret: "public-host-secret-sentinel",
        }
    }

    fn with_final_message(mut self, final_message: String) -> Self {
        self.retry_to_final = true;
        self.final_message = final_message;
        self
    }
}

impl AgentHost for Host {
    fn policy_epoch(&self) -> u64 {
        self.probe.policy_epoch()
    }

    fn elapsed_ms(&self) -> u64 {
        self.probe.elapsed_ms()
    }

    fn boundary_probe(&self) -> Box<dyn AgentBoundaryProbe> {
        Box::new(self.probe.clone())
    }

    fn tokenize(&mut self, _: &str, request: &str) -> Option<u64> {
        assert!(!request.contains(self.secret));
        (!self.tokenize_none).then_some(request.len() as u64)
    }

    fn attempt_provider(
        &mut self,
        provider_id: &str,
        model_id: &str,
        request: &str,
        _: u64,
        sink: &mut AgentProviderSink,
    ) -> AgentProviderAttempt {
        assert_eq!(provider_id, "fake.local");
        assert_eq!(model_id, "fake-basic");
        assert!(request.ends_with('\n'));
        let call = self.provider_calls.fetch_add(1, Ordering::AcqRel);
        if self.uncertain_first && call == 0 {
            return AgentProviderAttempt::new(
                AgentProviderDisposition::FailedUncertain,
                AgentProviderUsage::default(),
            );
        }
        if self.retry_first && call == 0 {
            return AgentProviderAttempt::new(
                AgentProviderDisposition::DefinitelyNotStarted,
                AgentProviderUsage::default(),
            );
        }
        let response = if !self.retry_to_final && call == usize::from(self.retry_first) {
            b"{\"schema\":\"semaprax.agent-runtime-action.v1\",\"kind\":\"tool\",\"tool_id\":\"fixture.read\",\"arguments\":{\"query\":\"alpha\"}}\n".to_vec()
        } else {
            format!(
                "{{\"schema\":\"semaprax.agent-runtime-action.v1\",\"kind\":\"final\",\"message\":{}}}\n",
                serde_json::to_string(&self.final_message).unwrap()
            )
            .into_bytes()
        };
        assert!(sink.push(&response[..response.len() / 2]));
        if self.cancel_provider_after_first {
            self.cancellation.as_ref().unwrap().cancel();
            assert!(!sink.push(&response[response.len() / 2..]));
            assert!(!sink.push(b"x"));
            return AgentProviderAttempt::new(
                AgentProviderDisposition::Succeeded,
                AgentProviderUsage::new(request.len() as u64, 0, 0),
            );
        }
        assert!(sink.push(&response[response.len() / 2..]));
        if self.provider_overflow {
            assert!(!sink.push(&vec![b'x'; 4097]));
            assert!(!sink.push(b"x"));
        }
        AgentProviderAttempt::new(
            AgentProviderDisposition::Succeeded,
            AgentProviderUsage::new(request.len() as u64, response.len() as u64, 0),
        )
    }

    fn invoke_tool(
        &mut self,
        call_id: &str,
        tool_id: &str,
        arguments_json: &str,
        sink: &mut AgentToolResultSink,
    ) -> bool {
        assert!(call_id.starts_with("sha256:"));
        assert_eq!(tool_id, "fixture.read");
        assert_eq!(arguments_json, "{\"query\":\"alpha\"}");
        self.tool_calls.fetch_add(1, Ordering::AcqRel);
        if self.cancel_tool_before_push {
            self.cancellation.as_ref().unwrap().cancel();
            assert!(!sink.push(b"{\"value\":\"alpha\"}"));
            assert!(!sink.push(b"x"));
            return true;
        }
        self.tool_succeeds && sink.push(b"{\"value\":\"alpha\"}")
    }
}

fn inventory(root: &Path) -> BTreeSet<(PathBuf, Vec<u8>)> {
    fn walk(root: &Path, path: &Path, output: &mut BTreeSet<(PathBuf, Vec<u8>)>) {
        let mut entries = fs::read_dir(path)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        entries.sort();
        for entry in entries {
            if entry.is_dir() {
                walk(root, &entry, output);
            } else {
                output.insert((
                    entry.strip_prefix(root).unwrap().to_owned(),
                    fs::read(entry).unwrap(),
                ));
            }
        }
    }
    let mut output = BTreeSet::new();
    walk(root, root, &mut output);
    output
}

#[test]
fn public_agent_completes_one_tool_with_private_kat_parity_and_no_write() {
    let usage = AgentProviderUsage::new(11, 7, 3);
    assert_eq!(usage.input_tokens(), 11);
    assert_eq!(usage.output_tokens(), 7);
    assert_eq!(usage.usd_microunits(), 3);
    let attempt = AgentProviderAttempt::new(AgentProviderDisposition::Succeeded, usage);
    assert_eq!(attempt.disposition(), AgentProviderDisposition::Succeeded);
    assert_eq!(attempt.usage(), usage);
    assert_eq!(
        raw_sha(&profile()),
        "sha256:14981ee99af965dcea311121a90cacfb9891a00d6365e7ad00cab8cefe69c01a"
    );
    assert_eq!(
        raw_sha(&task()),
        "sha256:b6be370dea6708b7b3f7c6bd46299061c8f146a684fdf9895c32dc7e50c9a425"
    );
    let root = std::env::temp_dir().join(format!(
        "semaprax-agent-public-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    fs::write(root.join("sentinel"), b"unchanged").unwrap();
    let before = inventory(&root);
    let host = Host::new();
    let providers = Arc::clone(&host.provider_calls);
    let tools = Arc::clone(&host.tool_calls);
    let cancellation = AgentCancellation::new();
    let mut agent = Agent::new(&profile(), host, cancellation).unwrap();
    let run = agent.run(&task()).unwrap();
    assert_eq!(run.status(), AgentRunStatus::Completed);
    assert_eq!(run.final_message(), Some("done"));
    assert_eq!(providers.load(Ordering::Acquire), 2);
    assert_eq!(tools.load(Ordering::Acquire), 1);
    assert_eq!(
        run.trace_digest(),
        digest(TRACE_DOMAIN, run.trace().as_bytes())
    );
    assert_eq!(
        run.evidence_digest(),
        digest(EVIDENCE_DOMAIN, run.evidence().as_bytes())
    );
    assert_eq!(
        raw_sha(run.trace()),
        "sha256:b418408ff16de76251e0b40eb2c7b68dd408bbae66b96e734138ad64f6f70eab"
    );
    assert_eq!(
        raw_sha(run.evidence()),
        "sha256:45da26349aa89514ca3066a0f14076d4220cb03560589b9f959f97e9564bd6ad"
    );
    assert!(!run.trace().contains("alpha"));
    assert!(!run.evidence().contains("alpha"));
    assert!(!run.trace().contains("public-host-secret-sentinel"));
    assert!(!run.evidence().contains("public-host-secret-sentinel"));
    assert_eq!(inventory(&root), before);
    fs::remove_dir_all(root).unwrap();
}

#[path = "agent_runtime_v1/agent_definition_v1.rs"]
mod agent_definition_v1;
#[path = "agent_runtime_v1/agent_deployment_v1.rs"]
mod agent_deployment_v1;
#[path = "agent_runtime_v1/agent_inspect_cli.rs"]
mod agent_inspect_cli;
#[path = "agent_runtime_v1/agent_payment_harness_v1.rs"]
mod agent_payment_harness_v1;
#[path = "agent_runtime_v1/agent_proposal_schema_v1.rs"]
mod agent_proposal_schema_v1;

#[test]
fn public_cancellation_retry_and_sink_limits_are_fail_closed() {
    let host = Host::new();
    let providers = Arc::clone(&host.provider_calls);
    let tools = Arc::clone(&host.tool_calls);
    let cancellation = AgentCancellation::new();
    let worker = cancellation.clone();
    std::thread::spawn(move || worker.cancel()).join().unwrap();
    assert!(cancellation.is_cancelled());
    cancellation.cancel();
    let mut agent = Agent::new(&profile(), host, cancellation).unwrap();
    let error = match agent.run(&task()) {
        Ok(_) => panic!("pre-effect cancellation produced an artifact"),
        Err(error) => error,
    };
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-I220");
    assert_eq!(error[0].message, "Agent Runtime run was cancelled");
    assert_eq!(providers.load(Ordering::Acquire), 0);
    assert_eq!(tools.load(Ordering::Acquire), 0);

    let malformed_calls = Arc::new(AtomicUsize::new(0));
    let mut malformed_host = Host::new();
    malformed_host.provider_calls = Arc::clone(&malformed_calls);
    let error = match Agent::new(
        profile().trim_end(),
        malformed_host,
        AgentCancellation::new(),
    ) {
        Ok(_) => panic!("noncanonical profile admitted"),
        Err(error) => error,
    };
    assert_eq!(error[0].code, "SPX-G204");
    assert_eq!(malformed_calls.load(Ordering::Acquire), 0);

    let mut retry = Host::new();
    retry.retry_first = true;
    retry.retry_to_final = true;
    let providers = Arc::clone(&retry.provider_calls);
    let tools = Arc::clone(&retry.tool_calls);
    let mut agent = Agent::new(&profile(), retry, AgentCancellation::new()).unwrap();
    let run = agent.run(&task()).unwrap();
    assert_eq!(run.status(), AgentRunStatus::Completed);
    assert_eq!(providers.load(Ordering::Acquire), 2);
    assert_eq!(tools.load(Ordering::Acquire), 0);

    let mut overflow = Host::new();
    overflow.provider_overflow = true;
    let providers = Arc::clone(&overflow.provider_calls);
    let tools = Arc::clone(&overflow.tool_calls);
    let mut agent = Agent::new(&profile(), overflow, AgentCancellation::new()).unwrap();
    let run = agent.run(&task()).unwrap();
    assert_eq!(run.status(), AgentRunStatus::BudgetExhausted);
    assert_eq!(run.final_message(), None);
    assert!(run.evidence().contains("SPX-G208"));
    assert_eq!(providers.load(Ordering::Acquire), 1);
    assert_eq!(tools.load(Ordering::Acquire), 0);

    let mut uncertain = Host::new();
    uncertain.uncertain_first = true;
    let providers = Arc::clone(&uncertain.provider_calls);
    let tools = Arc::clone(&uncertain.tool_calls);
    let mut agent = Agent::new(&profile(), uncertain, AgentCancellation::new()).unwrap();
    let run = agent.run(&task()).unwrap();
    assert_eq!(run.status(), AgentRunStatus::ProviderFailed);
    assert_eq!(run.final_message(), None);
    assert_eq!(providers.load(Ordering::Acquire), 1);
    assert_eq!(tools.load(Ordering::Acquire), 0);

    let mut unavailable_tokenizer = Host::new();
    unavailable_tokenizer.tokenize_none = true;
    let providers = Arc::clone(&unavailable_tokenizer.provider_calls);
    let run = Agent::new(&profile(), unavailable_tokenizer, AgentCancellation::new())
        .unwrap()
        .run(&task())
        .unwrap();
    assert_eq!(run.status(), AgentRunStatus::ProviderFailed);
    assert!(run.evidence().contains("SPX-I218"));
    assert_eq!(providers.load(Ordering::Acquire), 0);

    let mut tool_failure = Host::new();
    tool_failure.tool_succeeds = false;
    let providers = Arc::clone(&tool_failure.provider_calls);
    let tools = Arc::clone(&tool_failure.tool_calls);
    let mut agent = Agent::new(&profile(), tool_failure, AgentCancellation::new()).unwrap();
    let run = agent.run(&task()).unwrap();
    assert_eq!(run.status(), AgentRunStatus::ToolFailed);
    assert_eq!(run.final_message(), None);
    assert_eq!(providers.load(Ordering::Acquire), 1);
    assert_eq!(tools.load(Ordering::Acquire), 1);

    let oversized_task = "x".repeat(4_194_305);
    let host = Host::new();
    let providers = Arc::clone(&host.provider_calls);
    let error = match Agent::new(&profile(), host, AgentCancellation::new())
        .unwrap()
        .run(&oversized_task)
    {
        Ok(_) => panic!("oversized task admitted"),
        Err(error) => error,
    };
    assert_eq!(error[0].code, "SPX-G208");
    assert_eq!(error[0].message, "task_bytes exceeds 4194304");
    assert_eq!(providers.load(Ordering::Acquire), 0);

    let cancellation = AgentCancellation::new();
    let mut cancel_provider = Host::new();
    cancel_provider.cancellation = Some(cancellation.clone());
    cancel_provider.cancel_provider_after_first = true;
    let providers = Arc::clone(&cancel_provider.provider_calls);
    let tools = Arc::clone(&cancel_provider.tool_calls);
    let mut agent = Agent::new(&profile(), cancel_provider, cancellation).unwrap();
    assert_eq!(
        agent.run(&task()).unwrap().status(),
        AgentRunStatus::Cancelled
    );
    assert_eq!(providers.load(Ordering::Acquire), 1);
    assert_eq!(tools.load(Ordering::Acquire), 0);

    let cancellation = AgentCancellation::new();
    let mut cancel_tool = Host::new();
    cancel_tool.cancellation = Some(cancellation.clone());
    cancel_tool.cancel_tool_before_push = true;
    let providers = Arc::clone(&cancel_tool.provider_calls);
    let tools = Arc::clone(&cancel_tool.tool_calls);
    let mut agent = Agent::new(&profile(), cancel_tool, cancellation).unwrap();
    assert_eq!(
        agent.run(&task()).unwrap().status(),
        AgentRunStatus::Cancelled
    );
    assert_eq!(providers.load(Ordering::Acquire), 1);
    assert_eq!(tools.load(Ordering::Acquire), 1);
}

#[test]
fn public_cumulative_builder_boundary_is_exact_and_stops_before_a_later_call() {
    let with_limit = |limit: u64| {
        profile().replace(
            "\"max_builder_bytes\":1048576",
            &format!("\"max_builder_bytes\":{limit}"),
        )
    };
    let completes = |limit| {
        let mut host = Host::new();
        host.retry_to_final = true;
        match Agent::new(&with_limit(limit), host, AgentCancellation::new()) {
            Ok(mut agent) => matches!(
                agent.run(&task()),
                Ok(run) if run.status() == AgentRunStatus::Completed
            ),
            Err(_) => false,
        }
    };
    assert!(completes(1_048_576));
    let mut low = 1_u64;
    let mut high = 1_048_576_u64;
    while low < high {
        let middle = low + (high - low) / 2;
        if completes(middle) {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    let minimum = low;
    assert!(minimum > 1);

    let mut exact_host = Host::new();
    exact_host.retry_to_final = true;
    let exact_providers = Arc::clone(&exact_host.provider_calls);
    let mut exact = Agent::new(&with_limit(minimum), exact_host, AgentCancellation::new()).unwrap();
    assert_eq!(
        exact.run(&task()).unwrap().status(),
        AgentRunStatus::Completed
    );
    assert_eq!(exact_providers.load(Ordering::Acquire), 1);

    let mut below_host = Host::new();
    below_host.retry_to_final = true;
    let below_providers = Arc::clone(&below_host.provider_calls);
    let below_tools = Arc::clone(&below_host.tool_calls);
    let result = Agent::new(
        &with_limit(minimum - 1),
        below_host,
        AgentCancellation::new(),
    )
    .and_then(|mut agent| agent.run(&task()));
    match result {
        Ok(run) => {
            assert_eq!(run.status(), AgentRunStatus::BudgetExhausted);
            assert!(run.evidence().contains("builder_bytes"));
        }
        Err(error) => {
            assert_eq!(error[0].code, "SPX-G208");
            assert!(error[0].message.starts_with("builder_bytes exceeds "));
        }
    }
    assert!(below_providers.load(Ordering::Acquire) <= 1);
    assert_eq!(below_tools.load(Ordering::Acquire), 0);
}

#[test]
fn external_consumer_surface_is_exact_and_opaque() {
    let root = std::env::temp_dir().join(format!(
        "semaprax-agent-surface-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(root.join("src")).unwrap();
    let manifest_root = env!("CARGO_MANIFEST_DIR").replace('\\', "\\\\");
    fs::write(
        root.join("Cargo.toml"),
        format!("[package]\nname=\"agent-surface-lock\"\nversion=\"0.0.0\"\nedition=\"2021\"\n[workspace]\n[dependencies]\nsemaprax={{path=\"{manifest_root}\",default-features=false}}\n"),
    )
    .unwrap();
    fs::write(
        root.join("src/main.rs"),
        r#"use semaprax::agent_runtime::{Agent,AgentHost,AgentRun,AgentProviderSink,AgentToolResultSink,parse_profile,replay_evidence,AgentRuntimeAuthority};
fn clone<T: Clone>() {}
fn debug<T: std::fmt::Debug>() {}
fn reject_agent<H: AgentHost>() { clone::<Agent<H>>(); debug::<Agent<H>>(); }
fn main() {
    clone::<AgentRun>(); debug::<AgentRun>();
    let _ = AgentRun { trace:String::new(), trace_digest:String::new(), evidence:String::new(), evidence_digest:String::new() };
    let _ = AgentProviderSink::new(); let _ = AgentToolResultSink::new();
    let _ = parse_profile; let _ = replay_evidence; let _ = std::mem::size_of::<AgentRuntimeAuthority>();
    let _ = std::mem::size_of::<Agent<NeverHost>>();
}
struct NeverHost;
"#,
    )
    .unwrap();
    let checked = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .args(["check", "--offline", "--manifest-path"])
        .arg(root.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", root.join("target"))
        .output()
        .unwrap();
    assert!(!checked.status.success());
    let stderr = String::from_utf8_lossy(&checked.stderr);
    for name in [
        "parse_profile",
        "replay_evidence",
        "AgentRuntimeAuthority",
        "Clone",
        "Debug",
        "private",
    ] {
        assert!(stderr.contains(name), "missing `{name}` in:\n{stderr}");
    }

    let cli = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .output()
        .unwrap();
    let cli_text = format!(
        "{}{}",
        String::from_utf8_lossy(&cli.stdout),
        String::from_utf8_lossy(&cli.stderr)
    );
    assert!(!cli_text.contains("agent-runtime"));
    assert!(!cli_text.contains("agent-profile"));

    let public_source = include_str!("../src/agent_runtime.rs");
    let private_source = include_str!("../src/agent_runtime/private.rs");
    for forbidden in [
        "std::net::",
        "TcpStream",
        "UdpSocket",
        "reqwest::",
        "Command::new",
        "fs::write",
        "File::create",
    ] {
        assert!(
            !public_source.contains(forbidden),
            "public source contains {forbidden}"
        );
        assert!(
            !private_source.contains(forbidden),
            "private source contains {forbidden}"
        );
    }
    fs::remove_dir_all(root).unwrap();
}
