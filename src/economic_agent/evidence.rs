//! Formatting sinks and the two deterministic run artifacts written through
//! them: the execution trace and the evidence capsule.

use super::documents::{Approval, Invoice, Plan, Simulation};
use super::journal::{write_journal, BroadcastReceipt, Journal, Reconciliation};
use super::policy::{limits_json, write_limits};
use super::validate::{digest, g216, g217};
use super::{
    Budget, Doc, EconomicRail, Event, Intent, Limits, Policy, Terminal, Usage, APPROVAL_SCHEMA,
    BROADCAST_SCHEMA, EVIDENCE_DOMAIN, EVIDENCE_SCHEMA, INTENT_SCHEMA, INVOICE_SCHEMA,
    JOURNAL_DOMAIN, JOURNAL_SCHEMA, NONCLAIMS, PLAN_SCHEMA, POLICY_SCHEMA, RECONCILIATION_SCHEMA,
    RUN_ID_DOMAIN, SIMULATION_SCHEMA, TRACE_DOMAIN, TRACE_SCHEMA,
};
use crate::bounded_output::{active_remaining, reserve_active};
use crate::diagnostic::Diagnostic;
use sha2::{Digest, Sha256};
use std::fmt;

#[derive(Default)]
pub(super) struct CountSink(pub(super) usize);
impl fmt::Write for CountSink {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.0 = self.0.checked_add(value.len()).ok_or(fmt::Error)?;
        Ok(())
    }
}

pub(super) struct MatchSink<'a> {
    pub(super) expected: &'a [u8],
    pub(super) offset: usize,
}

pub(super) struct DigestSink {
    pub(super) hash: Sha256,
    pub(super) bytes: usize,
}
impl DigestSink {
    pub(super) fn new(domain: &[u8]) -> Self {
        let mut hash = Sha256::new();
        hash.update(domain);
        Self { hash, bytes: 0 }
    }
    pub(super) fn finish(self) -> (String, usize) {
        (
            format!(
                "sha256:{:x}",
                crate::digest_hex::LowerHex(self.hash.finalize())
            ),
            self.bytes,
        )
    }
}
impl fmt::Write for DigestSink {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.bytes = self.bytes.checked_add(value.len()).ok_or(fmt::Error)?;
        self.hash.update(value.as_bytes());
        Ok(())
    }
}
impl fmt::Write for MatchSink<'_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let end = self.offset.checked_add(value.len()).ok_or(fmt::Error)?;
        if self.expected.get(self.offset..end) != Some(value.as_bytes()) {
            return Err(fmt::Error);
        }
        self.offset = end;
        Ok(())
    }
}

pub(super) fn write_json<W: fmt::Write>(output: &mut W, value: &str) -> fmt::Result {
    output.write_char('"')?;
    for character in value.chars() {
        match character {
            '"' => output.write_str("\\\"")?,
            '\\' => output.write_str("\\\\")?,
            '\n' => output.write_str("\\n")?,
            '\r' => output.write_str("\\r")?,
            '\t' => output.write_str("\\t")?,
            value if value.is_control() => write!(output, "\\u{:04x}", value as u32)?,
            value => output.write_char(value)?,
        }
    }
    output.write_char('"')
}
pub(super) fn write_optional_json<W: fmt::Write>(
    output: &mut W,
    value: Option<&str>,
) -> fmt::Result {
    match value {
        Some(value) => write_json(output, value),
        None => output.write_str("null"),
    }
}
pub(super) fn write_usage<W: fmt::Write>(output: &mut W, usage: &Usage) -> fmt::Result {
    write!(output,"{{\"journal_reads\":{},\"journal_writes\":{},\"invoice_reads\":{},\"snapshot_reads\":{},\"simulations\":{},\"approvals\":{},\"signatures\":{},\"broadcasts\":{},\"reconciliations\":{},\"input_bytes\":{},\"output_bytes\":{},\"elapsed_ms\":{}}}",usage.journal_reads,usage.journal_writes,usage.invoice_reads,usage.snapshot_reads,usage.simulations,usage.approvals,usage.signatures,usage.broadcasts,usage.reconciliations,usage.input_bytes,usage.output_bytes,usage.elapsed_ms)
}
pub(super) fn write_event<W: fmt::Write>(
    output: &mut W,
    index: usize,
    event: &Event,
) -> fmt::Result {
    write!(output, "{{\"index\":{index},\"kind\":")?;
    write_json(output, event.kind)?;
    output.write_str(",\"rail\":")?;
    write_optional_json(output, event.rail.map(EconomicRail::text))?;
    output.write_str(",\"input_digest\":")?;
    write_optional_json(output, event.input.as_deref())?;
    output.write_str(",\"output_digest\":")?;
    write_optional_json(output, event.output.as_deref())?;
    output.write_str(",\"status\":")?;
    write_json(output, event.status)?;
    output.write_str(",\"usage\":")?;
    write_usage(output, &event.usage)?;
    output.write_char('}')
}
pub(super) fn write_result<W: fmt::Write>(output: &mut W, terminal: &Terminal) -> fmt::Result {
    output.write_str("{\"status\":")?;
    write_json(output, terminal.status.text())?;
    output.write_str(",\"transaction_id\":")?;
    write_optional_json(output, terminal.transaction_id.as_deref())?;
    output.write_str(",\"confirmation_status\":")?;
    write_optional_json(output, terminal.confirmation.as_deref())?;
    output.write_str(",\"code\":")?;
    write_optional_json(output, terminal.code.as_deref())?;
    output.write_str(",\"message\":")?;
    write_optional_json(output, terminal.message.as_deref())?;
    output.write_char('}')
}
pub(super) fn write_nonclaims<W: fmt::Write>(output: &mut W) -> fmt::Result {
    output.write_char('[')?;
    for (index, value) in NONCLAIMS.iter().enumerate() {
        if index > 0 {
            output.write_char(',')?;
        }
        write_json(output, value)?;
    }
    output.write_char(']')
}
pub(super) fn write_trace<W: fmt::Write>(
    output: &mut W,
    run_id: &str,
    source_agent_digest: &str,
    policy: &Policy,
    intent: &Intent,
    events: &[Event],
    terminal: &Terminal,
) -> fmt::Result {
    output.write_str("{\"schema\":\"")?;
    output.write_str(TRACE_SCHEMA)?;
    output.write_str("\",\"run_id\":")?;
    write_json(output, run_id)?;
    output.write_str(",\"source_agent_evidence_digest\":")?;
    write_json(output, source_agent_digest)?;
    output.write_str(",\"policy_digest\":")?;
    write_json(output, &policy.digest)?;
    output.write_str(",\"intent_digest\":")?;
    write_json(output, &intent.digest)?;
    output.write_str(",\"events\":[")?;
    for (index, event) in events.iter().enumerate() {
        if index > 0 {
            output.write_char(',')?;
        }
        write_event(output, index, event)?;
    }
    output.write_str("],\"result\":")?;
    write_result(output, terminal)?;
    output.write_str(",\"nonclaims\":")?;
    write_nonclaims(output)?;
    output.write_str("}\n")
}

pub(super) fn usage_json(usage: &Usage) -> String {
    let mut output = String::new();
    write_usage(&mut output, usage).expect("String writes cannot fail");
    output
}
pub(super) fn event_json(index: usize, event: &Event) -> String {
    let mut output = String::new();
    write_event(&mut output, index, event).expect("String writes cannot fail");
    output
}
pub(super) fn result_json(terminal: &Terminal) -> String {
    let mut output = String::new();
    write_result(&mut output, terminal).expect("String writes cannot fail");
    output
}
pub(super) fn render_trace(
    run_id: &str,
    source_agent_digest: &str,
    policy: &Policy,
    intent: &Intent,
    events: &[Event],
    terminal: &Terminal,
) -> Result<Doc, Diagnostic> {
    if events.len() > policy.limits.max_trace_events as usize {
        return Err(g216("trace_events", policy.limits.max_trace_events));
    }
    let mut count = CountSink::default();
    write_trace(
        &mut count,
        run_id,
        source_agent_digest,
        policy,
        intent,
        events,
        terminal,
    )
    .map_err(|_| g217())?;
    if count.0 > policy.limits.max_trace_bytes as usize {
        return Err(g216("trace_bytes", policy.limits.max_trace_bytes));
    }
    if !reserve_active(count.0) {
        return Err(g216("builder_bytes", policy.limits.max_builder_bytes));
    }
    let mut source = String::with_capacity(count.0);
    write_trace(
        &mut source,
        run_id,
        source_agent_digest,
        policy,
        intent,
        events,
        terminal,
    )
    .map_err(|_| g217())?;
    if source.len() != count.0 {
        return Err(g217());
    }
    Ok(Doc {
        digest: digest(TRACE_DOMAIN, source.as_bytes()),
        source,
    })
}
pub(super) fn limits_evidence_json(l: &Limits) -> String {
    limits_json(l)
}
pub(super) fn budget_json(b: &Budget) -> String {
    let mut output = String::new();
    write_budget(&mut output, b).expect("String writes cannot fail");
    output
}
pub(super) fn write_budget<W: fmt::Write>(output: &mut W, b: &Budget) -> fmt::Result {
    write!(output,"{{\"used_policy_bytes\":{},\"used_intent_bytes\":{},\"used_invoice_bytes\":{},\"used_snapshot_bytes\":{},\"used_plan_bytes\":{},\"used_simulation_bytes\":{},\"used_approval_request_bytes\":{},\"used_approval_bytes\":{},\"used_journal_bytes\":{},\"used_unsigned_transaction_bytes\":{},\"used_signed_transaction_bytes\":{},\"used_broadcast_receipt_bytes\":{},\"used_reconciliation_bytes\":{},\"used_trace_events\":{},\"used_trace_bytes\":{},\"used_evidence_bytes\":{},\"used_builder_bytes\":{},\"used_recipients\":{},\"used_network_policies\":{},\"used_x402_origins\":{},\"used_utxos\":{},\"used_reconciliations\":{},\"used_elapsed_ms\":{},\"used_concurrency\":{},\"used_unexpected_authority_calls\":{}}}",b.policy_bytes,b.intent_bytes,b.invoice_bytes,b.snapshot_bytes,b.plan_bytes,b.simulation_bytes,b.approval_request_bytes,b.approval_bytes,b.journal_bytes,b.unsigned_bytes,b.signed_bytes,b.broadcast_bytes,b.reconciliation_bytes,b.trace_events,b.trace_bytes,b.evidence_bytes,b.builder_bytes,b.recipients,b.network_policies,b.x402_origins,b.utxos,b.reconciliations,b.elapsed_ms,b.concurrency,b.unexpected_authority_calls)
}
pub(super) struct EvidenceParts<'a> {
    pub(super) run_id: &'a str,
    pub(super) agent_run_id: &'a str,
    pub(super) agent_evidence: &'a str,
    pub(super) agent_digest: &'a str,
    pub(super) policy: &'a Policy,
    pub(super) intent: &'a Intent,
    pub(super) invoice: Option<&'a Invoice>,
    pub(super) plan: Option<&'a Plan>,
    pub(super) simulation: Option<&'a Simulation>,
    pub(super) approval: Option<&'a Approval>,
    pub(super) journal: &'a Journal,
    pub(super) broadcast: Option<&'a BroadcastReceipt>,
    pub(super) reconciliation: Option<&'a Reconciliation>,
    pub(super) trace: &'a Doc,
    pub(super) terminal: &'a Terminal,
    pub(super) budget: &'a mut Budget,
}

pub(super) fn write_doc_reference<W: fmt::Write>(
    output: &mut W,
    schema: &str,
    digest_value: &str,
    bytes: usize,
) -> fmt::Result {
    output.write_str("{\"schema\":")?;
    write_json(output, schema)?;
    output.write_str(",\"digest\":")?;
    write_json(output, digest_value)?;
    write!(output, ",\"bytes\":{bytes}}}")
}

pub(super) fn write_optional_reference<W: fmt::Write>(
    output: &mut W,
    schema: &str,
    document: Option<&Doc>,
) -> fmt::Result {
    match document {
        Some(document) => {
            write_doc_reference(output, schema, &document.digest, document.source.len())
        }
        None => output.write_str("null"),
    }
}

pub(super) fn write_evidence<W: fmt::Write>(
    output: &mut W,
    parts: &EvidenceParts<'_>,
    journal_digest: &str,
    journal_bytes: usize,
) -> fmt::Result {
    output.write_str("{\"schema\":\"")?;
    output.write_str(EVIDENCE_SCHEMA)?;
    output.write_str("\",\"run_id\":")?;
    write_json(output, parts.run_id)?;
    output.write_str(
        ",\"source_agent\":{\"schema\":\"semaprax.agent-runtime-evidence.v1\",\"digest\":",
    )?;
    write_json(output, parts.agent_digest)?;
    write!(
        output,
        ",\"bytes\":{},\"run_id\":",
        parts.agent_evidence.len()
    )?;
    write_json(output, parts.agent_run_id)?;
    output.write_str("},\"policy\":")?;
    write_doc_reference(
        output,
        POLICY_SCHEMA,
        &parts.policy.digest,
        parts.policy.source.len(),
    )?;
    output.write_str(",\"intent\":")?;
    write_doc_reference(
        output,
        INTENT_SCHEMA,
        &parts.intent.digest,
        parts.intent.source.len(),
    )?;
    output.write_str(",\"x402_invoice\":")?;
    write_optional_reference(output, INVOICE_SCHEMA, parts.invoice.map(|v| &v.doc))?;
    output.write_str(",\"plan\":")?;
    write_optional_reference(output, PLAN_SCHEMA, parts.plan.map(|v| &v.doc))?;
    output.write_str(",\"simulation\":")?;
    write_optional_reference(output, SIMULATION_SCHEMA, parts.simulation.map(|v| &v.doc))?;
    output.write_str(",\"approval\":")?;
    write_optional_reference(output, APPROVAL_SCHEMA, parts.approval.map(|v| &v.doc))?;
    output.write_str(",\"journal\":")?;
    write_doc_reference(output, JOURNAL_SCHEMA, journal_digest, journal_bytes)?;
    output.write_str(",\"broadcast\":")?;
    write_optional_reference(output, BROADCAST_SCHEMA, parts.broadcast.map(|v| &v.doc))?;
    output.write_str(",\"reconciliation\":")?;
    write_optional_reference(
        output,
        RECONCILIATION_SCHEMA,
        parts.reconciliation.map(|v| &v.doc),
    )?;
    output.write_str(",\"trace\":{\"schema\":")?;
    write_json(output, TRACE_SCHEMA)?;
    output.write_str(",\"digest\":")?;
    write_json(output, &parts.trace.digest)?;
    write!(
        output,
        ",\"bytes\":{},\"document\":",
        parts.trace.source.len()
    )?;
    write_json(output, &parts.trace.source)?;
    output.write_str("},\"result\":")?;
    write_result(output, parts.terminal)?;
    output.write_str(",\"limits\":")?;
    write_limits(output, &parts.policy.limits)?;
    output.write_str(",\"budget\":")?;
    write_budget(output, parts.budget)?;
    output.write_str(",\"nonclaims\":")?;
    write_nonclaims(output)?;
    output.write_str("}\n")
}
pub(super) fn journal_identity(parts: &EvidenceParts<'_>) -> (String, usize) {
    let mut sink = DigestSink::new(JOURNAL_DOMAIN);
    write_journal(&mut sink, parts.journal).expect("journal identity cannot fail");
    sink.finish()
}

pub(super) fn render_evidence(parts: &mut EvidenceParts<'_>) -> Result<Doc, Diagnostic> {
    let (journal_digest, journal_bytes) = journal_identity(parts);
    let builder_before_evidence = parts
        .policy
        .limits
        .max_builder_bytes
        .checked_sub(active_remaining().ok_or_else(g217)? as u64)
        .ok_or_else(g217)?;
    let mut evidence_bytes = 0;
    let mut builder_bytes = builder_before_evidence;
    let mut converged = false;
    for _ in 0..24 {
        parts.budget.evidence_bytes = evidence_bytes;
        parts.budget.builder_bytes = builder_bytes;
        let mut count = CountSink::default();
        write_evidence(&mut count, parts, &journal_digest, journal_bytes).map_err(|_| g217())?;
        let next_evidence = u64::try_from(count.0).map_err(|_| g217())?;
        let next_builder = builder_before_evidence
            .checked_add(next_evidence)
            .ok_or_else(g217)?;
        if next_evidence == evidence_bytes && next_builder == builder_bytes {
            converged = true;
            break;
        }
        evidence_bytes = next_evidence;
        builder_bytes = next_builder;
    }
    if !converged {
        return Err(g217());
    }
    if evidence_bytes > parts.policy.limits.max_evidence_bytes {
        return Err(g216(
            "evidence_bytes",
            parts.policy.limits.max_evidence_bytes,
        ));
    }
    parts.budget.evidence_bytes = evidence_bytes;
    parts.budget.builder_bytes = builder_bytes;
    let evidence_len = usize::try_from(evidence_bytes).map_err(|_| g217())?;
    if !reserve_active(evidence_len) {
        return Err(g216("builder_bytes", parts.policy.limits.max_builder_bytes));
    }
    let mut source = String::with_capacity(evidence_len);
    write_evidence(&mut source, parts, &journal_digest, journal_bytes).map_err(|_| g217())?;
    if source.len() != evidence_len {
        return Err(g217());
    }
    Ok(Doc {
        digest: digest(EVIDENCE_DOMAIN, source.as_bytes()),
        source,
    })
}

pub(super) fn run_id(
    agent_digest: &str,
    policy_digest: &str,
    intent_digest: &str,
    idempotency: &str,
) -> String {
    let mut hash = Sha256::new();
    hash.update(RUN_ID_DOMAIN);
    hash.update(agent_digest.as_bytes());
    hash.update(policy_digest.as_bytes());
    hash.update(intent_digest.as_bytes());
    hash.update(idempotency.as_bytes());
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}
