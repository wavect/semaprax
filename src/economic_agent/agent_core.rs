//! Economic Agent construction, the public `execute` entry point, and the
//! per-call guards and failure finalization shared by every driven path.

use super::documents::{Approval, Invoice, Plan, Simulation};
use super::intent::{admit_intent, parse_intent};
use super::journal::{
    clone_journal_bounded, BroadcastReceipt, Journal, JournalState, Reconciliation,
};
use super::replay::{cas_journal, cumulative_usage, diagnostic_terminal, finish_run};
use super::validate::{admitted_now_from, g212, g216, g217, info, terminal_floor};
use super::{
    admit_policy_source, Budget, EconomicAgent, EconomicAgentHost,
    EconomicRollingReservationUpdate, EconomicRun, Event, Intent, MAX_JSON_DEPTH,
};
use crate::agent_runtime::{AgentCancellation, AgentRun, AgentRunStatus};
use crate::bounded_output::{active_remaining, reserve_active, set_active_floor, with_limit_usage};
use crate::diagnostic::Diagnostic;

impl<H: EconomicAgentHost> EconomicAgent<H> {
    pub(super) fn terminal_floor(&self) -> Result<usize, Diagnostic> {
        let lane = terminal_floor(&self.policy.limits)?;
        if active_remaining().is_some_and(|remaining| remaining < lane) {
            return Err(g216("builder_bytes", self.policy.limits.max_builder_bytes));
        }
        if !set_active_floor(lane) {
            return Err(g216("builder_bytes", self.policy.limits.max_builder_bytes));
        }
        Ok(lane)
    }

    /// Parses and retains one canonical Policy before consulting the host.
    pub fn new(
        policy: &str,
        host: H,
        cancellation: AgentCancellation,
    ) -> Result<Self, Vec<Diagnostic>> {
        admit_policy_source(policy).map(|(policy, consumed)| Self {
            policy,
            retained_policy_bytes: consumed,
            host,
            cancellation,
        })
    }

    /// Executes one canonical Payment Intent proposed by a completed sealed Agent run.
    pub fn execute(&mut self, source: &AgentRun) -> Result<EconomicRun, Vec<Diagnostic>> {
        let binding = source.economic_binding();
        if binding.status != AgentRunStatus::Completed {
            return Err(vec![g212("agent run not completed")]);
        }
        let Some(message) = binding.final_message else {
            return Err(vec![g212("agent run not completed")]);
        };
        if self.cancellation.is_cancelled() {
            return Err(vec![info("SPX-I228", "Economic Agent run was cancelled")]);
        }
        let policy_limit = self.policy.limits.max_builder_bytes as usize;
        let started = self.host.boundary_probe().elapsed_ms();
        let result = with_limit_usage(policy_limit, || {
            if !reserve_active(self.retained_policy_bytes) {
                return Err(g216("builder_bytes", self.policy.limits.max_builder_bytes));
            }
            if !reserve_active(message.len().saturating_mul(MAX_JSON_DEPTH + 2)) {
                return Err(g216("builder_bytes", self.policy.limits.max_builder_bytes));
            }
            let intent = parse_intent(message).and_then(|intent| {
                admit_intent(&self.policy, &intent)?;
                Ok(intent)
            })?;
            self.terminal_floor()?;
            self.execute_bounded(&binding, intent, started)
        });
        match result {
            (Ok(run), false, _) => Ok(run),
            (Err(diagnostic), _, _) => Err(vec![diagnostic]),
            (Ok(_), true, _) => Err(vec![g216(
                "builder_bytes",
                self.policy.limits.max_builder_bytes,
            )]),
        }
    }

    pub(super) fn pre_call(&self, started: u64, maximum_output: usize) -> Result<(), Diagnostic> {
        if self.cancellation.is_cancelled() {
            return Err(info("SPX-I228", "Economic Agent run was cancelled"));
        }
        if self.elapsed_ms(started)? > self.policy.limits.max_elapsed_ms {
            return Err(info("SPX-I229", "Economic Agent deadline was exceeded"));
        }
        let multiplier = if maximum_output == self.policy.limits.max_journal_bytes as usize {
            2
        } else if maximum_output == self.policy.limits.max_signed_transaction_bytes as usize {
            3
        } else {
            usize::try_from(self.policy.limits.max_json_depth)
                .map_err(|_| g217())?
                .checked_add(3)
                .ok_or_else(g217)?
        };
        let output_lane = maximum_output
            .checked_mul(multiplier)
            .ok_or_else(|| g216("builder_bytes", self.policy.limits.max_builder_bytes))?;
        let required = output_lane
            .checked_add(self.terminal_floor()?)
            .ok_or_else(|| g216("builder_bytes", self.policy.limits.max_builder_bytes))?;
        if active_remaining().is_some_and(|remaining| remaining < required) {
            return Err(g216("builder_bytes", self.policy.limits.max_builder_bytes));
        }
        Ok(())
    }

    pub(super) fn elapsed_ms(&self, started: u64) -> Result<u64, Diagnostic> {
        self.host
            .boundary_probe()
            .elapsed_ms()
            .checked_sub(started)
            .ok_or_else(|| info("SPX-I229", "Economic Agent deadline was exceeded"))
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn finish_failure(
        &mut self,
        binding: &crate::agent_runtime::EconomicAgentBinding<'_>,
        intent: &Intent,
        economic_run_id: &str,
        journal: &Journal,
        invoice: Option<&Invoice>,
        plan: Option<&Plan>,
        simulation: Option<&Simulation>,
        approval: Option<&Approval>,
        broadcast: Option<&BroadcastReceipt>,
        reconciliation: Option<&Reconciliation>,
        events: &mut Vec<Event>,
        budget: &mut Budget,
        diagnostic: Diagnostic,
        started: u64,
    ) -> Result<EconomicRun, Diagnostic> {
        let mut terminal_diagnostic = diagnostic;
        let mut terminal_journal =
            clone_journal_bounded(journal, self.policy.limits.max_builder_bytes)?;
        let usage = cumulative_usage(events)?;
        let uncertain_journal_cas = events
            .last()
            .is_some_and(|event| event.kind == "journal_committed" && event.authority_uncertain);
        let already_sealed_effect_boundary = (journal.version == 4
            && journal.state == JournalState::Approved)
            || (journal.version == 6 && journal.state == JournalState::BroadcastUnknown)
            || journal.broadcast.is_some();
        let no_signature_or_broadcast_attempt = journal.signed.is_none()
            && journal.broadcast.is_none()
            && usage.signatures == 0
            && usage.broadcasts == 0
            && journal.version <= 3
            && (journal.version == 1 || usage.journal_writes > 0);
        if !uncertain_journal_cas {
            terminal_journal.state = if already_sealed_effect_boundary
                || journal.signed.is_some()
                || journal.broadcast.is_some()
            {
                journal.state
            } else {
                match terminal_diagnostic.code {
                    "SPX-I228" => JournalState::Cancelled,
                    "SPX-G212" | "SPX-G214" => JournalState::Rejected,
                    _ => JournalState::Failed,
                }
            };
        }
        if no_signature_or_broadcast_attempt && !uncertain_journal_cas {
            terminal_journal.updated_at = admitted_now_from(
                intent,
                journal.updated_at,
                self.elapsed_ms(started)
                    .unwrap_or(self.policy.limits.max_elapsed_ms),
            )
            .unwrap_or(intent.expires_at);
        }
        let rolling = if no_signature_or_broadcast_attempt {
            EconomicRollingReservationUpdate::Release
        } else {
            EconomicRollingReservationUpdate::Retain
        };
        if journal.version > 0
            && !uncertain_journal_cas
            && no_signature_or_broadcast_attempt
            && !already_sealed_effect_boundary
            && cas_journal(
                &mut self.host,
                &mut terminal_journal,
                events,
                budget,
                self.policy.limits.max_journal_bytes,
                rolling,
            )
            .is_err()
        {
            terminal_journal =
                clone_journal_bounded(journal, self.policy.limits.max_builder_bytes)?;
            terminal_diagnostic = info("SPX-I222", "Economic Agent journal adapter failed");
        }
        let terminal = diagnostic_terminal(&terminal_diagnostic);
        finish_run(
            economic_run_id,
            binding,
            &self.policy,
            intent,
            invoice,
            plan,
            simulation,
            approval,
            &terminal_journal,
            broadcast,
            reconciliation,
            events,
            terminal,
            budget,
            self.elapsed_ms(started)?,
        )
    }
}
