//! Unsigned transaction construction per rail and verification of the
//! signed bytes a custody adapter returns, including keccak and
//! transaction-identity derivation.

use super::address::{decode_base58_32, decode_regtest_p2wpkh, encode_base58};
use super::snapshot::{
    hex_bytes, rlp_bytes, rlp_list, rlp_u64, shortvec, Snapshot, SnapshotState, Utxo,
};
use super::validate::{g213, g216};
use super::{EconomicRail, Intent, Payment, MAX_BUILDER_BYTES, MAX_UNSIGNED_BYTES};
use crate::bounded_output::reserve_active_preserving;
use crate::diagnostic::Diagnostic;
use sha2::{Digest, Sha256};

pub(super) fn build_unsigned(
    intent: &Intent,
    snapshot: &Snapshot,
) -> Result<(Vec<u8>, &'static str), Diagnostic> {
    build_unsigned_limited(
        intent,
        snapshot,
        MAX_UNSIGNED_BYTES as u64,
        MAX_BUILDER_BYTES as u64,
        0,
    )
}
pub(super) fn build_unsigned_limited(
    intent: &Intent,
    snapshot: &Snapshot,
    unsigned_max: u64,
    builder_max: u64,
    terminal_floor: usize,
) -> Result<(Vec<u8>, &'static str), Diagnostic> {
    let bytes = match (&intent.payment, &snapshot.state) {
        (
            Payment::Evm {
                recipient,
                amount,
                max_fee,
            },
            SnapshotState::Evm {
                nonce,
                base_fee,
                priority,
                gas,
                ..
            },
        ) => {
            let per_gas = base_fee
                .checked_mul(2)
                .and_then(|v| v.checked_add(*priority))
                .ok_or_else(g213)?;
            let total = per_gas.checked_mul(21000).ok_or_else(g213)?;
            if total > *max_fee || *gas != 21000 {
                return Err(g213());
            }
            let to = hex_bytes(&recipient[2..]).ok_or_else(g213)?;
            let mut out = vec![0x02];
            out.extend(rlp_list(&[
                rlp_u64(11155111),
                rlp_u64(*nonce),
                rlp_u64(*priority),
                rlp_u64(per_gas),
                rlp_u64(21000),
                rlp_bytes(&to),
                rlp_u64(*amount),
                rlp_bytes(&[]),
                rlp_list(&[]),
            ]));
            out
        }
        (
            Payment::Solana {
                recipient,
                amount,
                max_fee,
                compute,
                priority,
            },
            SnapshotState::Solana {
                payer,
                blockhash,
                fee,
                ..
            },
        ) => {
            if *compute == 0 || *compute > 200000 {
                return Err(g216("compute_units", 200000));
            }
            let price = priority
                .checked_mul(1_000_000)
                .map(|v| v / compute)
                .ok_or_else(g213)?;
            let priority_fee = compute
                .checked_mul(price)
                .and_then(|v| v.checked_add(999999))
                .map(|v| v / 1000000)
                .ok_or_else(g213)?;
            if priority_fee > *priority
                || fee.checked_add(priority_fee).ok_or_else(g213)? > *max_fee
            {
                return Err(g213());
            }
            let payer = decode_base58_32(payer).ok_or_else(g213)?;
            let recipient = decode_base58_32(recipient).ok_or_else(g213)?;
            let system = decode_base58_32("11111111111111111111111111111111").ok_or_else(g213)?;
            let compute_program =
                decode_base58_32("ComputeBudget111111111111111111111111111111").ok_or_else(g213)?;
            let blockhash = decode_base58_32(blockhash).ok_or_else(g213)?;
            let mut out = vec![0x80, 1, 0, 2];
            out.extend(shortvec(4));
            out.extend(payer);
            out.extend(recipient);
            out.extend(compute_program);
            out.extend(system);
            out.extend(blockhash);
            out.extend(shortvec(3));
            out.push(2);
            out.extend(shortvec(0));
            out.extend(shortvec(5));
            out.push(2);
            out.extend_from_slice(&(*compute as u32).to_le_bytes());
            out.push(2);
            out.extend(shortvec(0));
            out.extend(shortvec(9));
            out.push(3);
            out.extend_from_slice(&price.to_le_bytes());
            out.push(3);
            out.extend(shortvec(2));
            out.extend([0, 1]);
            out.extend(shortvec(12));
            out.extend_from_slice(&2u32.to_le_bytes());
            out.extend_from_slice(&amount.to_le_bytes());
            out.extend(shortvec(0));
            out
        }
        (
            Payment::Bitcoin {
                recipient,
                amount,
                max_fee,
                ..
            },
            SnapshotState::Bitcoin {
                height,
                fee_rate,
                utxos,
                wallet_script,
            },
        ) => build_psbt(
            utxos,
            wallet_script,
            recipient,
            *amount,
            *max_fee,
            *fee_rate,
            *height,
        )?,
        (Payment::X402 { rail, .. }, _) => {
            let mut clone = intent.clone();
            clone.payment = match rail {
                EconomicRail::Evm => {
                    if let Payment::X402 {
                        payee,
                        amount,
                        max_fee,
                        ..
                    } = &intent.payment
                    {
                        Payment::Evm {
                            recipient: payee.clone(),
                            amount: *amount,
                            max_fee: *max_fee,
                        }
                    } else {
                        unreachable!()
                    }
                }
                EconomicRail::Solana => {
                    if let Payment::X402 {
                        payee,
                        amount,
                        max_fee,
                        ..
                    } = &intent.payment
                    {
                        Payment::Solana {
                            recipient: payee.clone(),
                            amount: *amount,
                            max_fee: *max_fee,
                            compute: 200_000,
                            priority: 0,
                        }
                    } else {
                        unreachable!()
                    }
                }
                EconomicRail::Bitcoin => {
                    if let Payment::X402 {
                        payee,
                        amount,
                        max_fee,
                        ..
                    } = &intent.payment
                    {
                        Payment::Bitcoin {
                            recipient: payee.clone(),
                            amount: *amount,
                            max_fee: *max_fee,
                            confirmations: 1,
                        }
                    } else {
                        unreachable!()
                    }
                }
            };
            return build_unsigned_limited(
                &clone,
                snapshot,
                unsigned_max,
                builder_max,
                terminal_floor,
            );
        }
        _ => return Err(g213()),
    };
    if bytes.len() as u64 > unsigned_max {
        return Err(g216("unsigned_transaction_bytes", unsigned_max));
    }
    if !reserve_active_preserving(bytes.len(), terminal_floor) {
        return Err(g216("builder_bytes", builder_max));
    }
    let format = match snapshot.rail {
        EconomicRail::Evm => "eip1559-unsigned-v1",
        EconomicRail::Solana => "solana-message-v0",
        EconomicRail::Bitcoin => "psbt-v2",
    };
    Ok((bytes, format))
}

pub(super) fn build_psbt(
    utxos: &[Utxo],
    wallet_script: &str,
    recipient: &str,
    amount: u64,
    max_fee: u64,
    fee_rate: u64,
    height: u64,
) -> Result<Vec<u8>, Diagnostic> {
    let mut selected = Vec::new();
    let mut total = 0u64;
    for u in utxos {
        selected.push(u);
        total = total.checked_add(u.value).ok_or_else(g213)?;
        let estimate = 10 + selected.len() as u64 * 68 + 2 * 31;
        let fee = estimate.checked_mul(fee_rate).ok_or_else(g213)?;
        if total >= amount.saturating_add(fee) {
            break;
        }
    }
    let estimate = 10 + selected.len() as u64 * 68 + 2 * 31;
    let mut fee = estimate.checked_mul(fee_rate).ok_or_else(g213)?;
    if fee > max_fee || total < amount.saturating_add(fee) {
        return Err(g213());
    }
    let mut change = total - amount - fee;
    if change < 546 {
        fee = fee.checked_add(change).ok_or_else(g213)?;
        change = 0;
    }
    if fee > max_fee {
        return Err(g213());
    }
    let recipient_script = decode_regtest_p2wpkh(recipient).ok_or_else(g213)?;
    let change_script = hex_bytes(wallet_script).ok_or_else(g213)?;
    let mut outputs = vec![(recipient_script, amount)];
    if change > 0 {
        outputs.push((change_script, change));
    }
    outputs.sort_by(|a, b| (a.1, a.0.as_slice()).cmp(&(b.1, b.0.as_slice())));
    let mut out = b"psbt\xff".to_vec();
    psbt_pair(&mut out, &[0x02], &2u32.to_le_bytes());
    psbt_pair(&mut out, &[0x03], &(height as u32).to_le_bytes());
    psbt_pair(&mut out, &[0x04], &compact_size(selected.len()));
    psbt_pair(&mut out, &[0x05], &compact_size(outputs.len()));
    psbt_pair(&mut out, &[0x06], &[0]);
    psbt_pair(&mut out, &[0xfb], &2u32.to_le_bytes());
    out.push(0);
    for u in selected {
        let script = hex_bytes(&u.script).ok_or_else(g213)?;
        let mut witness = u.value.to_le_bytes().to_vec();
        witness.extend(compact_size(script.len()));
        witness.extend(script);
        psbt_pair(&mut out, &[0x01], &witness);
        psbt_pair(&mut out, &[0x03], &1u32.to_le_bytes());
        let mut txid = hex_bytes(&u.txid).ok_or_else(g213)?;
        txid.reverse();
        psbt_pair(&mut out, &[0x0e], &txid);
        psbt_pair(&mut out, &[0x0f], &(u.vout as u32).to_le_bytes());
        psbt_pair(&mut out, &[0x10], &0xffff_ffffu32.to_le_bytes());
        out.push(0);
    }
    for (script, value) in outputs {
        psbt_pair(&mut out, &[0x03], &value.to_le_bytes());
        psbt_pair(&mut out, &[0x04], &script);
        out.push(0);
    }
    Ok(out)
}
pub(super) fn psbt_pair(out: &mut Vec<u8>, key: &[u8], value: &[u8]) {
    out.extend(shortvec(key.len()));
    out.extend(key);
    out.extend(shortvec(value.len()));
    out.extend(value);
}
pub(super) fn rlp_header(bytes: &[u8]) -> Option<(bool, usize, usize)> {
    let first = *bytes.first()?;
    match first {
        0x00..=0x7f => Some((false, 0, 1)),
        0x80..=0xb7 => {
            let len = (first - 0x80) as usize;
            (bytes.len() > len && !(len == 1 && bytes[1] < 0x80)).then_some((false, 1, len))
        }
        0xb8..=0xbf => {
            let n = (first - 0xb7) as usize;
            if bytes.len() < 1 + n || bytes[1] == 0 {
                return None;
            }
            let len = bytes[1..1 + n].iter().try_fold(0usize, |value, byte| {
                value.checked_mul(256)?.checked_add(*byte as usize)
            })?;
            (len >= 56 && bytes.len() >= 1 + n + len).then_some((false, 1 + n, len))
        }
        0xc0..=0xf7 => {
            let len = (first - 0xc0) as usize;
            (bytes.len() > len).then_some((true, 1, len))
        }
        0xf8..=0xff => {
            let n = (first - 0xf7) as usize;
            if bytes.len() < 1 + n || bytes[1] == 0 {
                return None;
            }
            let len = bytes[1..1 + n].iter().try_fold(0usize, |value, byte| {
                value.checked_mul(256)?.checked_add(*byte as usize)
            })?;
            (len >= 56 && bytes.len() >= 1 + n + len).then_some((true, 1 + n, len))
        }
    }
}
pub(super) fn rlp_list_items(bytes: &[u8]) -> Option<Vec<&[u8]>> {
    let (list, header, len) = rlp_header(bytes)?;
    if !list || header + len != bytes.len() {
        return None;
    }
    let mut body = &bytes[header..];
    let mut out = Vec::new();
    while !body.is_empty() {
        let (_, item_header, item_len) = rlp_header(body)?;
        let total = item_header.checked_add(item_len)?;
        out.push(&body[..total]);
        body = &body[total..];
    }
    Some(out)
}
pub(super) fn rlp_scalar(item: &[u8]) -> Option<&[u8]> {
    let (list, header, len) = rlp_header(item)?;
    (!list && header + len == item.len()).then_some(&item[header..])
}
pub(super) fn valid_secp_scalar(value: &[u8], low_s: bool) -> bool {
    const ORDER: [u8; 32] = [
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfe, 0xba, 0xae, 0xdc, 0xe6, 0xaf, 0x48, 0xa0,
        0x3b, 0xbf, 0xd2, 0x5e, 0x8c, 0xd0, 0x36, 0x41, 0x41, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00,
    ];
    const HALF: [u8; 32] = [
        0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x5d, 0x57, 0x6e, 0x73, 0x57, 0xa4, 0x50,
        0x1d, 0xdf, 0xe9, 0x2f, 0x46, 0x68, 0x1b, 0x20, 0xa0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00,
    ];
    if value.is_empty() || value.len() > 32 || value.first() == Some(&0) {
        return false;
    }
    let mut padded = [0u8; 32];
    padded[32 - value.len()..].copy_from_slice(value);
    padded < ORDER && (!low_s || padded <= HALF)
}
pub(super) fn verify_evm_signed(unsigned: &[u8], signed: &[u8]) -> bool {
    if unsigned.first() != Some(&2) || signed.first() != Some(&2) {
        return false;
    }
    let Some(unsigned_items) = rlp_list_items(&unsigned[1..]) else {
        return false;
    };
    let Some(signed_items) = rlp_list_items(&signed[1..]) else {
        return false;
    };
    if unsigned_items.len() != 9
        || signed_items.len() != 12
        || unsigned_items
            .iter()
            .zip(&signed_items[..9])
            .any(|(a, b)| a != b)
    {
        return false;
    }
    let Some(parity) = rlp_scalar(signed_items[9]) else {
        return false;
    };
    if !matches!(parity, [] | [1]) {
        return false;
    }
    let Some(r) = rlp_scalar(signed_items[10]) else {
        return false;
    };
    let Some(s) = rlp_scalar(signed_items[11]) else {
        return false;
    };
    valid_secp_scalar(r, false) && valid_secp_scalar(s, true)
}
pub(super) fn take<'a>(bytes: &mut &'a [u8], length: usize) -> Option<&'a [u8]> {
    if bytes.len() < length {
        return None;
    }
    let (value, rest) = bytes.split_at(length);
    *bytes = rest;
    Some(value)
}
pub(super) fn read_compact(bytes: &mut &[u8]) -> Option<u64> {
    let first = *take(bytes, 1)?.first()?;
    match first {
        0..=0xfc => Some(first as u64),
        0xfd => {
            let value = u16::from_le_bytes(take(bytes, 2)?.try_into().ok()?) as u64;
            (value >= 0xfd).then_some(value)
        }
        0xfe => {
            let value = u32::from_le_bytes(take(bytes, 4)?.try_into().ok()?) as u64;
            (value > u16::MAX as u64).then_some(value)
        }
        0xff => {
            let value = u64::from_le_bytes(take(bytes, 8)?.try_into().ok()?);
            (value > u32::MAX as u64).then_some(value)
        }
    }
}
#[derive(Eq, PartialEq)]
pub(super) struct BtcInput {
    pub(super) txid: [u8; 32],
    pub(super) vout: u32,
    pub(super) sequence: u32,
}
#[derive(Eq, PartialEq)]
pub(super) struct BtcOutput {
    pub(super) value: u64,
    pub(super) script: Vec<u8>,
}
pub(super) struct BtcTemplate {
    pub(super) locktime: u32,
    pub(super) inputs: Vec<BtcInput>,
    pub(super) outputs: Vec<BtcOutput>,
}
pub(super) fn psbt_map(bytes: &mut &[u8]) -> Option<Vec<(Vec<u8>, Vec<u8>)>> {
    let mut entries = Vec::new();
    let mut previous: Option<Vec<u8>> = None;
    loop {
        let key_len = read_compact(bytes)? as usize;
        if key_len == 0 {
            return Some(entries);
        }
        let key = take(bytes, key_len)?.to_vec();
        if previous.as_ref().is_some_and(|value| value >= &key) {
            return None;
        }
        previous = Some(key.clone());
        let value_len = read_compact(bytes)? as usize;
        let value = take(bytes, value_len)?.to_vec();
        entries.push((key, value));
    }
}
pub(super) fn parse_psbt_template(unsigned: &[u8]) -> Option<BtcTemplate> {
    let mut bytes = unsigned;
    if take(&mut bytes, 5)? != b"psbt\xff" {
        return None;
    }
    let globals = psbt_map(&mut bytes)?;
    let get = |key: u8| {
        globals
            .iter()
            .find(|(candidate, _)| candidate.as_slice() == [key])
            .map(|(_, value)| value.as_slice())
    };
    if get(0xfb)? != 2u32.to_le_bytes() || get(0x02)? != 2i32.to_le_bytes() || get(0x06)? != [0] {
        return None;
    }
    let locktime = u32::from_le_bytes(get(0x03)?.try_into().ok()?);
    let mut count_bytes = get(0x04)?;
    let input_count = read_compact(&mut count_bytes)? as usize;
    if !count_bytes.is_empty() || input_count > 100 {
        return None;
    }
    let mut count_bytes = get(0x05)?;
    let output_count = read_compact(&mut count_bytes)? as usize;
    if !count_bytes.is_empty() {
        return None;
    }
    let mut inputs = Vec::new();
    for _ in 0..input_count {
        let map = psbt_map(&mut bytes)?;
        let get = |key: u8| {
            map.iter()
                .find(|(candidate, _)| candidate.as_slice() == [key])
                .map(|(_, value)| value.as_slice())
        };
        let txid = get(0x0e)?.try_into().ok()?;
        let vout = u32::from_le_bytes(get(0x0f)?.try_into().ok()?);
        let sequence = u32::from_le_bytes(get(0x10)?.try_into().ok()?);
        if sequence != 0xffff_ffff || get(0x03)? != 1u32.to_le_bytes() || get(0x01).is_none() {
            return None;
        }
        inputs.push(BtcInput {
            txid,
            vout,
            sequence,
        });
    }
    let mut outputs = Vec::new();
    for _ in 0..output_count {
        let map = psbt_map(&mut bytes)?;
        let get = |key: u8| {
            map.iter()
                .find(|(candidate, _)| candidate.as_slice() == [key])
                .map(|(_, value)| value.as_slice())
        };
        outputs.push(BtcOutput {
            value: u64::from_le_bytes(get(0x03)?.try_into().ok()?),
            script: get(0x04)?.to_vec(),
        });
    }
    if !bytes.is_empty() {
        return None;
    }
    Some(BtcTemplate {
        locktime,
        inputs,
        outputs,
    })
}
pub(super) fn valid_der_signature(value: &[u8]) -> bool {
    if value.len() < 9
        || value.last() != Some(&1)
        || value[0] != 0x30
        || value[1] as usize + 3 != value.len()
    {
        return false;
    }
    let body = &value[2..value.len() - 1];
    if body.first() != Some(&2) || body.len() < 2 {
        return false;
    }
    let rlen = body[1] as usize;
    if body.len() < 2 + rlen + 2 || rlen == 0 {
        return false;
    }
    let r = &body[2..2 + rlen];
    let rest = &body[2 + rlen..];
    if rest.first() != Some(&2) || rest.len() < 2 || rest.len() != 2 + rest[1] as usize {
        return false;
    }
    let s = &rest[2..];
    fn integer(bytes: &[u8]) -> Option<&[u8]> {
        if bytes.is_empty() || bytes[0] & 0x80 != 0 {
            return None;
        }
        if bytes.len() > 1 && bytes[0] == 0 && bytes[1] & 0x80 == 0 {
            return None;
        }
        Some(if bytes[0] == 0 { &bytes[1..] } else { bytes })
    }
    let Some(r) = integer(r) else { return false };
    let Some(s) = integer(s) else { return false };
    valid_secp_scalar(r, false) && valid_secp_scalar(s, true)
}
pub(super) fn verify_bitcoin_signed(unsigned: &[u8], signed: &[u8]) -> bool {
    let Some(template) = parse_psbt_template(unsigned) else {
        return false;
    };
    let mut bytes = signed;
    if take(&mut bytes, 4) != Some(&2i32.to_le_bytes()) || take(&mut bytes, 2) != Some(&[0, 1]) {
        return false;
    }
    let Some(input_count) = read_compact(&mut bytes).and_then(|value| usize::try_from(value).ok())
    else {
        return false;
    };
    if input_count != template.inputs.len() {
        return false;
    }
    for expected in &template.inputs {
        let Some(txid) = take(&mut bytes, 32) else {
            return false;
        };
        let Some(vout) = take(&mut bytes, 4)
            .and_then(|value| value.try_into().ok())
            .map(u32::from_le_bytes)
        else {
            return false;
        };
        let Some(script_len) =
            read_compact(&mut bytes).and_then(|value| usize::try_from(value).ok())
        else {
            return false;
        };
        if script_len != 0 || take(&mut bytes, script_len).is_none() {
            return false;
        }
        let Some(sequence) = take(&mut bytes, 4)
            .and_then(|value| value.try_into().ok())
            .map(u32::from_le_bytes)
        else {
            return false;
        };
        if txid != expected.txid || vout != expected.vout || sequence != expected.sequence {
            return false;
        }
    }
    let Some(output_count) = read_compact(&mut bytes).and_then(|value| usize::try_from(value).ok())
    else {
        return false;
    };
    if output_count != template.outputs.len() {
        return false;
    }
    for expected in &template.outputs {
        let Some(value) = take(&mut bytes, 8)
            .and_then(|value| value.try_into().ok())
            .map(u64::from_le_bytes)
        else {
            return false;
        };
        let Some(script_len) =
            read_compact(&mut bytes).and_then(|value| usize::try_from(value).ok())
        else {
            return false;
        };
        let Some(script) = take(&mut bytes, script_len) else {
            return false;
        };
        if value != expected.value || script != expected.script {
            return false;
        }
    }
    for _ in &template.inputs {
        if read_compact(&mut bytes) != Some(2) {
            return false;
        }
        let Some(sig_len) = read_compact(&mut bytes).and_then(|value| usize::try_from(value).ok())
        else {
            return false;
        };
        let Some(signature) = take(&mut bytes, sig_len) else {
            return false;
        };
        if !valid_der_signature(signature) {
            return false;
        }
        if read_compact(&mut bytes) != Some(33) {
            return false;
        }
        let Some(pubkey) = take(&mut bytes, 33) else {
            return false;
        };
        if !matches!(pubkey.first(), Some(2 | 3)) {
            return false;
        }
    }
    take(&mut bytes, 4) == Some(&template.locktime.to_le_bytes()) && bytes.is_empty()
}
pub(super) fn verify_signed(
    rail: EconomicRail,
    unsigned: &[u8],
    signed: &[u8],
) -> Result<(), Diagnostic> {
    let valid = match rail {
        EconomicRail::Solana => {
            signed.len() == 1 + 64 + unsigned.len()
                && signed.first() == Some(&1)
                && signed[1..65].iter().any(|byte| *byte != 0)
                && &signed[65..] == unsigned
        }
        EconomicRail::Evm => verify_evm_signed(unsigned, signed),
        EconomicRail::Bitcoin => verify_bitcoin_signed(unsigned, signed),
    };
    if valid {
        Ok(())
    } else {
        Err(g213())
    }
}

pub(super) fn keccak_f(state: &mut [u64; 25]) {
    const R: [u32; 25] = [
        0, 1, 62, 28, 27, 36, 44, 6, 55, 20, 3, 10, 43, 25, 39, 41, 45, 15, 21, 8, 18, 2, 61, 56,
        14,
    ];
    const RC: [u64; 24] = [
        0x0000000000000001,
        0x0000000000008082,
        0x800000000000808a,
        0x8000000080008000,
        0x000000000000808b,
        0x0000000080000001,
        0x8000000080008081,
        0x8000000000008009,
        0x000000000000008a,
        0x0000000000000088,
        0x0000000080008009,
        0x000000008000000a,
        0x000000008000808b,
        0x800000000000008b,
        0x8000000000008089,
        0x8000000000008003,
        0x8000000000008002,
        0x8000000000000080,
        0x000000000000800a,
        0x800000008000000a,
        0x8000000080008081,
        0x8000000000008080,
        0x0000000080000001,
        0x8000000080008008,
    ];
    for rc in RC {
        let mut c = [0u64; 5];
        for x in 0..5 {
            c[x] = state[x] ^ state[x + 5] ^ state[x + 10] ^ state[x + 15] ^ state[x + 20];
        }
        let mut d = [0u64; 5];
        for x in 0..5 {
            d[x] = c[(x + 4) % 5] ^ c[(x + 1) % 5].rotate_left(1);
        }
        for y in 0..5 {
            for x in 0..5 {
                state[x + 5 * y] ^= d[x];
            }
        }
        let mut b = [0u64; 25];
        for y in 0..5 {
            for x in 0..5 {
                b[y % 5 + 5 * ((2 * x + 3 * y) % 5)] = state[x + 5 * y].rotate_left(R[x + 5 * y]);
            }
        }
        for y in 0..5 {
            for x in 0..5 {
                state[x + 5 * y] =
                    b[x + 5 * y] ^ ((!b[(x + 1) % 5 + 5 * y]) & b[(x + 2) % 5 + 5 * y]);
            }
        }
        state[0] ^= rc;
    }
}
pub(super) fn keccak256(bytes: &[u8]) -> [u8; 32] {
    let mut state = [0u64; 25];
    let (blocks, remainder) = bytes.as_chunks::<136>();
    for block in blocks {
        for (index, word) in block.as_chunks::<8>().0.iter().enumerate() {
            state[index] ^= u64::from_le_bytes(*word);
        }
        keccak_f(&mut state);
    }
    let mut tail = [0u8; 136];
    tail[..remainder.len()].copy_from_slice(remainder);
    tail[remainder.len()] = 0x01;
    tail[135] |= 0x80;
    for (index, word) in tail.as_chunks::<8>().0.iter().enumerate() {
        state[index] ^= u64::from_le_bytes(*word);
    }
    keccak_f(&mut state);
    let mut output = [0u8; 32];
    for (index, word) in state[..4].iter().enumerate() {
        output[index * 8..index * 8 + 8].copy_from_slice(&word.to_le_bytes());
    }
    output
}
pub(super) fn bitcoin_stripped(signed: &[u8]) -> Option<Vec<u8>> {
    let mut bytes = signed;
    let version = take(&mut bytes, 4)?;
    if take(&mut bytes, 2)? != [0, 1] {
        return None;
    }
    let input_count = read_compact(&mut bytes)?;
    let mut stripped = version.to_vec();
    stripped.extend(compact_size(input_count.try_into().ok()?));
    for _ in 0..input_count {
        let txid = take(&mut bytes, 32)?;
        let vout = take(&mut bytes, 4)?;
        let script_len = read_compact(&mut bytes)?;
        let script = take(&mut bytes, script_len.try_into().ok()?)?;
        let sequence = take(&mut bytes, 4)?;
        stripped.extend(txid);
        stripped.extend(vout);
        stripped.extend(compact_size(script.len()));
        stripped.extend(script);
        stripped.extend(sequence);
    }
    let output_count = read_compact(&mut bytes)?;
    stripped.extend(compact_size(output_count.try_into().ok()?));
    for _ in 0..output_count {
        let value = take(&mut bytes, 8)?;
        let script_len = read_compact(&mut bytes)?;
        let script = take(&mut bytes, script_len.try_into().ok()?)?;
        stripped.extend(value);
        stripped.extend(compact_size(script.len()));
        stripped.extend(script);
    }
    for _ in 0..input_count {
        let items = read_compact(&mut bytes)?;
        for _ in 0..items {
            let len = read_compact(&mut bytes)?;
            take(&mut bytes, len.try_into().ok()?)?;
        }
    }
    let locktime = take(&mut bytes, 4)?;
    if !bytes.is_empty() {
        return None;
    }
    stripped.extend(locktime);
    Some(stripped)
}
pub(super) fn transaction_id(rail: EconomicRail, signed: &[u8]) -> Option<String> {
    match rail {
        EconomicRail::Evm => Some(format!(
            "0x{}",
            keccak256(signed)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        )),
        EconomicRail::Solana => {
            (signed.len() >= 65 && signed[0] == 1).then(|| encode_base58(&signed[1..65]))
        }
        EconomicRail::Bitcoin => {
            let stripped = bitcoin_stripped(signed)?;
            let first = Sha256::digest(&stripped);
            let second = Sha256::digest(first);
            Some(
                second
                    .iter()
                    .rev()
                    .map(|byte| format!("{byte:02x}"))
                    .collect(),
            )
        }
    }
}

pub(super) fn compact_size(value: usize) -> Vec<u8> {
    if value < 0xfd {
        vec![value as u8]
    } else if value <= 0xffff {
        let mut out = vec![0xfd];
        out.extend_from_slice(&(value as u16).to_le_bytes());
        out
    } else {
        let mut out = vec![0xfe];
        out.extend_from_slice(&(value as u32).to_le_bytes());
        out
    }
}
