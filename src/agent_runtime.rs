//! Bounded native Agent Runtime v1 injected-host API.
//!
//! This safe-Rust injected-adapter state machine has no built-in transport, process,
//! environment, filesystem, mutation, publication, credential, or economic
//! authority. Model and tool bytes remain untrusted data.

#![allow(
    dead_code,
    reason = "private typed and replay internals support the opaque public C1 surface"
)]

use std::collections::BTreeSet;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::bounded_output::{reserve_active, with_limit_usage};
use crate::diagnostic::{quote_json, Diagnostic};

const PROFILE_SCHEMA: &str = "semaprax.agent-runtime-profile.v1";
const TASK_SCHEMA: &str = "semaprax.agent-runtime-task.v1";
const ACTION_SCHEMA: &str = "semaprax.agent-runtime-action.v1";
const TRACE_SCHEMA: &str = "semaprax.agent-runtime-trace.v1";
const EVIDENCE_SCHEMA: &str = "semaprax.agent-runtime-evidence.v1";
const PROVIDER_REQUEST_SCHEMA: &str = "semaprax.agent-runtime-provider-request.v1";
const TOOL_RESULT_SCHEMA: &str = "semaprax.agent-runtime-tool-result.v1";

const PROFILE_DOMAIN: &[u8] = b"semaprax.agent-runtime.profile-digest.v1\0";
const TASK_DOMAIN: &[u8] = b"semaprax.agent-runtime.task-digest.v1\0";
const ACTION_DOMAIN: &[u8] = b"semaprax.agent-runtime.action-digest.v1\0";
const TRACE_DOMAIN: &[u8] = b"semaprax.agent-runtime.trace-digest.v1\0";
const EVIDENCE_DOMAIN: &[u8] = b"semaprax.agent-runtime.evidence-digest.v1\0";
const REQUEST_DOMAIN: &[u8] = b"semaprax.agent-runtime.provider-request-digest.v1\0";
const PROVIDER_RESPONSE_DOMAIN: &[u8] = b"semaprax.agent-runtime.provider-response-digest.v1\0";
const TOOL_RESULT_DOMAIN: &[u8] = b"semaprax.agent-runtime.tool-result-digest.v1\0";
const RUN_ID_DOMAIN: &[u8] = b"semaprax.agent-runtime.run-id.v1\0";
const CALL_ID_DOMAIN: &[u8] = b"semaprax.agent-runtime.call-id.v1\0";
const FINAL_MESSAGE_DOMAIN: &[u8] = b"semaprax.agent-runtime.final-message-digest.v1\0";

const MAX_PROFILE_BYTES: usize = 1_048_576;
const MAX_TASK_BYTES: usize = 4_194_304;
const MAX_MODELS: usize = 32;
const MAX_TOOLS: usize = 32;
const MAX_CAPABILITIES: usize = 64;
const MAX_TURNS: u64 = 16;
const MAX_PROVIDER_ATTEMPTS: u64 = 32;
const MAX_RETRIES_PER_TURN: u64 = 1;
const MAX_CONCURRENCY: u64 = 1;
const MAX_ELAPSED_MS: u64 = 300_000;
const MAX_PROVIDER_REQUEST_BYTES: u64 = 4_194_304;
const MAX_PROVIDER_RESPONSE_BYTES: u64 = 1_048_576;
const MAX_STREAM_CHUNKS: u64 = 8_192;
const MAX_TOTAL_PROVIDER_INPUT_BYTES: u64 = 33_554_432;
const MAX_TOTAL_PROVIDER_OUTPUT_BYTES: u64 = 8_388_608;
const MAX_REPORTED_MODEL_INPUT_TOKENS: u64 = 2_097_152;
const MAX_REPORTED_MODEL_OUTPUT_TOKENS: u64 = 262_144;
const MAX_USD_MICROUNITS: u64 = 10_000_000;
const MAX_TOOL_CALLS: u64 = 32;
const MAX_TOOL_ARGUMENT_BYTES: u64 = 262_144;
const MAX_TOOL_RESULT_BYTES: u64 = 1_048_576;
const MAX_TOTAL_TOOL_BYTES: u64 = 16_777_216;
const MAX_RETAINED_STATE_BYTES: u64 = 16_777_216;
const MAX_TRACE_EVENTS: u64 = 4_096;
const MAX_TRACE_BYTES: u64 = 16_777_216;
const MAX_EVIDENCE_BYTES: u64 = 20_971_520;
const MAX_BUILDER_BYTES: usize = 67_108_864;
const MAX_JSON_DEPTH: usize = 16;
const MAX_IDENTIFIER_BYTES: usize = 240;
const MAX_DESCRIPTION_BYTES: usize = 4_096;

const NONCLAIMS: [&str; 24] = [
    "no_compiler_determinism_from_model_output",
    "no_model_output_authority",
    "no_provider_identity_provenance_or_quality_truth",
    "no_secret_input_or_secret_leakage_guarantee_for_caller_supplied_content",
    "no_credential_prompt_state_trace_or_diagnostic_exposure",
    "no_ambient_network_filesystem_process_home_or_environment_authority",
    "no_write_apply_mutation_or_target_execution_tool_authority",
    "no_capability_minting_delegation_or_self_approval",
    "no_human_approval_ui_or_policy",
    "no_semantic_prompt_injection_proof",
    "no_forced_cancellation_or_preemption",
    "no_exactly_once_provider_billing_or_retry",
    "no_durable_memory_persistence_recovery_or_resume",
    "no_crash_reboot_or_power_loss_durability",
    "no_distributed_or_parallel_execution",
    "no_model_quality_accuracy_or_completion_guarantee",
    "no_live_price_or_cost_accuracy_guarantee",
    "no_reusable_authorization_token",
    "no_signature_attestation_or_authenticated_provenance",
    "no_wallet_payment_signing_asset_or_economic_authority",
    "no_privacy_compliance_or_data_residency_guarantee",
    "no_general_formal_proof",
    "no_new_language_graph_cleanup_backend_or_runtime_semantics",
    "no_current_schema_api_or_kat_modification",
];

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
enum Locality {
    Local,
    Remote,
}

impl Locality {
    fn text(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Remote => "remote",
        }
    }
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
enum QualityTier {
    Basic,
    Standard,
    Advanced,
    Frontier,
}

impl QualityTier {
    fn text(self) -> &'static str {
        match self {
            Self::Basic => "basic",
            Self::Standard => "standard",
            Self::Advanced => "advanced",
            Self::Frontier => "frontier",
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum RequiredLocality {
    LocalOnly,
    RemoteAllowed,
}

impl RequiredLocality {
    fn text(self) -> &'static str {
        match self {
            Self::LocalOnly => "local_only",
            Self::RemoteAllowed => "remote_allowed",
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ScalarKind {
    String,
    Integer,
    Boolean,
}

impl ScalarKind {
    fn text(self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Integer => "integer",
            Self::Boolean => "boolean",
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
struct SchemaField {
    name: String,
    kind: ScalarKind,
    required: bool,
    max_bytes: u64,
}

#[derive(Clone, Eq, PartialEq)]
struct ClosedSchema {
    fields: Vec<SchemaField>,
}

#[derive(Clone, Eq, PartialEq)]
struct Model {
    provider_id: String,
    model_id: String,
    locality: Locality,
    quality_tier: QualityTier,
    tokenizer_id: String,
    max_context_tokens: u64,
    input_price: u64,
    output_price: u64,
    capabilities: Vec<String>,
}

#[derive(Clone, Eq, PartialEq)]
struct Tool {
    tool_id: String,
    description: String,
    arguments_schema: ClosedSchema,
    result_schema: ClosedSchema,
    required_capabilities: Vec<String>,
}

#[derive(Clone, Eq, PartialEq)]
struct Policy {
    allowed_provider_ids: Vec<String>,
    allowed_model_ids: Vec<String>,
    required_locality: RequiredLocality,
    minimum_quality_tier: QualityTier,
    required_model_capabilities: Vec<String>,
    granted_capabilities: Vec<String>,
    allowed_tool_ids: Vec<String>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct EffectiveLimits {
    max_turns: u64,
    max_provider_attempts: u64,
    max_retries_per_turn: u64,
    max_concurrency: u64,
    max_elapsed_ms: u64,
    max_provider_request_bytes: u64,
    max_provider_response_bytes: u64,
    max_stream_chunks: u64,
    max_total_provider_input_bytes: u64,
    max_total_provider_output_bytes: u64,
    max_reported_model_input_tokens: u64,
    max_reported_model_output_tokens: u64,
    max_usd_microunits: u64,
    max_tool_calls: u64,
    max_tool_arguments_bytes: u64,
    max_tool_result_bytes: u64,
    max_total_tool_bytes: u64,
    max_retained_state_bytes: u64,
    max_trace_events: u64,
    max_trace_bytes: u64,
    max_evidence_bytes: u64,
    max_builder_bytes: u64,
}

#[derive(Clone, Eq, PartialEq)]
struct Profile {
    agent_id: String,
    models: Vec<Model>,
    tools: Vec<Tool>,
    policy: Policy,
    limits: EffectiveLimits,
    source: String,
    digest: String,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Provenance {
    CallerTrusted,
    CallerUntrusted,
    RetrievedUntrusted,
}

impl Provenance {
    fn text(self) -> &'static str {
        match self {
            Self::CallerTrusted => "caller_trusted",
            Self::CallerUntrusted => "caller_untrusted",
            Self::RetrievedUntrusted => "retrieved_untrusted",
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
struct ContextItem {
    label: String,
    provenance: Provenance,
    content: String,
}

#[derive(Clone, Eq, PartialEq)]
struct Task {
    nonce: String,
    objective: String,
    context: Vec<ContextItem>,
    source: String,
    digest: String,
}

#[derive(Clone, Eq, PartialEq)]
enum Action {
    Final {
        message: String,
        source: String,
    },
    Tool {
        tool_id: String,
        arguments: Value,
        source: String,
    },
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
struct UsageDelta {
    provider_input_bytes: u64,
    provider_output_bytes: u64,
    reported_model_input_tokens: u64,
    reported_model_output_tokens: u64,
    usd_microunits: u64,
    tool_argument_bytes: u64,
    tool_result_bytes: u64,
    elapsed_ms: u64,
}

#[derive(Clone, Default, Eq, PartialEq)]
struct Usage {
    turns: u64,
    provider_attempts: u64,
    provider_input_bytes: u64,
    provider_output_bytes: u64,
    reported_model_input_tokens: u64,
    reported_model_output_tokens: u64,
    usd_microunits: u64,
    tool_calls: u64,
    tool_argument_bytes: u64,
    tool_result_bytes: u64,
    retained_state_bytes: u64,
    elapsed_ms: u64,
    max_concurrency: u64,
}

#[derive(Clone, Eq, PartialEq)]
struct TraceEvent {
    index: u64,
    turn: u64,
    kind: &'static str,
    provider_id: Option<String>,
    model_id: Option<String>,
    tool_id: Option<String>,
    input_digest: Option<String>,
    output_digest: Option<String>,
    status: &'static str,
    usage: UsageDelta,
}

/// The terminal status of one bounded Agent run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentRunStatus {
    Completed,
    Cancelled,
    DeadlineExceeded,
    BudgetExhausted,
    ProviderFailed,
    ToolFailed,
    PolicyRejected,
}

impl AgentRunStatus {
    fn text(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::DeadlineExceeded => "deadline_exceeded",
            Self::BudgetExhausted => "budget_exhausted",
            Self::ProviderFailed => "provider_failed",
            Self::ToolFailed => "tool_failed",
            Self::PolicyRejected => "policy_rejected",
        }
    }
}

#[derive(Clone)]
struct Termination {
    status: RunStatus,
    code: Option<&'static str>,
    message: Option<String>,
}

/// Opaque canonical Trace and Evidence produced by one run.
pub struct AgentRun {
    trace: String,
    trace_digest: String,
    evidence: String,
    evidence_digest: String,
    status: AgentRunStatus,
    replay: private::EvidenceReplay,
}

impl fmt::Debug for Profile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Profile(<redacted>)")
    }
}

impl fmt::Debug for Task {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Task(<redacted>)")
    }
}

/// An injected provider response. The runtime owns no transport.
/// The closed result of one injected provider attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AgentProviderAttempt {
    disposition: ProviderDisposition,
    usage: ProviderUsage,
}

impl AgentProviderAttempt {
    /// Constructs one closed attempt result without an error-text channel.
    pub const fn new(disposition: AgentProviderDisposition, usage: AgentProviderUsage) -> Self {
        Self { disposition, usage }
    }

    /// Returns whether the provider attempt started and how it finished.
    pub const fn disposition(&self) -> AgentProviderDisposition {
        self.disposition
    }
    /// Returns the closed reported usage.
    pub const fn usage(&self) -> AgentProviderUsage {
        self.usage
    }
}

/// Whether an injected provider attempt started and how it finished.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentProviderDisposition {
    Succeeded,
    DefinitelyNotStarted,
    FailedUncertain,
}

/// Closed provider-reported usage for one attempt.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AgentProviderUsage {
    input_tokens: u64,
    output_tokens: u64,
    usd_microunits: u64,
}

impl AgentProviderUsage {
    /// Constructs closed provider-reported usage.
    pub const fn new(input_tokens: u64, output_tokens: u64, usd_microunits: u64) -> Self {
        Self {
            input_tokens,
            output_tokens,
            usd_microunits,
        }
    }

    /// Returns reported input tokens.
    pub const fn input_tokens(&self) -> u64 {
        self.input_tokens
    }
    /// Returns reported output tokens.
    pub const fn output_tokens(&self) -> u64 {
        self.output_tokens
    }
    /// Returns reported cost in USD microunits.
    pub const fn usd_microunits(&self) -> u64 {
        self.usd_microunits
    }
}

/// Caller-injected provider and read-only tool host.
///
/// Observation, probing, and tokenization methods are pure local observations.
/// Only `attempt_provider` and `invoke_tool` may cross an external-effect
/// boundary. Implementations that violate this contract are outside v1.
pub trait AgentHost {
    /// Returns the current monotonic policy epoch as a pure observation.
    fn policy_epoch(&self) -> u64;
    /// Returns elapsed milliseconds as a pure observation.
    fn elapsed_ms(&self) -> u64;
    /// Returns a pure policy/time probe for runtime-owned sinks.
    fn boundary_probe(&self) -> Box<dyn AgentBoundaryProbe>;
    /// Counts request tokens locally without crossing an external boundary.
    fn tokenize(&mut self, tokenizer_id: &str, request: &str) -> Option<u64>;
    /// Performs the sole provider external boundary and streams into `sink`.
    fn attempt_provider(
        &mut self,
        provider_id: &str,
        model_id: &str,
        request: &str,
        deadline_ms: u64,
        sink: &mut AgentProviderSink,
    ) -> AgentProviderAttempt;
    /// Invokes one contractually read-only registered tool external boundary.
    fn invoke_tool(
        &mut self,
        call_id: &str,
        tool_id: &str,
        arguments_json: &str,
        sink: &mut AgentToolResultSink,
    ) -> bool;
}

/// Pure boundary observations used by runtime-owned streaming sinks.
pub trait AgentBoundaryProbe {
    /// Returns the current monotonic policy epoch without external effects.
    fn policy_epoch(&self) -> u64;
    /// Returns elapsed milliseconds without external effects.
    fn elapsed_ms(&self) -> u64;
}

/// Opaque runtime-owned bounded provider response sink.
pub struct AgentProviderSink {
    bytes: Vec<u8>,
    chunks: u64,
    byte_limit: usize,
    chunk_limit: u64,
    rejection: Option<SinkRejection>,
    probe: Box<dyn AgentBoundaryProbe>,
    admitted_policy_epoch: u64,
    deadline_ms: u64,
    boundary: Option<AgentRunStatus>,
    cancellation: AgentCancellation,
    builder_prepaid: bool,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum SinkRejection {
    Bytes,
    Chunks,
    Builder,
}

impl AgentProviderSink {
    fn new(
        limits: EffectiveLimits,
        byte_limit: u64,
        probe: Box<dyn AgentBoundaryProbe>,
        admitted_policy_epoch: u64,
        cancellation: AgentCancellation,
    ) -> Self {
        Self {
            bytes: Vec::new(),
            chunks: 0,
            byte_limit: byte_limit.min(limits.max_provider_response_bytes) as usize,
            chunk_limit: limits.max_stream_chunks,
            rejection: None,
            probe,
            admitted_policy_epoch,
            deadline_ms: limits.max_elapsed_ms,
            boundary: None,
            cancellation,
            builder_prepaid: true,
        }
    }

    /// Appends one bounded chunk; after the first `false`, all later pushes fail.
    pub fn push(&mut self, chunk: &[u8]) -> bool {
        if self.rejection.is_some() || self.boundary.is_some() {
            return false;
        }
        if self.cancellation.is_cancelled() {
            self.boundary = Some(AgentRunStatus::Cancelled);
            return false;
        }
        if self.probe.elapsed_ms() > self.deadline_ms {
            self.boundary = Some(AgentRunStatus::DeadlineExceeded);
            return false;
        }
        if self.probe.policy_epoch() != self.admitted_policy_epoch {
            self.boundary = Some(AgentRunStatus::PolicyRejected);
            return false;
        }
        self.chunks = self.chunks.saturating_add(1);
        let Some(length) = self.bytes.len().checked_add(chunk.len()) else {
            self.rejection = Some(SinkRejection::Bytes);
            return false;
        };
        if self.chunks > self.chunk_limit {
            self.rejection = Some(SinkRejection::Chunks);
            return false;
        }
        if length > self.byte_limit {
            self.rejection = Some(SinkRejection::Bytes);
            return false;
        }
        if !self.builder_prepaid && !reserve_active(chunk.len()) {
            self.rejection = Some(SinkRejection::Builder);
            return false;
        }
        self.bytes.extend_from_slice(chunk);
        true
    }
}

/// Opaque runtime-owned bounded tool-result sink.
pub struct AgentToolResultSink {
    bytes: Vec<u8>,
    byte_limit: usize,
    rejection: Option<SinkRejection>,
    probe: Box<dyn AgentBoundaryProbe>,
    admitted_policy_epoch: u64,
    deadline_ms: u64,
    boundary: Option<AgentRunStatus>,
    cancellation: AgentCancellation,
    builder_prepaid: bool,
}

impl AgentToolResultSink {
    fn new(
        limit: u64,
        probe: Box<dyn AgentBoundaryProbe>,
        admitted_policy_epoch: u64,
        deadline_ms: u64,
        cancellation: AgentCancellation,
    ) -> Self {
        Self {
            bytes: Vec::new(),
            byte_limit: limit as usize,
            rejection: None,
            probe,
            admitted_policy_epoch,
            deadline_ms,
            boundary: None,
            cancellation,
            builder_prepaid: true,
        }
    }
    /// Appends one bounded chunk; after the first `false`, all later pushes fail.
    pub fn push(&mut self, chunk: &[u8]) -> bool {
        if self.rejection.is_some() || self.boundary.is_some() {
            return false;
        }
        if self.cancellation.is_cancelled() {
            self.boundary = Some(AgentRunStatus::Cancelled);
            return false;
        }
        if self.probe.elapsed_ms() > self.deadline_ms {
            self.boundary = Some(AgentRunStatus::DeadlineExceeded);
            return false;
        }
        if self.probe.policy_epoch() != self.admitted_policy_epoch {
            self.boundary = Some(AgentRunStatus::PolicyRejected);
            return false;
        }
        let Some(length) = self.bytes.len().checked_add(chunk.len()) else {
            self.rejection = Some(SinkRejection::Bytes);
            return false;
        };
        if length > self.byte_limit {
            self.rejection = Some(SinkRejection::Bytes);
            return false;
        }
        if !self.builder_prepaid && !reserve_active(chunk.len()) {
            self.rejection = Some(SinkRejection::Builder);
            return false;
        }
        self.bytes.extend_from_slice(chunk);
        true
    }
}

/// Opaque single-run-at-a-time Agent over one caller-injected host.
pub struct Agent<H: AgentHost> {
    profile: Profile,
    profile_builder_bytes: u64,
    host: H,
    cancellation: AgentCancellation,
}

/// Monotonic cooperative cancellation shared by an Agent and its sinks.
#[derive(Clone)]
pub struct AgentCancellation {
    cancelled: Arc<AtomicBool>,
}

impl AgentCancellation {
    /// Creates an uncancelled monotonic cancellation handle.
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Cancels every clone permanently.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    /// Returns whether cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

impl Default for AgentCancellation {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentRun {
    /// Returns the closed terminal status.
    pub const fn status(&self) -> AgentRunStatus {
        self.status
    }
    /// Returns the untrusted final model message only for a completed run.
    pub fn final_message(&self) -> Option<&str> {
        self.replay.final_message()
    }
    /// Returns the canonical Trace document including its terminal LF.
    pub fn trace(&self) -> &str {
        &self.trace
    }
    /// Returns the domain-separated Trace digest.
    pub fn trace_digest(&self) -> &str {
        &self.trace_digest
    }
    /// Returns the canonical Evidence document including its terminal LF.
    pub fn evidence(&self) -> &str {
        &self.evidence
    }
    /// Returns the domain-separated Evidence digest.
    pub fn evidence_digest(&self) -> &str {
        &self.evidence_digest
    }

    pub(crate) fn economic_binding(&self) -> EconomicAgentBinding<'_> {
        EconomicAgentBinding {
            status: self.status,
            final_message: self.replay.final_message(),
            run_id: self.replay.run_id(),
            evidence: &self.evidence,
            evidence_digest: &self.evidence_digest,
        }
    }
}

pub(crate) struct EconomicAgentBinding<'a> {
    pub(crate) status: AgentRunStatus,
    pub(crate) final_message: Option<&'a str>,
    pub(crate) run_id: &'a str,
    pub(crate) evidence: &'a str,
    pub(crate) evidence_digest: &'a str,
}

type RunStatus = AgentRunStatus;
type AgentRuntimeEvidence = AgentRun;
type ProviderAttempt = AgentProviderAttempt;
type ProviderDisposition = AgentProviderDisposition;
type ProviderUsage = AgentProviderUsage;
type ProviderSink = AgentProviderSink;
type ToolResultSink = AgentToolResultSink;

fn g204(document: &str, schema: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-G204",
        format!("Agent Runtime {document} is not canonical {schema} JSON"),
    )
}

fn g205(field: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-G205",
        format!("Agent Runtime profile invariant failed: {field}"),
    )
}

fn g206() -> Diagnostic {
    Diagnostic::io(
        "SPX-G206",
        "Agent Runtime has no eligible model under the frozen routing policy",
    )
}

fn g207(reason: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-G207",
        format!("Agent Runtime action or tool authorization was rejected: {reason}"),
    )
}

fn g208(field: &str, maximum: u64) -> Diagnostic {
    Diagnostic::io("SPX-G208", format!("{field} exceeds {maximum}"))
}

fn g209() -> Diagnostic {
    Diagnostic::io(
        "SPX-G209",
        "Agent Runtime trace or Evidence disagrees with the replayed state machine",
    )
}

fn operational(code: &'static str, message: &'static str) -> Diagnostic {
    Diagnostic::io(code, message)
}

fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(bytes);
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

fn run_id(profile_digest: &str, task_digest: &str, nonce: &str) -> Result<String, Diagnostic> {
    let nonce = decode_hex_32(nonce).ok_or_else(|| g204("task", TASK_SCHEMA))?;
    let mut hash = Sha256::new();
    hash.update(RUN_ID_DOMAIN);
    hash.update(profile_digest.as_bytes());
    hash.update(task_digest.as_bytes());
    hash.update(nonce);
    Ok(format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hash.finalize())
    ))
}

fn call_id(run_id: &str, turn: u64, tool_id: &str, arguments: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(CALL_ID_DOMAIN);
    hash.update(run_id.as_bytes());
    hash.update(turn.to_be_bytes());
    hash.update(tool_id.as_bytes());
    hash.update(arguments.as_bytes());
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

fn decode_hex_32(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return None;
    }
    let mut output = [0u8; 32];
    for (index, slot) in output.iter_mut().enumerate() {
        *slot = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(output)
}

fn canonical_document<'a>(
    source: &'a str,
    label: &str,
    schema: &str,
    maximum: usize,
) -> Result<&'a str, Diagnostic> {
    if source.len() > maximum {
        return Err(g208(
            match label {
                "profile" => "profile_bytes",
                "task" => "task_bytes",
                "action" => "provider_response_bytes",
                "trace" => "trace_bytes",
                "evidence" => "evidence_bytes",
                "provider request" => "provider_request_bytes",
                "tool result" => "tool_result_bytes",
                _ => "builder_bytes",
            },
            maximum as u64,
        ));
    }
    let Some(body) = source.strip_suffix('\n') else {
        return Err(g204(label, schema));
    };
    if body.is_empty() || body.contains('\n') || body.contains('\r') || body.starts_with('\u{feff}')
    {
        return Err(g204(label, schema));
    }
    let value: Value = serde_json::from_str(body).map_err(|_| g204(label, schema))?;
    if json_depth(&value) > MAX_JSON_DEPTH {
        return Err(g208("json_depth", MAX_JSON_DEPTH as u64));
    }
    if value
        .as_object()
        .and_then(|object| object.get("schema"))
        .and_then(Value::as_str)
        != Some(schema)
    {
        return Err(g204(label, schema));
    }
    Ok(body)
}

fn json_depth(value: &Value) -> usize {
    match value {
        Value::Array(values) => 1 + values.iter().map(json_depth).max().unwrap_or(0),
        Value::Object(values) => 1 + values.values().map(json_depth).max().unwrap_or(0),
        _ => 1,
    }
}

fn exact_keys(object: &Map<String, Value>, keys: &[&str]) -> bool {
    object.len() == keys.len() && keys.iter().all(|key| object.contains_key(*key))
}

fn object<'a>(
    value: &'a Value,
    label: &str,
    schema: &str,
) -> Result<&'a Map<String, Value>, Diagnostic> {
    value.as_object().ok_or_else(|| g204(label, schema))
}

fn string_member<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    label: &str,
    schema: &str,
) -> Result<&'a str, Diagnostic> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| g204(label, schema))
}

fn u64_member(
    object: &Map<String, Value>,
    key: &str,
    label: &str,
    schema: &str,
) -> Result<u64, Diagnostic> {
    object
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| g204(label, schema))
}

fn string_array_member(
    object: &Map<String, Value>,
    key: &str,
    label: &str,
    schema: &str,
) -> Result<Vec<String>, Diagnostic> {
    object
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| g204(label, schema))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| g204(label, schema))
        })
        .collect()
}

fn sorted_unique(values: &[String]) -> bool {
    values.windows(2).all(|window| window[0] < window[1])
}

fn canonical_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_IDENTIFIER_BYTES
        && !value.contains('\0')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

fn json_string_array(values: &[String]) -> String {
    let mut output = String::from("[");
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&quote_json(value));
    }
    output.push(']');
    output
}

fn render_schema(schema: &ClosedSchema) -> String {
    let mut output = String::from("{\"type\":\"object\",\"fields\":[");
    for (index, field) in schema.fields.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str("{\"name\":");
        output.push_str(&quote_json(&field.name));
        output.push_str(",\"type\":");
        output.push_str(&quote_json(field.kind.text()));
        output.push_str(",\"required\":");
        output.push_str(if field.required { "true" } else { "false" });
        output.push_str(",\"max_bytes\":");
        output.push_str(&field.max_bytes.to_string());
        output.push('}');
    }
    output.push_str("],\"additional_properties\":false}");
    output
}

fn parse_schema(value: &Value) -> Result<ClosedSchema, Diagnostic> {
    let schema_object = object(value, "profile", PROFILE_SCHEMA)?;
    if !exact_keys(schema_object, &["type", "fields", "additional_properties"])
        || string_member(schema_object, "type", "profile", PROFILE_SCHEMA)? != "object"
        || schema_object
            .get("additional_properties")
            .and_then(Value::as_bool)
            != Some(false)
    {
        return Err(g204("profile", PROFILE_SCHEMA));
    }
    let mut fields = Vec::new();
    for value in schema_object
        .get("fields")
        .and_then(Value::as_array)
        .ok_or_else(|| g204("profile", PROFILE_SCHEMA))?
    {
        let row = object(value, "profile", PROFILE_SCHEMA)?;
        if !exact_keys(row, &["name", "type", "required", "max_bytes"]) {
            return Err(g204("profile", PROFILE_SCHEMA));
        }
        let name = string_member(row, "name", "profile", PROFILE_SCHEMA)?.to_owned();
        let kind = match string_member(row, "type", "profile", PROFILE_SCHEMA)? {
            "string" => ScalarKind::String,
            "integer" => ScalarKind::Integer,
            "boolean" => ScalarKind::Boolean,
            _ => return Err(g204("profile", PROFILE_SCHEMA)),
        };
        let required = row
            .get("required")
            .and_then(Value::as_bool)
            .ok_or_else(|| g204("profile", PROFILE_SCHEMA))?;
        let max_bytes = u64_member(row, "max_bytes", "profile", PROFILE_SCHEMA)?;
        let valid_max = match kind {
            ScalarKind::String => (1..=MAX_TOOL_RESULT_BYTES).contains(&max_bytes),
            ScalarKind::Integer => max_bytes == 20,
            ScalarKind::Boolean => max_bytes == 5,
        };
        if !canonical_identifier(&name) || !valid_max {
            return Err(g205("tools.schema.fields"));
        }
        fields.push(SchemaField {
            name,
            kind,
            required,
            max_bytes,
        });
    }
    if !fields.windows(2).all(|rows| rows[0].name < rows[1].name) {
        return Err(g205("tools.schema.fields"));
    }
    Ok(ClosedSchema { fields })
}

fn render_profile(profile: &Profile) -> String {
    let mut output = String::from("{\"schema\":\"");
    output.push_str(PROFILE_SCHEMA);
    output.push_str("\",\"agent_id\":");
    output.push_str(&quote_json(&profile.agent_id));
    output.push_str(",\"models\":[");
    for (index, model) in profile.models.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str("{\"provider_id\":");
        output.push_str(&quote_json(&model.provider_id));
        output.push_str(",\"model_id\":");
        output.push_str(&quote_json(&model.model_id));
        output.push_str(",\"locality\":");
        output.push_str(&quote_json(model.locality.text()));
        output.push_str(",\"quality_tier\":");
        output.push_str(&quote_json(model.quality_tier.text()));
        output.push_str(",\"tokenizer_id\":");
        output.push_str(&quote_json(&model.tokenizer_id));
        output.push_str(",\"max_context_tokens\":");
        output.push_str(&model.max_context_tokens.to_string());
        output.push_str(",\"input_usd_microunits_per_million_tokens\":");
        output.push_str(&model.input_price.to_string());
        output.push_str(",\"output_usd_microunits_per_million_tokens\":");
        output.push_str(&model.output_price.to_string());
        output.push_str(",\"capabilities\":");
        output.push_str(&json_string_array(&model.capabilities));
        output.push('}');
    }
    output.push_str("],\"tools\":[");
    for (index, tool) in profile.tools.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str("{\"tool_id\":");
        output.push_str(&quote_json(&tool.tool_id));
        output.push_str(",\"description\":");
        output.push_str(&quote_json(&tool.description));
        output.push_str(",\"arguments_schema\":");
        output.push_str(&render_schema(&tool.arguments_schema));
        output.push_str(",\"result_schema\":");
        output.push_str(&render_schema(&tool.result_schema));
        output.push_str(",\"effects\":[\"read\"],\"required_capabilities\":");
        output.push_str(&json_string_array(&tool.required_capabilities));
        output.push('}');
    }
    output.push_str("],\"policy\":{");
    output.push_str("\"allowed_provider_ids\":");
    output.push_str(&json_string_array(&profile.policy.allowed_provider_ids));
    output.push_str(",\"allowed_model_ids\":");
    output.push_str(&json_string_array(&profile.policy.allowed_model_ids));
    output.push_str(",\"required_locality\":");
    output.push_str(&quote_json(profile.policy.required_locality.text()));
    output.push_str(",\"minimum_quality_tier\":");
    output.push_str(&quote_json(profile.policy.minimum_quality_tier.text()));
    output.push_str(",\"required_model_capabilities\":");
    output.push_str(&json_string_array(
        &profile.policy.required_model_capabilities,
    ));
    output.push_str(",\"granted_capabilities\":");
    output.push_str(&json_string_array(&profile.policy.granted_capabilities));
    output.push_str(",\"allowed_tool_ids\":");
    output.push_str(&json_string_array(&profile.policy.allowed_tool_ids));
    output.push('}');
    output.push_str(",\"limits\":");
    output.push_str(&render_effective_limits(profile.limits));
    output.push_str(",\"nonclaims\":");
    output.push_str(&json_string_array(
        &NONCLAIMS
            .iter()
            .map(|s| (*s).to_owned())
            .collect::<Vec<_>>(),
    ));
    output.push_str("}\n");
    output
}

fn render_effective_limits(limits: EffectiveLimits) -> String {
    format!("{{\"max_turns\":{},\"max_provider_attempts\":{},\"max_retries_per_turn\":{},\"max_concurrency\":{},\"max_elapsed_ms\":{},\"max_provider_request_bytes\":{},\"max_provider_response_bytes\":{},\"max_stream_chunks\":{},\"max_total_provider_input_bytes\":{},\"max_total_provider_output_bytes\":{},\"max_reported_model_input_tokens\":{},\"max_reported_model_output_tokens\":{},\"max_usd_microunits\":{},\"max_tool_calls\":{},\"max_tool_arguments_bytes\":{},\"max_tool_result_bytes\":{},\"max_total_tool_bytes\":{},\"max_retained_state_bytes\":{},\"max_trace_events\":{},\"max_trace_bytes\":{},\"max_evidence_bytes\":{},\"max_builder_bytes\":{}}}", limits.max_turns, limits.max_provider_attempts, limits.max_retries_per_turn, limits.max_concurrency, limits.max_elapsed_ms, limits.max_provider_request_bytes, limits.max_provider_response_bytes, limits.max_stream_chunks, limits.max_total_provider_input_bytes, limits.max_total_provider_output_bytes, limits.max_reported_model_input_tokens, limits.max_reported_model_output_tokens, limits.max_usd_microunits, limits.max_tool_calls, limits.max_tool_arguments_bytes, limits.max_tool_result_bytes, limits.max_total_tool_bytes, limits.max_retained_state_bytes, limits.max_trace_events, limits.max_trace_bytes, limits.max_evidence_bytes, limits.max_builder_bytes)
}

// Parsing, routing, execution, trace rendering, replay, and evidence rendering
// are split into sibling implementation files to keep each authority boundary
// reviewable.
mod private;

#[cfg(test)]
pub(crate) use private::completed_run_for_economic_test;

#[cfg(test)]
mod tests;
