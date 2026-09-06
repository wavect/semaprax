//! Additive generic instance ownership for bounded Agent Context v2 facts.
use super::*;

impl AgentContextV2Index<'_> {
    pub(super) fn build_fact(
        &self,
        id: &DeclarationId,
        depth: usize,
        reached_by: BTreeSet<AgentContextDirection>,
        filters: &BTreeSet<AgentContextFilter>,
    ) -> Result<AgentFunctionFactV2, Diagnostic> {
        let calls = self
            .calls_by_id
            .get(id)
            .ok_or_else(|| graph_reference_error("function", id))?
            .clone();
        let called_by = self
            .callers_by_id
            .get(id)
            .ok_or_else(|| graph_reference_error("function", id))?
            .clone();
        let base_json = if let Some(function) = self.functions.get(id) {
            agent_function_json_for_schema(
                self.program,
                function,
                filters,
                nested_owned::generic_payload_schema(self.program)?,
            )?
        } else if let Some(template) = self.templates.get(id) {
            agent_template_json(self.program, template, filters)?
        } else {
            return Err(graph_reference_error("function", id));
        };
        let base_json =
            append_instances(self.program, self.source_revision, id, filters, base_json)?;
        Ok(AgentFunctionFactV2 {
            id: id.clone(),
            depth,
            reached_by,
            calls,
            called_by: called_by.clone(),
            json: agent_v2_fact_json(base_json, &called_by),
        })
    }
}

fn append_instances(
    program: &ResolvedProgram,
    source_revision: &str,
    template: &DeclarationId,
    filters: &BTreeSet<AgentContextFilter>,
    mut base: String,
) -> Result<String, Diagnostic> {
    if !filters.contains(&AgentContextFilter::Types)
        && !filters.contains(&AgentContextFilter::Ownership)
    {
        return Ok(base);
    }
    let mut instances = program
        .function_instances
        .iter()
        .filter(|instance| &instance.template == template)
        .collect::<Vec<_>>();
    if instances.is_empty() {
        return Ok(base);
    }
    instances.sort_by_key(|instance| (&instance.template, &instance.type_arguments));
    let facts = instances
        .into_iter()
        .map(|instance| generic_instances::instance_json(program, source_revision, instance))
        .collect::<Result<Vec<_>, _>>()?
        .budgeted_join(",");
    assert_eq!(base.pop(), Some('}'));
    write!(base, ",\"generic_instance_ownership\":[{facts}]}}")
        .expect("writing to a string cannot fail");
    Ok(base)
}
