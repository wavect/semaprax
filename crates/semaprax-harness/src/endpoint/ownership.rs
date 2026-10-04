//! Who owns selection, balancing, retries and failover; operator disclosure;
//! policy enforcement; reviewed LiteLLM config.

use super::types::{destination_from_json, destination_to_json, err, remote_unknown};
use crate::decision::Destination;
use crate::diag::HarnessResult;
use serde_json::{json, Map, Value};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Balancing {
    None,
    EquivalentDeploymentsOnly,
    Undisclosed,
}

/// `Exact(n)` is n retries after the first attempt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GatewayRetries {
    Disabled,
    Exact(u32),
    Undisclosed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GatewayFallbacks {
    Disabled,
    /// Each fallback target with its disclosed destination.
    Disclosed(Vec<(String, Destination)>),
    Undisclosed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttemptOwnership {
    /// Always true: the host selects the admitted logical model.
    pub semaprax_selects_logical_model: bool,
    pub gateway_balancing: Balancing,
    pub gateway_retries: GatewayRetries,
    pub gateway_fallbacks: GatewayFallbacks,
}

impl AttemptOwnership {
    /// A direct model server has no gateway layer to retry or fall back.
    pub fn direct() -> Self {
        Self {
            semaprax_selects_logical_model: true,
            gateway_balancing: Balancing::None,
            gateway_retries: GatewayRetries::Disabled,
            gateway_fallbacks: GatewayFallbacks::Disabled,
        }
    }

    /// A gateway with nothing disclosed.
    pub fn undisclosed() -> Self {
        Self {
            semaprax_selects_logical_model: true,
            gateway_balancing: Balancing::Undisclosed,
            gateway_retries: GatewayRetries::Undisclosed,
            gateway_fallbacks: GatewayFallbacks::Undisclosed,
        }
    }

    /// Upper bound on upstream attempts for one logical call, if knowable.
    pub fn max_upstream_attempts(&self) -> Option<u32> {
        let retries = match self.gateway_retries {
            GatewayRetries::Disabled => 0,
            GatewayRetries::Exact(n) => n,
            GatewayRetries::Undisclosed => return None,
        };
        let fallbacks = match &self.gateway_fallbacks {
            GatewayFallbacks::Disabled => 0,
            GatewayFallbacks::Disclosed(l) => l.len() as u32,
            GatewayFallbacks::Undisclosed => return None,
        };
        Some((1 + retries).saturating_mul(1 + fallbacks))
    }

    pub fn to_json(&self) -> Value {
        let balancing = match self.gateway_balancing {
            Balancing::None => "none",
            Balancing::EquivalentDeploymentsOnly => "equivalent_deployments_only",
            Balancing::Undisclosed => "undisclosed",
        };
        let retries = match self.gateway_retries {
            GatewayRetries::Disabled => json!("disabled"),
            GatewayRetries::Exact(n) => json!({"exact": n}),
            GatewayRetries::Undisclosed => json!("undisclosed"),
        };
        let fallbacks = match &self.gateway_fallbacks {
            GatewayFallbacks::Disabled => json!("disabled"),
            GatewayFallbacks::Disclosed(l) => Value::Array(
                l.iter()
                    .map(|(m, d)| json!({"model": m, "destination": destination_to_json(d)}))
                    .collect(),
            ),
            GatewayFallbacks::Undisclosed => json!("undisclosed"),
        };
        json!({
            "semaprax_selects_logical_model": self.semaprax_selects_logical_model,
            "gateway_balancing": balancing,
            "gateway_retries": retries,
            "gateway_fallbacks": fallbacks,
            "max_upstream_attempts": self.max_upstream_attempts(),
        })
    }

    pub fn from_json(v: &Value) -> Option<Self> {
        let balancing = match v.get("gateway_balancing")?.as_str()? {
            "none" => Balancing::None,
            "equivalent_deployments_only" => Balancing::EquivalentDeploymentsOnly,
            _ => Balancing::Undisclosed,
        };
        Some(Self {
            semaprax_selects_logical_model: true,
            gateway_balancing: balancing,
            gateway_retries: parse_retries(v.get("gateway_retries")),
            gateway_fallbacks: parse_fallbacks(v.get("gateway_fallbacks")),
        })
    }
}

fn parse_retries(v: Option<&Value>) -> GatewayRetries {
    match v {
        Some(Value::String(s)) if s == "disabled" => GatewayRetries::Disabled,
        Some(o) => match o.get("exact").and_then(Value::as_u64) {
            Some(n) if n <= u64::from(u32::MAX) => GatewayRetries::Exact(n as u32),
            _ => GatewayRetries::Undisclosed,
        },
        None => GatewayRetries::Undisclosed,
    }
}

fn parse_fallbacks(v: Option<&Value>) -> GatewayFallbacks {
    match v {
        Some(Value::String(s)) if s == "disabled" => GatewayFallbacks::Disabled,
        Some(Value::Array(a)) => {
            let mut out = Vec::new();
            for e in a {
                let (Some(m), Some(d)) = (
                    e.get("model").and_then(Value::as_str),
                    e.get("destination").and_then(destination_from_json),
                ) else {
                    return GatewayFallbacks::Undisclosed;
                };
                out.push((m.to_string(), d));
            }
            if out.is_empty() {
                GatewayFallbacks::Disabled
            } else {
                GatewayFallbacks::Disclosed(out)
            }
        }
        _ => GatewayFallbacks::Undisclosed,
    }
}

/// Operator-provided gateway disclosure (`semaprax.harness-endpoint-disclosure.v1`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Disclosure {
    /// Every destination a request may reach through this endpoint.
    pub destinations: Vec<Destination>,
    pub ownership: AttemptOwnership,
}

pub const DISCLOSURE_SCHEMA: &str = "semaprax.harness-endpoint-disclosure.v1";

impl Disclosure {
    /// Strict: unknown members and malformed values are `SPX-HPL005`.
    pub fn from_json(v: &Value) -> HarnessResult<Self> {
        let bad = |m: &str| err("SPX-HPL005", format!("disclosure: {m}"));
        let o = v.as_object().ok_or_else(|| bad("must be an object"))?;
        for k in o.keys() {
            if !matches!(
                k.as_str(),
                "schema" | "destinations" | "balancing" | "retries" | "fallbacks"
            ) {
                return Err(bad(&format!("unknown member `{k}`")));
            }
        }
        if o.get("schema").and_then(Value::as_str) != Some(DISCLOSURE_SCHEMA) {
            return Err(bad(
                "schema must be semaprax.harness-endpoint-disclosure.v1",
            ));
        }
        let mut destinations = Vec::new();
        if let Some(d) = o.get("destinations") {
            for e in d
                .as_array()
                .ok_or_else(|| bad("destinations must be an array"))?
            {
                destinations.push(destination_from_json(e).ok_or_else(|| bad("bad destination"))?);
            }
        }
        let balancing = match o.get("balancing").and_then(Value::as_str) {
            Some("none") => Balancing::None,
            Some("equivalent_deployments_only") => Balancing::EquivalentDeploymentsOnly,
            None | Some("undisclosed") => Balancing::Undisclosed,
            Some(_) => return Err(bad("unknown balancing")),
        };
        for (key, val) in [
            ("retries", o.get("retries")),
            ("fallbacks", o.get("fallbacks")),
        ] {
            let ok = match (key, val) {
                (_, None) => true,
                (_, Some(Value::String(s))) => s == "disabled" || s == "undisclosed",
                ("retries", Some(x)) => x.get("exact").is_some_and(Value::is_u64),
                ("fallbacks", Some(Value::Array(_))) => true,
                _ => false,
            };
            if !ok {
                return Err(bad(&format!("bad `{key}`")));
            }
        }
        let fallbacks = parse_fallbacks(o.get("fallbacks"));
        if matches!(o.get("fallbacks"), Some(Value::Array(a)) if !a.is_empty())
            && fallbacks == GatewayFallbacks::Undisclosed
        {
            return Err(bad("fallback entries need model and destination"));
        }
        Ok(Self {
            destinations,
            ownership: AttemptOwnership {
                semaprax_selects_logical_model: true,
                gateway_balancing: balancing,
                gateway_retries: parse_retries(o.get("retries")),
                gateway_fallbacks: fallbacks,
            },
        })
    }

    /// Destination derived from the disclosure; remote-unknown when absent.
    pub fn destination(&self) -> Destination {
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
}

/// Project-level guarantees the endpoint must be able to enforce.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EndpointPolicy {
    pub local_only: bool,
    pub strict_one_attempt: bool,
}

/// Refuse a policy the endpoint cannot satisfy or verify. `SPX-HPL010`
/// undisclosed fallback under local-only, `HPL011` non-local destination,
/// `HPL012` strict one-attempt unmet or unverifiable.
pub fn check_policy(
    policy: EndpointPolicy,
    ownership: &AttemptOwnership,
    destination: &Destination,
) -> HarnessResult<()> {
    if policy.local_only {
        if *destination != Destination::Local {
            return Err(err(
                "SPX-HPL011",
                format!(
                    "local-only refused: destination is {}",
                    dest_label(destination)
                ),
            ));
        }
        match &ownership.gateway_fallbacks {
            GatewayFallbacks::Undisclosed => {
                return Err(err("SPX-HPL010", "local-only refused: gateway fallbacks are undisclosed and may reach a remote destination"))
            }
            GatewayFallbacks::Disclosed(l) if l.iter().any(|(_, d)| *d != Destination::Local) => {
                return Err(err("SPX-HPL011", "local-only refused: a disclosed fallback is remote"))
            }
            _ => {}
        }
    }
    if policy.strict_one_attempt {
        match ownership.max_upstream_attempts() {
            Some(1) => {}
            Some(n) => {
                return Err(err(
                    "SPX-HPL012",
                    format!(
                        "strict one-attempt refused: gateway may make up to {n} upstream attempts"
                    ),
                ))
            }
            None => {
                return Err(err(
                    "SPX-HPL012",
                    "strict one-attempt refused: gateway retries or fallbacks are undisclosed",
                ))
            }
        }
    }
    Ok(())
}

pub fn dest_label(d: &Destination) -> String {
    match d {
        Destination::Local => "local".into(),
        Destination::Remote { origin } => format!("remote({origin})"),
    }
}

/// Reviewed LiteLLM proxy config for one admitted model: no retries, no
/// fallbacks. Deterministic text; contains no credentials.
pub fn litellm_config_snippet(logical: &str, upstream: &str, api_base: &str) -> String {
    format!(
        "# Reviewed by semaprax harness: one upstream attempt per call.\n\
model_list:\n  - model_name: {logical}\n    litellm_params:\n      model: {upstream}\n      api_base: {api_base}\n      num_retries: 0\n\
router_settings:\n  num_retries: 0\n  allowed_fails: 0\n  # no `fallbacks`, `context_window_fallbacks` or `content_policy_fallbacks`\n\
litellm_settings:\n  num_retries: 0\n  # api key comes from LITELLM_MASTER_KEY in the environment, never this file\n"
    )
}

/// Disclosure document matching [`litellm_config_snippet`] for a local upstream.
pub fn local_one_attempt_disclosure() -> Value {
    let mut m = Map::new();
    m.insert("schema".into(), json!(DISCLOSURE_SCHEMA));
    m.insert("destinations".into(), json!([{"kind": "local"}]));
    m.insert("balancing".into(), json!("none"));
    m.insert("retries".into(), json!("disabled"));
    m.insert("fallbacks".into(), json!("disabled"));
    Value::Object(m)
}
