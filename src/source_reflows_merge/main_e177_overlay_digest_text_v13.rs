//! Canonical 43-byte text for the same 256-bit source digests.
use serde_json::Value;
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
fn decode(text: &str) -> Result<String, &'static str> {
    if text.len() != 43 {
        return Err("v13 digest width");
    }
    let mut bytes = [0u8; 32];
    let mut accumulator = 0u32;
    let mut bits = 0;
    let mut at = 0;
    for byte in text.bytes() {
        let value = ALPHABET
            .iter()
            .position(|candidate| *candidate == byte)
            .ok_or("v13 digest alphabet")? as u32;
        accumulator = (accumulator << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            bytes[at] = (accumulator >> bits) as u8;
            at += 1;
        }
    }
    if at != 32 || bits != 2 || accumulator & 3 != 0 {
        return Err("v13 canonical digest tail");
    }
    use std::fmt::Write;
    let mut hex = String::with_capacity(64);
    for byte in bytes {
        write!(hex, "{byte:02x}").unwrap();
    }
    Ok(hex)
}
pub(super) fn unpack(envelope: &Value) -> Result<Value, &'static str> {
    // Wire, names and digest materialization limits remain the original bounds.
    if !super::super::super::keys(envelope, &["schema", "names", "digests", "page"])
        || envelope["schema"] != "semaprax.finite-source-overlay-page.v13"
    {
        return Err("v13 envelope");
    }
    if super::super::super::canonical(envelope).len() > super::super::super::PAGE_CAP {
        return Err("v13 wire cap");
    }
    let count = envelope["digests"].as_array().ok_or("v13 digests")?.len();
    if count
        .checked_mul(66)
        .is_none_or(|bytes| bytes > super::super::super::PAGE_CAP)
    {
        return Err("v13 materialized digest table cap");
    }
    let mut result = envelope.clone();
    result["schema"] = super::SCHEMA_V12.into();
    for digest in result["digests"].as_array_mut().ok_or("v13 digests")? {
        *digest = decode(digest.as_str().ok_or("v13 digest text")?)?.into();
    }
    result["page"] = super::v10_slots(&result["page"], &|value, named| {
        let Some(tuple) = value.as_array() else {
            return Ok(value.clone());
        };
        let offset = 1 + usize::from(named);
        let Some(slot) = tuple.get(offset) else {
            return Ok(value.clone());
        };
        let Some(text) = slot.as_str() else {
            return Ok(value.clone());
        };
        let mut decoded = tuple.clone();
        decoded[offset] = decode(text)?.into();
        Ok(decoded.into())
    })?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::super::super::super::{canonical, compose_values, PAGE_CAP, SOURCE_CAP};
    use super::*;
    fn fixture() -> Value {
        serde_json::from_str(include_str!(
            "../../docs/evidence/finite-source-overlay-v2-20261018/pin-digest-fixtures-v13.json"
        ))
        .unwrap()
    }
    #[test]
    fn v13_preserves_every_frozen_v12_field_and_the_complete_original_packet() {
        let fixture = fixture();
        let decoded = unpack(&fixture["encoded"]).unwrap();
        assert_eq!(canonical(&decoded), canonical(&fixture["input"]));
        let before = crate::source_reflows_merge::main_e177_final_inputs::packet_before_overlay();
        let old = compose_values(before.clone(), &fixture["policy"], &fixture["input"]).unwrap();
        let current = compose_values(before, &fixture["policy"], &fixture["encoded"]).unwrap();
        assert_eq!(canonical(&current), canonical(&old));
        assert!(canonical(&fixture["encoded"]).len() < canonical(&fixture["input"]).len());
        assert_eq!(PAGE_CAP, 262144);
        assert_eq!(SOURCE_CAP, 9753094);
        for pair in fixture["vectors"].as_array().unwrap() {
            assert_eq!(
                decode(pair[1].as_str().unwrap()).unwrap(),
                pair[0].as_str().unwrap()
            );
        }
    }
    #[test]
    fn v13_refuses_malformed_text_tables_indices_and_oversized_wire() {
        for bad in [
            "A".repeat(42),
            "A".repeat(44),
            format!("{}B", "A".repeat(42)),
            format!("{}=", "A".repeat(42)),
            format!("{}!", "A".repeat(42)),
        ] {
            assert!(decode(&bad).is_err());
        }
        let fixture = fixture();
        for mode in 0..8 {
            let mut bad = fixture["encoded"].clone();
            match mode {
                0 => bad["unknown"] = true.into(),
                1 => bad["schema"] = "foreign".into(),
                2 => bad["digests"][0] = Value::Null,
                3 => {
                    let first = bad["digests"][0].clone();
                    bad["digests"].as_array_mut().unwrap().insert(1, first);
                }
                4 => bad["digests"].as_array_mut().unwrap().reverse(),
                5 => bad["names"]
                    .as_array_mut()
                    .unwrap()
                    .push("zzzz-unused".into()),
                6 => bad["page"]["opaque"] = "x".repeat(PAGE_CAP).into(),
                7 => {
                    bad["page"]["replacements"][0]["old519_index"][1] =
                        serde_json::json!([0, u64::MAX])
                }
                _ => unreachable!(),
            }
            let result =
                unpack(&bad).and_then(|decoded| super::super::expand_v12_contents(&decoded));
            assert!(result.is_err(), "mode={mode}");
        }
    }
    #[test]
    fn v13_keeps_opaque_values_literal_and_both_original_materialization_bounds() {
        let fixture = fixture();
        let opaque = serde_json::json!({"sha256":"A".repeat(43),"current":[0,"A".repeat(43)]});
        let mut encoded = fixture["encoded"].clone();
        encoded["page"]["opaque"] = opaque.clone();
        assert_eq!(unpack(&encoded).unwrap()["page"]["opaque"], opaque);
        let zero = "A".repeat(43);
        let mut wire = serde_json::json!({"schema":super::super::SCHEMA_V13,"names":[],"digests":[],
            "page":{"schema":super::super::SCHEMA_V10,"replacements":[],"additions":[{
                "current":[0,zero],"views":vec![serde_json::json!(["n",0,"A".repeat(43)]);4500]}]}});
        assert!(canonical(&wire).len() < PAGE_CAP);
        let decoded = unpack(&wire).unwrap();
        assert_eq!(
            super::super::expand_v12_contents(&decoded).unwrap_err(),
            "v12 materialized digest cap"
        );
        wire["names"] = serde_json::json!(["n".repeat(64)]);
        wire["page"]["additions"][0]["current"] = serde_json::json!([0]);
        wire["page"]["additions"][0]["views"] =
            serde_json::json!(vec![serde_json::json!([0, 0]); 4500]);
        assert!(canonical(&wire).len() < PAGE_CAP);
        let decoded = unpack(&wire).unwrap();
        assert_eq!(
            super::super::expand_v12_contents(&decoded).unwrap_err(),
            "v12 materialized name cap"
        );
    }
}
