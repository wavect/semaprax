//! Canonical semantic workspace identity over one admitted Project revision.
//!
//! This module is authority-free. It projects retained compiler facts, never
//! reads paths, and exact replay always rebuilds from the supplied revision.

use std::path::Path;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;

use super::{ProjectRevision, PACKAGE_TARGET_NATIVE64, PACKAGE_TARGET_WASM32};

pub const SEMANTIC_WORKSPACE_REVISION_SCHEMA: &str = "semaprax.semantic-workspace-revision.v1";
pub const SEMANTIC_WORKSPACE_REVISION_COMPATIBILITY: &str =
    "semaprax.semantic-workspace-revision-compatibility.v1";
pub const MAX_SEMANTIC_WORKSPACE_REVISION_BYTES: usize = 32 * 1024 * 1024;

const REVISION_DOMAIN: &[u8] = b"semaprax.semantic-workspace-revision.digest.v1\0";
const SEMANTIC_DOMAIN: &[u8] = b"semaprax.semantic-workspace-revision.semantic.digest.v1\0";
const MANIFEST_DOMAIN: &[u8] = b"semaprax.semantic-workspace-revision.manifest.digest.v1\0";
const DEPENDENCY_LOCK_DOMAIN: &[u8] =
    b"semaprax.semantic-workspace-revision.dependency-lock.digest.v1\0";
const NORMALIZED_SOURCE_DOMAIN: &[u8] =
    b"semaprax.semantic-workspace-revision.normalized-source.digest.v1\0";
const PRELUDE_DOMAIN: &[u8] = b"semaprax.semantic-workspace-revision.prelude.digest.v1\0";

macro_rules! node_type {
    ($name:ident, $schema:literal, $domain:literal) => {
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub struct $name {
            json: String,
            digest: String,
        }

        impl $name {
            pub const SCHEMA: &'static str = $schema;
            const DOMAIN: &'static [u8] = $domain;

            fn new(payload: Value) -> Result<Self, Vec<Diagnostic>> {
                let json = canonical_json(json!({"schema": Self::SCHEMA, "payload": payload}))?;
                let digest = framed_digest(Self::DOMAIN, json.as_bytes());
                Ok(Self { json, digest })
            }

            /// Canonical compact JSON with exactly one terminal LF.
            pub fn to_json(&self) -> &str {
                &self.json
            }

            /// Domain-separated digest of the exact canonical node bytes.
            pub fn digest(&self) -> &str {
                &self.digest
            }
        }
    };
}

node_type!(
    SourceProjection,
    "semaprax.semantic-workspace-revision.source-projection.v1",
    b"semaprax.semantic-workspace-revision.source-projection.digest.v1\0"
);
node_type!(
    SemanticProgram,
    "semaprax.semantic-workspace-revision.semantic-program.v1",
    b"semaprax.semantic-workspace-revision.semantic-program.digest.v1\0"
);
node_type!(
    StableIdentityIndex,
    "semaprax.semantic-workspace-revision.stable-identity-index.v1",
    b"semaprax.semantic-workspace-revision.stable-identity-index.digest.v1\0"
);
node_type!(
    DependencyClosure,
    "semaprax.semantic-workspace-revision.dependency-closure.v1",
    b"semaprax.semantic-workspace-revision.dependency-closure.digest.v1\0"
);
node_type!(
    ContractsAndTests,
    "semaprax.semantic-workspace-revision.contracts-and-tests.v1",
    b"semaprax.semantic-workspace-revision.contracts-and-tests.digest.v1\0"
);
node_type!(
    AgentDefinitions,
    "semaprax.semantic-workspace-revision.agent-definitions.v1",
    b"semaprax.semantic-workspace-revision.agent-definitions.digest.v1\0"
);
node_type!(
    AuthorityPolicies,
    "semaprax.semantic-workspace-revision.authority-policies.v1",
    b"semaprax.semantic-workspace-revision.authority-policies.digest.v1\0"
);
node_type!(
    TargetProfiles,
    "semaprax.semantic-workspace-revision.target-profiles.v1",
    b"semaprax.semantic-workspace-revision.target-profiles.digest.v1\0"
);
node_type!(
    ProjectionMetadata,
    "semaprax.semantic-workspace-revision.projection-metadata.v1",
    b"semaprax.semantic-workspace-revision.projection-metadata.digest.v1\0"
);

/// One immutable composite identity and its nine typed canonical projections.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticWorkspaceRevision {
    source_projection: SourceProjection,
    semantic_program: SemanticProgram,
    stable_identity_index: StableIdentityIndex,
    dependency_closure: DependencyClosure,
    contracts_and_tests: ContractsAndTests,
    agent_definitions: AgentDefinitions,
    authority_policies: AuthorityPolicies,
    target_profiles: TargetProfiles,
    projection_metadata: ProjectionMetadata,
    semantic_digest: String,
    source_projection_digest: String,
    manifest_digest: String,
    dependency_lock_digest: String,
    workspace_revision: String,
    json: String,
}

impl SemanticWorkspaceRevision {
    pub fn derive(revision: &ProjectRevision) -> Result<Self, Vec<Diagnostic>> {
        let source_projection = SourceProjection::new(json!({
            "files": revision.sources().iter().map(|source| json!({
                "bytes": source.source().len(),
                "path": source.path(),
                "source_digest": source.source_digest(),
                "source_graph_schema": source.source_graph_schema(),
                "source_revision": source.source_revision(),
            })).collect::<Vec<_>>(),
            "workspace_manifest": revision.workspace_manifest(),
        }))?;

        let normalized_sources = revision
            .sources()
            .iter()
            .map(|source| {
                let (program, _) =
                    crate::parse_with_comments(source.source(), Path::new(source.path()))
                        .map_err(|error| vec![error])?;
                let normalized = crate::format::canonical(&program);
                Ok(json!({
                    "path": source.path(),
                    "semantic_source_digest": framed_digest(
                        NORMALIZED_SOURCE_DOMAIN,
                        normalized.as_bytes(),
                    ),
                }))
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
        let semantic_program = SemanticProgram::new(json!({
            "entry_module": revision.manifest().entry(),
            "normalized_sources": normalized_sources,
            "prelude_digest": framed_digest(PRELUDE_DOMAIN, &crate::prelude::contract_bytes_v1()),
        }))?;

        let indexes = revision.semantic.image_indexes();
        let stable_identity_index = StableIdentityIndex::new(json!({
            "stable_ids": indexes.get("stable_ids").cloned().unwrap_or_else(|| json!([])),
        }))?;

        let dependency_sources = revision
            .sources()
            .iter()
            .filter(|source| source.path().starts_with("dependencies/"))
            .map(|source| {
                json!({
                    "path": source.path(),
                    "source_digest": source.source_digest(),
                    "source_revision": source.source_revision(),
                })
            })
            .collect::<Vec<_>>();
        let dependency_closure = DependencyClosure::new(json!({
            "declared_source_bindings": revision.manifest().dependency_sources().iter().map(|source| json!({
                "name": source.name(), "path": source.path(),
            })).collect::<Vec<_>>(),
            "requirements": revision.manifest().dependencies().iter().map(|dependency| json!({
                "name": dependency.name(), "range": dependency.range(),
            })).collect::<Vec<_>>(),
            "resolved_sources": dependency_sources,
        }))?;

        let contracts_and_tests = ContractsAndTests::new(json!({
            "contract_fingerprints": semantic_program.digest(),
            "test_module": revision.manifest().test_module(),
        }))?;
        let agent_definitions = AgentDefinitions::new(json!({
            "definitions": [],
            "integration": "no_project_agent_definition_declarations",
        }))?;
        let authority_policies = AuthorityPolicies::new(json!({
            "required_capabilities": revision.manifest().capabilities(),
        }))?;
        let targets = revision.manifest().target_matrix().map_or_else(
            || {
                vec![
                    PACKAGE_TARGET_NATIVE64.to_owned(),
                    PACKAGE_TARGET_WASM32.to_owned(),
                ]
            },
            <[String]>::to_vec,
        );
        let target_profiles = TargetProfiles::new(json!({
            "contract": revision.manifest().schema(),
            "profile": revision.manifest().profile().unwrap_or("scalar"),
            "targets": targets,
            "web_exports": revision.manifest().web_exports(),
        }))?;
        let projection_metadata = ProjectionMetadata::new(json!({
            "compiler_package": env!("CARGO_PKG_NAME"),
            "compiler_version": env!("CARGO_PKG_VERSION"),
            "compatibility": SEMANTIC_WORKSPACE_REVISION_COMPATIBILITY,
            "legacy_project_revision": revision.project_revision(),
            "legacy_workspace_revision": revision.workspace_revision(),
            "project_graph_digest": revision.semantic_graph_digest(),
        }))?;

        let semantic_digest = digest_sequence(
            SEMANTIC_DOMAIN,
            [
                semantic_program.digest(),
                stable_identity_index.digest(),
                contracts_and_tests.digest(),
                agent_definitions.digest(),
                authority_policies.digest(),
                target_profiles.digest(),
            ],
        );
        let source_projection_digest = source_projection.digest().to_owned();
        let manifest_digest = framed_digest(
            MANIFEST_DOMAIN,
            revision.manifest().to_canonical_toml().as_bytes(),
        );
        let dependency_lock_digest = framed_digest(
            DEPENDENCY_LOCK_DOMAIN,
            dependency_closure.to_json().as_bytes(),
        );
        let workspace_revision = digest_sequence(
            REVISION_DOMAIN,
            [
                semantic_digest.as_str(),
                source_projection_digest.as_str(),
                manifest_digest.as_str(),
                dependency_lock_digest.as_str(),
            ],
        );

        let nodes = json!({
            "agent_definitions": node_value(&agent_definitions.json, agent_definitions.digest())?,
            "authority_policies": node_value(&authority_policies.json, authority_policies.digest())?,
            "contracts_and_tests": node_value(&contracts_and_tests.json, contracts_and_tests.digest())?,
            "dependency_closure": node_value(&dependency_closure.json, dependency_closure.digest())?,
            "projection_metadata": node_value(&projection_metadata.json, projection_metadata.digest())?,
            "semantic_program": node_value(&semantic_program.json, semantic_program.digest())?,
            "source_projection": node_value(&source_projection.json, source_projection.digest())?,
            "stable_identity_index": node_value(&stable_identity_index.json, stable_identity_index.digest())?,
            "target_profiles": node_value(&target_profiles.json, target_profiles.digest())?,
        });
        let json = canonical_json(json!({
            "compatibility": SEMANTIC_WORKSPACE_REVISION_COMPATIBILITY,
            "digests": {
                "dependency_lock": dependency_lock_digest,
                "manifest": manifest_digest,
                "semantic": semantic_digest,
                "source_projection": source_projection_digest,
            },
            "limits": {"max_revision_bytes": MAX_SEMANTIC_WORKSPACE_REVISION_BYTES},
            "nodes": nodes,
            "nonclaims": [
                "no_filesystem_or_publication_authority",
                "no_trusted_hir_deserialization",
                "no_project_agent_definition_integration",
                "dependency_lock_is_a_local_admitted_closure_projection_not_project_lock_v1",
            ],
            "schema": SEMANTIC_WORKSPACE_REVISION_SCHEMA,
            "workspace_revision": workspace_revision,
        }))?;

        Ok(Self {
            source_projection,
            semantic_program,
            stable_identity_index,
            dependency_closure,
            contracts_and_tests,
            agent_definitions,
            authority_policies,
            target_profiles,
            projection_metadata,
            semantic_digest,
            source_projection_digest,
            manifest_digest,
            dependency_lock_digest,
            workspace_revision,
            json,
        })
    }

    pub fn replay(
        revision: &ProjectRevision,
        expected_workspace_revision: &str,
        bytes: &[u8],
    ) -> Result<Self, Vec<Diagnostic>> {
        if bytes.len() > MAX_SEMANTIC_WORKSPACE_REVISION_BYTES {
            return Err(invalid(
                "canonical semantic workspace revision exceeds its byte limit",
            ));
        }
        validate_digest(expected_workspace_revision)?;
        let source = std::str::from_utf8(bytes)
            .map_err(|_| invalid("canonical semantic workspace revision is not UTF-8"))?;
        let value: Value = serde_json::from_str(source)
            .map_err(|_| invalid("canonical semantic workspace revision is not JSON"))?;
        if canonical_json(value.clone())?.as_bytes() != bytes {
            return Err(invalid(
                "canonical semantic workspace revision is not canonical JSON",
            ));
        }
        validate_wire_shape(&value)?;
        let derived = Self::derive(revision)?;
        if expected_workspace_revision != derived.workspace_revision()
            || bytes != derived.json.as_bytes()
        {
            return Err(stale(
                "canonical semantic workspace revision does not match exact replay",
            ));
        }
        Ok(derived)
    }

    pub fn to_json(&self) -> &str {
        &self.json
    }
    pub fn workspace_revision(&self) -> &str {
        &self.workspace_revision
    }
    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }
    pub fn source_projection_digest(&self) -> &str {
        &self.source_projection_digest
    }
    pub fn manifest_digest(&self) -> &str {
        &self.manifest_digest
    }
    pub fn dependency_lock_digest(&self) -> &str {
        &self.dependency_lock_digest
    }
    pub fn source_projection(&self) -> &SourceProjection {
        &self.source_projection
    }
    pub fn semantic_program(&self) -> &SemanticProgram {
        &self.semantic_program
    }
    pub fn stable_identity_index(&self) -> &StableIdentityIndex {
        &self.stable_identity_index
    }
    pub fn dependency_closure(&self) -> &DependencyClosure {
        &self.dependency_closure
    }
    pub fn contracts_and_tests(&self) -> &ContractsAndTests {
        &self.contracts_and_tests
    }
    pub fn agent_definitions(&self) -> &AgentDefinitions {
        &self.agent_definitions
    }
    pub fn authority_policies(&self) -> &AuthorityPolicies {
        &self.authority_policies
    }
    pub fn target_profiles(&self) -> &TargetProfiles {
        &self.target_profiles
    }
    pub fn projection_metadata(&self) -> &ProjectionMetadata {
        &self.projection_metadata
    }
}

fn validate_wire_shape(value: &Value) -> Result<(), Vec<Diagnostic>> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("canonical semantic workspace revision is not an object"))?;
    let expected = [
        "compatibility",
        "digests",
        "limits",
        "nodes",
        "nonclaims",
        "schema",
        "workspace_revision",
    ];
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        return Err(invalid(
            "canonical semantic workspace revision has an invalid field set",
        ));
    }
    if value["schema"] != SEMANTIC_WORKSPACE_REVISION_SCHEMA
        || value["compatibility"] != SEMANTIC_WORKSPACE_REVISION_COMPATIBILITY
    {
        return Err(invalid(
            "canonical semantic workspace revision has an invalid schema",
        ));
    }
    validate_digest(
        value["workspace_revision"]
            .as_str()
            .ok_or_else(|| invalid("canonical semantic workspace revision digest is invalid"))?,
    )?;
    let digests = value["digests"]
        .as_object()
        .ok_or_else(|| invalid("canonical semantic workspace digest set is invalid"))?;
    let digest_keys = [
        "dependency_lock",
        "manifest",
        "semantic",
        "source_projection",
    ];
    if digests.len() != digest_keys.len()
        || digest_keys.iter().any(|key| !digests.contains_key(*key))
    {
        return Err(invalid(
            "canonical semantic workspace digest set is invalid",
        ));
    }
    for key in digest_keys {
        validate_digest(
            digests[key]
                .as_str()
                .ok_or_else(|| invalid("canonical semantic workspace digest is invalid"))?,
        )?;
    }
    let nodes = value["nodes"]
        .as_object()
        .ok_or_else(|| invalid("canonical semantic workspace node set is invalid"))?;
    let node_schemas = [
        ("agent_definitions", AgentDefinitions::SCHEMA),
        ("authority_policies", AuthorityPolicies::SCHEMA),
        ("contracts_and_tests", ContractsAndTests::SCHEMA),
        ("dependency_closure", DependencyClosure::SCHEMA),
        ("projection_metadata", ProjectionMetadata::SCHEMA),
        ("semantic_program", SemanticProgram::SCHEMA),
        ("source_projection", SourceProjection::SCHEMA),
        ("stable_identity_index", StableIdentityIndex::SCHEMA),
        ("target_profiles", TargetProfiles::SCHEMA),
    ];
    if nodes.len() != node_schemas.len()
        || node_schemas
            .iter()
            .any(|(key, _)| !nodes.contains_key(*key))
    {
        return Err(invalid("canonical semantic workspace node set is invalid"));
    }
    for (key, schema) in node_schemas {
        let node = nodes[key]
            .as_object()
            .ok_or_else(|| invalid("canonical semantic workspace node is invalid"))?;
        if node.len() != 2 || !node.contains_key("digest") || !node.contains_key("value") {
            return Err(invalid("canonical semantic workspace node is invalid"));
        }
        validate_digest(
            node["digest"]
                .as_str()
                .ok_or_else(|| invalid("canonical semantic workspace node digest is invalid"))?,
        )?;
        let node_value = node["value"]
            .as_object()
            .ok_or_else(|| invalid("canonical semantic workspace node value is invalid"))?;
        if node_value.len() != 2
            || !node_value.contains_key("payload")
            || node_value.get("schema").and_then(Value::as_str) != Some(schema)
        {
            return Err(invalid(
                "canonical semantic workspace node value is invalid",
            ));
        }
    }
    Ok(())
}

fn validate_digest(value: &str) -> Result<(), Vec<Diagnostic>> {
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(invalid(
            "canonical semantic workspace revision digest is invalid",
        ));
    }
    Ok(())
}

fn node_value(source: &str, digest: &str) -> Result<Value, Vec<Diagnostic>> {
    let value: Value = serde_json::from_str(source)
        .map_err(|_| invalid("canonical semantic workspace node is invalid"))?;
    Ok(json!({"digest": digest, "value": value}))
}

fn canonical_json(mut value: Value) -> Result<String, Vec<Diagnostic>> {
    value.sort_all_objects();
    let mut json = serde_json::to_string(&value)
        .map_err(|_| invalid("canonical semantic workspace revision could not be rendered"))?;
    json.push('\n');
    if json.len() > MAX_SEMANTIC_WORKSPACE_REVISION_BYTES {
        return Err(invalid(
            "canonical semantic workspace revision exceeds its byte limit",
        ));
    }
    Ok(json)
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

fn digest_sequence<'a>(domain: &[u8], values: impl IntoIterator<Item = &'a str>) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    for value in values {
        digest.update((value.len() as u64).to_le_bytes());
        digest.update(value.as_bytes());
    }
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(digest.finalize())
    )
}

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G222", message)]
}

fn stale(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G223", message)]
}
