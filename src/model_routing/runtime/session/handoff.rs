//! The bounded typed handoff a re-routed turn receives: committed state,
//! accepted-output digests and tool-result references, never the transcript.

use serde_json::{json, Value};

use crate::model_routing::engine::json;
use crate::model_routing::engine::Confidentiality;

pub const HANDOFF_SCHEMA: &str = "semaprax.runtime-handoff.v1";
/// Committed state carried across a destination change.
pub const MAX_HANDOFF_STATE_BYTES: usize = 16 * 1024;
/// Accepted-output digests and tool-result references carried forward.
pub const MAX_HANDOFF_REFS: usize = 32;

/// One turn's task bytes. Instructions and acceptance are carried as the
/// host's authoritative digests and tool identities as the destination
/// deployment's granted capabilities, so a destination change cannot rewrite
/// either; confidentiality is the session's, never lowered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Handoff {
    pub session: String,
    pub turn: u32,
    pub from_profile: Option<String>,
    pub to_profile: String,
    pub confidentiality: Confidentiality,
    pub instructions_digest: String,
    pub acceptance_digest: String,
    pub tool_capabilities: Vec<String>,
    pub turn_features_digest: String,
    pub committed_state: Vec<u8>,
    pub accepted_outputs: Vec<String>,
    pub tool_results: Vec<String>,
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub(crate) fn unhex(text: &str) -> Option<Vec<u8>> {
    if text.len() % 2 != 0 {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
        .collect()
}

impl Handoff {
    pub fn to_json(&self) -> Value {
        json!({
            "schema": HANDOFF_SCHEMA,
            "session": self.session, "turn": self.turn,
            "from_profile": self.from_profile, "to_profile": self.to_profile,
            "confidentiality": self.confidentiality.as_str(),
            "instructions": self.instructions_digest, "acceptance": self.acceptance_digest,
            "tool_capabilities": self.tool_capabilities,
            "turn_features": self.turn_features_digest,
            "committed_state": hex(&self.committed_state),
            "accepted_outputs": self.accepted_outputs, "tool_results": self.tool_results,
        })
    }

    /// Canonical bytes: the next invocation's task objective.
    pub fn bytes(&self) -> Vec<u8> {
        json::canonical(&self.to_json()).into_bytes()
    }

    pub fn digest(&self) -> String {
        json::digest("semaprax.runtime-handoff.digest.v1", &self.to_json())
    }
}
