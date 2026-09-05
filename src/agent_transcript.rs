//! Scripted Agent Runtime v1 host for `semaprax agent run` and `agent replay`.
//!
//! A transcript is a caller-supplied document of provider responses and tool
//! results consumed in order. The host it drives has no transport, process,
//! filesystem, network, clock, or credential authority: every observation the
//! runtime makes is a pure function of the transcript, so a run is
//! deterministic and `replay` can recompute its evidence byte for byte. The
//! runtime's non-claims are unchanged: nothing here resumes, persists, or
//! reconciles a run.

use serde_json::Value;

use crate::agent_definition::compile_agent_definition;
use crate::agent_runtime::{
    Agent, AgentBoundaryProbe, AgentCancellation, AgentHost, AgentProviderAttempt,
    AgentProviderDisposition, AgentProviderSink, AgentProviderUsage, AgentRun, AgentRunStatus,
    AgentToolResultSink,
};
use crate::diagnostic::{quote_json, Diagnostic};

/// Schema of the transcript document.
pub const SCHEMA_V1: &str = "semaprax.agent-runtime-transcript.v1";
/// Largest transcript the host reads.
pub const MAX_TRANSCRIPT_BYTES: usize = 4 * 1024 * 1024;
const MAX_ENTRIES: usize = 256;

/// One scripted provider attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderEntry {
    pub disposition: AgentProviderDisposition,
    /// The response bytes streamed on success; empty for failed attempts.
    pub response: String,
}

/// A parsed transcript: the provider attempts and tool results in order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Transcript {
    pub policy_epoch: u64,
    pub provider: Vec<ProviderEntry>,
    /// `None` scripts a failed tool invocation.
    pub tools: Vec<Option<String>>,
}

fn malformed(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-V221", message)
}

/// Parse one transcript document. Objects are closed: an unknown key, a wrong
/// type, or a foreign schema rejects the whole document.
pub fn parse_transcript(source: &str) -> Result<Transcript, Diagnostic> {
    if source.len() > MAX_TRANSCRIPT_BYTES {
        return Err(malformed(format!(
            "transcript exceeds {MAX_TRANSCRIPT_BYTES} bytes"
        )));
    }
    let value: Value =
        serde_json::from_str(source).map_err(|_| malformed("transcript is not a JSON document"))?;
    let object = value
        .as_object()
        .ok_or_else(|| malformed("transcript must be a JSON object"))?;
    for key in object.keys() {
        if !matches!(
            key.as_str(),
            "schema" | "policy_epoch" | "provider" | "tools"
        ) {
            return Err(malformed(format!("transcript has an unknown key `{key}`")));
        }
    }
    if object.get("schema").and_then(Value::as_str) != Some(SCHEMA_V1) {
        return Err(malformed(format!(
            "transcript schema must be `{SCHEMA_V1}`"
        )));
    }
    let policy_epoch = match object.get("policy_epoch") {
        None => 0,
        Some(value) => value
            .as_u64()
            .ok_or_else(|| malformed("transcript `policy_epoch` must be an unsigned integer"))?,
    };
    let provider_values = object
        .get("provider")
        .and_then(Value::as_array)
        .ok_or_else(|| malformed("transcript `provider` must be an array"))?;
    if provider_values.len() > MAX_ENTRIES {
        return Err(malformed(format!(
            "transcript `provider` holds more than {MAX_ENTRIES} entries"
        )));
    }
    let mut provider = Vec::with_capacity(provider_values.len());
    for entry in provider_values {
        let entry = entry
            .as_object()
            .ok_or_else(|| malformed("transcript provider entries must be objects"))?;
        for key in entry.keys() {
            if !matches!(key.as_str(), "disposition" | "response") {
                return Err(malformed(format!(
                    "transcript provider entry has an unknown key `{key}`"
                )));
            }
        }
        let disposition = match entry.get("disposition").and_then(Value::as_str) {
            Some("succeeded") => AgentProviderDisposition::Succeeded,
            Some("definitely_not_started") => AgentProviderDisposition::DefinitelyNotStarted,
            Some("failed_uncertain") => AgentProviderDisposition::FailedUncertain,
            _ => {
                return Err(malformed(
                    "transcript provider `disposition` must be `succeeded`, `definitely_not_started`, or `failed_uncertain`",
                ))
            }
        };
        let response = match (disposition, entry.get("response")) {
            (AgentProviderDisposition::Succeeded, Some(Value::String(response))) => {
                response.clone()
            }
            (AgentProviderDisposition::Succeeded, _) => {
                return Err(malformed(
                    "a succeeded transcript provider entry must carry a string `response`",
                ))
            }
            (_, None) => String::new(),
            (_, Some(_)) => {
                return Err(malformed(
                    "a failed transcript provider entry carries no `response`",
                ))
            }
        };
        provider.push(ProviderEntry {
            disposition,
            response,
        });
    }
    let tool_values = match object.get("tools") {
        None => &[][..],
        Some(value) => value
            .as_array()
            .ok_or_else(|| malformed("transcript `tools` must be an array"))?
            .as_slice(),
    };
    if tool_values.len() > MAX_ENTRIES {
        return Err(malformed(format!(
            "transcript `tools` holds more than {MAX_ENTRIES} entries"
        )));
    }
    let mut tools = Vec::with_capacity(tool_values.len());
    for entry in tool_values {
        let entry = entry
            .as_object()
            .ok_or_else(|| malformed("transcript tool entries must be objects"))?;
        for key in entry.keys() {
            if key != "result" {
                return Err(malformed(format!(
                    "transcript tool entry has an unknown key `{key}`"
                )));
            }
        }
        tools.push(match entry.get("result") {
            None | Some(Value::Null) => None,
            Some(Value::String(result)) => Some(result.clone()),
            Some(_) => {
                return Err(malformed(
                    "transcript tool `result` must be a string or null",
                ))
            }
        });
    }
    Ok(Transcript {
        policy_epoch,
        provider,
        tools,
    })
}

#[derive(Clone)]
struct Probe {
    policy_epoch: u64,
}

impl AgentBoundaryProbe for Probe {
    fn policy_epoch(&self) -> u64 {
        self.policy_epoch
    }

    fn elapsed_ms(&self) -> u64 {
        0
    }
}

/// The scripted host: consumes the transcript in order and observes nothing
/// else. An exhausted provider script answers `failed_uncertain`; an
/// exhausted tool script fails the invocation.
pub struct TranscriptHost {
    transcript: Transcript,
    next_provider: usize,
    next_tool: usize,
}

impl TranscriptHost {
    #[must_use]
    pub fn new(transcript: Transcript) -> Self {
        Self {
            transcript,
            next_provider: 0,
            next_tool: 0,
        }
    }
}

impl AgentHost for TranscriptHost {
    fn policy_epoch(&self) -> u64 {
        self.transcript.policy_epoch
    }

    fn elapsed_ms(&self) -> u64 {
        0
    }

    fn boundary_probe(&self) -> Box<dyn AgentBoundaryProbe> {
        Box::new(Probe {
            policy_epoch: self.transcript.policy_epoch,
        })
    }

    fn tokenize(&mut self, _: &str, request: &str) -> Option<u64> {
        Some(request.len() as u64)
    }

    fn attempt_provider(
        &mut self,
        _: &str,
        _: &str,
        request: &str,
        _: u64,
        sink: &mut AgentProviderSink,
    ) -> AgentProviderAttempt {
        let Some(entry) = self.transcript.provider.get(self.next_provider) else {
            return AgentProviderAttempt::new(
                AgentProviderDisposition::FailedUncertain,
                AgentProviderUsage::default(),
            );
        };
        self.next_provider += 1;
        if entry.disposition != AgentProviderDisposition::Succeeded {
            return AgentProviderAttempt::new(entry.disposition, AgentProviderUsage::default());
        }
        let accepted = sink.push(entry.response.as_bytes());
        AgentProviderAttempt::new(
            AgentProviderDisposition::Succeeded,
            AgentProviderUsage::new(
                request.len() as u64,
                if accepted {
                    entry.response.len() as u64
                } else {
                    0
                },
                0,
            ),
        )
    }

    fn invoke_tool(&mut self, _: &str, _: &str, _: &str, sink: &mut AgentToolResultSink) -> bool {
        let Some(entry) = self.transcript.tools.get(self.next_tool) else {
            return false;
        };
        self.next_tool += 1;
        match entry {
            Some(result) => sink.push(result.as_bytes()),
            None => false,
        }
    }
}

/// The stable text of a run status.
#[must_use]
pub fn status_text(status: AgentRunStatus) -> &'static str {
    match status {
        AgentRunStatus::Completed => "completed",
        AgentRunStatus::Cancelled => "cancelled",
        AgentRunStatus::DeadlineExceeded => "deadline_exceeded",
        AgentRunStatus::BudgetExhausted => "budget_exhausted",
        AgentRunStatus::ProviderFailed => "provider_failed",
        AgentRunStatus::ToolFailed => "tool_failed",
        AgentRunStatus::PolicyRejected => "policy_rejected",
    }
}

/// One completed scripted run together with the agent identity it ran.
pub struct ScriptedRun {
    pub agent_id: String,
    pub run: AgentRun,
}

/// Compile the definition, derive its Runtime v1 profile, and run the task
/// against the transcript. Pure: the only inputs are the three documents.
pub fn run(
    definition_source: &str,
    task_source: &str,
    transcript_source: &str,
) -> Result<ScriptedRun, Vec<Diagnostic>> {
    let compiled = compile_agent_definition(definition_source)?;
    let transcript = parse_transcript(transcript_source).map_err(|error| vec![error])?;
    let mut agent = Agent::new(
        compiled.runtime_v1_profile(),
        TranscriptHost::new(transcript),
        AgentCancellation::new(),
    )?;
    let run = agent.run(task_source)?;
    Ok(ScriptedRun {
        agent_id: compiled.definition().agent_id().to_owned(),
        run,
    })
}

/// The receipt `agent run` prints: identity, status, and the run's digests.
#[must_use]
pub fn run_receipt(scripted: &ScriptedRun) -> String {
    format!(
        "{{\"schema\":\"semaprax.agent-run-receipt.v1\",\"agent_id\":{},\"status\":{},\"final_message\":{},\"trace_digest\":{},\"evidence_digest\":{},\"authority\":false}}\n",
        quote_json(&scripted.agent_id),
        quote_json(status_text(scripted.run.status())),
        scripted
            .run
            .final_message()
            .map_or_else(|| "null".to_owned(), quote_json),
        quote_json(scripted.run.trace_digest()),
        quote_json(scripted.run.evidence_digest())
    )
}

/// Re-run the transcript and require the recomputed evidence to equal the
/// supplied evidence byte for byte. Nothing is trusted from the capsule.
pub fn replay(
    definition_source: &str,
    task_source: &str,
    transcript_source: &str,
    evidence_source: &str,
) -> Result<String, Vec<Diagnostic>> {
    let scripted = run(definition_source, task_source, transcript_source)?;
    if scripted.run.evidence().as_bytes() != evidence_source.as_bytes() {
        return Err(vec![Diagnostic::io(
            "SPX-V222",
            format!(
                "the supplied evidence does not equal the replayed run's evidence {}",
                scripted.run.evidence_digest()
            ),
        )]);
    }
    Ok(format!(
        "{{\"schema\":\"semaprax.agent-replay-receipt.v1\",\"agent_id\":{},\"status\":{},\"evidence_digest\":{},\"verified\":true,\"authority\":false}}\n",
        quote_json(&scripted.agent_id),
        quote_json(status_text(scripted.run.status())),
        quote_json(scripted.run.evidence_digest())
    ))
}
