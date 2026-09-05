//! Exact source-interface and generated-artifact facts for a future ProgramRoot segment.
//!
//! The bundle is derived only from an admitted immutable Project revision and
//! pathless compiler projections. It retains evidence bytes but grants no
//! filesystem, build publication, execution, deployment, or other authority.

use std::sync::Arc;

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;

use super::image_targets::{
    ImageArtifactKind, MAX_IMAGE_ARTIFACT_BUILD_BYTES, MAX_IMAGE_ARTIFACT_REPORT_BYTES,
};
use super::profile::ProjectProfile;
use super::{ProjectRevision, ProjectSemanticImage};

pub const INTERFACE_ARTIFACT_FACTS_SCHEMA: &str = "semaprax.interface-artifact-facts.v1";
pub const INTERFACE_ARTIFACT_FACTS_COMPATIBILITY: &str =
    "semaprax.interface-artifact-facts-compatibility.v1";
pub const MAX_INTERFACE_ARTIFACT_FACTS: usize = 4;
pub const MAX_INTERFACE_ARTIFACT_FACT_INPUT_BYTES: usize =
    MAX_INTERFACE_ARTIFACT_FACTS * MAX_IMAGE_ARTIFACT_REPORT_BYTES + 1024 * 1024;
pub const MAX_INTERFACE_ARTIFACT_FACTS_BYTES: usize = 8 * 1024 * 1024;

const BUNDLE_DOMAIN: &[u8] = b"semaprax.interface-artifact-facts.digest.v1\0";
const INTERFACE_BYTES_DOMAIN: &[u8] =
    b"semaprax.interface-artifact-facts.interface-bytes.digest.v1\0";
const ARTIFACT_REPORT_DOMAIN: &[u8] =
    b"semaprax.interface-artifact-facts.artifact-report.digest.v1\0";

/// Exact compiler-owned public-interface bytes, when the selected Project
/// profile defines an existing canonical interface descriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceInterfaceFact {
    kind: &'static str,
    schema: &'static str,
    canonical_bytes: String,
    descriptor_digest: String,
    bytes_digest: String,
}

impl SourceInterfaceFact {
    pub fn kind(&self) -> &str {
        self.kind
    }
    pub fn schema(&self) -> &str {
        self.schema
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        self.canonical_bytes.as_bytes()
    }
    pub fn descriptor_digest(&self) -> &str {
        &self.descriptor_digest
    }
    pub fn bytes_digest(&self) -> &str {
        &self.bytes_digest
    }

    fn value(&self) -> Value {
        json!({
            "bytes": self.canonical_bytes.len(),
            "bytes_digest": self.bytes_digest,
            "canonical_bytes": self.canonical_bytes,
            "descriptor_digest": self.descriptor_digest,
            "kind": self.kind,
            "schema": self.schema,
        })
    }
}

/// Exact authority-free artifact-projection report derived by the existing
/// Project Semantic Image target machinery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedArtifactFact {
    kind: ImageArtifactKind,
    max_build_bytes: usize,
    report: String,
    report_digest: String,
}

impl GeneratedArtifactFact {
    pub fn kind(&self) -> ImageArtifactKind {
        self.kind
    }
    pub fn max_build_bytes(&self) -> usize {
        self.max_build_bytes
    }
    pub fn report(&self) -> &str {
        &self.report
    }
    pub fn report_digest(&self) -> &str {
        &self.report_digest
    }

    fn value(&self) -> Value {
        json!({
            "kind": self.kind.name(),
            "max_build_bytes": self.max_build_bytes,
            "report": self.report,
            "report_bytes": self.report.len(),
            "report_digest": self.report_digest,
        })
    }
}

/// A bounded, canonical fact bundle suitable for private retention by a
/// canonical workspace and for a future versioned ProgramRoot segment.
///
/// It is deliberately not inserted into Canonical Semantic Workspace Revision
/// v1 or ProgramRoot v1, whose fixed bytes and segment inventory stay intact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InterfaceArtifactFacts {
    source_interface: Option<SourceInterfaceFact>,
    artifact_projections: Vec<GeneratedArtifactFact>,
    project_revision: String,
    image_revision: String,
    json: String,
    digest: String,
}

impl InterfaceArtifactFacts {
    /// Derive exact interface and selected generated-artifact facts for one
    /// caller-bound Project revision. Artifact kinds must be unique and in the
    /// closed byte order `web`, `npm`, `openapi`, `c`.
    pub fn derive(
        revision: Arc<ProjectRevision>,
        expected_project_revision: &str,
        artifact_kinds: &[ImageArtifactKind],
        max_build_bytes: usize,
    ) -> Result<Self, Vec<Diagnostic>> {
        validate_digest(expected_project_revision)?;
        if expected_project_revision != revision.project_revision() {
            return Err(stale(
                "interface/artifact facts expected Project revision is stale",
            ));
        }
        validate_kind_inventory(artifact_kinds)?;
        if !(1024..=MAX_IMAGE_ARTIFACT_BUILD_BYTES).contains(&max_build_bytes) {
            return Err(invalid(
                "interface/artifact facts build limit is outside the host bound",
            ));
        }

        let source_interface = derive_source_interface(&revision)?;
        let image = ProjectSemanticImage::derive(revision.clone(), expected_project_revision)?;
        let mut total_input_bytes = source_interface
            .as_ref()
            .map_or(0, |fact| fact.canonical_bytes.len());
        let mut artifact_projections = Vec::with_capacity(artifact_kinds.len());
        for kind in artifact_kinds {
            let report = image.artifact_projection(image.image_digest(), *kind, max_build_bytes)?;
            image.verify_artifact_projection(
                image.image_digest(),
                *kind,
                max_build_bytes,
                report.as_bytes(),
            )?;
            total_input_bytes = total_input_bytes
                .checked_add(report.len())
                .ok_or_else(|| invalid("interface/artifact fact input byte count overflowed"))?;
            if total_input_bytes > MAX_INTERFACE_ARTIFACT_FACT_INPUT_BYTES {
                return Err(invalid(
                    "interface/artifact fact inputs exceed their aggregate byte limit",
                ));
            }
            artifact_projections.push(GeneratedArtifactFact {
                kind: *kind,
                max_build_bytes,
                report_digest: framed_digest(ARTIFACT_REPORT_DOMAIN, report.as_bytes()),
                report,
            });
        }

        let value = json!({
            "artifact_materialization": false,
            "artifact_projections": artifact_projections.iter().map(GeneratedArtifactFact::value).collect::<Vec<_>>(),
            "compatibility": INTERFACE_ARTIFACT_FACTS_COMPATIBILITY,
            "evidence_class": "exact_replayed_source_interface_and_pathless_artifact_projections",
            "image_revision": image.image_digest(),
            "limits": {
                "max_artifact_facts": MAX_INTERFACE_ARTIFACT_FACTS,
                "max_build_bytes": max_build_bytes,
                "max_bundle_bytes": MAX_INTERFACE_ARTIFACT_FACTS_BYTES,
                "max_input_bytes": MAX_INTERFACE_ARTIFACT_FACT_INPUT_BYTES,
            },
            "nonclaims": [
                "not_a_program_root_v1_segment_or_program_root_v2",
                "no_filesystem_or_artifact_materialization_authority",
                "no_execution_deployment_publication_or_external_consumer_evidence",
                "no_spx_agent_syntax_or_agent_definition_claim",
            ],
            "project_graph_digest": revision.semantic_graph_digest(),
            "project_revision": revision.project_revision(),
            "schema": INTERFACE_ARTIFACT_FACTS_SCHEMA,
            "source_authority": false,
            "source_interface": source_interface.as_ref().map(SourceInterfaceFact::value),
            "target_execution": false,
            "workspace_revision": revision.workspace_revision(),
        });
        let json = canonical_json(value)?;
        if json.len() > MAX_INTERFACE_ARTIFACT_FACTS_BYTES {
            return Err(invalid(
                "canonical interface/artifact facts exceed their byte limit",
            ));
        }
        let digest = framed_digest(BUNDLE_DOMAIN, json.as_bytes());
        Ok(Self {
            source_interface,
            artifact_projections,
            project_revision: revision.project_revision().to_owned(),
            image_revision: image.image_digest().to_owned(),
            json,
            digest,
        })
    }

    /// Rebuild every interface descriptor and artifact projection, then require
    /// exact bundle identity and bytes. Submitted JSON never becomes compiler
    /// state.
    pub fn replay(
        revision: Arc<ProjectRevision>,
        expected_project_revision: &str,
        artifact_kinds: &[ImageArtifactKind],
        max_build_bytes: usize,
        expected_digest: &str,
        bytes: &[u8],
    ) -> Result<Self, Vec<Diagnostic>> {
        validate_digest(expected_digest)?;
        if bytes.len() > MAX_INTERFACE_ARTIFACT_FACTS_BYTES {
            return Err(invalid(
                "submitted interface/artifact facts exceed their byte limit",
            ));
        }
        let source = std::str::from_utf8(bytes)
            .map_err(|_| invalid("submitted interface/artifact facts are not UTF-8"))?;
        let value: Value = serde_json::from_str(source)
            .map_err(|_| invalid("submitted interface/artifact facts are not JSON"))?;
        if canonical_json(value.clone())?.as_bytes() != bytes {
            return Err(invalid(
                "submitted interface/artifact facts are not canonical JSON",
            ));
        }
        validate_wire_shape(&value)?;
        let derived = Self::derive(
            revision,
            expected_project_revision,
            artifact_kinds,
            max_build_bytes,
        )?;
        if expected_digest != derived.digest || bytes != derived.json.as_bytes() {
            return Err(stale(
                "interface/artifact facts differ from exact Project replay",
            ));
        }
        Ok(derived)
    }

    pub fn source_interface(&self) -> Option<&SourceInterfaceFact> {
        self.source_interface.as_ref()
    }
    pub fn artifact_projections(&self) -> &[GeneratedArtifactFact] {
        &self.artifact_projections
    }
    pub fn project_revision(&self) -> &str {
        &self.project_revision
    }
    pub fn image_revision(&self) -> &str {
        &self.image_revision
    }
    pub fn to_json(&self) -> &str {
        &self.json
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

fn derive_source_interface(
    revision: &ProjectRevision,
) -> Result<Option<SourceInterfaceFact>, Vec<Diagnostic>> {
    let (kind, schema, bytes, descriptor_digest) = match revision.manifest().project_profile() {
        ProjectProfile::ScalarV1 => {
            let descriptor = revision.scalar_wit_interface_v1()?;
            (
                "scalar_wit",
                descriptor.schema(),
                descriptor.canonical_bytes(),
                descriptor.digest(),
            )
        }
        ProjectProfile::OwnedDataApiV1 => {
            let descriptor = revision.public_api_descriptor()?;
            (
                "owned_data_api",
                descriptor.schema(),
                descriptor.canonical_bytes(),
                descriptor.digest(),
            )
        }
        ProjectProfile::FlatOwnedRecordApiV1 => {
            let descriptor = revision.flat_owned_record_api_descriptor()?;
            (
                "flat_owned_record_api",
                super::FLAT_OWNED_RECORD_API_SCHEMA,
                descriptor.canonical_bytes(),
                descriptor.digest(),
            )
        }
        ProjectProfile::OwnedUtf8ApiV1 => {
            let descriptor = revision.owned_utf8_api_descriptor()?;
            (
                "owned_utf8_api",
                descriptor.schema(),
                descriptor.canonical_bytes(),
                descriptor.digest(),
            )
        }
        ProjectProfile::NestedOwnedRecordApiV1 => {
            let descriptor = revision.nested_owned_record_api_descriptor()?;
            (
                "nested_owned_record_api",
                super::NESTED_OWNED_RECORD_API_SCHEMA,
                descriptor.canonical_bytes(),
                descriptor.digest(),
            )
        }
        ProjectProfile::UsefulTextConsumerV1
        | ProjectProfile::UsefulDataV1
        | ProjectProfile::UsefulDataCommandV1
        | ProjectProfile::UsefulDataCommandV2
        | ProjectProfile::LanguageCommandIoV1
        | ProjectProfile::LineCommandIoV1
        | ProjectProfile::NetworkCommandIoV1
        | ProjectProfile::HttpsCommandIoV1 => return Ok(None),
    };
    let canonical_bytes = String::from_utf8(bytes)
        .map_err(|_| invalid("compiler-owned interface descriptor is not canonical UTF-8"))?;
    let bytes_digest = framed_digest(INTERFACE_BYTES_DOMAIN, canonical_bytes.as_bytes());
    Ok(Some(SourceInterfaceFact {
        kind,
        schema,
        canonical_bytes,
        descriptor_digest,
        bytes_digest,
    }))
}

fn validate_kind_inventory(kinds: &[ImageArtifactKind]) -> Result<(), Vec<Diagnostic>> {
    if kinds.is_empty() || kinds.len() > MAX_INTERFACE_ARTIFACT_FACTS {
        return Err(invalid(
            "interface/artifact facts require one to four artifact kinds",
        ));
    }
    let mut previous = None;
    for kind in kinds {
        let ordinal = match kind {
            ImageArtifactKind::Web => 0,
            ImageArtifactKind::Npm => 1,
            ImageArtifactKind::OpenApi => 2,
            ImageArtifactKind::C => 3,
        };
        if previous.is_some_and(|prior| prior >= ordinal) {
            return Err(invalid(
                "artifact kinds must be unique and ordered web, npm, openapi, c",
            ));
        }
        previous = Some(ordinal);
    }
    Ok(())
}

fn validate_wire_shape(value: &Value) -> Result<(), Vec<Diagnostic>> {
    require_keys(
        value,
        &[
            "artifact_materialization",
            "artifact_projections",
            "compatibility",
            "evidence_class",
            "image_revision",
            "limits",
            "nonclaims",
            "project_graph_digest",
            "project_revision",
            "schema",
            "source_authority",
            "source_interface",
            "target_execution",
            "workspace_revision",
        ],
    )?;
    if value["schema"] != INTERFACE_ARTIFACT_FACTS_SCHEMA
        || value["compatibility"] != INTERFACE_ARTIFACT_FACTS_COMPATIBILITY
        || value["artifact_materialization"] != false
        || value["source_authority"] != false
        || value["target_execution"] != false
        || !value["artifact_projections"].is_array()
        || !value["source_interface"].is_null() && !value["source_interface"].is_object()
    {
        return Err(invalid(
            "interface/artifact facts have an invalid wire shape",
        ));
    }
    require_keys(
        &value["limits"],
        &[
            "max_artifact_facts",
            "max_build_bytes",
            "max_bundle_bytes",
            "max_input_bytes",
        ],
    )?;
    if let Some(interface) = value["source_interface"].as_object() {
        require_object_keys(
            interface,
            &[
                "bytes",
                "bytes_digest",
                "canonical_bytes",
                "descriptor_digest",
                "kind",
                "schema",
            ],
        )?;
    }
    for artifact in value["artifact_projections"]
        .as_array()
        .expect("checked above")
    {
        require_keys(
            artifact,
            &[
                "kind",
                "max_build_bytes",
                "report",
                "report_bytes",
                "report_digest",
            ],
        )?;
    }
    Ok(())
}

fn require_keys(value: &Value, expected: &[&str]) -> Result<(), Vec<Diagnostic>> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("interface/artifact facts contain a non-object record"))?;
    require_object_keys(object, expected)
}

fn require_object_keys(
    object: &Map<String, Value>,
    expected: &[&str],
) -> Result<(), Vec<Diagnostic>> {
    if object.len() != expected.len() || !expected.iter().all(|key| object.contains_key(*key)) {
        return Err(invalid(
            "interface/artifact facts contain missing or unknown fields",
        ));
    }
    Ok(())
}

fn canonical_json(mut value: Value) -> Result<String, Vec<Diagnostic>> {
    sort_json(&mut value);
    let mut rendered = serde_json::to_string(&value)
        .map_err(|_| invalid("interface/artifact facts could not be serialized"))?;
    rendered.push('\n');
    Ok(rendered)
}

fn sort_json(value: &mut Value) {
    match value {
        Value::Object(object) => {
            let old = std::mem::take(object);
            let mut entries = old.into_iter().collect::<Vec<_>>();
            entries.sort_by(|left, right| left.0.as_bytes().cmp(right.0.as_bytes()));
            for (key, mut child) in entries {
                sort_json(&mut child);
                object.insert(key, child);
            }
        }
        Value::Array(values) => values.iter_mut().for_each(sort_json),
        _ => {}
    }
}

fn validate_digest(value: &str) -> Result<(), Vec<Diagnostic>> {
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(invalid(
            "interface/artifact facts require a lowercase sha256 digest",
        ));
    }
    Ok(())
}

fn framed_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(digest.finalize())
    )
}

fn invalid(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G552", message)]
}

fn stale(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G553", message)]
}
