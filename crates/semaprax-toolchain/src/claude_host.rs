//! Explicit native Claude print-JSON host. No OpenCode event/export synthesis.
use crate::opencode_host::{OpenCodeHostConfig, OpenCodeRunnerFailure};
use semaprax::agent_runtime_v2::SourceModelAdapterIdentity;
use semaprax::digest_hex::LowerHex;
use semaprax::live_invocation::ModelFailure;
use semaprax::provider_adapter_sdk::*;
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const MODEL: &str = "claude-haiku-4-5";
const PROFILE: &str = "claude-code-subscription-print-json.v1";
const MAX_EXECUTABLE: u64 = 256 * 1024 * 1024;
const MAX_WIRE: usize = 1_048_576;
const MAX_REQUEST: usize = 65_536;
const MAX_RESPONSE: usize = 65_536;

pub(crate) fn identity(
    executable: &Path,
    scratch: &Path,
) -> Result<SourceModelAdapterIdentity, String> {
    let binding = crate::opencode_host::executable_snapshot_with_limit(executable, MAX_EXECUTABLE)
        .ok_or("Claude executable is unavailable or exceeds bounds")?
        .0;
    Ok(identity_with_binding(executable, scratch, &binding))
}
fn identity_with_binding(
    executable: &Path,
    scratch: &Path,
    binding: &str,
) -> SourceModelAdapterIdentity {
    let mut hash = Sha256::new();
    hash.update(b"semaprax.claude-print-host.v1\0");
    for bytes in [
        executable.as_os_str().as_encoded_bytes(),
        scratch.as_os_str().as_encoded_bytes(),
        binding.as_bytes(),
        MODEL.as_bytes(),
    ] {
        hash.update(bytes);
        hash.update(b"\0");
    }
    SourceModelAdapterIdentity {
        provider_id: "anthropic".into(),
        model_id: MODEL.into(),
        adapter_identity: format!(
            "claude-print-adapter:sha256:{:x}",
            LowerHex(hash.finalize())
        ),
        adapter_version: "1.0.0".into(),
        provider_profile: PROFILE.into(),
    }
}

#[derive(Clone)]
pub(crate) struct Config {
    host: OpenCodeHostConfig,
    deadline: Duration,
}
impl Config {
    pub(crate) fn new(
        executable: PathBuf,
        scratch: PathBuf,
        deadline: Duration,
    ) -> Result<Self, String> {
        if std::fs::read_dir(&scratch)
            .map_err(|_| "Claude scratch is unavailable")?
            .next()
            .is_some()
            && !crate::opencode_host::scratch_inventory_is_repair_post_settled_marker(&scratch)
        {
            return Err("Claude scratch must be empty or contain its retained pause marker".into());
        }
        // Shares bounded image admission only. No OpenCode policy or transport is run.
        let host =
            OpenCodeHostConfig::new_process_image(executable, scratch, deadline, MAX_EXECUTABLE)?;
        Ok(Self { host, deadline })
    }
    pub(crate) fn marker_host(&self) -> &OpenCodeHostConfig {
        &self.host
    }
    pub(crate) fn identity(&self) -> SourceModelAdapterIdentity {
        identity_with_binding(
            &self.host.executable,
            &self.host.sandbox,
            &self.host.executable_binding,
        )
    }
}

pub(crate) struct ClaudeAdapter {
    config: Config,
    capabilities: AdapterCapabilities,
    identity: AdapterModelIdentity,
    polls: VecDeque<AdapterPoll>,
    terminal: Option<AdapterPoll>,
    started: bool,
}
impl ClaudeAdapter {
    pub(crate) fn new(config: Config, adapter_identity: String) -> Self {
        Self {
            config,
            capabilities: AdapterCapabilities {
                adapter_identity,
                adapter_version: "1.0.0".into(),
                provider_profile: PROFILE.into(),
                structured_output_modes: vec![StructuredOutputMode::RawText],
                supports_streaming: true,
                token_accounting_source: TokenAccountingSource::ProviderReported,
                cancellation_semantics: CancellationSemantics::BestEffortRequestStop,
                retryable_failure_classes: vec![],
                endpoint_policy: EndpointPolicy::HostInjected,
                max_request_bytes: MAX_REQUEST,
                max_response_bytes: MAX_RESPONSE,
                max_context_tokens: 16_384,
                max_output_tokens: 16_384,
            },
            identity: AdapterModelIdentity {
                provider_id: "anthropic".into(),
                model_id: MODEL.into(),
                capabilities: vec!["raw_text".into(), "streaming".into()],
            },
            polls: VecDeque::new(),
            terminal: None,
            started: false,
        }
    }
    fn settle(&mut self, poll: AdapterPoll) {
        self.terminal = Some(poll.clone());
        self.polls.push_back(poll);
    }
}
impl ProviderAdapter for ClaudeAdapter {
    fn capabilities(&self) -> &AdapterCapabilities {
        &self.capabilities
    }
    fn model_identity(&self) -> Option<&AdapterModelIdentity> {
        Some(&self.identity)
    }
    fn start(
        &mut self,
        _: &AdapterInvocationCapability,
        request: &AdapterRequest,
    ) -> Result<(), AdapterRefusal> {
        if self.started
            || request.request_bytes.is_empty()
            || request.request_bytes.len() > MAX_REQUEST
            || request.max_response_bytes == 0
            || request.max_response_bytes > MAX_RESPONSE
        {
            return Err(AdapterRefusal(
                "Claude repair request exceeds bounds or was already started".into(),
            ));
        }
        let prompt = std::str::from_utf8(&request.request_bytes)
            .map_err(|_| AdapterRefusal("Claude repair request is not UTF-8".into()))?;
        self.started = true;
        let wire = invoke(&self.config, prompt);
        let attempted_bytes = wire
            .as_ref()
            .map_or(0, |bytes| bytes.len().min(request.max_response_bytes));
        match wire.and_then(|wire| parse(&wire, request.max_response_bytes)) {
            Ok(settlement) => {
                self.polls.push_back(AdapterPoll::Event(AdapterEvent::Delta(
                    settlement.response_bytes.clone(),
                )));
                self.polls
                    .push_back(AdapterPoll::Event(AdapterEvent::Completed));
                self.settle(AdapterPoll::Settled(settlement));
            }
            Err(failure) => self.settle(AdapterPoll::Failed {
                failure,
                attempted_bytes,
            }),
        }
        Ok(())
    }
    fn poll(&mut self) -> AdapterPoll {
        self.polls
            .pop_front()
            .or_else(|| self.terminal.clone())
            .unwrap_or(AdapterPoll::Pending)
    }
    fn cancel(&mut self, _: &str) {
        self.config.host.cancellation().cancel();
        if self.terminal.is_none() {
            self.settle(AdapterPoll::Failed {
                failure: ModelFailure::Cancelled,
                attempted_bytes: 0,
            });
        }
    }
}

fn parse(wire: &[u8], maximum: usize) -> Result<AdapterSettlement, ModelFailure> {
    use serde_json::Value;
    let fail = ModelFailure::MalformedResponse;
    if wire.len() > MAX_WIRE {
        return Err(fail);
    }
    let value: Value = serde_json::from_slice(wire).map_err(|_| fail)?;
    if value["type"] != "result"
        || value["subtype"] != "success"
        || value["is_error"] != false
        || value["num_turns"] != 1
        || value["stop_reason"] != "end_turn"
        || value["terminal_reason"] != "completed"
        || value["queued_turn_count"] != 0
        || value["result_index"] != 0
        || value["permission_denials"]
            .as_array()
            .is_none_or(|items| !items.is_empty())
        || value["subagent_stats"]["spawned"] != 0
    {
        return Err(fail);
    }
    let models = value["modelUsage"].as_object().ok_or(fail)?;
    if models.len() != 1
        || !models.contains_key(MODEL)
        || models[MODEL]["canonicalModel"] != MODEL
        || models[MODEL]["provider"] != "firstParty"
        || models[MODEL]["webSearchRequests"] != 0
    {
        return Err(fail);
    }
    let result = value["result"].as_str().ok_or(fail)?;
    if result.is_empty() || result.len() > maximum {
        return Err(fail);
    }
    let tokens_in = value["usage"]["input_tokens"].as_u64().ok_or(fail)?;
    let tokens_out = value["usage"]["output_tokens"].as_u64().ok_or(fail)?;
    Ok(AdapterSettlement {
        response_bytes: result.as_bytes().to_vec(),
        usage: AdapterUsage {
            tokens_in: Some(tokens_in),
            tokens_out: Some(tokens_out),
            cost_micros: None,
        },
    })
}

#[cfg(not(unix))]
fn invoke(_: &Config, _: &str) -> Result<Vec<u8>, ModelFailure> {
    Err(ModelFailure::Refused)
}
#[cfg(unix)]
fn invoke(config: &Config, prompt: &str) -> Result<Vec<u8>, ModelFailure> {
    use std::process::{Command, Stdio};
    let failure = |error| match error {
        OpenCodeRunnerFailure::Timeout => ModelFailure::Timeout,
        OpenCodeRunnerFailure::Cancelled => ModelFailure::Cancelled,
        OpenCodeRunnerFailure::Malformed => ModelFailure::MalformedResponse,
        OpenCodeRunnerFailure::Refused => ModelFailure::Refused,
        _ => ModelFailure::ProviderError,
    };
    if config.host.cancellation().is_cancelled() {
        return Err(ModelFailure::Cancelled);
    }
    if identity(&config.host.executable, &config.host.sandbox)
        .map_err(|_| ModelFailure::Refused)?
        .adapter_identity
        != config.identity().adapter_identity
    {
        return Err(ModelFailure::Refused);
    }
    let deadline = Instant::now()
        .checked_add(config.deadline)
        .ok_or(ModelFailure::Refused)?;
    let stage = crate::opencode_host::StagedExecutable::create(&config.host).map_err(failure)?;
    let mut command = Command::new(&stage.path);
    configure(&mut command, &config.host.sandbox)?;
    command
        .args([
            "--print",
            "--output-format",
            "json",
            "--tools",
            "",
            "--no-session-persistence",
            "--safe-mode",
            "--restricted",
            "--strict-mcp-config",
            "--permission-prompts",
            "none",
            "--prompt-suggestions",
            "false",
            "--model",
            MODEL,
            "--system-prompt",
            "The user message is a checked source-adapter request. Decode task_hex as UTF-8 for the task and previous_effect_hex for feedback. Return only the requested canonical proposal JSON, with one actual trailing LF byte and no Markdown fences. Follow proposal_schema exactly. No tools are available.",
            "--",
            prompt,
        ])
        .current_dir(&config.host.sandbox)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let (exit, bytes) = crate::bounded_capture::capture(command, deadline, MAX_WIRE, || {
        config.host.cancellation().is_cancelled()
    })
    .map_err(failure)?;
    if !exit.success() {
        return Err(ModelFailure::ProviderError);
    }
    Ok(bytes)
}

#[cfg(unix)]
fn configure(command: &mut std::process::Command, scratch: &Path) -> Result<(), ModelFailure> {
    // Safe/restricted modes still admit admin-managed policy. Refuse its known
    // file routes rather than allowing host customizations into the prompt.
    for path in [
        "/Library/Application Support/ClaudeCode/managed-settings.json",
        "/Library/Application Support/ClaudeCode/managed-settings.d",
        "/Library/Application Support/ClaudeCode/managed-mcp.json",
        "/etc/claude-code/managed-settings.json",
        "/etc/claude-code/managed-settings.d",
        "/etc/claude-code/managed-mcp.json",
        "/Library/Managed Preferences/com.anthropic.claudecode.plist",
    ] {
        match std::fs::symlink_metadata(path) {
            Ok(_) => return Err(ModelFailure::Refused),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(ModelFailure::Refused),
        }
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .ok_or(ModelFailure::Refused)?;
    let login = std::env::var("USER")
        .ok()
        .filter(|value| valid_login(value))
        .ok_or(ModelFailure::Refused)?;
    command
        .env_clear()
        .env("HOME", home)
        .env("USER", &login)
        .env("LOGNAME", &login)
        .env("PATH", "/usr/bin:/bin")
        .env("TMPDIR", scratch)
        .env("DISABLE_AUTOUPDATER", "1")
        .env("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1")
        .env("CLAUDE_CODE_SAFE_MODE", "1");
    Ok(())
}

/// Login metadata only: this never copies a credential or endpoint setting.
fn valid_login(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

#[cfg(test)]
#[path = "claude_host_tests.rs"]
mod tests;
