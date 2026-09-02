//! Cross-document reference helpers and the adapter documents that sit
//! between an admitted intent and the journal: x402 invoice, payment plan,
//! simulation, approval request, and approval.

use super::evidence::{
    write_doc_reference, write_json, write_optional_json, write_optional_reference, CountSink,
};
use super::snapshot::{reserve_parse_sidecar, Snapshot, SnapshotState};
use super::transaction::build_unsigned;
use super::validate::{
    canonical, canonical_policy_limited, configured_document_limits, digest, g210, g212, g213,
    g214, g216, g217, identifier, keys, number, object, rail, text,
};
use super::{
    Doc, EconomicRail, Intent, Limits, Payment, Policy, APPROVAL_DOMAIN, APPROVAL_REQUEST_DOMAIN,
    APPROVAL_REQUEST_SCHEMA, APPROVAL_SCHEMA, INTENT_SCHEMA, INVOICE_DOMAIN, INVOICE_SCHEMA,
    MAX_APPROVAL_BYTES, MAX_INVOICE_BYTES, MAX_SIMULATION_BYTES, PLAN_DOMAIN, PLAN_SCHEMA,
    POLICY_SCHEMA, SIMULATION_DOMAIN, SIMULATION_SCHEMA, SNAPSHOT_SCHEMA, UNSIGNED_DOMAIN,
};
use crate::diagnostic::{quote_json, Diagnostic};
use serde_json::Value;
use std::fmt;

pub(super) fn verify_unsigned(
    intent: &Intent,
    snapshot: &Snapshot,
    bytes: &[u8],
) -> Result<(), Diagnostic> {
    let (expected, _) = build_unsigned(intent, snapshot)?;
    if expected == bytes {
        Ok(())
    } else {
        Err(g213())
    }
}

pub(super) fn doc_ref(schema: &str, doc: &Doc) -> String {
    format!(
        "{{\"schema\":{},\"digest\":{},\"bytes\":{}}}",
        quote_json(schema),
        quote_json(&doc.digest),
        doc.source.len()
    )
}
pub(super) fn agent_ref(run_id: &str, evidence: &str, digest_value: &str) -> String {
    format!("{{\"schema\":\"semaprax.agent-runtime-evidence.v1\",\"digest\":{},\"bytes\":{},\"run_id\":{}}}",quote_json(digest_value),evidence.len(),quote_json(run_id))
}
pub(super) fn ref_matches(value: &Value, schema: &str, doc: &Doc) -> bool {
    let Some(row) = value.as_object() else {
        return false;
    };
    keys(row, &["schema", "digest", "bytes"])
        && row.get("schema").and_then(Value::as_str) == Some(schema)
        && row.get("digest").and_then(Value::as_str) == Some(doc.digest.as_str())
        && row.get("bytes").and_then(Value::as_u64) == u64::try_from(doc.source.len()).ok()
}
pub(super) fn ref_identity_matches(
    value: &Value,
    schema: &str,
    digest_value: &str,
    bytes: usize,
) -> bool {
    let Some(row) = value.as_object() else {
        return false;
    };
    keys(row, &["schema", "digest", "bytes"])
        && row.get("schema").and_then(Value::as_str) == Some(schema)
        && row.get("digest").and_then(Value::as_str) == Some(digest_value)
        && row.get("bytes").and_then(Value::as_u64) == u64::try_from(bytes).ok()
}
pub(super) fn unsigned_ref(bytes: &[u8], format: &str) -> String {
    format!(
        "{{\"digest\":{},\"bytes\":{},\"format\":{}}}",
        quote_json(&digest(UNSIGNED_DOMAIN, bytes)),
        bytes.len(),
        quote_json(format)
    )
}

#[derive(Clone)]
pub(super) struct Invoice {
    pub(super) origin: String,
    pub(super) method: String,
    pub(super) resource: String,
    pub(super) invoice_id: String,
    pub(super) payee: String,
    pub(super) rail: EconomicRail,
    pub(super) network: String,
    pub(super) asset: String,
    pub(super) amount: u64,
    pub(super) max_fee: u64,
    pub(super) expires: u64,
    pub(super) nonce: String,
    pub(super) idempotency: String,
    pub(super) doc: Doc,
}
pub(super) fn render_invoice(i: &Invoice) -> String {
    format!("{{\"schema\":\"{INVOICE_SCHEMA}\",\"origin\":{},\"method\":{},\"resource\":{},\"invoice_id\":{},\"payee\":{},\"settlement_rail\":{},\"network\":{},\"asset\":{},\"amount_atomic\":{},\"max_fee_atomic\":{},\"expires_at_ms\":{},\"nonce\":{},\"idempotency_key\":{}}}\n",quote_json(&i.origin),quote_json(&i.method),quote_json(&i.resource),quote_json(&i.invoice_id),quote_json(&i.payee),quote_json(i.rail.text()),quote_json(&i.network),quote_json(&i.asset),i.amount,i.max_fee,i.expires,quote_json(&i.nonce),quote_json(&i.idempotency))
}
pub(super) fn parse_invoice(source: &str, intent: &Intent) -> Result<Invoice, Diagnostic> {
    let (_, value) = canonical(source, "x402 invoice", INVOICE_SCHEMA, MAX_INVOICE_BYTES)?;
    let row = object(&value, "x402 invoice", INVOICE_SCHEMA)?;
    if !keys(
        row,
        &[
            "schema",
            "origin",
            "method",
            "resource",
            "invoice_id",
            "payee",
            "settlement_rail",
            "network",
            "asset",
            "amount_atomic",
            "max_fee_atomic",
            "expires_at_ms",
            "nonce",
            "idempotency_key",
        ],
    ) {
        return Err(g210("x402 invoice", INVOICE_SCHEMA));
    }
    let i = Invoice {
        origin: text(row, "origin", "x402 invoice", INVOICE_SCHEMA)?.to_owned(),
        method: text(row, "method", "x402 invoice", INVOICE_SCHEMA)?.to_owned(),
        resource: text(row, "resource", "x402 invoice", INVOICE_SCHEMA)?.to_owned(),
        invoice_id: text(row, "invoice_id", "x402 invoice", INVOICE_SCHEMA)?.to_owned(),
        payee: text(row, "payee", "x402 invoice", INVOICE_SCHEMA)?.to_owned(),
        rail: rail(text(
            row,
            "settlement_rail",
            "x402 invoice",
            INVOICE_SCHEMA,
        )?)
        .ok_or_else(g213)?,
        network: text(row, "network", "x402 invoice", INVOICE_SCHEMA)?.to_owned(),
        asset: text(row, "asset", "x402 invoice", INVOICE_SCHEMA)?.to_owned(),
        amount: number(row, "amount_atomic", "x402 invoice", INVOICE_SCHEMA)?,
        max_fee: number(row, "max_fee_atomic", "x402 invoice", INVOICE_SCHEMA)?,
        expires: number(row, "expires_at_ms", "x402 invoice", INVOICE_SCHEMA)?,
        nonce: text(row, "nonce", "x402 invoice", INVOICE_SCHEMA)?.to_owned(),
        idempotency: text(row, "idempotency_key", "x402 invoice", INVOICE_SCHEMA)?.to_owned(),
        doc: Doc {
            source: source.to_owned(),
            digest: digest(INVOICE_DOMAIN, source.as_bytes()),
        },
    };
    if render_invoice(&i) != source {
        return Err(g210("x402 invoice", INVOICE_SCHEMA));
    }
    if let Payment::X402 {
        origin,
        method,
        resource,
        invoice_digest,
        payee,
        rail,
        network,
        asset,
        amount,
        max_fee,
        invoice_expires,
        nonce,
    } = &intent.payment
    {
        if i.origin != *origin
            || i.method != *method
            || i.resource != *resource
            || i.doc.digest != *invoice_digest
            || i.payee != *payee
            || i.rail != *rail
            || i.network != *network
            || i.asset != *asset
            || i.amount != *amount
            || i.max_fee != *max_fee
            || i.expires != *invoice_expires
            || i.nonce != *nonce
            || i.idempotency != intent.idempotency_key
        {
            return Err(g213());
        }
    } else {
        return Err(g213());
    }
    Ok(i)
}
pub(super) fn parse_invoice_limited(
    source: &str,
    intent: &Intent,
    limits: &Limits,
) -> Result<Invoice, Diagnostic> {
    configured_document_limits(source, "x402 invoice", limits.max_invoice_bytes, limits)?;
    reserve_parse_sidecar(source, limits)?;
    let invoice = parse_invoice(source, intent)?;
    if [
        invoice.invoice_id.as_str(),
        invoice.nonce.as_str(),
        invoice.idempotency.as_str(),
    ]
    .into_iter()
    .any(|value| value.len() > limits.max_identifier_bytes as usize)
    {
        return Err(g216("identifier_bytes", limits.max_identifier_bytes));
    }
    Ok(invoice)
}

#[derive(Clone)]
pub(super) struct Plan {
    pub(super) doc: Doc,
    pub(super) unsigned: Vec<u8>,
    pub(super) unsigned_digest: String,
    pub(super) format: &'static str,
    pub(super) observed: u64,
    pub(super) expires: u64,
    pub(super) utxos: u64,
}
pub(super) fn make_plan(
    run_id: &str,
    agent_run_id: &str,
    agent_evidence: &str,
    agent_digest: &str,
    policy: &Policy,
    intent: &Intent,
    invoice: Option<&Invoice>,
    snapshot: &Snapshot,
    unsigned: Vec<u8>,
    format: &'static str,
) -> Result<Plan, Diagnostic> {
    if snapshot.observed < intent.created_at
        || snapshot.observed >= intent.expires_at
        || invoice.is_some_and(|value| snapshot.observed >= value.expires)
    {
        return Err(g212("expired"));
    }
    let unsigned_digest = digest(UNSIGNED_DOMAIN, &unsigned);
    let expires = snapshot
        .expires
        .min(intent.expires_at)
        .min(invoice.map_or(u64::MAX, |value| value.expires));
    if expires <= snapshot.observed {
        return Err(g212("expired"));
    }
    let mut count = CountSink::default();
    write_plan(
        &mut count,
        run_id,
        agent_run_id,
        agent_evidence,
        agent_digest,
        policy,
        intent,
        invoice,
        snapshot,
        &unsigned,
        format,
        &unsigned_digest,
        expires,
    )
    .map_err(|_| g217())?;
    if count.0 > policy.limits.max_plan_bytes as usize {
        return Err(g216("plan_bytes", policy.limits.max_plan_bytes));
    }
    let mut source = String::with_capacity(count.0);
    write_plan(
        &mut source,
        run_id,
        agent_run_id,
        agent_evidence,
        agent_digest,
        policy,
        intent,
        invoice,
        snapshot,
        &unsigned,
        format,
        &unsigned_digest,
        expires,
    )
    .map_err(|_| g217())?;
    let doc = Doc {
        digest: digest(PLAN_DOMAIN, source.as_bytes()),
        source,
    };
    let utxos = match &snapshot.state {
        SnapshotState::Bitcoin { utxos, .. } => utxos.len() as u64,
        _ => 0,
    };
    Ok(Plan {
        doc,
        unsigned,
        unsigned_digest,
        format,
        observed: snapshot.observed,
        expires,
        utxos,
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn write_plan<W: fmt::Write>(
    output: &mut W,
    run_id: &str,
    agent_run_id: &str,
    agent_evidence: &str,
    agent_digest: &str,
    policy: &Policy,
    intent: &Intent,
    invoice: Option<&Invoice>,
    snapshot: &Snapshot,
    unsigned: &[u8],
    format: &str,
    unsigned_digest: &str,
    expires: u64,
) -> fmt::Result {
    output.write_str("{\"schema\":")?;
    write_json(output, PLAN_SCHEMA)?;
    output.write_str(",\"run_id\":")?;
    write_json(output, run_id)?;
    output.write_str(
        ",\"source_agent_evidence\":{\"schema\":\"semaprax.agent-runtime-evidence.v1\",\"digest\":",
    )?;
    write_json(output, agent_digest)?;
    write!(output, ",\"bytes\":{},\"run_id\":", agent_evidence.len())?;
    write_json(output, agent_run_id)?;
    output.write_char('}')?;
    output.write_str(",\"policy\":")?;
    write_doc_reference(output, POLICY_SCHEMA, &policy.digest, policy.source.len())?;
    output.write_str(",\"intent\":")?;
    write_doc_reference(output, INTENT_SCHEMA, &intent.digest, intent.source.len())?;
    output.write_str(",\"x402_invoice\":")?;
    write_optional_reference(output, INVOICE_SCHEMA, invoice.map(|v| &v.doc))?;
    output.write_str(",\"chain_snapshot\":")?;
    write_doc_reference(
        output,
        SNAPSHOT_SCHEMA,
        &snapshot.doc.digest,
        snapshot.doc.source.len(),
    )?;
    output.write_str(",\"rail\":")?;
    write_json(output, intent.settlement_rail().text())?;
    let (network, asset) = intent.network_asset();
    output.write_str(",\"network\":")?;
    write_json(output, network)?;
    output.write_str(",\"asset\":")?;
    write_json(output, asset)?;
    output.write_str(",\"wallet_id\":")?;
    write_json(output, &intent.wallet_id)?;
    output.write_str(",\"recipient\":")?;
    write_json(output, intent.recipient())?;
    write!(
        output,
        ",\"amount_atomic\":{},\"max_fee_atomic\":{}",
        intent.amount(),
        intent.max_fee()
    )?;
    output.write_str(",\"unsigned_transaction\":{\"digest\":")?;
    write_json(output, unsigned_digest)?;
    write!(output, ",\"bytes\":{},\"format\":", unsigned.len())?;
    write_json(output, format)?;
    writeln!(output, "}},\"expires_at_ms\":{expires}}}")
}

#[derive(Clone)]
pub(super) struct Simulation {
    pub(super) doc: Doc,
    pub(super) fee: u64,
    pub(super) expires: u64,
}
pub(super) fn parse_simulation(
    source: &str,
    plan: &Plan,
    intent: &Intent,
) -> Result<Simulation, Diagnostic> {
    let (_, value) = canonical(
        source,
        "simulation",
        SIMULATION_SCHEMA,
        MAX_SIMULATION_BYTES,
    )?;
    let row = object(&value, "simulation", SIMULATION_SCHEMA)?;
    if !keys(
        row,
        &[
            "schema",
            "plan",
            "success",
            "fee_atomic",
            "balance_before_atomic",
            "balance_after_atomic",
            "allowance_atomic",
            "units",
            "expires_at_ms",
        ],
    ) {
        return Err(g210("simulation", SIMULATION_SCHEMA));
    }
    if !ref_matches(&row["plan"], PLAN_SCHEMA, &plan.doc) || row["success"].as_bool() != Some(true)
    {
        return Err(g213());
    }
    let fee = number(row, "fee_atomic", "simulation", SIMULATION_SCHEMA)?;
    if fee > intent.max_fee() {
        return Err(g213());
    }
    let before = number(
        row,
        "balance_before_atomic",
        "simulation",
        SIMULATION_SCHEMA,
    )?;
    let after = number(row, "balance_after_atomic", "simulation", SIMULATION_SCHEMA)?;
    if after
        .checked_add(intent.amount())
        .and_then(|value| value.checked_add(fee))
        != Some(before)
    {
        return Err(g213());
    }
    if intent.settlement_rail() == EconomicRail::Evm && row["allowance_atomic"].as_u64() != Some(0)
    {
        return Err(g213());
    }
    if intent.settlement_rail() != EconomicRail::Evm && !row["allowance_atomic"].is_null() {
        return Err(g213());
    }
    let units = number(row, "units", "simulation", SIMULATION_SCHEMA)?;
    if intent.settlement_rail() == EconomicRail::Evm && units != 21_000 {
        return Err(g213());
    }
    if let Payment::Solana { compute, .. } = &intent.payment {
        if units != *compute {
            return Err(g213());
        }
    }
    // An x402-over-Solana intent executes through the synthesized direct
    // Solana payment with the fixed compute-unit budget, so its declared
    // units must match that exact budget just like a native Solana intent.
    if matches!(
        &intent.payment,
        Payment::X402 {
            rail: EconomicRail::Solana,
            ..
        }
    ) && units != 200_000
    {
        return Err(g213());
    }
    let expires = number(row, "expires_at_ms", "simulation", SIMULATION_SCHEMA)?;
    if expires <= plan.observed || expires > plan.expires {
        return Err(g213());
    }
    let plan_ref = doc_ref(PLAN_SCHEMA, &plan.doc);
    let canonical_source=format!("{{\"schema\":\"{SIMULATION_SCHEMA}\",\"plan\":{},\"success\":true,\"fee_atomic\":{fee},\"balance_before_atomic\":{before},\"balance_after_atomic\":{after},\"allowance_atomic\":{},\"units\":{units},\"expires_at_ms\":{expires}}}\n",plan_ref,if intent.settlement_rail()==EconomicRail::Evm{"0"}else{"null"});
    if canonical_source != source {
        return Err(g210("simulation", SIMULATION_SCHEMA));
    }
    Ok(Simulation {
        doc: Doc {
            source: source.to_owned(),
            digest: digest(SIMULATION_DOMAIN, source.as_bytes()),
        },
        fee,
        expires,
    })
}
pub(super) fn parse_simulation_limited(
    source: &str,
    plan: &Plan,
    intent: &Intent,
    limits: &Limits,
) -> Result<Simulation, Diagnostic> {
    configured_document_limits(source, "simulation", limits.max_simulation_bytes, limits)?;
    reserve_parse_sidecar(source, limits)?;
    parse_simulation(source, plan, intent)
}

pub(super) fn make_approval_request(
    run_id: &str,
    policy: &Policy,
    intent: &Intent,
    plan: &Plan,
    simulation: &Simulation,
) -> Result<Doc, Diagnostic> {
    let mut count = CountSink::default();
    write_approval_request(&mut count, run_id, policy, intent, plan, simulation)
        .map_err(|_| g217())?;
    if count.0 > policy.limits.max_approval_request_bytes as usize {
        return Err(g216(
            "approval_request_bytes",
            policy.limits.max_approval_request_bytes,
        ));
    }
    let mut source = String::with_capacity(count.0);
    write_approval_request(&mut source, run_id, policy, intent, plan, simulation)
        .map_err(|_| g217())?;
    canonical_policy_limited(
        &source,
        "approval request",
        APPROVAL_REQUEST_SCHEMA,
        policy.limits.max_approval_request_bytes,
        policy.limits.max_json_depth,
    )?;
    Ok(Doc {
        digest: digest(APPROVAL_REQUEST_DOMAIN, source.as_bytes()),
        source,
    })
}
pub(super) fn write_approval_request<W: fmt::Write>(
    output: &mut W,
    run_id: &str,
    policy: &Policy,
    intent: &Intent,
    plan: &Plan,
    simulation: &Simulation,
) -> fmt::Result {
    output.write_str("{\"schema\":")?;
    write_json(output, APPROVAL_REQUEST_SCHEMA)?;
    output.write_str(",\"run_id\":")?;
    write_json(output, run_id)?;
    output.write_str(",\"wallet_id\":")?;
    write_json(output, &intent.wallet_id)?;
    output.write_str(",\"rail\":")?;
    write_json(output, intent.settlement_rail().text())?;
    let (network, asset) = intent.network_asset();
    output.write_str(",\"network\":")?;
    write_json(output, network)?;
    output.write_str(",\"asset\":")?;
    write_json(output, asset)?;
    output.write_str(",\"recipient\":")?;
    write_json(output, intent.recipient())?;
    write!(
        output,
        ",\"amount_atomic\":{},\"max_fee_atomic\":{}",
        intent.amount(),
        intent.max_fee()
    )?;
    let x402 = match &intent.payment {
        Payment::X402 {
            origin,
            method,
            resource,
            ..
        } => Some((origin.as_str(), method.as_str(), resource.as_str())),
        _ => None,
    };
    output.write_str(",\"origin\":")?;
    write_optional_json(output, x402.map(|v| v.0))?;
    output.write_str(",\"method\":")?;
    write_optional_json(output, x402.map(|v| v.1))?;
    output.write_str(",\"resource\":")?;
    write_optional_json(output, x402.map(|v| v.2))?;
    output.write_str(",\"policy\":")?;
    write_doc_reference(output, POLICY_SCHEMA, &policy.digest, policy.source.len())?;
    output.write_str(",\"intent\":")?;
    write_doc_reference(output, INTENT_SCHEMA, &intent.digest, intent.source.len())?;
    output.write_str(",\"plan\":")?;
    write_doc_reference(output, PLAN_SCHEMA, &plan.doc.digest, plan.doc.source.len())?;
    output.write_str(",\"simulation\":")?;
    write_doc_reference(
        output,
        SIMULATION_SCHEMA,
        &simulation.doc.digest,
        simulation.doc.source.len(),
    )?;
    writeln!(output, ",\"expires_at_ms\":{}}}", simulation.expires)
}

#[derive(Clone)]
pub(super) struct Approval {
    pub(super) doc: Doc,
}
pub(super) fn parse_approval(
    source: &str,
    policy: &Policy,
    intent: &Intent,
    plan: &Plan,
    simulation: &Simulation,
    request: &Doc,
) -> Result<Approval, Diagnostic> {
    let (_, value) = canonical(source, "approval", APPROVAL_SCHEMA, MAX_APPROVAL_BYTES)?;
    let row = object(&value, "approval", APPROVAL_SCHEMA)?;
    if !keys(
        row,
        &[
            "schema",
            "approval_id",
            "approver_id",
            "policy",
            "intent",
            "plan",
            "simulation",
            "approval_request",
            "decision",
            "approved_amount_atomic",
            "approved_fee_atomic",
            "expires_at_ms",
        ],
    ) {
        return Err(g210("approval", APPROVAL_SCHEMA));
    }
    let approval_expires = number(row, "expires_at_ms", "approval", APPROVAL_SCHEMA)?;
    let approval_id = text(row, "approval_id", "approval", APPROVAL_SCHEMA)?;
    let approver_id = text(row, "approver_id", "approval", APPROVAL_SCHEMA)?;
    if approval_id.len() > policy.limits.max_identifier_bytes as usize
        || approver_id.len() > policy.limits.max_identifier_bytes as usize
    {
        return Err(g216("identifier_bytes", policy.limits.max_identifier_bytes));
    }
    if !identifier(approval_id)
        || !identifier(approver_id)
        || text(row, "decision", "approval", APPROVAL_SCHEMA)? != "approved"
        || number(row, "approved_amount_atomic", "approval", APPROVAL_SCHEMA)? != intent.amount()
        || number(row, "approved_fee_atomic", "approval", APPROVAL_SCHEMA)? != intent.max_fee()
        || approval_expires <= plan.observed
        || approval_expires > simulation.expires
    {
        return Err(g214());
    }
    let refs = [
        (
            "policy",
            POLICY_SCHEMA,
            Doc {
                source: policy.source.clone(),
                digest: policy.digest.clone(),
            },
        ),
        (
            "intent",
            INTENT_SCHEMA,
            Doc {
                source: intent.source.clone(),
                digest: intent.digest.clone(),
            },
        ),
        ("plan", PLAN_SCHEMA, plan.doc.clone()),
        ("simulation", SIMULATION_SCHEMA, simulation.doc.clone()),
        ("approval_request", APPROVAL_REQUEST_SCHEMA, request.clone()),
    ];
    if refs
        .iter()
        .any(|(key, schema, doc)| !ref_matches(&row[*key], schema, doc))
    {
        return Err(g214());
    }
    let canonical_source=format!("{{\"schema\":\"{APPROVAL_SCHEMA}\",\"approval_id\":{},\"approver_id\":{},\"policy\":{},\"intent\":{},\"plan\":{},\"simulation\":{},\"approval_request\":{},\"decision\":\"approved\",\"approved_amount_atomic\":{},\"approved_fee_atomic\":{},\"expires_at_ms\":{}}}\n",quote_json(text(row,"approval_id","approval",APPROVAL_SCHEMA)?),quote_json(text(row,"approver_id","approval",APPROVAL_SCHEMA)?),doc_ref(POLICY_SCHEMA,&Doc{source:policy.source.clone(),digest:policy.digest.clone()}),doc_ref(INTENT_SCHEMA,&Doc{source:intent.source.clone(),digest:intent.digest.clone()}),doc_ref(PLAN_SCHEMA,&plan.doc),doc_ref(SIMULATION_SCHEMA,&simulation.doc),doc_ref(APPROVAL_REQUEST_SCHEMA,request),intent.amount(),intent.max_fee(),number(row,"expires_at_ms","approval",APPROVAL_SCHEMA)?);
    if canonical_source != source {
        return Err(g210("approval", APPROVAL_SCHEMA));
    }
    Ok(Approval {
        doc: Doc {
            source: source.to_owned(),
            digest: digest(APPROVAL_DOMAIN, source.as_bytes()),
        },
    })
}
pub(super) fn parse_approval_limited(
    source: &str,
    policy: &Policy,
    intent: &Intent,
    plan: &Plan,
    simulation: &Simulation,
    request: &Doc,
) -> Result<Approval, Diagnostic> {
    configured_document_limits(
        source,
        "approval",
        policy.limits.max_approval_bytes,
        &policy.limits,
    )?;
    reserve_parse_sidecar(source, &policy.limits)?;
    parse_approval(source, policy, intent, plan, simulation, request)
}
pub(super) fn approval_expires(approval: &Approval) -> u64 {
    serde_json::from_str::<Value>(approval.doc.source.trim_end())
        .ok()
        .and_then(|value| value.get("expires_at_ms").and_then(Value::as_u64))
        .unwrap_or(0)
}
