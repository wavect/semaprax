//! Capability handshake: one owner per delegable capability. A capability the
//! host does not declare stays host-owned and is never claimed optimized.

use crate::diag::{HarnessDiagnostic, HarnessResult};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

pub const PROTOCOL: &str = "semaprax.harness-bridge.v1";
pub const HANDSHAKE_SCHEMA: &str = "semaprax.harness-bridge-handshake.v1";
/// Marker in the environment/lineage of a process started by a Semaprax bridge.
pub const DEPTH_VAR: &str = "SEMAPRAX_HARNESS_BRIDGE_DEPTH";

/// Capabilities a host may declare it can delegate, in canonical order.
pub const CAPABILITIES: [&str; 6] = [
    "semantic_query",
    "tool_result_observation",
    "command_wrapper",
    "model_routing",
    "cancellation",
    "publication",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Owner {
    Semaprax,
    ExternalHost,
    Unavailable,
}

impl Owner {
    pub fn as_str(self) -> &'static str {
        match self {
            Owner::Semaprax => "semaprax",
            Owner::ExternalHost => "external-host",
            Owner::Unavailable => "unavailable",
        }
    }
}

/// What the host says about itself.
#[derive(Clone, Debug, Default)]
pub struct HostDeclaration {
    pub name: String,
    pub version: String,
    pub declared: BTreeMap<String, bool>,
    /// An output/command rewriter the host already runs (for example an RTK hook).
    pub command_rewriter: Option<String>,
    pub depth: u64,
    pub lineage: Vec<String>,
}

/// What this side can actually serve for the project.
#[derive(Clone, Debug, Default)]
pub struct Availability {
    pub compiler: bool,
    pub command_view_enabled: bool,
}

fn bad(msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new("SPX-HPN001", msg)
}

fn str_field(m: &Map<String, Value>, k: &str) -> HarnessResult<String> {
    match m.get(k) {
        Some(Value::String(s)) if !s.is_empty() && s.len() <= 128 => Ok(s.clone()),
        _ => Err(bad(format!(
            "`{k}` must be a non-empty string of at most 128 bytes"
        ))),
    }
}

/// Strict, closed parse of `bridge/handshake` params.
pub fn parse_declaration(params: &Value) -> HarnessResult<HostDeclaration> {
    let m = params
        .as_object()
        .ok_or_else(|| bad("handshake params must be an object"))?;
    for k in m.keys() {
        if ![
            "protocol",
            "version",
            "host",
            "capabilities",
            "command_rewriter",
            "bridge_depth",
            "lineage",
        ]
        .contains(&k.as_str())
        {
            return Err(bad(format!("unknown handshake member `{k}`")));
        }
    }
    match m.get("protocol").and_then(Value::as_str) {
        Some(PROTOCOL) => {}
        other => {
            return Err(bad(format!(
                "incompatible protocol {:?}; this bridge speaks `{PROTOCOL}` version 1",
                other.unwrap_or("<missing>")
            )))
        }
    }
    if m.get("version").and_then(Value::as_u64) != Some(1) {
        return Err(bad(
            "incompatible protocol version; this bridge speaks version 1",
        ));
    }
    let host = m
        .get("host")
        .and_then(Value::as_object)
        .ok_or_else(|| bad("`host` must be an object {name, version}"))?;
    let mut d = HostDeclaration {
        name: str_field(host, "name")?,
        version: str_field(host, "version")?,
        ..Default::default()
    };
    let caps = m
        .get("capabilities")
        .and_then(Value::as_object)
        .ok_or_else(|| bad("`capabilities` must be an object of booleans"))?;
    for (k, v) in caps {
        if !CAPABILITIES.contains(&k.as_str()) {
            return Err(bad(format!("unknown capability `{k}`")));
        }
        d.declared.insert(
            k.clone(),
            v.as_bool()
                .ok_or_else(|| bad(format!("capability `{k}` must be a boolean")))?,
        );
    }
    match m.get("command_rewriter") {
        None | Some(Value::Null) => {}
        Some(Value::String(s)) if !s.is_empty() && s.len() <= 128 => {
            d.command_rewriter = Some(s.clone())
        }
        _ => return Err(bad("`command_rewriter` must be null or a short string")),
    }
    if let Some(v) = m.get("bridge_depth") {
        d.depth = v
            .as_u64()
            .ok_or_else(|| bad("`bridge_depth` must be a non-negative integer"))?;
    }
    if let Some(v) = m.get("lineage") {
        let a = v
            .as_array()
            .ok_or_else(|| bad("`lineage` must be an array of strings"))?;
        for x in a {
            d.lineage.push(
                x.as_str()
                    .ok_or_else(|| bad("`lineage` must be an array of strings"))?
                    .to_string(),
            );
        }
    }
    Ok(d)
}

/// Negotiated outcome for one capability.
#[derive(Clone, Debug)]
pub struct Decision {
    pub owner: Owner,
    pub reason: String,
}

fn d(owner: Owner, reason: &str) -> Decision {
    Decision {
        owner,
        reason: reason.to_string(),
    }
}

pub fn decide(decl: &HostDeclaration, avail: &Availability) -> BTreeMap<&'static str, Decision> {
    let has = |k: &str| decl.declared.get(k).copied().unwrap_or(false);
    let mut out = BTreeMap::new();
    out.insert(
        "semantic_query",
        if !has("semantic_query") {
            d(
                Owner::ExternalHost,
                "host did not declare semantic-query delegation; host-owned, not optimized",
            )
        } else if avail.compiler {
            d(
                Owner::Semaprax,
                "host delegates; answered by the context broker (`bridge/context`)",
            )
        } else {
            d(
                Owner::Unavailable,
                "host delegates but no compiler executable is configured",
            )
        },
    );
    out.insert(
        "tool_result_observation",
        if has("tool_result_observation") {
            d(
                Owner::Semaprax,
                "observes only calls routed through the bridge; not host-wide tool traffic",
            )
        } else {
            d(
                Owner::ExternalHost,
                "host did not declare tool-result observation; its other tool calls are unobserved",
            )
        },
    );
    out.insert(
        "command_wrapper",
        if let Some(r) = &decl.command_rewriter {
            Decision {
                owner: Owner::ExternalHost,
                reason: format!(
                    "host already runs command rewriter `{r}`; Semaprax does not wrap again"
                ),
            }
        } else if !has("command_wrapper") {
            d(
                Owner::ExternalHost,
                "host did not declare command-wrapper delegation; host-owned, not optimized",
            )
        } else if !avail.command_view_enabled {
            d(
                Owner::Unavailable,
                "command.view is disabled by the project profile",
            )
        } else {
            d(
                Owner::Semaprax,
                "host delegates; commands run once through command_view (`bridge/command_view`)",
            )
        },
    );
    out.insert(
        "model_routing",
        if has("model_routing") {
            d(Owner::Semaprax, "host delegates model choice; advice comes only from the profile's decision binding")
        } else {
            d(Owner::ExternalHost, "host-controlled: the host does not delegate model choice; no Jev/Laya routing is applied or advertised")
        },
    );
    out.insert(
        "cancellation",
        if has("cancellation") {
            d(
                Owner::Semaprax,
                "host forwards `bridge/cancel`; the bridge serves one request at a time",
            )
        } else {
            d(
                Owner::ExternalHost,
                "host did not declare cancellation forwarding; the host cancels its own calls",
            )
        },
    );
    out.insert(
        "publication",
        d(Owner::ExternalHost, "publication stays with the host-authorized compiler route; `bridge/publish` is always refused"),
    );
    out
}

/// Handshake response document.
pub fn response(decl: &HostDeclaration, avail: &Availability) -> Value {
    let dec = decide(decl, avail);
    let mut caps = Map::new();
    let mut not_optimized = Vec::new();
    for (k, v) in &dec {
        caps.insert(
            k.to_string(),
            json!({"owner": v.owner.as_str(), "reason": v.reason}),
        );
        if v.owner != Owner::Semaprax {
            not_optimized.push(json!(k));
        }
    }
    let wrapper = dec["command_wrapper"].owner.as_str();
    json!({
        "schema": HANDSHAKE_SCHEMA,
        "protocol": PROTOCOL,
        "version": 1,
        "host": {"name": decl.name, "version": decl.version},
        "capabilities": caps,
        "single_owner": {
            "command_interception": wrapper,
            "compression": wrapper,
            "retries": "external-host",
            "model_routing": dec["model_routing"].owner.as_str(),
        },
        "not_claimed_optimized": not_optimized,
        "observed_scope": {
            "observed": "semaprax-routed-calls-only",
            "not_observed": ["host tool calls not routed through the bridge", "conversation history", "model choice unless delegated"],
            "whole_session_savings_claimed": false,
            "metrics": "use the #357 report surface (`semaprax-harness report`, editors/vscode token-report.js)",
        },
    })
}

pub fn owner_of(decl: &HostDeclaration, avail: &Availability, cap: &str) -> Owner {
    decide(decl, avail)[cap].owner
}
