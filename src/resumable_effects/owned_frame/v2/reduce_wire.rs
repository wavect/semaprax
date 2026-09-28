//! Compiler-derived inert Reduce facts. No owner, restoration or release API.
use super::data::{
    owned_wait_operations_v8, validate_owned_wait_observed_receipt_v8,
    validate_owned_wait_operations_v8,
};
use super::reduce_plan::CheckedOwnedReduceV2;
use crate::cleanup_plan::FinalizeAction;
use crate::hir::{DeclarationId, ExpressionId, ResolvedExpr, ResolvedExprKind, ResolvedType};
use crate::interpreter::ArgumentValue;
use crate::resumable_effects::owned_frame::{codec, OwnedFrameError as Error};
use serde_json::Value;

pub(crate) struct CheckedOwnedReduceCleanupV8 {
    operations: Value,
    active_operations: Value,
    flags: Vec<u32>,
    failure: Option<Value>,
}
impl CheckedOwnedReduceCleanupV8 {
    pub(crate) fn operations(&self) -> &Value {
        &self.operations
    }
    pub(crate) fn active_operations(&self) -> &Value {
        &self.active_operations
    }
    pub(crate) fn flags(&self) -> &[u32] {
        &self.flags
    }
    pub(crate) fn failure(&self) -> Option<&Value> {
        self.failure.as_ref()
    }
    pub(crate) fn validate_receipt(&self, receipt: &Value) -> Result<(), Error> {
        validate_owned_wait_observed_receipt_v8(&self.active_operations, receipt)
    }
}
fn bounded(value: &Value) -> Result<(), Error> {
    if codec::canonical(value).len() > codec::MAX_CARRIER {
        return Err(Error::Capacity);
    }
    Ok(())
}
fn leaf(ty: &ResolvedType, value: &Value) -> Result<(), Error> {
    if *ty == ResolvedType::Bytes {
        codec::keys(value, &["kind", "hex"])?;
        if value["kind"] != "bytes" {
            return Err(Error::Binding);
        }
        codec::unhex(value["hex"].as_str().ok_or(Error::Malformed)?, 1024)?;
        return Ok(());
    }
    let scalar = codec::decode_scalar(value)?;
    if !matches!(
        (ty, scalar),
        (ResolvedType::Bool, ArgumentValue::Bool(_))
            | (ResolvedType::I32, ArgumentValue::Int32(_))
            | (ResolvedType::I64, ArgumentValue::Int(_))
            | (ResolvedType::U8, ArgumentValue::Uint8(_))
            | (ResolvedType::Usize, ArgumentValue::Usize(_))
            | (ResolvedType::Char, ArgumentValue::Char(_))
            | (ResolvedType::F32, ArgumentValue::Float32(_))
            | (ResolvedType::F64, ArgumentValue::Float64(_))
    ) {
        return Err(Error::Binding);
    }
    Ok(())
}
fn fields<'a>(
    value: &Value,
    expected: impl ExactSizeIterator<Item = (&'a DeclarationId, &'a ResolvedType)>,
) -> Result<(), Error> {
    let fields = value.as_array().ok_or(Error::Malformed)?;
    if fields.len() != expected.len() || fields.len() > 8 {
        return Err(Error::Binding);
    }
    for (field, (id, ty)) in fields.iter().zip(expected) {
        codec::keys(field, &["identity", "value"])?;
        if field["identity"] != id.as_str() {
            return Err(Error::Binding);
        }
        leaf(ty, &field["value"])?;
    }
    Ok(())
}
pub(crate) fn validate_owned_reduce_step_v8(
    plan: &CheckedOwnedReduceV2,
    value: &Value,
) -> Result<(), Error> {
    bounded(value)?;
    codec::keys(value, &["declaration", "case", "fields"])?;
    let nominal = plan
        .function()
        .return_type
        .nominal_id()
        .ok_or(Error::Binding)?;
    if value["declaration"] != nominal.as_str() {
        return Err(Error::Binding);
    }
    let mapping = plan
        .mappings()
        .iter()
        .find(|m| value["case"] == m.case.as_str())
        .ok_or(Error::Binding)?;
    let declared = plan
        .helper()
        .program()
        .declarations
        .case_fields(&mapping.case)
        .ok_or(Error::Binding)?;
    fields(&value["fields"], declared.iter().map(|f| (&f.id, &f.ty)))
}
pub(crate) fn validate_owned_reduce_target_v8(
    plan: &CheckedOwnedReduceV2,
    case: &str,
    value: &Value,
) -> Result<(), Error> {
    bounded(value)?;
    let mapping = plan
        .mappings()
        .iter()
        .find(|m| m.case.as_str() == case)
        .ok_or(Error::Binding)?;
    let (kind, field) = match mapping.role {
        "Continue" => ("continue", "state"),
        "Suspend" => ("suspend", "state"),
        "Complete" => ("complete", "report"),
        "Fail" => ("fail", "code"),
        _ => return Err(Error::Binding),
    };
    codec::keys(value, &["kind", field])?;
    if value["kind"] != kind {
        return Err(Error::Binding);
    }
    if kind == "fail" {
        value["code"].as_i64().ok_or(Error::Malformed)?;
        return Ok(());
    }
    let record = &value[field];
    codec::keys(record, &["declaration", "fields"])?;
    if record["declaration"] != mapping.target.as_str() {
        return Err(Error::Binding);
    }
    let declared = plan
        .helper()
        .program()
        .declarations
        .record_fields(&mapping.target)
        .ok_or(Error::Binding)?;
    fields(&record["fields"], declared.iter().map(|f| (&f.id, &f.ty)))
}
// The checked Reduce profile has empty Block prefixes, scalar-copy conditions
// and direct constructors; it admits no let/call prefix. Restrict arithmetic to
// exactly the evaluation segment represented by the committed-owner basis.
fn copy_contains(expr: &ResolvedExpr, at: &ExpressionId) -> bool {
    if expr.id == *at {
        return true;
    }
    match &expr.kind {
        ResolvedExprKind::Unary { value, .. } => copy_contains(value, at),
        ResolvedExprKind::Binary { left, right, .. } => {
            copy_contains(left, at) || copy_contains(right, at)
        }
        _ => false,
    }
}
fn conditions_contain(expr: &ResolvedExpr, at: &ExpressionId) -> bool {
    match &expr.kind {
        ResolvedExprKind::Block { statements, tail } if statements.is_empty() => {
            conditions_contain(tail, at)
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            copy_contains(condition, at)
                || conditions_contain(then_branch, at)
                || conditions_contain(else_branch, at)
        }
        _ => false,
    }
}
fn constructor<'a>(expr: &'a ResolvedExpr, at: &str) -> Option<&'a ResolvedExpr> {
    if expr.id.as_str() == at && matches!(expr.kind, ResolvedExprKind::ConstructVariant { .. }) {
        return Some(expr);
    }
    match &expr.kind {
        ResolvedExprKind::Block { statements, tail } if statements.is_empty() => {
            constructor(tail, at)
        }
        ResolvedExprKind::If {
            then_branch,
            else_branch,
            ..
        } => constructor(then_branch, at).or_else(|| constructor(else_branch, at)),
        _ => None,
    }
}
fn arithmetic_reachable(plan: &CheckedOwnedReduceV2, basis: &Value, at: &ExpressionId) -> bool {
    let f = plan.function();
    match basis["kind"].as_str() {
        Some("initial_failure") => {
            f.requires.iter().any(|e| copy_contains(e, at)) || conditions_contain(&f.body, at)
        }
        Some("provisional_failure") => f.ensures.iter().any(|e| copy_contains(e, at)),
        Some("partial_failure") => {
            let Some(case) = plan.transfers().cases.iter().find(|c| {
                basis["constructor"] == c.constructor.as_str() && basis["case"] == c.case.as_str()
            }) else {
                return false;
            };
            let Some(prefix) = basis["transfer_prefix"].as_array() else {
                return false;
            };
            let Some(expr) = constructor(&f.body, case.constructor.as_str()) else {
                return false;
            };
            let ResolvedExprKind::ConstructVariant { fields, .. } = &expr.kind else {
                return false;
            };
            let start = if prefix.is_empty() {
                0
            } else {
                match case
                    .fields
                    .get(prefix.len() - 1)
                    .and_then(|field| field.field_index.checked_add(1))
                {
                    Some(n) => n,
                    None => return false,
                }
            };
            let end = match case.fields.get(prefix.len()) {
                Some(field) => match field.field_index.checked_add(1) {
                    Some(n) => n,
                    None => return false,
                },
                None => fields.len(),
            };
            fields
                .get(start..end)
                .is_some_and(|segment| segment.iter().any(|field| copy_contains(&field.value, at)))
        }
        _ => false,
    }
}
fn failure(plan: &CheckedOwnedReduceV2, basis: &Value) -> Result<(), Error> {
    let status = &basis["status"];
    bounded(status)?;
    codec::keys(status, &["failure", "language_status"])?;
    let tag = status["failure"].as_str().ok_or(Error::Malformed)?;
    if tag != "language_failure" {
        if ![
            "fuel_exhausted",
            "host_abandoned",
            "answer_type_mismatch",
            "evaluation_rejected",
            "handler_failed",
            "call_depth_exceeded",
        ]
        .contains(&tag)
            || !status["language_status"].is_null()
        {
            return Err(Error::Binding);
        }
        return Ok(());
    }
    let f = plan.function();
    for source in &f.cleanup_plan.status_sources {
        use crate::cleanup_plan::{ContractPhase, StatusProducer};
        let values = match &source.producer {
            StatusProducer::ContractFalse { phase, ordinal } => {
                let (kind, contracts) = match phase {
                    ContractPhase::Requires => ("initial_failure", &f.requires),
                    ContractPhase::Ensures => ("provisional_failure", &f.ensures),
                };
                if basis["kind"] != kind
                    || !contracts
                        .get(*ordinal as usize)
                        .is_some_and(|c| c.id == source.id.expression)
                {
                    continue;
                }
                vec![crate::conformance::NormalizedStatus::contract(*phase)]
            }
            StatusProducer::CheckedArithmetic {
                normalized_cases, ..
            } => {
                if !arithmetic_reachable(plan, basis, &source.id.expression) {
                    continue;
                }
                normalized_cases
                    .iter()
                    .map(|c| crate::conformance::NormalizedStatus::arithmetic(*c))
                    .collect()
            }
            StatusProducer::PropagatedCall { .. } => Vec::new(),
        };
        for value in values {
            if status["language_status"]
                == codec::parse(value.to_json().as_bytes(), codec::MAX_CARRIER)?
            {
                return Ok(());
            }
        }
    }
    Err(Error::Binding)
}
pub(crate) fn validate_owned_reduce_cleanup_v8(
    plan: &CheckedOwnedReduceV2,
    basis: &Value,
    full_operations: &Value,
) -> Result<CheckedOwnedReduceCleanupV8, Error> {
    bounded(basis)?;
    let kind = basis["kind"].as_str().ok_or(Error::Malformed)?;
    let failed = kind != "success";
    let (actions, flags): (&[FinalizeAction], Vec<u32>) = if kind == "initial_failure" {
        codec::keys(basis, &["kind", "status"])?;
        let actions = &plan.transfers().initial_disposal;
        (actions, actions.iter().map(|a| a.guard_flag.0).collect())
    } else {
        let expected: &[&str] = match kind {
            "partial_failure" => &[
                "kind",
                "status",
                "constructor",
                "case",
                "transfer_prefix",
                "active_flags",
            ],
            "provisional_failure" => &["kind", "status", "constructor", "case", "active_flags"],
            "success" => &["kind", "staged", "constructor", "case", "active_flags"],
            _ => return Err(Error::Malformed),
        };
        codec::keys(basis, expected)?;
        let case = plan
            .transfers()
            .cases
            .iter()
            .find(|c| {
                basis["constructor"] == c.constructor.as_str() && basis["case"] == c.case.as_str()
            })
            .ok_or(Error::Binding)?;
        let (actions, flags) = if kind == "partial_failure" {
            let prefix = basis["transfer_prefix"]
                .as_array()
                .ok_or(Error::Malformed)?;
            if prefix.len() > case.fields.len()
                || prefix
                    .iter()
                    .zip(&case.fields)
                    .any(|(a, e)| a.as_str() != Some(e.at.as_str()))
            {
                return Err(Error::Binding);
            }
            let actions = case
                .failure_by_prefix
                .get(prefix.len())
                .ok_or(Error::Binding)?;
            (
                actions.as_slice(),
                actions.iter().map(|a| a.guard_flag.0).collect::<Vec<_>>(),
            )
        } else {
            let mut flags: Vec<_> = case.completion_live_flags.iter().map(|f| f.0).collect();
            if failed {
                flags.extend(
                    plan.transfers()
                        .result_disposal
                        .iter()
                        .filter(|a| a.active_case.as_ref().is_some_and(|c| c.case == case.case))
                        .map(|a| a.guard_flag.0),
                );
            } else {
                basis["staged"]
                    .as_u64()
                    .and_then(|n| u32::try_from(n).ok())
                    .ok_or(Error::Malformed)?;
            }
            (
                if failed {
                    plan.transfers().provisional_failure.as_slice()
                } else {
                    plan.transfers().completion_cleanup.as_slice()
                },
                flags,
            )
        };
        let supplied = basis["active_flags"].as_array().ok_or(Error::Malformed)?;
        if supplied.len() != flags.len()
            || supplied
                .iter()
                .zip(&flags)
                .any(|(a, e)| a.as_u64() != Some(u64::from(*e)))
        {
            return Err(Error::Binding);
        }
        (actions, flags)
    };
    if failed {
        failure(plan, basis)?;
    }
    validate_owned_wait_operations_v8(actions, full_operations)?;
    let active: Vec<_> = actions
        .iter()
        .filter(|a| flags.contains(&a.guard_flag.0))
        .cloned()
        .collect();
    Ok(CheckedOwnedReduceCleanupV8 {
        operations: full_operations.clone(),
        active_operations: owned_wait_operations_v8(&active)?,
        flags,
        failure: failed.then(|| basis["status"].clone()),
    })
}

#[cfg(test)]
#[path = "reduce_wire/tests.rs"]
mod tests;
