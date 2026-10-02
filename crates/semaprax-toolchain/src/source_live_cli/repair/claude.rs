//! Additive V3 physical transport; the checked lifecycle and journal are shared.
use super::*;
use crate::claude_host::{ClaudeAdapter, Config};

pub(super) fn identity(
    operands: &OpenCodeOperands,
) -> Result<SourceModelAdapterIdentity, CliError> {
    let executable = operands
        .executable
        .canonicalize()
        .map_err(|_| CliError::refused("repair Claude executable is unavailable"))?;
    let scratch = operands
        .scratch
        .canonicalize()
        .map_err(|_| CliError::refused("repair Claude scratch is unavailable"))?;
    crate::claude_host::identity(&executable, &scratch)
        .map_err(|_| CliError::refused("repair Claude identity admission refused"))
}

pub(super) fn factory(
    operands: OpenCodeOperands,
    remaining: i64,
    expected: &str,
    bound: String,
    proposal_schema: &semaprax::agent_proposal::CompiledAgentProposalSchema,
    retained_marker: Option<&[u8]>,
    pause_host: &mut Option<OpenCodeHostConfig>,
) -> Result<Box<dyn FnMut() -> Box<dyn ProviderAdapter>>, CliError> {
    let pause = operands.pause_after_settled;
    let config = Config::new(
        operands.executable,
        operands.scratch,
        Duration::from_millis(remaining.clamp(1, MAX_ONE_PROVIDER_CALL_MS) as u64),
    )
    .and_then(|config| config.with_proposal_schema(proposal_schema))
    .map_err(|_| CliError::refused("repair Claude host configuration refused"))?;
    if config.identity().adapter_identity != expected {
        return Err(CliError::refused(
            "repair Claude executable changed while binding the host",
        ));
    }
    config
        .marker_host()
        .clear_repair_post_settled_marker(retained_marker)
        .map_err(|_| {
            CliError::refused(
                "repair Claude post-settlement marker does not match authenticated checkpoint",
            )
        })?;
    if pause {
        *pause_host = Some(config.marker_host().clone());
    }
    Ok(Box::new(move || {
        Box::new(ClaudeAdapter::new(config.clone(), bound.clone()))
    }))
}
