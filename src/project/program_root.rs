//! Content-addressed ProgramRoot projection over one canonical workspace revision.
//!
//! This is an additive segmented view of [`SemanticWorkspaceRevision`], not a
//! second source/program representation. Segment descriptors reference the
//! existing typed node bytes and digests without deserializing or duplicating
//! their payloads.

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;

use super::{
    ProjectRevision, SemanticWorkspaceRevision, SEMANTIC_WORKSPACE_REVISION_COMPATIBILITY,
};

mod dependency_lock;
mod v2;
pub use dependency_lock::{
    ProgramRootDependencyLockAssociation, MAX_PROGRAM_ROOT_DEPENDENCY_LOCK_ASSOCIATION_BYTES,
    PROGRAM_ROOT_DEPENDENCY_LOCK_ASSOCIATION_SCHEMA,
};
pub use v2::{
    ProgramRootV2, MAX_PROGRAM_ROOT_V2_BYTES, PROGRAM_ROOT_V2_COMPATIBILITY, PROGRAM_ROOT_V2_SCHEMA,
};

pub const PROGRAM_ROOT_SCHEMA: &str = "semaprax.program-root.v1";
pub const PROGRAM_ROOT_SEGMENT_SCHEMA: &str = "semaprax.program-root.segment.v1";
pub const PROGRAM_ROOT_RELATIONSHIP_SCHEMA: &str = "semaprax.program-root.relationship.v1";
pub const PROGRAM_ROOT_COMPATIBILITY: &str =
    "extends-semaprax.semantic-workspace-revision-compatibility.v1";
pub const MAX_PROGRAM_ROOT_BYTES: usize = 256 * 1024;
pub const MAX_PROGRAM_ROOT_SEGMENT_BYTES: usize = 16 * 1024;
pub const MAX_PROGRAM_ROOT_RELATIONSHIP_BYTES: usize = 4 * 1024;

const ROOT_DOMAIN: &[u8] = b"semaprax.program-root.digest.v1\0";
const SEGMENT_DOMAIN: &[u8] = b"semaprax.program-root.segment.digest.v1\0";

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

/// One content-addressed descriptor for an existing canonical workspace node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramRootSegment {
    kind: &'static str,
    node_schema: &'static str,
    node_digest: String,
    node_bytes: usize,
    segment_digest: String,
    json: String,
}

impl ProgramRootSegment {
    fn derive(
        kind: &'static str,
        node_schema: &'static str,
        node_digest: &str,
        node_json: &str,
    ) -> Result<Self> {
        validate_digest(node_digest)?;
        let payload = json!({
            "kind": kind,
            "node_bytes": node_json.len(),
            "node_digest": node_digest,
            "node_schema": node_schema,
            "schema": PROGRAM_ROOT_SEGMENT_SCHEMA,
        });
        let payload_json = canonical_json(payload.clone(), MAX_PROGRAM_ROOT_SEGMENT_BYTES)?;
        let segment_digest = framed_digest(SEGMENT_DOMAIN, payload_json.as_bytes());
        let json = canonical_json(
            with_field(payload, "segment_digest", json!(segment_digest)),
            MAX_PROGRAM_ROOT_SEGMENT_BYTES,
        )?;
        Ok(Self {
            kind,
            node_schema,
            node_digest: node_digest.to_owned(),
            node_bytes: node_json.len(),
            segment_digest,
            json,
        })
    }

    pub const fn kind(&self) -> &'static str {
        self.kind
    }
    pub const fn node_schema(&self) -> &'static str {
        self.node_schema
    }
    pub fn node_digest(&self) -> &str {
        &self.node_digest
    }
    pub const fn node_bytes(&self) -> usize {
        self.node_bytes
    }
    pub fn segment_digest(&self) -> &str {
        &self.segment_digest
    }
    pub fn to_json(&self) -> &str {
        &self.json
    }
}

/// An explicitly unbound runtime-root relationship.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramRootRelationship {
    kind: &'static str,
    expected_root_schema: &'static str,
    json: String,
}

impl ProgramRootRelationship {
    fn unbound(kind: &'static str, expected_root_schema: &'static str) -> Result<Self> {
        let json = canonical_json(
            json!({
                "binding": "unbound",
                "digest": null,
                "expected_root_schema": expected_root_schema,
                "kind": kind,
                "schema": PROGRAM_ROOT_RELATIONSHIP_SCHEMA,
            }),
            MAX_PROGRAM_ROOT_RELATIONSHIP_BYTES,
        )?;
        Ok(Self {
            kind,
            expected_root_schema,
            json,
        })
    }

    pub const fn kind(&self) -> &'static str {
        self.kind
    }
    pub const fn binding(&self) -> &'static str {
        "unbound"
    }
    pub const fn expected_root_schema(&self) -> &'static str {
        self.expected_root_schema
    }
    pub fn digest(&self) -> Option<&str> {
        None
    }
    pub fn to_json(&self) -> &str {
        &self.json
    }
}

/// Small manifest and identity for the segmented source-owned ProgramRoot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramRoot {
    program_root: String,
    workspace_revision: String,
    segments: Vec<ProgramRootSegment>,
    relationships: Vec<ProgramRootRelationship>,
    json: String,
}

impl ProgramRoot {
    pub fn derive(workspace: &SemanticWorkspaceRevision) -> Result<Self> {
        let segments = vec![
            segment(
                "source_projection",
                workspace.source_projection().to_json(),
                workspace.source_projection().digest(),
                super::SourceProjection::SCHEMA,
            )?,
            segment(
                "semantic_program",
                workspace.semantic_program().to_json(),
                workspace.semantic_program().digest(),
                super::SemanticProgram::SCHEMA,
            )?,
            segment(
                "stable_identity_index",
                workspace.stable_identity_index().to_json(),
                workspace.stable_identity_index().digest(),
                super::StableIdentityIndex::SCHEMA,
            )?,
            segment(
                "dependency_closure",
                workspace.dependency_closure().to_json(),
                workspace.dependency_closure().digest(),
                super::DependencyClosure::SCHEMA,
            )?,
            segment(
                "contracts_and_tests",
                workspace.contracts_and_tests().to_json(),
                workspace.contracts_and_tests().digest(),
                super::ContractsAndTests::SCHEMA,
            )?,
            segment(
                "agent_definitions",
                workspace.agent_definitions().to_json(),
                workspace.agent_definitions().digest(),
                super::AgentDefinitions::SCHEMA,
            )?,
            segment(
                "authority_policies",
                workspace.authority_policies().to_json(),
                workspace.authority_policies().digest(),
                super::AuthorityPolicies::SCHEMA,
            )?,
            segment(
                "target_profiles",
                workspace.target_profiles().to_json(),
                workspace.target_profiles().digest(),
                super::TargetProfiles::SCHEMA,
            )?,
            segment(
                "projection_metadata",
                workspace.projection_metadata().to_json(),
                workspace.projection_metadata().digest(),
                super::ProjectionMetadata::SCHEMA,
            )?,
        ];
        let relationships = [
            ("deployment_root", "semaprax.deployment-root.v1"),
            ("instance_root", "semaprax.instance-root.v1"),
            ("evidence_root", "semaprax.evidence-root.v1"),
        ]
        .into_iter()
        .map(|(kind, schema)| ProgramRootRelationship::unbound(kind, schema))
        .collect::<Result<Vec<_>>>()?;
        let segment_values = segments
            .iter()
            .map(|segment| parse_canonical(segment.to_json(), "ProgramRoot segment"))
            .collect::<Result<Vec<_>>>()?;
        let relationship_values = relationships
            .iter()
            .map(|relationship| parse_canonical(relationship.to_json(), "ProgramRoot relationship"))
            .collect::<Result<Vec<_>>>()?;
        let payload = json!({
            "canonical_workspace_revision": workspace.workspace_revision(),
            "compatibility": PROGRAM_ROOT_COMPATIBILITY,
            "component_digests": {
                "dependency_lock": workspace.dependency_lock_digest(),
                "manifest": workspace.manifest_digest(),
                "semantic": workspace.semantic_digest(),
                "source_projection": workspace.source_projection_digest(),
            },
            "limits": {
                "max_program_root_bytes": MAX_PROGRAM_ROOT_BYTES,
                "max_relationship_bytes": MAX_PROGRAM_ROOT_RELATIONSHIP_BYTES,
                "max_segment_bytes": MAX_PROGRAM_ROOT_SEGMENT_BYTES,
            },
            "nonclaims": [
                "additive_projection_of_canonical_semantic_workspace_revision_v1",
                "no_node_payload_duplication_or_trusted_deserialization",
                "runtime_root_relationships_are_unbound_placeholders",
                "no_deployment_instance_evidence_execution_or_publication_authority",
                "no_durable_runtime_state",
            ],
            "relationships": relationship_values,
            "schema": PROGRAM_ROOT_SCHEMA,
            "segments": segment_values,
            "workspace_compatibility": SEMANTIC_WORKSPACE_REVISION_COMPATIBILITY,
        });
        let identity_bytes = canonical_json(payload.clone(), MAX_PROGRAM_ROOT_BYTES)?;
        let program_root = framed_digest(ROOT_DOMAIN, identity_bytes.as_bytes());
        let json = canonical_json(
            with_field(payload, "program_root", json!(program_root)),
            MAX_PROGRAM_ROOT_BYTES,
        )?;
        Ok(Self {
            program_root,
            workspace_revision: workspace.workspace_revision().to_owned(),
            segments,
            relationships,
            json,
        })
    }

    pub fn replay(
        workspace: &SemanticWorkspaceRevision,
        expected_program_root: &str,
        bytes: &[u8],
    ) -> Result<Self> {
        if bytes.len() > MAX_PROGRAM_ROOT_BYTES {
            return Err(invalid("ProgramRoot exceeds its byte limit"));
        }
        validate_digest(expected_program_root)?;
        let source = std::str::from_utf8(bytes).map_err(|_| invalid("ProgramRoot is not UTF-8"))?;
        let value: Value =
            serde_json::from_str(source).map_err(|_| invalid("ProgramRoot is not valid JSON"))?;
        if canonical_json(value.clone(), MAX_PROGRAM_ROOT_BYTES)?.as_bytes() != bytes {
            return Err(invalid("ProgramRoot is not exact canonical JSON"));
        }
        validate_wire_shape(&value)?;
        let derived = Self::derive(workspace)?;
        if expected_program_root != derived.program_root() || bytes != derived.to_json().as_bytes()
        {
            return Err(stale("ProgramRoot failed exact replay"));
        }
        Ok(derived)
    }

    pub fn program_root(&self) -> &str {
        &self.program_root
    }
    pub fn program_root_digest(&self) -> &str {
        &self.program_root
    }
    pub fn workspace_revision(&self) -> &str {
        &self.workspace_revision
    }
    pub fn segments(&self) -> &[ProgramRootSegment] {
        &self.segments
    }
    pub fn segment(&self, kind: &str) -> Option<&ProgramRootSegment> {
        self.segments.iter().find(|segment| segment.kind() == kind)
    }
    pub fn relationships(&self) -> &[ProgramRootRelationship] {
        &self.relationships
    }
    pub fn to_json(&self) -> &str {
        &self.json
    }
}

impl SemanticWorkspaceRevision {
    /// Derive the segmented ProgramRoot view without changing this revision's bytes or identity.
    pub fn program_root(&self) -> Result<ProgramRoot> {
        ProgramRoot::derive(self)
    }
}

impl ProjectRevision {
    /// Derive the canonical workspace revision and its additive ProgramRoot.
    pub fn program_root(&self) -> Result<ProgramRoot> {
        self.canonical_workspace_revision()?.program_root()
    }
}

fn segment(
    kind: &'static str,
    node_json: &str,
    node_digest: &str,
    node_schema: &'static str,
) -> Result<ProgramRootSegment> {
    ProgramRootSegment::derive(kind, node_schema, node_digest, node_json)
}

fn validate_wire_shape(value: &Value) -> Result<()> {
    let object = exact_object(value, "ProgramRoot")?;
    let fields = [
        "canonical_workspace_revision",
        "compatibility",
        "component_digests",
        "limits",
        "nonclaims",
        "program_root",
        "relationships",
        "schema",
        "segments",
        "workspace_compatibility",
    ];
    exact_fields(object, &fields, "ProgramRoot")?;
    if value["schema"] != PROGRAM_ROOT_SCHEMA
        || value["compatibility"] != PROGRAM_ROOT_COMPATIBILITY
        || value["workspace_compatibility"] != SEMANTIC_WORKSPACE_REVISION_COMPATIBILITY
    {
        return Err(invalid("ProgramRoot fixed fields are invalid"));
    }
    validate_digest(text(object, "program_root", "ProgramRoot identity")?)?;
    let identity = framed_digest(
        ROOT_DOMAIN,
        canonical_json(
            without_field(value, "program_root")?,
            MAX_PROGRAM_ROOT_BYTES,
        )?
        .as_bytes(),
    );
    if value["program_root"] != identity {
        return Err(invalid(
            "ProgramRoot identity does not authenticate its manifest",
        ));
    }
    validate_digest(text(
        object,
        "canonical_workspace_revision",
        "ProgramRoot workspace revision",
    )?)?;
    let components = exact_object(&value["component_digests"], "ProgramRoot components")?;
    exact_fields(
        components,
        &[
            "dependency_lock",
            "manifest",
            "semantic",
            "source_projection",
        ],
        "ProgramRoot components",
    )?;
    for digest in components.values() {
        validate_digest(
            digest
                .as_str()
                .ok_or_else(|| invalid("ProgramRoot component digest is invalid"))?,
        )?;
    }
    validate_segments(&value["segments"])?;
    validate_relationships(&value["relationships"])?;
    Ok(())
}

fn validate_segments(value: &Value) -> Result<()> {
    const KINDS: [&str; 9] = [
        "source_projection",
        "semantic_program",
        "stable_identity_index",
        "dependency_closure",
        "contracts_and_tests",
        "agent_definitions",
        "authority_policies",
        "target_profiles",
        "projection_metadata",
    ];
    let segments = value
        .as_array()
        .ok_or_else(|| invalid("ProgramRoot segments are invalid"))?;
    if segments.len() != KINDS.len() {
        return Err(invalid("ProgramRoot segment inventory is invalid"));
    }
    for (segment, kind) in segments.iter().zip(KINDS) {
        let object = exact_object(segment, "ProgramRoot segment")?;
        exact_fields(
            object,
            &[
                "kind",
                "node_bytes",
                "node_digest",
                "node_schema",
                "schema",
                "segment_digest",
            ],
            "ProgramRoot segment",
        )?;
        if segment["schema"] != PROGRAM_ROOT_SEGMENT_SCHEMA || segment["kind"] != kind {
            return Err(invalid("ProgramRoot segment fixed fields are invalid"));
        }
        validate_digest(text(object, "node_digest", "ProgramRoot node digest")?)?;
        validate_digest(text(
            object,
            "segment_digest",
            "ProgramRoot segment digest",
        )?)?;
        let descriptor_digest = framed_digest(
            SEGMENT_DOMAIN,
            canonical_json(
                without_field(segment, "segment_digest")?,
                MAX_PROGRAM_ROOT_SEGMENT_BYTES,
            )?
            .as_bytes(),
        );
        if segment["segment_digest"] != descriptor_digest {
            return Err(invalid(
                "ProgramRoot segment digest does not authenticate its descriptor",
            ));
        }
        if object["node_schema"].as_str().is_none() || object["node_bytes"].as_u64().is_none() {
            return Err(invalid("ProgramRoot segment node descriptor is invalid"));
        }
    }
    Ok(())
}

fn validate_relationships(value: &Value) -> Result<()> {
    const ROOTS: [(&str, &str); 3] = [
        ("deployment_root", "semaprax.deployment-root.v1"),
        ("instance_root", "semaprax.instance-root.v1"),
        ("evidence_root", "semaprax.evidence-root.v1"),
    ];
    let relationships = value
        .as_array()
        .ok_or_else(|| invalid("ProgramRoot relationships are invalid"))?;
    if relationships.len() != ROOTS.len() {
        return Err(invalid("ProgramRoot relationship inventory is invalid"));
    }
    for (relationship, (kind, expected_schema)) in relationships.iter().zip(ROOTS) {
        let object = exact_object(relationship, "ProgramRoot relationship")?;
        exact_fields(
            object,
            &[
                "binding",
                "digest",
                "expected_root_schema",
                "kind",
                "schema",
            ],
            "ProgramRoot relationship",
        )?;
        if relationship["schema"] != PROGRAM_ROOT_RELATIONSHIP_SCHEMA
            || relationship["kind"] != kind
            || relationship["expected_root_schema"] != expected_schema
            || relationship["binding"] != "unbound"
            || !relationship["digest"].is_null()
        {
            return Err(invalid(
                "ProgramRoot relationship is not an unbound placeholder",
            ));
        }
    }
    Ok(())
}

fn parse_canonical(source: &str, subject: &'static str) -> Result<Value> {
    serde_json::from_str(source).map_err(|_| invalid(subject))
}

fn with_field(value: Value, key: &str, field: Value) -> Value {
    let mut object = value
        .as_object()
        .expect("ProgramRoot construction always uses an object")
        .clone();
    object.insert(key.to_owned(), field);
    Value::Object(object)
}

fn without_field(value: &Value, key: &str) -> Result<Value> {
    let mut object = exact_object(value, "ProgramRoot digest subject")?.clone();
    if object.remove(key).is_none() {
        return Err(invalid(
            "ProgramRoot digest subject lacks its identity field",
        ));
    }
    Ok(Value::Object(object))
}

fn exact_object<'a>(value: &'a Value, subject: &'static str) -> Result<&'a Map<String, Value>> {
    value.as_object().ok_or_else(|| invalid(subject))
}

fn exact_fields(object: &Map<String, Value>, fields: &[&str], subject: &'static str) -> Result<()> {
    if object.len() != fields.len() || fields.iter().any(|field| !object.contains_key(*field)) {
        return Err(invalid(subject));
    }
    Ok(())
}

fn text<'a>(object: &'a Map<String, Value>, key: &str, subject: &'static str) -> Result<&'a str> {
    object[key].as_str().ok_or_else(|| invalid(subject))
}

fn validate_digest(value: &str) -> Result<()> {
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(invalid("ProgramRoot digest is invalid"));
    }
    Ok(())
}

fn canonical_json(mut value: Value, maximum: usize) -> Result<String> {
    value.sort_all_objects();
    let mut output = serde_json::to_string(&value)
        .map_err(|_| invalid("ProgramRoot value cannot be rendered"))?;
    output.push('\n');
    if output.len() > maximum {
        return Err(invalid("ProgramRoot value exceeds its byte limit"));
    }
    Ok(output)
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

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G550", message)]
}

fn stale(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G551", message)]
}
