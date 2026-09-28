//! Offline SDK bridge for the ordinary source `ProposalSource` seam.
//!
//! Checkpointing is opt-in. A checkpointed source derives its host-bound
//! request/prompt identity before it acknowledges the durable intent; the
//! ordinary SDK route remains one-pass and does not create journal rows.

use crate::agent_lifecycle::iterative::driver::{ProposalRequest, ProposalSource};
use crate::agent_lifecycle::iterative::source_live::{
    SourceAttemptIdentity, SourceProposalOutcome, SourceProposalPolicy,
};
use crate::agent_proposal::CompiledAgentProposalSchema;
use crate::agent_runtime::AgentCancellation;
use crate::agent_runtime_v2::source_model::source_request_digest;
use crate::agent_runtime_v2::{
    SourceModelBinding, SourceModelEvidence, SourceModelInvocationCapability,
    SourceModelPolicyBinding,
};
use crate::diagnostic::{quote_json, Diagnostic};
use crate::live_invocation::model_invoke::{BudgetRefusal, ModelFailure};
use crate::live_invocation::source_journal::{
    source_response_digest, PolicyAttemptIntentV6, PolicyAttemptReservationV6,
    PolicyAttemptUsageV6, SourceAttemptFailure, SourceCheckpointSink, SourceJournalEntry,
    SourcePolicyQuoteV6,
};
use crate::live_invocation::InvocationClock;
use crate::live_invocation::{CumulativeBudgetLedger, SourceInvocationClock};
use crate::model_budget_policy::live_hook::ModelAttemptQuote;
use crate::model_budget_policy::AttemptReservation;
use crate::streaming_proposal_decode::{
    SourceProposalStreamDecoder, SourcePushOutcome, MAX_STREAM_BYTES,
};

use super::adapter::{
    AdapterEvent, AdapterInvocationCapability, AdapterPoll, AdapterRequest, ProviderAdapter,
};
use super::capability::{negotiate, RequiredCapabilities, StructuredOutputMode};

mod policy;

use policy::SourceModelPolicySession;

const MAX_SOURCE_POLLS: usize = 10_000;

/// Constructs exactly one fresh adapter per source attempt. Reusing an
/// already-settled adapter would violate `ProviderAdapter::start`'s one-start
/// contract, so the factory is explicit rather than hidden inside the source.
pub trait SourceAdapterFactory {
    fn create(&mut self) -> Box<dyn ProviderAdapter>;
}

/// Host-owned, request-bound tokenizer and pricing quote for the existing
/// [`ModelAttemptQuote`] carrier. It grants no provider authority and does
/// not decode model data.
pub trait SourceModelAttemptQuoter {
    fn quote(
        &mut self,
        request: &AdapterRequest,
        binding: &SourceModelBinding,
    ) -> Result<ModelAttemptQuote, BudgetRefusal>;
}

impl<F> SourceAdapterFactory for F
where
    F: FnMut() -> Box<dyn ProviderAdapter>,
{
    fn create(&mut self) -> Box<dyn ProviderAdapter> {
        self()
    }
}

pub struct StreamingSourceProposalAdapter<'a> {
    factory: &'a mut dyn SourceAdapterFactory,
    capability: AdapterInvocationCapability,
    schema: &'a CompiledAgentProposalSchema,
    cancellation: Option<&'a AgentCancellation>,
    clock: Option<&'a dyn InvocationClock>,
    deadline_millis: Option<i64>,
    binding: Option<SourceModelBinding>,
    binding_capability: Option<SourceModelInvocationCapability>,
    policy: Option<SourceModelPolicySession<'a>>,
    pending_reservation: Option<AttemptReservation>,
    last_dispatch: Option<SourceAdapterDispatchFact>,
    checkpoint: Option<(String, usize, i64)>,
    durable_cancellation: Option<AgentCancellation>,
    durable_deadline_millis: Option<i64>,
    evidence: SourceModelEvidence,
}

/// Private terminal facts retained only until the caller projects one source
/// proposal. They make the exact adapter settlement available to the durable
/// wrapper without making raw response bytes part of public evidence.
enum SourceAdapterDispatchFact {
    Settled {
        response: Vec<u8>,
        usage: Option<(u64, u64, i64)>,
    },
    Failed {
        reason: SourceAttemptFailure,
        attempted_bytes: usize,
    },
}

/// Complete one private adapter attempt.  Raw bytes remain available only
/// until the durable checkpoint path records its closed settlement fact.
enum SourceAdapterDispatch {
    Settled {
        canonical: String,
        response: Vec<u8>,
        #[allow(dead_code)]
        usage: Option<(u64, u64, i64)>,
    },
    Failed {
        diagnostics: Vec<Diagnostic>,
        reason: SourceAttemptFailure,
        attempted_bytes: usize,
    },
}

impl<'a> StreamingSourceProposalAdapter<'a> {
    #[must_use]
    pub fn new(
        factory: &'a mut dyn SourceAdapterFactory,
        capability: AdapterInvocationCapability,
        schema: &'a CompiledAgentProposalSchema,
    ) -> Self {
        Self {
            factory,
            capability,
            schema,
            cancellation: None,
            clock: None,
            deadline_millis: None,
            binding: None,
            binding_capability: None,
            policy: None,
            pending_reservation: None,
            last_dispatch: None,
            checkpoint: None,
            durable_cancellation: None,
            durable_deadline_millis: None,
            evidence: SourceModelEvidence::default(),
        }
    }

    /// Constructs the additive Direct Runtime v2 bound source route. The
    /// existing constructor remains the compatibility route; this one refuses
    /// schema/revision or capability drift before a factory can be called.
    pub fn new_bound(
        factory: &'a mut dyn SourceAdapterFactory,
        capability: AdapterInvocationCapability,
        schema: &'a CompiledAgentProposalSchema,
        binding: SourceModelBinding,
        binding_capability: SourceModelInvocationCapability,
    ) -> Result<Self, Vec<Diagnostic>> {
        if schema.schema().digest() != binding.proposal_schema_digest()
            || schema.source_revision() != binding.source_revision()
            || !binding.capability_matches(&binding_capability)
        {
            return Err(Self::refusal("source.model_binding"));
        }
        Ok(Self {
            factory,
            capability,
            schema,
            cancellation: None,
            clock: None,
            deadline_millis: None,
            binding: Some(binding),
            binding_capability: Some(binding_capability),
            policy: None,
            pending_reservation: None,
            last_dispatch: None,
            checkpoint: None,
            durable_cancellation: None,
            durable_deadline_millis: None,
            evidence: SourceModelEvidence::default(),
        })
    }

    pub fn new_bound_checkpointed(
        factory: &'a mut dyn SourceAdapterFactory,
        capability: AdapterInvocationCapability,
        schema: &'a CompiledAgentProposalSchema,
        binding: SourceModelBinding,
        binding_capability: SourceModelInvocationCapability,
        policy: SourceProposalPolicy<'_>,
    ) -> Result<Self, Vec<Diagnostic>> {
        if policy.deployment_binding != binding.digest()
            || policy.response_limit != binding.max_response_bytes()
            || policy.reservation_units <= 0
        {
            return Err(Self::refusal("source.model_checkpoint_binding"));
        }
        let mut source = Self::new_bound(factory, capability, schema, binding, binding_capability)?;
        source.checkpoint = Some((
            policy.deployment_binding.to_owned(),
            policy.response_limit,
            policy.reservation_units,
        ));
        Ok(source)
    }

    /// Constructs the additive V6 durable policy route.  Its request-bound
    /// quote is prepared before an intent, while the exact reservation is
    /// admitted only by `propose_policy_checkpointed` against that intent.
    #[allow(clippy::too_many_arguments)]
    pub fn new_bound_checkpointed_with_policy(
        factory: &'a mut dyn SourceAdapterFactory,
        capability: AdapterInvocationCapability,
        schema: &'a CompiledAgentProposalSchema,
        binding: SourceModelBinding,
        binding_capability: SourceModelInvocationCapability,
        checkpoint_policy: SourceProposalPolicy<'_>,
        policy_binding: SourceModelPolicyBinding,
        quoter: &'a mut dyn SourceModelAttemptQuoter,
        cancellation: &'a AgentCancellation,
        clock: &'a dyn InvocationClock,
        deadline_millis: i64,
    ) -> Result<Self, Vec<Diagnostic>> {
        if !policy_binding.matches(&binding) {
            return Err(Self::refusal("source.model_policy"));
        }
        let mut source = Self::new_bound_checkpointed(
            factory,
            capability,
            schema,
            binding,
            binding_capability,
            checkpoint_policy,
        )?;
        source.cancellation = Some(cancellation);
        source.clock = Some(clock);
        source.deadline_millis = Some(deadline_millis);
        source.policy = Some(SourceModelPolicySession::new(
            policy_binding,
            quoter,
            cancellation,
            clock,
            Some(deadline_millis),
        ));
        Ok(source)
    }

    /// Constructs the opt-in source route whose model calls are admitted by
    /// the existing #179 ledger before adapter construction.
    #[allow(clippy::too_many_arguments)]
    pub fn new_bound_with_policy(
        factory: &'a mut dyn SourceAdapterFactory,
        capability: AdapterInvocationCapability,
        schema: &'a CompiledAgentProposalSchema,
        binding: SourceModelBinding,
        binding_capability: SourceModelInvocationCapability,
        policy_binding: SourceModelPolicyBinding,
        quoter: &'a mut dyn SourceModelAttemptQuoter,
        cancellation: &'a AgentCancellation,
        clock: &'a dyn InvocationClock,
        started_at_millis: i64,
    ) -> Result<Self, Vec<Diagnostic>> {
        if !policy_binding.matches(&binding) {
            return Err(Self::refusal("source.model_policy"));
        }
        let limits = policy_binding.effective().limits();
        let deadline_millis = if limits.max_latency_millis == i64::MAX {
            None
        } else {
            Some(
                started_at_millis
                    .checked_add(limits.max_latency_millis)
                    .ok_or_else(|| Self::refusal("source.model_policy"))?,
            )
        };
        let mut source = Self::new_bound(factory, capability, schema, binding, binding_capability)?;
        source.cancellation = Some(cancellation);
        source.clock = Some(clock);
        source.deadline_millis = deadline_millis;
        source.policy = Some(SourceModelPolicySession::new(
            policy_binding,
            quoter,
            cancellation,
            clock,
            deadline_millis,
        ));
        Ok(source)
    }

    /// Checks that this adapter is still attached to the exact runtime
    /// binding. It performs no host operation and exposes no adapter.
    pub fn validate_model_binding(
        &self,
        binding: &SourceModelBinding,
    ) -> Result<(), Vec<Diagnostic>> {
        if self.binding.as_ref() != Some(binding)
            || self
                .binding_capability
                .as_ref()
                .is_none_or(|value| !binding.capability_matches(value))
            || self.schema.schema().digest() != binding.proposal_schema_digest()
            || self.schema.source_revision() != binding.source_revision()
        {
            return Err(Self::refusal("source.model_binding"));
        }
        Ok(())
    }

    /// Returns only bounded digests, lengths, closed terminal tags and usage
    /// observations. Prompt and response bytes are never retained here.
    #[must_use]
    pub fn model_evidence(&self) -> &SourceModelEvidence {
        &self.evidence
    }

    /// Exposes only the checked binding facts needed by the consuming Direct
    /// Runtime producer to reject cross-runtime substitution before dispatch.
    #[must_use]
    pub fn model_binding(&self) -> Option<&SourceModelBinding> {
        self.binding.as_ref()
    }

    #[must_use]
    pub fn model_policy_binding(&self) -> Option<&SourceModelPolicyBinding> {
        self.policy.as_ref().map(SourceModelPolicySession::binding)
    }
    pub(crate) fn ordinary_checkpoint_matches(&self, policy: &SourceProposalPolicy<'_>) -> bool {
        self.binding.is_some()
            && self.binding_capability.is_some()
            && self.policy.is_none()
            && self
                .checkpoint
                .as_ref()
                .is_some_and(|(deployment, response, units)| {
                    deployment == policy.deployment_binding
                        && *response == policy.response_limit
                        && *units == policy.reservation_units
                })
    }
    #[must_use]
    pub fn with_cancellation(mut self, cancellation: &'a AgentCancellation) -> Self {
        self.cancellation = Some(cancellation);
        self
    }
    #[must_use]
    pub fn with_cancellation_and_deadline(
        mut self,
        cancellation: &'a AgentCancellation,
        clock: &'a dyn InvocationClock,
        deadline_millis: i64,
    ) -> Self {
        self.cancellation = Some(cancellation);
        self.clock = Some(clock);
        self.deadline_millis = Some(deadline_millis);
        self
    }

    /// Attaches the caller-owned Source Live cancellation/deadline boundary
    /// for one durable run. The cancellation handle is cloned atomically; the
    /// clock itself remains borrowed only across `propose_checkpointed`.
    pub(crate) fn configure_durable_boundary(
        &mut self,
        cancellation: &AgentCancellation,
        deadline_millis: i64,
    ) {
        self.durable_cancellation = Some(cancellation.clone());
        self.durable_deadline_millis = Some(deadline_millis);
    }

    pub(crate) fn model_policy_deadline_matches(&self, deadline_millis: i64) -> bool {
        self.policy
            .as_ref()
            .is_none_or(|policy| policy.deadline_millis() == Some(deadline_millis))
    }
    fn refusal(code: &'static str) -> Vec<Diagnostic> {
        vec![Diagnostic::io(
            code,
            "provider adapter source proposal refused",
        )]
    }

    fn record(
        &mut self,
        request: &[u8],
        response: Option<&[u8]>,
        terminal: &str,
        tokens_in: Option<u64>,
        tokens_out: Option<u64>,
        cost_micros: Option<i64>,
    ) {
        self.last_dispatch = Some(SourceAdapterDispatchFact::Failed {
            reason: failure_from_terminal(terminal),
            attempted_bytes: response.map_or(0, <[u8]>::len),
        });
        if self.binding.is_some() {
            let reservation = self.pending_reservation.take();
            self.evidence.record(
                request,
                response,
                terminal,
                tokens_in,
                tokens_out,
                cost_micros,
                reservation.as_ref(),
            );
        }
    }

    /// Charges only a provider observation that passed the stream's monotonic
    /// validation.  Reservations remain spent even if no such observation
    /// arrives; an observed excess can only reduce later admission headroom.
    fn observe_pending_usage(&mut self, tokens_in: u64, tokens_out: u64, cost_micros: i64) {
        let reservation = self.pending_reservation.clone();
        if let (Some(policy), Some(reservation)) = (self.policy.as_mut(), reservation.as_ref()) {
            policy.observe(reservation, tokens_in, tokens_out, cost_micros);
        }
    }

    fn dispatch(&mut self, request: ProposalRequest<'_>) -> SourceAdapterDispatch {
        self.dispatch_at(request, None)
    }

    fn dispatch_at(
        &mut self,
        request: ProposalRequest<'_>,
        source_clock: Option<&dyn SourceInvocationClock>,
    ) -> SourceAdapterDispatch {
        self.dispatch_with_policy(request, source_clock, false)
    }

    fn dispatch_reserved_at(
        &mut self,
        request: ProposalRequest<'_>,
        source_clock: Option<&dyn SourceInvocationClock>,
    ) -> SourceAdapterDispatch {
        self.dispatch_with_policy(request, source_clock, true)
    }

    fn dispatch_with_policy(
        &mut self,
        request: ProposalRequest<'_>,
        source_clock: Option<&dyn SourceInvocationClock>,
        policy_already_reserved: bool,
    ) -> SourceAdapterDispatch {
        self.last_dispatch = None;
        match self.propose_inner(request, source_clock, policy_already_reserved) {
            Ok(canonical) => match self.last_dispatch.take() {
                Some(SourceAdapterDispatchFact::Settled { response, usage }) => {
                    SourceAdapterDispatch::Settled {
                        canonical,
                        response,
                        usage,
                    }
                }
                _ => SourceAdapterDispatch::Failed {
                    diagnostics: Self::refusal("source.adapter_settlement"),
                    reason: SourceAttemptFailure::Refused,
                    attempted_bytes: 0,
                },
            },
            Err(diagnostics) => match self.last_dispatch.take() {
                Some(SourceAdapterDispatchFact::Failed {
                    reason,
                    attempted_bytes,
                }) => SourceAdapterDispatch::Failed {
                    diagnostics,
                    reason,
                    attempted_bytes,
                },
                _ => SourceAdapterDispatch::Failed {
                    diagnostics,
                    reason: SourceAttemptFailure::Refused,
                    attempted_bytes: 0,
                },
            },
        }
    }

    fn checked_adapter_request(
        &self,
        request: &ProposalRequest<'_>,
    ) -> Result<AdapterRequest, Vec<Diagnostic>> {
        if request.proposal_schema_digest != self.schema.schema().digest()
            || request.source_revision != self.schema.source_revision()
        {
            return Err(Self::refusal("source.adapter_schema_drift"));
        }
        let max_request_bytes = self
            .binding
            .as_ref()
            .map_or(MAX_STREAM_BYTES, SourceModelBinding::max_request_bytes);
        let max_response_bytes = self
            .binding
            .as_ref()
            .map_or(MAX_STREAM_BYTES, SourceModelBinding::max_response_bytes);
        let schema = self.schema.schema().canonical_json();
        if schema.len() > max_request_bytes || request.task.objective.len() > max_request_bytes {
            return Err(Self::refusal("source.adapter_request_bound"));
        }
        if request
            .previous_effect
            .is_some_and(|bytes| bytes.len() > MAX_STREAM_BYTES)
            || request
                .previous_rejection
                .is_some_and(|text| text.len() > MAX_STREAM_BYTES)
            || !bounded_retained(request.state)
            || !bounded_retained(request.observation)
        {
            return Err(Self::refusal("source.adapter_request_bound"));
        }
        let prompt = canonical_prompt(request, schema);
        if prompt.len() > max_request_bytes {
            return Err(Self::refusal("source.adapter_request_bound"));
        }
        Ok(AdapterRequest {
            request_bytes: prompt.into_bytes(),
            max_response_bytes,
        })
    }

    fn dispatch_failure(&mut self, reason: SourceAttemptFailure, attempted_bytes: usize) {
        self.last_dispatch = Some(SourceAdapterDispatchFact::Failed {
            reason,
            attempted_bytes,
        });
    }

    fn deadline_failure(&self) -> SourceAttemptFailure {
        if self.cancellation.is_some_and(|value| value.is_cancelled())
            || self
                .durable_cancellation
                .as_ref()
                .is_some_and(|value| value.is_cancelled())
        {
            SourceAttemptFailure::Cancelled
        } else {
            SourceAttemptFailure::DeadlineExceeded
        }
    }

    fn check_deadline_at(
        &self,
        source_clock: Option<&dyn SourceInvocationClock>,
    ) -> Result<(), Vec<Diagnostic>> {
        if self.cancellation.is_some_and(|value| value.is_cancelled())
            || self
                .durable_cancellation
                .as_ref()
                .is_some_and(|value| value.is_cancelled())
        {
            return Err(Self::refusal("source.adapter_cancelled"));
        }
        if self
            .clock
            .zip(self.deadline_millis)
            .is_some_and(|(clock, deadline)| clock.now_millis() >= deadline)
            || source_clock
                .zip(self.durable_deadline_millis)
                .is_some_and(|(clock, deadline)| clock.now_millis() >= deadline)
        {
            return Err(Self::refusal("source.adapter_timeout"));
        }
        Ok(())
    }
}

impl ProposalSource for StreamingSourceProposalAdapter<'_> {
    fn checkpoint_policy(&self) -> Option<SourceProposalPolicy<'_>> {
        self.checkpoint
            .as_ref()
            .map(
                |(binding, response_limit, reservation_units)| SourceProposalPolicy {
                    deployment_binding: binding,
                    response_limit: *response_limit,
                    reservation_units: *reservation_units,
                },
            )
    }

    fn checkpoint_attempt_identity(
        &self,
        request: &ProposalRequest<'_>,
    ) -> Result<SourceAttemptIdentity, Vec<Diagnostic>> {
        self.check_deadline()?;
        if self.binding.is_some() && !self.evidence.can_record() {
            return Err(Self::refusal("source.model_evidence_capacity"));
        }
        let adapter_request = self.checked_adapter_request(request)?;
        let digest = source_request_digest(&adapter_request.request_bytes);
        Ok(SourceAttemptIdentity {
            request_digest: digest.clone(),
            prompt_digest: digest,
            request_bytes: adapter_request.request_bytes.len(),
        })
    }

    fn policy_quote_checkpointed(
        &mut self,
        request: &ProposalRequest<'_>,
        identity: &SourceAttemptIdentity,
    ) -> Result<Option<SourcePolicyQuoteV6>, Vec<Diagnostic>> {
        if self.policy.is_none() {
            return Ok(None);
        }
        self.check_deadline()?;
        let adapter_request = self.checked_adapter_request(request)?;
        if source_request_digest(&adapter_request.request_bytes) != identity.request_digest {
            return Err(Self::refusal("source.model_policy_identity"));
        }
        let binding = self
            .binding
            .as_ref()
            .ok_or_else(|| Self::refusal("source.model_binding"))?;
        let quote = {
            let policy = self.policy.as_mut().expect("checked source policy");
            policy.quote_v6(&adapter_request, binding)
        };
        let quote = match quote {
            Ok(quote) => quote,
            Err(terminal) => {
                self.record(
                    &adapter_request.request_bytes,
                    None,
                    terminal,
                    None,
                    None,
                    None,
                );
                return Err(Self::refusal("source.model_policy"));
            }
        };
        Ok(Some(quote))
    }

    fn propose_policy_checkpointed(
        &mut self,
        request: ProposalRequest<'_>,
        quote: &SourcePolicyQuoteV6,
        intent: &PolicyAttemptIntentV6,
        sink: &mut SourceCheckpointSink<'_>,
        _ledger: &mut CumulativeBudgetLedger<'_>,
        clock: &dyn SourceInvocationClock,
    ) -> SourceProposalOutcome {
        let identity = match self.checkpoint_attempt_identity(&request) {
            Ok(identity) => identity,
            Err(errors) => {
                return SourceProposalOutcome {
                    terminal_failure: None,
                    result: Err(errors),
                    model_dispatches: 0,
                }
            }
        };
        let adapter_request = match self.checked_adapter_request(&request) {
            Ok(request) => request,
            Err(errors) => {
                return SourceProposalOutcome {
                    terminal_failure: None,
                    result: Err(errors),
                    model_dispatches: 0,
                }
            }
        };
        let (response_limit, reserved_units) = match &self.checkpoint {
            Some((_, response_limit, reserved_units)) => (*response_limit, *reserved_units),
            None => {
                return SourceProposalOutcome {
                    terminal_failure: None,
                    result: Err(Self::refusal("source.model_checkpoint_binding")),
                    model_dispatches: 0,
                }
            }
        };
        let reservation = {
            let Some(policy) = self.policy.as_mut() else {
                return SourceProposalOutcome {
                    terminal_failure: None,
                    result: Err(Self::refusal("source.model_policy")),
                    model_dispatches: 0,
                };
            };
            if quote.policy_binding_digest != policy.binding().digest()
                || quote.request_digest != identity.request_digest
                || quote.provider_id != policy.binding().provider_id()
                || quote
                    .context_tokens
                    .checked_add(quote.output_tokens)
                    .is_none()
                || quote.estimated_cost_micros < 0
                || intent.turn != request.turn as u32
                || intent.attempt != request.attempt as u32
                || intent.request_digest != identity.request_digest
                || intent.prompt_digest != identity.prompt_digest
                || intent.request_bytes != identity.request_bytes
                || intent.reserved_units != reserved_units
                || intent.response_limit != response_limit
                || intent.request_digest != source_request_digest(&adapter_request.request_bytes)
            {
                return SourceProposalOutcome {
                    terminal_failure: None,
                    result: Err(Self::refusal("source.model_policy_identity")),
                    model_dispatches: 0,
                };
            }
            policy.reserve(&ModelAttemptQuote {
                request_digest: quote.request_digest.clone(),
                context_tokens: quote.context_tokens,
                output_tokens: quote.output_tokens,
                estimated_cost_micros: quote.estimated_cost_micros,
            })
        };
        let reservation = match reservation {
            Ok(reservation) if reservation_matches_v6(&reservation, &intent.reservation) => {
                reservation
            }
            Ok(_) | Err(_) => {
                self.record(
                    &adapter_request.request_bytes,
                    None,
                    "policy_reservation_refused",
                    None,
                    None,
                    None,
                );
                return SourceProposalOutcome {
                    terminal_failure: None,
                    result: Err(Self::refusal("source.model_policy")),
                    model_dispatches: 0,
                };
            }
        };
        self.pending_reservation = Some(reservation);
        if sink
            .append_at(
                SourceJournalEntry::PolicyAttemptIntent(intent.clone()),
                clock.now_millis(),
            )
            .is_err()
        {
            return SourceProposalOutcome {
                terminal_failure: None,
                result: Err(Self::refusal("source.model_checkpoint")),
                model_dispatches: 0,
            };
        }
        let result = self.dispatch_reserved_at(request, Some(clock));
        let (settlement, usage) = match &result {
            SourceAdapterDispatch::Settled {
                response, usage, ..
            } => (
                SourceJournalEntry::AttemptSettled {
                    turn: intent.turn,
                    attempt: intent.attempt,
                    response: response.clone(),
                    response_digest: source_response_digest(response),
                },
                usage.map_or(PolicyAttemptUsageV6::Unknown, |(input, output, cost)| {
                    PolicyAttemptUsageV6::Observed {
                        context_tokens: input,
                        output_tokens: output,
                        cost_micros: cost,
                    }
                }),
            ),
            SourceAdapterDispatch::Failed {
                reason,
                attempted_bytes,
                ..
            } => (
                SourceJournalEntry::AttemptFailed {
                    turn: intent.turn,
                    attempt: intent.attempt,
                    reason: *reason,
                    attempted_bytes: *attempted_bytes,
                },
                PolicyAttemptUsageV6::Unknown,
            ),
        };
        if sink.append_at(settlement, clock.now_millis()).is_err() {
            return SourceProposalOutcome {
                terminal_failure: None,
                result: Err(Self::refusal("source.model_checkpoint")),
                model_dispatches: 1,
            };
        }
        let usage = match sink.policy_attempt_usage(
            intent.turn,
            intent.attempt,
            intent.reservation.ordinal,
            usage,
        ) {
            Ok(usage) => usage,
            Err(_) => {
                return SourceProposalOutcome {
                    terminal_failure: None,
                    result: Err(Self::refusal("source.model_checkpoint")),
                    model_dispatches: 1,
                }
            }
        };
        if sink.append_at(usage, clock.now_millis()).is_err() {
            return SourceProposalOutcome {
                terminal_failure: None,
                result: Err(Self::refusal("source.model_checkpoint")),
                model_dispatches: 1,
            };
        }
        let result = match result {
            SourceAdapterDispatch::Settled { canonical, .. } => Ok(canonical),
            SourceAdapterDispatch::Failed { diagnostics, .. } => Err(diagnostics),
        };
        SourceProposalOutcome {
            terminal_failure: None,
            result,
            model_dispatches: 1,
        }
    }

    fn restore_policy_checkpointed(
        &mut self,
        reservations: &[PolicyAttemptReservationV6],
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(policy) = self.policy.as_mut() else {
            return if reservations.is_empty() {
                Ok(())
            } else {
                Err(Self::refusal("source.model_policy_recovery"))
            };
        };
        policy.resume(reservations).map_err(Self::refusal)
    }

    fn propose_checkpointed(
        &mut self,
        request: ProposalRequest<'_>,
        sink: &mut SourceCheckpointSink<'_>,
        _ledger: &mut CumulativeBudgetLedger<'_>,
        clock: &dyn SourceInvocationClock,
    ) -> SourceProposalOutcome {
        let turn = request.turn as u32;
        let attempt = request.attempt as u32;
        let identity = match self.checkpoint_attempt_identity(&request) {
            Ok(value) => value,
            Err(errors) => {
                return SourceProposalOutcome {
                    terminal_failure: None,
                    result: Err(errors),
                    model_dispatches: 0,
                }
            }
        };
        let intent = match sink.attempt_intent(
            turn,
            attempt,
            identity.request_digest,
            identity.prompt_digest,
            identity.request_bytes,
        ) {
            Ok(value) => value,
            Err(_) => {
                return SourceProposalOutcome {
                    terminal_failure: None,
                    result: Err(Self::refusal("source.model_checkpoint")),
                    model_dispatches: 0,
                }
            }
        };
        if sink.append_at(intent, clock.now_millis()).is_err() {
            return SourceProposalOutcome {
                terminal_failure: None,
                result: Err(Self::refusal("source.model_checkpoint")),
                model_dispatches: 0,
            };
        }
        let result = self.dispatch_at(request, Some(clock));
        let entry = match &result {
            SourceAdapterDispatch::Settled { response, .. } => SourceJournalEntry::AttemptSettled {
                turn,
                attempt,
                response: response.clone(),
                response_digest: source_response_digest(response),
            },
            SourceAdapterDispatch::Failed {
                reason,
                attempted_bytes,
                ..
            } => SourceJournalEntry::AttemptFailed {
                turn,
                attempt,
                reason: *reason,
                attempted_bytes: *attempted_bytes,
            },
        };
        if sink.append_at(entry, clock.now_millis()).is_err() {
            return SourceProposalOutcome {
                terminal_failure: None,
                result: Err(Self::refusal("source.model_checkpoint")),
                model_dispatches: 1,
            };
        }
        let result = match result {
            SourceAdapterDispatch::Settled { canonical, .. } => Ok(canonical),
            SourceAdapterDispatch::Failed { diagnostics, .. } => Err(diagnostics),
        };
        SourceProposalOutcome {
            terminal_failure: None,
            result,
            model_dispatches: 1,
        }
    }

    fn check_deadline(&self) -> Result<(), Vec<Diagnostic>> {
        self.check_deadline_at(None)
    }
    fn propose(&mut self, request: ProposalRequest<'_>) -> Result<String, Vec<Diagnostic>> {
        match self.dispatch(request) {
            SourceAdapterDispatch::Settled { canonical, .. } => Ok(canonical),
            SourceAdapterDispatch::Failed { diagnostics, .. } => Err(diagnostics),
        }
    }
}

impl StreamingSourceProposalAdapter<'_> {
    fn propose_inner(
        &mut self,
        request: ProposalRequest<'_>,
        source_clock: Option<&dyn SourceInvocationClock>,
        policy_already_reserved: bool,
    ) -> Result<String, Vec<Diagnostic>> {
        if let Err(diagnostics) = self.check_deadline_at(source_clock) {
            self.dispatch_failure(self.deadline_failure(), 0);
            return Err(diagnostics);
        }
        let adapter_request = self.checked_adapter_request(&request)?;
        let max_response_bytes = adapter_request.max_response_bytes;
        if let Err(diagnostics) = self.check_deadline_at(source_clock) {
            self.dispatch_failure(self.deadline_failure(), 0);
            return Err(diagnostics);
        }
        if self.binding.is_some() && !self.evidence.can_record() {
            return Err(Self::refusal("source.model_evidence_capacity"));
        }
        if !policy_already_reserved {
            let policy_attempt = if let (Some(policy), Some(binding)) =
                (self.policy.as_mut(), self.binding.as_ref())
            {
                policy
                    .quote(&adapter_request, binding)
                    .and_then(|quote| {
                        policy
                            .reserve(&quote)
                            .map_err(|_| "policy_reservation_refused")
                    })
                    .map(Some)
            } else {
                Ok(None)
            };
            let policy_reservation = match policy_attempt {
                Ok(reservation) => reservation,
                Err(terminal) => {
                    self.record(
                        &adapter_request.request_bytes,
                        None,
                        terminal,
                        None,
                        None,
                        None,
                    );
                    return Err(Self::refusal("source.model_policy"));
                }
            };
            self.pending_reservation = policy_reservation;
        }
        let mut adapter = self.factory.create();
        if self
            .binding
            .as_ref()
            .is_some_and(|binding| !binding.adapter_matches(adapter.capabilities()))
        {
            self.record(
                &adapter_request.request_bytes,
                None,
                "adapter_identity_refused",
                None,
                None,
                None,
            );
            return Err(Self::refusal("source.model_adapter_identity"));
        }
        let required = RequiredCapabilities {
            require_streaming: true,
            require_structured_output_mode: Some(StructuredOutputMode::RawText),
            max_request_bytes: adapter_request.request_bytes.len(),
            max_response_bytes,
        };
        if negotiate(adapter.capabilities(), &required).is_err() {
            self.record(
                &adapter_request.request_bytes,
                None,
                "adapter_negotiation_refused",
                None,
                None,
                None,
            );
            return Err(Self::refusal("source.adapter_negotiation"));
        }
        if let Err(diagnostics) = self.check_deadline_at(source_clock) {
            self.record(
                &adapter_request.request_bytes,
                None,
                "cancelled_or_timed_out",
                None,
                None,
                None,
            );
            self.dispatch_failure(self.deadline_failure(), 0);
            return Err(diagnostics);
        }
        if adapter.start(&self.capability, &adapter_request).is_err() {
            adapter.cancel("source start refusal");
            self.record(
                &adapter_request.request_bytes,
                None,
                "adapter_start_refused",
                None,
                None,
                None,
            );
            return Err(Self::refusal("source.adapter_negotiation"));
        }
        let mut decoder = SourceProposalStreamDecoder::new(self.schema);
        let mut bytes = Vec::new();
        let mut completed = false;
        let mut usage: Option<(u64, u64, i64)> = None;
        for _ in 0..MAX_SOURCE_POLLS {
            if let Err(diagnostics) = self.check_deadline_at(source_clock) {
                adapter.cancel("source cancellation or deadline");
                self.record(
                    &adapter_request.request_bytes,
                    Some(&bytes),
                    "cancelled_or_timed_out",
                    usage.map(|value| value.0),
                    usage.map(|value| value.1),
                    usage.map(|value| value.2),
                );
                self.dispatch_failure(self.deadline_failure(), bytes.len());
                return Err(diagnostics);
            }
            match adapter.poll() {
                AdapterPoll::Pending => continue,
                AdapterPoll::Event(AdapterEvent::Delta(chunk)) => {
                    if completed {
                        adapter.cancel("source stream after completion");
                        self.record(
                            &adapter_request.request_bytes,
                            Some(&bytes),
                            "stream_refused",
                            usage.map(|value| value.0),
                            usage.map(|value| value.1),
                            usage.map(|value| value.2),
                        );
                        self.dispatch_failure(SourceAttemptFailure::MalformedResponse, bytes.len());
                        return Err(Self::refusal("source.adapter_stream"));
                    }
                    if bytes.len().saturating_add(chunk.len()) > max_response_bytes {
                        adapter.cancel("source stream bound");
                        self.record(
                            &adapter_request.request_bytes,
                            Some(&bytes),
                            "stream_refused",
                            usage.map(|value| value.0),
                            usage.map(|value| value.1),
                            usage.map(|value| value.2),
                        );
                        self.dispatch_failure(
                            SourceAttemptFailure::CapacityExceeded,
                            bytes
                                .len()
                                .saturating_add(chunk.len())
                                .min(max_response_bytes.saturating_add(1)),
                        );
                        return Err(Self::refusal("source.adapter_stream"));
                    }
                    bytes.extend_from_slice(&chunk);
                    if matches!(decoder.push(&chunk), SourcePushOutcome::Refused(_)) {
                        adapter.cancel("source decoder refusal");
                        self.record(
                            &adapter_request.request_bytes,
                            Some(&bytes),
                            "decode_refused",
                            usage.map(|value| value.0),
                            usage.map(|value| value.1),
                            usage.map(|value| value.2),
                        );
                        self.dispatch_failure(SourceAttemptFailure::MalformedResponse, bytes.len());
                        return Err(Self::refusal("source.adapter_decode"));
                    }
                }
                AdapterPoll::Event(AdapterEvent::Completed) => {
                    if completed {
                        adapter.cancel("duplicate completion");
                        self.record(
                            &adapter_request.request_bytes,
                            Some(&bytes),
                            "stream_refused",
                            usage.map(|value| value.0),
                            usage.map(|value| value.1),
                            usage.map(|value| value.2),
                        );
                        self.dispatch_failure(SourceAttemptFailure::MalformedResponse, bytes.len());
                        return Err(Self::refusal("source.adapter_stream"));
                    }
                    completed = true;
                }
                AdapterPoll::Event(AdapterEvent::Usage {
                    tokens_in,
                    tokens_out,
                    cost_micros,
                }) => {
                    if completed
                        || cost_micros < 0
                        || usage.is_some_and(|prior| {
                            tokens_in < prior.0 || tokens_out < prior.1 || cost_micros < prior.2
                        })
                    {
                        adapter.cancel("invalid usage");
                        self.record(
                            &adapter_request.request_bytes,
                            Some(&bytes),
                            "usage_refused",
                            usage.map(|value| value.0),
                            usage.map(|value| value.1),
                            usage.map(|value| value.2),
                        );
                        self.dispatch_failure(SourceAttemptFailure::MalformedResponse, bytes.len());
                        return Err(Self::refusal("source.adapter_usage"));
                    }
                    usage = Some((tokens_in, tokens_out, cost_micros));
                    self.observe_pending_usage(tokens_in, tokens_out, cost_micros);
                }
                AdapterPoll::Settled(settlement) => {
                    if let Err(diagnostics) = self.check_deadline_at(source_clock) {
                        adapter.cancel("source cancellation or deadline at settlement");
                        self.record(
                            &adapter_request.request_bytes,
                            Some(&bytes),
                            "cancelled_or_timed_out",
                            settlement.usage.tokens_in,
                            settlement.usage.tokens_out,
                            settlement.usage.cost_micros,
                        );
                        self.dispatch_failure(self.deadline_failure(), bytes.len());
                        return Err(diagnostics);
                    }
                    if !completed
                        || settlement.response_bytes != bytes
                        || settlement.usage.cost_micros.is_some_and(|cost| cost < 0)
                        || usage.is_some_and(|prior| {
                            settlement.usage.tokens_in.is_some_and(|v| v < prior.0)
                                || settlement.usage.tokens_out.is_some_and(|v| v < prior.1)
                                || settlement.usage.cost_micros.is_some_and(|v| v < prior.2)
                        })
                    {
                        adapter.cancel("settlement mismatch");
                        self.record(
                            &adapter_request.request_bytes,
                            Some(&bytes),
                            "settlement_refused",
                            settlement.usage.tokens_in,
                            settlement.usage.tokens_out,
                            settlement.usage.cost_micros,
                        );
                        self.dispatch_failure(SourceAttemptFailure::MalformedResponse, bytes.len());
                        return Err(Self::refusal("source.adapter_stream"));
                    }
                    if let (Some(tokens_in), Some(tokens_out), Some(cost_micros)) = (
                        settlement.usage.tokens_in,
                        settlement.usage.tokens_out,
                        settlement.usage.cost_micros,
                    ) {
                        self.observe_pending_usage(tokens_in, tokens_out, cost_micros);
                    }
                    return match decoder.finish() {
                        SourcePushOutcome::Accepted(value) => {
                            self.record(
                                &adapter_request.request_bytes,
                                Some(&bytes),
                                "admitted",
                                settlement.usage.tokens_in,
                                settlement.usage.tokens_out,
                                settlement.usage.cost_micros,
                            );
                            self.last_dispatch = Some(SourceAdapterDispatchFact::Settled {
                                response: bytes,
                                usage: match (
                                    settlement.usage.tokens_in,
                                    settlement.usage.tokens_out,
                                    settlement.usage.cost_micros,
                                ) {
                                    (Some(tokens_in), Some(tokens_out), Some(cost_micros)) => {
                                        Some((tokens_in, tokens_out, cost_micros))
                                    }
                                    _ => None,
                                },
                            });
                            Ok(value.canonical_json().to_owned())
                        }
                        _ => {
                            self.record(
                                &adapter_request.request_bytes,
                                Some(&bytes),
                                "decode_refused",
                                settlement.usage.tokens_in,
                                settlement.usage.tokens_out,
                                settlement.usage.cost_micros,
                            );
                            self.dispatch_failure(
                                SourceAttemptFailure::MalformedResponse,
                                bytes.len(),
                            );
                            Err(Self::refusal("source.adapter_decode"))
                        }
                    };
                }
                AdapterPoll::Failed {
                    failure,
                    attempted_bytes,
                } => {
                    adapter.cancel("source adapter failed");
                    self.record(
                        &adapter_request.request_bytes,
                        Some(&bytes),
                        failure.as_str(),
                        usage.map(|value| value.0),
                        usage.map(|value| value.1),
                        usage.map(|value| value.2),
                    );
                    self.dispatch_failure(
                        source_failure_from_model(failure),
                        attempted_bytes.min(max_response_bytes.saturating_add(1)),
                    );
                    return Err(Self::refusal("source.adapter_failed"));
                }
            }
        }
        adapter.cancel("source adapter poll budget");
        self.record(
            &adapter_request.request_bytes,
            Some(&bytes),
            "poll_budget_exhausted",
            usage.map(|value| value.0),
            usage.map(|value| value.1),
            usage.map(|value| value.2),
        );
        self.dispatch_failure(SourceAttemptFailure::Timeout, bytes.len());
        Err(Self::refusal("source.adapter_timeout"))
    }
}

fn canonical_prompt(request: &ProposalRequest<'_>, schema: &str) -> String {
    format!(
        "{{\"schema\":\"semaprax.source-adapter-prompt.v1\",\"task_hex\":{},\"task_budget\":{},\"source_revision\":{},\"turn\":{},\"attempt\":{},\"remaining_iterations\":{},\"state\":{},\"observation\":{},\"previous_effect_hex\":{},\"previous_rejection\":{},\"proposal_schema\":{}}}",
        quote_json(&hex(&request.task.objective)),
        request.task.budget,
        quote_json(request.source_revision),
        request.turn,
        request.attempt,
        request.remaining_iterations,
        crate::agent_lifecycle::canonical_retained_value_json(request.state),
        crate::agent_lifecycle::canonical_retained_value_json(request.observation),
        request
            .previous_effect
            .map(hex)
            .map_or_else(|| "null".to_owned(), |value| quote_json(&value)),
        request
            .previous_rejection
            .map_or_else(|| "null".to_owned(), quote_json),
        schema,
    )
}

fn failure_from_terminal(terminal: &str) -> SourceAttemptFailure {
    match terminal {
        "poll_budget_exhausted" => SourceAttemptFailure::Timeout,
        "cancelled_or_timed_out" => SourceAttemptFailure::DeadlineExceeded,
        "stream_refused" | "decode_refused" | "usage_refused" | "settlement_refused" => {
            SourceAttemptFailure::MalformedResponse
        }
        _ => SourceAttemptFailure::Refused,
    }
}

fn reservation_matches_v6(
    reservation: &AttemptReservation,
    durable: &PolicyAttemptReservationV6,
) -> bool {
    reservation.ordinal == durable.ordinal
        && reservation.kind == durable.kind
        && reservation.provider_id == durable.provider_id
        && reservation.reserved_context_tokens == durable.reserved_context_tokens
        && reservation.reserved_output_tokens == durable.reserved_output_tokens
        && reservation.reserved_cost_micros == durable.reserved_cost_micros
}

fn source_failure_from_model(failure: ModelFailure) -> SourceAttemptFailure {
    match failure {
        ModelFailure::Timeout => SourceAttemptFailure::Timeout,
        ModelFailure::Cancelled => SourceAttemptFailure::Cancelled,
        ModelFailure::CapacityExceeded => SourceAttemptFailure::CapacityExceeded,
        ModelFailure::ProviderError => SourceAttemptFailure::ProviderError,
        ModelFailure::MalformedResponse => SourceAttemptFailure::MalformedResponse,
        ModelFailure::Refused => SourceAttemptFailure::Refused,
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        output.push(DIGITS[usize::from(byte >> 4)] as char);
        output.push(DIGITS[usize::from(byte & 15)] as char);
    }
    output
}

// Refuse hostile host-constructed carriers before the recursive canonical
// renderer allocates. This is a conservative byte upper bound, not a second
// retained-value serializer; the lifecycle renderer still owns exact bytes.
fn bounded_retained(value: &crate::interpreter::retained_call::RetainedValue) -> bool {
    use crate::interpreter::retained_call::RetainedValue;
    fn visit(value: &RetainedValue, depth: usize, remaining: &mut usize) -> bool {
        if depth > 64 {
            return false;
        }
        let (overhead, fields) = match value {
            RetainedValue::Bytes(bytes) => (bytes.len().saturating_mul(2).saturating_add(16), None),
            RetainedValue::Record(record) => (
                record
                    .record
                    .as_str()
                    .len()
                    .saturating_mul(6)
                    .saturating_add(32),
                Some(record.fields.as_slice()),
            ),
            RetainedValue::Variant(variant) => (
                variant
                    .variant
                    .as_str()
                    .len()
                    .saturating_add(variant.case.as_str().len())
                    .saturating_mul(6)
                    .saturating_add(48),
                Some(variant.fields.as_slice()),
            ),
            _ => (24, None),
        };
        let Some(left) = remaining.checked_sub(overhead) else {
            return false;
        };
        *remaining = left;
        if let Some(fields) = fields {
            for field in fields {
                let Some(left) = remaining.checked_sub(
                    field
                        .field
                        .as_str()
                        .len()
                        .saturating_mul(6)
                        .saturating_add(32),
                ) else {
                    return false;
                };
                *remaining = left;
                if !visit(&field.value, depth + 1, remaining) {
                    return false;
                }
            }
        }
        true
    }
    let mut remaining = MAX_STREAM_BYTES;
    visit(value, 0, &mut remaining)
}

#[cfg(test)]
mod tests;
