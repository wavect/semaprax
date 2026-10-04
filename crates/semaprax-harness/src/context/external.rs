//! External repository providers. A source answers one capability-shaped
//! query for one worktree snapshot; `HostExternal` runs a trusted adapter
//! through the host, tests may substitute any `ExternalSource`.

use super::identity::Snapshot;
use super::item::{Span, Tier};
use crate::cli::Environment;
use crate::contract::{
    CapabilityKind, CapabilityRef, ProjectBinding, RequestEnvelope, ResultStatus, Runtime,
};
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::host::{
    AdapterManager, CancelToken, HostConfig, InvocationClass, IsolationRequest, LaunchSpec, Outcome,
};
use crate::json::{digest, sha256_labeled};
use crate::profile::ResolvedLaunch;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};

fn d(code: &'static str, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

/// Everything that makes one provider's answer different from another's: part
/// of every cache key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderIdentity {
    pub provider_id: String,
    pub provider_version: String,
    pub adapter_version: String,
    pub upstream_version: Option<String>,
    pub descriptor_digest: String,
    pub config_digest: String,
    /// Granted permissions and configured scope prefixes.
    pub permission_scope: Value,
}

impl ProviderIdentity {
    pub fn to_json(&self) -> Value {
        json!({"provider_id": self.provider_id, "provider_version": self.provider_version,
               "adapter_version": self.adapter_version, "upstream_version": self.upstream_version,
               "descriptor_digest": self.descriptor_digest, "config_digest": self.config_digest,
               "permission_scope": self.permission_scope})
    }
    pub fn digest(&self) -> String {
        digest(
            "semaprax.harness-context.provider-identity.v1",
            &self.to_json(),
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExternalQuery {
    pub op: String,
    pub payload: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RawItem {
    pub path: String,
    pub span: Span,
    pub digest: String,
    pub tier: Tier,
    pub language: String,
    pub rank: f64,
    pub text: Option<String>,
    /// Optional: whether the span is the whole definition or only its first line.
    pub span_kind: Option<String>,
    pub edges: Vec<RawEdge>,
}

/// Provider-reported relationship; `resolution` is the provider's own claim
/// about whether the target was resolved (`resolved|ambiguous|unsupported`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawEdge {
    pub target: String,
    pub relation: String,
    pub tier: Tier,
    pub resolution: Option<String>,
}

fn edges_json(edges: &[RawEdge]) -> Value {
    Value::Array(
        edges
            .iter()
            .map(|e| {
                let mut m = json!({"target": e.target, "relation": e.relation, "provenance": e.tier.as_str()});
                if let Some(r) = &e.resolution {
                    m["resolution"] = json!(r);
                }
                m
            })
            .collect(),
    )
}

fn edges_from(v: &Value) -> Option<Vec<RawEdge>> {
    let Some(a) = v.as_array() else {
        return Some(vec![]);
    };
    a.iter()
        .map(|e| {
            Some(RawEdge {
                target: e["target"].as_str()?.to_string(),
                relation: e["relation"].as_str()?.to_string(),
                tier: Tier::parse(e["provenance"].as_str()?)?,
                resolution: e["resolution"].as_str().map(str::to_string),
            })
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Coverage {
    pub complete: bool,
    pub exhaustive: bool,
    pub indexed_files: u64,
    pub skipped: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExternalResponse {
    /// `complete`, `partial`, or the non-answer status name.
    pub status: String,
    pub items: Vec<RawItem>,
    pub coverage: Coverage,
    pub no_references: bool,
    pub upstream_version: Option<String>,
    pub provider_id: String,
    pub diagnostics: Vec<(String, String)>,
}

impl ExternalResponse {
    pub fn to_json(&self) -> Value {
        let items: Vec<Value> = self
            .items
            .iter()
            .map(|i| {
                let mut m = json!({"path": i.path, "span": {"start_line": i.span.start_line, "end_line": i.span.end_line},
                    "digest": i.digest, "provenance": i.tier.as_str(), "language": i.language, "rank": i.rank});
                if let Some(t) = &i.text {
                    m["text"] = json!(t);
                }
                if let Some(k) = &i.span_kind {
                    m["span_kind"] = json!(k);
                }
                if !i.edges.is_empty() {
                    m["edges"] = edges_json(&i.edges);
                }
                m
            })
            .collect();
        let skipped: Vec<Value> = self
            .coverage
            .skipped
            .iter()
            .map(|(p, r)| json!({"path": p, "reason": r}))
            .collect();
        json!({"status": self.status, "items": items, "no_references": self.no_references,
               "coverage": {"complete": self.coverage.complete, "exhaustive": self.coverage.exhaustive,
                            "indexed_files": self.coverage.indexed_files, "skipped": skipped},
               "upstream_version": self.upstream_version, "provider_id": self.provider_id,
               "diagnostics": self.diagnostics.iter().map(|(c, m)| json!({"code": c, "message": m})).collect::<Vec<_>>()})
    }

    /// Rebuild from a cache file; `None` on any shape drift (treated as a miss).
    pub fn from_json(v: &Value) -> Option<Self> {
        let items = v["items"]
            .as_array()?
            .iter()
            .map(|i| {
                Some(RawItem {
                    path: i["path"].as_str()?.to_string(),
                    span: Span {
                        start_line: i["span"]["start_line"].as_u64()?,
                        end_line: i["span"]["end_line"].as_u64()?,
                    },
                    digest: i["digest"].as_str()?.to_string(),
                    tier: Tier::parse(i["provenance"].as_str()?)?,
                    language: i["language"].as_str()?.to_string(),
                    rank: i["rank"].as_f64()?,
                    text: i["text"].as_str().map(str::to_string),
                    span_kind: i["span_kind"].as_str().map(str::to_string),
                    edges: edges_from(&i["edges"])?,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        let c = &v["coverage"];
        let skipped = c["skipped"]
            .as_array()?
            .iter()
            .map(|s| {
                Some((
                    s["path"].as_str()?.to_string(),
                    s["reason"].as_str()?.to_string(),
                ))
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Self {
            status: v["status"].as_str()?.to_string(),
            items,
            coverage: Coverage {
                complete: c["complete"].as_bool()?,
                exhaustive: c["exhaustive"].as_bool()?,
                indexed_files: c["indexed_files"].as_u64()?,
                skipped,
            },
            no_references: v["no_references"].as_bool()?,
            upstream_version: v["upstream_version"].as_str().map(str::to_string),
            provider_id: v["provider_id"].as_str()?.to_string(),
            diagnostics: v["diagnostics"]
                .as_array()?
                .iter()
                .map(|x| {
                    Some((
                        x["code"].as_str()?.to_string(),
                        x["message"].as_str()?.to_string(),
                    ))
                })
                .collect::<Option<Vec<_>>>()?,
        })
    }
}

pub trait ExternalSource {
    fn identity(&self) -> ProviderIdentity;
    /// Project-relative prefixes this provider covers (empty = whole project).
    fn scope(&self) -> Vec<String>;
    /// Re-verify the user's grant now. Called on every read, cache hits included.
    fn recheck_authority(&self) -> HarnessResult<()>;
    fn query(
        &self,
        snap: &Snapshot,
        q: &ExternalQuery,
        max_result_bytes: usize,
    ) -> HarnessResult<ExternalResponse>;
}

/// Adapter-backed provider: grant rechecked through the profile trust store,
/// invocations run by the host as `SafeRead`.
pub struct HostExternal {
    launch: ResolvedLaunch,
    env: Environment,
    manager: AdapterManager,
    lock_digest: String,
    config_digest: String,
    scope: Vec<String>,
    counter: AtomicU64,
}

impl HostExternal {
    pub fn new(
        launch: ResolvedLaunch,
        env: Environment,
        lock_digest: String,
        config_digest: String,
        scope: Vec<String>,
    ) -> Self {
        Self {
            launch,
            env,
            manager: AdapterManager::new(HostConfig::default()),
            lock_digest,
            config_digest,
            scope,
            counter: AtomicU64::new(0),
        }
    }

    fn spec(&self, snap: &Snapshot) -> HarnessResult<LaunchSpec> {
        let home = self.env.harness_home.clone().ok_or_else(|| {
            d(
                "SPX-HPE030",
                "no harness home: cannot place the provider cache",
            )
        })?;
        let desc = &self.launch.descriptor;
        // The runtime recorded at adoption wins; `HARNESS_*` is the fallback.
        let runtime_executable = self.launch.runtime.clone().or_else(|| {
            match desc.runtime {
                Runtime::Python => self.env.vars.get("HARNESS_PYTHON"),
                Runtime::Node => self.env.vars.get("HARNESS_NODE"),
                _ => None,
            }
            .map(std::path::PathBuf::from)
        });
        let tag = format!(
            "{}-{}",
            desc.provider_id.replace('/', "_"),
            &snap.worktree_id[7..23]
        );
        Ok(LaunchSpec {
            descriptor: desc.clone(),
            descriptor_dir: self
                .launch
                .descriptor_path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_default(),
            runtime_executable,
            upstream_executable: self.launch.upstream_path.clone(),
            grant: self.launch.grant.clone(),
            project_root: snap.root.clone(),
            cache_dir: home.join("cache").join("adapters").join(tag.clone()),
            retention_dir: home.join("retention").join(tag),
            isolation: IsolationRequest::None,
            forward_env: Default::default(),
        })
    }
}

impl ExternalSource for HostExternal {
    fn identity(&self) -> ProviderIdentity {
        let desc = &self.launch.descriptor;
        let g = self.launch.grant.permissions();
        ProviderIdentity {
            provider_id: desc.provider_id.clone(),
            provider_version: desc.provider_version.clone(),
            adapter_version: desc.adapter_version.clone(),
            upstream_version: desc
                .upstream
                .as_ref()
                .filter(|u| u.versions.len() == 1)
                .map(|u| u.versions[0].clone()),
            descriptor_digest: desc.digest().to_string(),
            config_digest: self.config_digest.clone(),
            permission_scope: json!({"read": g.read, "write": g.write, "network": g.network, "process": g.process,
                                     "secrets": g.secrets, "scope": self.scope, "lock": self.lock_digest,
                                     "upstream": self.launch.grant.upstream_digest(), "entry": self.launch.grant.entry_digest()}),
        }
    }

    fn scope(&self) -> Vec<String> {
        self.scope.clone()
    }

    fn recheck_authority(&self) -> HarnessResult<()> {
        crate::profile::check_grant_current(&self.env, &self.launch.grant)
    }

    fn query(
        &self,
        snap: &Snapshot,
        q: &ExternalQuery,
        max_result_bytes: usize,
    ) -> HarnessResult<ExternalResponse> {
        self.recheck_authority()?;
        let handle = self.manager.prepare(&snap.project_id, self.spec(snap)?)?;
        let n = self.counter.fetch_add(1, Ordering::SeqCst) + 1;
        let req = RequestEnvelope {
            invocation_id: format!("inv-ctx-{n:06}"),
            project: ProjectBinding {
                id: snap.project_id.clone(),
                worktree: snap.worktree_id.clone(),
                revision: snap.revision.clone(),
            },
            lock_digest: self.lock_digest.clone(),
            capability: CapabilityRef {
                kind: CapabilityKind::ContextRepository,
                version: 1,
            },
            operation: q.op.clone(),
            deadline_ms: 120_000,
            max_result_bytes: max_result_bytes.clamp(1, 4 * 1024 * 1024),
            remaining_calls: 8,
            lineage: vec![],
            payload: q.payload.clone(),
        };
        match handle.invoke(&req, InvocationClass::SafeRead, &CancelToken::new()) {
            Outcome::Completed(r) => {
                let status = r.status.as_str().to_string();
                let diagnostics = r.diagnostics.clone();
                let (items, coverage, no_refs) = match (&r.payload, r.status) {
                    (Some(p), ResultStatus::Complete | ResultStatus::Partial) => parse_payload(p)?,
                    _ => (vec![], Coverage::default(), false),
                };
                Ok(ExternalResponse {
                    status,
                    items,
                    coverage,
                    no_references: no_refs,
                    upstream_version: r.provenance.upstream_version.clone(),
                    provider_id: r.provenance.provider_id.clone(),
                    diagnostics,
                })
            }
            Outcome::Refused(e) | Outcome::Quarantined(e) | Outcome::Uncertain(e) => Err(e),
            Outcome::Unavailable { reason, .. } => Err(reason),
            Outcome::Cancelled => Err(d("SPX-HPE031", "provider invocation cancelled")),
        }
    }
}

/// Parse an already-validated `context.repository/v1` result payload.
fn parse_payload(p: &Value) -> HarnessResult<(Vec<RawItem>, Coverage, bool)> {
    let bad = || {
        d(
            "SPX-HPE032",
            "validated context payload had an unexpected shape",
        )
    };
    let mut items = Vec::new();
    for i in p["items"].as_array().ok_or_else(bad)? {
        items.push(RawItem {
            path: i["path"].as_str().ok_or_else(bad)?.to_string(),
            span: Span {
                start_line: i["span"]["start_line"].as_u64().ok_or_else(bad)?,
                end_line: i["span"]["end_line"].as_u64().ok_or_else(bad)?,
            },
            digest: i["digest"].as_str().ok_or_else(bad)?.to_string(),
            tier: Tier::parse(i["provenance"].as_str().ok_or_else(bad)?).ok_or_else(bad)?,
            language: i["language"].as_str().ok_or_else(bad)?.to_string(),
            rank: i["rank"].as_f64().ok_or_else(bad)?,
            text: i["text"].as_str().map(str::to_string),
            span_kind: i["span_kind"].as_str().map(str::to_string),
            edges: edges_from(&i["edges"]).ok_or_else(bad)?,
        });
    }
    let c = &p["coverage"];
    let skipped = c["skipped"]
        .as_array()
        .ok_or_else(bad)?
        .iter()
        .map(|s| {
            Some((
                s["path"].as_str()?.to_string(),
                s["reason"].as_str()?.to_string(),
            ))
        })
        .collect::<Option<Vec<_>>>()
        .ok_or_else(bad)?;
    // Extraction errors are files the provider failed on: report them as skipped.
    let mut skipped = skipped;
    for x in c["extraction_errors"].as_array().into_iter().flatten() {
        if let (Some(p), Some(r)) = (x["path"].as_str(), x["reason"].as_str()) {
            skipped.push((p.to_string(), format!("extraction error: {r}")));
        }
    }
    let cov = Coverage {
        complete: c["complete"].as_bool().ok_or_else(bad)?,
        exhaustive: c["exhaustive"].as_bool().ok_or_else(bad)?,
        indexed_files: c["indexed_files"].as_u64().ok_or_else(bad)?,
        skipped,
    };
    Ok((items, cov, p["no_references"].as_bool().unwrap_or(false)))
}

/// Stable short digest used to name per-provider directories.
pub fn short(s: &str) -> String {
    sha256_labeled("semaprax.harness-context.short.v1", s.as_bytes())[7..23].to_string()
}
