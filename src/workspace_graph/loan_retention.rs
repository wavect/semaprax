//! Physical union of retained HIR identities and its loan-proof sidecar.
//!
//! The core prebound owns retained HIR text (the sixteen-identity payload
//! allowance) and shared carriers (the explicit retained/direct AST allowance,
//! including defaults, generic instances and desugared loops). A loan point
//! clones its body's ExpressionId backing. Count that allocation once, while
//! retaining every proof vector/place and every independently allocated ID.
//! This is storage accounting only; pointer keys never establish graph meaning.

use crate::diagnostic::Diagnostic;
use crate::hir::{ExpressionId, ResolvedExpr, ResolvedExprKind as E, ResolvedFunction};
use crate::loan_plan::LoanPlan;

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

fn refusal() -> Vec<Diagnostic> {
    vec![super::limit_error(
        "builder_bytes",
        super::active_builder_limit(),
    )]
}

pub(super) fn retained_loan_plan_bytes(plan: &LoanPlan) -> Result<usize> {
    if plan.loans.is_empty() {
        return Ok(0);
    }
    crate::loan_plan::owned_capacity_bytes(plan)
        .and_then(|owned| std::mem::size_of::<LoanPlan>().checked_add(owned))
        .ok_or_else(refusal)
}

pub(super) fn retained_function_loan_bytes(function: &ResolvedFunction) -> Result<usize> {
    if function.loan_plan.loans.is_empty() {
        return Ok(0);
    }
    let mut count = 0usize;
    match visit_function(function, &mut |identity| {
        identity.shared_allocation_key().ok_or(WalkError::Invalid)?;
        identity
            .shared_allocation_bytes()
            .ok_or(WalkError::Invalid)?;
        count = count.checked_add(1).ok_or(WalkError::Invalid)?;
        if count > crate::loan_plan::MAX_LOAN_ENDPOINTS_V1 {
            return Err(WalkError::Uncertain);
        }
        Ok(())
    }) {
        Ok(()) => {}
        // A bounded inventory is optional proof of sharing. When its limits
        // cannot establish that proof, retain the conservative full debit.
        Err(WalkError::Uncertain) => return retained_loan_plan_bytes(&function.loan_plan),
        Err(WalkError::Invalid) => return Err(refusal()),
    }
    let bytes = count
        .checked_mul(std::mem::size_of::<usize>())
        .ok_or_else(refusal)?;
    super::reserve_builder_structure(bytes)?;
    let mut keys = Vec::with_capacity(count);
    let excess = keys
        .capacity()
        .checked_sub(count)
        .and_then(|count| count.checked_mul(std::mem::size_of::<usize>()))
        .ok_or_else(refusal)?;
    super::reserve_builder_structure(excess)?;
    visit_function(function, &mut |identity| {
        keys.push(identity.shared_allocation_key().ok_or(WalkError::Invalid)?);
        Ok(())
    })
    .map_err(|_| refusal())?;
    debug_assert_eq!(keys.len(), count);
    keys.sort_unstable();
    keys.dedup();
    crate::loan_plan::owned_capacity::owned_capacity_bytes_excluding(&function.loan_plan, &keys)
        .and_then(|owned| std::mem::size_of::<LoanPlan>().checked_add(owned))
        .ok_or_else(refusal)
}

enum WalkError {
    Invalid,
    Uncertain,
}

type WalkResult = std::result::Result<(), WalkError>;

fn visit_function(
    function: &ResolvedFunction,
    visit: &mut impl FnMut(&ExpressionId) -> WalkResult,
) -> WalkResult {
    for expression in function
        .requires
        .iter()
        .chain(&function.ensures)
        .chain([&function.body])
    {
        walk(expression, 0, visit)?;
    }
    Ok(())
}

fn walk(
    expression: &ResolvedExpr,
    depth: usize,
    visit: &mut impl FnMut(&ExpressionId) -> WalkResult,
) -> WalkResult {
    if depth >= crate::cache_codec::MAX_DEPTH {
        return Err(WalkError::Uncertain);
    }
    visit(&expression.id)?;
    match &expression.kind {
        E::NativeRustImportCall(call) => visit(&call.expression)?,
        E::HostCommandCall(call) => visit(&call.expression)?,
        _ => {}
    }
    let mut child = |expression: &ResolvedExpr| walk(expression, depth + 1, visit);
    match &expression.kind {
        E::Closure { captures, body, .. } => {
            for capture in captures {
                child(&capture.value)?;
            }
            child(body)?;
        }
        E::Invoke { callable, args } => {
            child(callable)?;
            for argument in args {
                child(argument)?;
            }
        }
        E::Call { args, .. } => {
            for argument in args {
                child(argument)?;
            }
        }
        E::NativeRustImportCall(call) => {
            // The call carrier may alias the root or have its own immutable
            // backing. Its direct-lowering allowance covers either case.
            for argument in &call.args {
                child(argument)?;
            }
        }
        E::HostCommandCall(call) => {
            for argument in &call.args {
                child(argument)?;
            }
        }
        E::ByteRange {
            source, start, end, ..
        } => {
            child(source)?;
            child(start)?;
            child(end)?;
        }
        E::Unary { value, .. } => child(value)?,
        E::Binary { left, right, .. } => {
            child(left)?;
            child(right)?;
        }
        E::Block { statements, tail } => {
            for statement in statements {
                for index in 0..statement.child_count() {
                    child(statement.child(index).ok_or(WalkError::Invalid)?)?;
                }
            }
            child(tail)?;
        }
        E::If {
            condition,
            then_branch,
            else_branch,
        } => {
            child(condition)?;
            child(then_branch)?;
            child(else_branch)?;
        }
        E::ConstructRecord { fields, .. } | E::ConstructVariant { fields, .. } => {
            for field in fields {
                child(&field.value)?;
            }
        }
        E::Match {
            scrutinee, arms, ..
        } => {
            child(scrutinee)?;
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    child(guard)?;
                }
                child(&arm.value)?;
            }
        }
        E::Try { operand, .. } | E::TryOption { operand, .. } => child(operand)?,
        E::UpdateRecord { base, fields, .. } => {
            child(base)?;
            for field in fields {
                child(&field.value)?;
            }
        }
        E::Project { base, .. } => child(base)?,
        E::Upcast { source } => child(source)?,
        E::Yield { request } => child(request)?,
        E::Int(_)
        | E::Int32(_)
        | E::Char(_)
        | E::Uint8(_)
        | E::Usize(_)
        | E::ArrayU8(_)
        | E::RepeatArrayU8 { .. }
        | E::Float32(_)
        | E::Float64(_)
        | E::Bool(_)
        | E::String(_)
        | E::Place(_)
        | E::BorrowPlace { .. }
        | E::FunctionReference { .. } => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests;
