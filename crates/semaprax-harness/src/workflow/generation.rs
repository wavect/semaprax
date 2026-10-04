//! Host-owned generation policy (TC-02): the output-token cap and optional
//! reasoning control sent with each `model.generate` request, chosen by
//! response shape. Opt-in: with no tiers configured the cap is the existing
//! budget `output_reserve_tokens` and nothing else changes.

use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::receipt::{Effort, GenerationControls, GenerationSupport, Support};
use std::collections::BTreeMap;

/// What kind of reply a step expects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ResponseShape {
    /// A compact structured intent (the normal proposal).
    StructuredIntent,
    /// A bounded source edit for an unverified-baseline repair.
    SourceRepair,
}

/// One configured tier. `None` members keep the existing default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ShapeTier {
    pub max_output_tokens: Option<u64>,
    pub reasoning: Option<Effort>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GenerationPolicy {
    /// Refuse before any outbound request when the provider cannot honor the cap.
    pub strict: bool,
    pub tiers: BTreeMap<ResponseShape, ShapeTier>,
    /// Explicit user override of every tier's reasoning control.
    pub reasoning_override: Option<Effort>,
    /// A larger cap for one new, separately reserved attempt after a
    /// length-limited reply. Unset: a truncated reply is never retried.
    pub length_retry_cap: Option<u64>,
}

/// Marker beginning the message of a length-truncation refusal (`SPX-HPD030`).
pub const TRUNCATED_PREFIX: &str = "incomplete model output";

pub fn is_truncation(e: &HarnessDiagnostic) -> bool {
    e.code == "SPX-HPD030" && e.message.starts_with(TRUNCATED_PREFIX)
}

fn d(code: &'static str, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

impl GenerationPolicy {
    pub fn with_tier(mut self, shape: ResponseShape, tier: ShapeTier) -> Self {
        self.tiers.insert(shape, tier);
        self
    }
    /// Reserved output tokens for `shape`; the configured tier, else `default`.
    pub fn reserve_for(&self, shape: ResponseShape, default: u64) -> u64 {
        self.tiers
            .get(&shape)
            .and_then(|t| t.max_output_tokens)
            .unwrap_or(default)
    }
    pub fn reasoning_for(&self, shape: ResponseShape) -> Option<Effort> {
        self.reasoning_override
            .or_else(|| self.tiers.get(&shape).and_then(|t| t.reasoning))
    }
    pub fn controls(&self, shape: ResponseShape, reserve: u64) -> GenerationControls {
        GenerationControls {
            max_output_tokens: Some(reserve),
            reasoning: self.reasoning_for(shape),
            strict: self.strict,
        }
    }
    /// Strict mode: refuse before routing or dispatch when the provider is not
    /// declared to honor the cap (or a requested reasoning control).
    pub fn gate(&self, shape: ResponseShape, support: &GenerationSupport) -> HarnessResult<()> {
        if !self.strict {
            return Ok(());
        }
        if support.output_cap != Support::Supported {
            return Err(d(
                "SPX-HPD101",
                "strict budget: the model provider is not declared to enforce an output-token cap; refused before dispatch",
            ));
        }
        if self.reasoning_for(shape).is_some() && support.reasoning != Support::Supported {
            return Err(d(
                "SPX-HPD101",
                "strict budget: the model provider is not declared to honor the reasoning control; refused before dispatch",
            ));
        }
        Ok(())
    }
}
