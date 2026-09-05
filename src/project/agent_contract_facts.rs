//! Content-addressed Proposal and Observation contracts for source-owned Agents.

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::agent_definition::CompiledAgentDefinition;
use crate::agent_observation::{
    compile_source_agent_observation_schema, verify_source_agent_observation_schema_bundle,
};
use crate::agent_proposal::{compile_agent_proposal_schema, verify_agent_proposal_schema_bundle};
use crate::ast::Program;
use crate::diagnostic::Diagnostic;
use crate::semantic_workspace::SemanticWorkspaceFileFact;

pub const AGENT_INTERACTION_CONTRACT_FACTS_SCHEMA: &str =
    "semaprax.agent-interaction-contract-facts.v1";
pub const MAX_AGENT_INTERACTION_CONTRACT_FACTS_BYTES: usize = 16 * 1024 * 1024;

const DIGEST_DOMAIN: &[u8] = b"semaprax.agent-interaction-contract-facts.digest.v1\0";

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentInteractionContractFact {
    agent_id: String,
    proposal_schema: String,
    proposal_schema_digest: String,
    observation_schema: String,
    observation_schema_digest: String,
    proposal_type_id: String,
    proposal_type_revision: String,
    observation_type_id: String,
    observation_type_revision: String,
    value: Value,
}

impl AgentInteractionContractFact {
    pub fn agent_id(&self) -> &str {
        &self.agent_id
    }
    pub fn proposal_schema(&self) -> &str {
        &self.proposal_schema
    }
    pub fn proposal_schema_digest(&self) -> &str {
        &self.proposal_schema_digest
    }
    pub fn observation_schema(&self) -> &str {
        &self.observation_schema
    }
    pub fn observation_schema_digest(&self) -> &str {
        &self.observation_schema_digest
    }
    pub fn proposal_type_id(&self) -> &str {
        &self.proposal_type_id
    }
    pub fn proposal_type_revision(&self) -> &str {
        &self.proposal_type_revision
    }
    pub fn observation_type_id(&self) -> &str {
        &self.observation_type_id
    }
    pub fn observation_type_revision(&self) -> &str {
        &self.observation_type_revision
    }
    pub(crate) fn value(&self) -> &Value {
        &self.value
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentInteractionContractFacts {
    project_revision: String,
    source_workspace_revision: String,
    project_graph_digest: String,
    facts: Vec<AgentInteractionContractFact>,
    digest: String,
    json: String,
}

impl AgentInteractionContractFacts {
    pub(crate) fn derive(
        project_revision: &str,
        source_workspace_revision: &str,
        project_graph_digest: &str,
        files: &[SemanticWorkspaceFileFact],
        programs: &[Program],
        definitions: &[CompiledAgentDefinition],
    ) -> Result<Self> {
        if definitions.is_empty()
            || definitions.len()
                != programs
                    .iter()
                    .map(|program| program.agents.len())
                    .sum::<usize>()
        {
            return Err(invalid(
                "Agent interaction contracts require every source Agent",
            ));
        }
        let mut facts = Vec::with_capacity(definitions.len());
        for definition in definitions {
            let agent_id = definition.definition().agent_id();
            let (program, file) = programs
                .iter()
                .zip(files)
                .find(|(program, _)| {
                    program
                        .agents
                        .iter()
                        .any(|agent| agent.stable_id == agent_id)
                })
                .ok_or_else(|| invalid("Agent interaction contract source module is missing"))?;
            let proposal = compile_agent_proposal_schema(
                file.source(),
                file.path(),
                definition.definition().canonical_source(),
            )?;
            let observation =
                compile_source_agent_observation_schema(file.source(), file.path(), agent_id)?;
            if proposal.definition_digest() != definition.definition().digest()
                || observation.definition_digest() != definition.definition().digest()
                || proposal.schema().agent_id() != agent_id
                || observation.schema().agent_id() != agent_id
                || program
                    .agents
                    .iter()
                    .filter(|agent| agent.stable_id == agent_id)
                    .count()
                    != 1
            {
                return Err(stale("Agent interaction contract replay is stale"));
            }
            let proposal_schema = proposal.schema().canonical_json().to_owned();
            let observation_schema = observation.schema().canonical_json().to_owned();
            verify_agent_proposal_schema_bundle(
                file.source(),
                file.path(),
                definition.definition().canonical_source(),
                &proposal_schema,
            )?;
            verify_source_agent_observation_schema_bundle(
                file.source(),
                file.path(),
                agent_id,
                &observation_schema,
            )?;
            let proposal_type_id = proposal.schema().proposal_type_id().to_owned();
            let proposal_type_revision = proposal.schema().proposal_type_revision().to_owned();
            let observation_type_id = observation.schema().observation_type_id().to_owned();
            let observation_type_revision =
                observation.schema().observation_type_revision().to_owned();
            let value = json!({
                "agent_id": agent_id,
                "definition_digest": definition.definition().digest(),
                "observation": {
                    "schema": observation_schema,
                    "schema_digest": observation.schema().digest(),
                    "source_revision": observation.source_revision(),
                    "type_id": observation_type_id,
                    "type_revision": observation_type_revision,
                },
                "proposal": {
                    "schema": proposal_schema,
                    "schema_digest": proposal.schema().digest(),
                    "source_revision": proposal.source_revision(),
                    "type_id": proposal_type_id,
                    "type_revision": proposal_type_revision,
                },
            });
            facts.push(AgentInteractionContractFact {
                agent_id: agent_id.to_owned(),
                proposal_schema,
                proposal_schema_digest: proposal.schema().digest().to_owned(),
                observation_schema,
                observation_schema_digest: observation.schema().digest().to_owned(),
                proposal_type_id,
                proposal_type_revision,
                observation_type_id,
                observation_type_revision,
                value,
            });
        }
        if facts
            .windows(2)
            .any(|pair| pair[0].agent_id >= pair[1].agent_id)
        {
            return Err(invalid(
                "Agent interaction contracts are not in stable-ID order",
            ));
        }
        let value = json!({
            "authority": false,
            "facts": facts.iter().map(|fact| fact.value()).collect::<Vec<_>>(),
            "limits": {"max_bundle_bytes": MAX_AGENT_INTERACTION_CONTRACT_FACTS_BYTES},
            "nonclaims": [
                "schemas_are_data_not_authorization_or_capabilities",
                "no_provider_tool_filesystem_network_execution_or_publication_authority",
                "same_module_closed_record_or_variant_roles_only",
            ],
            "project_graph_digest": project_graph_digest,
            "project_revision": project_revision,
            "schema": AGENT_INTERACTION_CONTRACT_FACTS_SCHEMA,
            "source_workspace_revision": source_workspace_revision,
        });
        let json = canonical_json(value)?;
        if json.len() > MAX_AGENT_INTERACTION_CONTRACT_FACTS_BYTES {
            return Err(invalid(
                "Agent interaction contract facts exceed their byte limit",
            ));
        }
        let digest = framed_digest(DIGEST_DOMAIN, json.as_bytes());
        Ok(Self {
            project_revision: project_revision.to_owned(),
            source_workspace_revision: source_workspace_revision.to_owned(),
            project_graph_digest: project_graph_digest.to_owned(),
            facts,
            digest,
            json,
        })
    }

    pub fn project_revision(&self) -> &str {
        &self.project_revision
    }
    pub fn source_workspace_revision(&self) -> &str {
        &self.source_workspace_revision
    }
    pub fn project_graph_digest(&self) -> &str {
        &self.project_graph_digest
    }
    pub fn facts(&self) -> &[AgentInteractionContractFact] {
        &self.facts
    }
    pub fn fact(&self, agent_id: &str) -> Option<&AgentInteractionContractFact> {
        self.facts.iter().find(|fact| fact.agent_id == agent_id)
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub fn to_json(&self) -> &str {
        &self.json
    }
}

fn canonical_json(mut value: Value) -> Result<String> {
    value.sort_all_objects();
    let mut output = serde_json::to_string(&value)
        .map_err(|_| invalid("Agent interaction contracts cannot be rendered"))?;
    output.push('\n');
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
    vec![Diagnostic::io("SPX-G564", message)]
}

fn stale(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G565", message)]
}
