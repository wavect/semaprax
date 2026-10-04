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

/// Task mode (`semaprax.harness-task.v2`). `Repair` is the legacy behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskMode {
    Repair,
    Change,
    /// Read-only inspect/plan: nothing is exported, checked in a candidate or published.
    Plan,
}

impl TaskMode {
    pub fn as_str(self) -> &'static str {
        match self {
            TaskMode::Repair => "repair",
            TaskMode::Change => "change",
            TaskMode::Plan => "plan",
        }
    }
}

pub const TASK_V1: &str = "semaprax.harness-task.v1";
pub const TASK_V2: &str = "semaprax.harness-task.v2";

#[derive(Clone, Debug, PartialEq)]
pub struct Task {
    pub goal: String,
    pub seed: Option<String>,
    pub family: String,
    pub external_context: ExternalContext,
    pub models: Option<Value>,
    /// 1 for `semaprax.harness-task.v1` (and no task), 2 for v2.
    pub schema_version: u8,
    pub mode: TaskMode,
    /// Acceptance criteria carried through every attempt: strings are
    /// informational; `{"stable_id","contains"}` objects are verified by the host
    /// against the compiler's own `context` output.
    pub acceptance: Vec<Value>,
    /// Operation (compiler candidate kind) the task expects, when known.
    pub operation: Option<String>,
    /// Names of authorized checks to run (`None`: all configured).
    pub checks: Option<Vec<String>>,
    pub budget: Option<super::budget::BudgetPolicy>,
    pub tokenizer_map: Option<super::budget::ModelTokenizerMap>,
    pub session: Option<super::session::SessionBounds>,
}

impl Default for Task {
    fn default() -> Self {
        Task {
            goal: "repair the compiler-reported failure".into(),
            seed: None,
            family: "localized_debug".into(),
            external_context: ExternalContext::WhenNeeded,
            models: None,
            schema_version: 1,
            mode: TaskMode::Repair,
            acceptance: vec![],
            operation: None,
            checks: None,
            budget: None,
            tokenizer_map: None,
            session: None,
        }
    }
}

const V1_MEMBERS: [&str; 6] = [
    "schema",
    "goal",
    "seed",
    "task_family",
    "external_context",
    "models",
];
const V2_MEMBERS: [&str; 7] = [
    "mode",
    "acceptance",
    "operation",
    "checks",
    "budget",
    "tokenizer_map",
    "session",
];

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
        let version = match m.get("schema").and_then(Value::as_str) {
            Some(TASK_V1) => 1,
            Some(TASK_V2) => 2,
            _ => {
                return Err(bad(format!(
                    "task schema must be `{TASK_V1}` or `{TASK_V2}`"
                )))
            }
        };
        for k in m.keys() {
            if !V1_MEMBERS.contains(&k.as_str())
                && !(version == 2 && V2_MEMBERS.contains(&k.as_str()))
            {
                return Err(bad(format!("unknown task member `{k}`")));
            }
        }
        let mut t = Task {
            schema_version: version,
            ..Task::default()
        };
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
        if version == 2 {
            t.parse_v2(m)?;
        }
        Ok(t)
    }

    fn parse_v2(&mut self, m: &Map<String, Value>) -> HarnessResult<()> {
        let bad = |m: String| d("SPX-HPD081", m);
        self.mode = match m.get("mode").and_then(Value::as_str) {
            None | Some("repair") => TaskMode::Repair,
            Some("change") => TaskMode::Change,
            Some("plan") | Some("inspect") => TaskMode::Plan,
            _ => return Err(bad("`mode` must be repair, change or plan".into())),
        };
        if self.mode != TaskMode::Repair && self.goal.trim().is_empty() {
            return Err(bad("a change or plan task needs a nonempty `goal`".into()));
        }
        if self.mode != TaskMode::Repair && !m.contains_key("goal") {
            return Err(bad("a change or plan task must state its `goal`".into()));
        }
        if let Some(a) = m.get("acceptance") {
            let arr = a
                .as_array()
                .filter(|a| a.len() <= 32)
                .ok_or_else(|| bad("`acceptance` must be an array of at most 32 items".into()))?;
            for x in arr {
                let ok = match x {
                    Value::String(s) => s.len() <= 1024,
                    Value::Object(o) => {
                        o.len() == 2
                            && o.get("stable_id").is_some_and(Value::is_string)
                            && o.get("contains").is_some_and(Value::is_string)
                    }
                    _ => false,
                };
                if !ok {
                    return Err(bad(
                        "`acceptance` items are strings or {stable_id, contains}".into(),
                    ));
                }
            }
            self.acceptance = arr.clone();
        }
        if let Some(o) = m.get("operation") {
            self.operation = Some(
                o.as_str()
                    .filter(|s| !s.is_empty() && s.len() <= 64)
                    .ok_or_else(|| bad("`operation` must be a candidate kind".into()))?
                    .into(),
            );
        }
        if let Some(c) = m.get("checks") {
            self.checks = Some(
                c.as_array()
                    .and_then(|a| a.iter().map(|x| x.as_str().map(str::to_string)).collect())
                    .ok_or_else(|| bad("`checks` must be an array of check names".into()))?,
            );
        }
        if let Some(b) = m.get("budget") {
            let o = b
                .as_object()
                .ok_or_else(|| bad("`budget` must be an object".into()))?;
            let mut p = super::budget::BudgetPolicy::default();
            for (k, v) in o {
                let n = v
                    .as_u64()
                    .ok_or_else(|| bad(format!("`budget.{k}` must be a number")))?;
                match k.as_str() {
                    "output_reserve_tokens" => p.output_reserve_tokens = n,
                    "protocol_overhead_tokens" => p.protocol_overhead_tokens = n,
                    "max_task_tokens" => p.max_task_tokens = Some(n),
                    "max_task_cost_micros" => p.max_task_cost_micros = Some(n),
                    _ => return Err(bad(format!("unknown budget member `{k}`"))),
                }
            }
            self.budget = Some(p);
        }
        if let Some(t) = m.get("tokenizer_map") {
            self.tokenizer_map = Some(super::budget::ModelTokenizerMap::from_json(t)?);
        }
        if let Some(s) = m.get("session") {
            self.session = Some(super::session::SessionBounds::from_json(s)?);
        }
        Ok(())
    }

    /// Digest of the task; the goal text itself never enters reports.
    pub fn digest(&self) -> String {
        let mut v = json!({"goal": self.goal, "seed": self.seed, "family": self.family,
                    "external_context": format!("{:?}", self.external_context), "models": self.models});
        if self.schema_version == 2 {
            v["v2"] = json!({"mode": self.mode.as_str(), "acceptance": self.acceptance, "operation": self.operation,
                "checks": self.checks, "budget": self.budget.as_ref().map(|b| format!("{b:?}")),
                "session": self.session.as_ref().map(|s| s.to_json())});
        }
        crate::json::digest(TASK_V1, &v)
    }

    /// Non-secret task summary for v2 reports (goal text is never included).
    pub fn summary_json(&self) -> Value {
        json!({"mode": self.mode.as_str(), "task_family": self.family,
               "goal_digest": crate::json::sha256_plain(self.goal.as_bytes()),
               "goal_bytes": self.goal.len(), "seed": self.seed,
               "acceptance_digest": crate::json::digest("semaprax.harness-acceptance.v1", &json!(self.acceptance)),
               "acceptance_items": self.acceptance.len(), "operation": self.operation})
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
    /// The proposer states the goal is complete (`done: true`, no intent).
    pub done: bool,
    /// The proposer states the goal needs an operation the compiler lacks.
    pub unsupported: Option<String>,
    /// Bounded scratch edit, accepted only for an unverified baseline (HN-02).
    pub source_patch: Option<Value>,
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
            "schema" | "intent" | "claims" | "summary" | "done" | "unsupported"
            | "source_patch" => {}
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
    let done = m.get("done").and_then(Value::as_bool).unwrap_or(false);
    let unsupported = m
        .get("unsupported")
        .map(|u| {
            u.as_str()
                .filter(|s| !s.is_empty() && s.len() <= 512)
                .map(str::to_string)
                .ok_or_else(|| d("SPX-HPD030", "`unsupported` must be a short reason string"))
        })
        .transpose()?;
    let source_patch = m.get("source_patch").cloned();
    let claims = m.get("claims").cloned().unwrap_or(Value::Null);
    let special = |kind: &str| Proposal {
        intent: Value::Null,
        kind: kind.into(),
        claims: claims.clone(),
        done,
        unsupported: unsupported.clone(),
        source_patch: source_patch.clone(),
    };
    if m.contains_key("intent") as u8
        + done as u8
        + unsupported.is_some() as u8
        + source_patch.is_some() as u8
        > 1
    {
        return Err(d(
            "SPX-HPD030",
            "a proposal carries exactly one of intent, done, unsupported, source_patch",
        ));
    }
    if unsupported.is_some() {
        return Ok(special("unsupported"));
    }
    if done {
        return Ok(special("done"));
    }
    if source_patch.is_some() {
        return Ok(special("source_patch"));
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
        claims,
        done: false,
        unsupported: None,
        source_patch: None,
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
