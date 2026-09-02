//! Economic policy document rendering and parsing, plus the recipient,
//! origin, and resource admission predicates policy enforcement relies on.

use super::address::{decode_base58_32, decode_regtest_p2wpkh};
use super::validate::{
    canonical, depth, digest, g210, g211, g216, identifier, keys, nonclaims_json, number, object,
    policy_limit, rail, sorted_unique, string_array, string_list, text,
};
use super::{
    EconomicRail, Limits, NetworkPolicy, OriginPolicy, Policy, MAX_APPROVAL_BYTES,
    MAX_APPROVAL_REQUEST_BYTES, MAX_BROADCAST_BYTES, MAX_BUILDER_BYTES, MAX_EVIDENCE_BYTES,
    MAX_IDENTIFIER_BYTES, MAX_INTENT_BYTES, MAX_INVOICE_BYTES, MAX_JOURNAL_BYTES, MAX_JSON_DEPTH,
    MAX_MEMO_BYTES, MAX_NETWORK_POLICIES, MAX_PLAN_BYTES, MAX_POLICY_BYTES, MAX_RECIPIENTS,
    MAX_RECONCILIATION_BYTES, MAX_SIGNED_BYTES, MAX_SIMULATION_BYTES, MAX_SNAPSHOT_BYTES,
    MAX_TRACE_BYTES, MAX_TRACE_EVENTS, MAX_UNSIGNED_BYTES, MAX_UTXOS, MAX_X402_ORIGINS, NONCLAIMS,
    POLICY_DOMAIN, POLICY_SCHEMA,
};
use crate::diagnostic::{quote_json, Diagnostic};
use serde_json::Value;
use std::fmt;
use std::net::IpAddr;

pub(super) fn limits_json(limits: &Limits) -> String {
    let mut output = String::new();
    write_limits(&mut output, limits).expect("String writes cannot fail");
    output
}
pub(super) fn write_limits<W: fmt::Write>(output: &mut W, limits: &Limits) -> fmt::Result {
    write!(output,"{{\"max_policy_bytes\":{},\"max_intent_bytes\":{},\"max_invoice_bytes\":{},\"max_snapshot_bytes\":{},\"max_plan_bytes\":{},\"max_simulation_bytes\":{},\"max_approval_request_bytes\":{},\"max_approval_bytes\":{},\"max_journal_bytes\":{},\"max_unsigned_transaction_bytes\":{},\"max_signed_transaction_bytes\":{},\"max_broadcast_receipt_bytes\":{},\"max_reconciliation_bytes\":{},\"max_trace_events\":{},\"max_trace_bytes\":{},\"max_evidence_bytes\":{},\"max_builder_bytes\":{},\"max_json_depth\":{},\"max_identifier_bytes\":{},\"max_memo_bytes\":{},\"max_recipients\":{},\"max_network_policies\":{},\"max_x402_origins\":{},\"max_utxos\":{},\"max_reconciliations\":{},\"max_elapsed_ms\":{},\"max_amount_atomic\":{},\"max_fee_atomic\":{},\"max_compute_units\":{},\"max_confirmation_target\":{},\"max_concurrency\":{},\"max_unexpected_authority_calls\":{}}}",limits.max_policy_bytes,limits.max_intent_bytes,limits.max_invoice_bytes,limits.max_snapshot_bytes,limits.max_plan_bytes,limits.max_simulation_bytes,limits.max_approval_request_bytes,limits.max_approval_bytes,limits.max_journal_bytes,limits.max_unsigned_transaction_bytes,limits.max_signed_transaction_bytes,limits.max_broadcast_receipt_bytes,limits.max_reconciliation_bytes,limits.max_trace_events,limits.max_trace_bytes,limits.max_evidence_bytes,limits.max_builder_bytes,limits.max_json_depth,limits.max_identifier_bytes,limits.max_memo_bytes,limits.max_recipients,limits.max_network_policies,limits.max_x402_origins,limits.max_utxos,limits.max_reconciliations,limits.max_elapsed_ms,limits.max_amount_atomic,limits.max_fee_atomic,limits.max_compute_units,limits.max_confirmation_target,limits.max_concurrency,limits.max_unexpected_authority_calls)
}

pub(super) fn render_policy(policy: &Policy) -> String {
    let mut networks = String::from("[");
    for (index, row) in policy.networks.iter().enumerate() {
        if index > 0 {
            networks.push(',');
        }
        networks.push_str(&format!("{{\"rail\":{},\"network\":{},\"asset\":{},\"recipients\":{},\"max_amount_atomic\":{},\"max_fee_atomic\":{},\"max_rolling_24h_atomic\":{}}}",quote_json(row.rail.text()),quote_json(&row.network),quote_json(&row.asset),string_list(&row.recipients),row.max_amount,row.max_fee,row.max_rolling));
    }
    networks.push(']');
    let mut origins = String::from("[");
    for (index, row) in policy.origins.iter().enumerate() {
        if index > 0 {
            origins.push(',');
        }
        origins.push_str(&format!("{{\"origin\":{},\"methods\":{},\"resources\":{},\"settlement_rails\":{},\"max_amount_atomic\":{}}}",quote_json(&row.origin),string_list(&row.methods),string_list(&row.resources),string_list(&row.rails.iter().map(|r|r.text().to_owned()).collect::<Vec<_>>()),row.max_amount));
    }
    origins.push(']');
    format!("{{\"schema\":\"{POLICY_SCHEMA}\",\"economic_agent_id\":{},\"wallet_id\":{},\"network_policies\":{networks},\"x402_origins\":{origins},\"limits\":{},\"nonclaims\":{}}}\n",quote_json(&policy.economic_agent_id),quote_json(&policy.wallet_id),limits_json(&policy.limits),nonclaims_json())
}

pub(super) fn parse_policy(source: &str) -> Result<Policy, Diagnostic> {
    let (_, value) = canonical(source, "policy", POLICY_SCHEMA, MAX_POLICY_BYTES)?;
    let top = object(&value, "policy", POLICY_SCHEMA)?;
    if !keys(
        top,
        &[
            "schema",
            "economic_agent_id",
            "wallet_id",
            "network_policies",
            "x402_origins",
            "limits",
            "nonclaims",
        ],
    ) {
        return Err(g210("policy", POLICY_SCHEMA));
    }
    let economic_agent_id = text(top, "economic_agent_id", "policy", POLICY_SCHEMA)?.to_owned();
    let wallet_id = text(top, "wallet_id", "policy", POLICY_SCHEMA)?.to_owned();
    if !identifier(&economic_agent_id) || !identifier(&wallet_id) {
        return Err(g211("identifiers"));
    }
    let rows = top["network_policies"]
        .as_array()
        .ok_or_else(|| g210("policy", POLICY_SCHEMA))?;
    if rows.is_empty() || rows.len() > MAX_NETWORK_POLICIES {
        return Err(g211("network_policies"));
    }
    let mut networks = Vec::new();
    for value in rows {
        let row = object(value, "policy", POLICY_SCHEMA)?;
        if !keys(
            row,
            &[
                "rail",
                "network",
                "asset",
                "recipients",
                "max_amount_atomic",
                "max_fee_atomic",
                "max_rolling_24h_atomic",
            ],
        ) {
            return Err(g210("policy", POLICY_SCHEMA));
        }
        let rail = rail(text(row, "rail", "policy", POLICY_SCHEMA)?)
            .ok_or_else(|| g211("network_policies.rail"))?;
        let network = text(row, "network", "policy", POLICY_SCHEMA)?.to_owned();
        let asset = text(row, "asset", "policy", POLICY_SCHEMA)?.to_owned();
        if (rail, network.as_str(), asset.as_str()) != (EconomicRail::Evm, "sepolia", "native:eth")
            && (rail, network.as_str(), asset.as_str())
                != (EconomicRail::Solana, "devnet", "native:sol")
            && (rail, network.as_str(), asset.as_str())
                != (EconomicRail::Bitcoin, "regtest", "native:btc")
        {
            return Err(g211("network_policies.network"));
        }
        let recipients =
            string_array(&row["recipients"]).ok_or_else(|| g210("policy", POLICY_SCHEMA))?;
        if recipients.is_empty()
            || recipients.len() > MAX_RECIPIENTS
            || !sorted_unique(&recipients)
            || recipients.iter().any(|v| !valid_recipient(rail, v))
        {
            return Err(g211("network_policies.recipients"));
        }
        let max_amount = number(row, "max_amount_atomic", "policy", POLICY_SCHEMA)?;
        let max_fee = number(row, "max_fee_atomic", "policy", POLICY_SCHEMA)?;
        let max_rolling = number(row, "max_rolling_24h_atomic", "policy", POLICY_SCHEMA)?;
        if max_amount == 0
            || max_amount > 1_000_000_000_000_000_000
            || max_fee > 1_000_000_000_000_000
            || max_rolling < max_amount
        {
            return Err(g211("network_policies.limits"));
        }
        networks.push(NetworkPolicy {
            rail,
            network,
            asset,
            recipients,
            max_amount,
            max_fee,
            max_rolling,
        });
    }
    if !networks.windows(2).all(|w| {
        (w[0].rail.text(), w[0].network.as_str(), w[0].asset.as_str())
            < (w[1].rail.text(), w[1].network.as_str(), w[1].asset.as_str())
    }) {
        return Err(g211("network_policies.order"));
    }
    let origin_rows = top["x402_origins"]
        .as_array()
        .ok_or_else(|| g210("policy", POLICY_SCHEMA))?;
    if origin_rows.len() > MAX_X402_ORIGINS {
        return Err(g211("x402_origins"));
    }
    let mut origins = Vec::new();
    for value in origin_rows {
        let row = object(value, "policy", POLICY_SCHEMA)?;
        if !keys(
            row,
            &[
                "origin",
                "methods",
                "resources",
                "settlement_rails",
                "max_amount_atomic",
            ],
        ) {
            return Err(g210("policy", POLICY_SCHEMA));
        }
        let origin = text(row, "origin", "policy", POLICY_SCHEMA)?.to_owned();
        if !valid_origin(&origin) {
            return Err(g211("x402_origins.origin"));
        }
        let methods = string_array(&row["methods"]).ok_or_else(|| g210("policy", POLICY_SCHEMA))?;
        let resources =
            string_array(&row["resources"]).ok_or_else(|| g210("policy", POLICY_SCHEMA))?;
        let rail_text =
            string_array(&row["settlement_rails"]).ok_or_else(|| g210("policy", POLICY_SCHEMA))?;
        let rails: Vec<_> = rail_text
            .iter()
            .map(|v| rail(v).ok_or_else(|| g211("x402_origins.settlement_rails")))
            .collect::<Result<_, _>>()?;
        let max_amount = number(row, "max_amount_atomic", "policy", POLICY_SCHEMA)?;
        if methods.is_empty()
            || !sorted_unique(&methods)
            || methods.iter().any(|v| v != "GET" && v != "POST")
            || resources.is_empty()
            || !sorted_unique(&resources)
            || resources.iter().any(|v| !valid_resource(v))
            || rails.is_empty()
            || !rails.windows(2).all(|w| w[0].text() < w[1].text())
            || max_amount == 0
            || max_amount > 1_000_000_000_000_000_000
        {
            return Err(g211("x402_origins"));
        }
        origins.push(OriginPolicy {
            origin,
            methods,
            resources,
            rails,
            max_amount,
        });
    }
    if !origins.windows(2).all(|w| w[0].origin < w[1].origin) {
        return Err(g211("x402_origins.order"));
    }
    let limits = top["limits"]
        .as_object()
        .ok_or_else(|| g210("policy", POLICY_SCHEMA))?;
    let expected_limit_keys = [
        "max_policy_bytes",
        "max_intent_bytes",
        "max_invoice_bytes",
        "max_snapshot_bytes",
        "max_plan_bytes",
        "max_simulation_bytes",
        "max_approval_request_bytes",
        "max_approval_bytes",
        "max_journal_bytes",
        "max_unsigned_transaction_bytes",
        "max_signed_transaction_bytes",
        "max_broadcast_receipt_bytes",
        "max_reconciliation_bytes",
        "max_trace_events",
        "max_trace_bytes",
        "max_evidence_bytes",
        "max_builder_bytes",
        "max_json_depth",
        "max_identifier_bytes",
        "max_memo_bytes",
        "max_recipients",
        "max_network_policies",
        "max_x402_origins",
        "max_utxos",
        "max_reconciliations",
        "max_elapsed_ms",
        "max_amount_atomic",
        "max_fee_atomic",
        "max_compute_units",
        "max_confirmation_target",
        "max_concurrency",
        "max_unexpected_authority_calls",
    ];
    if !keys(limits, &expected_limit_keys) {
        return Err(g211("limits"));
    }
    let limits = Limits {
        max_policy_bytes: policy_limit(limits, "max_policy_bytes", MAX_POLICY_BYTES as u64, true)?,
        max_intent_bytes: policy_limit(limits, "max_intent_bytes", MAX_INTENT_BYTES as u64, true)?,
        max_invoice_bytes: policy_limit(
            limits,
            "max_invoice_bytes",
            MAX_INVOICE_BYTES as u64,
            true,
        )?,
        max_snapshot_bytes: policy_limit(
            limits,
            "max_snapshot_bytes",
            MAX_SNAPSHOT_BYTES as u64,
            true,
        )?,
        max_plan_bytes: policy_limit(limits, "max_plan_bytes", MAX_PLAN_BYTES as u64, true)?,
        max_simulation_bytes: policy_limit(
            limits,
            "max_simulation_bytes",
            MAX_SIMULATION_BYTES as u64,
            true,
        )?,
        max_approval_request_bytes: policy_limit(
            limits,
            "max_approval_request_bytes",
            MAX_APPROVAL_REQUEST_BYTES as u64,
            true,
        )?,
        max_approval_bytes: policy_limit(
            limits,
            "max_approval_bytes",
            MAX_APPROVAL_BYTES as u64,
            true,
        )?,
        max_journal_bytes: policy_limit(
            limits,
            "max_journal_bytes",
            MAX_JOURNAL_BYTES as u64,
            true,
        )?,
        max_unsigned_transaction_bytes: policy_limit(
            limits,
            "max_unsigned_transaction_bytes",
            MAX_UNSIGNED_BYTES as u64,
            true,
        )?,
        max_signed_transaction_bytes: policy_limit(
            limits,
            "max_signed_transaction_bytes",
            MAX_SIGNED_BYTES as u64,
            true,
        )?,
        max_broadcast_receipt_bytes: policy_limit(
            limits,
            "max_broadcast_receipt_bytes",
            MAX_BROADCAST_BYTES as u64,
            true,
        )?,
        max_reconciliation_bytes: policy_limit(
            limits,
            "max_reconciliation_bytes",
            MAX_RECONCILIATION_BYTES as u64,
            true,
        )?,
        max_trace_events: policy_limit(limits, "max_trace_events", MAX_TRACE_EVENTS as u64, true)?,
        max_trace_bytes: policy_limit(limits, "max_trace_bytes", MAX_TRACE_BYTES as u64, true)?,
        max_evidence_bytes: policy_limit(
            limits,
            "max_evidence_bytes",
            MAX_EVIDENCE_BYTES as u64,
            true,
        )?,
        max_builder_bytes: policy_limit(
            limits,
            "max_builder_bytes",
            MAX_BUILDER_BYTES as u64,
            true,
        )?,
        max_json_depth: policy_limit(limits, "max_json_depth", MAX_JSON_DEPTH as u64, true)?,
        max_identifier_bytes: policy_limit(
            limits,
            "max_identifier_bytes",
            MAX_IDENTIFIER_BYTES as u64,
            true,
        )?,
        max_memo_bytes: policy_limit(limits, "max_memo_bytes", MAX_MEMO_BYTES as u64, true)?,
        max_recipients: policy_limit(limits, "max_recipients", MAX_RECIPIENTS as u64, true)?,
        max_network_policies: policy_limit(
            limits,
            "max_network_policies",
            MAX_NETWORK_POLICIES as u64,
            true,
        )?,
        max_x402_origins: policy_limit(limits, "max_x402_origins", MAX_X402_ORIGINS as u64, false)?,
        max_utxos: policy_limit(limits, "max_utxos", MAX_UTXOS as u64, true)?,
        max_reconciliations: policy_limit(limits, "max_reconciliations", 64, true)?,
        max_elapsed_ms: policy_limit(limits, "max_elapsed_ms", 600_000, true)?,
        max_amount_atomic: policy_limit(
            limits,
            "max_amount_atomic",
            1_000_000_000_000_000_000,
            true,
        )?,
        max_fee_atomic: policy_limit(limits, "max_fee_atomic", 1_000_000_000_000_000, true)?,
        max_compute_units: policy_limit(limits, "max_compute_units", 200_000, true)?,
        max_confirmation_target: policy_limit(limits, "max_confirmation_target", 144, true)?,
        max_concurrency: policy_limit(limits, "max_concurrency", 1, true)?,
        max_unexpected_authority_calls: policy_limit(
            limits,
            "max_unexpected_authority_calls",
            0,
            false,
        )?,
    };
    if limits.max_concurrency != 1 || limits.max_unexpected_authority_calls != 0 {
        return Err(g211("limits"));
    }
    if economic_agent_id.len() > limits.max_identifier_bytes as usize
        || wallet_id.len() > limits.max_identifier_bytes as usize
    {
        return Err(g211("limits.max_identifier_bytes"));
    }
    if networks.len() > limits.max_network_policies as usize
        || origins.len() > limits.max_x402_origins as usize
        || networks.iter().any(|network| {
            network.recipients.len() > limits.max_recipients as usize
                || network.max_amount > limits.max_amount_atomic
                || network.max_fee > limits.max_fee_atomic
                || network.max_rolling > limits.max_amount_atomic
        })
        || origins
            .iter()
            .any(|origin| origin.max_amount > limits.max_amount_atomic)
    {
        return Err(g211("limits"));
    }
    let claims = string_array(&top["nonclaims"]).ok_or_else(|| g210("policy", POLICY_SCHEMA))?;
    if claims
        != NONCLAIMS
            .iter()
            .map(|v| (*v).to_owned())
            .collect::<Vec<_>>()
    {
        return Err(g211("nonclaims"));
    }
    let mut policy = Policy {
        economic_agent_id,
        wallet_id,
        networks,
        origins,
        limits,
        source: source.to_owned(),
        digest: digest(POLICY_DOMAIN, source.as_bytes()),
    };
    if render_policy(&policy) != source {
        return Err(g210("policy", POLICY_SCHEMA));
    }
    if source.len() > policy.limits.max_policy_bytes as usize {
        return Err(g216("policy_bytes", policy.limits.max_policy_bytes));
    }
    let policy_value: Value =
        serde_json::from_str(source.trim_end()).map_err(|_| g210("policy", POLICY_SCHEMA))?;
    if depth(&policy_value) as u64 > policy.limits.max_json_depth {
        return Err(g216("json_depth", policy.limits.max_json_depth));
    }
    policy.source = source.to_owned();
    Ok(policy)
}

pub(super) fn valid_recipient(rail: EconomicRail, value: &str) -> bool {
    match rail {
        EconomicRail::Evm => {
            value.len() == 42
                && value.starts_with("0x")
                && value[2..]
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        }
        EconomicRail::Solana => decode_base58_32(value).is_some(),
        EconomicRail::Bitcoin => decode_regtest_p2wpkh(value).is_some(),
    }
}
pub(super) fn valid_origin(value: &str) -> bool {
    let Some(host) = value.strip_prefix("https://") else {
        return false;
    };
    !host.is_empty()
        && !host.contains(['/', ':', '@', '#', '?', '[', ']'])
        && host.parse::<IpAddr>().is_err()
        && host != "localhost"
        && !host.ends_with(".localhost")
        && !host.ends_with(".local")
        && host.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-')
        })
        && host
            .split('.')
            .all(|part| !part.is_empty() && !part.starts_with('-') && !part.ends_with('-'))
}

pub(super) fn valid_resource(value: &str) -> bool {
    if !value.starts_with('/') || value.starts_with("//") || value.contains(['?', '#', '\\']) {
        return false;
    }
    if value
        .split('/')
        .any(|segment| matches!(segment, "." | ".."))
    {
        return false;
    }
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'%' {
            index += 1;
            continue;
        }
        let Some(pair) = bytes.get(index + 1..index + 3) else {
            return false;
        };
        let Some(high) = (pair[0] as char).to_digit(16) else {
            return false;
        };
        let Some(low) = (pair[1] as char).to_digit(16) else {
            return false;
        };
        if matches!(((high << 4) | low) as u8, b'.' | b'/' | b'\\') {
            return false;
        }
        index += 3;
    }
    true
}
