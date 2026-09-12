//! Deliberately non-conforming adapters and capability declarations.
//!
//! Issue #181: "Build a shared hostile server/fixture that emits partial
//! UTF-8, malformed event order, oversized chunks, slow trickle,
//! disconnects, duplicate completion, contradictory usage, and late data
//! after cancellation." Every non-conforming behavior named there has an
//! exact, named variant here, each producing exactly one violation so a
//! conformance case can name precisely which one it caught — never a
//! generic "the adapter misbehaved". "A suite that only ever passes proves
//! nothing" (this issue's own words): every variant here exists to be
//! driven through [`super::conformance::drive_to_settlement`] or
//! [`super::capability::negotiate`] and rejected by name, in
//! `super::tests`.

use crate::live_invocation::model_invoke::ModelFailure;

use super::adapter::{AdapterEvent, AdapterPoll, AdapterSettlement};
use super::capability::{AdapterCapabilities, EndpointPolicy};
use super::fixture_adapters::{usage, ScriptedAdapter};

fn hostile_capabilities(identity: &str) -> AdapterCapabilities {
    super::fixture_adapters::base_capabilities(identity, true)
}

/// Two `Completed` events for one request. Issue #181's "duplicate
/// completion".
#[must_use]
pub fn duplicate_completion_adapter() -> ScriptedAdapter {
    let script = vec![
        AdapterPoll::Event(AdapterEvent::Delta(b"partial".to_vec())),
        AdapterPoll::Event(AdapterEvent::Completed),
        AdapterPoll::Event(AdapterEvent::Completed),
        AdapterPoll::Settled(AdapterSettlement {
            response_bytes: b"partial".to_vec(),
            usage: usage(1, 1, 1),
        }),
    ];
    ScriptedAdapter::new(
        hostile_capabilities("hostile-duplicate-completion"),
        script,
        true,
    )
}

/// A later usage snapshot reports fewer tokens/lower cost than an earlier
/// one — internally inconsistent, never legal progress for a single
/// request. Issue #181's "contradictory usage".
#[must_use]
pub fn contradictory_usage_adapter() -> ScriptedAdapter {
    let script = vec![
        AdapterPoll::Event(AdapterEvent::Delta(b"ab".to_vec())),
        AdapterPoll::Event(AdapterEvent::Usage {
            tokens_in: 10,
            tokens_out: 10,
            cost_micros: 500,
        }),
        AdapterPoll::Event(AdapterEvent::Usage {
            tokens_in: 10,
            tokens_out: 4, // regresses: fewer output tokens than already reported
            cost_micros: 500,
        }),
        AdapterPoll::Event(AdapterEvent::Completed),
        AdapterPoll::Settled(AdapterSettlement {
            response_bytes: b"ab".to_vec(),
            usage: usage(10, 10, 500),
        }),
    ];
    ScriptedAdapter::new(
        hostile_capabilities("hostile-contradictory-usage"),
        script,
        true,
    )
}

/// One `Delta` alone larger than the request's declared
/// `max_response_bytes`. Issue #181's "oversized chunks".
#[must_use]
pub fn oversized_chunk_adapter(oversized_len: usize) -> ScriptedAdapter {
    let script = vec![
        AdapterPoll::Event(AdapterEvent::Delta(vec![b'x'; oversized_len])),
        AdapterPoll::Event(AdapterEvent::Completed),
        AdapterPoll::Settled(AdapterSettlement {
            response_bytes: vec![b'x'; oversized_len],
            usage: usage(1, 1, 1),
        }),
    ];
    ScriptedAdapter::new(hostile_capabilities("hostile-oversized-chunk"), script, true)
}

/// Ignores cancellation and keeps delivering scripted deltas after the
/// driver requests it stop. Issue #181's "late data after cancellation".
/// `honor_cancellation: false` is the exact mechanism: a conforming
/// adapter (every type in [`super::fixture_adapters`]) always honors it.
#[must_use]
pub fn late_data_after_cancel_adapter() -> ScriptedAdapter {
    let script = vec![
        AdapterPoll::Event(AdapterEvent::Delta(b"before-cancel".to_vec())),
        AdapterPoll::Event(AdapterEvent::Delta(b"after-cancel-should-not-arrive".to_vec())),
        AdapterPoll::Event(AdapterEvent::Completed),
        AdapterPoll::Settled(AdapterSettlement {
            response_bytes: b"before-cancelafter-cancel-should-not-arrive".to_vec(),
            usage: usage(1, 1, 1),
        }),
    ];
    ScriptedAdapter::new(
        hostile_capabilities("hostile-late-data-after-cancel"),
        script,
        false,
    )
}

/// A `Delta` arrives after `Completed`. Issue #181's "malformed event
/// order".
#[must_use]
pub fn malformed_order_delta_after_completed_adapter() -> ScriptedAdapter {
    let script = vec![
        AdapterPoll::Event(AdapterEvent::Delta(b"first".to_vec())),
        AdapterPoll::Event(AdapterEvent::Completed),
        AdapterPoll::Event(AdapterEvent::Delta(b"trailing-after-completed".to_vec())),
        AdapterPoll::Settled(AdapterSettlement {
            response_bytes: b"first".to_vec(),
            usage: usage(1, 1, 1),
        }),
    ];
    ScriptedAdapter::new(
        hostile_capabilities("hostile-malformed-event-order"),
        script,
        true,
    )
}

/// Delivers a few deltas, then fails mid-stream as if the connection
/// dropped. This is not itself a violation the driver must catch — a real
/// disconnect is a legitimate, if unfortunate, outcome — it is a fixture
/// proving the driver surfaces it unchanged as
/// [`ModelFailure::ProviderError`] with a bounded `attempted_bytes` rather
/// than losing, hiding, or misclassifying it. Issue #181's "disconnects".
#[must_use]
pub fn disconnect_mid_stream_adapter() -> ScriptedAdapter {
    let script = vec![
        AdapterPoll::Event(AdapterEvent::Delta(b"only-this-much-arrived".to_vec())),
        AdapterPoll::Failed {
            failure: ModelFailure::ProviderError,
            attempted_bytes: b"only-this-much-arrived".len(),
        },
    ];
    ScriptedAdapter::new(
        hostile_capabilities("hostile-disconnect-mid-stream"),
        script,
        true,
    )
}

/// Declares its own ambient endpoint instead of
/// [`EndpointPolicy::HostInjected`]. Issue #181's "Adapters can hide
/// ambient endpoint, proxy, environment, or credential lookup" — refused
/// unconditionally by [`super::capability::negotiate`], before any
/// dispatch.
#[must_use]
pub fn ambient_endpoint_capabilities() -> AdapterCapabilities {
    let mut caps = hostile_capabilities("hostile-ambient-endpoint");
    caps.endpoint_policy = EndpointPolicy::AdapterDeclaredAmbient(
        "http://169.254.169.254/latest/meta-data/iam/security-credentials/".into(),
    );
    caps
}

/// Declares [`crate::model_budget_policy::AttemptOutcomeClass::Uncertain`]
/// (effect-delivery-uncertain, never safe to retry) as retryable. Refused
/// unconditionally by [`super::capability::negotiate`].
#[must_use]
pub fn unsafe_retryable_capabilities() -> AdapterCapabilities {
    let mut caps = hostile_capabilities("hostile-unsafe-retryable-class");
    caps.retryable_failure_classes
        .push(crate::model_budget_policy::AttemptOutcomeClass::Uncertain);
    caps
}

/// A structurally ordinary, conforming adapter that additionally carries an
/// opaque value shaped like a real credential — the way a real transport
/// adapter necessarily must, to authenticate to a provider. Used only to
/// prove that nothing in this SDK's own report/rendering path ever echoes
/// an adapter's internal state: see
/// `super::tests::a_credential_held_by_an_adapter_never_appears_in_its_conformance_report`.
pub struct CredentialHoldingAdapter {
    inner: ScriptedAdapter,
    held_credential: String,
}

impl CredentialHoldingAdapter {
    #[must_use]
    pub fn new(held_credential: impl Into<String>) -> Self {
        let script = vec![AdapterPoll::Settled(AdapterSettlement {
            response_bytes: b"ok".to_vec(),
            usage: usage(1, 1, 1),
        })];
        Self {
            inner: ScriptedAdapter::new(
                hostile_capabilities("credential-holding-adapter"),
                script,
                true,
            ),
            held_credential: held_credential.into(),
        }
    }

    /// Test-only accessor so the negative-control test can assert the exact
    /// value it must never see leaked, without this SDK exposing any
    /// production path to it.
    #[must_use]
    pub fn held_credential(&self) -> &str {
        &self.held_credential
    }
}

impl super::adapter::ProviderAdapter for CredentialHoldingAdapter {
    fn capabilities(&self) -> &AdapterCapabilities {
        self.inner.capabilities()
    }
    fn start(
        &mut self,
        capability: &super::adapter::AdapterInvocationCapability,
        request: &super::adapter::AdapterRequest,
    ) -> Result<(), super::adapter::AdapterRefusal> {
        self.inner.start(capability, request)
    }
    fn poll(&mut self) -> AdapterPoll {
        self.inner.poll()
    }
    fn cancel(&mut self, reason: &str) {
        self.inner.cancel(reason);
    }
}
