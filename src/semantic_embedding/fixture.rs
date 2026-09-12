//! The only [`EmbeddingProvider`] implementations this crate ships.
//!
//! [`FixtureEmbeddingProvider`] is a pure, offline, bit-constructed
//! function of the request digest — **not a model**. See "Determinism" in
//! `docs/SEMANTIC-EMBEDDING-V1.md` for exactly what it does and does not
//! prove: it carries zero information about what a real trained model
//! would return, and its output must never be presented as embedding
//! evidence. [`ScriptedEmbeddingProvider`] exists only to exercise
//! failure/cancellation/capacity behavior the pure fixture never produces
//! on its own, mirroring
//! `live_invocation::fixture::FixtureModelHandler`'s scripted-queue
//! convention.

use std::collections::VecDeque;

use super::capability::EmbeddingCapability;
use super::provider::EmbeddingProvider;
use super::request::{digest as domain_digest, EmbeddingOutcome, EmbeddingRequest};

const COMPONENT_DOMAIN: &[u8] = b"semaprax.semantic-embedding.fixture-component.v1\0";

/// One deterministic `f32` component derived from `request_digest` and
/// `index` by direct bit construction — no floating-point instruction
/// runs while computing it, only a sha256 digest (pure integer/bitwise
/// arithmetic) and a bit-pattern reinterpretation via [`f32::from_bits`].
///
/// The digest's first 4 bytes after the `sha256:` prefix become a `u32`
/// word: its top bit seeds the sign, its low 23 bits become the mantissa,
/// and the exponent field is fixed to the IEEE-754 binary32 bias (`127`).
/// Every produced value is therefore finite and lies in
/// `(-2.0, -1.0] ∪ [1.0, 2.0)` — never zero, subnormal, infinite, or NaN.
#[must_use]
fn fixture_component(request_digest: &str, index: u32) -> f32 {
    let seed = domain_digest(
        COMPONENT_DOMAIN,
        format!("{request_digest}:{index}").as_bytes(),
    );
    // `seed` is `"sha256:"` followed by 64 lowercase hex characters; the
    // first 8 of those (4 bytes) become the integer word this function
    // reinterprets as a float's bit pattern.
    let hex_word = &seed[7..15];
    let word = u32::from_str_radix(hex_word, 16).unwrap_or(0);
    let sign_bit = (word >> 31) & 0x1;
    let mantissa = word & 0x007f_ffff;
    let bits = (sign_bit << 31) | (0x7Fu32 << 23) | mantissa;
    f32::from_bits(bits)
}

/// A pure, offline, deterministic [`EmbeddingProvider`] fixture.
///
/// Every call with byte-identical [`EmbeddingRequest`] fields returns the
/// exact same vector, on every host, every run, and every build, because
/// every component is derived from a sha256 digest by direct bit
/// construction rather than by any floating-point computation. This is a
/// deterministic identity-shaped function, not a semantic model: distinct
/// inputs produce (with overwhelming likelihood, not proof) distinct
/// vectors, but no two inputs' vectors relate to each other the way a
/// trained model's would — there is no notion of semantic similarity here
/// at all, and none should be inferred from it.
#[derive(Clone, Copy, Debug, Default)]
pub struct FixtureEmbeddingProvider;

impl EmbeddingProvider for FixtureEmbeddingProvider {
    fn embed(
        &mut self,
        _capability: &EmbeddingCapability,
        request: &EmbeddingRequest,
    ) -> EmbeddingOutcome {
        let request_digest = request.digest();
        let vector = (0..request.dimensions)
            .map(|index| fixture_component(&request_digest, index))
            .collect();
        EmbeddingOutcome::Settled(vector)
    }
}

/// A scripted provider for exercising failure/cancellation/capacity paths
/// [`FixtureEmbeddingProvider`] never produces on its own. Calling `embed`
/// past the end of the script panics: a passing replay test proves it
/// never runs out, matching
/// `live_invocation::fixture::FixtureModelHandler`'s convention.
pub struct ScriptedEmbeddingProvider {
    script: VecDeque<EmbeddingOutcome>,
    pub calls: usize,
}

impl ScriptedEmbeddingProvider {
    #[must_use]
    pub fn scripted(script: Vec<EmbeddingOutcome>) -> Self {
        Self {
            script: script.into(),
            calls: 0,
        }
    }

    /// A provider that panics if it is ever called — used to prove a
    /// kernel-side check (cancellation, capacity) refused a request before
    /// any provider was reached.
    #[must_use]
    pub fn must_not_be_called() -> Self {
        Self {
            script: VecDeque::new(),
            calls: 0,
        }
    }
}

impl EmbeddingProvider for ScriptedEmbeddingProvider {
    fn embed(
        &mut self,
        _capability: &EmbeddingCapability,
        _request: &EmbeddingRequest,
    ) -> EmbeddingOutcome {
        self.calls += 1;
        self.script
            .pop_front()
            .unwrap_or_else(|| panic!("scripted embedding provider called past its script"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(dimensions: u32) -> EmbeddingRequest {
        EmbeddingRequest {
            input: b"hello".to_vec(),
            model_binding: "fixture-v1".to_owned(),
            dimensions,
            max_input_bytes: 64,
        }
    }

    #[test]
    fn fixture_component_is_a_stable_known_answer() {
        // Locks the exact bit-construction algorithm in place: the request
        // digest is the one `request::tests::digest_is_a_stable_known_
        // answer_for_a_fixed_request` also pins down.
        let request_digest =
            "sha256:47e991f823e01280fd1ab8edf848fe4971862d82088414311244c4212885e40f";
        assert_eq!(fixture_component(request_digest, 0).to_bits(), 0xbf9d_b3f1);
        assert_eq!(fixture_component(request_digest, 1).to_bits(), 0xbff0_37ff);
        assert_eq!(fixture_component(request_digest, 2).to_bits(), 0x3fc2_9ee9);
    }

    #[test]
    fn every_component_is_finite_and_within_the_documented_range() {
        let mut provider = FixtureEmbeddingProvider;
        let capability = EmbeddingCapability::grant("test");
        let outcome = provider.embed(&capability, &request(64));
        let EmbeddingOutcome::Settled(vector) = outcome else {
            panic!("fixture provider must always settle");
        };
        assert_eq!(vector.len(), 64);
        for component in vector {
            assert!(component.is_finite());
            assert!(
                (-2.0..=-1.0).contains(&component) || (1.0..2.0).contains(&component),
                "component {component} outside the documented fixture range"
            );
        }
    }

    #[test]
    fn scripted_provider_must_not_be_called_panics_if_reached() {
        let mut provider = ScriptedEmbeddingProvider::must_not_be_called();
        let capability = EmbeddingCapability::grant("test");
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            provider.embed(&capability, &request(3));
        }));
        assert!(result.is_err());
    }
}
