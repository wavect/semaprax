//! The typed embedding request/response vocabulary and closed failure
//! domain. See `docs/SEMANTIC-EMBEDDING-V1.md`.

use sha2::{Digest, Sha256};

use crate::diagnostic::quote_json;
use crate::digest_hex::LowerHex;

const REQUEST_DOMAIN: &[u8] = b"semaprax.semantic-embedding.request.v1\0";

/// One embedding request for exactly one input.
///
/// Every field is caller-supplied. Nothing here is discovered from the
/// filesystem, environment, or network: `input` is bytes the caller
/// already holds (e.g. a canonical declaration's source text or a graph
/// projection slice already fetched through the ordinary `context`/`graph`
/// tools), never a path this module resolves itself.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmbeddingRequest {
    /// The exact bytes to embed. Caller-canonicalized; this module applies
    /// no normalization of its own (case folding, whitespace collapse,
    /// encoding repair), so two byte-distinct inputs are always distinct
    /// requests.
    pub input: Vec<u8>,
    /// Explicit identity of the model/policy binding in force. Two
    /// requests carrying different bindings are never comparable: a
    /// caller must not assume vectors produced under different bindings
    /// share a coordinate space.
    pub model_binding: String,
    /// The exact vector length the caller expects back. [`super::kernel::
    /// embed`] enforces this against whatever the provider returns; a
    /// provider that answers with a different length is treated as
    /// malformed, never silently padded or truncated.
    pub dimensions: u32,
    /// The maximum accepted `input` length in bytes, enforced by the
    /// kernel before a provider is ever called.
    pub max_input_bytes: usize,
}

impl EmbeddingRequest {
    /// The canonical digest identifying this exact request. Two requests
    /// that would ask a provider the same question produce the same
    /// digest; any differing field changes it.
    #[must_use]
    pub fn digest(&self) -> String {
        let body = format!(
            "{{\"schema\":\"semaprax.semantic-embedding.request.v1\",\"input\":{},\"model_binding\":{},\"dimensions\":{},\"max_input_bytes\":{}}}",
            quote_json(&hex(&self.input)),
            quote_json(&self.model_binding),
            self.dimensions,
            self.max_input_bytes,
        );
        digest(REQUEST_DOMAIN, body.as_bytes())
    }
}

/// Lowercase hexadecimal, this module's only byte transport for the digest
/// body (matches the convention `live_invocation::identity` documents).
fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

/// A generic domain-separated sha256 digest, matching the convention every
/// other agent/assurance module in this crate uses (`domain || bytes`).
#[must_use]
pub(crate) fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(bytes);
    format!("sha256:{:x}", LowerHex(hash.finalize()))
}

/// The closed embedding failure domain.
///
/// Provider-specific errors never become caller-visible detail beyond
/// this: a handler normalizes whatever a real transport reports into
/// exactly one of these cases before returning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmbeddingFailure {
    /// The call did not settle before its deadline.
    Timeout,
    /// Cancellation was observed before or during the call.
    Cancelled,
    /// The provider or deployment reported no capacity for this call, or
    /// the request's own declared `max_input_bytes` ceiling was exceeded
    /// before any provider was reached.
    CapacityExceeded,
    /// The provider reported an error unrelated to the above.
    ProviderError,
    /// A response arrived but was the wrong length, carried a non-finite
    /// component, or was otherwise not a vector the kernel could trust.
    MalformedResponse,
    /// The provider declined to make the call (e.g. a policy refusal).
    Refused,
}

impl EmbeddingFailure {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Timeout => "timeout",
            Self::Cancelled => "cancelled",
            Self::CapacityExceeded => "capacity_exceeded",
            Self::ProviderError => "provider_error",
            Self::MalformedResponse => "malformed_response",
            Self::Refused => "refused",
        }
    }
}

/// The outcome of one dispatched embedding call, after [`super::kernel::
/// embed`]'s validation.
#[derive(Clone, Debug, PartialEq)]
pub enum EmbeddingOutcome {
    /// The provider answered with exactly `dimensions` finite `f32`
    /// components. See "Determinism" in `docs/SEMANTIC-EMBEDDING-V1.md`
    /// for exactly what byte-for-byte reproducibility this module can and
    /// cannot promise for these values.
    Settled(Vec<f32>),
    /// The call did not settle successfully, in the closed failure
    /// domain. `attempted_bytes` is a bounded measurement of whatever
    /// partial, oversized, or malformed payload was seen.
    Failed {
        failure: EmbeddingFailure,
        attempted_bytes: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_request() -> EmbeddingRequest {
        EmbeddingRequest {
            input: b"hello".to_vec(),
            model_binding: "fixture-v1".to_owned(),
            dimensions: 3,
            max_input_bytes: 64,
        }
    }

    #[test]
    fn digest_is_a_stable_known_answer_for_a_fixed_request() {
        // Locks the exact digest algorithm (domain, JSON body shape, hex
        // input encoding) in place; any change to the encoding changes
        // this literal, which is the point.
        assert_eq!(
            base_request().digest(),
            "sha256:47e991f823e01280fd1ab8edf848fe4971862d82088414311244c4212885e40f"
        );
    }

    #[test]
    fn digest_changes_with_every_field_independently() {
        let base = base_request();
        let mut with_different_input = base.clone();
        with_different_input.input = b"world".to_vec();
        let mut with_different_binding = base.clone();
        with_different_binding.model_binding = "fixture-v2".to_owned();
        let mut with_different_dimensions = base.clone();
        with_different_dimensions.dimensions = 4;
        let mut with_different_cap = base.clone();
        with_different_cap.max_input_bytes = 65;

        let base_digest = base.digest();
        assert_ne!(base_digest, with_different_input.digest());
        assert_ne!(base_digest, with_different_binding.digest());
        assert_ne!(base_digest, with_different_dimensions.digest());
        assert_ne!(base_digest, with_different_cap.digest());
    }

    #[test]
    fn digest_is_identical_for_byte_identical_requests() {
        assert_eq!(base_request().digest(), base_request().digest());
    }

    #[test]
    fn failure_as_str_is_a_closed_stable_vocabulary() {
        assert_eq!(EmbeddingFailure::Timeout.as_str(), "timeout");
        assert_eq!(EmbeddingFailure::Cancelled.as_str(), "cancelled");
        assert_eq!(
            EmbeddingFailure::CapacityExceeded.as_str(),
            "capacity_exceeded"
        );
        assert_eq!(EmbeddingFailure::ProviderError.as_str(), "provider_error");
        assert_eq!(
            EmbeddingFailure::MalformedResponse.as_str(),
            "malformed_response"
        );
        assert_eq!(EmbeddingFailure::Refused.as_str(), "refused");
    }
}
