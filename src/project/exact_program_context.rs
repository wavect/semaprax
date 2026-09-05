//! One exact, authority-free selection context over the enriched ProgramRoot family.
//!
//! This context retains already-admitted typed objects. It does not acquire a
//! Project snapshot, reread source or lock paths, execute targets, or publish
//! artifacts.

use std::sync::Arc;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;

use super::{
    ImageArtifactKind, InterfaceArtifactFacts, ProgramRoot, ProgramRootDependencyLockAssociation,
    ProgramRootV2, ProjectRevision, SemanticWorkspaceRevision,
};

pub const EXACT_PROGRAM_CONTEXT_SCHEMA: &str = "semaprax.exact-program-context.v1";
pub const MAX_EXACT_PROGRAM_CONTEXT_BYTES: usize = 64 * 1024;

const CONTEXT_DOMAIN: &[u8] = b"semaprax.exact-program-context.digest.v1\0";
const LOCK_BYTES_DOMAIN: &[u8] =
    b"semaprax.program-root.dependency-lock-association.lock-bytes.digest.v1\0";

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

/// An immutable exact-context generation. The default Project-derived v1 root
/// and the enriched semantic-workspace v1 root are intentionally distinct.
pub struct ExactProgramContext {
    revision: Arc<ProjectRevision>,
    semantic_workspace: SemanticWorkspaceRevision,
    base_project_root: ProgramRoot,
    semantic_workspace_root: ProgramRoot,
    interface_artifact_facts: InterfaceArtifactFacts,
    dependency_lock_association: ProgramRootDependencyLockAssociation,
    program_root_v2: ProgramRootV2,
    context_digest: String,
    json: String,
}

impl ExactProgramContext {
    /// Assemble a context and derive its ProgramRoot v2 internally.
    pub fn assemble(
        revision: Arc<ProjectRevision>,
        expected_project_revision: &str,
        semantic_workspace: SemanticWorkspaceRevision,
        expected_workspace_revision: &str,
        interface_artifact_facts: InterfaceArtifactFacts,
        dependency_lock_association: ProgramRootDependencyLockAssociation,
    ) -> Result<Self> {
        let base_project_root = revision.canonical_workspace_revision()?.program_root()?;
        let program_root_v2 = ProgramRootV2::derive(
            &semantic_workspace,
            &base_project_root,
            &interface_artifact_facts,
            &dependency_lock_association,
        )?;
        let expected_program_root_v2_digest = program_root_v2.program_root_v2_digest().to_owned();
        Self::derive(
            revision,
            expected_project_revision,
            semantic_workspace,
            expected_workspace_revision,
            interface_artifact_facts,
            dependency_lock_association,
            program_root_v2,
            &expected_program_root_v2_digest,
        )
    }

    /// Construct a context only after exact replay of every extension and the
    /// submitted ProgramRoot v2 against one retained Project revision.
    #[allow(clippy::too_many_arguments)]
    pub fn derive(
        revision: Arc<ProjectRevision>,
        expected_project_revision: &str,
        semantic_workspace: SemanticWorkspaceRevision,
        expected_workspace_revision: &str,
        interface_artifact_facts: InterfaceArtifactFacts,
        dependency_lock_association: ProgramRootDependencyLockAssociation,
        program_root_v2: ProgramRootV2,
        expected_program_root_v2_digest: &str,
    ) -> Result<Self> {
        validate_digest(expected_project_revision)?;
        validate_digest(expected_workspace_revision)?;
        validate_digest(expected_program_root_v2_digest)?;
        if revision.project_revision() != expected_project_revision {
            return Err(stale("exact context Project revision selector is stale"));
        }
        if semantic_workspace.workspace_revision() != expected_workspace_revision {
            return Err(stale(
                "exact context semantic workspace revision selector is stale",
            ));
        }

        validate_enriched_workspace(&revision, &semantic_workspace)?;
        let default_workspace = revision.canonical_workspace_revision()?;
        let base_project_root = default_workspace.program_root()?;
        let semantic_workspace_root = semantic_workspace.program_root()?;

        replay_interface_artifact_facts(&revision, &interface_artifact_facts)?;
        validate_lock_association(
            &revision,
            &default_workspace,
            &base_project_root,
            &dependency_lock_association,
        )?;

        if program_root_v2.semantic_workspace_revision() != semantic_workspace.workspace_revision()
            || program_root_v2.semantic_workspace_root_digest()
                != semantic_workspace_root.program_root_digest()
            || program_root_v2.base_project_root_digest() != base_project_root.program_root_digest()
            || program_root_v2.program_root_v2_digest() != expected_program_root_v2_digest
        {
            return Err(stale(
                "exact context ProgramRoot v1/v2 associations are stale",
            ));
        }
        let replayed_v2 = ProgramRootV2::replay(
            &semantic_workspace,
            &base_project_root,
            &interface_artifact_facts,
            &dependency_lock_association,
            expected_program_root_v2_digest,
            program_root_v2.to_json().as_bytes(),
        )?;
        if replayed_v2.program_root_v2_digest() != program_root_v2.program_root_v2_digest()
            || replayed_v2.to_json() != program_root_v2.to_json()
        {
            return Err(stale("exact context ProgramRoot v2 failed exact replay"));
        }

        let payload = json!({
            "base_project_root_digest": base_project_root.program_root_digest(),
            "dependency_lock_association_digest": dependency_lock_association.association_digest(),
            "interface_artifact_facts_digest": interface_artifact_facts.digest(),
            "legacy_workspace_revision": revision.workspace_revision(),
            "limits": {"max_context_bytes": MAX_EXACT_PROGRAM_CONTEXT_BYTES},
            "nonclaims": [
                "selection_context_not_source_hir_or_runtime_state",
                "default_and_enriched_program_root_v1_identities_remain_distinct",
                "no_lock_bytes_embedded_in_context_json",
                "no_filesystem_network_process_execution_deployment_commit_or_publication_authority",
            ],
            "program_root_v2_digest": program_root_v2.program_root_v2_digest(),
            "project_revision": revision.project_revision(),
            "schema": EXACT_PROGRAM_CONTEXT_SCHEMA,
            "semantic_workspace_revision": semantic_workspace.workspace_revision(),
            "semantic_workspace_root_digest": semantic_workspace_root.program_root_digest(),
        });
        let identity_bytes = canonical_json(payload.clone())?;
        let context_digest = framed_digest(CONTEXT_DOMAIN, identity_bytes.as_bytes());
        let mut final_value = payload;
        final_value
            .as_object_mut()
            .expect("context payload is an object")
            .insert("context_digest".to_owned(), json!(context_digest));
        let json = canonical_json(final_value)?;
        if json.len() > MAX_EXACT_PROGRAM_CONTEXT_BYTES {
            return Err(invalid("exact program context exceeds its byte limit"));
        }
        if json.contains(dependency_lock_association.project_lock_bytes()) {
            return Err(invalid(
                "exact program context must not embed private Project Lock bytes",
            ));
        }

        Ok(Self {
            revision,
            semantic_workspace,
            base_project_root,
            semantic_workspace_root,
            interface_artifact_facts,
            dependency_lock_association,
            program_root_v2,
            context_digest,
            json,
        })
    }

    /// Select this generation only when both outer semantic identities match.
    pub fn select(
        &self,
        expected_workspace_revision: &str,
        expected_program_root_v2_digest: &str,
    ) -> Result<&ProgramRootV2> {
        validate_digest(expected_workspace_revision)?;
        validate_digest(expected_program_root_v2_digest)?;
        if expected_workspace_revision != self.semantic_workspace.workspace_revision()
            || expected_program_root_v2_digest != self.program_root_v2.program_root_v2_digest()
        {
            return Err(stale(
                "exact context requires matching workspace and ProgramRoot v2 selectors",
            ));
        }
        Ok(&self.program_root_v2)
    }

    pub fn revision(&self) -> &Arc<ProjectRevision> {
        &self.revision
    }
    pub fn project_revision(&self) -> &str {
        self.revision.project_revision()
    }
    pub fn semantic_workspace(&self) -> &SemanticWorkspaceRevision {
        &self.semantic_workspace
    }
    pub fn base_project_root(&self) -> &ProgramRoot {
        &self.base_project_root
    }
    pub fn semantic_workspace_root(&self) -> &ProgramRoot {
        &self.semantic_workspace_root
    }
    pub fn interface_artifact_facts(&self) -> &InterfaceArtifactFacts {
        &self.interface_artifact_facts
    }
    pub fn dependency_lock_association(&self) -> &ProgramRootDependencyLockAssociation {
        &self.dependency_lock_association
    }
    pub fn program_root_v2(&self) -> &ProgramRootV2 {
        &self.program_root_v2
    }
    pub fn context_digest(&self) -> &str {
        &self.context_digest
    }
    pub fn to_json(&self) -> &str {
        &self.json
    }
}

fn validate_enriched_workspace(
    revision: &ProjectRevision,
    workspace: &SemanticWorkspaceRevision,
) -> Result<()> {
    let metadata = parse_json(
        workspace.projection_metadata().to_json(),
        "exact context projection metadata",
    )?;
    if metadata["payload"]["legacy_project_revision"] != revision.project_revision()
        || metadata["payload"]["legacy_workspace_revision"] != revision.workspace_revision()
        || metadata["payload"]["project_graph_digest"] != revision.semantic_graph_digest()
    {
        return Err(stale(
            "exact context semantic workspace does not bind the retained Project",
        ));
    }
    let agents = parse_json(
        workspace.agent_definitions().to_json(),
        "exact context AgentDefinitions",
    )?;
    let definitions = agents["payload"]["definitions"]
        .as_array()
        .ok_or_else(|| invalid("exact context AgentDefinitions inventory is malformed"))?;
    if definitions.is_empty()
        || agents["payload"]["integration"] != "explicit_compiler_admitted_association_input"
        || agents["payload"]["expected_project_revision"] != revision.project_revision()
    {
        return Err(invalid(
            "exact context requires a non-empty explicit AgentDefinitions association",
        ));
    }
    Ok(())
}

fn replay_interface_artifact_facts(
    revision: &Arc<ProjectRevision>,
    facts: &InterfaceArtifactFacts,
) -> Result<()> {
    if facts.project_revision() != revision.project_revision() {
        return Err(stale(
            "exact context interface/artifact facts select another Project",
        ));
    }
    let kinds = facts
        .artifact_projections()
        .iter()
        .map(|fact| fact.kind())
        .collect::<Vec<ImageArtifactKind>>();
    let max_build_bytes = facts
        .artifact_projections()
        .first()
        .ok_or_else(|| invalid("exact context interface/artifact inventory is empty"))?
        .max_build_bytes();
    if facts
        .artifact_projections()
        .iter()
        .any(|fact| fact.max_build_bytes() != max_build_bytes)
    {
        return Err(invalid(
            "exact context artifact facts have inconsistent build bounds",
        ));
    }
    let fact_value = parse_json(facts.to_json(), "exact context interface/artifact facts")?;
    if fact_value["workspace_revision"] != revision.workspace_revision()
        || fact_value["project_graph_digest"] != revision.semantic_graph_digest()
    {
        return Err(stale(
            "exact context interface/artifact facts do not bind the retained Project",
        ));
    }
    let replayed = InterfaceArtifactFacts::replay(
        revision.clone(),
        revision.project_revision(),
        &kinds,
        max_build_bytes,
        facts.digest(),
        facts.to_json().as_bytes(),
    )?;
    if &replayed != facts {
        return Err(stale(
            "exact context interface/artifact facts failed exact replay",
        ));
    }
    Ok(())
}

fn validate_lock_association(
    revision: &ProjectRevision,
    default_workspace: &SemanticWorkspaceRevision,
    base_project_root: &ProgramRoot,
    association: &ProgramRootDependencyLockAssociation,
) -> Result<()> {
    if association.program_root_digest() != base_project_root.program_root_digest() {
        return Err(stale(
            "exact context dependency lock does not select the base Project root",
        ));
    }
    let value = parse_json(
        association.to_json(),
        "exact context dependency lock association",
    )?;
    if value["association_digest"] != association.association_digest()
        || value["canonical_workspace_revision"] != default_workspace.workspace_revision()
        || value["canonical_workspace_dependency_lock_digest"]
            != default_workspace.dependency_lock_digest()
        || value["program_root_digest"] != base_project_root.program_root_digest()
        || value["project_revision"] != revision.project_revision()
        || value["project_lock"]["program_root"] != revision.project_revision()
        || value["project_lock"]["digest"] != association.project_lock_digest()
        || value["project_lock"]["bytes"] != association.project_lock_bytes().len()
        || value["project_lock"]["bytes_digest"] != association.project_lock_bytes_digest()
        || framed_digest(
            LOCK_BYTES_DOMAIN,
            association.project_lock_bytes().as_bytes(),
        ) != association.project_lock_bytes_digest()
    {
        return Err(stale(
            "exact context dependency lock association is internally stale",
        ));
    }
    Ok(())
}

fn parse_json(source: &str, subject: &str) -> Result<Value> {
    serde_json::from_str(source).map_err(|_| invalid(format!("{subject} is not valid JSON")))
}

fn canonical_json(mut value: Value) -> Result<String> {
    value.sort_all_objects();
    let mut output = serde_json::to_string(&value)
        .map_err(|_| invalid("exact program context could not be serialized"))?;
    output.push('\n');
    Ok(output)
}

fn validate_digest(value: &str) -> Result<()> {
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(invalid(
            "exact program context requires a lowercase sha256 digest",
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
    vec![Diagnostic::io("SPX-G554", message)]
}

fn stale(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G555", message)]
}
