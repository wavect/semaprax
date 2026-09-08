//! Closed flat retained values; no recursive or ambient carrier admission.
use super::*;
use crate::hir::DeclarationId;
use crate::interpreter::retained_call::{RetainedField, RetainedRecord};

pub(super) fn identifier(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 240
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}
pub(super) fn encode(value: &RetainedValue) -> Result<Value, Diagnostic> {
    Ok(match value {
        RetainedValue::Bool(v) => json!({"kind":"bool","value":v}),
        RetainedValue::I32(v) => json!({"kind":"i32","value":v.to_string()}),
        RetainedValue::I64(v) => json!({"kind":"i64","value":v.to_string()}),
        RetainedValue::U8(v) => json!({"kind":"u8","value":v.to_string()}),
        RetainedValue::Usize(v) => json!({"kind":"usize","value":v.to_string()}),
        RetainedValue::Bytes(v) => {
            if v.len() > 65536 {
                return Err(rejected("value.bytes"));
            }
            let hex: String = v.iter().map(|b| format!("{b:02x}")).collect();
            json!({"kind":"bytes","value":hex})
        }
        RetainedValue::Record(record) => {
            if !identifier(record.record.as_str())
                || record.fields.is_empty()
                || record.fields.len() > 32
            {
                return Err(rejected("value.record"));
            }
            let mut seen = std::collections::BTreeSet::new();
            let mut fields = Vec::new();
            for field in &record.fields {
                if !identifier(field.field.as_str())
                    || !seen.insert(&field.field)
                    || matches!(
                        field.value,
                        RetainedValue::Record(_) | RetainedValue::Variant(_)
                    )
                {
                    return Err(rejected("value.record.field"));
                }
                fields.push(json!({"field":field.field.as_str(),"value":encode(&field.value)?}));
            }
            json!({"kind":"record","record":record.record.as_str(),"fields":fields})
        }
        _ => return Err(rejected("value.kind")),
    })
}
pub(super) fn decode(v: &Value) -> Result<RetainedValue, Diagnostic> {
    let kind = codec::text(v, "kind")?;
    if kind == "record" {
        codec::keys(v, &["kind", "record", "fields"])?;
        let record = codec::text(v, "record")?;
        let fields = v["fields"]
            .as_array()
            .ok_or_else(|| rejected("value.fields"))?;
        if fields.len() > 32 {
            return Err(rejected("value.fields"));
        }
        let fields: Result<Vec<_>, Diagnostic> = fields
            .iter()
            .map(|field| {
                codec::keys(field, &["field", "value"])?;
                if field["value"]["kind"] == "record" {
                    return Err(rejected("value.depth"));
                }
                Ok(RetainedField {
                    field: DeclarationId::new(codec::text(field, "field")?),
                    value: decode(&field["value"])?,
                })
            })
            .collect();
        let value = RetainedValue::Record(RetainedRecord {
            record: DeclarationId::new(record),
            fields: fields?,
        });
        encode(&value)?;
        return Ok(value);
    }
    codec::keys(v, &["kind", "value"])?;
    let integer = || v["value"].as_str().ok_or_else(|| rejected("value.integer"));
    let value = match kind {
        "bool" => RetainedValue::Bool(v["value"].as_bool().ok_or_else(|| rejected("value.bool"))?),
        "i32" => RetainedValue::I32(integer()?.parse().map_err(|_| rejected("value.i32"))?),
        "i64" => RetainedValue::I64(integer()?.parse().map_err(|_| rejected("value.i64"))?),
        "u8" => RetainedValue::U8(integer()?.parse().map_err(|_| rejected("value.u8"))?),
        "usize" => RetainedValue::Usize(integer()?.parse().map_err(|_| rejected("value.usize"))?),
        "bytes" => {
            let hex = integer()?;
            if hex.len() > 131072
                || hex.len() % 2 != 0
                || !hex
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err(rejected("value.hex"));
            }
            let mut bytes = Vec::with_capacity(hex.len() / 2);
            for pair in hex.as_bytes().as_chunks::<2>().0 {
                let digit = |b: u8| if b <= b'9' { b - b'0' } else { b - b'a' + 10 };
                bytes.push(digit(pair[0]) * 16 + digit(pair[1]));
            }
            RetainedValue::Bytes(bytes)
        }
        _ => return Err(rejected("value.kind")),
    };
    if encode(&value)? != *v {
        return Err(rejected("value.canonical"));
    }
    Ok(value)
}
pub(super) fn fields(values: &[(String, RetainedValue)]) -> Result<Value, Diagnostic> {
    if values.len() > 8 {
        return Err(rejected("effect.fields"));
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut result = Vec::new();
    for (id, value) in values {
        if !identifier(id)
            || !seen.insert(id)
            || matches!(
                value,
                RetainedValue::Bytes(_) | RetainedValue::Record(_) | RetainedValue::Variant(_)
            )
        {
            return Err(rejected("effect.field"));
        }
        result.push(json!({"field":id,"value":encode(value)?}));
    }
    Ok(Value::Array(result))
}
pub(super) fn decode_fields(v: &Value) -> Result<Vec<(String, RetainedValue)>, Diagnostic> {
    let entries = v.as_array().ok_or_else(|| rejected("effect.fields"))?;
    if entries.len() > 8 {
        return Err(rejected("effect.fields"));
    }
    let result: Result<Vec<_>, Diagnostic> = entries
        .iter()
        .map(|row| {
            codec::keys(row, &["field", "value"])?;
            Ok((
                codec::text(row, "field")?.to_owned(),
                decode(&row["value"])?,
            ))
        })
        .collect();
    let result = result?;
    fields(&result)?;
    Ok(result)
}

/// Exact bytes of the existing typed-effect transport, independently measured.
pub(super) fn transport_bytes(values: &[(String, RetainedValue)]) -> Result<u64, Diagnostic> {
    fields(values)?;
    let mut bytes =
        b"{\"schema\":\"semaprax.agent-effect-fields.v1\",\"fields\":[]}\n".len() as u64;
    for (index, (id, value)) in values.iter().enumerate() {
        bytes += id.len() as u64 + 5 + crate::agent_lifecycle::encode_value(value).len() as u64;
        bytes += u64::from(index != 0);
    }
    Ok(bytes)
}
