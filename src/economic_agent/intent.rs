//! Payment intent accessors, policy admission of an intent, and intent
//! document rendering and parsing.

use super::validate::{
    canonical, configured_depth, digest, g210, g212, g216, identifier, keys, number, object, rail,
    text,
};
use super::{
    EconomicRail, Intent, Payment, Policy, INTENT_DOMAIN, INTENT_SCHEMA, MAX_INTENT_BYTES,
    MAX_MEMO_BYTES,
};
use crate::diagnostic::{quote_json, Diagnostic};

impl Intent {
    pub(super) fn settlement_rail(&self) -> EconomicRail {
        match &self.payment {
            Payment::Evm { .. } => EconomicRail::Evm,
            Payment::Solana { .. } => EconomicRail::Solana,
            Payment::Bitcoin { .. } => EconomicRail::Bitcoin,
            Payment::X402 { rail, .. } => *rail,
        }
    }
    pub(super) fn recipient(&self) -> &str {
        match &self.payment {
            Payment::Evm { recipient, .. }
            | Payment::Solana { recipient, .. }
            | Payment::Bitcoin { recipient, .. } => recipient,
            Payment::X402 { payee, .. } => payee,
        }
    }
    pub(super) fn amount(&self) -> u64 {
        match &self.payment {
            Payment::Evm { amount, .. }
            | Payment::Solana { amount, .. }
            | Payment::Bitcoin { amount, .. }
            | Payment::X402 { amount, .. } => *amount,
        }
    }
    pub(super) fn max_fee(&self) -> u64 {
        match &self.payment {
            Payment::Evm { max_fee, .. }
            | Payment::Solana { max_fee, .. }
            | Payment::Bitcoin { max_fee, .. }
            | Payment::X402 { max_fee, .. } => *max_fee,
        }
    }
    pub(super) fn network_asset(&self) -> (&str, &str) {
        match &self.payment {
            Payment::Evm { .. } => ("sepolia", "native:eth"),
            Payment::Solana { .. } => ("devnet", "native:sol"),
            Payment::Bitcoin { .. } => ("regtest", "native:btc"),
            Payment::X402 { network, asset, .. } => (network, asset),
        }
    }
}

pub(super) fn admit_intent(policy: &Policy, intent: &Intent) -> Result<(), Diagnostic> {
    if intent.source.len() > policy.limits.max_intent_bytes as usize {
        return Err(g216("intent_bytes", policy.limits.max_intent_bytes));
    }
    configured_depth(&intent.source, &policy.limits)?;
    if intent.wallet_id != policy.wallet_id {
        return Err(g212("wallet mismatch"));
    }
    let identifier_limit = policy.limits.max_identifier_bytes as usize;
    if [
        intent.intent_id.as_str(),
        intent.wallet_id.as_str(),
        intent.rail_text.as_str(),
        intent.idempotency_key.as_str(),
    ]
    .into_iter()
    .any(|value| value.len() > identifier_limit)
    {
        return Err(g216("identifier_bytes", policy.limits.max_identifier_bytes));
    }
    if intent
        .memo
        .as_ref()
        .is_some_and(|memo| memo.len() > policy.limits.max_memo_bytes as usize)
    {
        return Err(g216("memo_bytes", policy.limits.max_memo_bytes));
    }
    let rail = intent.settlement_rail();
    let (network, asset) = intent.network_asset();
    let Some(network_policy) = policy
        .networks
        .iter()
        .find(|row| row.rail == rail && row.network == network && row.asset == asset)
    else {
        return Err(g212("rail/network/asset not allowed"));
    };
    if !network_policy
        .recipients
        .iter()
        .any(|recipient| recipient == intent.recipient())
    {
        return Err(g212("recipient not allowed"));
    }
    if intent.amount() == 0
        || intent.amount() > network_policy.max_amount
        || intent.amount() > policy.limits.max_amount_atomic
        || intent.max_fee() > network_policy.max_fee
        || intent.max_fee() > policy.limits.max_fee_atomic
    {
        return Err(g212("amount or fee not allowed"));
    }
    match &intent.payment {
        Payment::Solana {
            compute, priority, ..
        } if *compute == 0
            || *compute > policy.limits.max_compute_units
            || *priority > intent.max_fee() =>
        {
            return Err(g212("amount or fee not allowed"))
        }
        Payment::Bitcoin { confirmations, .. }
            if *confirmations == 0 || *confirmations > policy.limits.max_confirmation_target =>
        {
            return Err(g212("amount or fee not allowed"))
        }
        Payment::X402 {
            origin,
            method,
            resource,
            rail,
            nonce,
            ..
        } => {
            if *rail == EconomicRail::Solana && policy.limits.max_compute_units < 200_000 {
                return Err(g212("amount or fee not allowed"));
            }
            if nonce.len() > identifier_limit {
                return Err(g216("identifier_bytes", policy.limits.max_identifier_bytes));
            }
            let Some(row) = policy.origins.iter().find(|row| row.origin == *origin) else {
                return Err(g212("origin/method/resource not allowed"));
            };
            if !row.methods.iter().any(|v| v == method)
                || !row.resources.iter().any(|v| v == resource)
                || !row.rails.contains(rail)
                || intent.amount() > row.max_amount
            {
                return Err(g212("origin/method/resource not allowed"));
            }
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn render_intent(intent: &Intent) -> String {
    let memo = intent
        .memo
        .as_ref()
        .map_or_else(|| "null".to_owned(), |v| quote_json(v));
    let payment=match &intent.payment{
        Payment::Evm{recipient,amount,max_fee}=>format!("{{\"kind\":\"evm\",\"network\":\"sepolia\",\"asset\":\"native:eth\",\"recipient\":{},\"amount_atomic\":{amount},\"max_fee_atomic\":{max_fee}}}",quote_json(recipient)),
        Payment::Solana{recipient,amount,max_fee,compute,priority}=>format!("{{\"kind\":\"solana\",\"network\":\"devnet\",\"asset\":\"native:sol\",\"recipient\":{},\"amount_atomic\":{amount},\"max_fee_atomic\":{max_fee},\"max_compute_units\":{compute},\"max_priority_fee_atomic\":{priority}}}",quote_json(recipient)),
        Payment::Bitcoin{recipient,amount,max_fee,confirmations}=>format!("{{\"kind\":\"bitcoin\",\"network\":\"regtest\",\"asset\":\"native:btc\",\"recipient\":{},\"amount_atomic\":{amount},\"max_fee_atomic\":{max_fee},\"confirmation_target\":{confirmations}}}",quote_json(recipient)),
        Payment::X402{origin,method,resource,invoice_digest,payee,rail,network,asset,amount,max_fee,invoice_expires,nonce}=>format!("{{\"kind\":\"x402\",\"origin\":{},\"method\":{},\"resource\":{},\"invoice_digest\":{},\"payee\":{},\"settlement_rail\":{},\"network\":{},\"asset\":{},\"amount_atomic\":{amount},\"max_fee_atomic\":{max_fee},\"invoice_expires_at_ms\":{invoice_expires},\"invoice_nonce\":{}}}",quote_json(origin),quote_json(method),quote_json(resource),quote_json(invoice_digest),quote_json(payee),quote_json(rail.text()),quote_json(network),quote_json(asset),quote_json(nonce)),
    };
    format!("{{\"schema\":\"{INTENT_SCHEMA}\",\"intent_id\":{},\"wallet_id\":{},\"rail\":{},\"idempotency_key\":{},\"created_at_ms\":{},\"expires_at_ms\":{},\"memo\":{memo},\"payment\":{payment}}}\n",quote_json(&intent.intent_id),quote_json(&intent.wallet_id),quote_json(&intent.rail_text),quote_json(&intent.idempotency_key),intent.created_at,intent.expires_at)
}

pub(super) fn parse_intent(source: &str) -> Result<Intent, Diagnostic> {
    let (_, value) = canonical(source, "payment intent", INTENT_SCHEMA, MAX_INTENT_BYTES)?;
    let top = object(&value, "payment intent", INTENT_SCHEMA)?;
    if !keys(
        top,
        &[
            "schema",
            "intent_id",
            "wallet_id",
            "rail",
            "idempotency_key",
            "created_at_ms",
            "expires_at_ms",
            "memo",
            "payment",
        ],
    ) {
        return Err(g210("payment intent", INTENT_SCHEMA));
    }
    let intent_id = text(top, "intent_id", "payment intent", INTENT_SCHEMA)?.to_owned();
    let wallet_id = text(top, "wallet_id", "payment intent", INTENT_SCHEMA)?.to_owned();
    let rail_text = text(top, "rail", "payment intent", INTENT_SCHEMA)?.to_owned();
    let idempotency_key = text(top, "idempotency_key", "payment intent", INTENT_SCHEMA)?.to_owned();
    if !identifier(&intent_id) || !identifier(&wallet_id) || !identifier(&idempotency_key) {
        return Err(g210("payment intent", INTENT_SCHEMA));
    }
    let created_at = number(top, "created_at_ms", "payment intent", INTENT_SCHEMA)?;
    let expires_at = number(top, "expires_at_ms", "payment intent", INTENT_SCHEMA)?;
    if expires_at <= created_at || expires_at - created_at > 600_000 {
        return Err(g212("expired"));
    }
    let memo = if top["memo"].is_null() {
        None
    } else {
        Some(
            top["memo"]
                .as_str()
                .ok_or_else(|| g210("payment intent", INTENT_SCHEMA))?
                .to_owned(),
        )
    };
    if memo.as_ref().is_some_and(|v| v.len() > MAX_MEMO_BYTES) {
        return Err(g216("memo_bytes", MAX_MEMO_BYTES as u64));
    }
    let row = object(&top["payment"], "payment intent", INTENT_SCHEMA)?;
    let kind = text(row, "kind", "payment intent", INTENT_SCHEMA)?;
    let payment = match kind {
        "evm" => {
            if !keys(
                row,
                &[
                    "kind",
                    "network",
                    "asset",
                    "recipient",
                    "amount_atomic",
                    "max_fee_atomic",
                ],
            ) || text(row, "network", "payment intent", INTENT_SCHEMA)? != "sepolia"
                || text(row, "asset", "payment intent", INTENT_SCHEMA)? != "native:eth"
                || rail_text != "evm"
            {
                return Err(g210("payment intent", INTENT_SCHEMA));
            }
            Payment::Evm {
                recipient: text(row, "recipient", "payment intent", INTENT_SCHEMA)?.to_owned(),
                amount: number(row, "amount_atomic", "payment intent", INTENT_SCHEMA)?,
                max_fee: number(row, "max_fee_atomic", "payment intent", INTENT_SCHEMA)?,
            }
        }
        "solana" => {
            if !keys(
                row,
                &[
                    "kind",
                    "network",
                    "asset",
                    "recipient",
                    "amount_atomic",
                    "max_fee_atomic",
                    "max_compute_units",
                    "max_priority_fee_atomic",
                ],
            ) || text(row, "network", "payment intent", INTENT_SCHEMA)? != "devnet"
                || text(row, "asset", "payment intent", INTENT_SCHEMA)? != "native:sol"
                || rail_text != "solana"
            {
                return Err(g210("payment intent", INTENT_SCHEMA));
            }
            Payment::Solana {
                recipient: text(row, "recipient", "payment intent", INTENT_SCHEMA)?.to_owned(),
                amount: number(row, "amount_atomic", "payment intent", INTENT_SCHEMA)?,
                max_fee: number(row, "max_fee_atomic", "payment intent", INTENT_SCHEMA)?,
                compute: number(row, "max_compute_units", "payment intent", INTENT_SCHEMA)?,
                priority: number(
                    row,
                    "max_priority_fee_atomic",
                    "payment intent",
                    INTENT_SCHEMA,
                )?,
            }
        }
        "bitcoin" => {
            if !keys(
                row,
                &[
                    "kind",
                    "network",
                    "asset",
                    "recipient",
                    "amount_atomic",
                    "max_fee_atomic",
                    "confirmation_target",
                ],
            ) || text(row, "network", "payment intent", INTENT_SCHEMA)? != "regtest"
                || text(row, "asset", "payment intent", INTENT_SCHEMA)? != "native:btc"
                || rail_text != "bitcoin"
            {
                return Err(g210("payment intent", INTENT_SCHEMA));
            }
            Payment::Bitcoin {
                recipient: text(row, "recipient", "payment intent", INTENT_SCHEMA)?.to_owned(),
                amount: number(row, "amount_atomic", "payment intent", INTENT_SCHEMA)?,
                max_fee: number(row, "max_fee_atomic", "payment intent", INTENT_SCHEMA)?,
                confirmations: number(row, "confirmation_target", "payment intent", INTENT_SCHEMA)?,
            }
        }
        "x402" => {
            if !keys(
                row,
                &[
                    "kind",
                    "origin",
                    "method",
                    "resource",
                    "invoice_digest",
                    "payee",
                    "settlement_rail",
                    "network",
                    "asset",
                    "amount_atomic",
                    "max_fee_atomic",
                    "invoice_expires_at_ms",
                    "invoice_nonce",
                ],
            ) || rail_text != "x402"
            {
                return Err(g210("payment intent", INTENT_SCHEMA));
            }
            Payment::X402 {
                origin: text(row, "origin", "payment intent", INTENT_SCHEMA)?.to_owned(),
                method: text(row, "method", "payment intent", INTENT_SCHEMA)?.to_owned(),
                resource: text(row, "resource", "payment intent", INTENT_SCHEMA)?.to_owned(),
                invoice_digest: text(row, "invoice_digest", "payment intent", INTENT_SCHEMA)?
                    .to_owned(),
                payee: text(row, "payee", "payment intent", INTENT_SCHEMA)?.to_owned(),
                rail: rail(text(
                    row,
                    "settlement_rail",
                    "payment intent",
                    INTENT_SCHEMA,
                )?)
                .ok_or_else(|| g210("payment intent", INTENT_SCHEMA))?,
                network: text(row, "network", "payment intent", INTENT_SCHEMA)?.to_owned(),
                asset: text(row, "asset", "payment intent", INTENT_SCHEMA)?.to_owned(),
                amount: number(row, "amount_atomic", "payment intent", INTENT_SCHEMA)?,
                max_fee: number(row, "max_fee_atomic", "payment intent", INTENT_SCHEMA)?,
                invoice_expires: number(
                    row,
                    "invoice_expires_at_ms",
                    "payment intent",
                    INTENT_SCHEMA,
                )?,
                nonce: text(row, "invoice_nonce", "payment intent", INTENT_SCHEMA)?.to_owned(),
            }
        }
        _ => return Err(g210("payment intent", INTENT_SCHEMA)),
    };
    let intent = Intent {
        intent_id,
        wallet_id,
        rail_text,
        idempotency_key,
        created_at,
        expires_at,
        memo,
        payment,
        source: source.to_owned(),
        digest: digest(INTENT_DOMAIN, source.as_bytes()),
    };
    if render_intent(&intent) != source {
        return Err(g210("payment intent", INTENT_SCHEMA));
    }
    Ok(intent)
}
