use super::{capacity, invalid, Result, MAX_BYTES, SCHEMA};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope<T> {
    schema: String,
    payload: T,
    payload_digest: String,
}

pub(super) fn canonical<T: Serialize>(value: &T) -> Result<String> {
    let value = serde_json::to_value(value).map_err(|_| invalid("law data cannot be encoded"))?;
    let mut result =
        serde_json::to_string(&value).map_err(|_| invalid("law data cannot be encoded"))?;
    result.push('\n');
    if result.len() > MAX_BYTES {
        return Err(capacity());
    }
    Ok(result)
}
pub(super) fn digest(domain: &[u8], bytes: &str) -> String {
    super::super::render::domain_digest(domain, bytes.as_bytes())
}
pub(super) fn encode<T: Serialize>(payload: &T) -> Result<(String, String)> {
    let payload_digest = digest(b"semaprax.law-set.payload.v1\0", &canonical(payload)?);
    let document = canonical(&Envelope {
        schema: SCHEMA.to_owned(),
        payload,
        payload_digest: payload_digest.clone(),
    })?;
    Ok((document, payload_digest))
}
pub(super) fn decode<T: DeserializeOwned + Serialize>(document: &str) -> Result<T> {
    if document.len() > MAX_BYTES {
        return Err(capacity());
    }
    // Typed serde structs reject duplicates and unknown fields; exact canonical
    // comparison also rejects duplicate keys inside any future Value surface.
    let envelope: Envelope<T> = serde_json::from_str(document)
        .map_err(|_| invalid("malformed law envelope, unknown or duplicate field"))?;
    if envelope.schema != SCHEMA {
        return Err(invalid("unsupported law schema"));
    }
    if encode(&envelope.payload)?.0 != document {
        return Err(invalid("noncanonical law envelope or digest mismatch"));
    }
    Ok(envelope.payload)
}
pub(super) fn report_value(document: &str) -> Result<Value> {
    decode(document)
}
