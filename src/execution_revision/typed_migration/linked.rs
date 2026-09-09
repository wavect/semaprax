//! Migration program selection from sealed runtime bindings, never submitted HIR.
use super::*;
use serde_json::Value;

pub(super) struct Programs {
    pub(super) previous: hir::ResolvedProgram,
    pub(super) destination: hir::ResolvedProgram,
    pub(super) linked_sources: Option<Value>,
}

fn is_linked(runtime: &AgentRuntimeV2) -> bool {
    runtime
        .lifecycle
        .canonical_json()
        .starts_with("{\"schema\":\"semaprax.agent-typed-effects.v4\"")
}

pub(super) fn programs(
    previous: &AgentRuntimeV2,
    destination: &AgentRuntimeV2,
    migration: &str,
) -> Result<Programs> {
    let (old, old_association) = selected(previous, None)?;
    let (new, new_association) = selected(destination, Some(migration))?;
    let linked_sources = (is_linked(previous) || is_linked(destination)).then(|| {
        json!({
            "previous": old_association,
            "destination": new_association,
        })
    });
    Ok(Programs {
        previous: old,
        destination: new,
        linked_sources,
    })
}

fn selected(
    runtime: &AgentRuntimeV2,
    migration: Option<&str>,
) -> Result<(hir::ResolvedProgram, Option<Value>)> {
    if !is_linked(runtime) {
        return Ok((selected_program(runtime)?, None));
    }
    let deployment: Value = serde_json::from_str(runtime.deployment.canonical_json())
        .map_err(|_| refused("migration.deployment"))?;
    let path = deployment["facts"]["source_path"]
        .as_str()
        .ok_or_else(|| refused("migration.source_path"))?;
    let agent_id = deployment["facts"]["agent_id"]
        .as_str()
        .ok_or_else(|| refused("migration.agent_id"))?;
    let source = runtime
        .project
        .sources()
        .iter()
        .find(|source| source.path() == path)
        .ok_or_else(|| refused("migration.retained_source"))?;
    let parsed =
        crate::parse(source.source(), std::path::Path::new(path)).map_err(|error| vec![error])?;
    let agent = parsed
        .agents
        .iter()
        .find(|agent| agent.stable_id == agent_id)
        .ok_or_else(|| refused("migration.retained_agent"))?;
    let definition = crate::project::compile_source_agent_declaration(agent)?;
    // Reconstruct the original checked role association before extending it.
    // Complete equality includes imported bodies through the exact Project root.
    let roles = runtime.project.linked_agent_program(
        path,
        agent_id,
        definition.definition().canonical_source(),
    )?;
    let association: Value =
        serde_json::from_str(&roles.association).map_err(|_| refused("migration.linked_source"))?;
    let lifecycle: Value = serde_json::from_str(runtime.lifecycle.canonical_json())
        .map_err(|_| refused("migration.lifecycle"))?;
    if lifecycle["lifecycle"]["linked_source"] != association {
        return Err(refused("migration.linked_source_drift"));
    }
    if let Some(migration) = migration {
        let selected = runtime.project.linked_agent_migration_program(
            path,
            agent_id,
            definition.definition().canonical_source(),
            migration,
        )?;
        let association = serde_json::from_str(&selected.association)
            .map_err(|_| refused("migration.linked_migration_source"))?;
        Ok((selected.program, Some(association)))
    } else {
        Ok((roles.program, Some(association)))
    }
}
