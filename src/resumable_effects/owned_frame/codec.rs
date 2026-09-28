//! Bounded inert facts only. No decoder restores a language owner.
use super::{CheckedOwnedFramePlan, OwnedFrameError as Error};
use crate::cleanup_plan::{FinalizeAction, StorageId};
use crate::hir::DeclarationId;
use crate::interpreter::resumable::owned_frame::{
    snapshot, OwnedFrameInput, OwnedFrameInputField, OwnedFrameInputValue,
};
use crate::interpreter::resumable::{checkpoint::scalar_from_json, checkpoint::scalar_json};
use crate::interpreter::ArgumentValue;
use crate::resumable_effects::source_checkpoint::SourceCheckpointScope;
use serde::de::{DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fmt;

pub(super) const MAX_CARRIER: usize = 32768;
pub(super) const MAX_CHECKPOINT: usize = 65536;
pub(super) const MAX_RECORD: usize = 163840;
pub(super) const MAX_JOURNAL: usize = 524288;
pub(super) const MAX_RECORDS: usize = 64;

pub(super) fn canonical(value: &Value) -> Vec<u8> {
    fn write(value: &Value, out: &mut String) {
        match value {
            Value::Object(map) => {
                out.push('{');
                let mut keys: Vec<_> = map.keys().collect();
                keys.sort();
                for (index, key) in keys.into_iter().enumerate() {
                    if index != 0 {
                        out.push(',');
                    }
                    out.push_str(&serde_json::to_string(key).expect("string encoding"));
                    out.push(':');
                    write(&map[key], out);
                }
                out.push('}');
            }
            Value::Array(values) => {
                out.push('[');
                for (index, value) in values.iter().enumerate() {
                    if index != 0 {
                        out.push(',');
                    }
                    write(value, out);
                }
                out.push(']');
            }
            _ => out.push_str(&value.to_string()),
        }
    }
    let mut text = String::new();
    write(value, &mut text);
    text.into_bytes()
}
pub(super) fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(DIGITS[(byte >> 4) as usize] as char);
        text.push(DIGITS[(byte & 15) as usize] as char);
    }
    text
}
pub(super) fn unhex(text: &str, max: usize) -> Result<Vec<u8>, Error> {
    if text.len() % 2 != 0
        || text.len() > max * 2
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(Error::Malformed);
    }
    Ok(text
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let digit = |b: u8| if b <= b'9' { b - b'0' } else { b - b'a' + 10 };
            digit(pair[0]) * 16 + digit(pair[1])
        })
        .collect())
}
pub(super) fn digest(domain: &'static [u8], bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(bytes);
    format!("sha256:{}", hex(&hash.finalize()))
}
pub(super) fn fact_digest(domain: &'static [u8], value: &Value) -> String {
    digest(domain, &canonical(value))
}
pub(super) fn keys(value: &Value, expected: &[&str]) -> Result<(), Error> {
    let map = value.as_object().ok_or(Error::Malformed)?;
    if map.len() != expected.len() || expected.iter().any(|key| !map.contains_key(*key)) {
        Err(Error::Malformed)
    } else {
        Ok(())
    }
}
pub(super) fn text<'a>(value: &'a Value, max: usize) -> Result<&'a str, Error> {
    let text = value.as_str().ok_or(Error::Malformed)?;
    if text.is_empty() || text.len() > max {
        Err(Error::Capacity)
    } else {
        Ok(text)
    }
}
pub(super) fn scalar(value: &ArgumentValue) -> Result<Value, Error> {
    if matches!(
        value,
        ArgumentValue::BorrowedStr(_) | ArgumentValue::BorrowedSlice(_)
    ) {
        return Err(Error::Binding);
    }
    if matches!(value, ArgumentValue::Usize(v) if *v > u32::MAX as u64)
        || matches!(value, ArgumentValue::Char(v) if char::from_u32(*v).is_none())
    {
        return Err(Error::Binding);
    }
    Ok(scalar_json(value))
}
pub(super) fn decode_scalar(value: &Value) -> Result<ArgumentValue, Error> {
    let scalar = scalar_from_json(value).map_err(|_| Error::Malformed)?;
    if canonical(&self::scalar(&scalar)?) != canonical(value) {
        return Err(Error::Malformed);
    }
    Ok(scalar)
}
pub(super) fn scope(scope: &SourceCheckpointScope) -> Result<Value, Error> {
    if scope.invocation_id().is_empty()
        || scope.invocation_id().len() > 128
        || scope.program_root().is_empty()
        || scope.program_root().len() > 256
    {
        return Err(Error::Capacity);
    }
    Ok(
        json!({"invocation":scope.invocation_id(),"policy_epoch":scope.policy_epoch(),"program_root":scope.program_root()}),
    )
}
pub(super) fn input(plan: &CheckedOwnedFramePlan, input: &OwnedFrameInput) -> Result<Value, Error> {
    snapshot::validate_input(plan, input).map_err(|_| Error::Binding)?;
    let fields: Result<Vec<_>, Error> = input
        .fields
        .iter()
        .map(|field| {
            Ok(
                json!({"identity":field.identity.as_str(),"value":match &field.value {
                    OwnedFrameInputValue::Bytes(bytes) => json!({"kind":"bytes","hex":hex(bytes)}),
                    OwnedFrameInputValue::Scalar(value) => scalar(value)?,
                }}),
            )
        })
        .collect();
    let value = json!({"declaration":input.declaration.as_str(),"fields":fields?});
    if canonical(&value).len() > MAX_CARRIER {
        return Err(Error::Capacity);
    }
    Ok(value)
}
pub(super) fn decode_input(
    plan: &CheckedOwnedFramePlan,
    value: &Value,
) -> Result<OwnedFrameInput, Error> {
    keys(value, &["declaration", "fields"])?;
    if canonical(value).len() > MAX_CARRIER {
        return Err(Error::Capacity);
    }
    let fields = value["fields"].as_array().ok_or(Error::Malformed)?;
    if fields.len() > 8 {
        return Err(Error::Capacity);
    }
    let fields: Result<Vec<_>, Error> = fields
        .iter()
        .map(|field| {
            keys(field, &["identity", "value"])?;
            let payload = &field["value"];
            let value = if payload.get("kind").is_some() {
                keys(payload, &["kind", "hex"])?;
                if payload["kind"] != "bytes" {
                    return Err(Error::Malformed);
                }
                OwnedFrameInputValue::Bytes(unhex(
                    payload["hex"].as_str().ok_or(Error::Malformed)?,
                    1024,
                )?)
            } else {
                OwnedFrameInputValue::Scalar(decode_scalar(payload)?)
            };
            Ok(OwnedFrameInputField {
                identity: DeclarationId::new(text(&field["identity"], 256)?),
                value,
            })
        })
        .collect();
    let input = OwnedFrameInput {
        declaration: DeclarationId::new(text(&value["declaration"], 256)?),
        fields: fields?,
    };
    snapshot::validate_input(plan, &input).map_err(|_| Error::Binding)?;
    Ok(input)
}
pub(super) fn storage(storage: &StorageId) -> Result<Value, Error> {
    Ok(match storage {
        StorageId::Value(id) => json!({"kind":"value","value":id.as_str()}),
        StorageId::ProvisionalResult => json!({"kind":"provisional_result"}),
        _ => return Err(Error::Binding),
    })
}
pub(super) fn operations(actions: &[FinalizeAction]) -> Result<Value, Error> {
    let operations: Result<Vec<_>,Error> = actions.iter().map(|action| {
        if action.active_case.is_some() || action.source.projections.len()!=1 { return Err(Error::Binding); }
        Ok(json!({"kind":"finalize","source":{"kind":"cleanup_place","storage":storage(&action.source.storage)?,"projections":[action.source.projections[0].as_str()]},"lifecycle_id":action.lifecycle_id.as_str(),"guard_flag":action.guard_flag.0}))
    }).collect();
    Ok(json!(operations?))
}
pub(super) fn leaf_flags(plan: &CheckedOwnedFramePlan) -> Value {
    json!(plan.liveness().leaves.iter().map(|leaf| json!({"field":leaf.field.as_str(),"flag":leaf.flag.0,"live":true,"lifecycle":leaf.lifecycle.as_str()})).collect::<Vec<_>>())
}
pub(super) fn frame(plan: &CheckedOwnedFramePlan, input: &OwnedFrameInput) -> Result<Value, Error> {
    let mut value = self::input(plan, input)?;
    let map = value.as_object_mut().expect("typed input object");
    map.insert("storage".into(), storage(&plan.liveness().storage)?);
    map.insert("leaf_flags".into(), leaf_flags(plan));
    map.insert(
        "suspension_cleanup".into(),
        operations(&plan.liveness().suspension_cleanup)?,
    );
    map.insert(
        "failure_cleanup".into(),
        operations(&plan.liveness().failure_cleanup)?,
    );
    map.insert(
        "completion_cleanup".into(),
        operations(&plan.liveness().completion_cleanup)?,
    );
    if canonical(&value).len() > MAX_CARRIER {
        return Err(Error::Capacity);
    }
    Ok(value)
}
pub(super) fn decode_frame(
    plan: &CheckedOwnedFramePlan,
    value: &Value,
) -> Result<OwnedFrameInput, Error> {
    keys(
        value,
        &[
            "declaration",
            "fields",
            "storage",
            "leaf_flags",
            "suspension_cleanup",
            "failure_cleanup",
            "completion_cleanup",
        ],
    )?;
    let input = decode_input(
        plan,
        &json!({"declaration":value["declaration"],"fields":value["fields"]}),
    )?;
    if frame(plan, &input)? != *value {
        return Err(Error::Binding);
    }
    Ok(input)
}
pub(super) fn signature(plan: &CheckedOwnedFramePlan) -> Value {
    let yields = plan
        .function()
        .yields
        .as_ref()
        .expect("sealed checked yields");
    json!({"request_shape":format!("semaprax.resolved-type.v1:{}",yields.request_type.identity_key()),"answer_shape":format!("semaprax.resolved-type.v1:{}",yields.response_type.identity_key()),"plan_identity":plan.binding(),"yield_count":1})
}

struct Strict(usize);
impl<'de> DeserializeSeed<'de> for Strict {
    type Value = Value;
    fn deserialize<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        if self.0 > 24 {
            return Err(serde::de::Error::custom("depth"));
        }
        deserializer.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for Strict {
    type Value = Value;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("bounded unique-key JSON")
    }
    fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<Value, E> {
        Ok(json!(v))
    }
    fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Value, E> {
        Ok(json!(v))
    }
    fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Value, E> {
        Ok(json!(v))
    }
    fn visit_f64<E: serde::de::Error>(self, _: f64) -> Result<Value, E> {
        Err(E::custom("nonintegral number"))
    }
    fn visit_none<E: serde::de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_unit<E: serde::de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Value, E> {
        if v.len() > MAX_CHECKPOINT {
            Err(E::custom("string bound"))
        } else {
            Ok(json!(v))
        }
    }
    fn visit_string<E: serde::de::Error>(self, v: String) -> Result<Value, E> {
        self.visit_str(&v)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        let mut values = Vec::new();
        while let Some(v) = seq.next_element_seed(Strict(self.0 + 1))? {
            if values.len() == 16 {
                return Err(serde::de::Error::custom("array bound"));
            }
            values.push(v);
        }
        Ok(Value::Array(values))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut values = serde_json::Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if key.len() > 256 || values.len() == 32 || values.contains_key(&key) {
                return Err(serde::de::Error::custom("key bound/duplicate"));
            }
            values.insert(key, map.next_value_seed(Strict(self.0 + 1))?);
        }
        Ok(Value::Object(values))
    }
}
pub(super) fn parse(bytes: &[u8], maximum: usize) -> Result<Value, Error> {
    if bytes.len() > maximum {
        return Err(Error::Capacity);
    }
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    let value = Strict(0)
        .deserialize(&mut decoder)
        .map_err(|_| Error::Malformed)?;
    decoder.end().map_err(|_| Error::Malformed)?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn owned_frame_strict_json_rejects_nested_duplicate_unknown_float_and_depth() {
        assert_eq!(
            parse(br#"{"frame":{"flag":true,"flag":false}}"#, MAX_RECORD),
            Err(Error::Malformed)
        );
        assert_eq!(parse(b"1.5", MAX_RECORD), Err(Error::Malformed));
        let nested = format!("{}null{}", "[".repeat(25), "]".repeat(25));
        assert_eq!(parse(nested.as_bytes(), MAX_RECORD), Err(Error::Malformed));
        assert_eq!(
            parse(&vec![b' '; MAX_RECORD + 1], MAX_RECORD),
            Err(Error::Capacity)
        );
        assert_eq!(
            canonical(&json!({"z":{"b":2,"a":1},"a":[3,2]})),
            br#"{"a":[3,2],"z":{"a":1,"b":2}}"#
        );
        assert_eq!(
            keys(&json!({"unknown":1}), &["expected"]),
            Err(Error::Malformed)
        );
    }
}

pub(super) fn is_digest(text: &str) -> bool {
    text.len() == 71
        && text.starts_with("sha256:")
        && text.as_bytes()[7..]
            .iter()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b))
}
