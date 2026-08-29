//! Canonical npm build carrier, trusted binding, and exact replay machinery.
//!
//! This module owns the schema-closed envelope shared by the existing text,
//! data, and command package profiles. Profile-specific artifact derivation
//! remains in its existing renderer; replay dispatches to those unchanged
//! validators before accepting a canonical envelope.

use std::path::Path;

use sha2::{Digest, Sha256};

use crate::diagnostic::{quote_json, Diagnostic};

use super::{
    command, command_v2, command_v3, command_v4, data, owned_data, package_error,
    validate_replayed_package, UsefulTextNpmPackage, USEFUL_TEXT_PACKAGE_PATHS,
};

pub const PROJECT_NPM_BUILD_SCHEMA: &str = "semaprax.project-npm-build.v1";
pub const PROJECT_NPM_BUILD_SCHEMA_V2: &str = "semaprax.project-npm-build.v2";
pub const PROJECT_NPM_BUILD_SCHEMA_V3: &str = "semaprax.project-npm-build.v3";
pub const PROJECT_NPM_BUILD_SCHEMA_V4: &str = "semaprax.project-npm-build.v4";
pub const PROJECT_NPM_BUILD_SCHEMA_V5: &str = "semaprax.project-npm-build.v5";
pub const PROJECT_NPM_BUILD_SCHEMA_V6: &str = "semaprax.project-npm-build.v6";
pub const PROJECT_NPM_BUILD_SCHEMA_V7: &str = "semaprax.project-npm-build.v7";
const PROJECT_NPM_BUILD_DIGEST_DOMAIN: &[u8] = b"semaprax.project-npm-build.payload.v1\0";
const PROJECT_NPM_BUILD_DIGEST_DOMAIN_V2: &[u8] = b"semaprax.project-npm-build.payload.v2\0";
const PROJECT_NPM_BUILD_DIGEST_DOMAIN_V3: &[u8] = b"semaprax.project-npm-build.payload.v3\0";
const PROJECT_NPM_BUILD_DIGEST_DOMAIN_V4: &[u8] = b"semaprax.project-npm-build.payload.v4\0";
const PROJECT_NPM_BUILD_DIGEST_DOMAIN_V5: &[u8] = b"semaprax.project-npm-build.payload.v5\0";
const PROJECT_NPM_BUILD_DIGEST_DOMAIN_V6: &[u8] = b"semaprax.project-npm-build.payload.v6\0";
const PROJECT_NPM_BUILD_DIGEST_DOMAIN_V7: &[u8] = b"semaprax.project-npm-build.payload.v7\0";
pub const MAX_PROJECT_NPM_BUILD_BYTES: usize = 40 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NpmArtifact {
    pub(in crate::project) path: &'static str,
    pub(in crate::project) bytes: Vec<u8>,
}

impl NpmArtifact {
    #[cfg(test)]
    pub(crate) fn path(&self) -> &'static str {
        self.path
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

pub(in crate::project) fn artifact(path: &'static str, bytes: &[u8]) -> NpmArtifact {
    NpmArtifact {
        path,
        bytes: bytes.to_vec(),
    }
}

enum ReplayedNpmArtifacts {
    Text([NpmArtifact; 6]),
    Data([NpmArtifact; 6]),
    Command([NpmArtifact; 7]),
    CommandV2([NpmArtifact; 7]),
    CommandV3([NpmArtifact; 7]),
    CommandV4([NpmArtifact; 7]),
    OwnedData([NpmArtifact; 6]),
}

impl ReplayedNpmArtifacts {
    fn as_slice(&self) -> &[NpmArtifact] {
        match self {
            Self::Text(value) | Self::Data(value) | Self::OwnedData(value) => value,
            Self::Command(value)
            | Self::CommandV2(value)
            | Self::CommandV3(value)
            | Self::CommandV4(value) => value,
        }
    }
}

/// Canonical, replayable carrier for one schema-selected exact npm package.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectNpmBuild {
    pub(super) envelope: String,
    pub(super) payload_digest: String,
    pub(super) artifact_bytes: usize,
    pub(super) max_bytes: usize,
    pub(super) trusted: TrustedNpmBinding,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct TrustedNpmBinding {
    project_schema: String,
    package: String,
    version: String,
    project_revision: String,
    workspace_revision: String,
    project_graph_digest: String,
    semantic_recipe: String,
}

struct ReplayedNpmEnvelope {
    canonical: String,
    payload_digest: String,
    artifact_bytes: usize,
    trusted: TrustedNpmBinding,
}

impl ProjectNpmBuild {
    pub fn envelope(&self) -> &str {
        &self.envelope
    }

    pub fn payload_digest(&self) -> &str {
        &self.payload_digest
    }

    pub fn artifact_bytes(&self) -> usize {
        self.artifact_bytes
    }

    pub fn max_bytes(&self) -> usize {
        self.max_bytes
    }

    pub fn verify(&self) -> Result<(), Diagnostic> {
        let replayed = Self::replay_envelope(&self.envelope, self.max_bytes)?;
        if replayed.payload_digest != self.payload_digest
            || replayed.artifact_bytes != self.artifact_bytes
            || replayed.canonical != self.envelope
            || replayed.trusted != self.trusted
        {
            return Err(package_error(
                "npm build disagrees with its context-bound trusted Project facts",
            ));
        }
        Ok(())
    }

    /// Inspect an untrusted serialized envelope for canonical compiler
    /// consistency. Success does not authenticate its claimed Project facts,
    /// construct a publishable build, or grant publication authority.
    pub fn inspect_envelope(envelope: &str, max_bytes: usize) -> Result<(), Diagnostic> {
        Self::replay_envelope(envelope, max_bytes).map(|_| ())
    }

    fn replay_envelope(
        envelope: &str,
        max_bytes: usize,
    ) -> Result<ReplayedNpmEnvelope, Diagnostic> {
        validate_carrier_limit(envelope.len(), max_bytes)?;
        let value: serde_json::Value = serde_json::from_str(envelope)
            .map_err(|_| package_error("npm build envelope is not valid JSON"))?;
        let object = value
            .as_object()
            .ok_or_else(|| package_error("npm build envelope must be one JSON object"))?;
        require_exact_keys(
            object,
            &[
                "artifact_bytes",
                "artifacts",
                "package",
                "payload_digest",
                "project_graph_digest",
                "project_revision",
                "project_schema",
                "schema",
                "semantic_recipe",
                "version",
                "workspace_revision",
            ],
        )?;
        let schema = json_string(object, "schema")?;
        let expected_paths: &[&str] = match schema {
            PROJECT_NPM_BUILD_SCHEMA => &USEFUL_TEXT_PACKAGE_PATHS,
            PROJECT_NPM_BUILD_SCHEMA_V2 => &data::USEFUL_DATA_PACKAGE_PATHS,
            PROJECT_NPM_BUILD_SCHEMA_V3 => &command::USEFUL_DATA_COMMAND_PACKAGE_PATHS,
            PROJECT_NPM_BUILD_SCHEMA_V4 => &command_v2::USEFUL_DATA_COMMAND_V2_PACKAGE_PATHS,
            PROJECT_NPM_BUILD_SCHEMA_V5 => &command_v3::LANGUAGE_COMMAND_IO_PACKAGE_PATHS,
            PROJECT_NPM_BUILD_SCHEMA_V6 => &command_v4::LINE_COMMAND_IO_PACKAGE_PATHS,
            PROJECT_NPM_BUILD_SCHEMA_V7 => &owned_data::OWNED_DATA_PACKAGE_PATHS,
            _ => return Err(package_error("npm build schema is unsupported")),
        };
        let identity = NpmBuildIdentity {
            project_schema: json_string(object, "project_schema")?,
            package: json_string(object, "package")?,
            version: json_string(object, "version")?,
            project_revision: json_string(object, "project_revision")?,
            workspace_revision: json_string(object, "workspace_revision")?,
            project_graph_digest: json_string(object, "project_graph_digest")?,
            semantic_recipe: json_string(object, "semantic_recipe")?,
        };
        for value in [
            identity.project_schema,
            identity.package,
            identity.version,
            identity.project_revision,
            identity.workspace_revision,
            identity.project_graph_digest,
        ] {
            if value.is_empty() || value.len() > 512 {
                return Err(package_error("npm build identity fact is unbounded"));
            }
        }
        if identity.semantic_recipe.is_empty() || identity.semantic_recipe.len() > 1024 * 1024 {
            return Err(package_error("npm build semantic recipe is unbounded"));
        }
        let rows = object
            .get("artifacts")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| package_error("npm build artifacts are invalid"))?;
        if rows.len() != expected_paths.len() {
            return Err(package_error("npm build artifact inventory is not exact"));
        }
        let mut artifacts = Vec::with_capacity(rows.len());
        let mut total = 0_usize;
        for (row, expected_path) in rows.iter().zip(expected_paths.iter().copied()) {
            let row = row
                .as_object()
                .ok_or_else(|| package_error("npm build artifact row is invalid"))?;
            require_exact_keys(row, &["hex", "path", "sha256"])?;
            let path = json_string(row, "path")?;
            if path != expected_path {
                return Err(package_error("npm build artifact order is not canonical"));
            }
            let bytes = decode_hex(json_string(row, "hex")?, max_bytes.saturating_sub(total))?;
            total = total
                .checked_add(bytes.len())
                .ok_or_else(|| package_error("npm build artifact byte count overflowed"))?;
            if total > max_bytes {
                return Err(package_error(
                    "npm build artifacts exceed the trusted limit",
                ));
            }
            let digest = format!(
                "sha256:{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(&bytes))
            );
            if json_string(row, "sha256")? != digest {
                return Err(package_error("npm build artifact digest disagrees"));
            }
            artifacts.push(NpmArtifact {
                path: expected_path,
                bytes,
            });
        }
        let declared_total = object
            .get("artifact_bytes")
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| package_error("npm build artifact_bytes is invalid"))?;
        if declared_total != total {
            return Err(package_error("npm build artifact byte count disagrees"));
        }
        let artifacts = match schema {
            PROJECT_NPM_BUILD_SCHEMA => ReplayedNpmArtifacts::Text(
                artifacts
                    .try_into()
                    .map_err(|_| package_error("npm build artifact inventory is not exact"))?,
            ),
            PROJECT_NPM_BUILD_SCHEMA_V2 => ReplayedNpmArtifacts::Data(
                artifacts
                    .try_into()
                    .map_err(|_| package_error("npm build artifact inventory is not exact"))?,
            ),
            PROJECT_NPM_BUILD_SCHEMA_V3 => ReplayedNpmArtifacts::Command(
                artifacts
                    .try_into()
                    .map_err(|_| package_error("npm build artifact inventory is not exact"))?,
            ),
            PROJECT_NPM_BUILD_SCHEMA_V4 => ReplayedNpmArtifacts::CommandV2(
                artifacts
                    .try_into()
                    .map_err(|_| package_error("npm build artifact inventory is not exact"))?,
            ),
            PROJECT_NPM_BUILD_SCHEMA_V5 => ReplayedNpmArtifacts::CommandV3(
                artifacts
                    .try_into()
                    .map_err(|_| package_error("npm build artifact inventory is not exact"))?,
            ),
            PROJECT_NPM_BUILD_SCHEMA_V6 => ReplayedNpmArtifacts::CommandV4(
                artifacts
                    .try_into()
                    .map_err(|_| package_error("npm build artifact inventory is not exact"))?,
            ),
            PROJECT_NPM_BUILD_SCHEMA_V7 => ReplayedNpmArtifacts::OwnedData(
                artifacts
                    .try_into()
                    .map_err(|_| package_error("npm build artifact inventory is not exact"))?,
            ),
            _ => unreachable!("carrier schema selected above"),
        };
        let payload_digest = match &artifacts {
            ReplayedNpmArtifacts::Text(artifacts) => {
                let package = UsefulTextNpmPackage {
                    artifacts: artifacts.clone(),
                };
                validate_replayed_package(identity, &package)?;
                payload_digest(identity, &package)
            }
            ReplayedNpmArtifacts::Data(artifacts) => {
                data::validate_replayed(identity, artifacts)?;
                payload_digest_artifacts_v2(identity, artifacts)
            }
            ReplayedNpmArtifacts::Command(artifacts) => {
                command::validate_replayed(identity, artifacts)?;
                payload_digest_artifacts_v3(identity, artifacts)
            }
            ReplayedNpmArtifacts::CommandV2(artifacts) => {
                command_v2::validate_replayed(identity, artifacts)?;
                payload_digest_artifacts_v4(identity, artifacts)
            }
            ReplayedNpmArtifacts::CommandV3(artifacts) => {
                command_v3::validate_replayed(identity, artifacts)?;
                payload_digest_artifacts_v5(identity, artifacts)
            }
            ReplayedNpmArtifacts::CommandV4(artifacts) => {
                command_v4::validate_replayed(identity, artifacts)?;
                payload_digest_artifacts_v6(identity, artifacts)
            }
            ReplayedNpmArtifacts::OwnedData(artifacts) => {
                owned_data::validate_replayed(identity, artifacts)?;
                payload_digest_artifacts_v7(identity, artifacts)
            }
        };
        if json_string(object, "payload_digest")? != payload_digest {
            return Err(package_error("npm build payload digest disagrees"));
        }
        let canonical = render_carrier_artifacts(
            schema,
            identity,
            artifacts.as_slice(),
            total,
            &payload_digest,
        );
        if canonical != envelope {
            return Err(package_error("npm build envelope is not canonical"));
        }
        Ok(ReplayedNpmEnvelope {
            canonical,
            payload_digest,
            artifact_bytes: total,
            trusted: trusted_binding(identity),
        })
    }

    /// Publish into a destination that must not already exist. Files use
    /// create-new semantics, so this route never replaces foreign bytes.
    /// Publication is deliberately fail-stop, not atomic: after a write or
    /// settlement error, the newly created destination may contain the exact
    /// canonical artifact prefix already reported as successful. This method
    /// never guesses at cleanup authority or claims that prefix was removed.
    pub fn publish(&self, output: &Path) -> Result<(), Diagnostic> {
        self.verify()?;
        let package = decode_carrier_artifacts(&self.envelope, self.max_bytes)?;
        let value: serde_json::Value = serde_json::from_str(&self.envelope)
            .map_err(|_| package_error("npm build envelope is not valid JSON"))?;
        let schema = value
            .as_object()
            .ok_or_else(|| package_error("npm build envelope must be one JSON object"))
            .and_then(|object| json_string(object, "schema"))?;
        #[cfg(not(any(target_arch = "wasm32", target_arch = "wasm64")))]
        {
            super::publication::publish(output, package.as_slice(), schema)
        }
        #[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
        {
            let _ = (output, package, schema);
            Err(package_error(
                "npm package publication is unavailable on a Wasm host",
            ))
        }
    }
}

#[derive(Clone, Copy)]
pub(in crate::project) struct NpmBuildIdentity<'a> {
    pub(super) project_schema: &'a str,
    pub(super) package: &'a str,
    pub(super) version: &'a str,
    pub(super) project_revision: &'a str,
    pub(super) workspace_revision: &'a str,
    pub(super) project_graph_digest: &'a str,
    pub(super) semantic_recipe: &'a str,
}

pub(super) fn trusted_binding(identity: NpmBuildIdentity<'_>) -> TrustedNpmBinding {
    TrustedNpmBinding {
        project_schema: identity.project_schema.to_owned(),
        package: identity.package.to_owned(),
        version: identity.version.to_owned(),
        project_revision: identity.project_revision.to_owned(),
        workspace_revision: identity.workspace_revision.to_owned(),
        project_graph_digest: identity.project_graph_digest.to_owned(),
        semantic_recipe: identity.semantic_recipe.to_owned(),
    }
}

pub(super) fn payload_digest(
    identity: NpmBuildIdentity<'_>,
    package: &UsefulTextNpmPackage,
) -> String {
    payload_digest_artifacts(identity, package.artifacts())
}

fn payload_digest_artifacts(identity: NpmBuildIdentity<'_>, artifacts: &[NpmArtifact]) -> String {
    payload_digest_artifacts_with_domain(PROJECT_NPM_BUILD_DIGEST_DOMAIN, identity, artifacts)
}

pub(in crate::project) fn payload_digest_artifacts_v2(
    identity: NpmBuildIdentity<'_>,
    artifacts: &[NpmArtifact],
) -> String {
    payload_digest_artifacts_with_domain(PROJECT_NPM_BUILD_DIGEST_DOMAIN_V2, identity, artifacts)
}

pub(in crate::project) fn payload_digest_artifacts_v3(
    identity: NpmBuildIdentity<'_>,
    artifacts: &[NpmArtifact],
) -> String {
    payload_digest_artifacts_with_domain(PROJECT_NPM_BUILD_DIGEST_DOMAIN_V3, identity, artifacts)
}

pub(in crate::project) fn payload_digest_artifacts_v4(
    identity: NpmBuildIdentity<'_>,
    artifacts: &[NpmArtifact],
) -> String {
    payload_digest_artifacts_with_domain(PROJECT_NPM_BUILD_DIGEST_DOMAIN_V4, identity, artifacts)
}

pub(in crate::project) fn payload_digest_artifacts_v5(
    identity: NpmBuildIdentity<'_>,
    artifacts: &[NpmArtifact],
) -> String {
    payload_digest_artifacts_with_domain(PROJECT_NPM_BUILD_DIGEST_DOMAIN_V5, identity, artifacts)
}

pub(in crate::project) fn payload_digest_artifacts_v6(
    identity: NpmBuildIdentity<'_>,
    artifacts: &[NpmArtifact],
) -> String {
    payload_digest_artifacts_with_domain(PROJECT_NPM_BUILD_DIGEST_DOMAIN_V6, identity, artifacts)
}

pub(in crate::project) fn payload_digest_artifacts_v7(
    identity: NpmBuildIdentity<'_>,
    artifacts: &[NpmArtifact],
) -> String {
    payload_digest_artifacts_with_domain(PROJECT_NPM_BUILD_DIGEST_DOMAIN_V7, identity, artifacts)
}

fn payload_digest_artifacts_with_domain(
    domain: &[u8],
    identity: NpmBuildIdentity<'_>,
    artifacts: &[NpmArtifact],
) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    for value in [
        identity.project_schema,
        identity.package,
        identity.version,
        identity.project_revision,
        identity.workspace_revision,
        identity.project_graph_digest,
        identity.semantic_recipe,
    ] {
        digest.update((value.len() as u64).to_le_bytes());
        digest.update(value.as_bytes());
    }
    for artifact in artifacts {
        digest.update((artifact.path.len() as u64).to_le_bytes());
        digest.update(artifact.path.as_bytes());
        digest.update((artifact.bytes.len() as u64).to_le_bytes());
        digest.update(&artifact.bytes);
    }
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(digest.finalize())
    )
}

pub(super) fn render_carrier(
    identity: NpmBuildIdentity<'_>,
    package: &UsefulTextNpmPackage,
    artifact_bytes: usize,
    payload_digest: &str,
) -> String {
    render_carrier_artifacts(
        PROJECT_NPM_BUILD_SCHEMA,
        identity,
        package.artifacts(),
        artifact_bytes,
        payload_digest,
    )
}

pub(in crate::project) fn render_carrier_artifacts(
    schema: &str,
    identity: NpmBuildIdentity<'_>,
    package: &[NpmArtifact],
    artifact_bytes: usize,
    payload_digest: &str,
) -> String {
    let artifacts = package
        .iter()
        .map(|artifact| {
            format!(
                "{{\"path\":{},\"sha256\":\"sha256:{:x}\",\"hex\":\"{}\"}}",
                quote_json(artifact.path),
                crate::digest_hex::LowerHex(Sha256::digest(&artifact.bytes)),
                encode_hex(&artifact.bytes),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"schema\":{},\"project_schema\":{},\"package\":{},\"version\":{},\"project_revision\":{},\"workspace_revision\":{},\"project_graph_digest\":{},\"semantic_recipe\":{},\"artifact_bytes\":{},\"payload_digest\":{},\"artifacts\":[{}]}}",
        quote_json(schema),
        quote_json(identity.project_schema),
        quote_json(identity.package),
        quote_json(identity.version),
        quote_json(identity.project_revision),
        quote_json(identity.workspace_revision),
        quote_json(identity.project_graph_digest),
        quote_json(identity.semantic_recipe),
        artifact_bytes,
        quote_json(payload_digest),
        artifacts,
    )
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn decode_hex(value: &str, remaining: usize) -> Result<Vec<u8>, Diagnostic> {
    if value.len() & 1 == 1 || value.len() / 2 > remaining {
        return Err(package_error(
            "npm build artifact hex exceeds the trusted limit",
        ));
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    let encoded = value.as_bytes();
    let mut offset = 0;
    while offset < encoded.len() {
        let high = hex_nibble(encoded[offset])
            .ok_or_else(|| package_error("npm build artifact hex is not lowercase"))?;
        let low = hex_nibble(encoded[offset + 1])
            .ok_or_else(|| package_error("npm build artifact hex is not lowercase"))?;
        bytes.push((high << 4) | low);
        offset += 2;
    }
    Ok(bytes)
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

pub(super) fn require_exact_keys(
    object: &serde_json::Map<String, serde_json::Value>,
    expected: &[&str],
) -> Result<(), Diagnostic> {
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        return Err(package_error(
            "npm build object has an unknown or missing field",
        ));
    }
    Ok(())
}

pub(super) fn json_string<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<&'a str, Diagnostic> {
    object
        .get(key)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| package_error(format!("npm build {key} is invalid")))
}

pub(super) fn validate_carrier_limit(length: usize, max_bytes: usize) -> Result<(), Diagnostic> {
    if max_bytes == 0 || max_bytes > MAX_PROJECT_NPM_BUILD_BYTES || length > max_bytes {
        return Err(package_error("npm build exceeds the trusted carrier limit"));
    }
    Ok(())
}

fn decode_carrier_artifacts(
    envelope: &str,
    max_bytes: usize,
) -> Result<ReplayedNpmArtifacts, Diagnostic> {
    let value: serde_json::Value = serde_json::from_str(envelope)
        .map_err(|_| package_error("npm build envelope is not valid JSON"))?;
    let schema = value
        .get("schema")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| package_error("npm build schema is invalid"))?;
    let paths: &[&str] = match schema {
        PROJECT_NPM_BUILD_SCHEMA => &USEFUL_TEXT_PACKAGE_PATHS,
        PROJECT_NPM_BUILD_SCHEMA_V2 => &data::USEFUL_DATA_PACKAGE_PATHS,
        PROJECT_NPM_BUILD_SCHEMA_V3 => &command::USEFUL_DATA_COMMAND_PACKAGE_PATHS,
        PROJECT_NPM_BUILD_SCHEMA_V4 => &command_v2::USEFUL_DATA_COMMAND_V2_PACKAGE_PATHS,
        PROJECT_NPM_BUILD_SCHEMA_V5 => &command_v3::LANGUAGE_COMMAND_IO_PACKAGE_PATHS,
        PROJECT_NPM_BUILD_SCHEMA_V6 => &command_v4::LINE_COMMAND_IO_PACKAGE_PATHS,
        PROJECT_NPM_BUILD_SCHEMA_V7 => &owned_data::OWNED_DATA_PACKAGE_PATHS,
        _ => return Err(package_error("npm build schema is unsupported")),
    };
    let rows = value
        .get("artifacts")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| package_error("npm build artifacts are invalid"))?;
    let mut total = 0_usize;
    let mut artifacts = Vec::with_capacity(paths.len());
    if rows.len() != paths.len() {
        return Err(package_error("npm build artifact inventory is not exact"));
    }
    for (row, path) in rows.iter().zip(paths.iter().copied()) {
        let encoded = row
            .get("hex")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| package_error("npm build artifact hex is invalid"))?;
        let bytes = decode_hex(encoded, max_bytes.saturating_sub(total))?;
        total += bytes.len();
        artifacts.push(NpmArtifact { path, bytes });
    }
    match schema {
        PROJECT_NPM_BUILD_SCHEMA => artifacts
            .try_into()
            .map(ReplayedNpmArtifacts::Text)
            .map_err(|_| package_error("npm build artifact inventory is not exact")),
        PROJECT_NPM_BUILD_SCHEMA_V2 => artifacts
            .try_into()
            .map(ReplayedNpmArtifacts::Data)
            .map_err(|_| package_error("npm build artifact inventory is not exact")),
        PROJECT_NPM_BUILD_SCHEMA_V3 => artifacts
            .try_into()
            .map(ReplayedNpmArtifacts::Command)
            .map_err(|_| package_error("npm build artifact inventory is not exact")),
        PROJECT_NPM_BUILD_SCHEMA_V4 => artifacts
            .try_into()
            .map(ReplayedNpmArtifacts::CommandV2)
            .map_err(|_| package_error("npm build artifact inventory is not exact")),
        PROJECT_NPM_BUILD_SCHEMA_V5 => artifacts
            .try_into()
            .map(ReplayedNpmArtifacts::CommandV3)
            .map_err(|_| package_error("npm build artifact inventory is not exact")),
        PROJECT_NPM_BUILD_SCHEMA_V6 => artifacts
            .try_into()
            .map(ReplayedNpmArtifacts::CommandV4)
            .map_err(|_| package_error("npm build artifact inventory is not exact")),
        PROJECT_NPM_BUILD_SCHEMA_V7 => artifacts
            .try_into()
            .map(ReplayedNpmArtifacts::OwnedData)
            .map_err(|_| package_error("npm build artifact inventory is not exact")),
        _ => unreachable!("carrier schema selected above"),
    }
}
