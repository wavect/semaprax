//! Frozen source-owned Ready commitment; data only, distinct from target grant.
use super::*;
use crate::hir::ResolvedType;
use crate::resumable_effects::owned_frame::v2::{self, CheckedOwnedAgentWaitBindingV8};
use crate::resumable_effects::source_checkpoint::SourceCheckpointScope;

pub(crate) fn owned_wait_ready_commitment_v8(
    binding: &CheckedOwnedAgentWaitBindingV8,
    scope: &SourceCheckpointScope,
    turn: u32,
    attempt: u32,
    state: &Value,
    decision: &Value,
    proposal_digest: &str,
    authorization_binding: &str,
) -> Result<(String, i64), SourceJournalError> {
    v2::validate_owned_wait_state_v8(binding, state).map_err(|_| SourceJournalError::Binding)?;
    v2::validate_owned_wait_decision_v8(binding, decision)
        .map_err(|_| SourceJournalError::Binding)?;
    let authorize = binding.authorize();
    if decision["case"] != authorize.granted().as_str() {
        return Err(SourceJournalError::Binding);
    }
    // The checked Granted proof has exactly one scalar i64 budget field.
    // Select its retained compiler identity before reading structural data.
    let declared = authorize
        .helper()
        .program()
        .declarations
        .case_fields(authorize.granted())
        .ok_or(SourceJournalError::Binding)?;
    let mut budgets = declared.iter().filter(|f| f.ty == ResolvedType::I64);
    let budget_field = budgets.next().ok_or(SourceJournalError::Binding)?;
    if budgets.next().is_some() {
        return Err(SourceJournalError::Binding);
    }
    let budget = decision["fields"]
        .as_array()
        .and_then(|fields| {
            fields
                .iter()
                .find(|field| field["identity"] == budget_field.id.as_str())
        })
        .and_then(|field| field["value"]["value"].as_i64())
        .ok_or(SourceJournalError::Binding)?;
    let scope = serde_json::json!({"program_root":scope.program_root(),"invocation":scope.invocation_id(),"policy_epoch":scope.policy_epoch()});
    let state_digest = crate::live_invocation::identity::digest(
        b"semaprax.source-owned-frame-args.v2\0",
        &wire::canonical(state),
    );
    let decision_digest = wire::recipe_digest(
        wire::RecipeV8::Decision,
        &serde_json::json!({"scope":scope,"turn":turn,"attempt":attempt,"authorize":authorize.function().id.as_str(),"decision":decision}),
    )?;
    let grant = wire::recipe_digest(
        wire::RecipeV8::Grant,
        &serde_json::json!({"scope":scope,"turn":turn,"attempt":attempt,"state_digest":state_digest,"proposal_digest":proposal_digest,"decision_digest":decision_digest,"authorization_binding":authorization_binding,"budget":budget}),
    )?;
    Ok((grant, budget))
}
