//! Replay of a rendered trace against its evidence, event construction,
//! compare-and-swap journal persistence, and run finalization.

use super::documents::{Approval, Invoice, Plan, Simulation};
use super::evidence::{
    journal_identity, render_evidence, render_trace, write_evidence, write_trace, CountSink,
    DigestSink, EvidenceParts, MatchSink,
};
use super::journal::{
    clone_journal_bounded, write_journal, BroadcastReceipt, Journal, Reconciliation,
};
use super::validate::{digest, g212, g215, g216, g217, info};
use super::{
    Budget, Doc, EconomicAdapterDisposition, EconomicRail, EconomicRollingReservationUpdate,
    EconomicRun, EconomicRunStatus, Event, Intent, PaymentJournal, Policy, Terminal, Usage,
    EVIDENCE_DOMAIN, JOURNAL_DOMAIN, MAX_BUILDER_BYTES, TRACE_DOMAIN,
};
use crate::bounded_output::{active_limit, active_remaining, clear_active_floor, reserve_active};
use crate::diagnostic::Diagnostic;
use std::collections::BTreeMap;

pub(super) fn cumulative_usage(events: &[Event]) -> Result<Usage, Diagnostic> {
    let mut total = Usage::default();
    for event in events {
        macro_rules! add {
            ($field:ident) => {
                total.$field = total
                    .$field
                    .checked_add(event.usage.$field)
                    .ok_or_else(g217)?
            };
        }
        add!(journal_reads);
        add!(journal_writes);
        add!(invoice_reads);
        add!(snapshot_reads);
        add!(simulations);
        add!(approvals);
        add!(signatures);
        add!(broadcasts);
        add!(reconciliations);
        add!(input_bytes);
        add!(output_bytes);
        add!(elapsed_ms);
    }
    Ok(total)
}
pub(super) fn valid_event(kind: &str, status: &str) -> bool {
    match kind {
        "run_started" => status == "started",
        "journal_loaded" => matches!(status, "missing" | "present" | "failed"),
        "intent_reserved" => matches!(status, "reserved" | "failed"),
        "invoice_loaded" | "snapshot_loaded" => matches!(status, "loaded" | "failed"),
        "plan_built" => matches!(status, "built" | "failed"),
        "simulation_finished" => matches!(status, "succeeded" | "rejected" | "failed"),
        "approval_finished" => matches!(status, "approved" | "rejected" | "failed"),
        "transaction_signed" => matches!(status, "signed" | "failed"),
        "broadcast_finished" => matches!(
            status,
            "accepted" | "pending" | "unknown" | "rejected" | "failed"
        ),
        "reconciliation_finished" => matches!(
            status,
            "pending" | "confirmed" | "reorged" | "dropped" | "failed"
        ),
        "journal_committed" => matches!(status, "committed" | "failed"),
        "run_finished" => matches!(
            status,
            "confirmed"
                | "pending"
                | "reorged"
                | "dropped"
                | "rejected"
                | "cancelled"
                | "deadline_exceeded"
                | "budget_exhausted"
                | "journal_failed"
                | "adapter_failed"
                | "approval_failed"
                | "custody_failed"
                | "broadcast_unknown"
                | "reconciliation_failed"
        ),
        _ => false,
    }
}
pub(super) fn replay_events(events: &[Event], terminal: &Terminal) -> Result<(), Diagnostic> {
    if events
        .first()
        .is_none_or(|event| event.kind != "run_started")
        || events.last().is_none_or(|event| {
            event.kind != "run_finished" || event.status != terminal.status.text()
        })
        || events
            .iter()
            .any(|event| !valid_event(event.kind, event.status))
    {
        return Err(g217());
    }
    let order = [
        "run_started",
        "journal_loaded",
        "intent_reserved",
        "invoice_loaded",
        "snapshot_loaded",
        "plan_built",
        "simulation_finished",
        "approval_finished",
        "transaction_signed",
        "broadcast_finished",
        "reconciliation_finished",
        "run_finished",
    ];
    let mut previous = 0usize;
    let mut counts = BTreeMap::<&str, u64>::new();
    for event in events {
        let count = counts.entry(event.kind).or_default();
        *count = count.checked_add(1).ok_or_else(g217)?;
        if event.kind == "journal_committed" {
            continue;
        }
        let Some(position) = order.iter().position(|kind| *kind == event.kind) else {
            return Err(g217());
        };
        if position < previous {
            return Err(g217());
        }
        previous = position;
    }
    if counts.get("run_started") != Some(&1)
        || counts.get("journal_loaded") != Some(&1)
        || counts.get("run_finished") != Some(&1)
        || counts.get("transaction_signed").copied().unwrap_or(0) > 1
        || counts.get("broadcast_finished").copied().unwrap_or(0) > 1
    {
        return Err(g217());
    }
    for kind in [
        "intent_reserved",
        "invoice_loaded",
        "snapshot_loaded",
        "plan_built",
        "simulation_finished",
        "approval_finished",
        "reconciliation_finished",
    ] {
        if counts.get(kind).copied().unwrap_or(0) > 1 {
            return Err(g217());
        }
    }
    let has = |kind: &str, status: &str| {
        events
            .iter()
            .any(|event| event.kind == kind && event.status == status)
    };
    let successful_fresh_prefix = has("intent_reserved", "reserved")
        && has("snapshot_loaded", "loaded")
        && has("plan_built", "built")
        && has("simulation_finished", "succeeded")
        && has("approval_finished", "approved")
        && has("transaction_signed", "signed");
    let broadcast_terminal = matches!(
        terminal.status,
        EconomicRunStatus::Confirmed
            | EconomicRunStatus::Pending
            | EconomicRunStatus::Reorged
            | EconomicRunStatus::Dropped
            | EconomicRunStatus::BroadcastUnknown
    );
    let fresh_invocation = counts.get("intent_reserved").copied().unwrap_or(0) != 0;
    if broadcast_terminal
        && fresh_invocation
        && (!successful_fresh_prefix
            || !(has("broadcast_finished", "accepted")
                || has("broadcast_finished", "pending")
                || has("broadcast_finished", "unknown")))
    {
        return Err(g217());
    }
    if matches!(
        terminal.status,
        EconomicRunStatus::Confirmed
            | EconomicRunStatus::Pending
            | EconomicRunStatus::Reorged
            | EconomicRunStatus::Dropped
    ) && !events.iter().any(|event| {
        event.kind == "reconciliation_finished"
            && matches!(
                event.status,
                "confirmed" | "pending" | "reorged" | "dropped"
            )
    }) {
        return Err(g217());
    }
    if terminal.status == EconomicRunStatus::BroadcastUnknown
        && ((!has("broadcast_finished", "unknown") && fresh_invocation)
            || counts.get("reconciliation_finished").copied().unwrap_or(0) != 0)
    {
        return Err(g217());
    }
    if let Some(failed) = events.iter().position(|event| {
        event.status == "failed" || matches!(event.status, "rejected" | "unknown")
    }) {
        if events[failed + 1..]
            .iter()
            .any(|event| event.kind != "journal_committed" && event.kind != "run_finished")
        {
            return Err(g217());
        }
    }
    let usage = cumulative_usage(events)?;
    if usage.journal_reads != 1 || usage.signatures > 1 || usage.broadcasts > 1 {
        return Err(g217());
    }
    Ok(())
}
pub(super) fn diagnostic_terminal(diagnostic: &Diagnostic) -> Terminal {
    let (code, message) = (diagnostic.code, diagnostic.message.as_str());
    let status = match code {
        "SPX-I222" => EconomicRunStatus::JournalFailed,
        "SPX-I224" => EconomicRunStatus::ApprovalFailed,
        "SPX-I225" => EconomicRunStatus::CustodyFailed,
        "SPX-I226" => EconomicRunStatus::BroadcastUnknown,
        "SPX-I227" => EconomicRunStatus::ReconciliationFailed,
        "SPX-I228" => EconomicRunStatus::Cancelled,
        "SPX-I229" => EconomicRunStatus::DeadlineExceeded,
        "SPX-G216" => EconomicRunStatus::BudgetExhausted,
        "SPX-G214" => EconomicRunStatus::ApprovalFailed,
        "SPX-G212" => EconomicRunStatus::Rejected,
        _ => EconomicRunStatus::AdapterFailed,
    };
    Terminal {
        status,
        transaction_id: None,
        confirmation: None,
        code: Some(code.to_owned()),
        message: Some(message.to_owned()),
    }
}

pub(super) fn replay_bundle(
    evidence: &Doc,
    trace: &Doc,
    parts: &EvidenceParts<'_>,
    events: &[Event],
) -> Result<(), Diagnostic> {
    replay_events(events, parts.terminal)?;
    let mut trace_match = MatchSink {
        expected: trace.source.as_bytes(),
        offset: 0,
    };
    if write_trace(
        &mut trace_match,
        parts.run_id,
        parts.agent_digest,
        parts.policy,
        parts.intent,
        events,
        parts.terminal,
    )
    .is_err()
        || trace_match.offset != trace.source.len()
        || digest(TRACE_DOMAIN, trace.source.as_bytes()) != trace.digest
    {
        return Err(g217());
    }
    let (_journal_digest, journal_bytes) = journal_identity(parts);
    if digest(EVIDENCE_DOMAIN, evidence.source.as_bytes()) != evidence.digest
        || evidence.source.len() as u64 != parts.budget.evidence_bytes
        || trace.source.len() as u64 != parts.budget.trace_bytes
        || events.len() as u64 != parts.budget.trace_events
    {
        return Err(g217());
    }
    let usage = cumulative_usage(events)?;
    let expected_journal_bytes = journal_bytes as u64;
    let expected_recipients = parts
        .policy
        .networks
        .iter()
        .try_fold(0u64, |sum, row| {
            sum.checked_add(row.recipients.len() as u64)
        })
        .ok_or_else(g217)?;
    let expected_utxos = parts.plan.map_or(0, |plan| plan.utxos);
    if usage.journal_reads != 1
        || usage.reconciliations > parts.budget.reconciliations
        || usage.signatures > 1
        || usage.broadcasts > 1
        || parts.budget.policy_bytes != parts.policy.source.len() as u64
        || parts.budget.intent_bytes != parts.intent.source.len() as u64
        || parts.budget.invoice_bytes
            != parts
                .invoice
                .map_or(0, |value| value.doc.source.len() as u64)
        || parts.budget.plan_bytes != parts.plan.map_or(0, |value| value.doc.source.len() as u64)
        || parts.budget.simulation_bytes
            != parts
                .simulation
                .map_or(0, |value| value.doc.source.len() as u64)
        || parts.budget.approval_bytes
            != parts
                .approval
                .map_or(0, |value| value.doc.source.len() as u64)
        || parts.budget.journal_bytes != expected_journal_bytes
        || parts.budget.unsigned_bytes != parts.plan.map_or(0, |value| value.unsigned.len() as u64)
        || parts.budget.signed_bytes
            != parts
                .journal
                .signed
                .as_ref()
                .map_or(0, |value| value.1 as u64)
        || (parts
            .broadcast
            .is_some_and(|broadcast| broadcast.observed != 0)
            && parts.budget.broadcast_bytes
                != parts
                    .broadcast
                    .map_or(0, |value| value.doc.source.len() as u64))
        || parts.budget.reconciliation_bytes
            != parts
                .reconciliation
                .map_or(0, |value| value.doc.source.len() as u64)
        || parts.budget.recipients != expected_recipients
        || parts.budget.network_policies != parts.policy.networks.len() as u64
        || parts.budget.x402_origins != parts.policy.origins.len() as u64
        || parts.budget.utxos != expected_utxos
        || parts.budget.concurrency != 1
        || parts.budget.unexpected_authority_calls != 0
    {
        return Err(g217());
    }
    let (journal_digest, journal_bytes) = journal_identity(parts);
    let mut evidence_match = MatchSink {
        expected: evidence.source.as_bytes(),
        offset: 0,
    };
    if write_evidence(&mut evidence_match, parts, &journal_digest, journal_bytes).is_err()
        || evidence_match.offset != evidence.source.len()
        || digest(EVIDENCE_DOMAIN, evidence.source.as_bytes()) != evidence.digest
    {
        return Err(g217());
    }
    Ok(())
}

pub(super) fn event(
    kind: &'static str,
    rail: Option<EconomicRail>,
    input: Option<String>,
    output: Option<String>,
    status: &'static str,
    usage: Usage,
) -> Result<Event, Diagnostic> {
    let owned_bytes = input
        .as_ref()
        .map_or(0, String::len)
        .checked_add(output.as_ref().map_or(0, String::len))
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Event>()))
        .unwrap_or(usize::MAX);
    if !reserve_active(owned_bytes) {
        return Err(g216(
            "builder_bytes",
            active_limit().unwrap_or(MAX_BUILDER_BYTES) as u64,
        ));
    }
    Ok(Event {
        kind,
        rail,
        input,
        output,
        status,
        usage,
        authority_uncertain: false,
    })
}

pub(super) fn push_event(events: &mut Vec<Event>, event: Event) -> Result<(), Diagnostic> {
    events.try_reserve(1).map_err(|_| {
        g216(
            "builder_bytes",
            active_limit().unwrap_or(MAX_BUILDER_BYTES) as u64,
        )
    })?;
    events.push(event);
    Ok(())
}
pub(super) fn journal_digest(journal: &Journal) -> String {
    let mut sink = DigestSink::new(JOURNAL_DOMAIN);
    write_journal(&mut sink, journal).expect("journal digest cannot fail");
    sink.finish().0
}
pub(super) fn cas_journal<H: PaymentJournal>(
    host: &mut H,
    journal: &mut Journal,
    events: &mut Vec<Event>,
    budget: &mut Budget,
    maximum: u64,
    rolling: EconomicRollingReservationUpdate<'_>,
) -> Result<(), Diagnostic> {
    let expected = journal.version;
    let authenticated_journal_bytes = budget.journal_bytes;
    let mut current_count = CountSink::default();
    write_journal(&mut current_count, journal).map_err(|_| g215())?;
    budget.journal_bytes = u64::try_from(current_count.0).map_err(|_| g217())?;
    let mut prospective =
        clone_journal_bounded(journal, active_limit().unwrap_or(MAX_BUILDER_BYTES) as u64)?;
    prospective.version = prospective.version.checked_add(1).ok_or_else(g215)?;
    let mut count = CountSink::default();
    write_journal(&mut count, &prospective).map_err(|_| g215())?;
    if count.0 > maximum as usize {
        return Err(g216("journal_bytes", maximum));
    }
    if active_remaining().is_some_and(|remaining| count.0 > remaining) || !reserve_active(count.0) {
        return Err(g216(
            "builder_bytes",
            active_limit().unwrap_or(MAX_BUILDER_BYTES) as u64,
        ));
    }
    let mut source = String::with_capacity(count.0);
    write_journal(&mut source, &prospective).map_err(|_| g215())?;
    let mut journal_match = MatchSink {
        expected: source.as_bytes(),
        offset: 0,
    };
    if write_journal(&mut journal_match, &prospective).is_err()
        || journal_match.offset != source.len()
    {
        return Err(g215());
    }
    let prospective_digest = digest(JOURNAL_DOMAIN, source.as_bytes());
    if !reserve_active(
        prospective_digest
            .len()
            .checked_add(std::mem::size_of::<Event>())
            .ok_or_else(g217)?,
    ) {
        return Err(g216(
            "builder_bytes",
            active_limit().unwrap_or(MAX_BUILDER_BYTES) as u64,
        ));
    }
    events.try_reserve(1).map_err(|_| {
        g216(
            "builder_bytes",
            active_limit().unwrap_or(MAX_BUILDER_BYTES) as u64,
        )
    })?;
    let disposition = host.compare_and_swap(&journal.idempotency_key, expected, &source, rolling);
    let mut usage = Usage::default();
    usage.journal_writes = 1;
    usage.input_bytes = source.len() as u64;
    events.push(Event {
        kind: "journal_committed",
        rail: None,
        input: Some(prospective_digest),
        output: None,
        status: if disposition == EconomicAdapterDisposition::Succeeded {
            "committed"
        } else {
            "failed"
        },
        usage,
        authority_uncertain: disposition == EconomicAdapterDisposition::FailedUncertain,
    });
    if disposition == EconomicAdapterDisposition::FailedUncertain {
        debug_assert!(events.last().is_some_and(|event| event.authority_uncertain));
    }
    if disposition == EconomicAdapterDisposition::PolicyRejected {
        budget.journal_bytes = if expected == 0 {
            current_count.0 as u64
        } else {
            authenticated_journal_bytes
        };
        return Err(g212("amount or fee not allowed"));
    }
    if disposition != EconomicAdapterDisposition::Succeeded {
        budget.journal_bytes = if expected == 0 {
            current_count.0 as u64
        } else {
            authenticated_journal_bytes
        };
        return Err(info("SPX-I222", "Economic Agent journal adapter failed"));
    }
    *journal = prospective;
    budget.journal_bytes = source.len() as u64;
    Ok(())
}
pub(super) fn finish_run(
    run_id: &str,
    binding: &crate::agent_runtime::EconomicAgentBinding<'_>,
    policy: &Policy,
    intent: &Intent,
    invoice: Option<&Invoice>,
    plan: Option<&Plan>,
    simulation: Option<&Simulation>,
    approval: Option<&Approval>,
    journal: &Journal,
    broadcast: Option<&BroadcastReceipt>,
    reconciliation: Option<&Reconciliation>,
    events: &mut Vec<Event>,
    terminal: Terminal,
    budget: &mut Budget,
    elapsed_ms: u64,
) -> Result<EconomicRun, Diagnostic> {
    clear_active_floor();
    push_event(
        events,
        event(
            "run_finished",
            None,
            None,
            None,
            terminal.status.text(),
            Usage::default(),
        )?,
    )?;
    replay_events(events, &terminal)?;
    let usage = cumulative_usage(events)?;
    if usage.journal_reads != 1 || usage.signatures > 1 || usage.broadcasts > 1 {
        return Err(g217());
    }
    budget.trace_events = events.len().try_into().map_err(|_| g217())?;
    budget.elapsed_ms = elapsed_ms.min(policy.limits.max_elapsed_ms);
    let trace = render_trace(
        run_id,
        binding.evidence_digest,
        policy,
        intent,
        events,
        &terminal,
    )?;
    budget.trace_bytes = trace.source.len() as u64;
    let mut parts = EvidenceParts {
        run_id,
        agent_run_id: binding.run_id,
        agent_evidence: binding.evidence,
        agent_digest: binding.evidence_digest,
        policy,
        intent,
        invoice,
        plan,
        simulation,
        approval,
        journal,
        broadcast,
        reconciliation,
        trace: &trace,
        terminal: &terminal,
        budget,
    };
    let evidence = render_evidence(&mut parts)?;
    let exact_builder = policy
        .limits
        .max_builder_bytes
        .checked_sub(active_remaining().ok_or_else(g217)? as u64)
        .ok_or_else(g217)?;
    if exact_builder != parts.budget.builder_bytes {
        return Err(g217());
    }
    replay_bundle(&evidence, &trace, &parts, events)?;
    Ok(EconomicRun {
        status: terminal.status,
        transaction_id: terminal.transaction_id,
        confirmation_status: terminal.confirmation,
        trace: trace.source,
        trace_digest: trace.digest,
        evidence: evidence.source,
        evidence_digest: evidence.digest,
    })
}
