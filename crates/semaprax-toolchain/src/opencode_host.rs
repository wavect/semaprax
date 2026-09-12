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
use std::sync::mpsc;
use std::time::{Duration, Instant};

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

/// Host-owned process settings. Constructing this value is distinct from
/// granting the per-call `ModelInvokeCapability`; both are required to invoke.
#[derive(Clone, Debug)]
pub struct OpenCodeHostConfig {
    executable: PathBuf,
    sandbox: PathBuf,
    deadline: Duration,
}

impl OpenCodeHostConfig {
    /// Accepts only an absolute executable and an existing, empty, absolute
    /// workspace. The runner writes the fixed deny-all agent policy itself.
    pub fn new(executable: PathBuf, sandbox: PathBuf, deadline: Duration) -> Result<Self, String> {
        if !executable.is_absolute() || !sandbox.is_absolute() || deadline.is_zero() {
            return Err("OpenCode host requires absolute paths and a positive deadline".into());
        }
        let mut entries = sandbox.read_dir().map_err(|error| error.to_string())?;
        if entries.next().is_some() {
            return Err("OpenCode host sandbox must be an existing empty directory".into());
        }
        Ok(Self {
            executable,
            sandbox,
            deadline,
        })
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
    fn cancelled(&self) -> bool {
        false
    }
}

/// The actual bounded subprocess runner. It uses no shell, inherited stdin,
/// prompt-supplied path, model fallback, or source-derived credential.
pub struct ProcessOpenCodeRunner;

impl ProcessOpenCodeRunner {
    fn capture(
        config: &OpenCodeHostConfig,
        args: &[String],
        limit: usize,
    ) -> Result<Vec<u8>, OpenCodeRunnerFailure> {
        let mut child = Command::new(&config.executable)
            .args(args)
            .current_dir(&config.sandbox)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| OpenCodeRunnerFailure::Refused)?;
        let mut stdout = child.stdout.take().ok_or(OpenCodeRunnerFailure::Provider)?;
        let (sender, receiver) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let mut output = Vec::new();
            let mut chunk = [0u8; 8192];
            loop {
                match stdout.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(count) => {
                        if output.len().saturating_add(count) > limit {
                            let _ = sender.send(Err(OpenCodeRunnerFailure::Malformed));
                            return;
                        }
                        output.extend_from_slice(&chunk[..count]);
                    }
                    Err(_) => {
                        let _ = sender.send(Err(OpenCodeRunnerFailure::Provider));
                        return;
                    }
                }
            }
            let _ = sender.send(Ok(output));
        });
        let deadline = Instant::now() + config.deadline;
        loop {
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait(); // reap the direct child before returning
                return Err(OpenCodeRunnerFailure::Timeout);
            }
            if let Some(status) = child
                .try_wait()
                .map_err(|_| OpenCodeRunnerFailure::Provider)?
            {
                let output = receiver
                    .recv_timeout(Duration::from_millis(50))
                    .map_err(|_| OpenCodeRunnerFailure::Provider)??;
                return if status.success() {
                    Ok(output)
                } else {
                    Err(OpenCodeRunnerFailure::Provider)
                };
            }
            std::thread::sleep(Duration::from_millis(5));
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
        std::fs::write(config.sandbox.join("opencode.json"), policy)
            .map_err(|_| OpenCodeRunnerFailure::Refused)?;
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

fn wire_prompt(request: &ModelInvocationRequest) -> Result<String, ModelFailure> {
    let prompt = format!(
        "SEMAPRAX live proposal v1\ntask={}\nobservation={}\ngrammar={}\ndeployment={}\nturn={}\nReturn only one proposal document.\n",
        hex(&request.task), hex(&request.observation), request.proposal_grammar_digest,
        request.deployment_binding, request.turn,
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
        || user["info"]["role"].as_str() != Some("user")
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
        let prompt = match wire_prompt(request) {
            Ok(prompt) => prompt,
            Err(failure) => {
                return ModelInvocationOutcome::Failed {
                    failure,
                    attempted_bytes: 0,
                }
            }
        };
        if self.runner.cancelled() {
            return ModelInvocationOutcome::Failed {
                failure: ModelFailure::Cancelled,
                attempted_bytes: 0,
            };
        }
        let events = match self.runner.run(&self.config, &prompt) {
            Ok(events) => events,
            Err(error) => return failure(error, 0, self.runner.cancelled()),
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
            Err(error) => return failure(error, attempted_bytes, self.runner.cancelled()),
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
