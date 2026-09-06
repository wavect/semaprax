//! ProgramRoot v3: exact ProgramRoot v2 plus source-derived contract/test facts.
//!
//! The first eleven descriptors and all unbound relationships are retained
//! byte-for-byte from v2. The appended descriptor names an independently
//! replayable typed fact object; its payload is never copied into this root.

use serde_json::{json, Value};

use crate::diagnostic::Diagnostic;

use super::{
    canonical_json, exact_fields, exact_object, framed_digest, parse_canonical, text,
    validate_digest, with_field, without_field, ProgramRoot, ProgramRootDependencyLockAssociation,
    ProgramRootRelationship, ProgramRootSegment, ProgramRootV2,
    MAX_PROGRAM_ROOT_RELATIONSHIP_BYTES, MAX_PROGRAM_ROOT_SEGMENT_BYTES,
    PROGRAM_ROOT_SEGMENT_SCHEMA, PROGRAM_ROOT_V2_SCHEMA,
};
use crate::project::{
    ContractsAndTestsFacts, InterfaceArtifactFacts, SemanticWorkspaceRevision,
    CONTRACTS_AND_TESTS_FACTS_SCHEMA, MAX_CONTRACTS_AND_TESTS_FACTS_BYTES,
};

pub const PROGRAM_ROOT_V3_SCHEMA: &str = "semaprax.program-root.v3";
pub const PROGRAM_ROOT_V3_COMPATIBILITY: &str = "extends-semaprax.program-root.v2";
pub const MAX_PROGRAM_ROOT_V3_BYTES: usize = 768 * 1024;

const ROOT_V3_DOMAIN: &[u8] = b"semaprax.program-root.digest.v3\0";
const FACTS_SEGMENT_KIND: &str = "contracts_and_tests_facts";
const NONCLAIMS: [&str; 6] = [
    "additive_successor_of_program_root_v2",
    "first_eleven_segments_are_exact_program_root_v2_descriptors",
    "contracts_and_tests_facts_segment_is_a_descriptor_not_node_payload",
    "program_root_v1_v2_and_canonical_workspace_identities_are_unchanged",
    "runtime_root_relationships_are_unbound_acyclic_placeholders",
    "no_filesystem_network_execution_deployment_publication_or_commit_authority",
];

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramRootV3 {
    program_root_v3_digest: String,
    program_root_v2_digest: String,
    semantic_workspace_revision: String,
    segments: Vec<ProgramRootSegment>,
    relationships: Vec<ProgramRootRelationship>,
    json: String,
}

impl ProgramRootV3 {
    pub fn derive(
        workspace: &SemanticWorkspaceRevision,
        base_project_root: &ProgramRoot,
        interface_artifact_facts: &InterfaceArtifactFacts,
        dependency_lock_association: &ProgramRootDependencyLockAssociation,
        contracts_and_tests_facts: &ContractsAndTestsFacts,
    ) -> Result<Self> {
        let v2 = ProgramRootV2::derive(
            workspace,
            base_project_root,
            interface_artifact_facts,
            dependency_lock_association,
        )?;
        if contracts_and_tests_facts.project_revision()
            != interface_artifact_facts.project_revision()
        {
            return Err(stale(
                "ProgramRoot v3 facts do not share the ProgramRoot v2 Project subject",
            ));
        }
        let mut segments = v2.segments().to_vec();
        segments.push(ProgramRootSegment::derive(
            FACTS_SEGMENT_KIND,
            CONTRACTS_AND_TESTS_FACTS_SCHEMA,
            contracts_and_tests_facts.facts_digest(),
            contracts_and_tests_facts.to_json(),
        )?);
        let relationships = v2.relationships().to_vec();
        let segment_values = segments
            .iter()
            .map(|segment| parse_canonical(segment.to_json(), "ProgramRoot v3 segment"))
            .collect::<Result<Vec<_>>>()?;
        let relationship_values = relationships
            .iter()
            .map(|relationship| {
                parse_canonical(relationship.to_json(), "ProgramRoot v3 relationship")
            })
            .collect::<Result<Vec<_>>>()?;
        let payload = json!({
            "compatibility": PROGRAM_ROOT_V3_COMPATIBILITY,
            "limits": {
                "max_program_root_v3_bytes": MAX_PROGRAM_ROOT_V3_BYTES,
                "max_relationship_bytes": MAX_PROGRAM_ROOT_RELATIONSHIP_BYTES,
                "max_segment_bytes": MAX_PROGRAM_ROOT_SEGMENT_BYTES,
            },
            "nonclaims": NONCLAIMS,
            "program_root_v2_digest": v2.program_root_v2_digest(),
            "relationships": relationship_values,
            "schema": PROGRAM_ROOT_V3_SCHEMA,
            "segments": segment_values,
            "semantic_workspace_revision": workspace.workspace_revision(),
            "v2_program_root_schema": PROGRAM_ROOT_V2_SCHEMA,
        });
        let identity_bytes = canonical_json(payload.clone(), MAX_PROGRAM_ROOT_V3_BYTES)?;
        let program_root_v3_digest = framed_digest(ROOT_V3_DOMAIN, identity_bytes.as_bytes());
        let json = canonical_json(
            with_field(
                payload,
                "program_root_v3_digest",
                json!(program_root_v3_digest),
            ),
            MAX_PROGRAM_ROOT_V3_BYTES,
        )?;
        Ok(Self {
            program_root_v3_digest,
            program_root_v2_digest: v2.program_root_v2_digest().to_owned(),
            semantic_workspace_revision: workspace.workspace_revision().to_owned(),
            segments,
            relationships,
            json,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn replay(
        workspace: &SemanticWorkspaceRevision,
        base_project_root: &ProgramRoot,
        interface_artifact_facts: &InterfaceArtifactFacts,
        dependency_lock_association: &ProgramRootDependencyLockAssociation,
        contracts_and_tests_facts: &ContractsAndTestsFacts,
        expected_program_root_v3_digest: &str,
        bytes: &[u8],
    ) -> Result<Self> {
        validate_digest(expected_program_root_v3_digest)?;
        if bytes.len() > MAX_PROGRAM_ROOT_V3_BYTES {
            return Err(invalid("ProgramRoot v3 exceeds its byte limit"));
        }
        let source =
            std::str::from_utf8(bytes).map_err(|_| invalid("ProgramRoot v3 is not UTF-8"))?;
        let value: Value = serde_json::from_str(source)
            .map_err(|_| invalid("ProgramRoot v3 is not valid JSON"))?;
        if canonical_json(value.clone(), MAX_PROGRAM_ROOT_V3_BYTES)?.as_bytes() != bytes {
            return Err(invalid("ProgramRoot v3 is not exact canonical JSON"));
        }
        validate_wire_shape(&value)?;
        let derived = Self::derive(
            workspace,
            base_project_root,
            interface_artifact_facts,
            dependency_lock_association,
            contracts_and_tests_facts,
        )?;
        if expected_program_root_v3_digest != derived.program_root_v3_digest()
            || bytes != derived.to_json().as_bytes()
        {
            return Err(stale("ProgramRoot v3 failed exact replay"));
        }
        Ok(derived)
    }

    pub fn program_root_v3_digest(&self) -> &str {
        &self.program_root_v3_digest
    }
    pub fn program_root_v2_digest(&self) -> &str {
        &self.program_root_v2_digest
    }
    pub fn semantic_workspace_revision(&self) -> &str {
        &self.semantic_workspace_revision
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

fn validate_wire_shape(value: &Value) -> Result<()> {
    let object = exact_object(value, "ProgramRoot v3")?;
    exact_fields(
        object,
        &[
            "compatibility",
            "limits",
            "nonclaims",
            "program_root_v2_digest",
            "program_root_v3_digest",
            "relationships",
            "schema",
            "segments",
            "semantic_workspace_revision",
            "v2_program_root_schema",
        ],
        "ProgramRoot v3",
    )?;
    if value["schema"] != PROGRAM_ROOT_V3_SCHEMA
        || value["compatibility"] != PROGRAM_ROOT_V3_COMPATIBILITY
        || value["v2_program_root_schema"] != PROGRAM_ROOT_V2_SCHEMA
        || value["limits"]
            != json!({
                "max_program_root_v3_bytes": MAX_PROGRAM_ROOT_V3_BYTES,
                "max_relationship_bytes": MAX_PROGRAM_ROOT_RELATIONSHIP_BYTES,
                "max_segment_bytes": MAX_PROGRAM_ROOT_SEGMENT_BYTES,
            })
        || value["nonclaims"] != json!(NONCLAIMS)
    {
        return Err(invalid("ProgramRoot v3 fixed fields are invalid"));
    }
    for field in [
        "program_root_v2_digest",
        "program_root_v3_digest",
        "semantic_workspace_revision",
    ] {
        validate_digest(text(object, field, "ProgramRoot v3 digest is invalid")?)?;
    }
    let identity = framed_digest(
        ROOT_V3_DOMAIN,
        canonical_json(
            without_field(value, "program_root_v3_digest")?,
            MAX_PROGRAM_ROOT_V3_BYTES,
        )?
        .as_bytes(),
    );
    if value["program_root_v3_digest"] != identity {
        return Err(invalid(
            "ProgramRoot v3 identity does not authenticate its manifest",
        ));
    }
    validate_segments(&value["segments"])?;
    super::v2::validate_relationships(&value["relationships"])
        .map_err(|_| invalid("ProgramRoot v3 relationships are invalid"))?;
    Ok(())
}

fn validate_segments(value: &Value) -> Result<()> {
    let segments = value
        .as_array()
        .ok_or_else(|| invalid("ProgramRoot v3 segments are invalid"))?;
    if segments.len() != super::v2::SEGMENT_KINDS.len() + 1 {
        return Err(invalid("ProgramRoot v3 segment inventory is invalid"));
    }
    super::v2::validate_segments(&Value::Array(
        segments[..super::v2::SEGMENT_KINDS.len()].to_vec(),
    ))
    .map_err(|_| invalid("ProgramRoot v3 retained v2 segments are invalid"))?;
    for segment in segments {
        let object = exact_object(segment, "ProgramRoot v3 segment is invalid")?;
        let descriptor_identity = framed_digest(
            super::SEGMENT_DOMAIN,
            canonical_json(
                without_field(segment, "segment_digest")?,
                MAX_PROGRAM_ROOT_SEGMENT_BYTES,
            )?
            .as_bytes(),
        );
        if object["segment_digest"] != descriptor_identity {
            return Err(invalid(
                "ProgramRoot v3 segment identity is internally inconsistent",
            ));
        }
    }
    let facts = exact_object(
        &segments[super::v2::SEGMENT_KINDS.len()],
        "ProgramRoot v3 facts segment is invalid",
    )?;
    exact_fields(
        facts,
        &[
            "kind",
            "node_bytes",
            "node_digest",
            "node_schema",
            "schema",
            "segment_digest",
        ],
        "ProgramRoot v3 facts segment is invalid",
    )?;
    if facts["schema"] != PROGRAM_ROOT_SEGMENT_SCHEMA
        || facts["kind"] != FACTS_SEGMENT_KIND
        || facts["node_schema"] != CONTRACTS_AND_TESTS_FACTS_SCHEMA
        || facts["node_bytes"]
            .as_u64()
            .is_none_or(|bytes| bytes > MAX_CONTRACTS_AND_TESTS_FACTS_BYTES as u64)
    {
        return Err(invalid("ProgramRoot v3 facts segment is invalid"));
    }
    validate_digest(text(
        facts,
        "node_digest",
        "ProgramRoot v3 facts node digest is invalid",
    )?)?;
    validate_digest(text(
        facts,
        "segment_digest",
        "ProgramRoot v3 facts segment digest is invalid",
    )?)?;
    let segment_identity = framed_digest(
        super::SEGMENT_DOMAIN,
        canonical_json(
            without_field(&segments[super::v2::SEGMENT_KINDS.len()], "segment_digest")?,
            MAX_PROGRAM_ROOT_SEGMENT_BYTES,
        )?
        .as_bytes(),
    );
    if facts["segment_digest"] != segment_identity {
        return Err(invalid("ProgramRoot v3 facts segment identity is invalid"));
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G580", message.into())]
}

fn stale(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G581", message.into())]
}
