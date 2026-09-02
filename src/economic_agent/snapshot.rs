//! Chain snapshot documents and the byte-level primitives that consume
//! them: hex decoding, RLP encoding, and Solana shortvec encoding.

use super::address::decode_base58_32;
use super::policy::valid_recipient;
use super::validate::{
    canonical, configured_document_limits, digest, g210, g213, g216, g217, keys, number, object,
    rail, terminal_floor, text,
};
use super::{
    Doc, EconomicRail, Limits, MAX_SNAPSHOT_BYTES, MAX_UTXOS, SNAPSHOT_DOMAIN, SNAPSHOT_SCHEMA,
};
use crate::bounded_output::reserve_active_preserving;
use crate::diagnostic::{quote_json, Diagnostic};

#[derive(Clone)]
pub(super) struct Utxo {
    pub(super) txid: String,
    pub(super) vout: u64,
    pub(super) value: u64,
    pub(super) script: String,
    pub(super) confirmations: u64,
}
#[derive(Clone)]
pub(super) enum SnapshotState {
    Evm {
        from: String,
        nonce: u64,
        base_fee: u64,
        priority: u64,
        gas: u64,
    },
    Solana {
        payer: String,
        blockhash: String,
        last_height: u64,
        fee: u64,
    },
    Bitcoin {
        wallet_script: String,
        height: u64,
        fee_rate: u64,
        utxos: Vec<Utxo>,
    },
}
#[derive(Clone)]
pub(super) struct Snapshot {
    pub(super) rail: EconomicRail,
    pub(super) observed: u64,
    pub(super) expires: u64,
    pub(super) state: SnapshotState,
    pub(super) doc: Doc,
}

pub(super) fn render_snapshot(snapshot: &Snapshot) -> String {
    let(network,state)=match &snapshot.state{
    SnapshotState::Evm{from,nonce,base_fee,priority,gas}=>("sepolia",format!("{{\"chain_id\":11155111,\"from\":{},\"nonce\":{nonce},\"base_fee_per_gas\":{base_fee},\"max_priority_fee_per_gas\":{priority},\"gas_limit\":{gas}}}",quote_json(from))),
    SnapshotState::Solana{payer,blockhash,last_height,fee}=>("devnet",format!("{{\"fee_payer\":{},\"recent_blockhash\":{},\"last_valid_block_height\":{last_height},\"lamports_per_signature\":{fee}}}",quote_json(payer),quote_json(blockhash))),
    SnapshotState::Bitcoin{wallet_script,height,fee_rate,utxos}=>{let mut rows=String::from("[");for(index,u)in utxos.iter().enumerate(){if index>0{rows.push(',');}rows.push_str(&format!("{{\"txid\":{},\"vout\":{},\"value_atomic\":{},\"script_pubkey\":{},\"confirmations\":{}}}",quote_json(&u.txid),u.vout,u.value,quote_json(&u.script),u.confirmations));}rows.push(']');("regtest",format!("{{\"wallet_script_pubkey\":{},\"height\":{height},\"fee_rate_sat_vbyte\":{fee_rate},\"utxos\":{rows}}}",quote_json(wallet_script)))} };
    format!("{{\"schema\":\"{SNAPSHOT_SCHEMA}\",\"rail\":{},\"network\":{},\"observed_at_ms\":{},\"expires_at_ms\":{},\"state\":{state}}}\n",quote_json(snapshot.rail.text()),quote_json(network),snapshot.observed,snapshot.expires)
}

pub(super) fn parse_snapshot(source: &str, expected: EconomicRail) -> Result<Snapshot, Diagnostic> {
    let (_, value) = canonical(
        source,
        "chain snapshot",
        SNAPSHOT_SCHEMA,
        MAX_SNAPSHOT_BYTES,
    )?;
    let top = object(&value, "chain snapshot", SNAPSHOT_SCHEMA)?;
    if !keys(
        top,
        &[
            "schema",
            "rail",
            "network",
            "observed_at_ms",
            "expires_at_ms",
            "state",
        ],
    ) {
        return Err(g210("chain snapshot", SNAPSHOT_SCHEMA));
    }
    let parsed = rail(text(top, "rail", "chain snapshot", SNAPSHOT_SCHEMA)?).ok_or_else(g213)?;
    if parsed != expected {
        return Err(g213());
    }
    let observed = number(top, "observed_at_ms", "chain snapshot", SNAPSHOT_SCHEMA)?;
    let expires = number(top, "expires_at_ms", "chain snapshot", SNAPSHOT_SCHEMA)?;
    if expires <= observed || expires - observed > 600_000 {
        return Err(g213());
    }
    let row = object(&top["state"], "chain snapshot", SNAPSHOT_SCHEMA)?;
    let state = match parsed {
        EconomicRail::Evm => {
            if text(top, "network", "chain snapshot", SNAPSHOT_SCHEMA)? != "sepolia"
                || !keys(
                    row,
                    &[
                        "chain_id",
                        "from",
                        "nonce",
                        "base_fee_per_gas",
                        "max_priority_fee_per_gas",
                        "gas_limit",
                    ],
                )
                || number(row, "chain_id", "chain snapshot", SNAPSHOT_SCHEMA)? != 11155111
            {
                return Err(g213());
            }
            let from = text(row, "from", "chain snapshot", SNAPSHOT_SCHEMA)?.to_owned();
            if !valid_recipient(parsed, &from) {
                return Err(g213());
            }
            let gas = number(row, "gas_limit", "chain snapshot", SNAPSHOT_SCHEMA)?;
            if gas != 21000 {
                return Err(g213());
            }
            SnapshotState::Evm {
                from,
                nonce: number(row, "nonce", "chain snapshot", SNAPSHOT_SCHEMA)?,
                base_fee: number(row, "base_fee_per_gas", "chain snapshot", SNAPSHOT_SCHEMA)?,
                priority: number(
                    row,
                    "max_priority_fee_per_gas",
                    "chain snapshot",
                    SNAPSHOT_SCHEMA,
                )?,
                gas,
            }
        }
        EconomicRail::Solana => {
            if text(top, "network", "chain snapshot", SNAPSHOT_SCHEMA)? != "devnet"
                || !keys(
                    row,
                    &[
                        "fee_payer",
                        "recent_blockhash",
                        "last_valid_block_height",
                        "lamports_per_signature",
                    ],
                )
            {
                return Err(g213());
            }
            let payer = text(row, "fee_payer", "chain snapshot", SNAPSHOT_SCHEMA)?.to_owned();
            let blockhash =
                text(row, "recent_blockhash", "chain snapshot", SNAPSHOT_SCHEMA)?.to_owned();
            if decode_base58_32(&payer).is_none() || decode_base58_32(&blockhash).is_none() {
                return Err(g213());
            }
            SnapshotState::Solana {
                payer,
                blockhash,
                last_height: number(
                    row,
                    "last_valid_block_height",
                    "chain snapshot",
                    SNAPSHOT_SCHEMA,
                )?,
                fee: number(
                    row,
                    "lamports_per_signature",
                    "chain snapshot",
                    SNAPSHOT_SCHEMA,
                )?,
            }
        }
        EconomicRail::Bitcoin => {
            if text(top, "network", "chain snapshot", SNAPSHOT_SCHEMA)? != "regtest"
                || !keys(
                    row,
                    &[
                        "wallet_script_pubkey",
                        "height",
                        "fee_rate_sat_vbyte",
                        "utxos",
                    ],
                )
            {
                return Err(g213());
            }
            let wallet_script = text(
                row,
                "wallet_script_pubkey",
                "chain snapshot",
                SNAPSHOT_SCHEMA,
            )?
            .to_owned();
            if !valid_script(&wallet_script) {
                return Err(g213());
            }
            let values = row["utxos"].as_array().ok_or_else(g213)?;
            if values.is_empty() || values.len() > MAX_UTXOS {
                return Err(g216("utxos", MAX_UTXOS as u64));
            }
            let mut utxos = Vec::new();
            for value in values {
                let u = object(value, "chain snapshot", SNAPSHOT_SCHEMA)?;
                if !keys(
                    u,
                    &[
                        "txid",
                        "vout",
                        "value_atomic",
                        "script_pubkey",
                        "confirmations",
                    ],
                ) {
                    return Err(g210("chain snapshot", SNAPSHOT_SCHEMA));
                }
                let txid = text(u, "txid", "chain snapshot", SNAPSHOT_SCHEMA)?.to_owned();
                let script =
                    text(u, "script_pubkey", "chain snapshot", SNAPSHOT_SCHEMA)?.to_owned();
                if !lower_hex(&txid, 64) || !valid_script(&script) {
                    return Err(g213());
                }
                utxos.push(Utxo {
                    txid,
                    vout: number(u, "vout", "chain snapshot", SNAPSHOT_SCHEMA)?,
                    value: number(u, "value_atomic", "chain snapshot", SNAPSHOT_SCHEMA)?,
                    script,
                    confirmations: number(u, "confirmations", "chain snapshot", SNAPSHOT_SCHEMA)?,
                });
            }
            if !utxos
                .windows(2)
                .all(|w| (w[0].txid.as_str(), w[0].vout) < (w[1].txid.as_str(), w[1].vout))
                || utxos
                    .iter()
                    .any(|u| u.confirmations == 0 || u.script != wallet_script)
            {
                return Err(g213());
            }
            SnapshotState::Bitcoin {
                wallet_script,
                height: number(row, "height", "chain snapshot", SNAPSHOT_SCHEMA)?,
                fee_rate: number(row, "fee_rate_sat_vbyte", "chain snapshot", SNAPSHOT_SCHEMA)?,
                utxos,
            }
        }
    };
    let mut snapshot = Snapshot {
        rail: parsed,
        observed,
        expires,
        state,
        doc: Doc {
            source: source.to_owned(),
            digest: digest(SNAPSHOT_DOMAIN, source.as_bytes()),
        },
    };
    if render_snapshot(&snapshot) != source {
        return Err(g210("chain snapshot", SNAPSHOT_SCHEMA));
    }
    snapshot.doc.source = source.to_owned();
    Ok(snapshot)
}
pub(super) fn parse_snapshot_limited(
    source: &str,
    expected: EconomicRail,
    limits: &Limits,
) -> Result<Snapshot, Diagnostic> {
    configured_document_limits(source, "chain snapshot", limits.max_snapshot_bytes, limits)?;
    reserve_parse_sidecar(source, limits)?;
    let snapshot = parse_snapshot(source, expected)?;
    if let SnapshotState::Bitcoin { utxos, .. } = &snapshot.state {
        if utxos.len() > limits.max_utxos as usize {
            return Err(g216("utxos", limits.max_utxos));
        }
    }
    Ok(snapshot)
}
pub(super) fn reserve_parse_sidecar(source: &str, limits: &Limits) -> Result<(), Diagnostic> {
    let multiplier = usize::try_from(limits.max_json_depth)
        .map_err(|_| g217())?
        .checked_add(2)
        .ok_or_else(g217)?;
    let sidecar = source.len().checked_mul(multiplier).ok_or_else(g217)?;
    if !reserve_active_preserving(sidecar, terminal_floor(limits)?) {
        return Err(g216("builder_bytes", limits.max_builder_bytes));
    }
    Ok(())
}
pub(super) fn lower_hex(value: &str, n: usize) -> bool {
    value.len() == n
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
pub(super) fn valid_script(value: &str) -> bool {
    value.len() == 44 && value.starts_with("0014") && lower_hex(value, 44)
}
pub(super) fn hex_bytes(value: &str) -> Option<Vec<u8>> {
    if !value.len().is_multiple_of(2) {
        return None;
    }
    value
        .as_bytes()
        .chunks(2)
        .map(|c| u8::from_str_radix(std::str::from_utf8(c).ok()?, 16).ok())
        .collect()
}

pub(super) fn rlp_bytes(value: &[u8]) -> Vec<u8> {
    if value.len() == 1 && value[0] < 0x80 {
        return value.to_vec();
    }
    if value.len() < 56 {
        let mut out = vec![0x80 + value.len() as u8];
        out.extend_from_slice(value);
        out
    } else {
        let len = (value.len() as u64).to_be_bytes();
        let first = len.iter().position(|b| *b != 0).unwrap_or(7);
        let mut out = vec![0xb7 + (8 - first) as u8];
        out.extend_from_slice(&len[first..]);
        out.extend_from_slice(value);
        out
    }
}
pub(super) fn rlp_u64(value: u64) -> Vec<u8> {
    if value == 0 {
        return vec![0x80];
    }
    let bytes = value.to_be_bytes();
    rlp_bytes(&bytes[bytes.iter().position(|b| *b != 0).unwrap_or(7)..])
}
pub(super) fn rlp_list(items: &[Vec<u8>]) -> Vec<u8> {
    let payload = items.concat();
    if payload.len() < 56 {
        let mut out = vec![0xc0 + payload.len() as u8];
        out.extend(payload);
        out
    } else {
        let len = (payload.len() as u64).to_be_bytes();
        let first = len.iter().position(|b| *b != 0).unwrap_or(7);
        let mut out = vec![0xf7 + (8 - first) as u8];
        out.extend_from_slice(&len[first..]);
        out.extend(payload);
        out
    }
}
pub(super) fn shortvec(mut value: usize) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value > 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            return out;
        }
    }
}
