//! Independent closed replay for the owned-data SDK's semantic descriptor and
//! additive outer manifest.

use super::*;

pub(super) fn replay_descriptor(
    bytes: &[u8],
    expected: &semaprax::project::PublicApiDescriptor,
) -> Result<(), Diagnostic> {
    if bytes.len() > semaprax::project::MAX_PUBLIC_API_DESCRIPTOR_BYTES
        || bytes != expected.canonical_bytes().as_slice()
    {
        return Err(error("owned-data SDK descriptor bytes disagree"));
    }
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| error("owned-data SDK descriptor JSON is malformed"))?;
    let root = value
        .as_object()
        .filter(|root| root.len() == 7)
        .ok_or_else(|| error("owned-data SDK descriptor root is not closed"))?;
    if root.get("schema").and_then(Value::as_str)
        != Some(semaprax::project::PUBLIC_OWNED_DATA_API_SCHEMA)
        || root.get("project_schema").and_then(Value::as_str)
            != Some(semaprax::project::PUBLIC_OWNED_DATA_PROJECT_SCHEMA)
        || root.get("project_revision").and_then(Value::as_str) != Some(expected.project_revision())
        || root.get("workspace_revision").and_then(Value::as_str)
            != Some(expected.workspace_revision())
        || root.get("project_graph_digest").and_then(Value::as_str)
            != Some(expected.project_graph_digest())
    {
        return Err(error("owned-data SDK descriptor subject replay failed"));
    }
    let exports = root
        .get("exports")
        .and_then(Value::as_array)
        .filter(|exports| exports.len() == expected.exports().len())
        .ok_or_else(|| error("owned-data SDK descriptor export replay failed"))?;
    for (row, expected) in exports.iter().zip(expected.exports()) {
        let row = row
            .as_object()
            .filter(|row| row.len() == 5)
            .ok_or_else(|| error("owned-data SDK descriptor export is not closed"))?;
        if row.get("stable_id").and_then(Value::as_str) != Some(expected.stable_id().as_str())
            || row.get("typescript_name").and_then(Value::as_str)
                != Some(expected.typescript_name())
            || row.get("rust_method_name").and_then(Value::as_str)
                != Some(expected.rust_method_name())
            || row.get("result").and_then(Value::as_str) != Some(expected.result().wire_name())
        {
            return Err(error("owned-data SDK descriptor export disagrees"));
        }
        let parameters = row
            .get("parameters")
            .and_then(Value::as_array)
            .filter(|parameters| parameters.len() == expected.parameters().len())
            .ok_or_else(|| error("owned-data SDK descriptor parameters disagree"))?;
        for (ordinal, (parameter, expected)) in
            parameters.iter().zip(expected.parameters()).enumerate()
        {
            let parameter = parameter
                .as_object()
                .filter(|parameter| parameter.len() == 4)
                .ok_or_else(|| error("owned-data SDK descriptor parameter is not closed"))?;
            if parameter.get("stable_id").and_then(Value::as_str)
                != Some(expected.stable_id().as_str())
                || parameter.get("source_name").and_then(Value::as_str)
                    != Some(expected.source_name())
                || parameter.get("ordinal").and_then(Value::as_u64) != Some(ordinal as u64)
                || parameter.get("type").and_then(Value::as_str) != Some(expected.ty().wire_name())
            {
                return Err(error("owned-data SDK descriptor parameter disagrees"));
            }
        }
    }
    Ok(())
}

pub(super) fn verify_manifest(bytes: &[u8], expected: &str) -> Result<(), Diagnostic> {
    if bytes != expected.as_bytes() || !expected.ends_with('\n') {
        return Err(error("owned-data SDK manifest exact replay failed"));
    }
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| error("owned-data SDK manifest JSON is malformed"))?;
    let root = value
        .as_object()
        .filter(|root| root.len() == 8)
        .ok_or_else(|| error("owned-data SDK manifest root is not closed"))?;
    if root.get("schema").and_then(Value::as_str) != Some(NATIVE_RUST_OWNED_DATA_SDK_SCHEMA) {
        return Err(error("owned-data SDK manifest schema is unsupported"));
    }
    Ok(())
}

fn error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-B114", message)
}
