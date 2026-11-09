//! V11 changes only positional digest transport; V10 remains the exact inverse.
use super::{expand_v10, SCHEMA_V10};
use serde_json::Value;

pub(in super::super) const SCHEMA_V11: &str = "semaprax.finite-source-overlay-page.v11";
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn encoded_digest(raw: &[u8; 32]) -> String {
    let mut result = String::with_capacity(43);
    let (mut accumulator, mut bits) = (0u32, 0u32);
    for byte in raw {
        accumulator = (accumulator << 8) | u32::from(*byte);
        bits += 8;
        while bits >= 6 {
            bits -= 6;
            result.push(char::from(ALPHABET[((accumulator >> bits) & 63) as usize]));
        }
        accumulator &= (1 << bits) - 1;
    }
    if bits != 0 {
        result.push(char::from(ALPHABET[(accumulator << (6 - bits)) as usize]));
    }
    result
}

fn decoded_digest(value: &Value) -> Result<String, &'static str> {
    let encoded = value.as_str().ok_or("v11 digest type")?;
    if encoded.len() != 43 {
        return Err("v11 digest length");
    }
    let mut raw = [0u8; 32];
    let (mut accumulator, mut bits, mut count) = (0u32, 0u32, 0usize);
    for byte in encoded.bytes() {
        let digit = ALPHABET
            .iter()
            .position(|candidate| *candidate == byte)
            .ok_or("v11 digest alphabet")?;
        accumulator = (accumulator << 6) | digit as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            if count >= raw.len() {
                return Err("v11 digest bytes");
            }
            raw[count] = (accumulator >> bits) as u8;
            count += 1;
        }
        accumulator &= (1 << bits) - 1;
    }
    if count != 32 || encoded_digest(&raw) != encoded {
        return Err("v11 canonical digest");
    }
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(64);
    for byte in raw {
        result.push(char::from(HEX[usize::from(byte >> 4)]));
        result.push(char::from(HEX[usize::from(byte & 15)]));
    }
    Ok(result)
}

fn pin(value: &mut Value, named: bool) -> Result<(), &'static str> {
    let Some(tuple) = value.as_array_mut() else {
        // Object pins, including objects carrying extra fields, stay literal.
        return Ok(());
    };
    let count = if named { 3 } else { 2 };
    let offset = usize::from(named);
    if tuple.len() != count && tuple.len() != count + 1 {
        return Err("v11 pin arity");
    }
    if named && !tuple[0].is_string() {
        return Err("v11 view name");
    }
    let length = tuple[offset].as_u64().ok_or("v11 length")?;
    if length > super::super::SOURCE_CAP as u64 {
        return Err("v11 length");
    }
    tuple[offset + 1] = decoded_digest(&tuple[offset + 1])?.into();
    Ok(())
}

fn views(value: &mut Value) -> Result<(), &'static str> {
    for value in value.as_array_mut().ok_or("v11 views")? {
        pin(value, true)?;
    }
    Ok(())
}

fn row(value: &mut Value) -> Result<(), &'static str> {
    let object = value.as_object_mut().ok_or("v11 literal row")?;
    if let Some(current) = object.get_mut("current") {
        pin(current, false)?;
    }
    if let Some(value) = object.get_mut("views") {
        views(value)?;
    }
    Ok(())
}

fn fragments(value: &mut Value) -> Result<(), &'static str> {
    if value.is_null() {
        return Ok(());
    }
    for value in value.as_array_mut().ok_or("v11 fragments")? {
        if !value.is_object() {
            return Err("v11 fragment row");
        }
        if value.get(super::REFERENCE).is_none() {
            row(value)?;
        }
    }
    Ok(())
}

fn expand_v11_to_v10(page: &Value) -> Result<Value, &'static str> {
    if page["schema"] != SCHEMA_V11 {
        return Err("v11 schema");
    }
    let mut result = page.clone();
    for group in ["replacements", "additions"] {
        for value in result[group].as_array_mut().ok_or("v11 rows")? {
            if !value.is_object() {
                return Err("v11 row");
            }
            if value.get("old519_index").is_some() {
                if group != "replacements" || value.as_object().unwrap().len() != 1 {
                    return Err("v11 indexed row");
                }
                let tuple = value["old519_index"].as_array_mut().ok_or("v11 old519 arity")?;
                if tuple.len() != 4 {
                    return Err("v11 old519 arity");
                }
                pin(&mut tuple[1], false)?;
                views(&mut tuple[2])?;
                fragments(&mut tuple[3])?;
            } else {
                row(value)?;
                if let Some(value) = value.get_mut("fragments") {
                    fragments(value)?;
                }
            }
        }
    }
    result["schema"] = SCHEMA_V10.into();
    Ok(result)
}

pub(in super::super) fn expand_v11(
    before: &Value,
    policy: &Value,
    page: &Value,
) -> Result<Value, &'static str> {
    expand_v10(before, policy, &expand_v11_to_v10(page)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn encode_pin(value: &mut Value, named: bool) {
        let Some(tuple) = value.as_array_mut() else { return; };
        let index = if named { 2 } else { 1 };
        let sha = tuple[index].as_str().unwrap();
        let raw = std::array::from_fn(|byte| u8::from_str_radix(&sha[byte * 2..byte * 2 + 2], 16).unwrap());
        tuple[index] = encoded_digest(&raw).into();
    }
    fn encode_row(value: &mut Value) {
        if let Some(current) = value.get_mut("current") { encode_pin(current, false); }
        if let Some(views) = value.get_mut("views") {
            for view in views.as_array_mut().unwrap() { encode_pin(view, true); }
        }
    }
    fn encode_fragments(value: &mut Value) {
        if value.is_null() { return; }
        for fragment in value.as_array_mut().unwrap() {
            if fragment.get(super::super::REFERENCE).is_none() { encode_row(fragment); }
        }
    }
    #[test]
    fn v11_complete_frozen_v10_inverse_preserves_every_value_and_canonical_byte() {
        const V10: &str = include_str!("../../docs/evidence/finite-source-overlay-v2-20261018/v10-unqualified-full-union.json");
        let full: Value = serde_json::from_str(V10).unwrap();
        let mut compact = full.clone();
        for group in ["replacements", "additions"] {
            for row in compact[group].as_array_mut().unwrap() {
                if let Some(reference) = row.get_mut("old519_index") {
                    encode_pin(&mut reference[1], false);
                    for view in reference[2].as_array_mut().unwrap() { encode_pin(view, true); }
                    encode_fragments(&mut reference[3]);
                } else {
                    encode_row(row);
                    if let Some(fragments) = row.get_mut("fragments") { encode_fragments(fragments); }
                }
            }
        }
        compact["schema"] = SCHEMA_V11.into();
        let restored = expand_v11_to_v10(&compact).unwrap();
        assert_eq!(restored, full);
        assert_eq!(super::super::canonical(&restored).as_slice(), V10.as_bytes());
        assert!(super::super::canonical(&compact).len() < V10.len());
    }

    #[test]
    fn v11_digest_transport_restores_every_byte_and_rejects_noncanonical_inputs() {
        assert_eq!(encoded_digest(&std::array::from_fn(|index| index as u8)),
            "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8");
        for seed in 0..=255u8 {
            let raw = std::array::from_fn(|index| seed.wrapping_add(index as u8));
            let encoded = encoded_digest(&raw);
            let expected: String = raw.iter().map(|byte| format!("{byte:02x}")).collect();
            assert_eq!(decoded_digest(&json!(encoded)).unwrap(), expected);
        }
        for bad in [json!(true), json!(null), json!(17), json!([]), json!({}),
            json!("A".repeat(42)), json!("A".repeat(44)), json!("=".repeat(43)),
            json!("/".repeat(43)), json!("+".repeat(43)),
            json!(format!("{}B", "A".repeat(42))), json!("é".repeat(43))] {
            assert!(decoded_digest(&bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn v11_slots_restore_whole_v10_and_keep_unknown_objects_and_references() {
        let sha = "00".repeat(32);
        let compact = encoded_digest(&[0; 32]);
        let literal = json!({"length":true,"sha256":sha,"archive":"e/literal","unknown":[compact]});
        let mut full = json!({"schema":SCHEMA_V10,"archive_prefixes":["unchanged/"],
            "unknown":[compact,sha],"replacements":[{"old519_index":[0,[1,sha],
            [["old",2,sha,[0,"kept"]]],[{"current":[3,sha],"views":[]},
            {"canonical_source":"untouched","current":[false,"literal"]}]]}],
            "additions":[{"current":literal,"views":[["named",4,sha,"literal"]],"fragments":null}]});
        let mut encoded = full.clone();
        encoded["schema"] = SCHEMA_V11.into();
        encoded["replacements"][0]["old519_index"][1][1] = compact.clone().into();
        encoded["replacements"][0]["old519_index"][2][0][2] = compact.clone().into();
        encoded["replacements"][0]["old519_index"][3][0]["current"][1] = compact.clone().into();
        encoded["additions"][0]["views"][0][2] = compact.into();
        assert_eq!(expand_v11_to_v10(&encoded).unwrap(), full);
        for bad in [json!([]),json!([1]),json!([1,"A",0,0]),json!([true,"A".repeat(43)]),
            json!([-1,"A".repeat(43)]),json!([9753095,"A".repeat(43)]),
            json!([1,false]),json!([1,"A".repeat(64)])] {
            let mut hostile=encoded.clone();
            hostile["replacements"][0]["old519_index"][1]=bad;
            assert!(expand_v11_to_v10(&hostile).is_err());
        }
        full["schema"] = "semaprax.finite-source-overlay-page.v110".into();
        assert!(expand_v11_to_v10(&full).is_err());
    }
}
