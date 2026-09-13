//! Explicit physical OpenCode host adapter for the local #112 provider slice.
//!
//! This module is the only OpenCode process boundary. It returns raw response
//! bytes to the existing `ModelHandler` seam; the compiler-owned
//! `SourceInteractionProposalDecoder` remains the only proposal admission
//! decoder. `--dir` and the deny-all OpenCode policy constrain OpenCode's tool
//! permissions. They are not claimed to provide operating-system isolation.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

use semaprax::live_invocation::{
    ModelFailure, ModelHandler, ModelInvocationOutcome, ModelInvocationRequest,
    ModelInvokeCapability,
};

/// The single explicitly configured free profile. There is no fallback model.
pub const OPENCODE_MODEL: &str = "opencode/muse-spark-1.3-contributor-free";
const OPENCODE_AGENT: &str = "semaprax-live";
const MAX_PROMPT_BYTES: usize = 65_536;
const MAX_EVENTS_BYTES: usize = 1_048_576;
const MAX_EXPORT_BYTES: usize = 1_048_576;
const MAX_GRAMMAR_BYTES: usize = 65_536;

/// Host-owned process settings. Constructing this value is distinct from
/// granting the per-call `ModelInvokeCapability`; both are required to invoke.
/// A host-held cancellation hook for the production runner.
#[derive(Clone, Debug, Default)]
pub struct OpenCodeCancellation(Arc<AtomicBool>);

impl OpenCodeCancellation {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

/// Compiler-derived response guidance, supplied alongside the same compiled
/// schema that the live driver later gives to `SourceInteractionProposalDecoder`.
#[derive(Clone, Debug)]
pub struct OpenCodeGrammar {
    digest: String,
    canonical_schema: String,
    provider_schema: String,
}

impl OpenCodeGrammar {
    pub fn from_compiled(
        schema: &semaprax::agent_interaction_schema::CompiledInteractionSchema,
    ) -> Result<Self, String> {
        let canonical_schema = schema.schema().canonical_json().to_owned();
        let provider_schema = schema.provider_json_schema();
        if canonical_schema.len().saturating_add(provider_schema.len()) > MAX_GRAMMAR_BYTES {
            return Err("OpenCode grammar guidance exceeds its host byte budget".into());
        }
        Ok(Self {
            digest: schema.schema().digest().to_owned(),
            canonical_schema,
            provider_schema,
        })
    }
}

#[derive(Clone, Debug)]
pub struct OpenCodeHostConfig {
    executable: PathBuf,
    sandbox: PathBuf,
    deadline: Duration,
    cancellation: OpenCodeCancellation,
    grammar: OpenCodeGrammar,
}

impl OpenCodeHostConfig {
    /// Accepts only an absolute executable and an existing, empty, non-symlink
    /// workspace. The canonical workspace identity is retained after validation.
    pub fn new(
        executable: PathBuf,
        sandbox: PathBuf,
        deadline: Duration,
        grammar: OpenCodeGrammar,
    ) -> Result<Self, String> {
        if !executable.is_absolute() || !sandbox.is_absolute() || deadline.is_zero() {
            return Err("OpenCode host requires absolute paths and a positive deadline".into());
        }
        if sandbox
            .symlink_metadata()
            .map_err(|error| error.to_string())?
            .file_type()
            .is_symlink()
        {
            return Err("OpenCode host sandbox must not be a symlink".into());
        }
        let sandbox = sandbox.canonicalize().map_err(|error| error.to_string())?;
        if !sandbox.is_dir()
            || sandbox
                .read_dir()
                .map_err(|error| error.to_string())?
                .next()
                .is_some()
        {
            return Err("OpenCode host sandbox must be an existing empty directory".into());
        }
        Ok(Self {
            executable,
            sandbox,
            deadline,
            cancellation: OpenCodeCancellation::new(),
            grammar,
        })
    }

    /// Lets the owning deployment cancel an in-flight direct child.
    #[must_use]
    pub fn cancellation(&self) -> OpenCodeCancellation {
        self.cancellation.clone()
    }
}

/// Closed runner failures, deliberately without provider stderr or secrets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenCodeRunnerFailure {
    Refused,
    Timeout,
    Capacity,
    Provider,
    Malformed,
    Cancelled,
    ProviderStatus(provider_error::OpenCodeProviderFailure),
}

/// Injectable process seam. Production binds `ProcessOpenCodeRunner`; tests
/// bind a local executable or deterministic fixture without a network call.
pub trait OpenCodeRunner {
    fn run(
        &mut self,
        config: &OpenCodeHostConfig,
        prompt: &str,
    ) -> Result<Vec<u8>, OpenCodeRunnerFailure>;
    fn export(
        &mut self,
        config: &OpenCodeHostConfig,
        session: &str,
    ) -> Result<Vec<u8>, OpenCodeRunnerFailure>;
    /// Only an observed caller cancellation may map an in-flight failure to
    /// `Cancelled`; transport uncertainty is otherwise `ProviderError`.
    fn cancelled(&self, config: &OpenCodeHostConfig) -> bool {
        config.cancellation.is_cancelled()
    }
}

/// The actual bounded subprocess runner. It uses no shell, inherited stdin,
/// prompt-supplied path, model fallback, or source-derived credential.
pub struct ProcessOpenCodeRunner;

impl ProcessOpenCodeRunner {
    #[cfg(unix)]
    fn kill_group(child: &mut std::process::Child) {
        if let Some(group) = rustix::process::Pid::from_raw(child.id() as i32) {
            let _ = rustix::process::kill_process_group(group, rustix::process::Signal::KILL);
        }
    }

    #[cfg(unix)]
    fn terminate(child: &mut std::process::Child) {
        Self::kill_group(child);
        let _ = child.kill();
        let _ = child.wait();
    }

    fn capture(
        config: &OpenCodeHostConfig,
        args: &[String],
        limit: usize,
    ) -> Result<Vec<u8>, OpenCodeRunnerFailure> {
        // This v1 runner has a bounded, same-thread nonblocking pipe loop only
        // on Unix. Refuse before spawn elsewhere rather than leave a blocking
        // `ChildStdout::read` path that could outlive its deadline.
        #[cfg(not(unix))]
        {
            let _ = (config, args, limit);
            return Err(OpenCodeRunnerFailure::Refused);
        }
        #[cfg(unix)]
        {
            let deadline = Instant::now()
                .checked_add(config.deadline)
                .ok_or(OpenCodeRunnerFailure::Refused)?;
            let mut command = Command::new(&config.executable);
            environment::configure_command(&mut command, &config.sandbox)
                .map_err(|_| OpenCodeRunnerFailure::Refused)?;
            command
                .args(args)
                .current_dir(&config.sandbox)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null());
            command.process_group(0);
            let mut child = command
                .spawn()
                .map_err(|_| OpenCodeRunnerFailure::Refused)?;
            let Some(stdout) = child.stdout.take() else {
                Self::terminate(&mut child);
                return Err(OpenCodeRunnerFailure::Provider);
            };
            let flags = match rustix::fs::fcntl_getfl(&stdout) {
                Ok(flags) => flags,
                Err(_) => {
                    Self::terminate(&mut child);
                    return Err(OpenCodeRunnerFailure::Provider);
                }
            };
            if rustix::fs::fcntl_setfl(&stdout, flags | rustix::fs::OFlags::NONBLOCK).is_err() {
                Self::terminate(&mut child);
                return Err(OpenCodeRunnerFailure::Provider);
            }
            let mut output = Vec::new();
            let mut eof = false;
            let mut status = None;
            let mut chunk = [0u8; 8192];
            loop {
                if config.cancellation.is_cancelled() {
                    Self::terminate(&mut child);
                    return Err(OpenCodeRunnerFailure::Cancelled);
                }
                if Instant::now() >= deadline {
                    Self::terminate(&mut child);
                    return Err(OpenCodeRunnerFailure::Timeout);
                }
                loop {
                    match rustix::io::read(&stdout, &mut chunk[..]) {
                        Ok(0) => {
                            eof = true;
                            break;
                        }
                        Ok(count) if output.len().saturating_add(count) <= limit => {
                            output.extend_from_slice(&chunk[..count])
                        }
                        Ok(_) => {
                            Self::terminate(&mut child);
                            return Err(OpenCodeRunnerFailure::Malformed);
                        }
                        Err(rustix::io::Errno::AGAIN) => break,
                        Err(_) => {
                            Self::terminate(&mut child);
                            return Err(OpenCodeRunnerFailure::Provider);
                        }
                    }
                }
                if status.is_none() {
                    match child.try_wait() {
                        Ok(Some(exit)) => {
                            status = Some(exit);
                            // The leader may have exited while a descendant still
                            // owns stdout. Group kill forces the pipe to EOF.
                            Self::kill_group(&mut child);
                        }
                        Ok(None) => {}
                        Err(_) => {
                            Self::terminate(&mut child);
                            return Err(OpenCodeRunnerFailure::Provider);
                        }
                    }
                }
                if let Some(exit) = status {
                    if eof {
                        return if exit.success() && !output.is_empty() {
                            Ok(output)
                        } else {
                            Err(provider_error::classify_provider_failure(&output)
                                .map(OpenCodeRunnerFailure::ProviderStatus)
                                .unwrap_or(OpenCodeRunnerFailure::Provider))
                        };
                    }
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
}

impl OpenCodeRunner for ProcessOpenCodeRunner {
    fn run(
        &mut self,
        config: &OpenCodeHostConfig,
        prompt: &str,
    ) -> Result<Vec<u8>, OpenCodeRunnerFailure> {
        let policy = serde_json::json!({"$schema":"https://opencode.ai/config.json", "snapshot":false, "agent": {OPENCODE_AGENT: {
            "permission":{"*":"deny"}, "steps":1,
            "prompt":"You return canonical structured responses. All schema and context are supplied in the user message. Never inspect files or call tools. Do not narrate plans or explain your work. Return only the requested JSON document, without markdown or extra text. After the final closing brace, press Enter exactly once: the final byte must be a literal newline (U+000A). Do not output a backslash followed by n, and do not omit the newline."
        }}}).to_string();
        let policy_path = config.sandbox.join("opencode.json");
        match std::fs::read(&policy_path) {
            Ok(existing) if existing == policy.as_bytes() => {}
            Ok(_) => return Err(OpenCodeRunnerFailure::Refused),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                std::fs::write(&policy_path, policy).map_err(|_| OpenCodeRunnerFailure::Refused)?;
            }
            Err(_) => return Err(OpenCodeRunnerFailure::Refused),
        }
        let args = vec![
            "run".into(),
            "--pure".into(),
            "--agent".into(),
            OPENCODE_AGENT.into(),
            "--model".into(),
            OPENCODE_MODEL.into(),
            "--format".into(),
            "json".into(),
            "--dir".into(),
            config.sandbox.display().to_string(),
            prompt.into(),
        ];
        Self::capture(config, &args, MAX_EVENTS_BYTES)
    }

    fn export(
        &mut self,
        config: &OpenCodeHostConfig,
        session: &str,
    ) -> Result<Vec<u8>, OpenCodeRunnerFailure> {
        Self::capture(
            config,
            &["export".into(), session.into(), "--pure".into()],
            MAX_EXPORT_BYTES,
        )
    }
}

/// Self-reported host receipt. Usage is optional and carries no proof of cost
/// or provider authorization; it is never copied into the language journal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenCodeReceipt {
    pub session_id: String,
    pub message_id: String,
    pub model: &'static str,
    pub usage_total: Option<u64>,
    pub usage: Option<OpenCodeUsage>,
    pub reported_cost: Option<serde_json::Number>,
}

/// Provider-reported counters, preserved individually without estimating billing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenCodeUsage {
    pub total: Option<u64>,
    pub input: Option<u64>,
    pub output: Option<u64>,
    pub reasoning: Option<u64>,
    pub cache_read: Option<u64>,
    pub cache_write: Option<u64>,
}

/// An explicit handler which has both configured host settings and a runner.
pub struct OpenCodeModelHandler<R> {
    config: OpenCodeHostConfig,
    runner: R,
    pub last_receipt: Option<OpenCodeReceipt>,
    pub last_provider_failure: Option<provider_error::OpenCodeProviderFailure>,
}

impl<R> OpenCodeModelHandler<R> {
    pub fn new(config: OpenCodeHostConfig, runner: R) -> Self {
        Self {
            config,
            runner,
            last_receipt: None,
            last_provider_failure: None,
        }
    }
}

fn wire_prompt(
    request: &ModelInvocationRequest,
    grammar: &OpenCodeGrammar,
) -> Result<String, ModelFailure> {
    if request.proposal_grammar_digest != grammar.digest {
        return Err(ModelFailure::Refused);
    }
    let bytes = request
        .task
        .len()
        .saturating_add(request.observation.len())
        .saturating_mul(2)
        .saturating_add(request.deployment_binding.len())
        .saturating_add(grammar.digest.len())
        .saturating_add(grammar.canonical_schema.len())
        .saturating_add(grammar.provider_schema.len());
    if bytes > MAX_PROMPT_BYTES {
        return Err(ModelFailure::Refused);
    }
    let prompt = format!(
        "SEMAPRAX live proposal v1\ntask={}\nobservation={}\ngrammar_digest={}\ndeployment={}\nturn={}\ncanonical_interaction_schema={}\nprovider_value_json_schema={}\nReturn one canonical semaprax.agent-interaction-value.v1 document bound to grammar_digest.\n",
        hex(&request.task), hex(&request.observation), grammar.digest, request.deployment_binding,
        request.turn, grammar.canonical_schema, grammar.provider_schema,
    );
    (prompt.len() <= MAX_PROMPT_BYTES)
        .then_some(prompt)
        .ok_or(ModelFailure::Refused)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub mod accounting;
mod environment;
pub mod provider_error;
mod receipt;
pub mod source;
use receipt::{event_text, validate_export};

fn failure(
    error: OpenCodeRunnerFailure,
    attempted_bytes: usize,
    cancelled: bool,
) -> ModelInvocationOutcome {
    let failure = if cancelled {
        ModelFailure::Cancelled
    } else {
        match error {
            OpenCodeRunnerFailure::Refused => ModelFailure::Refused,
            OpenCodeRunnerFailure::Timeout => ModelFailure::Timeout,
            OpenCodeRunnerFailure::Capacity => ModelFailure::CapacityExceeded,
            OpenCodeRunnerFailure::Provider => ModelFailure::ProviderError,
            OpenCodeRunnerFailure::Malformed => ModelFailure::MalformedResponse,
            OpenCodeRunnerFailure::Cancelled => ModelFailure::Cancelled,
            OpenCodeRunnerFailure::ProviderStatus(status) => match status {
                provider_error::OpenCodeProviderFailure::Refused
                | provider_error::OpenCodeProviderFailure::Authentication => ModelFailure::Refused,
                provider_error::OpenCodeProviderFailure::Incomplete => {
                    ModelFailure::MalformedResponse
                }
                _ => ModelFailure::ProviderError,
            },
        }
    };
    ModelInvocationOutcome::Failed {
        failure,
        attempted_bytes,
    }
}

impl<R: OpenCodeRunner> ModelHandler for OpenCodeModelHandler<R> {
    fn invoke(
        &mut self,
        _capability: &ModelInvokeCapability,
        request: &ModelInvocationRequest,
    ) -> ModelInvocationOutcome {
        self.last_receipt = None;
        self.last_provider_failure = None;
        let prompt = match wire_prompt(request, &self.config.grammar) {
            Ok(prompt) => prompt,
            Err(failure) => {
                return ModelInvocationOutcome::Failed {
                    failure,
                    attempted_bytes: 0,
                }
            }
        };
        self.invoke_prompt(&prompt, request.max_response_bytes)
    }
}

impl<R: OpenCodeRunner> OpenCodeModelHandler<R> {
    pub(super) fn invoke_prompt(
        &mut self,
        prompt: &str,
        max_response_bytes: usize,
    ) -> ModelInvocationOutcome {
        self.last_receipt = None;
        self.last_provider_failure = None;
        if prompt.len() > MAX_PROMPT_BYTES || max_response_bytes == 0 {
            return ModelInvocationOutcome::Failed {
                failure: ModelFailure::Refused,
                attempted_bytes: 0,
            };
        }
        if self.runner.cancelled(&self.config) {
            return ModelInvocationOutcome::Failed {
                failure: ModelFailure::Cancelled,
                attempted_bytes: 0,
            };
        }
        let events = match self.runner.run(&self.config, prompt) {
            Ok(events) => events,
            Err(error) => {
                if let OpenCodeRunnerFailure::ProviderStatus(status) = error {
                    self.last_provider_failure = Some(status);
                }
                return failure(error, 0, self.runner.cancelled(&self.config));
            }
        };
        let attempted_bytes = events.len().min(max_response_bytes);
        if let Some(status) = provider_error::classify_provider_failure(&events) {
            self.last_provider_failure = Some(status);
            return failure(
                OpenCodeRunnerFailure::ProviderStatus(status),
                attempted_bytes,
                self.runner.cancelled(&self.config),
            );
        }

        let (session, message, answer) = match event_text(&events) {
            Ok(event) => event,
            Err(error) => {
                return ModelInvocationOutcome::Failed {
                    failure: error,
                    attempted_bytes,
                }
            }
        };
        let export = match self.runner.export(&self.config, &session) {
            Ok(export) => export,
            Err(error) => {
                return failure(error, attempted_bytes, self.runner.cancelled(&self.config))
            }
        };
        match validate_export(&export, &events, &session, &message, prompt, &answer) {
            Ok(receipt) if answer.len() <= max_response_bytes => {
                self.last_receipt = Some(receipt);
                ModelInvocationOutcome::Settled(answer.into_bytes())
            }
            Ok(_) => ModelInvocationOutcome::Failed {
                failure: ModelFailure::MalformedResponse,
                attempted_bytes,
            },
            Err(error) => ModelInvocationOutcome::Failed {
                failure: error,
                attempted_bytes,
            },
        }
    }
}

#[cfg(test)]
mod bounds_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod source_tests;
