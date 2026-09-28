//! Checked inert Step commitments and exact compiler field mapping.
//! These facts do not attest evaluator execution or retain runtime owners.
use super::reduce_wire::{recipe_digest, ReduceRecipeV8};
use super::SourceJournalError as Error;
use crate::resumable_effects::owned_frame::v2;
use serde_json::{json, Value};

pub(super) struct CheckedReduceStepV8 {
    step: Value,
    case: String,
    mapping: Value,
    target: Value,
}
impl CheckedReduceStepV8 {
    pub(super) fn step(&self) -> &Value {
        &self.step
    }
    pub(super) fn case(&self) -> &str {
        &self.case
    }
    pub(super) fn target(&self) -> &Value {
        &self.target
    }
    pub(super) fn ordinary_carrier_bytes(
        &self,
        plan: &v2::CheckedOwnedReduceV2,
    ) -> Result<Vec<u8>, Error> {
        v2::owned_reduce_target_bytes_v8(plan, &self.case, &self.target).map_err(|_| Error::Binding)
    }

    pub(super) fn transfer_digest(
        &self,
        scope: &Value,
        plan: &v2::CheckedOwnedReduceV2,
        turn: u32,
        attempt: u32,
        reserved: u32,
    ) -> Result<String, Error> {
        recipe_digest(
            ReduceRecipeV8::Transfer,
            &json!({"scope":scope,"binding":plan.binding(),
            "plan":plan.binding(),"turn":turn,"attempt":attempt,"reserved":reserved,
            "case":self.case,"mapping":self.mapping,"target":self.target}),
        )
    }
    pub(super) fn matches_target(&self, target: &Value) -> Result<(), Error> {
        if self.target == *target {
            Ok(())
        } else {
            Err(Error::Binding)
        }
    }
}

/// The parent supplies this exact proof from its authenticated Recorded join.
/// The accepted SDK mapper has already run inside that proof; never re-decode
/// result wire here or accept an independently supplied byte snapshot.
pub(super) fn checked_outcome(
    binding: &v2::CheckedOwnedAgentWaitBindingV8,
    settlement:&crate::agent_lifecycle::authorization::target_protocol::owned_wait_v8::settlement::CheckedOwnedEffectSettlementV8,
) -> Result<Value, Error> {
    let payload = settlement.accepted_payload().ok_or(Error::Binding)?;
    if payload.len() > 1024 {
        return Err(Error::Capacity);
    }
    let metadata = binding.lifecycle().owned_wait_outcome_v8();
    let fields = metadata
        .fields()
        .map(|(id, ty)| {
            let value = if id == metadata.bytes_field && *ty == crate::hir::ResolvedType::Bytes {
                json!({"kind":"bytes","hex":crate::live_invocation::identity::hex(payload)})
            } else if id == metadata.status_field && *ty == crate::hir::ResolvedType::I64 {
                json!({"tag":"i64","value":0})
            } else {
                return Err(Error::Binding);
            };
            Ok(json!({"identity":id.as_str(),"value":value}))
        })
        .collect::<Result<Vec<_>, Error>>()?;
    if fields.len() != 2 {
        return Err(Error::Binding);
    }
    // Structural data only: no interpreter Value, backing owner or admission.
    Ok(json!({"declaration":metadata.id.as_str(),"fields":fields}))
}

/// Inputs have already passed the parent's authenticated ordinary/owned inventory.
/// The supplied Step is compiler-shaped proof data, not a replayed owner/result.
pub(super) fn checked_step(
    plan: &v2::CheckedOwnedReduceV2,
    scope: &Value,
    turn: u32,
    attempt: u32,
    stage_reservation: u32,
    step: &Value,
    expected_digest: &str,
) -> Result<CheckedReduceStepV8, Error> {
    v2::validate_owned_reduce_step_v8(plan, step).map_err(|_| Error::Binding)?;
    let digest = recipe_digest(
        ReduceRecipeV8::Step,
        &json!({"scope":scope,
        "binding":plan.binding(),"plan":plan.binding(),"turn":turn,"attempt":attempt,
        "stage_reservation":stage_reservation,"step":step}),
    )?;
    if digest != expected_digest {
        return Err(Error::Binding);
    }
    let mapping = plan
        .mappings()
        .iter()
        .find(|m| step["case"] == m.case.as_str())
        .ok_or(Error::Binding)?;
    let supplied = step["fields"].as_array().ok_or(Error::Binding)?;
    let mapped = mapping
        .fields
        .iter()
        .map(|(source, destination)| {
            let field = supplied
                .iter()
                .find(|f| f["identity"] == source.as_str())
                .ok_or(Error::Binding)?;
            Ok(json!({"identity":destination.as_str(),"value":field["value"]}))
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let target = match mapping.role {
        "Continue" | "Suspend" | "Complete" => {
            let (kind, key) = match mapping.role {
                "Continue" => ("continue", "state"),
                "Suspend" => ("suspend", "state"),
                _ => ("complete", "report"),
            };
            let mut target = serde_json::Map::new();
            target.insert("kind".into(), kind.into());
            let declared = plan
                .helper()
                .program()
                .declarations
                .record_fields(&mapping.target)
                .ok_or(Error::Binding)?;
            let ordered = declared
                .iter()
                .map(|field| {
                    mapped
                        .iter()
                        .find(|value| value["identity"] == field.id.as_str())
                        .cloned()
                        .ok_or(Error::Binding)
                })
                .collect::<Result<Vec<_>, Error>>()?;
            target.insert(
                key.into(),
                json!({"declaration":mapping.target.as_str(),"fields":ordered}),
            );
            Value::Object(target)
        }
        "Fail" => {
            if mapped.len() != 1 {
                return Err(Error::Binding);
            }
            let value = &mapped[0]["value"];
            // Full Step validation already proved the exact declared i64 tag.
            let code = value["value"].as_i64().ok_or(Error::Binding)?;
            json!({"kind":"fail","code":code})
        }
        _ => return Err(Error::Binding),
    };
    v2::validate_owned_reduce_target_v8(plan, mapping.case.as_str(), &target)
        .map_err(|_| Error::Binding)?;
    Ok(CheckedReduceStepV8 {
        step: step.clone(),
        case: mapping.case.as_str().to_owned(),
        mapping: Value::Array(
            mapping
                .fields
                .iter()
                .map(|(a, b)| json!([a.as_str(), b.as_str()]))
                .collect(),
        ),
        target,
    })
}

#[cfg(test)]
#[path = "reduce_inventory/tests.rs"]
pub(super) mod tests;
