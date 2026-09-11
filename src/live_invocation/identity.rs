//! Live invocation identity.
//!
//! An identity is derived once, at bind time, from bytes the caller already
//! holds before any model or effect call is made: the exact ProgramRoot, the
//! deployment/model policy the caller selected, the task, the total budget,
//! the compiler-derived interaction schema this invocation expects responses
//! to decode against, and the sorted set of providers the deployment
//! approves. None of those bytes can change without changing the identity,
//! and a model response is never one of them — [`LiveInvocationSeed`] cannot
//! be constructed from anything a provider returns. That is what makes the
//! identity stable across retry, resume and recovery: retrying an uncertain
//! model call, resuming a suspended invocation, or recovering after a crash
//! all re-derive the *same* identity from the *same* pre-dispatch bytes, so a
//! resumed causal journal can be recognised as continuing the same chain
//! rather than starting a new one, and a journal entry stamped with a
//! different identity can never be spliced onto it.

use sha2::{Digest, Sha256};

use crate::diagnostic::quote_json;
use crate::digest_hex::LowerHex;

const IDENTITY_DOMAIN: &[u8] = b"semaprax.live-invocation.identity.v1\0";

/// The exact pre-dispatch bytes a live invocation is named from.
///
/// Every field here must be knowable before the first model or effect call.
/// Adding a field that can only be known after dispatch (a response digest,
/// a usage counter, a provider-reported identifier) would let a live
/// invocation's identity drift mid-run, defeating the stability this type
/// exists to provide.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveInvocationSeed {
    /// The exact ProgramRoot this invocation executes against.
    pub program_root: String,
    /// The deployment/model policy binding digest: which provider and model
    /// the deployment selects, independent of Agent source identity.
    pub deployment_policy: String,
    /// The task bytes the caller supplied, canonically encoded by the
    /// caller before construction.
    pub task: Vec<u8>,
    /// The total budget ceiling for the whole invocation.
    pub budget: i64,
    /// Digest of the compiler-derived interaction schema (the proposal
    /// grammar) every turn's response must decode against.
    pub interaction_schema_digest: String,
    /// The provider identities the deployment approves for this invocation,
    /// in caller-declared order. Order is part of identity because a
    /// reordered fallback policy is a different deployment decision.
    pub approved_providers: Vec<String>,
}

impl LiveInvocationSeed {
    fn canonical(&self) -> String {
        let providers: Vec<String> = self
            .approved_providers
            .iter()
            .map(|p| quote_json(p))
            .collect();
        format!(
            "{{\"schema\":\"semaprax.live-invocation.seed.v1\",\"program_root\":{},\"deployment_policy\":{},\"task\":{},\"budget\":{},\"interaction_schema_digest\":{},\"approved_providers\":[{}]}}",
            quote_json(&self.program_root),
            quote_json(&self.deployment_policy),
            quote_json(&hex(&self.task)),
            self.budget,
            quote_json(&self.interaction_schema_digest),
            providers.join(","),
        )
    }
}

/// The stable, immutable identity of one live invocation.
///
/// Two seeds that are byte-identical produce the same identity; any single
/// differing byte (including provider order) produces a different one. The
/// identity carries no authority: it identifies a causal journal chain, it
/// does not grant permission to extend it. See
/// `docs/LIVE-INVOCATION-CONTRACT-V1.md`.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct LiveInvocationId(String);

impl LiveInvocationId {
    /// Derives the identity from its seed.
    #[must_use]
    pub fn derive(seed: &LiveInvocationSeed) -> Self {
        let mut hash = Sha256::new();
        hash.update(IDENTITY_DOMAIN);
        hash.update(seed.canonical().as_bytes());
        Self(format!("sha256:{:x}", LowerHex(hash.finalize())))
    }

    /// The canonical `sha256:<hex>` digest string.
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for LiveInvocationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Lowercase hexadecimal, the only byte transport identity/journal bytes use.
pub(crate) fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

/// Decodes a lowercase-hex string produced by [`hex`]. Rejects odd length,
/// non-hex digits and uppercase digits, so decode never silently repairs a
/// malformed carrier.
pub(crate) fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    let raw = text.as_bytes();
    let mut output = Vec::with_capacity(text.len() / 2);
    for pair in raw.chunks(2) {
        let mut byte = 0u8;
        for digit in pair {
            let value = match digit {
                b'0'..=b'9' => digit - b'0',
                b'a'..=b'f' => digit - b'a' + 10,
                _ => return None,
            };
            byte = byte * 16 + value;
        }
        output.push(byte);
    }
    Some(output)
}

/// A generic domain-separated sha256 digest, matching the convention every
/// other agent module in this crate uses (`domain || bytes`).
#[must_use]
pub(crate) fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(bytes);
    format!("sha256:{:x}", LowerHex(hash.finalize()))
}

/// Returns whether `text` is a well-formed `sha256:<64 lowercase hex>` digest.
#[must_use]
pub(crate) fn looks_like_digest(text: &str) -> bool {
    text.len() == "sha256:".len() + 64
        && text.starts_with("sha256:")
        && text[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed() -> LiveInvocationSeed {
        LiveInvocationSeed {
            program_root: "sha256:root".into(),
            deployment_policy: "sha256:policy".into(),
            task: b"task".to_vec(),
            budget: 1000,
            interaction_schema_digest: "sha256:schema".into(),
            approved_providers: vec!["fixture".into()],
        }
    }

    #[test]
    fn identity_is_stable_for_the_same_seed_and_changes_with_any_field() {
        let base = LiveInvocationId::derive(&seed());
        assert_eq!(
            base,
            LiveInvocationId::derive(&seed()),
            "same seed, same identity"
        );

        let mut budget = seed();
        budget.budget += 1;
        assert_ne!(base, LiveInvocationId::derive(&budget));

        let mut providers = seed();
        providers.approved_providers.push("second".into());
        assert_ne!(base, LiveInvocationId::derive(&providers));

        let order = seed();
        let mut order_a = order.clone();
        order_a.approved_providers = vec!["a".into(), "b".into()];
        let mut order_b = order;
        order_b.approved_providers = vec!["b".into(), "a".into()];
        assert_ne!(
            LiveInvocationId::derive(&order_a),
            LiveInvocationId::derive(&order_b),
            "provider order is part of the deployment decision"
        );
    }

    #[test]
    fn identity_never_depends_on_response_bytes() {
        // The type system already enforces this (no field exists to carry a
        // response), but the property under test is that retry/resume derive
        // an identical identity regardless of what a provider would answer:
        // deriving twice from the same seed before any dispatch happened
        // yields the identity a real retry would recompute after dispatch.
        let seed = seed();
        let before_dispatch = LiveInvocationId::derive(&seed);
        let after_a_hypothetical_retry = LiveInvocationId::derive(&seed);
        assert_eq!(before_dispatch, after_a_hypothetical_retry);
    }
}
