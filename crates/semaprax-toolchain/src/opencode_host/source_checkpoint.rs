//! Attempt-level source checkpointing for the explicit OpenCode adapter.
//!
//! This records one model attempt only.  It deliberately does not make the
//! iterative driver, authorization, effects, transitions, or replay durable.

use semaprax::agent_lifecycle::iterative::driver::ProposalRequest;
use semaprax::diagnostic::Diagnostic;
use semaprax::live_invocation::{
    source_journal::{
        source_prompt_digest, source_response_digest, SourceAttemptFailure, SourceCheckpointSink,
        SourceJournalEntry,
    },
    ModelFailure, ModelInvocationOutcome, SourceInvocationClock,
};

use super::{
    accounting_diagnostic, OpenCodeAccountingRefusal, OpenCodeProposalSource, OpenCodeRunner,
};

impl<R: OpenCodeRunner> OpenCodeProposalSource<'_, R> {
    /// Dispatches exactly one source proposal after its attempt intent is
    /// acknowledged by `sink`.
    ///
    /// The caller owns the preceding `RunOpened` and `TurnObserved` entries.
    /// A poisoned sink is never retried here. This is not a replay route: an
    /// unresolved acknowledged intent remains uncertain; a future recovery
    /// driver must not redispatch it.
    pub fn propose_checkpointed(
        &mut self,
        context: ProposalRequest<'_>,
        sink: &mut SourceCheckpointSink<'_>,
        clock: &dyn SourceInvocationClock,
    ) -> Result<String, Vec<Diagnostic>> {
        let (prompt, request) = self
            .prepare_request(&context)
            .map_err(|error| vec![*error])?;
        let binding = sink.journal().binding();
        let binding_matches = binding.matches_proposal_source(
            context.source_revision,
            &self.deployment_binding,
            &context.task.objective,
            context.task.budget,
            context.proposal_schema_digest,
        );
        let response_limit = binding.response_limit();
        let reservation_units = binding.reservation_units();
        let deadline_millis = binding.deadline_millis();
        let clock_domain_matches = binding.clock_domain() == clock.clock_domain();
        if !binding_matches
            || response_limit != self.max_response_bytes
            || reservation_units != self.accounting.reservation_units()
            || !clock_domain_matches
        {
            return Err(vec![checkpoint_diagnostic("binding_mismatch")]);
        }
        let now = clock.now_millis();
        if now >= deadline_millis {
            return Err(vec![checkpoint_diagnostic("deadline_exceeded")]);
        }
        if self.handler.runner.cancelled(&self.handler.config) {
            return Err(vec![checkpoint_diagnostic("cancelled_before_dispatch")]);
        }
        let attempt = u32::try_from(context.attempt)
            .map_err(|_| vec![checkpoint_diagnostic("attempt_out_of_range")])?;
        let turn = request.turn;
        let request_digest = request.digest();
        let prompt_digest = source_prompt_digest(prompt.as_bytes());
        let intent = SourceJournalEntry::AttemptIntent {
            turn,
            attempt,
            attempt_digest: binding.attempt_digest(
                turn,
                attempt,
                &request_digest,
                &prompt_digest,
                prompt.len(),
            ),
            request_digest,
            prompt_digest,
            request_bytes: prompt.len(),
            reserved_units: reservation_units,
            response_limit: self.max_response_bytes,
        };
        // Check causal phase and future capacity before charging the shared
        // ledger. `append_at` repeats this validation before its store write.
        sink.preflight_at(&intent, now)
            .map_err(|_| vec![checkpoint_diagnostic("checkpoint_refused")])?;
        let reserved = self
            .accounting
            .reserve(request, context.attempt, prompt.len())
            .map_err(|refusal| vec![accounting_diagnostic(refusal)])?;
        if reserved.amount != reservation_units {
            self.finish_without_dispatch(ModelFailure::Refused, 0);
            return Err(vec![checkpoint_diagnostic("reservation_mismatch")]);
        }
        if sink.append_at(intent, now).is_err() {
            self.finish_without_dispatch(ModelFailure::Refused, 0);
            return Err(vec![checkpoint_diagnostic("checkpoint_store_failed")]);
        }

        // A store implementation may take time or notify cancellation while
        // acknowledging the intent. Recheck both boundaries before transport.
        let (after_ack, guard) = self.checkpoint_guard(clock, now, deadline_millis);
        if let Some(guard) = guard {
            let (failure, reason) = guard.failure();
            self.finish_without_dispatch(failure, 0);
            sink.append_at(
                SourceJournalEntry::AttemptFailed {
                    turn,
                    attempt,
                    reason,
                    attempted_bytes: 0,
                },
                after_ack.max(now),
            )
            .map_err(|_| vec![checkpoint_diagnostic("checkpoint_store_failed")])?;
            return Err(vec![guard.diagnostic()]);
        }

        let _capability_reason = self.capability.reason();
        let outcome = self.handler.invoke_prompt(&prompt, self.max_response_bytes);
        let reported_usage = self
            .handler
            .last_receipt
            .as_ref()
            .and_then(|receipt| receipt.usage.clone());
        let (settled_at, late_guard) = self.checkpoint_guard(clock, after_ack, deadline_millis);
        let finish = self.accounting.finish(&outcome, reported_usage);
        let selected_guard = late_guard.or_else(|| {
            finish
                .as_ref()
                .err()
                .copied()
                .map(CheckpointGuard::Accounting)
        });
        let entry = match &outcome {
            ModelInvocationOutcome::Settled(bytes) => {
                let refusal = selected_guard.as_ref().map(CheckpointGuard::failure);
                if let Some((_, reason)) = refusal {
                    SourceJournalEntry::AttemptFailed {
                        turn,
                        attempt,
                        reason,
                        attempted_bytes: bytes.len(),
                    }
                } else {
                    outcome_entry(&outcome, turn, attempt)
                }
            }
            _ => outcome_entry(&outcome, turn, attempt),
        };
        let floor = settled_at.max(after_ack);
        sink.append_at(entry, floor)
            .map_err(|_| vec![checkpoint_diagnostic("checkpoint_store_failed")])?;
        // Settlement acknowledgement is also an injected callback. Preserve
        // the raw outcome first, then refuse exposing it if that callback
        // cancelled or exhausted the same absolute deadline.
        let (_, after_settlement_guard) = self.checkpoint_guard(clock, floor, deadline_millis);
        if matches!(outcome, ModelInvocationOutcome::Settled(_)) {
            if let Some(guard) = selected_guard.or(after_settlement_guard) {
                return Err(vec![guard.diagnostic()]);
            }
        }
        if let Err(refusal) = finish {
            return Err(vec![accounting_diagnostic(refusal)]);
        }
        match outcome {
            ModelInvocationOutcome::Settled(bytes) => String::from_utf8(bytes).map_err(|_| {
                vec![Diagnostic::io(
                    "SPX-I239",
                    "OpenCode settled non-UTF-8 proposal bytes",
                )]
            }),
            ModelInvocationOutcome::Failed {
                failure,
                attempted_bytes,
            } => Err(vec![Diagnostic::io(
                "SPX-I239",
                format!(
                    "OpenCode source transport failed: {}; attempted response bytes: {attempted_bytes}",
                    failure.as_str()
                ),
            )]),
        }
    }

    fn checkpoint_guard(
        &self,
        clock: &dyn SourceInvocationClock,
        floor: i64,
        deadline: i64,
    ) -> (i64, Option<CheckpointGuard>) {
        let now = clock.now_millis();
        let refusal = if self.handler.runner.cancelled(&self.handler.config) {
            Some(CheckpointGuard::Cancelled)
        } else if now < floor {
            Some(CheckpointGuard::ClockRegressed)
        } else if now >= deadline {
            Some(CheckpointGuard::DeadlineExceeded)
        } else {
            self.accounting
                .check_deadline()
                .err()
                .map(CheckpointGuard::Accounting)
        };
        (now, refusal)
    }

    fn finish_without_dispatch(&mut self, failure: ModelFailure, attempted_bytes: usize) {
        let _ = self.accounting.finish(
            &ModelInvocationOutcome::Failed {
                failure,
                attempted_bytes,
            },
            None,
        );
    }
}

enum CheckpointGuard {
    Cancelled,
    ClockRegressed,
    DeadlineExceeded,
    Accounting(OpenCodeAccountingRefusal),
}

impl CheckpointGuard {
    fn failure(&self) -> (ModelFailure, SourceAttemptFailure) {
        match self {
            Self::Cancelled => (ModelFailure::Cancelled, SourceAttemptFailure::Cancelled),
            Self::DeadlineExceeded
            | Self::Accounting(OpenCodeAccountingRefusal::DeadlineExceeded) => (
                ModelFailure::Refused,
                SourceAttemptFailure::DeadlineExceeded,
            ),
            Self::ClockRegressed | Self::Accounting(_) => {
                (ModelFailure::Refused, SourceAttemptFailure::Refused)
            }
        }
    }

    fn diagnostic(self) -> Diagnostic {
        match self {
            Self::Cancelled => checkpoint_diagnostic("cancelled_before_publication"),
            Self::ClockRegressed => checkpoint_diagnostic("clock_regressed"),
            Self::DeadlineExceeded => checkpoint_diagnostic("deadline_exceeded"),
            Self::Accounting(refusal) => accounting_diagnostic(refusal),
        }
    }
}

pub(super) fn outcome_entry(
    outcome: &ModelInvocationOutcome,
    turn: u32,
    attempt: u32,
) -> SourceJournalEntry {
    match outcome {
        ModelInvocationOutcome::Settled(response) => SourceJournalEntry::AttemptSettled {
            turn,
            attempt,
            response: response.clone(),
            response_digest: source_response_digest(response),
        },
        ModelInvocationOutcome::Failed {
            failure,
            attempted_bytes,
        } => SourceJournalEntry::AttemptFailed {
            turn,
            attempt,
            reason: failure_reason(*failure),
            attempted_bytes: *attempted_bytes,
        },
    }
}

pub(super) fn failure_reason(failure: ModelFailure) -> SourceAttemptFailure {
    match failure {
        ModelFailure::Timeout => SourceAttemptFailure::Timeout,
        ModelFailure::Cancelled => SourceAttemptFailure::Cancelled,
        ModelFailure::CapacityExceeded => SourceAttemptFailure::CapacityExceeded,
        ModelFailure::ProviderError => SourceAttemptFailure::ProviderError,
        ModelFailure::MalformedResponse => SourceAttemptFailure::MalformedResponse,
        ModelFailure::Refused => SourceAttemptFailure::Refused,
    }
}

fn checkpoint_diagnostic(reason: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-I239",
        format!("OpenCode source checkpoint refused: {reason}"),
    )
}
