//! Provider schema projection from the actual authenticated Project graph.
//! The entry closure is unchanged; these explicit type/role roots admit data.
use super::*;
use crate::agent_observation::CompiledAgentObservationSchema;
use crate::agent_proposal::CompiledAgentProposalSchema;
use crate::workspace_graph::WorkspaceGraphBuild;

pub(in crate::project) struct PreparedProviderSchemas {
    agent: String,
    path: String,
    proposal: CompiledAgentProposalSchema,
    observation: CompiledAgentObservationSchema,
}
impl PreparedProviderSchemas {
    pub(super) fn into_parts(
        self,
    ) -> (CompiledAgentProposalSchema, CompiledAgentObservationSchema) {
        (self.proposal, self.observation)
    }
}
pub(super) fn take(
    prepared: &mut Vec<PreparedProviderSchemas>,
    agent: &str,
    path: &str,
) -> Option<PreparedProviderSchemas> {
    let index = prepared
        .iter()
        .position(|row| row.agent == agent && row.path == path)?;
    Some(prepared.remove(index))
}
/// Called while the same preflight graph still owns all original-source HIR,
/// before it is consumed into the independently selected executable closures.
/// No parser, standalone entry check, synthetic main, import or file I/O here.
pub(in crate::project) fn prepare(
    graph: &WorkspaceGraphBuild,
    entry: &str,
    files: &[SemanticWorkspaceFileFact],
    programs: &[&Program],
    definitions: &[CompiledAgentDefinition],
) -> Result<Vec<PreparedProviderSchemas>> {
    let count = programs
        .iter()
        .filter(|program| !program.functions.iter().any(|f| f.name == "main"))
        .try_fold(0usize, |count, program| {
            count.checked_add(program.agents.len())
        })
        .ok_or_else(|| invalid("provider Agent inventory exceeds its bound"))?;
    let mut out = Vec::with_capacity(count);
    for definition in definitions {
        let agent_id = definition.definition().agent_id();
        let (program, file) = programs
            .iter()
            .zip(files)
            .find(|(p, _)| p.agents.iter().any(|a| a.stable_id == agent_id))
            .ok_or_else(|| invalid("provider Agent original source is missing"))?;
        if program.functions.iter().any(|f| f.name == "main") {
            continue;
        }
        let agent = program
            .agents
            .iter()
            .find(|a| a.stable_id == agent_id)
            .ok_or_else(|| invalid("provider Agent is missing"))?;
        let mut roots = ["initialize", "observe", "authorize", "reduce"]
            .iter()
            .map(|role| {
                definition
                    .definition()
                    .operation(role)
                    .map(|(id, _)| id.to_owned())
                    .ok_or_else(|| invalid("provider Agent role missing"))
            })
            .collect::<Result<Vec<_>>>()?;
        if let Some(wait) = &agent.model_wait {
            roots.push(wait.helper_id.clone());
        }
        let types = agent
            .types
            .iter()
            .map(|role| crate::hir::DeclarationId::new(&role.stable_id))
            .collect::<Vec<_>>();
        // The graph was checked against every original module before linkage.
        // This API retains its sealed module association and verifies linked HIR.
        let linked = graph.linked_agent_role_program(entry, &roots, &types)?;
        let selected = linked
            .agents
            .iter()
            .find(|a| a.stable_id.as_str() == agent_id)
            .ok_or_else(|| invalid("provider Agent checked association missing"))?;
        if selected.name != agent.name
            || selected.runtime_v1_json != agent.runtime_v1_json
            || selected.types.len() != agent.types.len()
            || selected
                .types
                .iter()
                .zip(&agent.types)
                .any(|(actual, expected)| {
                    actual.role as u8 != expected.role as u8
                        || actual.stable_id.as_str() != expected.stable_id
                })
            || selected.model_wait.as_ref().map(|w| w.helper_id.as_str())
                != agent.model_wait.as_ref().map(|w| w.helper_id.as_str())
            || selected
                .operations
                .iter()
                .zip(&agent.operations)
                .any(|(actual, expected)| {
                    actual.role as u8 != expected.role as u8
                        || actual.kind as u8 != expected.kind as u8
                        || actual.stable_id.as_str() != expected.stable_id
                        || actual.embedded != expected.embedded_function_index.is_some()
                })
            || selected.operations.len() != agent.operations.len()
        {
            return Err(stale("provider Agent source association differs"));
        }
        let proposal = crate::agent_proposal::compile_resolved_agent_proposal_schema(
            &linked,
            file.source_revision().to_owned(),
            definition,
        )?;
        let observation = crate::agent_observation::compile_resolved_agent_observation_schema(
            &linked,
            file.source_revision().to_owned(),
            definition,
        )?;
        let replay_proposal = crate::agent_proposal::compile_resolved_agent_proposal_schema(
            &linked,
            file.source_revision().to_owned(),
            definition,
        )?;
        let replay_observation =
            crate::agent_observation::compile_resolved_agent_observation_schema(
                &linked,
                file.source_revision().to_owned(),
                definition,
            )?;
        if proposal.schema().canonical_json() != replay_proposal.schema().canonical_json()
            || observation.schema().canonical_json() != replay_observation.schema().canonical_json()
        {
            return Err(stale("provider Agent schema replay differs"));
        }
        out.push(PreparedProviderSchemas {
            agent: agent_id.to_owned(),
            path: file.path().to_owned(),
            proposal,
            observation,
        });
    }
    Ok(out)
}
