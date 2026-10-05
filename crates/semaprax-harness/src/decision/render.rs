//! Deterministic host renderer `semaprax.route-render.v2` (MR-01, MR-03).
//!
//! The host prepares the complete, bounded `model-route/v2` request before
//! dispatch: typed features, the policy-admitted candidate table under
//! selection ids `m0..`, model-visible text and its digest, and a conservative
//! wire bound. Adapters use the rendered text verbatim; the host maps the
//! answer back to the opaque model id exactly. Oversized input is refused,
//! never truncated.

use super::policy::RoutePolicy;
use super::registry::DecisionTask;
use super::route::{bad, Confidentiality, ModelPlan, RouteRequest};
use super::route_v2::{
    Disclosure, EstimateBasis, QualityTier, TaskFeaturesV2, MAX_EXCERPT, MAX_LABEL,
};
use crate::diag::HarnessResult;
use crate::json::{self, canonical, sha256_plain};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub const RENDERER_V2: &str = "semaprax.route-render.v2";
pub const INSTRUCTIONS_V2: &str = "Which candidate model should handle this task step? Choose exactly one option. Prefer the least costly candidate expected to complete the step; unknown values are unknown, not zero.";
/// Decision-task bound on candidates in one v2 request.
pub const MAX_CANDIDATES_V2: usize = 16;
pub const MAX_STATE_BYTES: usize = 4096;
/// Wire-bound framing allowance for an adapter's own fixed template.
pub const WIRE_FRAMING_BYTES: u64 = 2048;

/// An estimate and where it came from; `value` is `None` iff unknown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Estimate {
    pub value: Option<u64>,
    pub basis: EstimateBasis,
}

impl Estimate {
    fn of(v: u64, basis: EstimateBasis) -> Self {
        Self {
            value: (basis != EstimateBasis::Unknown).then_some(v),
            basis,
        }
    }

    fn to_json(self) -> Value {
        json!({"value": self.value, "basis": self.basis.as_str()})
    }

    fn text(self) -> String {
        match self.value {
            Some(v) => format!("{v} ({})", self.basis.as_str()),
            None => "unknown".into(),
        }
    }
}

/// One admitted candidate as the router sees it. No opaque id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CandidateV2 {
    pub id: String,
    pub label: String,
    pub capabilities: Vec<&'static str>,
    pub max_context: u64,
    pub quality_tier: QualityTier,
    pub est_cost_micros: Estimate,
    pub est_latency_ms: Estimate,
}

impl CandidateV2 {
    pub fn to_json(&self) -> Value {
        json!({"id": self.id, "label": self.label, "capabilities": self.capabilities,
               "max_context": self.max_context, "quality_tier": self.quality_tier.as_str(),
               "est_cost_micros": self.est_cost_micros.to_json(),
               "est_latency_ms": self.est_latency_ms.to_json()})
    }
}

fn context_text(n: u64) -> String {
    if n >= 1_000_000 && n % 1_000_000 == 0 {
        format!("{}M", n / 1_000_000)
    } else if n >= 1000 {
        format!("{}k", n / 1000)
    } else {
        n.to_string()
    }
}

/// Host-derived label: tier, tool capability and context size.
pub fn derived_label(p: &ModelPlan) -> String {
    let tier = match p.descriptor.tier() {
        QualityTier::Unknown => "unknown tier",
        t => t.as_str(),
    };
    let tools = if p.tools { "tools" } else { "no tools" };
    format!("{tier}, {tools}, {} context", context_text(p.max_context))
}

/// The model-visible request and its digest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderedRequest {
    pub renderer: String,
    pub instructions: String,
    pub state: String,
    pub option_labels: BTreeMap<String, String>,
    /// `sha256:` over the canonical JSON of the four members above.
    pub digest: String,
}

impl RenderedRequest {
    fn body(&self) -> Value {
        json!({"renderer": self.renderer, "instructions": self.instructions,
               "state": self.state, "option_labels": self.option_labels})
    }

    pub fn digest_of(body: &Value) -> String {
        sha256_plain(canonical(body).as_bytes())
    }

    pub fn to_json(&self) -> Value {
        let mut v = self.body();
        v["digest"] = json!(self.digest);
        v
    }

    /// Bytes an adapter must make model-visible.
    pub fn model_visible_bytes(&self) -> u64 {
        (self.instructions.len()
            + self.state.len()
            + self.option_labels.values().map(String::len).sum::<usize>()) as u64
    }
}

/// The v2 digests bound into cache, choice and evidence keys.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct V2Digests {
    pub features: String,
    pub candidates: String,
    pub renderer: String,
    pub disclosure: String,
}

impl V2Digests {
    pub fn to_json(&self) -> Value {
        json!({"features": self.features, "candidates": self.candidates,
               "renderer": self.renderer, "disclosure": self.disclosure})
    }
}

/// A complete prepared `model-route/v2` request.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedRouteV2 {
    pub features: TaskFeaturesV2,
    pub candidates: Vec<CandidateV2>,
    /// `(selection id, opaque model id)` in admissible (id) order.
    pub selection: Vec<(String, String)>,
    pub disclosure: Disclosure,
    pub excerpt: Option<String>,
    /// Why a requested excerpt was withheld, when it was.
    pub disclosure_note: Option<String>,
    pub rendered: RenderedRequest,
    pub max_wire_bytes: u64,
    pub payload: Value,
}

/// The router endpoint may see an excerpt only under the policy's own
/// router-disclosure rule, never by inheriting generation approval.
fn disclosure(
    request: &RouteRequest,
    policy: &RoutePolicy,
) -> (Disclosure, Option<String>, Option<String>) {
    let Some(ex) = &request.signals.excerpt else {
        return (Disclosure::MetadataOnly, None, None);
    };
    let conf = request.features.confidentiality;
    let why = match policy.router_excerpt_max_confidentiality {
        None => Some("routing-disclosure policy does not admit excerpts"),
        Some(_) if conf == Confidentiality::Secret => {
            Some("secret tasks never disclose to a router")
        }
        Some(max) if conf > max => {
            Some("task confidentiality exceeds the routing-disclosure policy")
        }
        _ if ex.len() > MAX_EXCERPT => Some("excerpt exceeds 1024 bytes"),
        _ if crate::profile::config::looks_like_secret(ex) => {
            Some("excerpt looks like a credential")
        }
        _ => None,
    };
    match why {
        Some(w) => (
            Disclosure::MetadataOnly,
            None,
            Some(format!("excerpt withheld: {w}")),
        ),
        None => (Disclosure::Excerpt, Some(ex.clone()), None),
    }
}

fn render_state(f: &TaskFeaturesV2, c: &[CandidateV2], excerpt: Option<&str>) -> String {
    let mut s = String::from("features:\n");
    let fj = f.to_json();
    for (k, v) in fj.as_object().into_iter().flatten() {
        let text = match v {
            Value::Null => "unknown".to_string(),
            Value::String(x) => x.clone(),
            Value::Array(a) => a
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(","),
            other => other.to_string(),
        };
        s.push_str(&format!("{k}={text}\n"));
    }
    s.push_str("candidates:\n");
    for x in c {
        let caps = if x.capabilities.is_empty() {
            "none".to_string()
        } else {
            x.capabilities.join(",")
        };
        s.push_str(&format!(
            "{}: tier={}; capabilities={caps}; max_context={}; est_cost_micros={}; est_latency_ms={}\n",
            x.id,
            x.quality_tier.as_str(),
            x.max_context,
            x.est_cost_micros.text(),
            x.est_latency_ms.text()
        ));
    }
    if let Some(e) = excerpt {
        s.push_str("excerpt:\n");
        s.push_str(e);
        s.push('\n');
    }
    s
}

/// Closed-choice output reserve: one selection id plus a score per option,
/// derived from the protocol rather than a generation-sized constant.
pub fn router_output_reserve(options: usize) -> u64 {
    16 + 8 * options as u64
}

impl PreparedRouteV2 {
    /// Prepare the request over the screened admissible set (sorted by id).
    /// Refuses (`SPX-HPJ019`) instead of truncating any bounded field.
    pub fn prepare(
        request: &RouteRequest,
        policy: &RoutePolicy,
        admissible: &[ModelPlan],
    ) -> HarnessResult<Self> {
        const C: &str = "SPX-HPJ019";
        if admissible.is_empty() || admissible.len() > MAX_CANDIDATES_V2 {
            return Err(bad(
                C,
                format!(
                    "model-route/v2 takes 1..={MAX_CANDIDATES_V2} admitted candidates, got {}",
                    admissible.len()
                ),
            ));
        }
        let features = TaskFeaturesV2::project(&request.features, &request.signals)?;
        let mut plans: Vec<&ModelPlan> = admissible.iter().collect();
        plans.sort_by(|a, b| a.id.cmp(&b.id));
        let mut candidates = Vec::new();
        let mut selection = Vec::new();
        for (i, p) in plans.iter().enumerate() {
            let id = format!("m{i}");
            let label = match &p.descriptor.label {
                // A configured label may never smuggle the opaque id onto the wire.
                Some(l) if !l.to_ascii_lowercase().contains(&p.id.to_ascii_lowercase()) => {
                    l.clone()
                }
                _ => derived_label(p),
            };
            if label.len() > MAX_LABEL {
                return Err(bad(C, "candidate label exceeds 64 bytes"));
            }
            let mut caps = Vec::new();
            if p.structured_output {
                caps.push("structured_output");
            }
            if p.tools {
                caps.push("tools");
            }
            candidates.push(CandidateV2 {
                id: id.clone(),
                label,
                capabilities: caps,
                max_context: p.max_context,
                quality_tier: p.descriptor.tier(),
                est_cost_micros: Estimate::of(p.est_cost_micros, p.descriptor.cost_basis()),
                est_latency_ms: Estimate::of(p.est_latency_ms, p.descriptor.latency_basis()),
            });
            selection.push((id, p.id.clone()));
        }
        let (disclosure, excerpt, disclosure_note) = disclosure(request, policy);
        let state = render_state(&features, &candidates, excerpt.as_deref());
        if state.len() > MAX_STATE_BYTES {
            return Err(bad(
                C,
                format!(
                    "rendered routing state is {} bytes, above {MAX_STATE_BYTES}",
                    state.len()
                ),
            ));
        }
        let option_labels: BTreeMap<String, String> = candidates
            .iter()
            .map(|c| (c.id.clone(), format!("{}: {}", c.id, c.label)))
            .collect();
        let mut rendered = RenderedRequest {
            renderer: RENDERER_V2.into(),
            instructions: INSTRUCTIONS_V2.into(),
            state,
            option_labels,
            digest: String::new(),
        };
        rendered.digest = RenderedRequest::digest_of(&rendered.body());
        let max_wire_bytes =
            (2 * rendered.model_visible_bytes() + WIRE_FRAMING_BYTES).clamp(4096, 65_536);
        let options: Vec<&str> = candidates.iter().map(|c| c.id.as_str()).collect();
        let mut payload = json!({
            "task": DecisionTask::ModelRouteV2.id(),
            "features": features.to_json(),
            "candidates": candidates.iter().map(CandidateV2::to_json).collect::<Vec<_>>(),
            "options": options,
            "disclosure": disclosure.as_str(),
            "rendered": rendered.to_json(),
            "max_wire_bytes": max_wire_bytes,
        });
        if let Some(e) = &excerpt {
            payload["excerpt"] = json!(e);
        }
        Ok(Self {
            features,
            candidates,
            selection,
            disclosure,
            excerpt,
            disclosure_note,
            rendered,
            max_wire_bytes,
            payload,
        })
    }

    /// The opaque model id behind a selection id (exact; `None` if foreign).
    pub fn opaque(&self, selection: &str) -> Option<&str> {
        self.selection
            .iter()
            .find(|(s, _)| s == selection)
            .map(|(_, o)| o.as_str())
    }

    pub fn options(&self) -> Vec<&str> {
        self.selection.iter().map(|(s, _)| s.as_str()).collect()
    }

    pub fn digests(&self) -> V2Digests {
        let map: Vec<[&str; 2]> = self
            .selection
            .iter()
            .map(|(s, o)| [s.as_str(), o.as_str()])
            .collect();
        V2Digests {
            features: self.features.digest(),
            candidates: json::digest(
                "semaprax.decision.candidates.v2",
                &json!({"candidates": self.payload["candidates"], "selection": map}),
            ),
            renderer: json::digest(
                "semaprax.decision.renderer.v2",
                &json!({"renderer": self.rendered.renderer, "rendered": self.rendered.digest}),
            ),
            disclosure: json::digest(
                "semaprax.decision.disclosure.v2",
                &json!({"disclosure": self.disclosure.as_str(),
                        "excerpt": self.excerpt.as_ref().map(|e| sha256_plain(e.as_bytes()))}),
            ),
        }
    }

    /// The text the host reserves router spend against: the whole prepared
    /// payload, a superset of the model-visible content.
    pub fn accounting_text(&self) -> String {
        canonical(&self.payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cross-language vector: the Python adapters compute the same digest with
    /// `json.dumps(sort_keys=True, separators=(",", ":"), ensure_ascii=False)`.
    #[test]
    fn rendered_digest_matches_the_pinned_cross_language_vector() {
        let body = json!({"renderer": "semaprax.route-render.v2", "instructions": "Pick one.",
            "state": "a=1\nb=\u{fc}\ttab\n",
            "option_labels": {"m0": "m0: x", "m1": "m1: y", "m10": "m10: z", "m2": "m2: w"}});
        assert_eq!(
            canonical(&body),
            "{\"instructions\":\"Pick one.\",\"option_labels\":{\"m0\":\"m0: x\",\"m1\":\"m1: y\",\"m10\":\"m10: z\",\"m2\":\"m2: w\"},\"renderer\":\"semaprax.route-render.v2\",\"state\":\"a=1\\nb=\u{fc}\\ttab\\n\"}"
        );
        assert_eq!(
            RenderedRequest::digest_of(&body),
            "sha256:bcd08c2144439e8f326f540e57f52888c4667b504ff360cdb976834e0656ec50"
        );
    }

    #[test]
    fn closed_choice_output_reserve_is_protocol_derived_and_small() {
        assert_eq!(router_output_reserve(2), 32);
        assert!(router_output_reserve(MAX_CANDIDATES_V2) < 256);
    }
}
