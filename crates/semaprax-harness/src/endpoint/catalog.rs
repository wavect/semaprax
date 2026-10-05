//! Machine-local endpoint catalog (`<harness_home>/endpoints.json`), logical
//! model bindings and their revalidation. No secrets are stored: credentials
//! are environment variable names only.

use super::ownership::{
    check_policy, AttemptOwnership, EndpointPolicy, GatewayFallbacks, GatewayRetries,
};
use super::types::*;
use crate::decision::{Destination, ModelPlan};
use crate::diag::HarnessResult;
use crate::json::{self, JsonLimits};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const CATALOG_SCHEMA: &str = "semaprax.harness-endpoints.v1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogModel {
    pub name: String,
    pub identity: ModelIdentity,
    pub context_length: Option<u64>,
    pub remote_host: Option<String>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EndpointRecord {
    pub id: String,
    pub kind: EndpointKind,
    pub url: String,
    pub credential_env: Option<String>,
    pub probe_model: String,
    pub returned_model: Option<String>,
    pub models: Vec<CatalogModel>,
    pub probes: BTreeMap<String, ProtocolVerdict>,
    pub ownership: AttemptOwnership,
    pub destinations: Vec<Destination>,
    pub disclosed: bool,
}

impl EndpointRecord {
    pub fn model(&self, name: &str) -> Option<&CatalogModel> {
        self.models.iter().find(|m| m.name == name)
    }

    pub fn verdict(&self, key: &str) -> Verdict {
        self.probes
            .get(key)
            .map_or(Verdict::Unverified, |p| p.verdict)
    }

    /// Digest over the model names and identities; any catalog change moves it.
    pub fn catalog_digest(&self) -> String {
        let list: Vec<Value> = self
            .models
            .iter()
            .map(|m| json!({"name": m.name, "identity": m.identity.to_json()}))
            .collect();
        json::digest("semaprax.harness-endpoint-catalog.v1", &Value::Array(list))
    }

    /// Destination of one model. Loopback is not assumed local inference: it
    /// comes from the direct server's own record or the operator disclosure,
    /// else remote-unknown.
    pub fn destination_for(&self, model: &str) -> Destination {
        if let Some(h) = self.model(model).and_then(|m| m.remote_host.clone()) {
            return Destination::Remote { origin: h };
        }
        if self.kind == EndpointKind::Ollama && !self.disclosed {
            return Destination::Local;
        }
        let mut all = self.destinations.clone();
        if let GatewayFallbacks::Disclosed(l) = &self.ownership.gateway_fallbacks {
            all.extend(l.iter().map(|(_, d)| d.clone()));
        }
        if all.is_empty() {
            return remote_unknown();
        }
        all.into_iter()
            .find(|d| *d != Destination::Local)
            .unwrap_or(Destination::Local)
    }

    pub fn to_json(&self) -> Value {
        let models: Vec<Value> = self.models.iter().map(|m| json!({
            "name": m.name, "identity": m.identity.to_json(), "context_length": m.context_length,
            "remote_host": m.remote_host, "notes": m.notes,
        })).collect();
        let probes: Map<String, Value> = self
            .probes
            .iter()
            .map(|(k, v)| (k.clone(), v.to_json()))
            .collect();
        json!({
            "id": self.id, "kind": self.kind.as_str(), "url": self.url, "credential_env": self.credential_env,
            "probe_model": self.probe_model, "returned_model": self.returned_model, "models": models,
            "probes": probes, "ownership": self.ownership.to_json(),
            "destinations": self.destinations.iter().map(destination_to_json).collect::<Vec<_>>(),
            "disclosed": self.disclosed, "catalog_digest": self.catalog_digest(),
        })
    }

    pub fn from_json(v: &Value) -> Option<Self> {
        let models = v
            .get("models")?
            .as_array()?
            .iter()
            .map(|m| {
                Some(CatalogModel {
                    name: m.get("name")?.as_str()?.to_string(),
                    identity: ModelIdentity::from_json(m.get("identity")?)?,
                    context_length: m.get("context_length").and_then(Value::as_u64),
                    remote_host: m
                        .get("remote_host")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    notes: m
                        .get("notes")
                        .and_then(Value::as_array)
                        .map(|a| {
                            a.iter()
                                .filter_map(|s| s.as_str().map(str::to_string))
                                .collect()
                        })
                        .unwrap_or_default(),
                })
            })
            .collect::<Option<Vec<_>>>()?;
        let probes = v
            .get("probes")?
            .as_object()?
            .iter()
            .map(|(k, p)| Some((k.clone(), ProtocolVerdict::from_json(p)?)))
            .collect::<Option<BTreeMap<_, _>>>()?;
        Some(Self {
            id: v.get("id")?.as_str()?.to_string(),
            kind: EndpointKind::parse(v.get("kind")?.as_str()?).ok()?,
            url: v.get("url")?.as_str()?.to_string(),
            credential_env: v
                .get("credential_env")
                .and_then(Value::as_str)
                .map(str::to_string),
            probe_model: v.get("probe_model")?.as_str()?.to_string(),
            returned_model: v
                .get("returned_model")
                .and_then(Value::as_str)
                .map(str::to_string),
            models,
            probes,
            ownership: AttemptOwnership::from_json(v.get("ownership")?)?,
            destinations: v
                .get("destinations")?
                .as_array()?
                .iter()
                .map(destination_from_json)
                .collect::<Option<Vec<_>>>()?,
            disclosed: v.get("disclosed")?.as_bool()?,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttemptOwner {
    /// The gateway makes at most one upstream attempt; Semaprax owns retry/failover.
    Semaprax,
    /// The gateway may make further attempts (bounded or not).
    Gateway,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Capabilities {
    pub tools: bool,
    pub structured_output: bool,
    pub streaming: bool,
    pub usage_reporting: bool,
}

/// A logical model bound to one approved endpoint/model/protocol entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogicalModel {
    pub id: String,
    pub endpoint_id: String,
    pub upstream_model: String,
    pub protocol: Protocol,
    pub capabilities: Capabilities,
    pub observed_model_identity: ModelIdentity,
    pub observed_returned_model: Option<String>,
    pub observed_catalog_digest: String,
    pub destination: Destination,
    pub attempt_owner: AttemptOwner,
    pub max_context: u64,
    pub strength_rank: u32,
}

/// Why a binding is (in)valid after a re-probe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BindingStatus {
    Valid,
    /// HPL030
    ModelMissing,
    /// HPL031
    IdentityChanged {
        was: ModelIdentity,
        now: ModelIdentity,
    },
    /// HPL032
    ProtocolNoLongerSupported {
        protocol: Protocol,
        now: Verdict,
    },
    /// HPL033
    CatalogChanged {
        was: String,
        now: String,
    },
    /// HPL034
    EndpointMissing,
}

impl BindingStatus {
    pub fn is_valid(&self) -> bool {
        *self == Self::Valid
    }
    pub fn code(&self) -> Option<&'static str> {
        match self {
            Self::Valid => None,
            Self::ModelMissing => Some("SPX-HPL030"),
            Self::IdentityChanged { .. } => Some("SPX-HPL031"),
            Self::ProtocolNoLongerSupported { .. } => Some("SPX-HPL032"),
            Self::CatalogChanged { .. } => Some("SPX-HPL033"),
            Self::EndpointMissing => Some("SPX-HPL034"),
        }
    }
    pub fn to_json(&self) -> Value {
        match self {
            Self::Valid => json!({"status": "valid"}),
            other => json!({"status": "invalid", "code": other.code(), "reason": match other {
                Self::ModelMissing => "model no longer in the endpoint catalog".to_string(),
                Self::IdentityChanged { was, now } => format!("identity changed: {} -> {}", ident_label(was), ident_label(now)),
                Self::ProtocolNoLongerSupported { protocol, now } => format!("protocol {} is now {}", protocol.as_str(), now.as_str()),
                Self::CatalogChanged { .. } => "catalog digest changed; re-bind to re-approve".to_string(),
                _ => "endpoint is no longer adopted".to_string(),
            }}),
        }
    }
}

fn ident_label(i: &ModelIdentity) -> String {
    match i {
        ModelIdentity::Digest(d) => d.clone(),
        ModelIdentity::Reported(r) => format!("reported:{r}"),
        ModelIdentity::Unknown => "unknown".into(),
    }
}

impl LogicalModel {
    /// Bind a logical id to an endpoint model. Refuses unless the protocol was
    /// actually observed `supported` and the project policy can be enforced.
    pub fn bind(
        id: &str,
        endpoint: &EndpointRecord,
        upstream_model: &str,
        protocol: Protocol,
        policy: EndpointPolicy,
        strength_rank: u32,
    ) -> HarnessResult<Self> {
        let model = endpoint.model(upstream_model).ok_or_else(|| {
            err(
                "SPX-HPL030",
                format!(
                    "model `{upstream_model}` is not in endpoint `{}`",
                    endpoint.id
                ),
            )
        })?;
        let verdict = endpoint.verdict(protocol.probe_key());
        if verdict != Verdict::Supported {
            return Err(err(
                "SPX-HPL032",
                format!(
                    "protocol {} is {} on endpoint `{}`; no silent downgrade to another protocol",
                    protocol.as_str(),
                    verdict.as_str(),
                    endpoint.id
                ),
            ));
        }
        let destination = endpoint.destination_for(upstream_model);
        check_policy(policy, &endpoint.ownership, &destination)?;
        let streaming_key = match protocol {
            Protocol::Responses => "responses_streaming",
            Protocol::ChatCompletions => "chat_streaming",
            Protocol::AnthropicMessages => "",
        };
        let attempt_owner = match endpoint.ownership.max_upstream_attempts() {
            Some(1) => AttemptOwner::Semaprax,
            _ => AttemptOwner::Gateway,
        };
        let observed_returned_model = if upstream_model == endpoint.probe_model {
            endpoint.returned_model.clone()
        } else {
            None
        };
        Ok(Self {
            id: id.to_string(),
            endpoint_id: endpoint.id.clone(),
            upstream_model: upstream_model.to_string(),
            protocol,
            capabilities: Capabilities {
                tools: endpoint.verdict("tool_calls") == Verdict::Supported,
                structured_output: endpoint.verdict("structured_output") == Verdict::Supported,
                streaming: !streaming_key.is_empty()
                    && endpoint.verdict(streaming_key) == Verdict::Supported,
                usage_reporting: endpoint.verdict("usage") == Verdict::Supported,
            },
            observed_model_identity: model.identity.clone(),
            observed_returned_model,
            observed_catalog_digest: endpoint.catalog_digest(),
            destination,
            attempt_owner,
            max_context: model.context_length.unwrap_or(0),
            strength_rank,
        })
    }

    /// Decision-layer plan. The decision layer is transport-agnostic: the same
    /// plan results whether the endpoint is direct or behind a gateway.
    /// Local compute is not "free": cost is unknown and recorded as 0 micros
    /// only as an estimate input, never as measured usage.
    pub fn to_model_plan(&self, est_cost_micros: u64, est_latency_ms: u64) -> ModelPlan {
        ModelPlan {
            id: self.id.clone(),
            destination: self.destination.clone(),
            structured_output: self.capabilities.structured_output,
            tools: self.capabilities.tools,
            max_context: self.max_context,
            est_cost_micros,
            est_latency_ms,
            strength_rank: self.strength_rank,
            descriptor: Default::default(),
        }
    }

    /// Re-check against a freshly probed record.
    pub fn revalidate(&self, fresh: Option<&EndpointRecord>) -> BindingStatus {
        let Some(f) = fresh else {
            return BindingStatus::EndpointMissing;
        };
        let Some(m) = f.model(&self.upstream_model) else {
            return BindingStatus::ModelMissing;
        };
        if m.identity != self.observed_model_identity {
            return BindingStatus::IdentityChanged {
                was: self.observed_model_identity.clone(),
                now: m.identity.clone(),
            };
        }
        let v = f.verdict(self.protocol.probe_key());
        if v != Verdict::Supported {
            return BindingStatus::ProtocolNoLongerSupported {
                protocol: self.protocol,
                now: v,
            };
        }
        let now = f.catalog_digest();
        if now != self.observed_catalog_digest {
            return BindingStatus::CatalogChanged {
                was: self.observed_catalog_digest.clone(),
                now,
            };
        }
        BindingStatus::Valid
    }

    pub fn to_json(&self) -> Value {
        json!({
            "id": self.id, "endpoint_id": self.endpoint_id, "upstream_model": self.upstream_model,
            "protocol": self.protocol.as_str(),
            "capabilities": {"tools": self.capabilities.tools, "structured_output": self.capabilities.structured_output,
                "streaming": self.capabilities.streaming, "usage_reporting": self.capabilities.usage_reporting},
            "observed_model_identity": self.observed_model_identity.to_json(),
            "observed_returned_model": self.observed_returned_model,
            "observed_catalog_digest": self.observed_catalog_digest,
            "destination": destination_to_json(&self.destination),
            "attempt_owner": match self.attempt_owner { AttemptOwner::Semaprax => "semaprax", AttemptOwner::Gateway => "gateway" },
            "max_context": self.max_context, "strength_rank": self.strength_rank,
        })
    }

    pub fn from_json(v: &Value) -> Option<Self> {
        let c = v.get("capabilities")?;
        let b = |k: &str| c.get(k).and_then(Value::as_bool);
        Some(Self {
            id: v.get("id")?.as_str()?.to_string(),
            endpoint_id: v.get("endpoint_id")?.as_str()?.to_string(),
            upstream_model: v.get("upstream_model")?.as_str()?.to_string(),
            protocol: Protocol::parse(v.get("protocol")?.as_str()?).ok()?,
            capabilities: Capabilities {
                tools: b("tools")?,
                structured_output: b("structured_output")?,
                streaming: b("streaming")?,
                usage_reporting: b("usage_reporting")?,
            },
            observed_model_identity: ModelIdentity::from_json(v.get("observed_model_identity")?)?,
            observed_returned_model: v
                .get("observed_returned_model")
                .and_then(Value::as_str)
                .map(str::to_string),
            observed_catalog_digest: v.get("observed_catalog_digest")?.as_str()?.to_string(),
            destination: destination_from_json(v.get("destination")?)?,
            attempt_owner: if v.get("attempt_owner")?.as_str()? == "semaprax" {
                AttemptOwner::Semaprax
            } else {
                AttemptOwner::Gateway
            },
            max_context: v.get("max_context")?.as_u64()?,
            strength_rank: v.get("strength_rank")?.as_u64()? as u32,
        })
    }
}

/// The whole machine-local catalog.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Catalog {
    pub endpoints: BTreeMap<String, EndpointRecord>,
    pub bindings: BTreeMap<String, LogicalModel>,
}

impl Catalog {
    pub fn path(home: &Path) -> PathBuf {
        home.join("endpoints.json")
    }

    pub fn load(home: &Path) -> HarnessResult<Self> {
        let path = Self::path(home);
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => {
                return Err(err(
                    "SPX-HPL004",
                    format!("cannot read endpoint catalog: {e}"),
                ))
            }
        };
        let bad = || err("SPX-HPL004", "malformed endpoint catalog");
        let v = json::parse_strict(
            &bytes,
            &JsonLimits {
                max_bytes: 8 * 1024 * 1024,
                max_depth: 16,
                max_nodes: 200_000,
            },
        )
        .map_err(|_| bad())?;
        if v.get("schema").and_then(Value::as_str) != Some(CATALOG_SCHEMA) {
            return Err(bad());
        }
        let mut c = Self::default();
        for (k, e) in v
            .get("endpoints")
            .and_then(Value::as_object)
            .ok_or_else(bad)?
        {
            c.endpoints
                .insert(k.clone(), EndpointRecord::from_json(e).ok_or_else(bad)?);
        }
        for (k, b) in v
            .get("bindings")
            .and_then(Value::as_object)
            .ok_or_else(bad)?
        {
            c.bindings
                .insert(k.clone(), LogicalModel::from_json(b).ok_or_else(bad)?);
        }
        Ok(c)
    }

    pub fn to_json(&self) -> Value {
        json!({
            "schema": CATALOG_SCHEMA,
            "endpoints": self.endpoints.iter().map(|(k, v)| (k.clone(), v.to_json())).collect::<Map<_, _>>(),
            "bindings": self.bindings.iter().map(|(k, v)| (k.clone(), v.to_json())).collect::<Map<_, _>>(),
        })
    }

    /// Atomic write (temp file + rename).
    pub fn save(&self, home: &Path) -> HarnessResult<()> {
        std::fs::create_dir_all(home)
            .map_err(|e| err("SPX-HPL004", format!("cannot create harness home: {e}")))?;
        let path = Self::path(home);
        let tmp = home.join("endpoints.json.tmp");
        let text = format!("{}\n", json::canonical(&self.to_json()));
        std::fs::write(&tmp, text)
            .and_then(|_| std::fs::rename(&tmp, &path))
            .map_err(|e| err("SPX-HPL004", format!("cannot write endpoint catalog: {e}")))
    }

    /// Binding statuses against freshly probed records.
    pub fn revalidate(
        &self,
        fresh: &BTreeMap<String, EndpointRecord>,
    ) -> BTreeMap<String, BindingStatus> {
        self.bindings
            .iter()
            .map(|(k, b)| (k.clone(), b.revalidate(fresh.get(&b.endpoint_id))))
            .collect()
    }
}

/// True when the ownership leaves retry/failover undisclosed.
pub fn ownership_is_opaque(o: &AttemptOwnership) -> bool {
    o.gateway_retries == GatewayRetries::Undisclosed
        || o.gateway_fallbacks == GatewayFallbacks::Undisclosed
}
