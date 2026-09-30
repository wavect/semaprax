//! Agent Stage Semantic Work v1: the backend-neutral semantic fuel meter.
//!
//! One unit is charged when a checked source function frame is entered
//! (after call-depth admission, before its preconditions) and one when a
//! `while` body is entered after its condition evaluated `true`. The meter is
//! independent of the interpreter's per-node instruction steps: those remain
//! a backend-specific count that no other backend reproduces or compares.
//! See `docs/AGENT-ITERATIVE-LIFECYCLE-V2.md`, "Stage semantic work v1".

use super::{Evaluator, Flow};

/// Semantic fuel state for one evaluation. An unlimited meter still counts,
/// so every retained call reports its semantic work.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct SemanticMeter {
    used: u64,
    limit: Option<u64>,
    exhausted: bool,
}

impl SemanticMeter {
    pub(super) const fn limited(limit: u64) -> Self {
        Self {
            used: 0,
            limit: Some(limit),
            exhausted: false,
        }
    }

    pub(super) const fn used(self) -> u64 {
        self.used
    }

    pub(super) const fn limit(self) -> Option<u64> {
        self.limit
    }

    pub(super) const fn exhausted(self) -> bool {
        self.exhausted
    }
}

impl Evaluator<'_> {
    /// Charge one semantic unit. A refused charge is not counted, and
    /// exhaustion is sticky: every later charge fails the same way.
    pub(super) fn semantic_charge(&mut self) -> Result<(), Flow> {
        let meter = &mut self.semantic;
        if meter.exhausted || meter.limit.is_some_and(|limit| meter.used >= limit) {
            meter.exhausted = true;
            return Err(Flow::Exhausted);
        }
        meter.used = meter.used.saturating_add(1);
        Ok(())
    }
}
