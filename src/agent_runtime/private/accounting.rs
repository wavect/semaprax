use std::fmt::Write as _;

use super::execution::checked_add;
use super::replay::canonical_sha256;
use super::*;

pub(super) const ACCOUNTING_RECEIPT_SCHEMA: &str = "semaprax.agent-runtime-accounting-receipt.v1";
const ACCOUNTING_RECEIPT_DOMAIN: &[u8] = b"semaprax.agent-runtime.accounting-receipt-digest.v1\0";
const MAX_ACCOUNTING_RECEIPT_BYTES: usize = 16_384;

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum ChargeObservation {
    Unknown,
    Observed(u64),
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) struct ProviderAccounting {
    reserved_usd_microunits: u64,
    charge: ChargeObservation,
}

pub(super) fn reserve_provider_attempt(
    state: &mut RunState,
    route: &Route,
) -> Result<(), Diagnostic> {
    if state.provider_accounting.len() >= MAX_PROVIDER_ATTEMPTS as usize {
        return Err(g209());
    }
    state.provider_accounting.push(ProviderAccounting {
        reserved_usd_microunits: route.reserved_cost,
        charge: ChargeObservation::Unknown,
    });
    Ok(())
}

pub(super) fn observe_provider_attempt(
    state: &mut RunState,
    reported: ProviderUsage,
    route: &Route,
) -> Result<(), Diagnostic> {
    if !reported.is_reported() {
        return Ok(());
    }
    if reported.usd_microunits > route.reserved_cost {
        return Err(operational(
            "SPX-I218",
            "Agent Runtime provider adapter failed: usage invalid",
        ));
    }
    let Some(attempt) = state.provider_accounting.last_mut() else {
        return Err(g209());
    };
    if attempt.reserved_usd_microunits != route.reserved_cost
        || attempt.charge != ChargeObservation::Unknown
    {
        return Err(g209());
    }
    attempt.charge = ChargeObservation::Observed(reported.usd_microunits);
    Ok(())
}

pub(super) fn account_uncertain(
    usage: &mut Usage,
    sink: &ProviderSink,
    reported: ProviderUsage,
    route: &Route,
    limits: EffectiveLimits,
) -> Result<(), Diagnostic> {
    if reported.input_tokens > route.input_tokens
        || reported.output_tokens > route.output_token_reservation
        || reported.usd_microunits > route.reserved_cost
    {
        return Err(operational(
            "SPX-I218",
            "Agent Runtime provider adapter failed: usage invalid",
        ));
    }
    checked_add(
        &mut usage.provider_output_bytes,
        sink.bounded.bytes.len() as u64,
        "total_provider_output_bytes",
        limits.max_total_provider_output_bytes,
    )?;
    checked_add(
        &mut usage.reported_model_output_tokens,
        reported.output_tokens,
        "reported_model_output_tokens",
        limits.max_reported_model_output_tokens,
    )?;
    Ok(())
}

pub(super) fn account_partial_provider(
    state: &mut RunState,
    sink: &ProviderSink,
    reported: ProviderUsage,
    route: &Route,
    limits: EffectiveLimits,
) -> Result<(), Diagnostic> {
    account_uncertain(&mut state.usage, sink, reported, route, limits)
}

pub(super) fn receipt_digest(receipt: &str) -> String {
    digest(ACCOUNTING_RECEIPT_DOMAIN, receipt.as_bytes())
}

pub(super) fn render_accounting_receipt(
    state: &RunState,
    trace_digest: &str,
    evidence_digest: &str,
) -> Result<String, Diagnostic> {
    if state.usage.provider_attempts != state.provider_accounting.len() as u64 {
        return Err(g209());
    }
    let totals = totals(&state.provider_accounting)?;
    let mut attempts = String::new();
    for (index, attempt) in state.provider_accounting.iter().enumerate() {
        if index != 0 {
            attempts.push(',');
        }
        match attempt.charge {
            ChargeObservation::Unknown => {
                write!(
                    attempts,
                    "{{\"index\":{index},\"reserved_usd_microunits\":{},\"charge\":{{\"kind\":\"unknown\"}}}}",
                    attempt.reserved_usd_microunits,
                )
                .map_err(|_| g209())?;
            }
            ChargeObservation::Observed(usd_microunits) => {
                write!(
                    attempts,
                    "{{\"index\":{index},\"reserved_usd_microunits\":{},\"charge\":{{\"kind\":\"observed\",\"usd_microunits\":{usd_microunits}}}}}",
                    attempt.reserved_usd_microunits,
                )
                .map_err(|_| g209())?;
            }
        }
    }
    let receipt = format!(
        "{{\"schema\":\"{ACCOUNTING_RECEIPT_SCHEMA}\",\"run_id\":{},\"trace_digest\":{},\"evidence_digest\":{},\"status\":\"{}\",\"totals\":{{\"reserved_usd_microunits\":{},\"observed_usd_microunits\":{},\"unknown_usd_microunits\":{}}},\"attempts\":[{attempts}]}}\n",
        quote_json(&state.run_id),
        quote_json(trace_digest),
        quote_json(evidence_digest),
        state.termination.status.text(),
        totals.reserved_usd_microunits,
        totals.observed_usd_microunits,
        totals.unknown_usd_microunits,
    );
    if receipt.len() > MAX_ACCOUNTING_RECEIPT_BYTES {
        return Err(g208("builder_bytes", MAX_ACCOUNTING_RECEIPT_BYTES as u64));
    }
    Ok(receipt)
}

pub(super) fn reconcile_accounting_receipt(
    source: &str,
    state: &RunState,
    trace_digest: &str,
    evidence_digest: &str,
) -> Result<(), Diagnostic> {
    let value = canonical_document(
        source,
        "accounting receipt",
        ACCOUNTING_RECEIPT_SCHEMA,
        MAX_ACCOUNTING_RECEIPT_BYTES,
    )?;
    let top = value.as_object().ok_or_else(g209)?;
    if !exact_keys(
        top,
        &[
            "schema",
            "run_id",
            "trace_digest",
            "evidence_digest",
            "status",
            "totals",
            "attempts",
        ],
    ) || top.get("schema").and_then(Value::as_str) != Some(ACCOUNTING_RECEIPT_SCHEMA)
        || top.get("run_id").and_then(Value::as_str) != Some(&state.run_id)
        || top.get("trace_digest").and_then(Value::as_str) != Some(trace_digest)
        || top.get("evidence_digest").and_then(Value::as_str) != Some(evidence_digest)
        || top.get("status").and_then(Value::as_str) != Some(state.termination.status.text())
        || !canonical_sha256(&state.run_id)
        || !canonical_sha256(trace_digest)
        || !canonical_sha256(evidence_digest)
    {
        return Err(g209());
    }
    if state.usage.provider_attempts != state.provider_accounting.len() as u64 {
        return Err(g209());
    }
    let expected = totals(&state.provider_accounting)?;
    let totals = top
        .get("totals")
        .and_then(Value::as_object)
        .ok_or_else(g209)?;
    if !exact_keys(
        totals,
        &[
            "reserved_usd_microunits",
            "observed_usd_microunits",
            "unknown_usd_microunits",
        ],
    ) || totals
        .get("reserved_usd_microunits")
        .and_then(Value::as_u64)
        != Some(expected.reserved_usd_microunits)
        || totals
            .get("observed_usd_microunits")
            .and_then(Value::as_u64)
            != Some(expected.observed_usd_microunits)
        || totals.get("unknown_usd_microunits").and_then(Value::as_u64)
            != Some(expected.unknown_usd_microunits)
    {
        return Err(g209());
    }
    let attempts = top
        .get("attempts")
        .and_then(Value::as_array)
        .ok_or_else(g209)?;
    if attempts.len() != state.provider_accounting.len()
        || attempts.len() > MAX_PROVIDER_ATTEMPTS as usize
    {
        return Err(g209());
    }
    for (index, (row, expected)) in attempts.iter().zip(&state.provider_accounting).enumerate() {
        let row = row.as_object().ok_or_else(g209)?;
        if !exact_keys(row, &["index", "reserved_usd_microunits", "charge"])
            || row.get("index").and_then(Value::as_u64) != Some(index as u64)
            || row.get("reserved_usd_microunits").and_then(Value::as_u64)
                != Some(expected.reserved_usd_microunits)
        {
            return Err(g209());
        }
        let charge = row
            .get("charge")
            .and_then(Value::as_object)
            .ok_or_else(g209)?;
        match expected.charge {
            ChargeObservation::Unknown => {
                if !exact_keys(charge, &["kind"])
                    || charge.get("kind").and_then(Value::as_str) != Some("unknown")
                {
                    return Err(g209());
                }
            }
            ChargeObservation::Observed(usd_microunits) => {
                if !exact_keys(charge, &["kind", "usd_microunits"])
                    || charge.get("kind").and_then(Value::as_str) != Some("observed")
                    || charge.get("usd_microunits").and_then(Value::as_u64) != Some(usd_microunits)
                    || usd_microunits > expected.reserved_usd_microunits
                {
                    return Err(g209());
                }
            }
        }
    }
    if source != render_accounting_receipt(state, trace_digest, evidence_digest)? {
        return Err(g209());
    }
    Ok(())
}

#[cfg(test)]
pub(in crate::agent_runtime) fn replay_accounting_receipt(
    source: &str,
    run: &AgentRun,
) -> Result<(), Diagnostic> {
    reconcile_accounting_receipt(
        source,
        &run.replay.state,
        &run.trace_digest,
        &run.evidence_digest,
    )
}

struct Totals {
    reserved_usd_microunits: u64,
    observed_usd_microunits: u64,
    unknown_usd_microunits: u64,
}

fn totals(attempts: &[ProviderAccounting]) -> Result<Totals, Diagnostic> {
    let mut totals = Totals {
        reserved_usd_microunits: 0,
        observed_usd_microunits: 0,
        unknown_usd_microunits: 0,
    };
    for attempt in attempts {
        totals.reserved_usd_microunits = totals
            .reserved_usd_microunits
            .checked_add(attempt.reserved_usd_microunits)
            .ok_or_else(g209)?;
        match attempt.charge {
            ChargeObservation::Unknown => {
                totals.unknown_usd_microunits = totals
                    .unknown_usd_microunits
                    .checked_add(attempt.reserved_usd_microunits)
                    .ok_or_else(g209)?;
            }
            ChargeObservation::Observed(usd_microunits) => {
                if usd_microunits > attempt.reserved_usd_microunits {
                    return Err(g209());
                }
                totals.observed_usd_microunits = totals
                    .observed_usd_microunits
                    .checked_add(usd_microunits)
                    .ok_or_else(g209)?;
            }
        }
    }
    Ok(totals)
}
