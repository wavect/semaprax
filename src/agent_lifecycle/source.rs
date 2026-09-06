//! Exact bridge from one checked source Agent declaration to Agent Lifecycle v1.
//!
//! This module adds no second definition format. It selects one source Agent by
//! persistent identity, lowers that exact declaration through the existing
//! AgentDefinition-v1 compiler, and gives the resulting canonical bytes to the
//! existing lifecycle compiler.

use std::path::Path;

use crate::diagnostic::Diagnostic;

use super::{
    bundle_mismatch, compile_agent_lifecycle, invariant, CompiledAgentLifecycle,
    MAX_LIFECYCLE_BYTES,
};

const MAX_SOURCE_AGENT_ID_BYTES: usize = 240;

fn valid_agent_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_SOURCE_AGENT_ID_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

fn valid_revision(value: &str) -> bool {
    value.len() == "sha256:".len() + 64
        && value.starts_with("sha256:")
        && value["sha256:".len()..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Compiles one source-declared Agent into the existing bounded lifecycle.
///
/// Selection happens only after the complete module passes the ordinary
/// checked pipeline. The existing lifecycle and Proposal compilers then
/// independently recheck those same immutable source bytes at their ordinary
/// trust boundaries. The selected declaration remains the sole source of the
/// AgentDefinition bytes used for lifecycle binding; callers cannot pair it
/// with a separately mutable definition document.
pub fn compile_source_agent_lifecycle(
    module_source: &str,
    module_path: impl AsRef<Path>,
    agent_id: &str,
) -> Result<CompiledAgentLifecycle, Vec<Diagnostic>> {
    let module_path = module_path.as_ref();
    if !valid_agent_id(agent_id) {
        return Err(vec![invariant("source_agent.selection")]);
    }
    let program = crate::check(module_source, module_path)?;
    let mut selected = program
        .agents
        .iter()
        .filter(|declaration| declaration.stable_id == agent_id);
    let Some(declaration) = selected.next() else {
        return Err(vec![invariant("source_agent.selection")]);
    };
    if selected.next().is_some() {
        return Err(vec![invariant("source_agent.selection.duplicate")]);
    }

    let definition = crate::project::compile_source_agent_declaration(declaration)?;
    let lifecycle = compile_agent_lifecycle(
        module_source,
        module_path,
        definition.definition().canonical_source(),
    )?;
    if lifecycle.agent_id() != declaration.stable_id
        || lifecycle.definition_digest() != definition.definition().digest()
    {
        return Err(vec![invariant("source_agent.definition")]);
    }
    Ok(lifecycle)
}

/// Recompiles one source Agent lifecycle and requires exact lifecycle bytes.
pub fn verify_source_agent_lifecycle_bundle(
    module_source: &str,
    module_path: impl AsRef<Path>,
    agent_id: &str,
    expected_source_revision: &str,
    lifecycle_source: &str,
) -> Result<(), Vec<Diagnostic>> {
    if lifecycle_source.len() > MAX_LIFECYCLE_BYTES
        || !valid_agent_id(agent_id)
        || !valid_revision(expected_source_revision)
    {
        return Err(vec![bundle_mismatch()]);
    }
    let lifecycle = compile_source_agent_lifecycle(module_source, module_path, agent_id)?;
    if lifecycle.source_revision() != expected_source_revision
        || lifecycle.canonical_json().as_bytes() != lifecycle_source.as_bytes()
    {
        return Err(vec![bundle_mismatch()]);
    }
    Ok(())
}
