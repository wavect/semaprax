//! Independent closed replay for the owned-data SDK's semantic descriptor and
//! additive outer manifest.

use super::*;

const PUBLIC_DESCRIPTOR_DIGEST_DOMAIN: &[u8] = b"semaprax.public-owned-data-api.digest.v1\0";

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

#[derive(Clone, Copy)]
pub(super) struct ManifestFacts<'a> {
    pub target: &'a str,
    pub descriptor: &'a [u8],
    pub descriptor_digest: &'a str,
    pub archive_name: &'a str,
    pub files: [(&'a str, &'a [u8]); 6],
}

pub(super) fn verify_manifest(
    bytes: &[u8],
    expected: &ManifestFacts<'_>,
) -> Result<(), Diagnostic> {
    if !bytes.ends_with(b"\n") {
        return Err(error("owned-data SDK manifest is not newline terminated"));
    }
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| error("owned-data SDK manifest JSON is malformed"))?;
    let root = value
        .as_object()
        .filter(|root| root.len() == 8)
        .ok_or_else(|| error("owned-data SDK manifest root is not closed"))?;
    let crate_row = closed_object(root.get("crate"), 2, "crate")?;
    let descriptor = closed_object(root.get("descriptor"), 3, "descriptor")?;
    let provider = closed_object(root.get("provider"), 3, "provider")?;
    let limits = closed_object(root.get("limits"), 4, "limits")?;
    if expected.descriptor_digest != public_descriptor_digest(expected.descriptor) {
        return Err(error("owned-data SDK descriptor digest fact disagrees"));
    }
    let expected_names = [
        "Cargo.toml",
        "build.rs",
        "lib.rs",
        "owned_data_ffi.rs",
        expected.archive_name,
        "descriptor.json",
    ];
    if expected_names.iter().any(|name| {
        expected
            .files
            .iter()
            .filter(|(candidate, _)| candidate == name)
            .count()
            != 1
    }) || expected
        .files
        .iter()
        .find(|(name, _)| *name == "descriptor.json")
        .is_none_or(|(_, bytes)| *bytes != expected.descriptor)
    {
        return Err(error("owned-data SDK expected file facts are not closed"));
    }
    if root.get("schema").and_then(Value::as_str) != Some(NATIVE_RUST_OWNED_DATA_SDK_SCHEMA)
        || crate_row.get("name").and_then(Value::as_str)
            != Some(super::owned_data::OWNED_CRATE_NAME)
        || crate_row.get("version").and_then(Value::as_str)
            != Some(super::owned_data::OWNED_CRATE_VERSION)
        || root.get("target").and_then(Value::as_str) != Some(expected.target)
        || descriptor.get("schema").and_then(Value::as_str)
            != Some(semaprax::project::PUBLIC_OWNED_DATA_API_SCHEMA)
        || descriptor.get("bytes").and_then(Value::as_u64)
            != u64::try_from(expected.descriptor.len()).ok()
        || descriptor.get("digest").and_then(Value::as_str) != Some(expected.descriptor_digest)
        || provider.get("abi").and_then(Value::as_str) != Some("opaque-handle.v1")
        || provider.get("archive").and_then(Value::as_str) != Some(expected.archive_name)
    {
        return Err(error("owned-data SDK manifest facts disagree"));
    }
    let operations = provider
        .get("operations")
        .and_then(Value::as_array)
        .ok_or_else(|| error("owned-data SDK provider operations are malformed"))?;
    if !string_set_exact(operations, &["len", "copy", "drop"]) {
        return Err(error("owned-data SDK provider operations disagree"));
    }
    let rows = root
        .get("files")
        .and_then(Value::as_array)
        .filter(|rows| rows.len() == expected.files.len())
        .ok_or_else(|| error("owned-data SDK manifest file inventory disagrees"))?;
    let mut seen = BTreeSet::new();
    for row in rows {
        let row = row
            .as_object()
            .filter(|row| row.len() == 3)
            .ok_or_else(|| error("owned-data SDK manifest file row is not closed"))?;
        let path = row
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| error("owned-data SDK manifest file path is malformed"))?;
        if !seen.insert(path) {
            return Err(error("owned-data SDK manifest file path is duplicated"));
        }
        let (_, expected_bytes) = expected
            .files
            .iter()
            .find(|(candidate, _)| *candidate == path)
            .ok_or_else(|| error("owned-data SDK manifest contains an unknown file"))?;
        if row.get("bytes").and_then(Value::as_u64) != u64::try_from(expected_bytes.len()).ok()
            || row.get("sha256").and_then(Value::as_str)
                != Some(raw_digest(expected_bytes).as_str())
        {
            return Err(error("owned-data SDK manifest file binding disagrees"));
        }
    }
    if limits
        .get("max_borrowed_input_bytes")
        .and_then(Value::as_u64)
        != Some(65_536)
        || limits.get("max_owned_output_bytes").and_then(Value::as_u64) != Some(65_536)
        || limits.get("max_handles").and_then(Value::as_u64) != Some(4_096)
        || limits.get("exact_package_files").and_then(Value::as_u64) != Some(7)
    {
        return Err(error("owned-data SDK manifest limits disagree"));
    }
    let nonclaims = root
        .get("nonclaims")
        .and_then(Value::as_array)
        .ok_or_else(|| error("owned-data SDK manifest nonclaims are malformed"))?;
    let expected_nonclaims = [
        "no_raw_handle_or_context_public_api",
        "no_allocator_transfer",
        "no_allocator_oom_abort_or_panic_recovery_proof",
        "no_send_sync",
        "no_project_v8_activation",
    ];
    if !string_set_exact(nonclaims, &expected_nonclaims) {
        return Err(error("owned-data SDK manifest nonclaims disagree"));
    }
    Ok(())
}

fn public_descriptor_digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(PUBLIC_DESCRIPTOR_DIGEST_DOMAIN);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    )
}

fn string_set_exact(values: &[Value], expected: &[&str]) -> bool {
    values.len() == expected.len()
        && expected.iter().all(|expected| {
            values
                .iter()
                .filter(|value| value.as_str() == Some(expected))
                .count()
                == 1
        })
}

fn closed_object<'a>(
    value: Option<&'a Value>,
    length: usize,
    name: &str,
) -> Result<&'a serde_json::Map<String, Value>, Diagnostic> {
    value
        .and_then(Value::as_object)
        .filter(|row| row.len() == length)
        .ok_or_else(|| error(format!("owned-data SDK manifest {name} is not closed")))
}

fn error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-B114", message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_replay_rejects_independent_fact_mutations() {
        let descriptor = br#"{"schema":"fixture"}"#;
        let files: [(&str, &[u8]); 6] = [
            ("Cargo.toml", b"cargo"),
            ("build.rs", b"build"),
            ("lib.rs", b"lib"),
            ("owned_data_ffi.rs", b"ffi"),
            ("libsemaprax_native_rust_owned_data_sdk.a", b"archive"),
            ("descriptor.json", descriptor),
        ];
        let descriptor_digest = public_descriptor_digest(descriptor);
        let facts = ManifestFacts {
            target: "aarch64-apple-darwin",
            descriptor,
            descriptor_digest: &descriptor_digest,
            archive_name: "libsemaprax_native_rust_owned_data_sdk.a",
            files,
        };
        let manifest = super::super::owned_data::render_manifest(
            facts.target,
            facts.descriptor,
            facts.descriptor_digest,
            facts.archive_name,
            facts.files,
        );
        verify_manifest(manifest.as_bytes(), &facts).unwrap();

        let missing_length_digest = domain_digest(PUBLIC_DESCRIPTOR_DIGEST_DOMAIN, descriptor);
        let wrong_digest_facts = ManifestFacts {
            descriptor_digest: &missing_length_digest,
            ..facts
        };
        assert!(verify_manifest(manifest.as_bytes(), &wrong_digest_facts).is_err());

        let original: Value = serde_json::from_str(&manifest).unwrap();
        let mut mutations = Vec::new();
        let mut value = original.clone();
        value
            .as_object_mut()
            .unwrap()
            .insert("surplus".into(), Value::Null);
        mutations.push(value);
        let mut value = original.clone();
        value["descriptor"]["bytes"] = Value::from(0);
        mutations.push(value);
        let mut value = original.clone();
        value["files"][0]["sha256"] = Value::from("sha256:forged");
        mutations.push(value);
        let mut value = original.clone();
        value["provider"]["operations"] = serde_json::json!(["copy", "copy", "drop"]);
        mutations.push(value);
        let mut value = original.clone();
        value["limits"]["max_handles"] = Value::from(4095);
        mutations.push(value);
        let mut value = original;
        value["nonclaims"] = serde_json::json!(["no_project_v8_activation"]);
        mutations.push(value);
        for mutation in mutations {
            let mut bytes = serde_json::to_vec(&mutation).unwrap();
            bytes.push(b'\n');
            assert!(verify_manifest(&bytes, &facts).is_err());
        }
    }
}
