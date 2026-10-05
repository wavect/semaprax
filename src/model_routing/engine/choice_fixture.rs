//! Deterministic `choice-select/v1` fixture provider (MR-11). No model, no
//! transport, no clock: it makes choice examples and tests reproducible
//! without paid or provisioned services. It is a contract fixture, never
//! evidence of live inference.
//!
//! Default behavior: choose the option whose host description shares the most
//! words (3+ letters) with the disclosed excerpt; with no excerpt, no overlap
//! or a tie it abstains natively. [`FixtureChoiceInvoker::answering`] scripts a
//! fixed answer string instead (for forbidden/fabricated-choice tests).

use super::choice::CHOICE_WIRE_VERSION;
use super::json::canonical;
use super::provider::{DecisionCall, DecisionInvoker};
use super::request::DecisionRequest;
use serde_json::{json, Value};
use std::collections::BTreeSet;

pub const FIXTURE_ADAPTER: &str = "semaprax/choice-fixture@1.0.0";
pub const FIXTURE_MODEL: &str = "word-overlap-fixture";

/// A deterministic choice adapter fixture.
#[derive(Clone, Debug)]
pub struct FixtureChoiceInvoker {
    /// Negotiated `decision.evaluate` versions (default 1, 2, 3).
    pub versions: Vec<u32>,
    /// Scripted answer: any string, sent as the `choice` verbatim.
    pub scripted: Option<String>,
    /// Every payload this fixture received.
    pub seen: Vec<Value>,
}

impl Default for FixtureChoiceInvoker {
    fn default() -> Self {
        Self {
            versions: vec![1, 2, CHOICE_WIRE_VERSION],
            scripted: None,
            seen: vec![],
        }
    }
}

fn words(s: &str) -> BTreeSet<String> {
    s.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| w.len() >= 3)
        .map(str::to_ascii_lowercase)
        .collect()
}

impl FixtureChoiceInvoker {
    /// Always answer `choice` (which need not be a real option).
    pub fn answering(choice: &str) -> Self {
        Self {
            scripted: Some(choice.into()),
            ..Self::default()
        }
    }

    /// A fixture that did not negotiate `choice-select/v1`.
    pub fn without_choice() -> Self {
        Self {
            versions: vec![1, 2],
            ..Self::default()
        }
    }

    pub fn calls(&self) -> usize {
        self.seen.len()
    }

    fn pick(payload: &Value) -> Option<String> {
        let excerpt = words(payload["excerpt"].as_str()?);
        let mut best: Vec<(usize, String)> = vec![];
        for o in payload["options"]
            .as_array()?
            .iter()
            .filter_map(Value::as_str)
        {
            let label = payload["rendered"]["option_labels"][o]
                .as_str()
                .unwrap_or("");
            let label = label.split_once(": ").map_or(label, |(_, d)| d);
            let n = words(label).intersection(&excerpt).count();
            best.push((n, o.to_string()));
        }
        best.sort_by(|a, b| b.0.cmp(&a.0));
        match best.as_slice() {
            [(n, o), rest @ ..] if *n > 0 && rest.first().is_none_or(|r| r.0 < *n) => {
                Some(o.clone())
            }
            _ => None,
        }
    }

    /// The `choice-select/v1` result for `payload` (also usable directly).
    pub fn answer(&self, payload: &Value) -> Value {
        let options: Vec<&str> = payload["options"]
            .as_array()
            .map(|a| a.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        let choice = match &self.scripted {
            Some(s) => Some(s.clone()),
            None => Self::pick(payload),
        };
        let n = options.len().max(1) as f64;
        let scores: serde_json::Map<String, Value> = options
            .iter()
            .map(|o| {
                let x = match &choice {
                    None => 1.0 / n,
                    Some(c) if c == o => 0.9,
                    Some(_) => 0.1 / (n - 1.0).max(1.0),
                };
                (o.to_string(), json!(x))
            })
            .collect();
        json!({
            "choice": choice, "abstain": choice.is_none(),
            "abstention_reason": if choice.is_none() { "native" } else { "none" },
            "scores": scores, "score_kind": "option_distribution",
            "native_confidence": null, "native_confidence_kind": null, "calibration_id": null,
            "call": {
                "adapter": FIXTURE_ADAPTER, "requested_model": FIXTURE_MODEL,
                "answering_model": FIXTURE_MODEL, "checkpoint": "fixture-v1",
                "identity_kind": "local_declared", "rendered_digest": payload["rendered"]["digest"],
                "wire_bytes": canonical(&payload["rendered"]).len(),
                "usage": {"input_tokens": null, "output_tokens": null, "basis": "unknown"},
                "billing": "local",
            },
        })
    }
}

impl DecisionInvoker for FixtureChoiceInvoker {
    fn evaluate(&mut self, request: &DecisionRequest) -> DecisionCall {
        self.seen.push(request.payload.clone());
        DecisionCall::Answered {
            result: self.answer(&request.payload),
            elapsed_ms: 1,
            call: None,
        }
    }

    fn decision_versions(&self) -> Vec<u32> {
        self.versions.clone()
    }
}
