//! The bounded runtime feature projection taken at an invocation boundary.

use std::collections::BTreeSet;

use serde_json::{json, Value};

use super::error::RuntimeRoutingError;
use crate::model_routing::engine::json;
use crate::model_routing::engine::{
    Budget, Confidentiality, LatencyClass, Modality, TaskFamily, TaskFeatures,
};

pub const MAX_REQUIRED_MODALITIES: usize = 4;
const MAX_CONTEXT_TOKENS: u64 = 1_000_000_000;
const MAX_COST_MICROS: u64 = 1_000_000_000_000;
const MAX_LATENCY_MS: u64 = 86_400_000;

/// Closed, host-derived request features. No prompt text, transcript or
/// provider string is part of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeFeatures {
    pub task_family: TaskFamily,
    pub estimated_context_tokens: u64,
    pub requires_structured_output: bool,
    pub requires_tools: bool,
    pub required_modalities: BTreeSet<Modality>,
    pub confidentiality: Confidentiality,
    pub latency_class: LatencyClass,
    /// Remaining cost the caller may spend on this invocation.
    pub remaining_cost_micros: u64,
    /// Remaining latency before the caller's deadline.
    pub remaining_latency_ms: u64,
    /// Router calls this boundary may spend (0 forces the rules path).
    pub max_router_calls: u32,
    /// An operator pin to one approved profile id. A pin is screened like any
    /// candidate and refuses rather than falling back.
    pub operator_pin: Option<String>,
}

impl RuntimeFeatures {
    pub fn validate(&self) -> Result<(), RuntimeRoutingError> {
        let bad = |why: &str| Err(RuntimeRoutingError::InvalidFeatures(why.to_owned()));
        if self.estimated_context_tokens > MAX_CONTEXT_TOKENS {
            return bad("estimated_context_tokens out of range");
        }
        if self.remaining_cost_micros > MAX_COST_MICROS {
            return bad("remaining_cost_micros out of range");
        }
        if self.remaining_latency_ms > MAX_LATENCY_MS {
            return bad("remaining_latency_ms out of range");
        }
        if self.max_router_calls > 1000 {
            return bad("max_router_calls out of range");
        }
        if self.required_modalities.len() > MAX_REQUIRED_MODALITIES {
            return bad("too many required modalities");
        }
        if self
            .operator_pin
            .as_ref()
            .is_some_and(|p| p.is_empty() || p.len() > 128 || !p.is_ascii())
        {
            return bad("operator_pin must be bounded ASCII");
        }
        Ok(())
    }

    pub(crate) fn task_features(&self) -> TaskFeatures {
        TaskFeatures {
            task_family: self.task_family,
            estimated_context_tokens: self.estimated_context_tokens,
            requires_structured_output: self.requires_structured_output,
            requires_tools: self.requires_tools,
            confidentiality: self.confidentiality,
            latency_class: self.latency_class,
        }
    }

    pub(crate) fn budget(&self) -> Budget {
        Budget {
            max_cost_micros: self.remaining_cost_micros,
            max_latency_ms: self.remaining_latency_ms,
            max_router_calls: self.max_router_calls,
        }
    }

    pub fn to_json(&self) -> Value {
        json!({
            "task": self.task_features().to_json(),
            "modalities": self.required_modalities.iter().map(|m| m.as_str()).collect::<Vec<_>>(),
            "remaining_cost_micros": self.remaining_cost_micros,
            "remaining_latency_ms": self.remaining_latency_ms,
            "max_router_calls": self.max_router_calls,
            "operator_pin": self.operator_pin,
        })
    }

    /// `semaprax.runtime-route-features.v1` digest bound into the record.
    pub fn digest(&self) -> String {
        json::digest("semaprax.runtime-route-features.v1", &self.to_json())
    }
}
