//! Explicit checkpoint-capable OpenCode source adapter.
//!
//! This adapter is intentionally unusable through ordinary `run_live`: the
//! checked driver supplies the one journal, ledger, and restart-stable clock.

use semaprax::agent_lifecycle::iterative::driver::{ProposalRequest, ProposalSource};
use semaprax::agent_lifecycle::iterative::source_live::{
    SourceProposalOutcome, SourceProposalPolicy,
};
use semaprax::diagnostic::Diagnostic;
use semaprax::live_invocation::{
    source_journal::{
        SourceAttemptFailure, SourceCheckpointSink, SourceJournalEntry, SourceReportedUsage,
        SourceTerminalStatus,
    },
    CumulativeBudgetLedger, InvocationBudgetHook, SourceInvocationClock,
};

use super::super::OpenCodeUsage;
use super::{
    prepare_source_request, OpenCodeGrammar, OpenCodeModelHandler, OpenCodeProposalSource,
    OpenCodeRunner, OpenCodeSourceAccounting,
};

/// A host source that accepts only the driver's journaled proposal route.
pub struct OpenCodeDurableProposalSource<'a, R> {
    handler: &'a mut OpenCodeModelHandler<R>,
    capability: &'a semaprax::live_invocation::ModelInvokeCapability,
    deployment_binding: String,
    grammar: OpenCodeGrammar,
    response_limit: usize,
    reservation_units: i64,
}

impl<'a, R> OpenCodeDurableProposalSource<'a, R> {
    #[allow(clippy::result_large_err)]
    pub fn new(
        handler: &'a mut OpenCodeModelHandler<R>,
        capability: &'a semaprax::live_invocation::ModelInvokeCapability,
        deployment_binding: String,
        grammar: OpenCodeGrammar,
        response_limit: usize,
        reservation_units: i64,
    ) -> Result<Self, Diagnostic> {
        if response_limit == 0 || reservation_units <= 0 {
            return Err(Diagnostic::io(
                "SPX-I239",
                "OpenCode durable source limits must be positive",
            ));
        }
        Ok(Self {
            handler,
            capability,
            deployment_binding,
            grammar,
            response_limit,
            reservation_units,
        })
    }
}

impl<R: OpenCodeRunner> ProposalSource for OpenCodeDurableProposalSource<'_, R> {
    fn checkpoint_policy(&self) -> Option<SourceProposalPolicy<'_>> {
        Some(SourceProposalPolicy {
            deployment_binding: &self.deployment_binding,
            response_limit: self.response_limit,
            reservation_units: self.reservation_units,
        })
    }

    fn checkpoint_attempt_identity(
        &self,
        request: &ProposalRequest<'_>,
    ) -> Result<
        semaprax::agent_lifecycle::iterative::source_live::SourceAttemptIdentity,
        Vec<Diagnostic>,
    > {
        let (prompt, model_request) = prepare_source_request(
            &self.deployment_binding,
            &self.grammar,
            self.response_limit,
            self.reservation_units,
            request,
        )
        .map_err(|error| vec![*error])?;
        Ok(
            semaprax::agent_lifecycle::iterative::source_live::SourceAttemptIdentity {
                request_digest: model_request.digest(),
                prompt_digest: semaprax::live_invocation::source_journal::source_prompt_digest(
                    prompt.as_bytes(),
                ),
                request_bytes: prompt.len(),
            },
        )
    }

    fn propose_checkpointed(
        &mut self,
        request: ProposalRequest<'_>,
        sink: &mut SourceCheckpointSink<'_>,
        ledger: &mut CumulativeBudgetLedger<'_>,
        clock: &dyn SourceInvocationClock,
    ) -> SourceProposalOutcome {
        let turn = match u32::try_from(request.turn) {
            Ok(turn) => turn,
            Err(_) => return closed_outcome("OpenCode durable source turn exceeds u32"),
        };
        let attempt = match u32::try_from(request.attempt) {
            Ok(attempt) => attempt,
            Err(_) => return closed_outcome("OpenCode durable source attempt exceeds u32"),
        };
        let starting_generation = sink.generation();
        let accounting = match OpenCodeSourceAccounting::new(ledger, self.reservation_units, 4) {
            Ok(accounting) => accounting,
            Err(_) => return closed_outcome("OpenCode durable source accounting refused"),
        };
        let mut bridge = match OpenCodeProposalSource::new(
            &mut *self.handler,
            self.capability,
            self.deployment_binding.clone(),
            self.grammar.clone(),
            self.response_limit,
            accounting,
        ) {
            Ok(bridge) => bridge,
            Err(error) => {
                return SourceProposalOutcome {
                    terminal_failure: None,
                    result: Err(vec![error]),
                    model_dispatches: 0,
                }
            }
        };
        let mut result = bridge.propose_checkpointed(request, sink, clock);
        let model_dispatches = bridge.checkpoint_dispatches();
        let usage = bridge.reported_usage();
        drop(bridge);

        let settled_or_failed = sink.generation() > starting_generation
            && matches!(
                sink.journal().entries().last(),
                Some(SourceJournalEntry::AttemptSettled { turn: entry_turn, attempt: entry_attempt, .. }
                    | SourceJournalEntry::AttemptFailed { turn: entry_turn, attempt: entry_attempt, .. })
                    if *entry_turn == turn && *entry_attempt == attempt
            );
        if settled_or_failed && !sink.poisoned() {
            let usage_entry = SourceJournalEntry::AttemptUsage {
                turn,
                attempt,
                reported: usage.map(source_usage),
            };
            if let Err(_) = sink.append_at(usage_entry, clock.now_millis()) {
                if result.is_ok() {
                    result = Err(vec![Diagnostic::io(
                        "SPX-I239",
                        "OpenCode source usage checkpoint failed",
                    )]);
                }
            }
        }
        let mut terminal_failure = recorded_attempt_failure(sink, turn, attempt);
        if result.is_ok() {
            if self.handler.runner.cancelled(&self.handler.config) {
                terminal_failure = Some(SourceTerminalStatus::Cancelled);
                result = Err(vec![Diagnostic::io(
                    "SPX-I239",
                    "OpenCode source checkpoint cancelled before proposal publication",
                )]);
            } else if let Err(refusal) = ledger.check_deadline() {
                if refusal.0 == "deadline_exceeded" {
                    terminal_failure = Some(SourceTerminalStatus::DeadlineExceeded);
                }
                result = Err(vec![Diagnostic::io(
                    "SPX-I239",
                    "OpenCode source checkpoint policy refused before proposal publication",
                )]);
            }
        }
        SourceProposalOutcome {
            terminal_failure,
            result,
            model_dispatches,
        }
    }

    fn propose(&mut self, _: ProposalRequest<'_>) -> Result<String, Vec<Diagnostic>> {
        Err(vec![Diagnostic::io(
            "SPX-I239",
            "OpenCode durable source requires the checkpointed live driver",
        )])
    }
}

fn source_usage(usage: OpenCodeUsage) -> SourceReportedUsage {
    SourceReportedUsage {
        total: usage.total,
        input: usage.input,
        output: usage.output,
        reasoning: usage.reasoning,
        cache_read: usage.cache_read,
        cache_write: usage.cache_write,
    }
}

fn closed_outcome(message: &str) -> SourceProposalOutcome {
    SourceProposalOutcome {
        terminal_failure: None,
        result: Err(vec![Diagnostic::io("SPX-I239", message)]),
        model_dispatches: 0,
    }
}

fn recorded_attempt_failure(
    sink: &SourceCheckpointSink<'_>,
    turn: u32,
    attempt: u32,
) -> Option<SourceTerminalStatus> {
    sink.journal()
        .entries()
        .iter()
        .rev()
        .find_map(|entry| match entry {
            SourceJournalEntry::AttemptFailed {
                turn: entry_turn,
                attempt: entry_attempt,
                reason: SourceAttemptFailure::Cancelled,
                ..
            } if *entry_turn == turn && *entry_attempt == attempt => {
                Some(SourceTerminalStatus::Cancelled)
            }
            SourceJournalEntry::AttemptFailed {
                turn: entry_turn,
                attempt: entry_attempt,
                reason: SourceAttemptFailure::DeadlineExceeded,
                ..
            } if *entry_turn == turn && *entry_attempt == attempt => {
                Some(SourceTerminalStatus::DeadlineExceeded)
            }
            SourceJournalEntry::AttemptFailed {
                turn: entry_turn,
                attempt: entry_attempt,
                ..
            } if *entry_turn == turn && *entry_attempt == attempt => None,
            _ => None,
        })
}
