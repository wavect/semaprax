//! Post-broadcast reconciliation, resumption of a loaded journal, and the
//! public `reconcile` entry point.

use super::documents::{Approval, Invoice, Plan, Simulation};
use super::evidence::run_id;
use super::intent::{admit_intent, parse_intent};
use super::journal::{
    broadcast_is_provisional, clone_journal_bounded, parse_broadcast, parse_journal_classified,
    parse_provisional_broadcast, parse_reconciliation_limited, reconciliation_topology,
    BroadcastReceipt, Journal, JournalParseFailure, JournalState,
};
use super::replay::{cas_journal, event, finish_run, push_event};
use super::validate::{
    canonical, digest, g212, g215, g216, g217, identifier, info, object, text,
    validate_confirmation,
};
use super::{
    Budget, Doc, EconomicAdapterDisposition, EconomicAgent, EconomicAgentHost,
    EconomicDocumentSink, EconomicJournalLoad, EconomicRail, EconomicRollingReservationUpdate,
    EconomicRun, EconomicRunStatus, Event, Intent, Terminal, Usage, BROADCAST_SCHEMA,
    JOURNAL_DOMAIN, MAX_JSON_DEPTH,
};
use crate::agent_runtime::{AgentRun, AgentRunStatus};
use crate::bounded_output::{reserve_active, with_limit_usage};
use crate::diagnostic::Diagnostic;

impl<H: EconomicAgentHost> EconomicAgent<H> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn reconcile_after_broadcast(
        &mut self,
        binding: &crate::agent_runtime::EconomicAgentBinding<'_>,
        intent: &Intent,
        economic_run_id: &str,
        mut journal: Journal,
        invoice: Option<&Invoice>,
        plan: &Plan,
        simulation: &Simulation,
        approval: &Approval,
        broadcast: &BroadcastReceipt,
        mut events: Vec<Event>,
        mut budget: Budget,
        started: u64,
    ) -> Result<EconomicRun, Diagnostic> {
        macro_rules! terminal_try {
            ($expression:expr, $reconciliation:expr) => {
                match $expression {
                    Ok(value) => value,
                    Err(diagnostic) => {
                        return self.finish_failure(
                            binding,
                            intent,
                            economic_run_id,
                            &journal,
                            invoice,
                            Some(plan),
                            Some(simulation),
                            Some(approval),
                            Some(broadcast),
                            $reconciliation,
                            &mut events,
                            &mut budget,
                            diagnostic,
                            started,
                        )
                    }
                }
            };
        }
        terminal_try!(
            self.pre_call(
                started,
                self.policy.limits.max_reconciliation_bytes as usize,
            ),
            None
        );
        let (attempts, odd) = terminal_try!(reconciliation_topology(&journal), None);
        if odd || attempts >= self.policy.limits.max_reconciliations {
            return self.finish_failure(
                binding,
                intent,
                economic_run_id,
                &journal,
                invoice,
                Some(plan),
                Some(simulation),
                Some(approval),
                Some(broadcast),
                None,
                &mut events,
                &mut budget,
                g216("reconciliations", self.policy.limits.max_reconciliations),
                started,
            );
        }
        terminal_try!(
            cas_journal(
                &mut self.host,
                &mut journal,
                &mut events,
                &mut budget,
                self.policy.limits.max_journal_bytes,
                EconomicRollingReservationUpdate::Retain,
            ),
            None
        );
        terminal_try!(
            self.pre_call(
                started,
                self.policy.limits.max_reconciliation_bytes as usize,
            ),
            None
        );
        let rail = intent.settlement_rail();
        let (network, _) = intent.network_asset();
        let mut sink = EconomicDocumentSink::new(
            self.policy.limits.max_reconciliation_bytes as usize,
            self.cancellation.clone(),
            self.host.boundary_probe(),
            started,
            self.policy.limits.max_elapsed_ms,
            self.policy.limits.max_builder_bytes,
            self.terminal_floor()?,
        );
        let disposition = match rail {
            EconomicRail::Evm => self
                .host
                .evm_reconcile(&broadcast.transaction_id, &mut sink),
            EconomicRail::Solana => self
                .host
                .solana_reconcile(&broadcast.transaction_id, &mut sink),
            EconomicRail::Bitcoin => self
                .host
                .bitcoin_reconcile(&broadcast.transaction_id, &mut sink),
        };
        let mut usage = Usage::default();
        usage.reconciliations = 1;
        if disposition != EconomicAdapterDisposition::Succeeded {
            push_event(
                &mut events,
                event(
                    "reconciliation_finished",
                    Some(rail),
                    Some(broadcast.doc.digest.clone()),
                    None,
                    "failed",
                    usage,
                )?,
            )?;
            return self.finish_failure(
                binding,
                intent,
                economic_run_id,
                &journal,
                invoice,
                Some(plan),
                Some(simulation),
                Some(approval),
                Some(broadcast),
                None,
                &mut events,
                &mut budget,
                info("SPX-I227", "Economic Agent reconciliation adapter failed"),
                started,
            );
        }
        let source = terminal_try!(sink.finish("reconciliation_bytes"), None);
        usage.output_bytes = source.len() as u64;
        let reconciliation = terminal_try!(
            parse_reconciliation_limited(
                &source,
                rail,
                network,
                &broadcast.transaction_id,
                &self.policy.limits,
            ),
            None
        );
        terminal_try!(validate_confirmation(intent, &reconciliation), None);
        budget.reconciliation_bytes = source.len() as u64;
        budget.reconciliations = attempts.checked_add(1).ok_or_else(g217)?;
        push_event(
            &mut events,
            event(
                "reconciliation_finished",
                Some(rail),
                Some(broadcast.doc.digest.clone()),
                Some(reconciliation.doc.digest.clone()),
                reconciliation.status,
                usage,
            )?,
        )?;
        journal.state = match reconciliation.status {
            "confirmed" => JournalState::Confirmed,
            "reorged" => JournalState::Reorged,
            "dropped" => JournalState::Dropped,
            _ => JournalState::Pending,
        };
        journal.reconciliation = Some(reconciliation.doc.clone());
        journal.updated_at = reconciliation.observed;
        terminal_try!(
            cas_journal(
                &mut self.host,
                &mut journal,
                &mut events,
                &mut budget,
                self.policy.limits.max_journal_bytes,
                EconomicRollingReservationUpdate::Retain,
            ),
            Some(&reconciliation)
        );
        let status = match reconciliation.status {
            "confirmed" => EconomicRunStatus::Confirmed,
            "reorged" => EconomicRunStatus::Reorged,
            "dropped" => EconomicRunStatus::Dropped,
            _ => EconomicRunStatus::Pending,
        };
        let terminal = Terminal {
            status,
            transaction_id: Some(broadcast.transaction_id.clone()),
            confirmation: Some(reconciliation.status.to_owned()),
            code: None,
            message: None,
        };
        finish_run(
            economic_run_id,
            binding,
            &self.policy,
            intent,
            invoice,
            Some(plan),
            Some(simulation),
            Some(approval),
            &journal,
            Some(broadcast),
            Some(&reconciliation),
            &mut events,
            terminal,
            &mut budget,
            self.elapsed_ms(started)?,
        )
    }

    pub(super) fn resume_loaded(
        &mut self,
        binding: &crate::agent_runtime::EconomicAgentBinding<'_>,
        intent: Intent,
        economic_run_id: String,
        mut journal: Journal,
        mut events: Vec<Event>,
        mut budget: Budget,
        started: u64,
    ) -> Result<EconomicRun, Diagnostic> {
        macro_rules! terminal_try {
            ($expression:expr, $broadcast:expr, $reconciliation:expr) => {
                match $expression {
                    Ok(value) => value,
                    Err(diagnostic) => {
                        return self.finish_failure(
                            binding,
                            &intent,
                            &economic_run_id,
                            &journal,
                            None,
                            None,
                            None,
                            None,
                            $broadcast,
                            $reconciliation,
                            &mut events,
                            &mut budget,
                            diagnostic,
                            started,
                        )
                    }
                }
            };
        }
        let Some(broadcast_doc) = journal.broadcast.as_ref() else {
            return self.finish_failure(
                binding,
                &intent,
                &economic_run_id,
                &journal,
                None,
                None,
                None,
                None,
                None,
                None,
                &mut events,
                &mut budget,
                g215(),
                started,
            );
        };
        let (_, value) = terminal_try!(
            canonical(
                &broadcast_doc.source,
                "broadcast receipt",
                BROADCAST_SCHEMA,
                self.policy.limits.max_broadcast_receipt_bytes as usize,
            ),
            None,
            None
        );
        let row = terminal_try!(
            object(&value, "broadcast receipt", BROADCAST_SCHEMA),
            None,
            None
        );
        let signed_digest = terminal_try!(
            text(
                row,
                "signed_transaction_digest",
                "broadcast receipt",
                BROADCAST_SCHEMA,
            ),
            None,
            None
        );
        let (network, _) = intent.network_asset();
        let broadcast = terminal_try!(
            if broadcast_is_provisional(broadcast_doc) {
                let transaction_id = value["transaction_id"].as_str().ok_or_else(g215);
                transaction_id.and_then(|transaction_id| {
                    parse_provisional_broadcast(
                        &broadcast_doc.source,
                        intent.settlement_rail(),
                        network,
                        signed_digest,
                        transaction_id,
                    )
                })
            } else {
                parse_broadcast(
                    &broadcast_doc.source,
                    intent.settlement_rail(),
                    network,
                    signed_digest,
                    None,
                )
            },
            None,
            None
        );
        budget.signed_bytes = journal.signed.as_ref().map_or(0, |value| value.1 as u64);
        budget.broadcast_bytes = broadcast.doc.source.len() as u64;
        if matches!(
            journal.state,
            JournalState::Confirmed | JournalState::Reorged | JournalState::Dropped
        ) {
            let status = match journal.state {
                JournalState::Confirmed => EconomicRunStatus::Confirmed,
                JournalState::Reorged => EconomicRunStatus::Reorged,
                _ => EconomicRunStatus::Dropped,
            };
            let terminal = Terminal {
                status,
                transaction_id: Some(broadcast.transaction_id.clone()),
                confirmation: Some(journal.state.text().to_owned()),
                code: None,
                message: None,
            };
            return finish_run(
                &economic_run_id,
                binding,
                &self.policy,
                &intent,
                None,
                None,
                None,
                None,
                &journal,
                Some(&broadcast),
                None,
                &mut events,
                terminal,
                &mut budget,
                started,
            );
        }
        let (mut attempts, odd) =
            terminal_try!(reconciliation_topology(&journal), Some(&broadcast), None);
        if odd {
            let mut closed = terminal_try!(
                clone_journal_bounded(&journal, self.policy.limits.max_builder_bytes),
                Some(&broadcast),
                None
            );
            terminal_try!(
                cas_journal(
                    &mut self.host,
                    &mut closed,
                    &mut events,
                    &mut budget,
                    self.policy.limits.max_journal_bytes,
                    EconomicRollingReservationUpdate::Retain,
                ),
                Some(&broadcast),
                None
            );
            journal = closed;
        }
        if attempts >= self.policy.limits.max_reconciliations {
            return self.finish_failure(
                binding,
                &intent,
                &economic_run_id,
                &journal,
                None,
                None,
                None,
                None,
                Some(&broadcast),
                None,
                &mut events,
                &mut budget,
                g216("reconciliations", self.policy.limits.max_reconciliations),
                started,
            );
        }
        terminal_try!(
            self.pre_call(
                started,
                self.policy.limits.max_reconciliation_bytes as usize,
            ),
            Some(&broadcast),
            None
        );
        terminal_try!(
            cas_journal(
                &mut self.host,
                &mut journal,
                &mut events,
                &mut budget,
                self.policy.limits.max_journal_bytes,
                EconomicRollingReservationUpdate::Retain,
            ),
            Some(&broadcast),
            None
        );
        attempts = attempts.checked_add(1).ok_or_else(g217)?;
        terminal_try!(
            self.pre_call(
                started,
                self.policy.limits.max_reconciliation_bytes as usize,
            ),
            Some(&broadcast),
            None
        );
        let mut sink = EconomicDocumentSink::new(
            self.policy.limits.max_reconciliation_bytes as usize,
            self.cancellation.clone(),
            self.host.boundary_probe(),
            started,
            self.policy.limits.max_elapsed_ms,
            self.policy.limits.max_builder_bytes,
            self.terminal_floor()?,
        );
        let disposition = match intent.settlement_rail() {
            EconomicRail::Evm => self
                .host
                .evm_reconcile(&broadcast.transaction_id, &mut sink),
            EconomicRail::Solana => self
                .host
                .solana_reconcile(&broadcast.transaction_id, &mut sink),
            EconomicRail::Bitcoin => self
                .host
                .bitcoin_reconcile(&broadcast.transaction_id, &mut sink),
        };
        if disposition != EconomicAdapterDisposition::Succeeded {
            return self.finish_failure(
                binding,
                &intent,
                &economic_run_id,
                &journal,
                None,
                None,
                None,
                None,
                Some(&broadcast),
                None,
                &mut events,
                &mut budget,
                info("SPX-I227", "Economic Agent reconciliation adapter failed"),
                started,
            );
        }
        let source = terminal_try!(sink.finish("reconciliation_bytes"), Some(&broadcast), None);
        let reconciliation = terminal_try!(
            parse_reconciliation_limited(
                &source,
                intent.settlement_rail(),
                network,
                &broadcast.transaction_id,
                &self.policy.limits,
            ),
            Some(&broadcast),
            None
        );
        terminal_try!(
            validate_confirmation(&intent, &reconciliation),
            Some(&broadcast),
            None
        );
        budget.reconciliation_bytes = source.len() as u64;
        budget.reconciliations = attempts;
        push_event(
            &mut events,
            event(
                "reconciliation_finished",
                Some(intent.settlement_rail()),
                Some(broadcast.doc.digest.clone()),
                Some(reconciliation.doc.digest.clone()),
                reconciliation.status,
                Usage {
                    reconciliations: 1,
                    output_bytes: source.len() as u64,
                    ..Usage::default()
                },
            )?,
        )?;
        let mut next = terminal_try!(
            clone_journal_bounded(&journal, self.policy.limits.max_builder_bytes),
            Some(&broadcast),
            Some(&reconciliation)
        );
        next.state = match reconciliation.status {
            "confirmed" => JournalState::Confirmed,
            "reorged" => JournalState::Reorged,
            "dropped" => JournalState::Dropped,
            _ => JournalState::Pending,
        };
        next.reconciliation = Some(reconciliation.doc.clone());
        next.updated_at = reconciliation.observed;
        terminal_try!(
            cas_journal(
                &mut self.host,
                &mut next,
                &mut events,
                &mut budget,
                self.policy.limits.max_journal_bytes,
                EconomicRollingReservationUpdate::Retain,
            ),
            Some(&broadcast),
            Some(&reconciliation)
        );
        let terminal = Terminal {
            status: match reconciliation.status {
                "confirmed" => EconomicRunStatus::Confirmed,
                "reorged" => EconomicRunStatus::Reorged,
                "dropped" => EconomicRunStatus::Dropped,
                _ => EconomicRunStatus::Pending,
            },
            transaction_id: Some(broadcast.transaction_id.clone()),
            confirmation: Some(reconciliation.status.to_owned()),
            code: None,
            message: None,
        };
        finish_run(
            &economic_run_id,
            binding,
            &self.policy,
            &intent,
            None,
            None,
            None,
            None,
            &next,
            Some(&broadcast),
            Some(&reconciliation),
            &mut events,
            terminal,
            &mut budget,
            started,
        )
    }

    /// Reconciles an idempotency binding using the same sealed Agent source.
    pub fn reconcile(
        &mut self,
        idempotency_key: &str,
        source: &AgentRun,
    ) -> Result<EconomicRun, Vec<Diagnostic>> {
        if !identifier(idempotency_key) {
            return Err(vec![g215()]);
        }
        let binding = source.economic_binding();
        if binding.status != AgentRunStatus::Completed {
            return Err(vec![g212("agent run not completed")]);
        }
        let Some(message) = binding.final_message else {
            return Err(vec![g212("agent run not completed")]);
        };
        let started = self.host.boundary_probe().elapsed_ms();
        let limit = self.policy.limits.max_builder_bytes as usize;
        let (result, overflowed, _) = with_limit_usage(limit, || {
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
            if intent.idempotency_key != idempotency_key {
                return Err(g215());
            }
            self.terminal_floor()?;
            self.reconcile_bounded(&binding, intent, started)
        });
        if overflowed {
            return Err(vec![g216(
                "builder_bytes",
                self.policy.limits.max_builder_bytes,
            )]);
        }
        result.map_err(|diagnostic| vec![diagnostic])
    }

    pub(super) fn reconcile_bounded(
        &mut self,
        binding: &crate::agent_runtime::EconomicAgentBinding<'_>,
        intent: Intent,
        started: u64,
    ) -> Result<EconomicRun, Diagnostic> {
        let economic_run_id = run_id(
            binding.evidence_digest,
            &self.policy.digest,
            &intent.digest,
            &intent.idempotency_key,
        );
        let mut journal = Journal {
            idempotency_key: intent.idempotency_key.clone(),
            version: 0,
            policy: Doc {
                source: self.policy.source.clone(),
                digest: self.policy.digest.clone(),
            },
            intent: Doc {
                source: intent.source.clone(),
                digest: intent.digest.clone(),
            },
            run_id: economic_run_id.clone(),
            state: JournalState::Failed,
            reserved_amount: intent.amount(),
            reserved_fee: intent.max_fee(),
            plan: None,
            simulation: None,
            approval: None,
            unsigned: None,
            signed: None,
            broadcast: None,
            reconciliation: None,
            updated_at: intent.created_at,
        };
        let mut events = Vec::new();
        push_event(
            &mut events,
            event(
                "run_started",
                None,
                Some(binding.evidence_digest.to_owned()),
                Some(self.policy.digest.clone()),
                "started",
                Usage::default(),
            )?,
        )?;
        let mut budget = Budget {
            policy_bytes: self.policy.source.len() as u64,
            intent_bytes: intent.source.len() as u64,
            recipients: self
                .policy
                .networks
                .iter()
                .map(|row| row.recipients.len() as u64)
                .sum(),
            network_policies: self.policy.networks.len() as u64,
            x402_origins: self.policy.origins.len() as u64,
            concurrency: 1,
            ..Budget::default()
        };
        self.pre_call(started, self.policy.limits.max_journal_bytes as usize)?;
        let mut sink = EconomicDocumentSink::new(
            self.policy.limits.max_journal_bytes as usize,
            self.cancellation.clone(),
            self.host.boundary_probe(),
            started,
            self.policy.limits.max_elapsed_ms,
            self.policy.limits.max_builder_bytes,
            self.terminal_floor()?,
        );
        let load = self.host.load(&intent.idempotency_key, &mut sink);
        if load != EconomicJournalLoad::Present {
            push_event(
                &mut events,
                event(
                    "journal_loaded",
                    None,
                    None,
                    None,
                    "failed",
                    Usage {
                        journal_reads: 1,
                        ..Usage::default()
                    },
                )?,
            )?;
            return self.finish_failure(
                binding,
                &intent,
                &economic_run_id,
                &journal,
                None,
                None,
                None,
                None,
                None,
                None,
                &mut events,
                &mut budget,
                info("SPX-I222", "Economic Agent journal adapter failed"),
                started,
            );
        }
        let source = match sink.finish("journal_bytes") {
            Ok(source) => source,
            Err(diagnostic) => {
                push_event(
                    &mut events,
                    event(
                        "journal_loaded",
                        None,
                        None,
                        None,
                        "failed",
                        Usage {
                            journal_reads: 1,
                            ..Usage::default()
                        },
                    )?,
                )?;
                return self.finish_failure(
                    binding,
                    &intent,
                    &economic_run_id,
                    &journal,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    &mut events,
                    &mut budget,
                    diagnostic,
                    started,
                );
            }
        };
        push_event(
            &mut events,
            event(
                "journal_loaded",
                None,
                None,
                Some(digest(JOURNAL_DOMAIN, source.as_bytes())),
                "present",
                Usage {
                    journal_reads: 1,
                    output_bytes: source.len() as u64,
                    ..Usage::default()
                },
            )?,
        )?;
        journal = match parse_journal_classified(&source, &self.policy, &intent, &economic_run_id) {
            Ok(journal) => journal,
            Err(JournalParseFailure::BindingMismatch) => return Err(g215()),
            Err(JournalParseFailure::Diagnostic(diagnostic)) => {
                return self.finish_failure(
                    binding,
                    &intent,
                    &economic_run_id,
                    &journal,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    &mut events,
                    &mut budget,
                    diagnostic,
                    started,
                );
            }
        };
        budget.journal_bytes = source.len() as u64;
        self.resume_loaded(
            binding,
            intent,
            economic_run_id,
            journal,
            events,
            budget,
            started,
        )
    }
}
