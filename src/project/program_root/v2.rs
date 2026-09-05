//! ProgramRoot v2: an additive root over the exact v1 semantic workspace,
//! interface/artifact facts, and admitted Project Lock association.
//!
//! The first nine descriptors are the exact ProgramRoot v1 descriptors. The
//! two appended descriptors refer to already-canonical typed facts; node
//! payloads are not copied into this manifest.

use serde_json::{json, Value};

use crate::diagnostic::Diagnostic;

use super::{
    canonical_json, exact_fields, exact_object, framed_digest, invalid, parse_canonical, stale,
    text, validate_digest, with_field, without_field, ProgramRoot, ProgramRootRelationship,
    ProgramRootSegment, MAX_PROGRAM_ROOT_RELATIONSHIP_BYTES, MAX_PROGRAM_ROOT_SEGMENT_BYTES,
    PROGRAM_ROOT_RELATIONSHIP_SCHEMA, PROGRAM_ROOT_SCHEMA, PROGRAM_ROOT_SEGMENT_SCHEMA,
};
use crate::project::{
    InterfaceArtifactFacts, ProgramRootDependencyLockAssociation, SemanticWorkspaceRevision,
    INTERFACE_ARTIFACT_FACTS_SCHEMA, PROGRAM_ROOT_DEPENDENCY_LOCK_ASSOCIATION_SCHEMA,
};

pub const PROGRAM_ROOT_V2_SCHEMA: &str = "semaprax.program-root.v2";
pub const PROGRAM_ROOT_V2_COMPATIBILITY: &str = "extends-semaprax.program-root.v1";
pub const MAX_PROGRAM_ROOT_V2_BYTES: usize = 512 * 1024;

const ROOT_V2_DOMAIN: &[u8] = b"semaprax.program-root.digest.v2\0";
const NONCLAIMS: [&str; 6] = [
    "additive_successor_of_program_root_v1",
    "base_project_root_and_semantic_workspace_root_are_distinct_explicit_anchors",
    "interface_and_lock_segments_are_exact_fact_descriptors_not_node_payloads",
    "project_lock_association_is_not_dependency_resolution",
    "runtime_root_relationships_are_unbound_acyclic_placeholders",
    "no_filesystem_network_execution_deployment_publication_or_commit_authority",
];
const SEGMENT_KINDS: [&str; 11] = [
    "source_projection",
    "semantic_program",
    "stable_identity_index",
    "dependency_closure",
    "contracts_and_tests",
    "agent_definitions",
    "authority_policies",
    "target_profiles",
    "projection_metadata",
    "interface_artifact_facts",
    "project_lock_association",
];
const RELATIONSHIPS: [(&str, &str); 3] = [
    ("deployment_root", "semaprax.deployment-root.v1"),
    ("instance_root", "semaprax.instance-root.v1"),
    ("evidence_root", "semaprax.evidence-root.v1"),
];

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

/// Versioned ProgramRoot successor retaining both relevant v1 root anchors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramRootV2 {
    program_root_v2_digest: String,
    semantic_workspace_revision: String,
    semantic_workspace_root_digest: String,
    base_project_root_digest: String,
    segments: Vec<ProgramRootSegment>,
    relationships: Vec<ProgramRootRelationship>,
    json: String,
}

impl ProgramRootV2 {
    pub fn derive(
        workspace: &SemanticWorkspaceRevision,
        base_project_root: &ProgramRoot,
        interface_artifact_facts: &InterfaceArtifactFacts,
        dependency_lock_association: &ProgramRootDependencyLockAssociation,
    ) -> Result<Self> {
        let semantic_workspace_root = workspace.program_root()?;
        validate_input_bindings(
            workspace,
            &semantic_workspace_root,
            base_project_root,
            interface_artifact_facts,
            dependency_lock_association,
        )?;

        let mut segments = semantic_workspace_root.segments().to_vec();
        segments.push(ProgramRootSegment::derive(
            "interface_artifact_facts",
            INTERFACE_ARTIFACT_FACTS_SCHEMA,
            interface_artifact_facts.digest(),
            interface_artifact_facts.to_json(),
        )?);
        segments.push(dependency_lock_association.program_root_segment()?);
        let relationships = semantic_workspace_root.relationships().to_vec();
        let segment_values = segments
            .iter()
            .map(|segment| parse_canonical(segment.to_json(), "ProgramRoot v2 segment"))
            .collect::<Result<Vec<_>>>()?;
        let relationship_values = relationships
            .iter()
            .map(|relationship| {
                parse_canonical(relationship.to_json(), "ProgramRoot v2 relationship")
            })
            .collect::<Result<Vec<_>>>()?;
        let payload = json!({
            "base_project_root_digest": base_project_root.program_root_digest(),
            "compatibility": PROGRAM_ROOT_V2_COMPATIBILITY,
            "limits": {
                "max_program_root_v2_bytes": MAX_PROGRAM_ROOT_V2_BYTES,
                "max_relationship_bytes": MAX_PROGRAM_ROOT_RELATIONSHIP_BYTES,
                "max_segment_bytes": MAX_PROGRAM_ROOT_SEGMENT_BYTES,
            },
            "nonclaims": NONCLAIMS,
            "relationships": relationship_values,
            "schema": PROGRAM_ROOT_V2_SCHEMA,
            "segments": segment_values,
            "semantic_workspace_revision": workspace.workspace_revision(),
            "semantic_workspace_root_digest": semantic_workspace_root.program_root_digest(),
            "v1_program_root_schema": PROGRAM_ROOT_SCHEMA,
        });
        let identity_bytes = canonical_json(payload.clone(), MAX_PROGRAM_ROOT_V2_BYTES)?;
        let program_root_v2_digest = framed_digest(ROOT_V2_DOMAIN, identity_bytes.as_bytes());
        let json = canonical_json(
            with_field(
                payload,
                "program_root_v2_digest",
                json!(program_root_v2_digest),
            ),
            MAX_PROGRAM_ROOT_V2_BYTES,
        )?;
        Ok(Self {
            program_root_v2_digest,
            semantic_workspace_revision: workspace.workspace_revision().to_owned(),
            semantic_workspace_root_digest: semantic_workspace_root
                .program_root_digest()
                .to_owned(),
            base_project_root_digest: base_project_root.program_root_digest().to_owned(),
            segments,
            relationships,
            json,
        })
    }

    pub fn replay(
        workspace: &SemanticWorkspaceRevision,
        base_project_root: &ProgramRoot,
        interface_artifact_facts: &InterfaceArtifactFacts,
        dependency_lock_association: &ProgramRootDependencyLockAssociation,
        expected_program_root_v2_digest: &str,
        bytes: &[u8],
    ) -> Result<Self> {
        validate_digest(expected_program_root_v2_digest)?;
        if bytes.len() > MAX_PROGRAM_ROOT_V2_BYTES {
            return Err(invalid("ProgramRoot v2 exceeds its byte limit"));
        }
        let source =
            std::str::from_utf8(bytes).map_err(|_| invalid("ProgramRoot v2 is not UTF-8"))?;
        let value: Value = serde_json::from_str(source)
            .map_err(|_| invalid("ProgramRoot v2 is not valid JSON"))?;
        if canonical_json(value.clone(), MAX_PROGRAM_ROOT_V2_BYTES)?.as_bytes() != bytes {
            return Err(invalid("ProgramRoot v2 is not exact canonical JSON"));
        }
        validate_wire_shape(&value)?;
        let derived = Self::derive(
            workspace,
            base_project_root,
            interface_artifact_facts,
            dependency_lock_association,
        )?;
        if expected_program_root_v2_digest != derived.program_root_v2_digest()
            || bytes != derived.to_json().as_bytes()
        {
            return Err(stale("ProgramRoot v2 failed exact replay"));
        }
        Ok(derived)
    }

    pub fn program_root_v2_digest(&self) -> &str {
        &self.program_root_v2_digest
    }

    pub fn semantic_workspace_revision(&self) -> &str {
        &self.semantic_workspace_revision
    }

    pub fn semantic_workspace_root_digest(&self) -> &str {
        &self.semantic_workspace_root_digest
    }

    pub fn base_project_root_digest(&self) -> &str {
        &self.base_project_root_digest
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

fn validate_input_bindings(
    workspace: &SemanticWorkspaceRevision,
    semantic_workspace_root: &ProgramRoot,
    base_project_root: &ProgramRoot,
    interface_artifact_facts: &InterfaceArtifactFacts,
    dependency_lock_association: &ProgramRootDependencyLockAssociation,
) -> Result<()> {
    if semantic_workspace_root.workspace_revision() != workspace.workspace_revision() {
        return Err(stale("ProgramRoot v2 semantic workspace root is stale"));
    }
    let agent_definitions = parse_canonical(
        workspace.agent_definitions().to_json(),
        "ProgramRoot v2 AgentDefinitions node is invalid",
    )?;
    let agent_payload = exact_object(
        &agent_definitions["payload"],
        "ProgramRoot v2 AgentDefinitions payload is invalid",
    )?;
    let definitions = agent_payload["definitions"]
        .as_array()
        .ok_or_else(|| invalid("ProgramRoot v2 AgentDefinitions inventory is invalid"))?;
    let integration = agent_payload["integration"].as_str();
    if definitions.is_empty()
        || !matches!(
            integration,
            Some(
                "explicit_compiler_admitted_association_input"
                    | "source_owned_spx_agent_declarations"
            )
        )
    {
        return Err(invalid(
            "ProgramRoot v2 requires non-empty compiler-admitted AgentDefinitions",
        ));
    }
    let project_revision = text(
        agent_payload,
        "expected_project_revision",
        "ProgramRoot v2 AgentDefinitions Project binding is invalid",
    )?;
    validate_digest(project_revision)?;

    let projection = parse_canonical(
        workspace.projection_metadata().to_json(),
        "ProgramRoot v2 projection metadata node is invalid",
    )?;
    let projection_payload = exact_object(
        &projection["payload"],
        "ProgramRoot v2 projection metadata payload is invalid",
    )?;
    let legacy_project_revision = text(
        projection_payload,
        "legacy_project_revision",
        "ProgramRoot v2 projection Project binding is invalid",
    )?;
    let legacy_workspace_revision = text(
        projection_payload,
        "legacy_workspace_revision",
        "ProgramRoot v2 projection workspace binding is invalid",
    )?;

    let facts = parse_canonical(
        interface_artifact_facts.to_json(),
        "ProgramRoot v2 interface/artifact facts are invalid",
    )?;
    let facts_object = exact_object(
        &facts,
        "ProgramRoot v2 interface/artifact facts are invalid",
    )?;
    let facts_project_revision = text(
        facts_object,
        "project_revision",
        "ProgramRoot v2 interface/artifact Project binding is invalid",
    )?;
    let facts_workspace_revision = text(
        facts_object,
        "workspace_revision",
        "ProgramRoot v2 interface/artifact workspace binding is invalid",
    )?;

    let association = parse_canonical(
        dependency_lock_association.to_json(),
        "ProgramRoot v2 dependency lock association is invalid",
    )?;
    let association_object = exact_object(
        &association,
        "ProgramRoot v2 dependency lock association is invalid",
    )?;
    let association_project_revision = text(
        association_object,
        "project_revision",
        "ProgramRoot v2 dependency lock Project binding is invalid",
    )?;
    let association_workspace_revision = text(
        association_object,
        "canonical_workspace_revision",
        "ProgramRoot v2 dependency lock workspace binding is invalid",
    )?;

    if project_revision != legacy_project_revision
        || project_revision != interface_artifact_facts.project_revision()
        || project_revision != facts_project_revision
        || project_revision != association_project_revision
        || facts_workspace_revision != legacy_workspace_revision
        || dependency_lock_association.program_root_digest()
            != base_project_root.program_root_digest()
        || association_workspace_revision != base_project_root.workspace_revision()
        || (integration == Some("explicit_compiler_admitted_association_input")
            && base_project_root.program_root_digest()
                == semantic_workspace_root.program_root_digest())
    {
        return Err(stale(
            "ProgramRoot v2 inputs do not share exact Project and root bindings",
        ));
    }
    Ok(())
}

fn validate_wire_shape(value: &Value) -> Result<()> {
    let object = exact_object(value, "ProgramRoot v2")?;
    exact_fields(
        object,
        &[
            "base_project_root_digest",
            "compatibility",
            "limits",
            "nonclaims",
            "program_root_v2_digest",
            "relationships",
            "schema",
            "segments",
            "semantic_workspace_revision",
            "semantic_workspace_root_digest",
            "v1_program_root_schema",
        ],
        "ProgramRoot v2",
    )?;
    if value["schema"] != PROGRAM_ROOT_V2_SCHEMA
        || value["compatibility"] != PROGRAM_ROOT_V2_COMPATIBILITY
        || value["v1_program_root_schema"] != PROGRAM_ROOT_SCHEMA
        || value["limits"]
            != json!({
                "max_program_root_v2_bytes": MAX_PROGRAM_ROOT_V2_BYTES,
                "max_relationship_bytes": MAX_PROGRAM_ROOT_RELATIONSHIP_BYTES,
                "max_segment_bytes": MAX_PROGRAM_ROOT_SEGMENT_BYTES,
            })
        || value["nonclaims"] != json!(NONCLAIMS)
    {
        return Err(invalid("ProgramRoot v2 fixed fields are invalid"));
    }
    for field in [
        "base_project_root_digest",
        "program_root_v2_digest",
        "semantic_workspace_revision",
        "semantic_workspace_root_digest",
    ] {
        validate_digest(text(object, field, "ProgramRoot v2 digest is invalid")?)?;
    }
    let identity = framed_digest(
        ROOT_V2_DOMAIN,
        canonical_json(
            without_field(value, "program_root_v2_digest")?,
            MAX_PROGRAM_ROOT_V2_BYTES,
        )?
        .as_bytes(),
    );
    if value["program_root_v2_digest"] != identity {
        return Err(invalid(
            "ProgramRoot v2 identity does not authenticate its manifest",
        ));
    }
    validate_segments(&value["segments"])?;
    validate_relationships(&value["relationships"])?;
    Ok(())
}

fn validate_segments(value: &Value) -> Result<()> {
    let segments = value
        .as_array()
        .ok_or_else(|| invalid("ProgramRoot v2 segments are invalid"))?;
    if segments.len() != SEGMENT_KINDS.len() {
        return Err(invalid("ProgramRoot v2 segment inventory is invalid"));
    }
    for (segment, expected_kind) in segments.iter().zip(SEGMENT_KINDS) {
        let object = exact_object(segment, "ProgramRoot v2 segment is invalid")?;
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
            "ProgramRoot v2 segment is invalid",
        )?;
        if segment["schema"] != PROGRAM_ROOT_SEGMENT_SCHEMA
            || segment["kind"] != expected_kind
            || object["node_bytes"].as_u64().is_none()
        {
            return Err(invalid("ProgramRoot v2 segment fixed fields are invalid"));
        }
        validate_digest(text(
            object,
            "node_digest",
            "ProgramRoot v2 node digest is invalid",
        )?)?;
        validate_digest(text(
            object,
            "segment_digest",
            "ProgramRoot v2 segment digest is invalid",
        )?)?;
    }
    if segments[9]["node_schema"] != INTERFACE_ARTIFACT_FACTS_SCHEMA
        || segments[10]["node_schema"] != PROGRAM_ROOT_DEPENDENCY_LOCK_ASSOCIATION_SCHEMA
    {
        return Err(invalid(
            "ProgramRoot v2 extension segment schemas are invalid",
        ));
    }
    Ok(())
}

fn validate_relationships(value: &Value) -> Result<()> {
    let relationships = value
        .as_array()
        .ok_or_else(|| invalid("ProgramRoot v2 relationships are invalid"))?;
    if relationships.len() != RELATIONSHIPS.len() {
        return Err(invalid("ProgramRoot v2 relationship inventory is invalid"));
    }
    for (relationship, (kind, expected_root_schema)) in relationships.iter().zip(RELATIONSHIPS) {
        let object = exact_object(relationship, "ProgramRoot v2 relationship is invalid")?;
        exact_fields(
            object,
            &[
                "binding",
                "digest",
                "expected_root_schema",
                "kind",
                "schema",
            ],
            "ProgramRoot v2 relationship is invalid",
        )?;
        if relationship["schema"] != PROGRAM_ROOT_RELATIONSHIP_SCHEMA
            || relationship["kind"] != kind
            || relationship["expected_root_schema"] != expected_root_schema
            || relationship["binding"] != "unbound"
            || !relationship["digest"].is_null()
        {
            return Err(invalid(
                "ProgramRoot v2 relationship is not an unbound placeholder",
            ));
        }
    }
    Ok(())
}
