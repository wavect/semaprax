//! Common request/result envelopes. Results are untrusted data: they are bound
//! to the request, validated per capability kind, and may carry no authority.

use super::kind::{CapabilityKind, CapabilityRef};
use super::payload::{check_against_request, validate_payload, Direction};
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::json::{parse_frame, JsonLimits};
use serde_json::{json, Map, Value};

pub const REQUEST_SCHEMA: &str = "semaprax.harness-request.v1";
pub const RESULT_SCHEMA: &str = "semaprax.harness-result.v1";
const HOST_MAX_RESULT_BYTES: usize = 4 * 1024 * 1024;
const HOST_MAX_DEADLINE_MS: u64 = 600_000;

fn bad(code: &'static str, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectBinding {
    pub id: String,
    pub worktree: String,
    pub revision: String,
}

impl ProjectBinding {
    fn to_json(&self) -> Value {
        json!({"id": self.id, "worktree": self.worktree, "revision": self.revision})
    }

    fn from_json(v: &Value) -> HarnessResult<Self> {
        let m = exact(v, "project", &["id", "worktree", "revision"], &[])?;
        Ok(Self {
            id: text(m, "id")?,
            worktree: text(m, "worktree")?,
            revision: text(m, "revision")?,
        })
    }
}

fn exact<'a>(
    v: &'a Value,
    what: &str,
    required: &[&str],
    optional: &[&str],
) -> HarnessResult<&'a Map<String, Value>> {
    let m = v
        .as_object()
        .ok_or_else(|| bad("SPX-HPA037", format!("{what} must be an object")))?;
    for k in m.keys() {
        if !required.contains(&k.as_str()) && !optional.contains(&k.as_str()) {
            return Err(bad(
                "SPX-HPA037",
                format!("unexpected member `{k}` in {what}"),
            ));
        }
    }
    for r in required {
        if !m.contains_key(*r) {
            return Err(bad("SPX-HPA037", format!("{what} is missing `{r}`")));
        }
    }
    Ok(m)
}

fn text(m: &Map<String, Value>, k: &str) -> HarnessResult<String> {
    match m.get(k).and_then(Value::as_str) {
        Some(s) if !s.is_empty() && s.len() <= 256 && s.is_ascii() => Ok(s.to_string()),
        _ => Err(bad(
            "SPX-HPA037",
            format!("`{k}` must be a non-empty ASCII string of at most 256 bytes"),
        )),
    }
}

fn int(m: &Map<String, Value>, k: &str) -> HarnessResult<u64> {
    m.get(k).and_then(Value::as_u64).ok_or_else(|| {
        bad(
            "SPX-HPA037",
            format!("`{k}` must be a non-negative integer"),
        )
    })
}

fn cap_ref(v: &Value) -> HarnessResult<(String, u32)> {
    let m = exact(v, "capability", &["kind", "version"], &[])?;
    Ok((
        text(m, "kind")?,
        int(m, "version")?
            .try_into()
            .map_err(|_| bad("SPX-HPA037", "capability version out of range"))?,
    ))
}

fn parse_cap(v: &Value) -> HarnessResult<CapabilityRef> {
    let (name, version) = cap_ref(v)?;
    let kind = CapabilityKind::parse(&name)
        .ok_or_else(|| bad("SPX-HPA019", format!("unknown capability kind `{name}`")))?;
    Ok(CapabilityRef { kind, version })
}

fn cap_json(c: &CapabilityRef) -> Value {
    json!({"kind": c.kind.as_str(), "version": c.version})
}

#[derive(Clone, Debug, PartialEq)]
pub struct RequestEnvelope {
    pub invocation_id: String,
    pub project: ProjectBinding,
    pub lock_digest: String,
    pub capability: CapabilityRef,
    pub operation: String,
    pub deadline_ms: u64,
    pub max_result_bytes: usize,
    pub remaining_calls: u32,
    pub lineage: Vec<String>,
    pub payload: Value,
}

impl RequestEnvelope {
    pub fn to_json(&self) -> Value {
        json!({
            "schema": REQUEST_SCHEMA,
            "invocation_id": self.invocation_id,
            "project": self.project.to_json(),
            "lock_digest": self.lock_digest,
            "capability": cap_json(&self.capability),
            "operation": self.operation,
            "deadline_ms": self.deadline_ms,
            "budget": {"max_result_bytes": self.max_result_bytes, "remaining_calls": self.remaining_calls},
            "lineage": self.lineage,
            "payload": self.payload,
        })
    }

    /// Parse and [`validate`](Self::validate) a request document.
    pub fn from_json(v: &Value) -> HarnessResult<Self> {
        let m = exact(
            v,
            "request",
            &[
                "schema",
                "invocation_id",
                "project",
                "lock_digest",
                "capability",
                "operation",
                "deadline_ms",
                "budget",
                "lineage",
                "payload",
            ],
            &[],
        )?;
        if m["schema"].as_str() != Some(REQUEST_SCHEMA) {
            return Err(bad(
                "SPX-HPA030",
                format!("request schema must be `{REQUEST_SCHEMA}`"),
            ));
        }
        let b = exact(
            &m["budget"],
            "budget",
            &["max_result_bytes", "remaining_calls"],
            &[],
        )?;
        let lineage = m["lineage"]
            .as_array()
            .filter(|a| a.len() <= 64)
            .ok_or_else(|| bad("SPX-HPA037", "`lineage` must be an array"))?;
        let req = Self {
            invocation_id: text(m, "invocation_id")?,
            project: ProjectBinding::from_json(&m["project"])?,
            lock_digest: text(m, "lock_digest")?,
            capability: parse_cap(&m["capability"])?,
            operation: text(m, "operation")?,
            deadline_ms: int(m, "deadline_ms")?,
            max_result_bytes: int(b, "max_result_bytes")? as usize,
            remaining_calls: int(b, "remaining_calls")?
                .try_into()
                .map_err(|_| bad("SPX-HPA037", "`remaining_calls` out of range"))?,
            lineage: lineage
                .iter()
                .map(|x| {
                    x.as_str()
                        .filter(|s| !s.is_empty() && s.len() <= 256)
                        .map(str::to_string)
                })
                .collect::<Option<_>>()
                .ok_or_else(|| bad("SPX-HPA037", "`lineage` entries must be strings"))?,
            payload: m["payload"].clone(),
        };
        req.validate()?;
        Ok(req)
    }

    /// Envelope bounds, capability version/operation and the request payload.
    pub fn validate(&self) -> HarnessResult<()> {
        if !self
            .capability
            .kind
            .supported_versions()
            .contains(&self.capability.version)
        {
            return Err(bad(
                "SPX-HPA023",
                format!(
                    "{} v{} is not implemented by the host",
                    self.capability.kind.as_str(),
                    self.capability.version
                ),
            ));
        }
        if self.deadline_ms == 0 || self.deadline_ms > HOST_MAX_DEADLINE_MS {
            return Err(bad("SPX-HPA030", "deadline_ms must be within 1..=600000"));
        }
        if self.max_result_bytes == 0 || self.max_result_bytes > HOST_MAX_RESULT_BYTES {
            return Err(bad(
                "SPX-HPA030",
                "max_result_bytes must be within 1..=4194304",
            ));
        }
        for (what, s) in [
            ("invocation_id", &self.invocation_id),
            ("project.id", &self.project.id),
            ("project.worktree", &self.project.worktree),
            ("project.revision", &self.project.revision),
            ("lock_digest", &self.lock_digest),
        ] {
            if s.is_empty() || s.len() > 256 {
                return Err(bad(
                    "SPX-HPA030",
                    format!("{what} must be a non-empty identifier"),
                ));
            }
        }
        validate_payload(
            self.capability.kind,
            &self.operation,
            Direction::Request,
            &self.payload,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResultStatus {
    Complete,
    Partial,
    Stale,
    Unavailable,
    Unsupported,
    Refused,
    Failed,
}

impl ResultStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial => "partial",
            Self::Stale => "stale",
            Self::Unavailable => "unavailable",
            Self::Unsupported => "unsupported",
            Self::Refused => "refused",
            Self::Failed => "failed",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        [
            Self::Complete,
            Self::Partial,
            Self::Stale,
            Self::Unavailable,
            Self::Unsupported,
            Self::Refused,
            Self::Failed,
        ]
        .into_iter()
        .find(|x| x.as_str() == s)
    }

    fn carries_payload(&self) -> bool {
        matches!(self, Self::Complete | Self::Partial)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Provenance {
    pub provider_id: String,
    pub adapter_version: String,
    pub upstream_version: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResultEnvelope {
    pub invocation_id: String,
    pub project: ProjectBinding,
    pub capability: CapabilityRef,
    pub status: ResultStatus,
    pub payload: Option<Value>,
    pub diagnostics: Vec<(String, String)>,
    pub provenance: Provenance,
}

/// Member names that would read as an authority grant inside a result.
const AUTHORITY_KEYS: &[&str] = &[
    "grant",
    "grants",
    "permission",
    "permissions",
    "authority",
    "authorize",
    "authorized",
    "approve",
    "approved",
    "trust",
    "trusted",
    "publish",
    "execute",
    "allow",
];

fn find_authority(v: &Value) -> Option<String> {
    match v {
        Value::Object(m) => m.iter().find_map(|(k, x)| {
            if AUTHORITY_KEYS.contains(&k.to_ascii_lowercase().as_str()) {
                Some(k.clone())
            } else {
                find_authority(x)
            }
        }),
        Value::Array(a) => a.iter().find_map(find_authority),
        _ => None,
    }
}

impl ResultEnvelope {
    /// A well-formed `complete` result for `request`.
    pub fn complete(
        request: &RequestEnvelope,
        payload: Value,
        provider_id: &str,
        adapter_version: &str,
    ) -> Self {
        Self {
            invocation_id: request.invocation_id.clone(),
            project: request.project.clone(),
            capability: request.capability,
            status: ResultStatus::Complete,
            payload: Some(payload),
            diagnostics: Vec::new(),
            provenance: Provenance {
                provider_id: provider_id.into(),
                adapter_version: adapter_version.into(),
                upstream_version: None,
            },
        }
    }

    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("schema".into(), json!(RESULT_SCHEMA));
        m.insert("invocation_id".into(), json!(self.invocation_id));
        m.insert("project".into(), self.project.to_json());
        m.insert("capability".into(), cap_json(&self.capability));
        m.insert("status".into(), json!(self.status.as_str()));
        if let Some(p) = &self.payload {
            m.insert("payload".into(), p.clone());
        }
        m.insert(
            "diagnostics".into(),
            Value::Array(
                self.diagnostics
                    .iter()
                    .map(|(c, t)| json!({"code": c, "message": t}))
                    .collect(),
            ),
        );
        m.insert("provenance".into(), json!({"provider_id": self.provenance.provider_id, "adapter_version": self.provenance.adapter_version, "upstream_version": self.provenance.upstream_version}));
        Value::Object(m)
    }

    /// Parse an adapter result frame for exactly `request`.
    ///
    /// Refusals: oversize/duplicate keys/etc. from the strict JSON layer,
    /// HPA036 authority-like members, HPA031/032/033 spoofed invocation id /
    /// project / capability, HPA035 status/payload mismatch, HPA037 malformed
    /// envelope, plus the payload validators' codes.
    pub fn parse_for(request: &RequestEnvelope, bytes: &[u8]) -> HarnessResult<ResultEnvelope> {
        let limit = request.max_result_bytes.min(HOST_MAX_RESULT_BYTES);
        let doc = parse_frame(bytes, &JsonLimits::frame(limit))?;
        if let Some(k) = find_authority(&doc) {
            return Err(bad("SPX-HPA036", format!("result carries authority-like member `{k}`; results are data and grant nothing")));
        }
        let m = exact(
            &doc,
            "result",
            &[
                "schema",
                "invocation_id",
                "project",
                "capability",
                "status",
                "diagnostics",
                "provenance",
            ],
            &["payload"],
        )?;
        if m["schema"].as_str() != Some(RESULT_SCHEMA) {
            return Err(bad(
                "SPX-HPA037",
                format!("result schema must be `{RESULT_SCHEMA}`"),
            ));
        }
        let invocation_id = text(m, "invocation_id")?;
        if invocation_id != request.invocation_id {
            return Err(bad(
                "SPX-HPA031",
                format!("result invocation id `{invocation_id}` does not match the request"),
            ));
        }
        let project = ProjectBinding::from_json(&m["project"])?;
        if project != request.project {
            return Err(bad(
                "SPX-HPA032",
                "result project binding does not match the request",
            ));
        }
        let (kind_name, version) = cap_ref(&m["capability"])?;
        if kind_name != request.capability.kind.as_str() || version != request.capability.version {
            return Err(bad(
                "SPX-HPA033",
                format!("result capability `{kind_name}` v{version} does not match the request"),
            ));
        }
        let status = m["status"]
            .as_str()
            .and_then(ResultStatus::parse)
            .ok_or_else(|| bad("SPX-HPA037", "unknown result status"))?;
        let payload = match m.get("payload") {
            None | Some(Value::Null) => None,
            Some(p) => Some(p.clone()),
        };
        match (&payload, status.carries_payload()) {
            (Some(p), true) => {
                validate_payload(
                    request.capability.kind,
                    &request.operation,
                    Direction::Result,
                    p,
                )?;
                check_against_request(request.capability.kind, &request.payload, p)?;
            }
            (None, true) => {
                return Err(bad(
                    "SPX-HPA035",
                    format!("a `{}` result requires a payload", status.as_str()),
                ))
            }
            (Some(_), false) => {
                return Err(bad(
                    "SPX-HPA035",
                    format!("a `{}` result must not carry a payload", status.as_str()),
                ))
            }
            (None, false) => {}
        }
        let diags = m["diagnostics"]
            .as_array()
            .filter(|a| a.len() <= 64)
            .ok_or_else(|| bad("SPX-HPA037", "`diagnostics` must be an array of at most 64"))?;
        let mut diagnostics = Vec::new();
        for d in diags {
            let d = exact(d, "diagnostic", &["code", "message"], &[])?;
            let (c, t) = (d["code"].as_str(), d["message"].as_str());
            match (c, t) {
                (Some(c), Some(t)) if !c.is_empty() && c.len() <= 64 && t.len() <= 4096 => {
                    diagnostics.push((c.to_string(), t.to_string()))
                }
                _ => return Err(bad("SPX-HPA037", "diagnostic code/message malformed")),
            }
        }
        let pv = exact(
            &m["provenance"],
            "provenance",
            &["provider_id", "adapter_version"],
            &["upstream_version"],
        )?;
        let upstream_version = match pv.get("upstream_version") {
            None | Some(Value::Null) => None,
            Some(_) => Some(text(pv, "upstream_version")?),
        };
        Ok(ResultEnvelope {
            invocation_id,
            project,
            capability: request.capability,
            status,
            payload,
            diagnostics,
            provenance: Provenance {
                provider_id: text(pv, "provider_id")?,
                adapter_version: text(pv, "adapter_version")?,
                upstream_version,
            },
        })
    }
}
