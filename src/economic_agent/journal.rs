//! The payment journal: its state machine, rendering, bounded parsing, and
//! the broadcast receipt and reconciliation documents it carries.

use super::documents::{doc_ref, ref_matches};
use super::evidence::{write_doc_reference, write_json, CountSink};
use super::snapshot::reserve_parse_sidecar;
use super::validate::{
    canonical, canonical_policy_limited, configured_document_limits, digest, g210, g213, g215,
    g216, g217, keys, number, object, text, validate_confirmation,
};
use super::{
    Doc, DocRef, EconomicRail, Intent, Limits, Policy, APPROVAL_SCHEMA, BROADCAST_DOMAIN,
    BROADCAST_SCHEMA, INTENT_SCHEMA, JOURNAL_SCHEMA, MAX_BROADCAST_BYTES, MAX_BUILDER_BYTES,
    MAX_IDENTIFIER_BYTES, MAX_RECONCILIATION_BYTES, PLAN_SCHEMA, POLICY_SCHEMA,
    RECONCILIATION_DOMAIN, RECONCILIATION_SCHEMA, SIMULATION_SCHEMA,
};
use crate::bounded_output::{active_limit, active_remaining, reserve_active};
use crate::diagnostic::{quote_json, Diagnostic};
use serde_json::Value;
use std::fmt;

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum JournalState {
    Reserved,
    Prepared,
    Approved,
    Signed,
    BroadcastUnknown,
    Broadcasted,
    Pending,
    Confirmed,
    Reorged,
    Dropped,
    Rejected,
    Cancelled,
    Failed,
}

impl JournalState {
    pub(super) fn text(self) -> &'static str {
        match self {
            Self::Reserved => "reserved",
            Self::Prepared => "prepared",
            Self::Approved => "approved",
            Self::Signed => "signed",
            Self::BroadcastUnknown => "broadcast_unknown",
            Self::Broadcasted => "broadcasted",
            Self::Pending => "pending",
            Self::Confirmed => "confirmed",
            Self::Reorged => "reorged",
            Self::Dropped => "dropped",
            Self::Rejected => "rejected",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }
    pub(super) fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "reserved" => Self::Reserved,
            "prepared" => Self::Prepared,
            "approved" => Self::Approved,
            "signed" => Self::Signed,
            "broadcast_unknown" => Self::BroadcastUnknown,
            "broadcasted" => Self::Broadcasted,
            "pending" => Self::Pending,
            "confirmed" => Self::Confirmed,
            "reorged" => Self::Reorged,
            "dropped" => Self::Dropped,
            "rejected" => Self::Rejected,
            "cancelled" => Self::Cancelled,
            "failed" => Self::Failed,
            _ => return None,
        })
    }
}

#[derive(Clone)]
pub(super) struct Journal {
    pub(super) idempotency_key: String,
    pub(super) version: u64,
    pub(super) policy: Doc,
    pub(super) intent: Doc,
    pub(super) run_id: String,
    pub(super) state: JournalState,
    pub(super) reserved_amount: u64,
    pub(super) reserved_fee: u64,
    pub(super) plan: Option<DocRef>,
    pub(super) simulation: Option<DocRef>,
    pub(super) approval: Option<DocRef>,
    pub(super) unsigned: Option<(String, usize, &'static str)>,
    pub(super) signed: Option<(String, usize)>,
    pub(super) broadcast: Option<Doc>,
    pub(super) reconciliation: Option<Doc>,
    pub(super) updated_at: u64,
}
pub(super) fn journal_owned_bytes(journal: &Journal) -> Result<usize, Diagnostic> {
    let mut total = 0usize;
    let mut add = |value: usize| -> Result<(), Diagnostic> {
        total = total.checked_add(value).ok_or_else(g217)?;
        Ok(())
    };
    for value in [
        journal.idempotency_key.len(),
        journal.policy.source.len(),
        journal.policy.digest.len(),
        journal.intent.source.len(),
        journal.intent.digest.len(),
        journal.run_id.len(),
    ] {
        add(value)?;
    }
    for value in [&journal.plan, &journal.simulation, &journal.approval]
        .into_iter()
        .flatten()
    {
        add(value.digest.len())?;
    }
    if let Some((digest, _, _)) = &journal.unsigned {
        add(digest.len())?;
    }
    if let Some((digest, _)) = &journal.signed {
        add(digest.len())?;
    }
    for value in [&journal.broadcast, &journal.reconciliation]
        .into_iter()
        .flatten()
    {
        add(value.source.len())?;
        add(value.digest.len())?;
    }
    Ok(total)
}
pub(super) fn clone_journal_bounded(
    journal: &Journal,
    builder_max: u64,
) -> Result<Journal, Diagnostic> {
    let bytes = journal_owned_bytes(journal)?;
    if active_remaining().is_some_and(|remaining| bytes > remaining) || !reserve_active(bytes) {
        return Err(g216("builder_bytes", builder_max));
    }
    Ok(journal.clone())
}

pub(super) fn optional_ref(schema: &str, doc: Option<&Doc>) -> String {
    doc.map_or_else(|| "null".to_owned(), |value| doc_ref(schema, value))
}
pub(super) fn optional_typed_ref(schema: &str, value: Option<&DocRef>) -> String {
    value.map_or_else(
        || "null".to_owned(),
        |value| {
            format!(
                "{{\"schema\":{},\"digest\":{},\"bytes\":{}}}",
                quote_json(schema),
                quote_json(&value.digest),
                value.bytes
            )
        },
    )
}
pub(super) fn optional_capsule(schema: &str, doc: Option<&Doc>) -> String {
    doc.map_or_else(
        || "null".to_owned(),
        |value| {
            format!(
                "{{\"schema\":{},\"digest\":{},\"bytes\":{},\"document\":{}}}",
                quote_json(schema),
                quote_json(&value.digest),
                value.source.len(),
                quote_json(&value.source)
            )
        },
    )
}
pub(super) fn optional_unsigned(value: Option<&(String, usize, &'static str)>) -> String {
    value.map_or_else(
        || "null".to_owned(),
        |(digest_value, bytes, format)| {
            format!(
                "{{\"digest\":{},\"bytes\":{bytes},\"format\":{}}}",
                quote_json(digest_value),
                quote_json(format)
            )
        },
    )
}
pub(super) fn optional_signed(value: Option<&(String, usize)>) -> String {
    value.map_or_else(
        || "null".to_owned(),
        |(digest_value, bytes)| {
            format!(
                "{{\"digest\":{},\"bytes\":{bytes}}}",
                quote_json(digest_value)
            )
        },
    )
}
pub(super) fn render_journal(journal: &Journal) -> String {
    let mut count = CountSink::default();
    write_journal(&mut count, journal).expect("journal count cannot fail");
    let mut output = String::with_capacity(count.0);
    write_journal(&mut output, journal).expect("String writes cannot fail");
    output
}
pub(super) fn write_optional_journal_ref<W: fmt::Write>(
    output: &mut W,
    schema: &str,
    value: Option<&DocRef>,
) -> fmt::Result {
    match value {
        Some(value) => write_doc_reference(output, schema, &value.digest, value.bytes as usize),
        None => output.write_str("null"),
    }
}
pub(super) fn write_capsule<W: fmt::Write>(
    output: &mut W,
    schema: &str,
    value: Option<&Doc>,
) -> fmt::Result {
    match value {
        Some(value) => {
            output.write_str("{\"schema\":")?;
            write_json(output, schema)?;
            output.write_str(",\"digest\":")?;
            write_json(output, &value.digest)?;
            write!(output, ",\"bytes\":{},\"document\":", value.source.len())?;
            write_json(output, &value.source)?;
            output.write_char('}')
        }
        None => output.write_str("null"),
    }
}
pub(super) fn write_journal<W: fmt::Write>(output: &mut W, journal: &Journal) -> fmt::Result {
    output.write_str("{\"schema\":\"")?;
    output.write_str(JOURNAL_SCHEMA)?;
    output.write_str("\",\"idempotency_key\":")?;
    write_json(output, &journal.idempotency_key)?;
    write!(output, ",\"version\":{},\"policy\":", journal.version)?;
    write_doc_reference(
        output,
        POLICY_SCHEMA,
        &journal.policy.digest,
        journal.policy.source.len(),
    )?;
    output.write_str(",\"intent\":")?;
    write_doc_reference(
        output,
        INTENT_SCHEMA,
        &journal.intent.digest,
        journal.intent.source.len(),
    )?;
    output.write_str(",\"run_id\":")?;
    write_json(output, &journal.run_id)?;
    output.write_str(",\"state\":")?;
    write_json(output, journal.state.text())?;
    write!(
        output,
        ",\"reserved_amount_atomic\":{},\"reserved_fee_atomic\":{},\"plan\":",
        journal.reserved_amount, journal.reserved_fee
    )?;
    write_optional_journal_ref(output, PLAN_SCHEMA, journal.plan.as_ref())?;
    output.write_str(",\"simulation\":")?;
    write_optional_journal_ref(output, SIMULATION_SCHEMA, journal.simulation.as_ref())?;
    output.write_str(",\"approval\":")?;
    write_optional_journal_ref(output, APPROVAL_SCHEMA, journal.approval.as_ref())?;
    output.write_str(",\"unsigned_transaction\":")?;
    match journal.unsigned.as_ref() {
        Some((digest_value, bytes, format)) => {
            output.write_str("{\"digest\":")?;
            write_json(output, digest_value)?;
            write!(output, ",\"bytes\":{bytes},\"format\":")?;
            write_json(output, format)?;
            output.write_char('}')?;
        }
        None => output.write_str("null")?,
    }
    output.write_str(",\"signed_transaction\":")?;
    match journal.signed.as_ref() {
        Some((digest_value, bytes)) => {
            output.write_str("{\"digest\":")?;
            write_json(output, digest_value)?;
            write!(output, ",\"bytes\":{bytes}}}")?;
        }
        None => output.write_str("null")?,
    }
    output.write_str(",\"broadcast\":")?;
    write_capsule(output, BROADCAST_SCHEMA, journal.broadcast.as_ref())?;
    output.write_str(",\"reconciliation\":")?;
    write_capsule(
        output,
        RECONCILIATION_SCHEMA,
        journal.reconciliation.as_ref(),
    )?;
    writeln!(output, ",\"updated_at_ms\":{}}}", journal.updated_at)
}

#[derive(Clone)]
pub(super) struct BroadcastReceipt {
    pub(super) doc: Doc,
    pub(super) transaction_id: String,
    pub(super) disposition: &'static str,
    pub(super) observed: u64,
}
pub(super) fn parse_broadcast(
    source: &str,
    rail: EconomicRail,
    network: &str,
    signed_digest: &str,
    expected_transaction_id: Option<&str>,
) -> Result<BroadcastReceipt, Diagnostic> {
    parse_broadcast_mode(
        source,
        rail,
        network,
        signed_digest,
        expected_transaction_id,
        false,
    )
}
pub(super) fn parse_broadcast_limited(
    source: &str,
    rail: EconomicRail,
    network: &str,
    signed_digest: &str,
    expected_transaction_id: Option<&str>,
    limits: &Limits,
) -> Result<BroadcastReceipt, Diagnostic> {
    configured_document_limits(
        source,
        "broadcast receipt",
        limits.max_broadcast_receipt_bytes,
        limits,
    )?;
    reserve_parse_sidecar(source, limits)?;
    parse_broadcast(
        source,
        rail,
        network,
        signed_digest,
        expected_transaction_id,
    )
}
pub(super) fn parse_provisional_broadcast(
    source: &str,
    rail: EconomicRail,
    network: &str,
    signed_digest: &str,
    expected_transaction_id: &str,
) -> Result<BroadcastReceipt, Diagnostic> {
    parse_broadcast_mode(
        source,
        rail,
        network,
        signed_digest,
        Some(expected_transaction_id),
        true,
    )
}
pub(super) fn parse_broadcast_mode(
    source: &str,
    rail: EconomicRail,
    network: &str,
    signed_digest: &str,
    expected_transaction_id: Option<&str>,
    provisional: bool,
) -> Result<BroadcastReceipt, Diagnostic> {
    let (_, value) = canonical(
        source,
        "broadcast receipt",
        BROADCAST_SCHEMA,
        MAX_BROADCAST_BYTES,
    )?;
    let row = object(&value, "broadcast receipt", BROADCAST_SCHEMA)?;
    if !keys(
        row,
        &[
            "schema",
            "rail",
            "network",
            "signed_transaction_digest",
            "transaction_id",
            "disposition",
            "observed_at_ms",
        ],
    ) || text(row, "rail", "broadcast receipt", BROADCAST_SCHEMA)? != rail.text()
        || text(row, "network", "broadcast receipt", BROADCAST_SCHEMA)? != network
        || text(
            row,
            "signed_transaction_digest",
            "broadcast receipt",
            BROADCAST_SCHEMA,
        )? != signed_digest
    {
        return Err(g213());
    }
    let transaction_id =
        text(row, "transaction_id", "broadcast receipt", BROADCAST_SCHEMA)?.to_owned();
    if expected_transaction_id.is_some_and(|expected| expected != transaction_id) {
        return Err(g213());
    }
    let disposition = match text(row, "disposition", "broadcast receipt", BROADCAST_SCHEMA)? {
        "accepted" => "accepted",
        "pending" => "pending",
        "unknown" => "unknown",
        "rejected" => "rejected",
        _ => return Err(g210("broadcast receipt", BROADCAST_SCHEMA)),
    };
    let observed = number(row, "observed_at_ms", "broadcast receipt", BROADCAST_SCHEMA)?;
    if provisional {
        if disposition != "unknown" || observed != 0 {
            return Err(g213());
        }
    } else if observed == 0 {
        return Err(g213());
    }
    let canonical_source=format!("{{\"schema\":\"{BROADCAST_SCHEMA}\",\"rail\":{},\"network\":{},\"signed_transaction_digest\":{},\"transaction_id\":{},\"disposition\":{},\"observed_at_ms\":{observed}}}\n",quote_json(rail.text()),quote_json(network),quote_json(signed_digest),quote_json(&transaction_id),quote_json(disposition));
    if canonical_source != source {
        return Err(g210("broadcast receipt", BROADCAST_SCHEMA));
    }
    Ok(BroadcastReceipt {
        doc: Doc {
            source: source.to_owned(),
            digest: digest(BROADCAST_DOMAIN, source.as_bytes()),
        },
        transaction_id,
        disposition,
        observed,
    })
}

#[derive(Clone)]
pub(super) struct Reconciliation {
    pub(super) doc: Doc,
    pub(super) status: &'static str,
    pub(super) transaction_id: String,
    pub(super) observed: u64,
    pub(super) confirmations: Option<u64>,
}
pub(super) fn nullable_u64(value: &Value) -> Option<Option<u64>> {
    if value.is_null() {
        Some(None)
    } else {
        value.as_u64().map(Some)
    }
}
pub(super) fn nullable_text(value: &Value) -> Option<Option<String>> {
    if value.is_null() {
        Some(None)
    } else {
        value.as_str().map(|text| Some(text.to_owned()))
    }
}
pub(super) fn parse_reconciliation(
    source: &str,
    rail: EconomicRail,
    network: &str,
    transaction_id: &str,
) -> Result<Reconciliation, Diagnostic> {
    parse_reconciliation_with_identifier_limit(
        source,
        rail,
        network,
        transaction_id,
        MAX_IDENTIFIER_BYTES as u64,
    )
}

pub(super) fn parse_reconciliation_with_identifier_limit(
    source: &str,
    rail: EconomicRail,
    network: &str,
    transaction_id: &str,
    max_identifier_bytes: u64,
) -> Result<Reconciliation, Diagnostic> {
    let (_, value) = canonical(
        source,
        "reconciliation",
        RECONCILIATION_SCHEMA,
        MAX_RECONCILIATION_BYTES,
    )?;
    let row = object(&value, "reconciliation", RECONCILIATION_SCHEMA)?;
    if !keys(
        row,
        &[
            "schema",
            "rail",
            "network",
            "transaction_id",
            "status",
            "observed_at_ms",
            "observed_height",
            "confirmations",
            "canonical_block_id",
        ],
    ) || text(row, "rail", "reconciliation", RECONCILIATION_SCHEMA)? != rail.text()
        || text(row, "network", "reconciliation", RECONCILIATION_SCHEMA)? != network
        || text(
            row,
            "transaction_id",
            "reconciliation",
            RECONCILIATION_SCHEMA,
        )? != transaction_id
    {
        return Err(g215());
    }
    let status = match text(row, "status", "reconciliation", RECONCILIATION_SCHEMA)? {
        "pending" => "pending",
        "confirmed" => "confirmed",
        "reorged" => "reorged",
        "dropped" => "dropped",
        _ => return Err(g210("reconciliation", RECONCILIATION_SCHEMA)),
    };
    let observed = number(
        row,
        "observed_at_ms",
        "reconciliation",
        RECONCILIATION_SCHEMA,
    )?;
    let height = nullable_u64(&row["observed_height"])
        .ok_or_else(|| g210("reconciliation", RECONCILIATION_SCHEMA))?;
    let confirmations = nullable_u64(&row["confirmations"])
        .ok_or_else(|| g210("reconciliation", RECONCILIATION_SCHEMA))?;
    let block = nullable_text(&row["canonical_block_id"])
        .ok_or_else(|| g210("reconciliation", RECONCILIATION_SCHEMA))?;
    if transaction_id.len() > max_identifier_bytes as usize
        || block
            .as_deref()
            .is_some_and(|value| value.len() > max_identifier_bytes as usize)
    {
        return Err(g216("identifier_bytes", max_identifier_bytes));
    }
    if status == "confirmed" && (height.is_none() || confirmations.is_none() || block.is_none()) {
        return Err(g215());
    }
    let canonical_source=format!("{{\"schema\":\"{RECONCILIATION_SCHEMA}\",\"rail\":{},\"network\":{},\"transaction_id\":{},\"status\":{},\"observed_at_ms\":{observed},\"observed_height\":{},\"confirmations\":{},\"canonical_block_id\":{}}}\n",quote_json(rail.text()),quote_json(network),quote_json(transaction_id),quote_json(status),height.map_or_else(||"null".to_owned(),|v|v.to_string()),confirmations.map_or_else(||"null".to_owned(),|v|v.to_string()),block.as_deref().map_or_else(||"null".to_owned(),quote_json));
    if canonical_source != source {
        return Err(g210("reconciliation", RECONCILIATION_SCHEMA));
    }
    Ok(Reconciliation {
        doc: Doc {
            source: source.to_owned(),
            digest: digest(RECONCILIATION_DOMAIN, source.as_bytes()),
        },
        status,
        transaction_id: transaction_id.to_owned(),
        observed,
        confirmations,
    })
}
pub(super) fn parse_reconciliation_limited(
    source: &str,
    rail: EconomicRail,
    network: &str,
    transaction_id: &str,
    limits: &Limits,
) -> Result<Reconciliation, Diagnostic> {
    configured_document_limits(
        source,
        "reconciliation",
        limits.max_reconciliation_bytes,
        limits,
    )?;
    reserve_parse_sidecar(source, limits)?;
    parse_reconciliation_with_identifier_limit(
        source,
        rail,
        network,
        transaction_id,
        limits.max_identifier_bytes,
    )
}

pub(super) fn capsule_doc(
    value: &Value,
    schema: &str,
    domain: &[u8],
    maximum: usize,
    max_depth: u64,
    document: &str,
) -> Result<Option<Doc>, Diagnostic> {
    if value.is_null() {
        return Ok(None);
    }
    let row = object(value, "journal", JOURNAL_SCHEMA)?;
    if !keys(row, &["schema", "digest", "bytes", "document"]) {
        return Err(g215());
    }
    let source = text(row, "document", "journal", JOURNAL_SCHEMA)?.to_owned();
    let sidecar = source
        .len()
        .checked_mul(
            usize::try_from(max_depth)
                .map_err(|_| g217())?
                .checked_add(2)
                .ok_or_else(g217)?,
        )
        .ok_or_else(g217)?;
    if active_remaining().is_some_and(|remaining| sidecar > remaining) || !reserve_active(sidecar) {
        return Err(g216(
            "builder_bytes",
            active_limit().unwrap_or(MAX_BUILDER_BYTES) as u64,
        ));
    }
    if source.len() > maximum
        || row.get("schema").and_then(Value::as_str) != Some(schema)
        || row.get("bytes").and_then(Value::as_u64) != u64::try_from(source.len()).ok()
    {
        return Err(g215());
    }
    canonical_policy_limited(&source, document, schema, maximum as u64, max_depth)?;
    let digest_value = digest(domain, source.as_bytes());
    if row.get("digest").and_then(Value::as_str) != Some(digest_value.as_str()) {
        return Err(g215());
    }
    Ok(Some(Doc {
        source,
        digest: digest_value,
    }))
}
pub(super) fn generic_ref_doc(
    value: &Value,
    schema: &str,
    maximum: u64,
) -> Result<Option<DocRef>, Diagnostic> {
    if value.is_null() {
        return Ok(None);
    }
    let row = object(value, "journal", JOURNAL_SCHEMA)?;
    if !keys(row, &["schema", "digest", "bytes"])
        || row.get("schema").and_then(Value::as_str) != Some(schema)
    {
        return Err(g215());
    }
    let digest_value = text(row, "digest", "journal", JOURNAL_SCHEMA)?.to_owned();
    let bytes = number(row, "bytes", "journal", JOURNAL_SCHEMA)?;
    if bytes > maximum
        || !digest_value.starts_with("sha256:")
        || digest_value.len() != 71
        || !digest_value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(g215());
    }
    Ok(Some(DocRef {
        bytes,
        digest: digest_value,
    }))
}
pub(super) fn unsigned_journal_ref(
    value: &Value,
    maximum: u64,
) -> Result<Option<(String, usize, &'static str)>, Diagnostic> {
    if value.is_null() {
        return Ok(None);
    }
    let row = object(value, "journal", JOURNAL_SCHEMA)?;
    if !keys(row, &["digest", "bytes", "format"]) {
        return Err(g215());
    }
    let digest_value = text(row, "digest", "journal", JOURNAL_SCHEMA)?.to_owned();
    let bytes = number(row, "bytes", "journal", JOURNAL_SCHEMA)?;
    let format = match text(row, "format", "journal", JOURNAL_SCHEMA)? {
        "eip1559-unsigned-v1" => "eip1559-unsigned-v1",
        "solana-message-v0" => "solana-message-v0",
        "psbt-v2" => "psbt-v2",
        _ => return Err(g215()),
    };
    if bytes > maximum || digest_value.len() != 71 || !digest_value.starts_with("sha256:") {
        return Err(g215());
    }
    Ok(Some((digest_value, bytes as usize, format)))
}
pub(super) fn signed_journal_ref(
    value: &Value,
    maximum: u64,
) -> Result<Option<(String, usize)>, Diagnostic> {
    if value.is_null() {
        return Ok(None);
    }
    let row = object(value, "journal", JOURNAL_SCHEMA)?;
    if !keys(row, &["digest", "bytes"]) {
        return Err(g215());
    }
    let digest_value = text(row, "digest", "journal", JOURNAL_SCHEMA)?.to_owned();
    let bytes = number(row, "bytes", "journal", JOURNAL_SCHEMA)?;
    if bytes > maximum || digest_value.len() != 71 || !digest_value.starts_with("sha256:") {
        return Err(g215());
    }
    Ok(Some((digest_value, bytes as usize)))
}
pub(super) enum JournalParseFailure {
    BindingMismatch,
    Diagnostic(Diagnostic),
}

impl From<Diagnostic> for JournalParseFailure {
    fn from(diagnostic: Diagnostic) -> Self {
        Self::Diagnostic(diagnostic)
    }
}

pub(super) fn parse_journal(
    source: &str,
    policy: &Policy,
    intent: &Intent,
    run_id: &str,
) -> Result<Journal, Diagnostic> {
    parse_journal_classified(source, policy, intent, run_id).map_err(|failure| match failure {
        JournalParseFailure::BindingMismatch => g215(),
        JournalParseFailure::Diagnostic(diagnostic) => diagnostic,
    })
}

pub(super) fn parse_journal_classified(
    source: &str,
    policy: &Policy,
    intent: &Intent,
    run_id: &str,
) -> Result<Journal, JournalParseFailure> {
    configured_document_limits(
        source,
        "journal",
        policy.limits.max_journal_bytes,
        &policy.limits,
    )?;
    let sidecar = source.len().checked_mul(2).ok_or_else(g217)?;
    if active_remaining().is_some_and(|remaining| sidecar > remaining) || !reserve_active(sidecar) {
        return Err(g216("builder_bytes", policy.limits.max_builder_bytes).into());
    }
    let (_, value) = canonical(
        source,
        "journal",
        JOURNAL_SCHEMA,
        policy.limits.max_journal_bytes as usize,
    )?;
    let row = object(&value, "journal", JOURNAL_SCHEMA)?;
    if !keys(
        row,
        &[
            "schema",
            "idempotency_key",
            "version",
            "policy",
            "intent",
            "run_id",
            "state",
            "reserved_amount_atomic",
            "reserved_fee_atomic",
            "plan",
            "simulation",
            "approval",
            "unsigned_transaction",
            "signed_transaction",
            "broadcast",
            "reconciliation",
            "updated_at_ms",
        ],
    ) {
        return Err(g215().into());
    }
    let policy_doc = Doc {
        source: policy.source.clone(),
        digest: policy.digest.clone(),
    };
    let intent_doc = Doc {
        source: intent.source.clone(),
        digest: intent.digest.clone(),
    };
    if text(row, "idempotency_key", "journal", JOURNAL_SCHEMA)? != intent.idempotency_key
        || text(row, "run_id", "journal", JOURNAL_SCHEMA)? != run_id
        || !ref_matches(&row["policy"], POLICY_SCHEMA, &policy_doc)
        || !ref_matches(&row["intent"], INTENT_SCHEMA, &intent_doc)
    {
        return Err(JournalParseFailure::BindingMismatch);
    }
    let broadcast = capsule_doc(
        &row["broadcast"],
        BROADCAST_SCHEMA,
        BROADCAST_DOMAIN,
        policy.limits.max_broadcast_receipt_bytes as usize,
        policy.limits.max_json_depth,
        "broadcast receipt",
    )?;
    let reconciliation = capsule_doc(
        &row["reconciliation"],
        RECONCILIATION_SCHEMA,
        RECONCILIATION_DOMAIN,
        policy.limits.max_reconciliation_bytes as usize,
        policy.limits.max_json_depth,
        "reconciliation",
    )?;
    let journal = Journal {
        idempotency_key: intent.idempotency_key.clone(),
        version: number(row, "version", "journal", JOURNAL_SCHEMA)?,
        policy: policy_doc,
        intent: intent_doc,
        run_id: run_id.to_owned(),
        state: JournalState::parse(text(row, "state", "journal", JOURNAL_SCHEMA)?)
            .ok_or_else(g215)?,
        reserved_amount: number(row, "reserved_amount_atomic", "journal", JOURNAL_SCHEMA)?,
        reserved_fee: number(row, "reserved_fee_atomic", "journal", JOURNAL_SCHEMA)?,
        plan: generic_ref_doc(&row["plan"], PLAN_SCHEMA, policy.limits.max_plan_bytes)?,
        simulation: generic_ref_doc(
            &row["simulation"],
            SIMULATION_SCHEMA,
            policy.limits.max_simulation_bytes,
        )?,
        approval: generic_ref_doc(
            &row["approval"],
            APPROVAL_SCHEMA,
            policy.limits.max_approval_bytes,
        )?,
        unsigned: unsigned_journal_ref(
            &row["unsigned_transaction"],
            policy.limits.max_unsigned_transaction_bytes,
        )?,
        signed: signed_journal_ref(
            &row["signed_transaction"],
            policy.limits.max_signed_transaction_bytes,
        )?,
        broadcast,
        reconciliation,
        updated_at: number(row, "updated_at_ms", "journal", JOURNAL_SCHEMA)?,
    };
    if journal.reserved_amount != intent.amount() || journal.reserved_fee != intent.max_fee() {
        return Err(g215().into());
    }
    let prepared =
        journal.plan.is_some() && journal.simulation.is_some() && journal.unsigned.is_some();
    let approved = prepared && journal.approval.is_some();
    let signed = approved && journal.signed.is_some();
    let broadcasted = signed && journal.broadcast.is_some();
    let reserved_prefix = journal.plan.is_none()
        && journal.simulation.is_none()
        && journal.approval.is_none()
        && journal.unsigned.is_none()
        && journal.signed.is_none()
        && journal.broadcast.is_none()
        && journal.reconciliation.is_none();
    let prepared_prefix = prepared
        && journal.approval.is_none()
        && journal.signed.is_none()
        && journal.broadcast.is_none()
        && journal.reconciliation.is_none();
    let approved_prefix = approved
        && journal.signed.is_none()
        && journal.broadcast.is_none()
        && journal.reconciliation.is_none();
    let valid_shape = match journal.state {
        JournalState::Reserved => reserved_prefix,
        JournalState::Prepared => prepared_prefix,
        JournalState::Approved => approved_prefix,
        JournalState::Signed => {
            signed && journal.broadcast.is_none() && journal.reconciliation.is_none()
        }
        JournalState::BroadcastUnknown | JournalState::Broadcasted => {
            broadcasted && journal.reconciliation.is_none()
        }
        JournalState::Pending => broadcasted,
        JournalState::Confirmed | JournalState::Reorged | JournalState::Dropped => {
            broadcasted && journal.reconciliation.is_some()
        }
        JournalState::Rejected => {
            (reserved_prefix || prepared_prefix || approved_prefix)
                || (broadcasted && journal.reconciliation.is_none())
        }
        JournalState::Cancelled | JournalState::Failed => {
            reserved_prefix || prepared_prefix || approved_prefix
        }
    };
    let version_shape = match journal.state {
        JournalState::Reserved => journal.version == 1,
        JournalState::Prepared => journal.version == 2,
        JournalState::Approved => matches!(journal.version, 3 | 4),
        JournalState::Signed => journal.version == 5,
        JournalState::BroadcastUnknown => journal.version >= 6,
        JournalState::Broadcasted
        | JournalState::Pending
        | JournalState::Confirmed
        | JournalState::Reorged
        | JournalState::Dropped
        | JournalState::Rejected => journal.version >= 7,
        JournalState::Cancelled | JournalState::Failed => journal.version >= 2,
    };
    if !valid_shape || !version_shape {
        return Err(g215().into());
    }
    if let Some(broadcast_doc) = journal.broadcast.as_ref() {
        let signed_digest = journal.signed.as_ref().ok_or_else(g215)?.0.as_str();
        let (network, _) = intent.network_asset();
        let provisional = broadcast_is_provisional(broadcast_doc);
        let base = if provisional { 6 } else { 7 };
        let offset = journal.version.checked_sub(base).ok_or_else(g215)?;
        let attempts = offset.checked_add(1).ok_or_else(g215)? / 2;
        let odd = offset % 2 == 1;
        if attempts > policy.limits.max_reconciliations
            || (journal.reconciliation.is_some() && odd)
            || (matches!(
                journal.state,
                JournalState::Confirmed | JournalState::Reorged | JournalState::Dropped
            ) && (journal.reconciliation.is_none() || attempts == 0 || odd))
        {
            return Err(g215().into());
        }
        let broadcast = if provisional {
            let value: Value =
                serde_json::from_str(broadcast_doc.source.trim_end()).map_err(|_| g215())?;
            let transaction_id = value["transaction_id"].as_str().ok_or_else(g215)?;
            parse_provisional_broadcast(
                &broadcast_doc.source,
                intent.settlement_rail(),
                network,
                signed_digest,
                transaction_id,
            )?
        } else {
            parse_broadcast(
                &broadcast_doc.source,
                intent.settlement_rail(),
                network,
                signed_digest,
                None,
            )?
        };
        let allowed_disposition = match journal.state {
            JournalState::BroadcastUnknown => broadcast.disposition == "unknown",
            JournalState::Broadcasted => broadcast.disposition == "accepted",
            JournalState::Pending if journal.reconciliation.is_none() => {
                broadcast.disposition == "pending"
            }
            JournalState::Pending => matches!(broadcast.disposition, "accepted" | "pending"),
            JournalState::Confirmed | JournalState::Reorged | JournalState::Dropped => {
                matches!(broadcast.disposition, "accepted" | "pending")
            }
            JournalState::Rejected => broadcast.disposition == "rejected",
            _ => false,
        };
        if !allowed_disposition {
            return Err(g215().into());
        }
        if let Some(reconciliation_doc) = journal.reconciliation.as_ref() {
            let reconciliation = parse_reconciliation(
                &reconciliation_doc.source,
                intent.settlement_rail(),
                network,
                &broadcast.transaction_id,
            )?;
            validate_confirmation(intent, &reconciliation)?;
            let status_matches = match journal.state {
                JournalState::Pending => reconciliation.status == "pending",
                JournalState::Confirmed => reconciliation.status == "confirmed",
                JournalState::Reorged => reconciliation.status == "reorged",
                JournalState::Dropped => reconciliation.status == "dropped",
                _ => false,
            };
            if reconciliation.observed < broadcast.observed
                || reconciliation.observed != journal.updated_at
                || !status_matches
            {
                return Err(g215().into());
            }
        } else if journal.updated_at != broadcast.observed
            && !(journal.state == JournalState::BroadcastUnknown
                && provisional
                && broadcast.disposition == "unknown"
                && broadcast.observed == 0)
        {
            return Err(g215().into());
        }
    }
    Ok(journal)
}

pub(super) fn broadcast_is_provisional(document: &Doc) -> bool {
    serde_json::from_str::<Value>(document.source.trim_end())
        .ok()
        .is_some_and(|value| {
            value["disposition"].as_str() == Some("unknown")
                && value["observed_at_ms"].as_u64() == Some(0)
        })
}

pub(super) fn reconciliation_topology(journal: &Journal) -> Result<(u64, bool), Diagnostic> {
    let broadcast = journal.broadcast.as_ref().ok_or_else(g215)?;
    let base = if broadcast_is_provisional(broadcast) {
        6
    } else {
        7
    };
    let offset = journal.version.checked_sub(base).ok_or_else(g215)?;
    let attempts = offset.checked_add(1).ok_or_else(g215)? / 2;
    Ok((attempts, offset % 2 == 1))
}
