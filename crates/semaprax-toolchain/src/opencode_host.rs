//! Explicit physical OpenCode host adapter for the local #112 provider slice.
//!
//! This module is the only OpenCode process boundary. It returns raw response
//! bytes to the existing `ModelHandler` seam; the compiler-owned
//! `SourceInteractionProposalDecoder` remains the only proposal admission
//! decoder. `--dir` and the deny-all OpenCode policy constrain OpenCode's tool
//! permissions. They are not claimed to provide operating-system isolation.

use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

use semaprax::live_invocation::{
    ModelFailure, ModelHandler, ModelInvocationOutcome, ModelInvocationRequest,
    ModelInvokeCapability,
};
use serde_json::Value;

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
    fn terminate(child: &mut std::process::Child, reader: std::thread::JoinHandle<()>) {
        #[cfg(unix)]
        if let Some(group) = rustix::process::Pid::from_raw(child.id() as i32) {
            let _ = rustix::process::kill_process_group(group, rustix::process::Signal::Kill);
        }
        let _ = child.kill();
        let _ = child.wait();
        let _ = reader.join();
    }

    fn capture(
        config: &OpenCodeHostConfig,
        args: &[String],
        limit: usize,
    ) -> Result<Vec<u8>, OpenCodeRunnerFailure> {
        let mut command = Command::new(&config.executable);
        command
            .args(args)
            .current_dir(&config.sandbox)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(unix)]
        command.process_group(0);
        let mut child = command
            .spawn()
            .map_err(|_| OpenCodeRunnerFailure::Refused)?;
        let mut stdout = child.stdout.take().ok_or(OpenCodeRunnerFailure::Provider)?;
        let (sender, receiver) = mpsc::sync_channel(1);
        let reader = std::thread::spawn(move || {
            let mut output = Vec::new();
            let mut chunk = [0u8; 8192];
            loop {
                match stdout.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(count) if output.len().saturating_add(count) <= limit => {
                        output.extend_from_slice(&chunk[..count])
                    }
                    Ok(_) => {
                        let _ = sender.send(Err(OpenCodeRunnerFailure::Malformed));
                        return;
                    }
                    Err(_) => {
                        let _ = sender.send(Err(OpenCodeRunnerFailure::Provider));
                        return;
                    }
                }
            }
            let _ = sender.send(Ok(output));
        });
        let deadline = Instant::now()
            .checked_add(config.deadline)
            .ok_or(OpenCodeRunnerFailure::Refused)?;
        let mut output = None;
        loop {
            if config.cancellation.is_cancelled() {
                Self::terminate(&mut child, reader);
                return Err(OpenCodeRunnerFailure::Cancelled);
            }
            if Instant::now() >= deadline {
                Self::terminate(&mut child, reader);
                return Err(OpenCodeRunnerFailure::Timeout);
            }
            if output.is_none() {
                match receiver.try_recv() {
                    Ok(Ok(bytes)) => output = Some(bytes),
                    Ok(Err(error)) => {
                        Self::terminate(&mut child, reader);
                        return Err(error);
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        Self::terminate(&mut child, reader);
                        return Err(OpenCodeRunnerFailure::Provider);
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                }
            }
            match child.try_wait() {
                Ok(Some(status)) => {
                    // A direct child may exit while a descendant still owns stdout.
                    // Kill the dedicated process group before joining the reader, so a
                    // pipe inheritor cannot turn this bounded call into an unbounded join.
                    Self::terminate(&mut child, reader);
                    let bytes = match output {
                        Some(bytes) => bytes,
                        None => match receiver.try_recv() {
                            Ok(Ok(bytes)) => bytes,
                            Ok(Err(error)) => return Err(error),
                            Err(_) => return Err(OpenCodeRunnerFailure::Provider),
                        },
                    };
                    return if status.success() {
                        Ok(bytes)
                    } else {
                        Err(OpenCodeRunnerFailure::Provider)
                    };
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(5)),
                Err(_) => {
                    Self::terminate(&mut child, reader);
                    return Err(OpenCodeRunnerFailure::Provider);
                }
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
        let policy = format!(
            "{{\"$schema\":\"https://opencode.ai/config.json\",\"agent\":{{\"{OPENCODE_AGENT}\":{{\"permission\":{{\"*\":\"deny\"}}}}}}}}\n"
        );
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
        Self::capture(config, &["export".into(), session.into()], MAX_EXPORT_BYTES)
    }
}

/// Self-reported host receipt. Usage is optional and carries no proof of cost
/// or provider authorization; it is never copied into the language journal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenCodeReceipt {
    pub session_id: String,
    pub usage_total: Option<u64>,
}

/// An explicit handler which has both configured host settings and a runner.
pub struct OpenCodeModelHandler<R> {
    config: OpenCodeHostConfig,
    runner: R,
    pub last_receipt: Option<OpenCodeReceipt>,
}

impl<R> OpenCodeModelHandler<R> {
    pub fn new(config: OpenCodeHostConfig, runner: R) -> Self {
        Self {
            config,
            runner,
            last_receipt: None,
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

fn event_text(events: &[u8]) -> Result<(String, String, String), ModelFailure> {
    let text = std::str::from_utf8(events).map_err(|_| ModelFailure::MalformedResponse)?;
    let rows: Result<Vec<Value>, _> = text
        .lines()
        .filter(|line| !line.is_empty())
        .map(serde_json::from_str)
        .collect();
    let rows = rows.map_err(|_| ModelFailure::MalformedResponse)?;
    if rows.len() != 3
        || rows
            .iter()
            .map(|row| row["type"].as_str())
            .collect::<Vec<_>>()
            != [Some("step_start"), Some("text"), Some("step_finish")]
    {
        return Err(ModelFailure::MalformedResponse);
    }
    let session = rows[0]["sessionID"]
        .as_str()
        .ok_or(ModelFailure::MalformedResponse)?;
    let message = rows[0]["part"]["messageID"]
        .as_str()
        .ok_or(ModelFailure::MalformedResponse)?;
    let answer = rows[1]["part"]["text"]
        .as_str()
        .filter(|text| !text.is_empty())
        .ok_or(ModelFailure::MalformedResponse)?;
    if rows.iter().any(|row| {
        row["sessionID"].as_str() != Some(session)
            || row["part"]["sessionID"].as_str() != Some(session)
            || row["part"]["messageID"].as_str() != Some(message)
    }) || rows[2]["part"]["reason"].as_str() != Some("stop")
    {
        return Err(ModelFailure::MalformedResponse);
    }
    Ok((session.into(), message.into(), answer.into()))
}

fn validate_export(
    export: &[u8],
    session: &str,
    message: &str,
    prompt: &str,
    answer: &str,
) -> Result<OpenCodeReceipt, ModelFailure> {
    let export: Value =
        serde_json::from_slice(export).map_err(|_| ModelFailure::MalformedResponse)?;
    let info = export["info"]
        .as_object()
        .ok_or(ModelFailure::MalformedResponse)?;
    let model = info
        .get("model")
        .and_then(Value::as_object)
        .ok_or(ModelFailure::MalformedResponse)?;
    if info.get("id").and_then(Value::as_str) != Some(session)
        || model.get("providerID").and_then(Value::as_str) != Some("opencode")
        || model.get("id").and_then(Value::as_str) != Some("muse-spark-1.3-contributor-free")
    {
        return Err(ModelFailure::MalformedResponse);
    }
    let messages = export["messages"]
        .as_array()
        .ok_or(ModelFailure::MalformedResponse)?;
    let assistant = messages
        .iter()
        .find(|row| row["info"]["id"].as_str() == Some(message))
        .ok_or(ModelFailure::MalformedResponse)?;
    let parent = assistant["info"]["parentID"]
        .as_str()
        .ok_or(ModelFailure::MalformedResponse)?;
    let user = messages
        .iter()
        .find(|row| row["info"]["id"].as_str() == Some(parent))
        .ok_or(ModelFailure::MalformedResponse)?;
    if assistant["info"]["role"].as_str() != Some("assistant")
        || assistant["info"]["sessionID"].as_str() != Some(session)
        || user["info"]["role"].as_str() != Some("user")
        || user["info"]["sessionID"].as_str() != Some(session)
        || user["parts"]
            .as_array()
            .and_then(|parts| parts.first())
            .and_then(|part| part["text"].as_str())
            != Some(prompt)
        || assistant["parts"]
            .as_array()
            .and_then(|parts| parts.first())
            .and_then(|part| part["text"].as_str())
            != Some(answer)
    {
        return Err(ModelFailure::MalformedResponse);
    }
    Ok(OpenCodeReceipt {
        session_id: session.into(),
        usage_total: assistant["info"]["tokens"]["total"].as_u64(),
    })
}

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
        let prompt = match wire_prompt(request, &self.config.grammar) {
            Ok(prompt) => prompt,
            Err(failure) => {
                return ModelInvocationOutcome::Failed {
                    failure,
                    attempted_bytes: 0,
                }
            }
        };
        if self.runner.cancelled(&self.config) {
            return ModelInvocationOutcome::Failed {
                failure: ModelFailure::Cancelled,
                attempted_bytes: 0,
            };
        }
        let events = match self.runner.run(&self.config, &prompt) {
            Ok(events) => events,
            Err(error) => return failure(error, 0, self.runner.cancelled(&self.config)),
        };
        let attempted_bytes = events.len().min(request.max_response_bytes);
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
        match validate_export(&export, &session, &message, &prompt, &answer) {
            Ok(receipt) if answer.len() <= request.max_response_bytes => {
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
mod tests;
