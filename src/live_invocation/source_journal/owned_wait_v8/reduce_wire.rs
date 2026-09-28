//! Closed §23 commitment recipes. Hashes are proof data, never owner or ACK authority.
use super::{wire, SourceJournalError};
use serde_json::Value;

#[derive(Clone, Copy)]
pub(super) enum ReduceRecipeV8 {
    Step,
    Basis,
    Transfer,
}
impl ReduceRecipeV8 {
    fn domain(self) -> &'static [u8] {
        match self {
            Self::Step => b"semaprax.source-agent-owned-reduce.step.v1\0",
            Self::Basis => b"semaprax.source-agent-owned-reduce.basis.v1\0",
            Self::Transfer => b"semaprax.source-agent-owned-reduce.transfer.v1\0",
        }
    }
    fn fields(self) -> &'static [&'static str] {
        match self {
            Self::Step => &[
                "scope",
                "binding",
                "plan",
                "turn",
                "attempt",
                "stage_reservation",
                "step",
            ],
            Self::Basis => &[
                "scope",
                "binding",
                "plan",
                "turn",
                "attempt",
                "stage_reservation",
                "basis",
            ],
            Self::Transfer => &[
                "scope", "binding", "plan", "turn", "attempt", "reserved", "case", "mapping",
                "target",
            ],
        }
    }
}

/// This checks only the closed recipe envelope. Compiler/source/causal validation
/// supplies the values separately; a matching digest cannot establish provenance.
pub(super) fn recipe_digest(
    recipe: ReduceRecipeV8,
    payload: &Value,
) -> Result<String, SourceJournalError> {
    let fields = payload.as_object().ok_or(SourceJournalError::Malformed)?;
    let keys = recipe.fields();
    if fields.len() != keys.len() || !keys.iter().all(|key| fields.contains_key(*key)) {
        return Err(SourceJournalError::Malformed);
    }
    let bytes = wire::canonical(payload);
    // Reuse the existing bounded duplicate-aware integer-only codec; no new parser.
    wire::parse(&bytes)?;
    Ok(crate::live_invocation::identity::digest(
        recipe.domain(),
        &bytes,
    ))
}

#[cfg(test)]
#[path = "reduce_wire/tests.rs"]
mod tests;
