//! Deterministic, offline `ProviderAdapter` implementations.
//!
//! These are the only "real" adapters this crate ships. Every one of them
//! is pure, in-memory, scripted data: no socket, no environment lookup, no
//! credential, no clock. They exist so the conformance suite
//! ([`super::conformance`]) can be exercised end to end with no network, no
//! provider credentials, and no model spend — matching
//! [`crate::live_invocation::fixture`]'s own reason for existing, one layer
//! up the stack. [`ScriptedBatchAdapter`] and [`ScriptedStreamingAdapter`]
//! are "materially different" in transport shape (single-shot vs
//! multi-event streaming) precisely so passing both through the same
//! [`super::conformance::run_conformance_suite`] corpus demonstrates the
//! interface is not shaped around either one (issue #181's "prove the
//! interface is not provider-shaped"), entirely without a live provider.

use std::collections::VecDeque;

use crate::live_invocation::model_invoke::ModelFailure;

use super::adapter::{
    AdapterInvocationCapability, AdapterPoll, AdapterRefusal, AdapterRequest, AdapterSettlement,
    AdapterUsage, ProviderAdapter,
};
use super::capability::{
    AdapterCapabilities, CancellationSemantics, EndpointPolicy, StructuredOutputMode,
    TokenAccountingSource,
};

/// A scripted, in-order sequence of poll outcomes played back verbatim.
/// The shared engine behind every other adapter in this module: each
/// public type here differs only in its declared capabilities, its script,
/// and whether it honors `cancel`.
pub struct ScriptedAdapter {
    capabilities: AdapterCapabilities,
    script: VecDeque<AdapterPoll>,
    cancelled: bool,
    honor_cancellation: bool,
    terminal: Option<AdapterPoll>,
    pub start_calls: usize,
}

impl ScriptedAdapter {
    #[must_use]
    pub fn new(
        capabilities: AdapterCapabilities,
        script: Vec<AdapterPoll>,
        honor_cancellation: bool,
    ) -> Self {
        Self {
            capabilities,
            script: script.into(),
            cancelled: false,
            honor_cancellation,
            terminal: None,
            start_calls: 0,
        }
    }
}

impl ProviderAdapter for ScriptedAdapter {
    fn capabilities(&self) -> &AdapterCapabilities {
        &self.capabilities
    }

    fn start(
        &mut self,
        _capability: &AdapterInvocationCapability,
        _request: &AdapterRequest,
    ) -> Result<(), AdapterRefusal> {
        self.start_calls += 1;
        Ok(())
    }

    fn poll(&mut self) -> AdapterPoll {
        if let Some(terminal) = &self.terminal {
            return terminal.clone();
        }
        if self.cancelled && self.honor_cancellation {
            let outcome = AdapterPoll::Failed {
                failure: ModelFailure::Cancelled,
                attempted_bytes: 0,
            };
            self.terminal = Some(outcome.clone());
            return outcome;
        }
        match self.script.pop_front() {
            Some(terminal @ (AdapterPoll::Settled(_) | AdapterPoll::Failed { .. })) => {
                self.terminal = Some(terminal.clone());
                terminal
            }
            Some(other) => other,
            None => panic!("scripted provider adapter polled past the end of its script"),
        }
    }

    fn cancel(&mut self, _reason: &str) {
        self.cancelled = true;
    }
}

/// One usage snapshot with every field populated, for scripting settlement.
#[must_use]
pub fn usage(tokens_in: u64, tokens_out: u64, cost_micros: i64) -> AdapterUsage {
    AdapterUsage {
        tokens_in: Some(tokens_in),
        tokens_out: Some(tokens_out),
        cost_micros: Some(cost_micros),
    }
}

pub(crate) fn base_capabilities(
    adapter_identity: &str,
    supports_streaming: bool,
) -> AdapterCapabilities {
    AdapterCapabilities {
        adapter_identity: adapter_identity.to_owned(),
        adapter_version: "1.0.0".into(),
        provider_profile: "fixture".into(),
        structured_output_modes: vec![
            StructuredOutputMode::JsonMode,
            StructuredOutputMode::RawText,
        ],
        supports_streaming,
        token_accounting_source: TokenAccountingSource::LocalEstimate,
        cancellation_semantics: CancellationSemantics::BestEffortRequestStop,
        retryable_failure_classes: vec![
            crate::model_budget_policy::AttemptOutcomeClass::NotDispatched,
            crate::model_budget_policy::AttemptOutcomeClass::RejectedBeforeProcessing,
        ],
        endpoint_policy: EndpointPolicy::HostInjected,
        max_request_bytes: 4096,
        max_response_bytes: 65_536,
        max_context_tokens: 8192,
        max_output_tokens: 2048,
    }
}

/// A non-streaming, single-shot adapter: it settles the whole response in
/// one `Delta` on the first poll after `start`, never emitting more than
/// one event. Declares `supports_streaming: false` truthfully.
pub struct ScriptedBatchAdapter(ScriptedAdapter);

impl ScriptedBatchAdapter {
    #[must_use]
    pub fn new(response_bytes: Vec<u8>, response_usage: AdapterUsage) -> Self {
        let script = vec![AdapterPoll::Settled(AdapterSettlement {
            response_bytes,
            usage: response_usage,
        })];
        Self(ScriptedAdapter::new(
            base_capabilities("scripted-batch-adapter", false),
            script,
            true,
        ))
    }
}

impl ProviderAdapter for ScriptedBatchAdapter {
    fn capabilities(&self) -> &AdapterCapabilities {
        self.0.capabilities()
    }
    fn start(
        &mut self,
        capability: &AdapterInvocationCapability,
        request: &AdapterRequest,
    ) -> Result<(), AdapterRefusal> {
        self.0.start(capability, request)
    }
    fn poll(&mut self) -> AdapterPoll {
        self.0.poll()
    }
    fn cancel(&mut self, reason: &str) {
        self.0.cancel(reason);
    }
}

/// A streaming adapter: emits every element of `chunks` as its own `Delta`
/// event across successive polls, then one `Completed`, then settles with
/// `response_bytes` (the caller's own concatenation of `chunks`, kept
/// independent so a conformance case can deliberately mis-set it — see
/// `super::hostile`). Declares `supports_streaming: true` truthfully.
pub struct ScriptedStreamingAdapter(ScriptedAdapter);

impl ScriptedStreamingAdapter {
    #[must_use]
    pub fn new(
        chunks: Vec<Vec<u8>>,
        response_bytes: Vec<u8>,
        response_usage: AdapterUsage,
        honor_cancellation: bool,
    ) -> Self {
        let mut script: Vec<AdapterPoll> = chunks
            .into_iter()
            .map(|chunk| AdapterPoll::Event(super::adapter::AdapterEvent::Delta(chunk)))
            .collect();
        script.push(AdapterPoll::Event(super::adapter::AdapterEvent::Completed));
        script.push(AdapterPoll::Settled(AdapterSettlement {
            response_bytes,
            usage: response_usage,
        }));
        Self(ScriptedAdapter::new(
            base_capabilities("scripted-streaming-adapter", true),
            script,
            honor_cancellation,
        ))
    }
}

impl ProviderAdapter for ScriptedStreamingAdapter {
    fn capabilities(&self) -> &AdapterCapabilities {
        self.0.capabilities()
    }
    fn start(
        &mut self,
        capability: &AdapterInvocationCapability,
        request: &AdapterRequest,
    ) -> Result<(), AdapterRefusal> {
        self.0.start(capability, request)
    }
    fn poll(&mut self) -> AdapterPoll {
        self.0.poll()
    }
    fn cancel(&mut self, reason: &str) {
        self.0.cancel(reason);
    }
}

/// Replays one previously-recorded event sequence and terminal settlement
/// verbatim. There is no field here capable of making a network call or
/// any other host effect — construction takes only already-in-memory
/// bytes — so "replay reproduces the recording without dispatch" is a
/// structural property of this type, not a runtime check: nothing in this
/// module, or in this crate's dependency graph reachable from it, can turn
/// a `RecordedReplayAdapter::from_recording` call into an outbound call of
/// any kind.
pub struct RecordedReplayAdapter(ScriptedAdapter);

impl RecordedReplayAdapter {
    #[must_use]
    pub fn from_recording(
        capabilities: AdapterCapabilities,
        events: Vec<super::adapter::AdapterEvent>,
        settlement: AdapterSettlement,
    ) -> Self {
        let mut script: Vec<AdapterPoll> = events.into_iter().map(AdapterPoll::Event).collect();
        script.push(AdapterPoll::Settled(settlement));
        Self(ScriptedAdapter::new(capabilities, script, true))
    }

    #[must_use]
    pub fn start_calls(&self) -> usize {
        self.0.start_calls
    }
}

impl ProviderAdapter for RecordedReplayAdapter {
    fn capabilities(&self) -> &AdapterCapabilities {
        self.0.capabilities()
    }
    fn start(
        &mut self,
        capability: &AdapterInvocationCapability,
        request: &AdapterRequest,
    ) -> Result<(), AdapterRefusal> {
        self.0.start(capability, request)
    }
    fn poll(&mut self) -> AdapterPoll {
        self.0.poll()
    }
    fn cancel(&mut self, reason: &str) {
        self.0.cancel(reason);
    }
}

/// An adapter whose `start` panics if ever called. Used to prove
/// [`super::capability::negotiate`]'s refusal genuinely happens *before*
/// any dispatch: a caller that (incorrectly) called `start` on a
/// negotiation-refused adapter would fail this test immediately rather
/// than silently passing.
pub struct PanicsOnStartAdapter {
    capabilities: AdapterCapabilities,
}

impl PanicsOnStartAdapter {
    #[must_use]
    pub fn new(capabilities: AdapterCapabilities) -> Self {
        Self { capabilities }
    }
}

impl ProviderAdapter for PanicsOnStartAdapter {
    fn capabilities(&self) -> &AdapterCapabilities {
        &self.capabilities
    }
    fn start(
        &mut self,
        _capability: &AdapterInvocationCapability,
        _request: &AdapterRequest,
    ) -> Result<(), AdapterRefusal> {
        panic!(
            "start called on an adapter that negotiation should have refused before any dispatch"
        )
    }
    fn poll(&mut self) -> AdapterPoll {
        panic!("poll called on an adapter that was never legally started")
    }
    fn cancel(&mut self, _reason: &str) {
        panic!("cancel called on an adapter that was never legally started")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scripted_batch_adapter_settles_on_the_first_poll_after_start() {
        let mut adapter = ScriptedBatchAdapter::new(b"the-answer".to_vec(), usage(10, 5, 100));
        let capability = AdapterInvocationCapability::grant("test");
        let request = AdapterRequest {
            request_bytes: b"do-the-thing".to_vec(),
            max_response_bytes: 4096,
        };
        adapter
            .start(&capability, &request)
            .expect("scripted start always succeeds");
        match adapter.poll() {
            AdapterPoll::Settled(settlement) => {
                assert_eq!(settlement.response_bytes, b"the-answer");
            }
            other => panic!("expected Settled, got {other:?}"),
        }
    }

    #[test]
    fn a_recorded_replay_adapter_never_calls_start_more_than_once_per_instance() {
        let capabilities = base_capabilities("recorded-replay-adapter", true);
        let mut adapter = RecordedReplayAdapter::from_recording(
            capabilities,
            vec![super::super::adapter::AdapterEvent::Delta(
                b"chunk".to_vec(),
            )],
            AdapterSettlement {
                response_bytes: b"chunk".to_vec(),
                usage: usage(1, 1, 1),
            },
        );
        let capability = AdapterInvocationCapability::grant("test");
        let request = AdapterRequest {
            request_bytes: Vec::new(),
            max_response_bytes: 4096,
        };
        adapter.start(&capability, &request).unwrap();
        assert_eq!(adapter.start_calls(), 1);
    }
}
