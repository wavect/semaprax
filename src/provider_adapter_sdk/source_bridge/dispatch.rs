//! Shared private SDK executor. The old path has no live guard; v8 supplies
//! only its sealed actual Intent permit, never a caller authority callback.
use super::*;
use crate::live_invocation::source_journal::LiveModelIntentPermitV8;
impl StreamingSourceProposalAdapter<'_> {
    fn check_deadline_live_v8(
        &self,
        clock: Option<&dyn SourceInvocationClock>,
        live: Option<&LiveModelIntentPermitV8<'_>>,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(live) = live else {
            return self.check_deadline_at(clock);
        };
        let guard = || {
            live.validate_guard()
                .map_err(|_| Self::refusal("source.owned_wait_guard"))
        };
        guard()?;
        if self.cancellation.is_some_and(|c| c.is_cancelled())
            || self
                .durable_cancellation
                .as_ref()
                .is_some_and(|c| c.is_cancelled())
        {
            return Err(Self::refusal("source.adapter_cancelled"));
        }
        if let Some((clock, deadline)) = self.clock.zip(self.deadline_millis) {
            let now = clock.now_millis();
            guard()?;
            if now >= deadline {
                return Err(Self::refusal("source.adapter_timeout"));
            }
        }
        if let Some((clock, deadline)) = clock.zip(self.durable_deadline_millis) {
            let now = clock.now_millis();
            guard()?;
            if now >= deadline {
                return Err(Self::refusal("source.adapter_timeout"));
            }
        }
        guard()
    }
    fn propose_adapter_inner(
        &mut self,
        adapter_request: AdapterRequest,
        source_clock: Option<&dyn SourceInvocationClock>,
        policy_already_reserved: bool,
        live: Option<&LiveModelIntentPermitV8<'_>>,
    ) -> Result<String, Vec<Diagnostic>> {
        macro_rules! live_guard {
            ($bytes:expr) => {
                if let Some(live) = live {
                    if live.validate_guard().is_err() {
                        if self.last_dispatch.is_none() {
                            self.dispatch_failure(live.guard_failure(), $bytes);
                        }
                        return Err(Self::refusal("source.owned_wait_guard"));
                    }
                }
            };
        }
        macro_rules! cancel_guarded {
            ($adapter:expr, $reason:expr, $failure:expr, $attempted:expr) => {
                if let Some(live) = live {
                    // Select before the callback. Losing authority or a callback
                    // panic quarantines the owner without replacing that failure.
                    self.dispatch_failure($failure, $attempted);
                    if live.validate_store().is_err() {
                        return Err(Self::refusal("source.owned_wait_guard"));
                    }
                    let cancelled = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        $adapter.cancel($reason)
                    }));
                    if live.validate_store().is_err() || cancelled.is_err() {
                        return Err(Self::refusal("source.owned_wait_guard"));
                    }
                } else {
                    $adapter.cancel($reason);
                }
            };
        }
        let max_response_bytes = adapter_request.max_response_bytes;
        if let Err(diagnostics) = self.check_deadline_live_v8(source_clock, live) {
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
        live_guard!(0);
        let mut adapter = self.factory.create();
        live_guard!(0);
        let identity_refused = if let Some(binding) = self.binding.as_ref() {
            let capabilities = adapter.capabilities();
            let matches = binding.adapter_matches(capabilities);
            live_guard!(0);
            !matches
        } else {
            false
        };
        if identity_refused {
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
        let capabilities = adapter.capabilities();
        live_guard!(0);
        if negotiate(capabilities, &required).is_err() {
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
        if let Err(diagnostics) = self.check_deadline_live_v8(source_clock, live) {
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
        live_guard!(0);
        let started = adapter.start(&self.capability, &adapter_request);
        live_guard!(0);
        if started.is_err() {
            cancel_guarded!(
                adapter,
                "source start refusal",
                SourceAttemptFailure::Refused,
                0
            );
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
            if let Err(diagnostics) = self.check_deadline_live_v8(source_clock, live) {
                cancel_guarded!(
                    adapter,
                    "source cancellation or deadline",
                    self.deadline_failure(),
                    bytes.len()
                );
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
            live_guard!(bytes.len());
            let polled = adapter.poll();
            // Retain already-returned, bounded usage as inert observation data.
            // A lost guard still prevents stream processing and another callback.
            if live.is_some() {
                if let AdapterPoll::Failed {
                    failure,
                    attempted_bytes,
                } = &polled
                {
                    self.dispatch_failure(
                        source_failure_from_model(*failure),
                        (*attempted_bytes).min(max_response_bytes.saturating_add(1)),
                    );
                }
                let reported = match &polled {
                    AdapterPoll::Event(AdapterEvent::Usage {
                        tokens_in,
                        tokens_out,
                        cost_micros,
                    }) if !completed && *cost_micros >= 0 => {
                        Some((*tokens_in, *tokens_out, *cost_micros))
                    }
                    AdapterPoll::Settled(s) => {
                        match (s.usage.tokens_in, s.usage.tokens_out, s.usage.cost_micros) {
                            (Some(i), Some(o), Some(c)) if c >= 0 => Some((i, o, c)),
                            _ => None,
                        }
                    }
                    _ => None,
                };
                if let Some(next) = reported.filter(|next| {
                    usage.is_none_or(|old| next.0 >= old.0 && next.1 >= old.1 && next.2 >= old.2)
                }) {
                    self.last_owned_usage = Some(next);
                }
            }
            live_guard!(bytes.len());
            match polled {
                AdapterPoll::Pending => continue,
                AdapterPoll::Event(AdapterEvent::Delta(chunk)) => {
                    if completed {
                        cancel_guarded!(
                            adapter,
                            "source stream after completion",
                            SourceAttemptFailure::MalformedResponse,
                            bytes.len()
                        );
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
                        cancel_guarded!(
                            adapter,
                            "source stream bound",
                            SourceAttemptFailure::CapacityExceeded,
                            bytes
                                .len()
                                .saturating_add(chunk.len())
                                .min(max_response_bytes.saturating_add(1))
                        );
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
                    live_guard!(bytes.len());
                    let pushed = decoder.push(&chunk);
                    live_guard!(bytes.len());
                    if matches!(pushed, SourcePushOutcome::Refused(_)) {
                        cancel_guarded!(
                            adapter,
                            "source decoder refusal",
                            SourceAttemptFailure::MalformedResponse,
                            bytes.len()
                        );
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
                        cancel_guarded!(
                            adapter,
                            "duplicate completion",
                            SourceAttemptFailure::MalformedResponse,
                            bytes.len()
                        );
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
                        cancel_guarded!(
                            adapter,
                            "invalid usage",
                            SourceAttemptFailure::MalformedResponse,
                            bytes.len()
                        );
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
                    if let Err(diagnostics) = self.check_deadline_live_v8(source_clock, live) {
                        cancel_guarded!(
                            adapter,
                            "source cancellation or deadline at settlement",
                            self.deadline_failure(),
                            bytes.len()
                        );
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
                        cancel_guarded!(
                            adapter,
                            "settlement mismatch",
                            SourceAttemptFailure::MalformedResponse,
                            bytes.len()
                        );
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
                    live_guard!(bytes.len());
                    let decoded = decoder.finish();
                    live_guard!(bytes.len());
                    return match decoded {
                        SourcePushOutcome::Accepted(value) => {
                            self.record(
                                &adapter_request.request_bytes,
                                Some(&bytes),
                                "admitted",
                                settlement.usage.tokens_in,
                                settlement.usage.tokens_out,
                                settlement.usage.cost_micros,
                            );
                            let canonical = value.canonical_json().to_owned();
                            self.last_dispatch = Some(SourceAdapterDispatchFact::Settled {
                                decoded: value,
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
                            Ok(canonical)
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
                    cancel_guarded!(
                        adapter,
                        "source adapter failed",
                        source_failure_from_model(failure),
                        attempted_bytes.min(max_response_bytes.saturating_add(1))
                    );
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
        cancel_guarded!(
            adapter,
            "source adapter poll budget",
            SourceAttemptFailure::Timeout,
            bytes.len()
        );
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
