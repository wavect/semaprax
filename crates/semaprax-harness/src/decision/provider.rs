//! External decision-provider abstraction: the invoker trait, per-adapter
//! calibration profile and the experimental-mode enablement gate.

use super::route::{TaskFamily, TaskFeatures};
use crate::contract::RequestEnvelope;
use serde_json::Value;
use std::collections::BTreeSet;

/// What one `decision.evaluate/v1` invocation produced. A returned payload is
/// untrusted data; the router validates it before any use.
#[derive(Clone, Debug, PartialEq)]
pub enum DecisionCall {
    /// Result payload `{choice, scores, abstain}` and injected elapsed time.
    Answered {
        result: Value,
        elapsed_ms: u64,
    },
    Unavailable,
    Timeout,
}

/// Implemented by the host over adapters (and by fixtures in tests).
pub trait DecisionInvoker {
    fn evaluate(&mut self, request: &RequestEnvelope) -> DecisionCall;
}

/// Adapter/task calibration. The confidence threshold belongs here, never in
/// a universal constant; a score is evidence for fallback decisions only and
/// never grants a destination.
#[derive(Clone, Debug, PartialEq)]
pub struct ProviderProfile {
    pub provider_id: String,
    pub model_id: String,
    pub checkpoint: String,
    /// Minimum score of the chosen option; `None` means scores are ignored.
    pub min_confidence: Option<f64>,
    /// Declared feature ranges; outside them the provider is not consulted.
    pub max_context_tokens: Option<u64>,
    pub supported_families: Option<BTreeSet<TaskFamily>>,
}

impl ProviderProfile {
    pub fn covers(&self, f: &TaskFeatures) -> bool {
        self.max_context_tokens
            .is_none_or(|m| f.estimated_context_tokens <= m)
            && self
                .supported_families
                .as_ref()
                .is_none_or(|s| s.contains(&f.task_family))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GateStatus {
    NotEvaluated,
    Passed { evidence: String },
    Failed,
}

/// Evaluation gate for automatic selection of a learned provider for one
/// registered task and profile. Default is `NotEvaluated` (rules decide).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnablementGate {
    pub task: String,
    pub profile: String,
    pub status: GateStatus,
}

impl EnablementGate {
    pub fn not_evaluated(task: &str, profile: &str) -> Self {
        Self {
            task: task.into(),
            profile: profile.into(),
            status: GateStatus::NotEvaluated,
        }
    }

    fn passed_for(&self, task: &str, profile: &str) -> bool {
        self.task == task
            && self.profile == profile
            && matches!(&self.status, GateStatus::Passed { evidence } if !evidence.is_empty())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderMode {
    /// User configuration explicitly selected the provider (experimental).
    Explicit,
    /// Automatic selection; requires a passed evaluation gate.
    Auto,
}

pub struct ConfiguredProvider<'a> {
    pub profile: ProviderProfile,
    pub invoker: &'a mut dyn DecisionInvoker,
    pub mode: ProviderMode,
    pub gate: EnablementGate,
}

impl ConfiguredProvider<'_> {
    pub fn enabled(&self, task: &str) -> bool {
        match self.mode {
            ProviderMode::Explicit => true,
            ProviderMode::Auto => self.gate.passed_for(task, &self.profile.provider_id),
        }
    }

    /// Visible status label.
    pub fn status(&self, task: &str) -> &'static str {
        if self.gate.passed_for(task, &self.profile.provider_id) {
            "evaluated"
        } else if self.mode == ProviderMode::Explicit {
            "experimental"
        } else {
            "rules (learned provider not evaluated)"
        }
    }
}
