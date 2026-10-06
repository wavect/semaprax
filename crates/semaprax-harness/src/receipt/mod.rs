//! Provider usage and billing receipts for one `model.generate` attempt.
//!
//! The receipt travels beside the proposal bytes, never inside them: a
//! proposal body that claims usage, cost or success is untrusted model text
//! and cannot override it. Scripted and legacy providers report an explicit
//! `unavailable` receipt; unknown stays unknown.

pub mod controls;
pub mod normalize;
pub mod price;

pub use controls::{
    ControlReport, ControlStatus, Effort, GenerationControls, GenerationSupport, Support,
    OUTPUT_CAP_SEMANTICS,
};
pub use normalize::{event_usage, merge_native, merge_stream, normalize, numeric_only, Usage};
pub use price::{CostEstimate, PriceBook, PriceRecord, Pricing};

use crate::endpoint::Protocol;
use serde_json::{json, Value};

pub const RECEIPT_SCHEMA: &str = "semaprax.harness-model-receipt.v1";

/// Why the provider stopped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Finish {
    Complete,
    /// Cut by an output or context limit: the reply is incomplete.
    LengthLimited,
    Other(String),
    /// No receipt or no finish reason (legacy provider).
    Unknown,
}

impl Finish {
    pub fn parse(s: &str) -> Self {
        match s {
            "stop" | "end_turn" | "stop_sequence" | "completed" | "complete" => Self::Complete,
            "length" | "max_tokens" | "max_output_tokens" | "incomplete" => Self::LengthLimited,
            o => Self::Other(o.chars().take(64).collect()),
        }
    }
    pub fn as_str(&self) -> String {
        match self {
            Self::Complete => "complete".into(),
            Self::LengthLimited => "length_limited".into(),
            Self::Other(s) => format!("other:{s}"),
            Self::Unknown => "unknown".into(),
        }
    }
    /// A reply whose terminal state is known not to be a complete answer.
    pub fn incomplete(&self) -> bool {
        matches!(self, Self::LengthLimited | Self::Other(_))
    }
}

/// What upstream attempts a receipt covers (DV-20). A normal provider usage
/// object or charge describes only the final upstream request; a gateway that
/// retried may disclose aggregate usage or cost, and may prove retries unused.
/// Token and cost coverage are separate: an aggregate charge does not prove
/// aggregate token usage. The default is final-attempt coverage, no proof.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReceiptCoverage {
    /// `usage` covers every upstream attempt of the invocation.
    pub usage_aggregate: bool,
    /// `provider_cost_micros` covers every upstream attempt of the invocation.
    pub cost_aggregate: bool,
    /// Upstream attempts the gateway proves were never dispatched or billed.
    pub unused_attempts: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposalReceipt {
    /// `Some(reason)` when no receipt was produced (scripted or legacy provider).
    pub unavailable: Option<&'static str>,
    pub protocol: Option<Protocol>,
    /// Model label the provider returned (identity, not a request).
    pub model: Option<String>,
    pub request_id: Option<String>,
    pub finish: Finish,
    pub usage: Usage,
    /// Provider-reported charge in micro-units, or unknown.
    pub provider_cost_micros: Option<u64>,
    /// Which upstream attempts `usage` and the charge cover.
    pub coverage: ReceiptCoverage,
    pub max_output_tokens: ControlReport,
    pub reasoning: ControlReport,
    /// Numeric-only provider-native usage, for auditing the normalization.
    pub native: Value,
}

impl ProposalReceipt {
    pub fn unavailable(reason: &'static str) -> Self {
        Self {
            unavailable: Some(reason),
            protocol: None,
            model: None,
            request_id: None,
            finish: Finish::Unknown,
            usage: Usage::default(),
            provider_cost_micros: None,
            coverage: ReceiptCoverage::default(),
            max_output_tokens: ControlReport::NOT_REQUESTED,
            reasoning: ControlReport::NOT_REQUESTED,
            native: Value::Null,
        }
    }

    /// Build from the `receipt` member of a validated `model.generate` result.
    /// `requested` is what the host asked for; the provider's answer about
    /// what it applied is recorded next to it.
    pub fn from_result(requested: &GenerationControls, payload: &Value) -> Self {
        let mut r = Self::unavailable("adapter_reported_no_receipt");
        let ctl = payload.get("receipt").filter(|v| v.is_object());
        let cr = |req: bool, k: &str| {
            ControlReport::from_reply(req, ctl.and_then(|c| c.pointer(&format!("/controls/{k}"))))
        };
        r.max_output_tokens = cr(requested.max_output_tokens.is_some(), "max_output_tokens");
        r.reasoning = cr(requested.reasoning.is_some(), "reasoning");
        let Some(c) = ctl else { return r };
        r.unavailable = None;
        r.protocol = c
            .get("protocol")
            .and_then(Value::as_str)
            .and_then(|p| Protocol::parse(p).ok());
        let s = |k: &str, n: usize| {
            c.get(k)
                .and_then(Value::as_str)
                .map(|x| x.chars().take(n).collect::<String>())
        };
        r.model = s("model", 128);
        r.request_id = s("request_id", 128);
        r.finish = c
            .get("finish_reason")
            .and_then(Value::as_str)
            .map_or(Finish::Unknown, Finish::parse);
        r.provider_cost_micros = c.get("provider_cost_micros").and_then(Value::as_u64);
        let scope = |k: &str| c.get(k).and_then(Value::as_str) == Some("aggregate");
        r.coverage = ReceiptCoverage {
            usage_aggregate: scope("usage_scope"),
            cost_aggregate: scope("cost_scope"),
            unused_attempts: c
                .get("unused_attempts")
                .and_then(Value::as_u64)
                .unwrap_or(0),
        };
        let mut native = c.get("usage").cloned().unwrap_or_else(|| json!({}));
        if let Some(ev) = c.get("usage_events").and_then(Value::as_array) {
            let events = ev.iter().filter_map(|e| event_usage(e).or(Some(e)));
            merge_native(&mut native, &merge_stream(events));
        }
        r.native = numeric_only(&native, 3);
        r.usage = match r.protocol {
            Some(p) => normalize(p, &native),
            None => Usage::default(),
        };
        r
    }

    pub fn to_json(&self, estimate: Option<&CostEstimate>) -> Value {
        let unk = |o: Option<u64>| o.map_or(json!("unknown"), |n| json!(n));
        let mut v = json!({
            "schema": RECEIPT_SCHEMA,
            "availability": match self.unavailable { Some(r) => json!({"state": "unavailable", "reason": r}), None => json!({"state": "observed"}) },
            "protocol": self.protocol.map(Protocol::as_str),
            "returned_model": self.model.clone().unwrap_or_else(|| "unknown".into()),
            "request_id": self.request_id,
            "finish": self.finish.as_str(),
            "usage": self.usage.to_json(),
            "native_usage": self.native,
            "cost": {
                "provider_reported_micros": unk(self.provider_cost_micros),
                "estimated": estimate.map_or(json!({"kind": "estimate", "micros": "unknown", "basis": "not_priced"}), CostEstimate::to_json),
            },
            "controls": {"max_output_tokens": self.max_output_tokens.to_json(), "reasoning": self.reasoning.to_json()},
        });
        if self.coverage != ReceiptCoverage::default() {
            let c = &self.coverage;
            let scope = |a: bool| if a { "aggregate" } else { "final_attempt" };
            v["coverage"] = json!({"usage": scope(c.usage_aggregate), "cost": scope(c.cost_aggregate),
                                   "unused_attempts": c.unused_attempts});
        }
        v
    }
}

/// Per-run record of every dispatched attempt's receipt, with measured usage,
/// preflight counts and reserved tokens reported separately.
#[derive(Default)]
pub struct ReceiptLog {
    entries: Vec<Value>,
    usages: Vec<Usage>,
    preflight_input: u64,
    reserved_output: u64,
}

impl ReceiptLog {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    #[allow(clippy::too_many_arguments)]
    pub fn push(
        &mut self,
        step: &str,
        provider: &str,
        requested_model: &str,
        preflight_input_tokens: u64,
        reserved_output_tokens: u64,
        r: &ProposalReceipt,
        est: &CostEstimate,
    ) {
        self.preflight_input = self.preflight_input.saturating_add(preflight_input_tokens);
        self.reserved_output = self.reserved_output.saturating_add(reserved_output_tokens);
        self.usages.push(r.usage);
        self.entries.push(json!({
            "step": step, "provider": provider, "requested_model": requested_model,
            "preflight_input_tokens": preflight_input_tokens,
            "reserved_output_tokens": reserved_output_tokens,
            "receipt": r.to_json(Some(est)),
        }));
    }

    pub fn to_json(&self) -> Value {
        let mut known = [0u64; 5];
        let mut unknown = [0u64; 5];
        for u in &self.usages {
            let cats = [
                u.uncached_input,
                u.cache_read,
                u.cache_write,
                u.output,
                u.reasoning,
            ];
            for (i, c) in cats.iter().enumerate() {
                match c {
                    Some(n) => known[i] = known[i].saturating_add(*n),
                    None => unknown[i] += 1,
                }
            }
        }
        let names = [
            "uncached_input",
            "cache_read",
            "cache_write",
            "output",
            "reasoning",
        ];
        let m: serde_json::Map<String, Value> = names
            .iter()
            .enumerate()
            .map(|(i, n)| {
                (
                    n.to_string(),
                    json!({"known_sum": known[i], "attempts_unknown": unknown[i]}),
                )
            })
            .collect();
        json!({"attempts": self.entries.len(), "entries": self.entries,
               "measured": m,
               "preflight_input_tokens": self.preflight_input,
               "reserved_output_tokens": self.reserved_output})
    }
}
