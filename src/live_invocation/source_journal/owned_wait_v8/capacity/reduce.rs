//! Maximum serialized legal compiler-shaped Reduce closures; proof data only.
use super::*;
use crate::hir::ResolvedType;
use crate::interpreter::{resumable::checkpoint, ArgumentValue};
use crate::resumable_effects::owned_frame::v2;

#[derive(Debug, Eq, PartialEq)]
pub(super) struct ReduceRoomsV8 {
    pub before_stage: RoomV8,
    pub charged: RoomV8,
    pub cases: Vec<ReduceCaseRoomsV8>,
    pub mapped: RoomV8,
    pub failed_state: RoomV8,
}
#[derive(Debug, Eq, PartialEq)]
pub(super) struct ReduceCaseRoomsV8 {
    pub case: String,
    pub staged: RoomV8,
    pub transfer: RoomV8,
    pub completed: RoomV8,
}
impl ReduceRoomsV8 {
    /// Carry this exclusive-outcome maximum backward through Effect Intent,
    /// authorization and every permitted malformed-Proposal retry branch.
    /// It reserves bytes/rows only, and does not charge the Reduce allowance.
    pub(super) fn after_effect(&self) -> RoomV8 {
        self.before_stage.either(self.failed_state)
    }

    pub(super) fn outstanding(
        &self,
        fold: &super::super::reduce_fold::ReduceFoldV8,
    ) -> Result<RoomV8, SourceJournalError> {
        use super::super::reduce_fold::ReduceTailV8 as Tail;
        let f = fold.closure_facts();
        let case = || {
            self.cases
                .iter()
                .find(|c| Some(c.case.as_str()) == f.case)
                .ok_or(SourceJournalError::Order)
        };
        match f.tail {
            Tail::Charged => Ok(self.charged),
            Tail::Staged => Ok(case()?.staged),
            Tail::CleanupInDoubt => receipt(f.active_operations.ok_or(SourceJournalError::Order)?)?
                .add(if f.failure {
                    terminal()
                } else {
                    case()?.transfer
                }),
            Tail::CleanupSucceeded => Ok(case()?.transfer),
            Tail::FailureCleaned => Ok(terminal()),
            Tail::TransferInDoubt => Ok(case()?.completed),
            Tail::Mapped => Ok(self.mapped),
            Tail::TerminalPending => Ok(RoomV8 {
                rows: 1,
                ..self.mapped
            }),
            Tail::Quarantined | Tail::Continued => Ok(RoomV8::default()),
        }
    }
}
fn terminal() -> RoomV8 {
    // Existing complete source-terminal room includes all allowed bounded carrier
    // and evidence bytes. It does NOT assert exact consumed accounting or Claim.
    RoomV8 {
        bytes: super::super::super::execution::TERMINAL_ROOM_BYTES,
        rows: 2,
    }
}
fn maximum_fields<'a>(
    fields: impl Iterator<Item = (&'a crate::hir::DeclarationId, &'a ResolvedType)>,
) -> Result<Value, SourceJournalError> {
    let fields = fields
        .map(|(id, ty)| {
            let value = if *ty == ResolvedType::Bytes {
                json!({"kind":"bytes","hex":"ff".repeat(1024)})
            } else {
                let value = match ty {
                    ResolvedType::Bool => ArgumentValue::Bool(false),
                    ResolvedType::I32 => ArgumentValue::Int32(i32::MIN),
                    ResolvedType::I64 => ArgumentValue::Int(i64::MIN),
                    ResolvedType::U8 => ArgumentValue::Uint8(u8::MAX),
                    ResolvedType::Usize => ArgumentValue::Usize(u32::MAX as u64),
                    // Frozen ordinary Agent result identity excludes these kinds.
                    _ => return Err(SourceJournalError::Binding),
                };
                checkpoint::scalar_json(&value)
            };
            Ok(json!({"identity":id.as_str(),"value":value}))
        })
        .collect::<Result<Vec<_>, SourceJournalError>>()?;
    if fields.len() > 8 {
        return Err(SourceJournalError::Binding);
    }
    Ok(Value::Array(fields))
}
fn failure_statuses(plan: &v2::CheckedOwnedReduceV2) -> Result<Vec<Value>, SourceJournalError> {
    let mut statuses = [
        "fuel_exhausted",
        "host_abandoned",
        "answer_type_mismatch",
        "evaluation_rejected",
        "handler_failed",
        "call_depth_exceeded",
    ]
    .into_iter()
    .map(|failure| json!({"failure":failure,"language_status":null}))
    .collect::<Vec<_>>();
    for source in &plan.function().cleanup_plan.status_sources {
        use crate::cleanup_plan::StatusProducer;
        let values = match &source.producer {
            StatusProducer::ContractFalse { phase, .. } => {
                vec![crate::conformance::NormalizedStatus::contract(*phase)]
            }
            StatusProducer::CheckedArithmetic {
                normalized_cases, ..
            } => normalized_cases
                .iter()
                .map(|n| crate::conformance::NormalizedStatus::arithmetic(*n))
                .collect(),
            StatusProducer::PropagatedCall { .. } => Vec::new(),
        };
        for status in values {
            statuses.push(json!({"failure":"language_failure",
            "language_status":wire::parse(status.to_json().as_bytes())?}));
        }
    }
    // Capacity is the maximum over possible rows. Repeated compiler sites
    // with the same normalized status produce identical rows; retain the first
    // occurrence without changing status acceptance or canonical cleanup order.
    let mut unique = Vec::new();
    for status in statuses {
        if !unique.contains(&status) {
            unique.push(status);
        }
    }
    Ok(unique)
}
fn receipt(active: &Value) -> Result<RoomV8, SourceJournalError> {
    row(
        json!({"kind":"owned_reduce_cleanup_settled","turn":0,"attempt":u32::MAX,
        "started":u32::MAX,"receipt":templates::receipt(active)?}),
    )
}
pub(super) fn failed_state_receipt(active: &Value) -> Result<RoomV8, SourceJournalError> {
    row(
        json!({"kind":"owned_effect_failure_state_cleanup_settled","turn":0,
        "attempt":u32::MAX,"started":u32::MAX,"receipt":templates::receipt(active)?}),
    )
}
fn started(
    basis: &Value,
    operations: &Value,
    binding: &str,
    fuel: usize,
) -> Result<RoomV8, SourceJournalError> {
    row(
        json!({"kind":"owned_reduce_cleanup_started","turn":0,"attempt":u32::MAX,
        "plan":binding,"stage_reservation":u32::MAX,"effect_cleanup_settled":u32::MAX,
        "basis":basis,"basis_digest":hash(),"consumed":fuel,"operations":operations}),
    )
}
fn failure(
    plan: &v2::CheckedOwnedReduceV2,
    mut basis: Value,
    operations: &Value,
    fuel: usize,
    statuses: &[Value],
) -> Result<RoomV8, SourceJournalError> {
    let mut maximum = RoomV8::default();
    let mut admitted = false;
    for status in statuses {
        basis["status"] = status.clone();
        let Ok(checked) = v2::validate_owned_reduce_cleanup_v8(plan, &basis, operations) else {
            continue;
        };
        admitted = true;
        maximum = maximum.either(
            started(&basis, operations, plan.binding(), fuel)?
                .add(receipt(checked.active_operations())?)?
                .add(terminal())?,
        );
    }
    if !admitted {
        return Err(SourceJournalError::Binding);
    }
    Ok(maximum)
}
fn transfer(
    plan: &v2::CheckedOwnedReduceV2,
    case: &str,
    target: &Value,
    cleanup: Value,
) -> Result<(RoomV8, RoomV8), SourceJournalError> {
    v2::validate_owned_reduce_target_v8(plan, case, target)
        .map_err(|_| SourceJournalError::Binding)?;
    let completed = row(
        json!({"kind":"owned_step_transfer_completed","turn":0,"attempt":u32::MAX,
        "reserved":u32::MAX,"target":target,"transfer_digest":hash()}),
    )?
    .add(terminal())?;
    let full = row(
        json!({"kind":"owned_step_transfer_reserved","turn":0,"attempt":u32::MAX,
        "plan":plan.binding(),"stage_reservation":u32::MAX,"staged":u32::MAX,
        "cleanup":cleanup,"case":case}),
    )?
    .add(completed)?;
    Ok((full, completed))
}

pub(super) fn rooms(context: &FoldContextV8) -> Result<ReduceRoomsV8, SourceJournalError> {
    rooms_with_plan(context, context.checked_reduce()?)
}

fn rooms_with_plan(
    context: &FoldContextV8,
    plan: &v2::CheckedOwnedReduceV2,
) -> Result<ReduceRoomsV8, SourceJournalError> {
    let fuel = context
        .ordinary
        .max_steps_per_stage()
        .ok_or(SourceJournalError::Binding)?;
    let statuses = failure_statuses(&plan)?;
    let mut charged = RoomV8::default();
    let mut staged_rooms = Vec::new();
    let initial = v2::owned_wait_operations_v8(&plan.transfers().initial_disposal)
        .map_err(|_| SourceJournalError::Binding)?;
    charged = charged.either(failure(
        &plan,
        json!({"kind":"initial_failure","status":null}),
        &initial,
        fuel,
        &statuses,
    )?);
    for mapping in plan
        .mappings()
        .iter()
        .filter(|m| plan.transfers().cases.iter().any(|c| c.case == m.case))
    {
        let declared = plan
            .helper()
            .program()
            .declarations
            .case_fields(&mapping.case)
            .ok_or(SourceJournalError::Binding)?;
        let step = json!({"declaration":plan.function().return_type.nominal_id().ok_or(SourceJournalError::Binding)?.as_str(),
            "case":mapping.case.as_str(),"fields":maximum_fields(declared.iter().map(|f|(&f.id,&f.ty)))?});
        v2::validate_owned_reduce_step_v8(&plan, &step).map_err(|_| SourceJournalError::Binding)?;
        let scope = match &context.created {
            model::OwnedBodyV8::OwnedRunCreated { scope, .. } => scope,
            _ => return Err(SourceJournalError::Binding),
        };
        let digest = super::super::reduce_wire::recipe_digest(
            super::super::reduce_wire::ReduceRecipeV8::Step,
            &json!({"scope":scope,"binding":plan.binding(),"plan":plan.binding(),"turn":0,"attempt":u32::MAX,
                "stage_reservation":u32::MAX,"step":step}),
        )?;
        let checked = super::super::reduce_inventory::checked_step(
            &plan,
            scope,
            0,
            u32::MAX,
            u32::MAX,
            &step,
            &digest,
        )?;
        let staged = row(
            json!({"kind":"owned_reduce_staged","turn":0,"attempt":u32::MAX,
            "plan":plan.binding(),"stage_reservation":u32::MAX,"effect_cleanup_settled":u32::MAX,
            "step":step,"step_digest":hash(),"consumed":fuel}),
        )?;
        let mut after_staged = RoomV8::default();
        let mut transfer_max = RoomV8::default();
        let mut completed_max = RoomV8::default();
        for c in plan
            .transfers()
            .cases
            .iter()
            .filter(|c| c.case == mapping.case)
        {
            let basis = json!({"kind":"success","staged":u32::MAX,"constructor":c.constructor.as_str(),
                "case":c.case.as_str(),"active_flags":c.completion_live_flags.iter().map(|f|f.0).collect::<Vec<_>>()});
            let ops = v2::owned_wait_operations_v8(&plan.transfers().completion_cleanup)
                .map_err(|_| SourceJournalError::Binding)?;
            let cleanup = v2::validate_owned_reduce_cleanup_v8(&plan, &basis, &ops)
                .map_err(|_| SourceJournalError::Binding)?;
            let empty = cleanup
                .active_operations()
                .as_array()
                .ok_or(SourceJournalError::Binding)?
                .is_empty();
            let kind = if empty {
                json!({"kind":"compiler_empty"})
            } else {
                json!({"kind":"observed","started":u32::MAX,"settled":u32::MAX})
            };
            let (mut closure, completed) =
                transfer(&plan, c.case.as_str(), checked.target(), kind)?;
            transfer_max = transfer_max.either(closure);
            completed_max = completed_max.either(completed);
            if !empty {
                closure = started(&basis, &ops, plan.binding(), fuel)?
                    .add(receipt(cleanup.active_operations())?)?
                    .add(closure)?;
            }
            after_staged = after_staged.either(closure);
            for prefix in 0..=c.fields.len() {
                let actions = &c.failure_by_prefix[prefix];
                let basis = json!({"kind":"partial_failure","status":null,"constructor":c.constructor.as_str(),
                    "case":c.case.as_str(),"transfer_prefix":c.fields[..prefix].iter().map(|f|f.at.as_str()).collect::<Vec<_>>(),
                    "active_flags":actions.iter().map(|a|a.guard_flag.0).collect::<Vec<_>>()});
                let ops = v2::owned_wait_operations_v8(actions)
                    .map_err(|_| SourceJournalError::Binding)?;
                charged = charged.either(failure(&plan, basis, &ops, fuel, &statuses)?);
            }
            let mut flags = c
                .completion_live_flags
                .iter()
                .map(|f| f.0)
                .collect::<Vec<_>>();
            flags.extend(
                plan.transfers()
                    .result_disposal
                    .iter()
                    .filter(|a| a.active_case.as_ref().is_some_and(|x| x.case == c.case))
                    .map(|a| a.guard_flag.0),
            );
            let basis = json!({"kind":"provisional_failure","status":null,"constructor":c.constructor.as_str(),
                "case":c.case.as_str(),"active_flags":flags});
            let ops = v2::owned_wait_operations_v8(&plan.transfers().provisional_failure)
                .map_err(|_| SourceJournalError::Binding)?;
            charged = charged.either(failure(&plan, basis, &ops, fuel, &statuses)?);
        }
        charged = charged.either(staged.add(after_staged)?);
        staged_rooms.push(ReduceCaseRoomsV8 {
            case: mapping.case.as_str().to_owned(),
            staged: after_staged,
            transfer: transfer_max,
            completed: completed_max,
        });
    }
    let stage = ordinary(SourceJournalEntry::StageReservation {
        turn: 0,
        attempt: Some(u32::MAX),
        role: super::super::super::SourceStageRole::Reduce,
        fuel,
    })?;
    if context
        .checked_binding
        .helper()
        .liveness()
        .result_disposal
        .iter()
        .any(|a| a.active_case.is_some())
    {
        return Err(SourceJournalError::Binding);
    }
    let state_ops =
        v2::owned_wait_operations_v8(&context.checked_binding.helper().liveness().result_disposal)
            .map_err(|_| SourceJournalError::Binding)?;
    let failed_state=row(json!({"kind":"owned_effect_failure_state_cleanup_started","turn":0,"attempt":u32::MAX,
        "plan":plan.binding(),"settlement":u32::MAX,"recorded":u32::MAX,"decision_cleanup_settled":u32::MAX,
        "effect_failure":"handler_failed","state_digest":hash(),"operations":state_ops}))?
        .add(failed_state_receipt(&state_ops)?)?.add(terminal())?;
    Ok(ReduceRoomsV8 {
        before_stage: stage.add(charged)?,
        charged,
        cases: staged_rooms,
        mapped: terminal(),
        failed_state,
    })
}

#[cfg(test)]
#[path = "reduce/tests.rs"]
mod tests;
