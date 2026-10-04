//! Stage traits and implementations. Builtin stages need no process; the
//! `Host*` stages call a selected provider through the adapter host. Provider
//! output is untrusted data: it can only suggest context or a proposal.

use super::b64;
use super::lineage::Lineage;
use crate::cli::Environment;
use crate::contract::{CapabilityKind, CapabilityRef, RequestEnvelope};
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::host::grant::Grant;
use crate::host::{AdapterHandle, CancelToken, InvocationClass, Outcome};
use crate::profile::check_grant_current;
use serde_json::{json, Map, Value};
use std::path::PathBuf;
use std::sync::Arc;

pub const NATIVE_CONTEXT_ID: &str = "semaprax/native-context";
pub const SCRIPTED_ID: &str = "semaprax/scripted-proposer";
pub const RAW_VIEW_ID: &str = "semaprax/raw-command";

/// Intent kinds the compiler's candidate operation admits (docs/PROJECT-CANDIDATES-V1.md).
pub const INTENT_KINDS: [&str; 13] = [
    "rename_declaration",
    "change_function_signature",
    "replace_function_body",
    "replace_expression",
    "replace_contract_expression",
    "add_contract",
    "add_declaration",
    "delete_declaration",
    "extract_function",
    "move_declaration",
    "add_record_field",
    "implement_interface",
    "repair_diagnostic",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StageFailure {
    /// Nothing usable came back; the caller may fall back to a builtin.
    Unavailable(HarnessDiagnostic),
    /// The result was refused or the provider breached the protocol.
    Refused(HarnessDiagnostic),
    /// A side-effecting request was sent and its outcome is unknown.
    Uncertain(HarnessDiagnostic),
}

fn d(code: &'static str, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

// ---- task ---------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalContext {
    Never,
    WhenNeeded,
    Always,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Task {
    pub goal: String,
    pub seed: Option<String>,
    pub family: String,
    pub external_context: ExternalContext,
    pub models: Option<Value>,
}

impl Default for Task {
    fn default() -> Self {
        Task {
            goal: "repair the compiler-reported failure".into(),
            seed: None,
            family: "localized_debug".into(),
            external_context: ExternalContext::WhenNeeded,
            models: None,
        }
    }
}

impl Task {
    pub fn parse(bytes: &[u8]) -> HarnessResult<Task> {
        let bad = |m: String| d("SPX-HPD081", m);
        let v = crate::json::parse_strict(
            bytes,
            &crate::json::JsonLimits {
                max_bytes: 256 * 1024,
                max_depth: 16,
                max_nodes: 4096,
            },
        )
        .map_err(|e| bad(format!("task: {}", e.message)))?;
        let m = v
            .as_object()
            .ok_or_else(|| bad("task must be an object".into()))?;
        for k in m.keys() {
            if ![
                "schema",
                "goal",
                "seed",
                "task_family",
                "external_context",
                "models",
            ]
            .contains(&k.as_str())
            {
                return Err(bad(format!("unknown task member `{k}`")));
            }
        }
        if m.get("schema").and_then(Value::as_str) != Some("semaprax.harness-task.v1") {
            return Err(bad("task schema must be `semaprax.harness-task.v1`".into()));
        }
        let mut t = Task::default();
        if let Some(g) = m.get("goal") {
            t.goal = g
                .as_str()
                .filter(|s| s.len() <= 4096)
                .ok_or_else(|| bad("`goal` must be a string of at most 4096 bytes".into()))?
                .into();
        }
        if let Some(s) = m.get("seed") {
            t.seed = Some(
                s.as_str()
                    .filter(|s| !s.is_empty() && s.len() <= 256)
                    .ok_or_else(|| bad("`seed` must be a stable id".into()))?
                    .into(),
            );
        }
        if let Some(f) = m.get("task_family") {
            t.family = f
                .as_str()
                .ok_or_else(|| bad("`task_family` must be a string".into()))?
                .into();
        }
        if let Some(e) = m.get("external_context") {
            t.external_context = match e.as_str() {
                Some("never") => ExternalContext::Never,
                Some("when-needed") => ExternalContext::WhenNeeded,
                Some("always") => ExternalContext::Always,
                _ => {
                    return Err(bad(
                        "`external_context` must be never, when-needed or always".into(),
                    ))
                }
            };
        }
        t.models = m.get("models").cloned();
        Ok(t)
    }

    /// Digest of the task; the goal text itself never enters reports.
    pub fn digest(&self) -> String {
        crate::json::digest(
            "semaprax.harness-task.v1",
            &json!({"goal": self.goal, "seed": self.seed, "family": self.family,
                    "external_context": format!("{:?}", self.external_context), "models": self.models}),
        )
    }
}

// ---- context ------------------------------------------------------------

pub struct ContextRequest<'a> {
    pub lineage: &'a Lineage,
    pub project: PathBuf,
    pub seed: Option<&'a str>,
    pub query: String,
    pub max_bytes: usize,
    /// The task's external-context policy (a composite stage honors it).
    pub external: ExternalContext,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextItem {
    pub label: String,
    pub provenance: String,
    pub text: String,
}

impl ContextItem {
    pub fn bytes(&self) -> usize {
        self.label.len() + self.provenance.len() + self.text.len()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextPacket {
    pub provider: String,
    pub items: Vec<ContextItem>,
    pub complete: bool,
}

pub trait ContextStage {
    fn id(&self) -> String;
    fn collect(&mut self, req: &ContextRequest) -> Result<ContextPacket, StageFailure>;
    /// Provider invocations performed (builtin native calls are not host calls).
    fn calls(&self) -> u32;
    /// A one-shot note about the last collection (for example a provider
    /// failure absorbed by a fallback); reported, never silent.
    fn take_note(&mut self) -> Option<String> {
        None
    }
}

/// Native context: the compiler's own `semaprax context` for the seed.
pub struct NativeContext<'a> {
    pub compiler: &'a dyn super::compiler::CompilerService,
    calls: u32,
}

impl<'a> NativeContext<'a> {
    pub fn new(compiler: &'a dyn super::compiler::CompilerService) -> Self {
        Self { compiler, calls: 0 }
    }
}

impl ContextStage for NativeContext<'_> {
    fn id(&self) -> String {
        NATIVE_CONTEXT_ID.into()
    }
    fn collect(&mut self, req: &ContextRequest) -> Result<ContextPacket, StageFailure> {
        let Some(seed) = req.seed else {
            return Err(StageFailure::Unavailable(d(
                "SPX-HPD021",
                "no seed stable id for native context",
            )));
        };
        let text = self
            .compiler
            .context(&req.project, seed, req.max_bytes.max(2048))
            .map_err(StageFailure::Unavailable)?;
        Ok(ContextPacket {
            provider: self.id(),
            items: vec![ContextItem {
                label: format!("native:{seed}"),
                provenance: super::broker_stage::COMPILER_VERIFIED.into(),
                text,
            }],
            complete: true,
        })
    }
    fn calls(&self) -> u32 {
        self.calls
    }
}

fn envelope(
    lineage: &Lineage,
    kind: CapabilityKind,
    op: &str,
    max_result: usize,
    payload: Value,
) -> RequestEnvelope {
    RequestEnvelope {
        invocation_id: lineage.next_invocation(),
        project: lineage.project.clone(),
        lock_digest: lineage.lock_digest.clone(),
        capability: CapabilityRef { kind, version: 1 },
        operation: op.into(),
        deadline_ms: 30_000,
        max_result_bytes: max_result.clamp(1024, 4 * 1024 * 1024),
        remaining_calls: 8,
        lineage: lineage.parents(),
        payload,
    }
}

/// Decide how an invocation outcome maps to the stage contract.
fn failure_of(o: Outcome) -> StageFailure {
    match o {
        Outcome::Completed(r) => StageFailure::Unavailable(d(
            "SPX-HPD021",
            format!(
                "provider answered `{}` without usable data",
                r.status.as_str()
            ),
        )),
        Outcome::Refused(x) | Outcome::Quarantined(x) => StageFailure::Refused(x),
        Outcome::Unavailable { reason, .. } => StageFailure::Unavailable(reason),
        Outcome::Cancelled => {
            StageFailure::Unavailable(d("SPX-HPD021", "provider invocation cancelled"))
        }
        Outcome::Uncertain(x) => StageFailure::Uncertain(x),
    }
}

/// `context.repository` provider through the adapter host (safe-read class).
pub struct HostContext {
    pub handle: Arc<AdapterHandle>,
    pub grant: Grant,
    pub env: Environment,
    pub provider_id: String,
    calls: u32,
}

impl HostContext {
    pub fn new(
        handle: Arc<AdapterHandle>,
        grant: Grant,
        env: Environment,
        provider_id: String,
    ) -> Self {
        Self {
            handle,
            grant,
            env,
            provider_id,
            calls: 0,
        }
    }
}

impl ContextStage for HostContext {
    fn id(&self) -> String {
        self.provider_id.clone()
    }
    fn collect(&mut self, req: &ContextRequest) -> Result<ContextPacket, StageFailure> {
        // Revocation or any digest drift takes effect on this dispatch.
        check_grant_current(&self.env, &self.grant).map_err(StageFailure::Refused)?;
        let query = req
            .seed
            .map(str::to_string)
            .unwrap_or_else(|| req.query.clone());
        let request = envelope(
            req.lineage,
            CapabilityKind::ContextRepository,
            "search",
            req.max_bytes,
            json!({"query": query, "max_items": 8}),
        );
        self.calls += 1;
        match self
            .handle
            .invoke(&request, InvocationClass::SafeRead, &CancelToken::new())
        {
            Outcome::Completed(r) if r.payload.is_some() => {
                let p = r.payload.unwrap_or(Value::Null);
                let mut items = Vec::new();
                for it in p["items"].as_array().into_iter().flatten() {
                    let s = |k: &str| it[k].as_str().unwrap_or("").to_string();
                    items.push(ContextItem {
                        label: format!(
                            "{}:{}-{}",
                            s("path"),
                            it["span"]["start_line"],
                            it["span"]["end_line"]
                        ),
                        // Provider provenance is a label, never compiler verification.
                        provenance: format!("external:{}", s("provenance")),
                        text: s("text"),
                    });
                }
                Ok(ContextPacket {
                    provider: self.provider_id.clone(),
                    items,
                    complete: p["coverage"]["complete"].as_bool().unwrap_or(false),
                })
            }
            other => Err(failure_of(other)),
        }
    }
    fn calls(&self) -> u32 {
        self.calls
    }
}

// ---- proposals ----------------------------------------------------------

pub struct ProposalRequest<'a> {
    pub lineage: &'a Lineage,
    /// Host-built prompt document (bounded; no secrets).
    pub prompt: Value,
    /// Logical model id chosen by the route decision.
    pub model: String,
}

pub trait ProposalStage {
    fn id(&self) -> String;
    /// Raw proposal bytes (`semaprax.harness-proposal.v1`); still untrusted.
    fn propose(&mut self, req: &ProposalRequest) -> Result<Vec<u8>, StageFailure>;
    fn calls(&self) -> u32;
    /// Whether a call is non-idempotent (never replayed after a restart).
    fn side_effecting(&self) -> bool;
}

/// Reads a proposal from a host-supplied file (tests and CLI).
pub struct ScriptedProposer {
    pub path: Option<PathBuf>,
    pub inline: Option<Vec<u8>>,
}

impl ScriptedProposer {
    pub fn from_file(path: PathBuf) -> Self {
        Self {
            path: Some(path),
            inline: None,
        }
    }
    /// No source: every call is `Unavailable`.
    pub fn empty() -> Self {
        Self {
            path: None,
            inline: None,
        }
    }
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self {
            path: None,
            inline: Some(bytes),
        }
    }
}

impl ProposalStage for ScriptedProposer {
    fn id(&self) -> String {
        SCRIPTED_ID.into()
    }
    fn propose(&mut self, _req: &ProposalRequest) -> Result<Vec<u8>, StageFailure> {
        if let Some(b) = &self.inline {
            return Ok(b.clone());
        }
        let path = self
            .path
            .as_ref()
            .ok_or_else(|| StageFailure::Unavailable(d("SPX-HPD090", "no proposal source")))?;
        let meta = std::fs::metadata(path).map_err(|e| {
            StageFailure::Unavailable(d("SPX-HPD090", format!("proposal {}: {e}", path.display())))
        })?;
        if meta.len() > 1024 * 1024 {
            return Err(StageFailure::Refused(d(
                "SPX-HPD030",
                "proposal exceeds 1 MiB",
            )));
        }
        std::fs::read(path)
            .map_err(|e| StageFailure::Unavailable(d("SPX-HPD090", format!("proposal: {e}"))))
    }
    fn calls(&self) -> u32 {
        0
    }
    fn side_effecting(&self) -> bool {
        false
    }
}

/// `model.generate` provider through the adapter host (side-effecting class:
/// a request that was sent is never retried).
pub struct HostModel {
    pub handle: Arc<AdapterHandle>,
    pub grant: Grant,
    pub env: Environment,
    pub provider_id: String,
    calls: u32,
}

impl HostModel {
    pub fn new(
        handle: Arc<AdapterHandle>,
        grant: Grant,
        env: Environment,
        provider_id: String,
    ) -> Self {
        Self {
            handle,
            grant,
            env,
            provider_id,
            calls: 0,
        }
    }
}

impl ProposalStage for HostModel {
    fn id(&self) -> String {
        self.provider_id.clone()
    }
    fn propose(&mut self, req: &ProposalRequest) -> Result<Vec<u8>, StageFailure> {
        check_grant_current(&self.env, &self.grant).map_err(StageFailure::Refused)?;
        let prompt = crate::json::canonical(&req.prompt);
        let request = envelope(
            req.lineage,
            CapabilityKind::ModelGenerate,
            "generate",
            1 << 20,
            json!({"model": req.model, "input_base64": b64::encode(prompt.as_bytes()), "max_output_bytes": 65536}),
        );
        self.calls += 1;
        match self.handle.invoke(
            &request,
            InvocationClass::SideEffecting,
            &CancelToken::new(),
        ) {
            Outcome::Completed(r) if r.payload.is_some() => {
                let out = r
                    .payload
                    .as_ref()
                    .and_then(|p| p["output_base64"].as_str())
                    .and_then(b64::decode);
                out.ok_or_else(|| {
                    StageFailure::Refused(d("SPX-HPD030", "model output is not base64"))
                })
            }
            other => Err(failure_of(other)),
        }
    }
    fn calls(&self) -> u32 {
        self.calls
    }
    fn side_effecting(&self) -> bool {
        true
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Proposal {
    pub intent: Value,
    pub kind: String,
    /// Claims the provider made about validity or test results. Never used.
    pub claims: Value,
}

/// Strict proposal parse. The host builds the change envelope itself, so a
/// proposal carries only an `intent`: raw source, requirements, base revision
/// and authority-like members are all refused.
pub fn parse_proposal(bytes: &[u8]) -> HarnessResult<Proposal> {
    let v = crate::json::parse_strict(
        bytes,
        &crate::json::JsonLimits {
            max_bytes: 1024 * 1024,
            max_depth: 64,
            max_nodes: 8192,
        },
    )
    .map_err(|e| d("SPX-HPD030", format!("proposal: {}", e.message)))?;
    let m: &Map<String, Value> = v
        .as_object()
        .ok_or_else(|| d("SPX-HPD030", "proposal must be an object"))?;
    for k in m.keys() {
        let lk = k.to_ascii_lowercase();
        match lk.as_str() {
            "schema" | "intent" | "claims" | "summary" => {}
            "requirements" | "base_revision" => {
                return Err(d("SPX-HPD032", format!("`{k}` is a protected compiler fact; the host supplies it")))
            }
            "source" | "files" | "patch" | "diff" | "replacement_source" | "raw_source" | "write" => {
                return Err(d("SPX-HPD031", format!("`{k}`: raw source writes are never accepted; propose a compiler-supported intent")))
            }
            "publish" | "apply" | "approve" | "approved" | "execute" | "grant" | "authority" | "commit" => {
                return Err(d("SPX-HPD033", format!("`{k}`: a provider cannot initiate publication or grant authority")))
            }
            _ => return Err(d("SPX-HPD030", format!("unknown proposal member `{k}`"))),
        }
    }
    if m.get("schema").and_then(Value::as_str) != Some("semaprax.harness-proposal.v1") {
        return Err(d(
            "SPX-HPD030",
            "proposal schema must be `semaprax.harness-proposal.v1`",
        ));
    }
    let intent = m
        .get("intent")
        .filter(|i| i.is_object())
        .ok_or_else(|| d("SPX-HPD030", "proposal needs an `intent` object"))?;
    let kind = intent["kind"]
        .as_str()
        .ok_or_else(|| d("SPX-HPD030", "intent needs a `kind`"))?
        .to_string();
    if !INTENT_KINDS.contains(&kind.as_str()) {
        return Err(d(
            "SPX-HPD031",
            format!("unsupported change kind `{kind}`; the compiler admits: {}. Source is never written directly", INTENT_KINDS.join(", ")),
        ));
    }
    Ok(Proposal {
        intent: intent.clone(),
        kind,
        claims: m.get("claims").cloned().unwrap_or(Value::Null),
    })
}

// ---- command view -------------------------------------------------------

/// Model-facing view of authoritative command output. The authoritative
/// result (status, verdict) is never replaced by the view.
pub trait CommandStage {
    fn id(&self) -> String;
    fn view(&mut self, label: &str, raw: &str, max_bytes: usize) -> String;
    /// Run one authorized check in `workdir`. `None`: this stage cannot run
    /// checks (the pipeline then records the check as unavailable).
    fn run_check(
        &mut self,
        _check: &super::checks::CheckSpec,
        _workdir: &std::path::Path,
        _observer: &mut crate::observe::Observer,
    ) -> Option<Result<super::checks::CheckRun, HarnessDiagnostic>> {
        None
    }
}

pub struct RawCommandView;

impl CommandStage for RawCommandView {
    fn id(&self) -> String {
        RAW_VIEW_ID.into()
    }
    fn view(&mut self, label: &str, raw: &str, max_bytes: usize) -> String {
        if raw.len() <= max_bytes {
            return raw.to_string();
        }
        let mut end = max_bytes;
        while !raw.is_char_boundary(end) {
            end -= 1;
        }
        format!(
            "{}\n[{label}: truncated {} of {} bytes]",
            &raw[..end],
            raw.len() - end,
            raw.len()
        )
    }
}
