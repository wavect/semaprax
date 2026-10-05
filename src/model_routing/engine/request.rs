//! The bounded `decision.evaluate` request a host-supplied invoker receives
//! (MR-15 contract, MR-07 boundary). It carries the prepared payload and the
//! host's own deadline/call/lineage bounds, never credentials, endpoints,
//! deployments or grants: a host maps it onto its own transport envelope.

use super::diag::{DecisionResult, Diagnostic};
use super::wire;
use serde_json::{json, Value};

/// `decision.evaluate` versions this core can prepare and validate.
pub const DECISION_VERSIONS: &[u32] = &[1, 2];
const MAX_RESULT_BYTES: usize = 4 * 1024 * 1024;
const MAX_DEADLINE_MS: u64 = 600_000;

/// The project a decision is taken for (identifiers only).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectBinding {
    pub id: String,
    pub worktree: String,
    pub revision: String,
}

impl ProjectBinding {
    pub fn to_json(&self) -> Value {
        json!({"id": self.id, "worktree": self.worktree, "revision": self.revision})
    }
}

/// One bounded `decision.evaluate` invocation, built by the router.
#[derive(Clone, Debug, PartialEq)]
pub struct DecisionRequest {
    pub invocation_id: String,
    pub project: ProjectBinding,
    pub lock_digest: String,
    /// `decision.evaluate` contract version (1: `model-route/v1`, 2: `/v2`).
    pub version: u32,
    pub deadline_ms: u64,
    pub max_result_bytes: usize,
    pub remaining_calls: u32,
    pub lineage: Vec<String>,
    pub payload: Value,
}

impl DecisionRequest {
    /// The same refusals the harness request envelope applies to a
    /// `decision.evaluate` request (`SPX-HPA023`, `SPX-HPA030`, payload codes).
    pub fn validate(&self) -> DecisionResult<()> {
        if !DECISION_VERSIONS.contains(&self.version) {
            return Err(Diagnostic::new(
                "SPX-HPA023",
                format!(
                    "decision.evaluate v{} is not implemented by the host",
                    self.version
                ),
            ));
        }
        if self.deadline_ms == 0 || self.deadline_ms > MAX_DEADLINE_MS {
            return Err(Diagnostic::new(
                "SPX-HPA030",
                "deadline_ms must be within 1..=600000",
            ));
        }
        if self.max_result_bytes == 0 || self.max_result_bytes > MAX_RESULT_BYTES {
            return Err(Diagnostic::new(
                "SPX-HPA030",
                "max_result_bytes must be within 1..=4194304",
            ));
        }
        for (what, s) in [
            ("invocation_id", &self.invocation_id),
            ("project.id", &self.project.id),
            ("project.worktree", &self.project.worktree),
            ("project.revision", &self.project.revision),
            ("lock_digest", &self.lock_digest),
        ] {
            if s.is_empty() || s.len() > 256 {
                return Err(Diagnostic::new(
                    "SPX-HPA030",
                    format!("{what} must be a non-empty identifier"),
                ));
            }
        }
        wire::validate(wire::Direction::Request, &self.payload)
    }
}
