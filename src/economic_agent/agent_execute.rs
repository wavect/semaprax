//! The bounded forward execution path: budget reservation, adapter calls,
//! journal advancement, and broadcast for a fresh Economic Agent run.

use super::documents::{
    approval_expires, make_approval_request, make_plan, parse_approval_limited,
    parse_invoice_limited, parse_simulation_limited,
};
use super::evidence::run_id;
use super::journal::{
    clone_journal_bounded, parse_broadcast_limited, parse_journal, parse_provisional_broadcast,
    Journal, JournalState,
};
use super::replay::{
    cas_journal, diagnostic_terminal, event, finish_run, journal_digest, push_event,
};
use super::snapshot::{parse_snapshot_limited, SnapshotState};
use super::transaction::{build_unsigned_limited, transaction_id, verify_signed};
use super::validate::{admitted_now_from, digest, g212, g213, g216, info};
use super::{
    Budget, Doc, DocRef, EconomicAdapterDisposition, EconomicAgent, EconomicAgentHost,
    EconomicBytesSink, EconomicDocumentSink, EconomicJournalLoad, EconomicRail,
    EconomicRollingReservation, EconomicRollingReservationUpdate, EconomicRun, Event, Intent,
    Payment, Usage, BROADCAST_SCHEMA, JOURNAL_DOMAIN, SIGNED_DOMAIN,
};
use crate::bounded_output::reserve_active;
use crate::diagnostic::{quote_json, Diagnostic};

impl<H: EconomicAgentHost> EconomicAgent<H> {
    pub(super) fn execute_bounded(
        &mut self,
        binding: &crate::agent_runtime::EconomicAgentBinding<'_>,
        intent: Intent,
        started: u64,
    ) -> Result<EconomicRun, Diagnostic> {
        let profile_cost = binding.evidence.len();
        if !reserve_active(profile_cost) {
            return Err(g216("builder_bytes", self.policy.limits.max_builder_bytes));
        }
        let economic_run_id = run_id(
            binding.evidence_digest,
            &self.policy.digest,
            &intent.digest,
            &intent.idempotency_key,
        );
        let policy_doc = Doc {
            source: self.policy.source.clone(),
            digest: self.policy.digest.clone(),
        };
        let intent_doc = Doc {
            source: intent.source.clone(),
            digest: intent.digest.clone(),
        };
        let mut journal = Journal {
            idempotency_key: intent.idempotency_key.clone(),
            version: 0,
            policy: policy_doc,
            intent: intent_doc,
            run_id: economic_run_id.clone(),
            state: JournalState::Reserved,
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
        self.pre_call(started, self.policy.limits.max_journal_bytes as usize)?;
        let mut load_sink = EconomicDocumentSink::new(
            self.policy.limits.max_journal_bytes as usize,
            self.cancellation.clone(),
            self.host.boundary_probe(),
            started,
            self.policy.limits.max_elapsed_ms,
            self.policy.limits.max_builder_bytes,
            self.terminal_floor()?,
        );
        let load = self.host.load(&intent.idempotency_key, &mut load_sink);
        let mut load_usage = Usage::default();
        load_usage.journal_reads = 1;
        let loaded = match load {
            EconomicJournalLoad::Missing => {
                push_event(
                    &mut events,
                    event("journal_loaded", None, None, None, "missing", load_usage)?,
                )?;
                None
            }
            EconomicJournalLoad::Present => {
                let source = match load_sink.finish("journal_bytes") {
                    Ok(source) => source,
                    Err(diagnostic) => {
                        push_event(
                            &mut events,
                            event("journal_loaded", None, None, None, "failed", load_usage)?,
                        )?;
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
                            None,
                            None,
                            &mut events,
                            diagnostic_terminal(&diagnostic),
                            &mut budget,
                            started,
                        );
                    }
                };
                load_usage.output_bytes = source.len() as u64;
                push_event(
                    &mut events,
                    event(
                        "journal_loaded",
                        None,
                        None,
                        Some(digest(JOURNAL_DOMAIN, source.as_bytes())),
                        "present",
                        load_usage,
                    )?,
                )?;
                let parsed = match parse_journal(&source, &self.policy, &intent, &economic_run_id) {
                    Ok(parsed) => parsed,
                    Err(diagnostic) => {
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
                            None,
                            None,
                            &mut events,
                            diagnostic_terminal(&diagnostic),
                            &mut budget,
                            started,
                        );
                    }
                };
                budget.journal_bytes = source.len() as u64;
                Some(parsed)
            }
            EconomicJournalLoad::DefinitelyNotStarted | EconomicJournalLoad::FailedUncertain => {
                push_event(
                    &mut events,
                    event("journal_loaded", None, None, None, "failed", load_usage)?,
                )?;
                let terminal =
                    diagnostic_terminal(&info("SPX-I222", "Economic Agent journal adapter failed"));
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
                    None,
                    None,
                    &mut events,
                    terminal,
                    &mut budget,
                    started,
                );
            }
        };
        if let Some(existing) = loaded {
            if existing.version == 1 && existing.state == JournalState::Reserved {
                return self.execute_reserved(
                    binding,
                    intent,
                    economic_run_id,
                    existing,
                    events,
                    budget,
                    started,
                );
            }
            if existing.version < 6 || existing.broadcast.is_none() {
                budget.signed_bytes = existing.signed.as_ref().map_or(0, |value| value.1 as u64);
                return finish_run(
                    &economic_run_id,
                    binding,
                    &self.policy,
                    &intent,
                    None,
                    None,
                    None,
                    None,
                    &existing,
                    None,
                    None,
                    &mut events,
                    diagnostic_terminal(&info("SPX-I222", "Economic Agent journal adapter failed")),
                    &mut budget,
                    self.elapsed_ms(started)?,
                );
            }
            return self.resume_loaded(
                binding,
                intent,
                economic_run_id,
                existing,
                events,
                budget,
                started,
            );
        }
        let (network, asset) = intent.network_asset();
        let max_rolling = self
            .policy
            .networks
            .iter()
            .find(|row| {
                row.rail == intent.settlement_rail() && row.network == network && row.asset == asset
            })
            .ok_or_else(|| g212("rail/network/asset not allowed"))?
            .max_rolling;
        let rolling = EconomicRollingReservation {
            wallet_id: intent.wallet_id.clone(),
            rail: intent.settlement_rail(),
            network: network.to_owned(),
            asset: asset.to_owned(),
            requested_at_ms: intent.created_at,
            amount_atomic: intent.amount(),
            max_rolling_24h_atomic: max_rolling,
        };
        if let Err(diagnostic) = cas_journal(
            &mut self.host,
            &mut journal,
            &mut events,
            &mut budget,
            self.policy.limits.max_journal_bytes,
            EconomicRollingReservationUpdate::Reserve(&rolling),
        ) {
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
                None,
                None,
                &mut events,
                diagnostic_terminal(&diagnostic),
                &mut budget,
                started,
            );
        }
        push_event(
            &mut events,
            event(
                "intent_reserved",
                Some(intent.settlement_rail()),
                Some(intent.digest.clone()),
                Some(journal_digest(&journal)),
                "reserved",
                Usage::default(),
            )?,
        )?;
        self.execute_reserved(
            binding,
            intent,
            economic_run_id,
            journal,
            events,
            budget,
            started,
        )
    }

    pub(super) fn execute_reserved(
        &mut self,
        binding: &crate::agent_runtime::EconomicAgentBinding<'_>,
        intent: Intent,
        economic_run_id: String,
        mut journal: Journal,
        mut events: Vec<Event>,
        mut budget: Budget,
        started: u64,
    ) -> Result<EconomicRun, Diagnostic> {
        let rail = intent.settlement_rail();
        let (network, _) = intent.network_asset();
        macro_rules! terminal_try {
            ($expression:expr, $invoice:expr, $plan:expr, $simulation:expr, $approval:expr, $broadcast:expr, $reconciliation:expr) => {
                match $expression {
                    Ok(value) => value,
                    Err(diagnostic) => {
                        return self.finish_failure(
                            binding,
                            &intent,
                            &economic_run_id,
                            &journal,
                            $invoice,
                            $plan,
                            $simulation,
                            $approval,
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
        let invoice = if let Payment::X402 {
            origin,
            method,
            resource,
            ..
        } = &intent.payment
        {
            terminal_try!(
                self.pre_call(started, self.policy.limits.max_invoice_bytes as usize),
                None,
                None,
                None,
                None,
                None,
                None
            );
            let mut sink = EconomicDocumentSink::new(
                self.policy.limits.max_invoice_bytes as usize,
                self.cancellation.clone(),
                self.host.boundary_probe(),
                started,
                self.policy.limits.max_elapsed_ms,
                self.policy.limits.max_builder_bytes,
                self.terminal_floor()?,
            );
            let disposition = self.host.fetch_invoice(origin, method, resource, &mut sink);
            let mut usage = Usage::default();
            usage.invoice_reads = 1;
            if disposition != EconomicAdapterDisposition::Succeeded {
                push_event(
                    &mut events,
                    event(
                        "invoice_loaded",
                        Some(rail),
                        Some(intent.digest.clone()),
                        None,
                        "failed",
                        usage,
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
                    info("SPX-I223", "Economic Agent chain adapter failed"),
                    started,
                );
            }
            let source = terminal_try!(
                sink.finish("invoice_bytes"),
                None,
                None,
                None,
                None,
                None,
                None
            );
            usage.output_bytes = source.len() as u64;
            let parsed = terminal_try!(
                parse_invoice_limited(&source, &intent, &self.policy.limits),
                None,
                None,
                None,
                None,
                None,
                None
            );
            budget.invoice_bytes = source.len() as u64;
            push_event(
                &mut events,
                event(
                    "invoice_loaded",
                    Some(rail),
                    Some(intent.digest.clone()),
                    Some(parsed.doc.digest.clone()),
                    "loaded",
                    usage,
                )?,
            )?;
            Some(parsed)
        } else {
            None
        };
        terminal_try!(
            self.pre_call(started, self.policy.limits.max_snapshot_bytes as usize),
            invoice.as_ref(),
            None,
            None,
            None,
            None,
            None
        );
        let mut snapshot_sink = EconomicDocumentSink::new(
            self.policy.limits.max_snapshot_bytes as usize,
            self.cancellation.clone(),
            self.host.boundary_probe(),
            started,
            self.policy.limits.max_elapsed_ms,
            self.policy.limits.max_builder_bytes,
            self.terminal_floor()?,
        );
        let disposition = match rail {
            EconomicRail::Evm => self.host.evm_snapshot(&intent.source, &mut snapshot_sink),
            EconomicRail::Solana => self
                .host
                .solana_snapshot(&intent.source, &mut snapshot_sink),
            EconomicRail::Bitcoin => self
                .host
                .bitcoin_snapshot(&intent.source, &mut snapshot_sink),
        };
        let mut snapshot_usage = Usage::default();
        snapshot_usage.snapshot_reads = 1;
        if disposition != EconomicAdapterDisposition::Succeeded {
            push_event(
                &mut events,
                event(
                    "snapshot_loaded",
                    Some(rail),
                    Some(intent.digest.clone()),
                    None,
                    "failed",
                    snapshot_usage,
                )?,
            )?;
            return self.finish_failure(
                binding,
                &intent,
                &economic_run_id,
                &journal,
                invoice.as_ref(),
                None,
                None,
                None,
                None,
                None,
                &mut events,
                &mut budget,
                info("SPX-I223", "Economic Agent chain adapter failed"),
                started,
            );
        }
        let snapshot_source = terminal_try!(
            snapshot_sink.finish("snapshot_bytes"),
            invoice.as_ref(),
            None,
            None,
            None,
            None,
            None
        );
        snapshot_usage.output_bytes = snapshot_source.len() as u64;
        let snapshot = terminal_try!(
            parse_snapshot_limited(&snapshot_source, rail, &self.policy.limits),
            invoice.as_ref(),
            None,
            None,
            None,
            None,
            None
        );
        budget.snapshot_bytes = snapshot_source.len() as u64;
        if let SnapshotState::Bitcoin { utxos, .. } = &snapshot.state {
            budget.utxos = utxos.len() as u64;
        }
        push_event(
            &mut events,
            event(
                "snapshot_loaded",
                Some(rail),
                Some(intent.digest.clone()),
                Some(snapshot.doc.digest.clone()),
                "loaded",
                snapshot_usage,
            )?,
        )?;
        let (unsigned, format) = terminal_try!(
            build_unsigned_limited(
                &intent,
                &snapshot,
                self.policy.limits.max_unsigned_transaction_bytes,
                self.policy.limits.max_builder_bytes,
                self.terminal_floor()?,
            ),
            invoice.as_ref(),
            None,
            None,
            None,
            None,
            None
        );
        if unsigned.len() > self.policy.limits.max_unsigned_transaction_bytes as usize {
            return Err(g216(
                "unsigned_transaction_bytes",
                self.policy.limits.max_unsigned_transaction_bytes,
            ));
        }
        let plan = terminal_try!(
            make_plan(
                &economic_run_id,
                binding.run_id,
                binding.evidence,
                binding.evidence_digest,
                &self.policy,
                &intent,
                invoice.as_ref(),
                &snapshot,
                unsigned,
                format,
            ),
            invoice.as_ref(),
            None,
            None,
            None,
            None,
            None
        );
        budget.plan_bytes = plan.doc.source.len() as u64;
        budget.unsigned_bytes = plan.unsigned.len() as u64;
        push_event(
            &mut events,
            event(
                "plan_built",
                Some(rail),
                Some(snapshot.doc.digest.clone()),
                Some(plan.doc.digest.clone()),
                "built",
                Usage::default(),
            )?,
        )?;
        terminal_try!(
            self.pre_call(started, self.policy.limits.max_simulation_bytes as usize),
            invoice.as_ref(),
            Some(&plan),
            None,
            None,
            None,
            None
        );
        let mut simulation_sink = EconomicDocumentSink::new(
            self.policy.limits.max_simulation_bytes as usize,
            self.cancellation.clone(),
            self.host.boundary_probe(),
            started,
            self.policy.limits.max_elapsed_ms,
            self.policy.limits.max_builder_bytes,
            self.terminal_floor()?,
        );
        let disposition = match rail {
            EconomicRail::Evm => {
                self.host
                    .evm_simulate(&plan.doc.source, &plan.unsigned, &mut simulation_sink)
            }
            EconomicRail::Solana => {
                self.host
                    .solana_simulate(&plan.doc.source, &plan.unsigned, &mut simulation_sink)
            }
            EconomicRail::Bitcoin => {
                self.host
                    .bitcoin_simulate(&plan.doc.source, &plan.unsigned, &mut simulation_sink)
            }
        };
        let mut sim_usage = Usage::default();
        sim_usage.simulations = 1;
        if disposition != EconomicAdapterDisposition::Succeeded {
            push_event(
                &mut events,
                event(
                    "simulation_finished",
                    Some(rail),
                    Some(plan.doc.digest.clone()),
                    None,
                    "failed",
                    sim_usage,
                )?,
            )?;
            return self.finish_failure(
                binding,
                &intent,
                &economic_run_id,
                &journal,
                invoice.as_ref(),
                Some(&plan),
                None,
                None,
                None,
                None,
                &mut events,
                &mut budget,
                info("SPX-I223", "Economic Agent chain adapter failed"),
                started,
            );
        }
        let simulation_source = terminal_try!(
            simulation_sink.finish("simulation_bytes"),
            invoice.as_ref(),
            Some(&plan),
            None,
            None,
            None,
            None
        );
        sim_usage.output_bytes = simulation_source.len() as u64;
        let simulation = terminal_try!(
            parse_simulation_limited(&simulation_source, &plan, &intent, &self.policy.limits),
            invoice.as_ref(),
            Some(&plan),
            None,
            None,
            None,
            None
        );
        budget.simulation_bytes = simulation_source.len() as u64;
        push_event(
            &mut events,
            event(
                "simulation_finished",
                Some(rail),
                Some(plan.doc.digest.clone()),
                Some(simulation.doc.digest.clone()),
                "succeeded",
                sim_usage,
            )?,
        )?;
        let mut prepared_journal = terminal_try!(
            clone_journal_bounded(&journal, self.policy.limits.max_builder_bytes),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            None,
            None,
            None
        );
        prepared_journal.state = JournalState::Prepared;
        prepared_journal.plan = Some(DocRef::from(&plan.doc));
        prepared_journal.simulation = Some(DocRef::from(&simulation.doc));
        prepared_journal.unsigned = Some((
            plan.unsigned_digest.clone(),
            plan.unsigned.len(),
            plan.format,
        ));
        prepared_journal.updated_at = snapshot.observed;
        terminal_try!(
            cas_journal(
                &mut self.host,
                &mut prepared_journal,
                &mut events,
                &mut budget,
                self.policy.limits.max_journal_bytes,
                EconomicRollingReservationUpdate::Retain,
            ),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            None,
            None,
            None
        );
        journal = prepared_journal;
        let request = terminal_try!(
            make_approval_request(&economic_run_id, &self.policy, &intent, &plan, &simulation),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            None,
            None,
            None
        );
        budget.approval_request_bytes = request.source.len() as u64;
        terminal_try!(
            self.pre_call(started, self.policy.limits.max_approval_bytes as usize),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            None,
            None,
            None
        );
        let mut approval_sink = EconomicDocumentSink::new(
            self.policy.limits.max_approval_bytes as usize,
            self.cancellation.clone(),
            self.host.boundary_probe(),
            started,
            self.policy.limits.max_elapsed_ms,
            self.policy.limits.max_builder_bytes,
            self.terminal_floor()?,
        );
        let disposition = self.host.approve(&request.source, &mut approval_sink);
        let mut approval_usage = Usage::default();
        approval_usage.approvals = 1;
        if disposition != EconomicAdapterDisposition::Succeeded {
            push_event(
                &mut events,
                event(
                    "approval_finished",
                    Some(rail),
                    Some(request.digest.clone()),
                    None,
                    "failed",
                    approval_usage,
                )?,
            )?;
            return self.finish_failure(
                binding,
                &intent,
                &economic_run_id,
                &journal,
                invoice.as_ref(),
                Some(&plan),
                Some(&simulation),
                None,
                None,
                None,
                &mut events,
                &mut budget,
                info("SPX-I224", "Economic Agent approval adapter failed"),
                started,
            );
        }
        let approval_source = terminal_try!(
            approval_sink.finish("approval_bytes"),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            None,
            None,
            None
        );
        approval_usage.output_bytes = approval_source.len() as u64;
        let approval = terminal_try!(
            parse_approval_limited(
                &approval_source,
                &self.policy,
                &intent,
                &plan,
                &simulation,
                &request,
            ),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            None,
            None,
            None
        );
        budget.approval_bytes = approval_source.len() as u64;
        push_event(
            &mut events,
            event(
                "approval_finished",
                Some(rail),
                Some(request.digest.clone()),
                Some(approval.doc.digest.clone()),
                "approved",
                approval_usage,
            )?,
        )?;
        let mut approved_journal = terminal_try!(
            clone_journal_bounded(&journal, self.policy.limits.max_builder_bytes),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            Some(&approval),
            None,
            None
        );
        approved_journal.state = JournalState::Approved;
        approved_journal.approval = Some(DocRef::from(&approval.doc));
        approved_journal.updated_at = snapshot.observed;
        terminal_try!(
            cas_journal(
                &mut self.host,
                &mut approved_journal,
                &mut events,
                &mut budget,
                self.policy.limits.max_journal_bytes,
                EconomicRollingReservationUpdate::Retain,
            ),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            Some(&approval),
            None,
            None
        );
        journal = approved_journal;
        terminal_try!(
            self.pre_call(
                started,
                self.policy.limits.max_signed_transaction_bytes as usize,
            ),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            Some(&approval),
            None,
            None
        );
        let current_ms = terminal_try!(
            admitted_now_from(&intent, snapshot.observed, self.elapsed_ms(started)?),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            Some(&approval),
            None,
            None
        );
        if current_ms >= plan.expires || current_ms >= simulation.expires {
            return self.finish_failure(
                binding,
                &intent,
                &economic_run_id,
                &journal,
                invoice.as_ref(),
                Some(&plan),
                Some(&simulation),
                Some(&approval),
                None,
                None,
                &mut events,
                &mut budget,
                g212("expired"),
                started,
            );
        }
        let mut sign_marker = terminal_try!(
            clone_journal_bounded(&journal, self.policy.limits.max_builder_bytes),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            Some(&approval),
            None,
            None
        );
        terminal_try!(
            cas_journal(
                &mut self.host,
                &mut sign_marker,
                &mut events,
                &mut budget,
                self.policy.limits.max_journal_bytes,
                EconomicRollingReservationUpdate::Retain,
            ),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            Some(&approval),
            None,
            None
        );
        journal = sign_marker;
        terminal_try!(
            self.pre_call(
                started,
                self.policy.limits.max_signed_transaction_bytes as usize,
            ),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            Some(&approval),
            None,
            None
        );
        let mut signed_sink = EconomicBytesSink::new(
            self.policy.limits.max_signed_transaction_bytes as usize,
            self.cancellation.clone(),
            self.host.boundary_probe(),
            started,
            self.policy.limits.max_elapsed_ms,
            self.policy.limits.max_builder_bytes,
            self.terminal_floor()?,
        );
        let disposition = self.host.sign(
            &self.policy.wallet_id,
            rail,
            &plan.unsigned_digest,
            &plan.unsigned,
            &approval.doc.digest,
            &mut signed_sink,
        );
        let mut sign_usage = Usage::default();
        sign_usage.signatures = 1;
        if disposition != EconomicAdapterDisposition::Succeeded {
            push_event(
                &mut events,
                event(
                    "transaction_signed",
                    Some(rail),
                    Some(plan.unsigned_digest.clone()),
                    None,
                    "failed",
                    sign_usage,
                )?,
            )?;
            return self.finish_failure(
                binding,
                &intent,
                &economic_run_id,
                &journal,
                invoice.as_ref(),
                Some(&plan),
                Some(&simulation),
                Some(&approval),
                None,
                None,
                &mut events,
                &mut budget,
                info("SPX-I225", "Economic Agent custody adapter failed"),
                started,
            );
        }
        let signed = match signed_sink.finish() {
            Ok(value) => value,
            Err(diagnostic) => {
                push_event(
                    &mut events,
                    event(
                        "transaction_signed",
                        Some(rail),
                        Some(plan.unsigned_digest.clone()),
                        None,
                        "failed",
                        sign_usage,
                    )?,
                )?;
                return self.finish_failure(
                    binding,
                    &intent,
                    &economic_run_id,
                    &journal,
                    invoice.as_ref(),
                    Some(&plan),
                    Some(&simulation),
                    Some(&approval),
                    None,
                    None,
                    &mut events,
                    &mut budget,
                    diagnostic,
                    started,
                );
            }
        };
        if let Err(diagnostic) = verify_signed(rail, &plan.unsigned, &signed) {
            push_event(
                &mut events,
                event(
                    "transaction_signed",
                    Some(rail),
                    Some(plan.unsigned_digest.clone()),
                    None,
                    "failed",
                    sign_usage,
                )?,
            )?;
            return self.finish_failure(
                binding,
                &intent,
                &economic_run_id,
                &journal,
                invoice.as_ref(),
                Some(&plan),
                Some(&simulation),
                Some(&approval),
                None,
                None,
                &mut events,
                &mut budget,
                diagnostic,
                started,
            );
        }
        let signed_digest = digest(SIGNED_DOMAIN, &signed);
        budget.signed_bytes = signed.len() as u64;
        sign_usage.output_bytes = signed.len() as u64;
        push_event(
            &mut events,
            event(
                "transaction_signed",
                Some(rail),
                Some(plan.unsigned_digest.clone()),
                Some(signed_digest.clone()),
                "signed",
                sign_usage,
            )?,
        )?;
        let mut signed_journal = terminal_try!(
            clone_journal_bounded(&journal, self.policy.limits.max_builder_bytes),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            Some(&approval),
            None,
            None
        );
        signed_journal.state = JournalState::Signed;
        signed_journal.signed = Some((signed_digest.clone(), signed.len()));
        signed_journal.updated_at = snapshot.observed;
        terminal_try!(
            cas_journal(
                &mut self.host,
                &mut signed_journal,
                &mut events,
                &mut budget,
                self.policy.limits.max_journal_bytes,
                EconomicRollingReservationUpdate::Retain,
            ),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            Some(&approval),
            None,
            None
        );
        journal = signed_journal;
        let current_ms = terminal_try!(
            admitted_now_from(&intent, snapshot.observed, self.elapsed_ms(started)?),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            Some(&approval),
            None,
            None
        );
        if current_ms >= plan.expires || current_ms >= approval_expires(&approval) {
            return self.finish_failure(
                binding,
                &intent,
                &economic_run_id,
                &journal,
                invoice.as_ref(),
                Some(&plan),
                Some(&simulation),
                Some(&approval),
                None,
                None,
                &mut events,
                &mut budget,
                g212("expired"),
                started,
            );
        }
        let expected_transaction_id = terminal_try!(
            transaction_id(rail, &signed).ok_or_else(g213),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            Some(&approval),
            None,
            None
        );
        let provisional_source = format!("{{\"schema\":\"{BROADCAST_SCHEMA}\",\"rail\":{},\"network\":{},\"signed_transaction_digest\":{},\"transaction_id\":{},\"disposition\":\"unknown\",\"observed_at_ms\":0}}\n",quote_json(rail.text()),quote_json(network),quote_json(&signed_digest),quote_json(&expected_transaction_id));
        let provisional = terminal_try!(
            parse_provisional_broadcast(
                &provisional_source,
                rail,
                network,
                &signed_digest,
                &expected_transaction_id
            ),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            Some(&approval),
            None,
            None
        );
        terminal_try!(
            self.pre_call(
                started,
                self.policy.limits.max_broadcast_receipt_bytes as usize,
            ),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            Some(&approval),
            None,
            None
        );
        let mut broadcast_marker = terminal_try!(
            clone_journal_bounded(&journal, self.policy.limits.max_builder_bytes),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            Some(&approval),
            Some(&provisional),
            None
        );
        broadcast_marker.state = JournalState::BroadcastUnknown;
        broadcast_marker.broadcast = Some(provisional.doc.clone());
        broadcast_marker.updated_at = 0;
        terminal_try!(
            cas_journal(
                &mut self.host,
                &mut broadcast_marker,
                &mut events,
                &mut budget,
                self.policy.limits.max_journal_bytes,
                EconomicRollingReservationUpdate::Retain,
            ),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            Some(&approval),
            Some(&provisional),
            None
        );
        journal = broadcast_marker;
        terminal_try!(
            self.pre_call(
                started,
                self.policy.limits.max_broadcast_receipt_bytes as usize,
            ),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            Some(&approval),
            Some(&provisional),
            None
        );
        let mut broadcast_sink = EconomicDocumentSink::new(
            self.policy.limits.max_broadcast_receipt_bytes as usize,
            self.cancellation.clone(),
            self.host.boundary_probe(),
            started,
            self.policy.limits.max_elapsed_ms,
            self.policy.limits.max_builder_bytes,
            self.terminal_floor()?,
        );
        let disposition = match rail {
            EconomicRail::Evm => self.host.evm_broadcast(&signed, &mut broadcast_sink),
            EconomicRail::Solana => self.host.solana_broadcast(&signed, &mut broadcast_sink),
            EconomicRail::Bitcoin => self.host.bitcoin_broadcast(&signed, &mut broadcast_sink),
        };
        let mut broadcast_usage = Usage::default();
        broadcast_usage.broadcasts = 1;
        if disposition != EconomicAdapterDisposition::Succeeded {
            push_event(
                &mut events,
                event(
                    "broadcast_finished",
                    Some(rail),
                    Some(signed_digest.clone()),
                    Some(provisional.doc.digest.clone()),
                    "unknown",
                    broadcast_usage,
                )?,
            )?;
            return self.finish_failure(
                binding,
                &intent,
                &economic_run_id,
                &journal,
                invoice.as_ref(),
                Some(&plan),
                Some(&simulation),
                Some(&approval),
                Some(&provisional),
                None,
                &mut events,
                &mut budget,
                info("SPX-I226", "Economic Agent broadcast outcome is uncertain"),
                started,
            );
        }
        let broadcast_source = terminal_try!(
            broadcast_sink.finish("broadcast_receipt_bytes"),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            Some(&approval),
            None,
            None
        );
        broadcast_usage.output_bytes = broadcast_source.len() as u64;
        let broadcast = terminal_try!(
            parse_broadcast_limited(
                &broadcast_source,
                rail,
                network,
                &signed_digest,
                Some(&expected_transaction_id),
                &self.policy.limits,
            ),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            Some(&approval),
            None,
            None
        );
        budget.signed_bytes = journal.signed.as_ref().map_or(0, |value| value.1 as u64);
        budget.broadcast_bytes = broadcast.doc.source.len() as u64;
        budget.broadcast_bytes = broadcast_source.len() as u64;
        push_event(
            &mut events,
            event(
                "broadcast_finished",
                Some(rail),
                Some(signed_digest),
                Some(broadcast.doc.digest.clone()),
                broadcast.disposition,
                broadcast_usage,
            )?,
        )?;
        if broadcast.disposition == "unknown" {
            let mut outcome_journal = terminal_try!(
                clone_journal_bounded(&journal, self.policy.limits.max_builder_bytes),
                invoice.as_ref(),
                Some(&plan),
                Some(&simulation),
                Some(&approval),
                Some(&broadcast),
                None
            );
            outcome_journal.state = JournalState::BroadcastUnknown;
            outcome_journal.broadcast = Some(broadcast.doc.clone());
            outcome_journal.updated_at = broadcast.observed;
            terminal_try!(
                cas_journal(
                    &mut self.host,
                    &mut outcome_journal,
                    &mut events,
                    &mut budget,
                    self.policy.limits.max_journal_bytes,
                    EconomicRollingReservationUpdate::Retain,
                ),
                invoice.as_ref(),
                Some(&plan),
                Some(&simulation),
                Some(&approval),
                Some(&broadcast),
                None
            );
            journal = outcome_journal;
            return self.finish_failure(
                binding,
                &intent,
                &economic_run_id,
                &journal,
                invoice.as_ref(),
                Some(&plan),
                Some(&simulation),
                Some(&approval),
                Some(&broadcast),
                None,
                &mut events,
                &mut budget,
                info("SPX-I226", "Economic Agent broadcast outcome is uncertain"),
                started,
            );
        }
        if broadcast.disposition == "rejected" {
            let mut outcome_journal = terminal_try!(
                clone_journal_bounded(&journal, self.policy.limits.max_builder_bytes),
                invoice.as_ref(),
                Some(&plan),
                Some(&simulation),
                Some(&approval),
                Some(&broadcast),
                None
            );
            outcome_journal.state = JournalState::Rejected;
            outcome_journal.broadcast = Some(broadcast.doc.clone());
            outcome_journal.updated_at = broadcast.observed;
            terminal_try!(
                cas_journal(
                    &mut self.host,
                    &mut outcome_journal,
                    &mut events,
                    &mut budget,
                    self.policy.limits.max_journal_bytes,
                    EconomicRollingReservationUpdate::Retain,
                ),
                invoice.as_ref(),
                Some(&plan),
                Some(&simulation),
                Some(&approval),
                Some(&broadcast),
                None
            );
            journal = outcome_journal;
            return self.finish_failure(
                binding,
                &intent,
                &economic_run_id,
                &journal,
                invoice.as_ref(),
                Some(&plan),
                Some(&simulation),
                Some(&approval),
                Some(&broadcast),
                None,
                &mut events,
                &mut budget,
                info("SPX-I223", "Economic Agent chain adapter failed"),
                started,
            );
        }
        let mut outcome_journal = terminal_try!(
            clone_journal_bounded(&journal, self.policy.limits.max_builder_bytes),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            Some(&approval),
            Some(&broadcast),
            None
        );
        outcome_journal.state = if broadcast.disposition == "pending" {
            JournalState::Pending
        } else {
            JournalState::Broadcasted
        };
        outcome_journal.broadcast = Some(broadcast.doc.clone());
        outcome_journal.updated_at = broadcast.observed;
        terminal_try!(
            cas_journal(
                &mut self.host,
                &mut outcome_journal,
                &mut events,
                &mut budget,
                self.policy.limits.max_journal_bytes,
                EconomicRollingReservationUpdate::Retain,
            ),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            Some(&approval),
            Some(&broadcast),
            None
        );
        journal = outcome_journal;
        self.reconcile_after_broadcast(
            binding,
            &intent,
            &economic_run_id,
            journal,
            invoice.as_ref(),
            &plan,
            &simulation,
            &approval,
            &broadcast,
            events,
            budget,
            started,
        )
    }
}
