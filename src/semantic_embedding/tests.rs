//! Kernel-level tests: capability requirement, cancellation, capacity,
//! malformed-response rejection, and cross-run determinism. Unit tests for
//! the digest algorithm and the fixture's bit construction live alongside
//! their own modules (`request::tests`, `fixture::tests`).

use super::capability::EmbeddingCapability;
use super::fixture::{FixtureEmbeddingProvider, ScriptedEmbeddingProvider};
use super::kernel::embed;
use super::request::{EmbeddingFailure, EmbeddingOutcome, EmbeddingRequest};

fn request() -> EmbeddingRequest {
    EmbeddingRequest {
        input: b"hello".to_vec(),
        model_binding: "fixture-v1".to_owned(),
        dimensions: 3,
        max_input_bytes: 64,
    }
}

const NOT_CANCELLED: &dyn Fn() -> bool = &|| false;
const CANCELLED: &dyn Fn() -> bool = &|| true;

#[test]
fn cancelled_before_dispatch_never_reaches_the_provider() {
    let mut provider = ScriptedEmbeddingProvider::must_not_be_called();
    let capability = EmbeddingCapability::grant("test");
    let outcome = embed(&mut provider, &capability, &request(), CANCELLED);
    assert_eq!(
        outcome,
        EmbeddingOutcome::Failed {
            failure: EmbeddingFailure::Cancelled,
            attempted_bytes: 0,
        }
    );
    assert_eq!(provider.calls, 0);
}

#[test]
fn oversized_input_never_reaches_the_provider() {
    let mut provider = ScriptedEmbeddingProvider::must_not_be_called();
    let capability = EmbeddingCapability::grant("test");
    let mut oversized = request();
    oversized.max_input_bytes = 2; // input is 5 bytes ("hello")
    let outcome = embed(&mut provider, &capability, &oversized, NOT_CANCELLED);
    assert_eq!(
        outcome,
        EmbeddingOutcome::Failed {
            failure: EmbeddingFailure::CapacityExceeded,
            attempted_bytes: 5,
        }
    );
    assert_eq!(provider.calls, 0);
}

#[test]
fn a_settled_vector_of_the_wrong_length_is_reported_malformed_not_passed_through() {
    let mut provider =
        ScriptedEmbeddingProvider::scripted(vec![EmbeddingOutcome::Settled(vec![1.0, 2.0])]);
    let capability = EmbeddingCapability::grant("test");
    // request() asks for 3 dimensions; the scripted provider answers with 2.
    let outcome = embed(&mut provider, &capability, &request(), NOT_CANCELLED);
    assert_eq!(
        outcome,
        EmbeddingOutcome::Failed {
            failure: EmbeddingFailure::MalformedResponse,
            attempted_bytes: 8,
        }
    );
    assert_eq!(provider.calls, 1);
}

#[test]
fn a_non_finite_component_is_reported_malformed_not_passed_through() {
    let mut provider = ScriptedEmbeddingProvider::scripted(vec![EmbeddingOutcome::Settled(vec![
        1.0,
        f32::NAN,
        2.0,
    ])]);
    let capability = EmbeddingCapability::grant("test");
    let outcome = embed(&mut provider, &capability, &request(), NOT_CANCELLED);
    assert_eq!(
        outcome,
        EmbeddingOutcome::Failed {
            failure: EmbeddingFailure::MalformedResponse,
            attempted_bytes: 12,
        }
    );
}

#[test]
fn an_infinite_component_is_reported_malformed_not_passed_through() {
    let mut provider = ScriptedEmbeddingProvider::scripted(vec![EmbeddingOutcome::Settled(vec![
        1.0,
        f32::INFINITY,
        2.0,
    ])]);
    let capability = EmbeddingCapability::grant("test");
    let outcome = embed(&mut provider, &capability, &request(), NOT_CANCELLED);
    assert_eq!(
        outcome,
        EmbeddingOutcome::Failed {
            failure: EmbeddingFailure::MalformedResponse,
            attempted_bytes: 12,
        }
    );
}

#[test]
fn a_correctly_shaped_finite_vector_passes_through_unchanged() {
    let mut provider = FixtureEmbeddingProvider;
    let capability = EmbeddingCapability::grant("test");
    let outcome = embed(&mut provider, &capability, &request(), NOT_CANCELLED);
    match outcome {
        EmbeddingOutcome::Settled(vector) => assert_eq!(vector.len(), 3),
        other => panic!("expected Settled, got {other:?}"),
    }
}

#[test]
fn a_provider_failure_passes_through_unchanged() {
    let mut provider = ScriptedEmbeddingProvider::scripted(vec![EmbeddingOutcome::Failed {
        failure: EmbeddingFailure::ProviderError,
        attempted_bytes: 9,
    }]);
    let capability = EmbeddingCapability::grant("test");
    let outcome = embed(&mut provider, &capability, &request(), NOT_CANCELLED);
    assert_eq!(
        outcome,
        EmbeddingOutcome::Failed {
            failure: EmbeddingFailure::ProviderError,
            attempted_bytes: 9,
        }
    );
}

#[test]
fn the_fixture_provider_is_byte_identical_across_independent_calls() {
    // Two entirely separate provider instances and two entirely separate
    // dispatches; this is the determinism property the module claims for
    // the fixture: same request bytes in, bit-identical vector out, on
    // every call.
    let mut provider_a = FixtureEmbeddingProvider;
    let mut provider_b = FixtureEmbeddingProvider;
    let capability = EmbeddingCapability::grant("run a");
    let capability_b = EmbeddingCapability::grant("run b, different reason string");

    let outcome_a = embed(&mut provider_a, &capability, &request(), NOT_CANCELLED);
    let outcome_b = embed(&mut provider_b, &capability_b, &request(), NOT_CANCELLED);

    let (EmbeddingOutcome::Settled(vector_a), EmbeddingOutcome::Settled(vector_b)) =
        (outcome_a, outcome_b)
    else {
        panic!("fixture provider must always settle");
    };
    assert_eq!(vector_a.len(), vector_b.len());
    for (a, b) in vector_a.iter().zip(vector_b.iter()) {
        assert_eq!(a.to_bits(), b.to_bits());
    }
}

#[test]
fn distinct_input_bytes_produce_distinct_vectors() {
    let mut provider = FixtureEmbeddingProvider;
    let capability = EmbeddingCapability::grant("test");
    let mut other = request();
    other.input = b"world".to_vec();

    let EmbeddingOutcome::Settled(vector_hello) =
        embed(&mut provider, &capability, &request(), NOT_CANCELLED)
    else {
        panic!("fixture provider must always settle");
    };
    let EmbeddingOutcome::Settled(vector_world) =
        embed(&mut provider, &capability, &other, NOT_CANCELLED)
    else {
        panic!("fixture provider must always settle");
    };
    assert_ne!(vector_hello, vector_world);
}

#[test]
fn the_capability_carries_the_exact_reason_it_was_granted_with() {
    let capability = EmbeddingCapability::grant("editor semantic search index");
    assert_eq!(capability.reason(), "editor semantic search index");
}
