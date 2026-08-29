use serde_json::Value;

use crate::diagnostic::{quote_json, Diagnostic};

use super::super::public_api::valid_sha256_fact;
use super::{error, FlatOwnedRecordApiDescriptor, FLAT_OWNED_RECORD_METADATA_SCHEMA};

/// Render the v9 npm semantic metadata. Publication code must bind these bytes
/// into the additive v8 npm carrier; this function performs no I/O.
pub fn render_flat_owned_record_metadata(
    descriptor: &FlatOwnedRecordApiDescriptor,
    wasm_sha256: &str,
) -> Result<Vec<u8>, Diagnostic> {
    if !valid_sha256_fact(wasm_sha256) {
        return Err(error("flat owned-record Wasm digest is invalid"));
    }
    let mut output = String::new();
    output.push_str("{\"schema\":");
    output.push_str(&quote_json(FLAT_OWNED_RECORD_METADATA_SCHEMA));
    output.push_str(",\"descriptor\":");
    output.push_str(&quote_json(
        &String::from_utf8(descriptor.canonical_bytes()).expect("canonical descriptor is UTF-8"),
    ));
    output.push_str(",\"descriptor_digest\":");
    output.push_str(&quote_json(&descriptor.digest()));
    output.push_str(",\"wasm_sha256\":");
    output.push_str(&quote_json(wasm_sha256));
    output.push_str(",\"result_carrier\":\"opaque-handle-plus-scalars.v1\",\"settlement\":{\"copy_before_settle\":true,\"publish_after_settle\":true,\"failure_slot_unchanged\":true},\"artifacts\":[\"app.wasm\",\"semaprax.js\",\"semaprax.bindings.js\",\"semaprax.bindings.d.ts\",\"semaprax.api.json\",\"package.json\"]}\n");
    Ok(output.into_bytes())
}

pub fn replay_flat_owned_record_metadata(
    descriptor: &FlatOwnedRecordApiDescriptor,
    wasm_sha256: &str,
    submitted: &[u8],
) -> Result<(), Diagnostic> {
    let value: Value = serde_json::from_slice(submitted)
        .map_err(|_| error("flat owned-record npm metadata is invalid"))?;
    let root = value
        .as_object()
        .filter(|root| root.len() == 7)
        .ok_or_else(|| error("flat owned-record npm metadata root is not closed"))?;
    for key in root.keys() {
        if !matches!(
            key.as_str(),
            "schema"
                | "descriptor"
                | "descriptor_digest"
                | "wasm_sha256"
                | "result_carrier"
                | "settlement"
                | "artifacts"
        ) {
            return Err(error("flat owned-record npm metadata has an unknown field"));
        }
    }
    if submitted != render_flat_owned_record_metadata(descriptor, wasm_sha256)? {
        return Err(error(
            "flat owned-record npm metadata does not replay exactly",
        ));
    }
    Ok(())
}
