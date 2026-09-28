//! Canonical authenticated JSONL rows, with bounded duplicate-aware decoding.
use super::*;
use crate::live_invocation::identity::{hex, looks_like_digest, unhex};
use serde::de::{DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::Map;
use std::fmt;

pub(super) fn canonical(value: &Value) -> Vec<u8> {
    fn ordered(value: &Value) -> Value {
        match value {
            Value::Object(fields) => {
                let mut keys = fields.keys().collect::<Vec<_>>();
                keys.sort_unstable();
                let mut out = Map::new();
                for key in keys {
                    out.insert(key.clone(), ordered(&fields[key]));
                }
                Value::Object(out)
            }
            Value::Array(values) => Value::Array(values.iter().map(ordered).collect()),
            _ => value.clone(),
        }
    }
    serde_json::to_vec(&ordered(value)).expect("JSON values serialize")
}

struct StrictValue(usize);
impl<'de> DeserializeSeed<'de> for StrictValue {
    type Value = Value;
    fn deserialize<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        if self.0 > MAX_DEPTH {
            return Err(serde::de::Error::custom("v8 depth limit"));
        }
        deserializer.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for StrictValue {
    type Value = Value;
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("bounded integer JSON")
    }
    fn visit_bool<E: serde::de::Error>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }
    fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<Value, E> {
        Ok(value.into())
    }
    fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Value, E> {
        Ok(value.into())
    }
    fn visit_f64<E: serde::de::Error>(self, _: f64) -> Result<Value, E> {
        Err(E::custom("floating JSON refused"))
    }
    fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Value, E> {
        Ok(value.into())
    }
    fn visit_string<E: serde::de::Error>(self, value: String) -> Result<Value, E> {
        Ok(value.into())
    }
    fn visit_unit<E: serde::de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_none<E: serde::de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut access: A) -> Result<Value, A::Error> {
        let mut values = Vec::new();
        while let Some(value) = access.next_element_seed(StrictValue(self.0 + 1))? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Value, A::Error> {
        let mut values = Map::new();
        while let Some(key) = access.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(serde::de::Error::custom("duplicate v8 key"));
            }
            values.insert(key, access.next_value_seed(StrictValue(self.0 + 1))?);
        }
        Ok(Value::Object(values))
    }
}

pub(super) fn parse(bytes: &[u8]) -> Result<Value, SourceJournalError> {
    if bytes.len() > super::super::MAX_SOURCE_DOCUMENT_BYTES {
        return Err(SourceJournalError::Capacity);
    }
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    let value = StrictValue(0)
        .deserialize(&mut decoder)
        .map_err(|_| SourceJournalError::Malformed)?;
    decoder.end().map_err(|_| SourceJournalError::Malformed)?;
    Ok(value)
}

fn tag_valid(tag: &str) -> bool {
    tag.len() == 64
        && tag
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn expected_valid(expected: &ExpectedRowV8<'_>) -> bool {
    looks_like_digest(expected.invocation)
        && looks_like_digest(expected.generation)
        && tag_valid(expected.prev_mac)
}

fn body(entry: &EntryV8, seq: u32) -> Result<Value, SourceJournalError> {
    match entry {
        EntryV8::Ordinary(entry) => {
            parse(super::super::wire::encode_entry(entry, seq as usize).as_bytes())
        }
        EntryV8::Owned(entry) => {
            serde_json::to_value(entry).map_err(|_| SourceJournalError::Malformed)
        }
    }
}

pub(super) fn encode(
    entry: &EntryV8,
    expected: &ExpectedRowV8<'_>,
    key: &SourceCheckpointKey,
) -> Result<Vec<u8>, SourceJournalError> {
    if !expected_valid(expected) {
        return Err(SourceJournalError::Binding);
    }
    let mut row = body(entry, expected.seq)?
        .as_object()
        .cloned()
        .ok_or(SourceJournalError::Malformed)?;
    row.insert("seq".into(), expected.seq.into());
    row.insert("schema".into(), SCHEMA.into());
    row.insert("invocation".into(), expected.invocation.into());
    row.insert("generation".into(), expected.generation.into());
    row.insert("prev_mac".into(), expected.prev_mac.into());
    let payload = canonical(&Value::Object(row.clone()));
    row.insert(
        "authentication".into(),
        hex(&key.authenticate(RECORD_DOMAIN, &payload)).into(),
    );
    let mut bytes = canonical(&Value::Object(row));
    bytes.push(b'\n');
    if bytes.len() > super::super::MAX_SOURCE_DOCUMENT_BYTES {
        return Err(SourceJournalError::Capacity);
    }
    // Round-trip enforces the same structural caps for trusted typed producers.
    decode(&bytes, expected, key)?;
    Ok(bytes)
}

pub(super) fn decode(
    bytes: &[u8],
    expected: &ExpectedRowV8<'_>,
    key: &SourceCheckpointKey,
) -> Result<EntryV8, SourceJournalError> {
    if bytes.len() > super::super::MAX_SOURCE_DOCUMENT_BYTES {
        return Err(SourceJournalError::Capacity);
    }
    if !expected_valid(expected) {
        return Err(SourceJournalError::Binding);
    }
    if !bytes.ends_with(b"\n") {
        return Err(SourceJournalError::Malformed);
    }
    let value = parse(&bytes[..bytes.len() - 1])?;
    let mut canonical_row = canonical(&value);
    canonical_row.push(b'\n');
    if canonical_row != bytes {
        return Err(SourceJournalError::Malformed);
    }
    let mut row = value
        .as_object()
        .cloned()
        .ok_or(SourceJournalError::Malformed)?;
    for (field, expected_value) in [
        ("schema", SCHEMA),
        ("invocation", expected.invocation),
        ("generation", expected.generation),
        ("prev_mac", expected.prev_mac),
    ] {
        if row.get(field).and_then(Value::as_str) != Some(expected_value) {
            return Err(SourceJournalError::Binding);
        }
    }
    if row.get("seq").and_then(Value::as_u64) != Some(expected.seq as u64) {
        return Err(SourceJournalError::Order);
    }
    let tag = row
        .remove("authentication")
        .and_then(|v| v.as_str().map(str::to_owned))
        .ok_or(SourceJournalError::Malformed)?;
    if !tag_valid(&tag) {
        return Err(SourceJournalError::Malformed);
    }
    if !key.verify(
        RECORD_DOMAIN,
        &canonical(&Value::Object(row.clone())),
        &unhex(&tag).ok_or(SourceJournalError::Malformed)?,
    ) {
        return Err(SourceJournalError::Chain);
    }
    for field in ["schema", "invocation", "generation", "prev_mac"] {
        row.remove(field);
    }
    let owned = row
        .get("kind")
        .and_then(Value::as_str)
        .is_some_and(|kind| kind.starts_with("owned_"));
    let entry = if owned {
        row.remove("seq");
        let body: model::OwnedBodyV8 = serde_json::from_value(Value::Object(row.clone()))
            .map_err(|_| SourceJournalError::Malformed)?;
        let canonical_body =
            serde_json::to_value(&body).map_err(|_| SourceJournalError::Malformed)?;
        if canonical_body != Value::Object(row.clone()) {
            return Err(SourceJournalError::Malformed);
        }
        validate_owned(&canonical_body)?;
        EntryV8::Owned(body)
    } else {
        EntryV8::Ordinary(super::super::wire::decode_entry(
            &Value::Object(row),
            expected.seq as usize,
            expected.ordinary,
        )?)
    };
    Ok(entry)
}

fn validate_owned(value: &Value) -> Result<(), SourceJournalError> {
    let fields = value.as_object().ok_or(SourceJournalError::Malformed)?;
    for (name, value) in fields {
        if name.ends_with("_digest") || matches!(name.as_str(), "wait" | "execution" | "binding") {
            if !value.is_null() && !value.as_str().is_some_and(looks_like_digest) {
                return Err(SourceJournalError::Binding);
            }
        }
        if matches!(
            name.as_str(),
            "state"
                | "proposal"
                | "decision"
                | "observation"
                | "copy_arguments"
                | "signature"
                | "receipt"
                | "operations"
                | "status"
                | "terminal"
        ) && canonical(value).len() > super::super::MAX_SOURCE_CARRIER_BYTES
        {
            return Err(SourceJournalError::Capacity);
        }
    }
    if let Some(checkpoint) = fields.get("checkpoint") {
        let text = checkpoint.as_str().ok_or(SourceJournalError::Malformed)?;
        if text.len() > 2 * super::super::MAX_SOURCE_CARRIER_BYTES {
            return Err(SourceJournalError::Capacity);
        }
        if unhex(text).is_none() {
            return Err(SourceJournalError::Malformed);
        }
    }
    Ok(())
}

/// Closed hash recipes only. Computing a digest does not authenticate its inputs
/// or grant the caller a checked context, store registration, or runtime owner.
#[derive(Clone, Copy)]
pub(super) enum RecipeV8 {
    Invocation,
    Attempt,
    Generation,
    Transfer,
    Operations,
    Receipt,
    Decision,
    Grant,
}
impl RecipeV8 {
    fn domain(self) -> &'static [u8] {
        match self {
            Self::Invocation => b"semaprax.live-invocation.source-id.v8\0",
            Self::Attempt => b"semaprax.source-agent-owned-wait.attempt.v1\0",
            Self::Generation => b"semaprax.source-agent-owned-wait.generation.v1\0",
            Self::Transfer => b"semaprax.source-agent-owned-wait.transfer.v1\0",
            Self::Operations => b"semaprax.source-agent-owned-wait.operations.v1\0",
            Self::Receipt => b"semaprax.source-agent-owned-wait.receipt.v1\0",
            Self::Decision => b"semaprax.source-agent-owned-wait.decision.v1\0",
            Self::Grant => b"semaprax.source-agent-owned-wait.grant.v1\0",
        }
    }
    fn fields(self) -> Option<&'static [&'static str]> {
        Some(match self {
            Self::Invocation => &["execution", "owned_wait_binding"],
            Self::Attempt => &["invocation", "turn", "attempt", "binding"],
            Self::Generation => &["scope", "execution", "binding", "store_identity", "limits"],
            Self::Transfer => &[
                "scope",
                "generation",
                "turn",
                "attempt",
                "wait",
                "from",
                "to",
                "state_digest",
                "proposal_digest",
            ],
            Self::Operations => &["owner", "basis", "terminal", "operations"],
            Self::Receipt => return None,
            Self::Decision => &["scope", "turn", "attempt", "authorize", "decision"],
            Self::Grant => &[
                "scope",
                "turn",
                "attempt",
                "state_digest",
                "proposal_digest",
                "decision_digest",
                "authorization_binding",
                "budget",
            ],
        })
    }
}
pub(super) fn recipe_digest(recipe: RecipeV8, value: &Value) -> Result<String, SourceJournalError> {
    if let Some(keys) = recipe.fields() {
        let fields = value.as_object().ok_or(SourceJournalError::Malformed)?;
        if fields.len() != keys.len() || !keys.iter().all(|key| fields.contains_key(*key)) {
            return Err(SourceJournalError::Malformed);
        }
    }
    let bytes = canonical(value);
    // Reject float/depth/size even for in-memory producers; arrays remain ordered.
    parse(&bytes)?;
    Ok(crate::live_invocation::identity::digest(
        recipe.domain(),
        &bytes,
    ))
}
pub(super) fn checkpoint_bytes_digest(bytes: &[u8]) -> Result<String, SourceJournalError> {
    if bytes.len() > super::super::MAX_SOURCE_CARRIER_BYTES {
        return Err(SourceJournalError::Capacity);
    }
    if !bytes.ends_with(b"\n") {
        return Err(SourceJournalError::Malformed);
    }
    let envelope = parse(&bytes[..bytes.len() - 1])?;
    let fields = envelope.as_object().ok_or(SourceJournalError::Malformed)?;
    if fields.len() != 2
        || !fields.contains_key("payload")
        || !fields
            .get("authentication")
            .and_then(Value::as_str)
            .is_some_and(tag_valid)
    {
        return Err(SourceJournalError::Malformed);
    }
    let mut canonical_bytes = canonical(&envelope);
    canonical_bytes.push(b'\n');
    if canonical_bytes != bytes {
        return Err(SourceJournalError::Malformed);
    }
    Ok(crate::live_invocation::identity::digest(
        b"semaprax.source-agent-owned-wait.checkpoint.v1\0",
        bytes,
    ))
}

/// Authenticate the complete combined inventory before any future fold use.
/// This returns data only; store/checked-source provenance is independent.
pub(super) fn decode_inventory(
    bytes: &[u8],
    expected: &ExpectedRowV8<'_>,
    key: &SourceCheckpointKey,
) -> Result<Vec<EntryV8>, SourceJournalError> {
    if bytes.len() > super::super::MAX_SOURCE_DOCUMENT_BYTES {
        return Err(SourceJournalError::Capacity);
    }
    if expected.seq != 0 || expected.prev_mac != "0".repeat(64) {
        return Err(SourceJournalError::Binding);
    }
    if !bytes.is_empty() && !bytes.ends_with(b"\n") {
        return Err(SourceJournalError::Malformed);
    }
    let mut entries = Vec::new();
    let mut previous = expected.prev_mac.to_owned();
    for row in bytes.split_inclusive(|byte| *byte == b'\n') {
        if entries.len() >= super::super::MAX_SOURCE_ENTRIES {
            return Err(SourceJournalError::Capacity);
        }
        let current = ExpectedRowV8 {
            invocation: expected.invocation,
            generation: expected.generation,
            seq: u32::try_from(entries.len()).map_err(|_| SourceJournalError::Capacity)?,
            prev_mac: &previous,
            ordinary: expected.ordinary,
        };
        let entry = decode(row, &current, key)?;
        // Already canonical, duplicate-free and authenticated by decode.
        let value = parse(&row[..row.len() - 1])?;
        previous = value["authentication"]
            .as_str()
            .ok_or(SourceJournalError::Malformed)?
            .to_owned();
        entries.push(entry);
    }
    Ok(entries)
}
