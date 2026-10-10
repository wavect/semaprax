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

/// A final uncached build reuses this allocation across retained functions and
/// modules. Growth reserves the entire replacement allocation before creating
/// it; prior charges remain in the cumulative ledger. Keys are cleared before
/// each census, so unrelated functions can never supply exclusion authority.
#[derive(Default)]
pub(super) struct RetentionScratch {
    keys: Vec<usize>,
    matched: Vec<u8>,
}

impl RetentionScratch {
    fn prepare<'a>(&mut self, functions: impl Iterator<Item = &'a ResolvedFunction>) -> Result<()> {
        let mut maximum = 0;
        for function in functions {
            if let Some(count) = candidate_count(function)? {
                maximum = maximum.max(count);
            }
        }
        if maximum > self.capacity() {
            *self = allocate_candidates(maximum)?;
        }
        Ok(())
    }

    fn capacity(&self) -> usize {
        self.keys.capacity().min(self.matched.capacity())
    }

    pub(super) fn measure(&mut self, function: &ResolvedFunction) -> Result<usize> {
        self.keys.clear();
        self.matched.clear();
        let Some(count) = candidate_count(function)? else {
            return retained_loan_plan_bytes(&function.loan_plan);
        };
        if count > self.capacity() {
            *self = allocate_candidates(count)?;
        }
        for identity in crate::loan_plan::owned_capacity::proof_identities(&function.loan_plan) {
            self.keys
                .push(identity.shared_allocation_key().ok_or_else(refusal)?);
        }
        self.keys.sort_unstable();
        self.keys.dedup();
        self.matched.resize(self.keys.len(), 0);
        // Only live HIR backing may authorize an exclusion. Check every HIR
        // identity, but retain no key that the loan sidecar cannot reference.
        let visited = visit_function(function, &mut |identity| {
            let key = identity.shared_allocation_key().ok_or(WalkError::Invalid)?;
            identity
                .shared_allocation_bytes()
                .ok_or(WalkError::Invalid)?;
            if let Ok(index) = self.keys.binary_search(&key) {
                self.matched[index] = 1;
            }
            Ok(())
        });
        if let Err(error) = visited {
            self.keys.clear();
            self.matched.clear();
            return match error {
                WalkError::Uncertain => retained_loan_plan_bytes(&function.loan_plan),
                WalkError::Invalid => Err(refusal()),
            };
        }
        let mut length = 0;
        for index in 0..self.keys.len() {
            if self.matched[index] != 0 {
                self.keys[length] = self.keys[index];
                length += 1;
            }
        }
        self.keys.truncate(length);
        self.matched.clear();
        crate::loan_plan::owned_capacity::owned_capacity_bytes_excluding(
            &function.loan_plan,
            &self.keys,
        )
        .and_then(|owned| std::mem::size_of::<LoanPlan>().checked_add(owned))
        .ok_or_else(refusal)
    }
}

pub(super) fn authored_function(
    function: &ResolvedFunction,
    program: &crate::ast::Program,
    authored: &std::collections::BTreeMap<&str, super::AuthoredDeclaration<'_>>,
) -> bool {
    authored
        .get(function.id.as_str())
        .is_some_and(|owner| owner.module == program.module)
}

/// Reserve the largest selected census once before consuming this module.
/// The physical carrier is reused; each later module still charges any complete
/// replacement before allocation. Legacy retention takes its original path.
pub(super) fn prepare_functions(
    functions: Vec<ResolvedFunction>,
    scratch: Option<&mut RetentionScratch>,
    program: &crate::ast::Program,
    authored: &std::collections::BTreeMap<&str, super::AuthoredDeclaration<'_>>,
) -> Result<Vec<ResolvedFunction>> {
    if let Some(scratch) = scratch {
        scratch.prepare(
            functions
                .iter()
                .filter(|function| authored_function(function, program, authored)),
        )?;
    }
    Ok(functions)
}

pub(super) fn retained_function_loan_bytes(function: &ResolvedFunction) -> Result<usize> {
    // Earlier attempts preserve their established per-function scratch receipt.
    let Some(count) = inventory_count(function)? else {
        return retained_loan_plan_bytes(&function.loan_plan);
    };
    inventory_bytes(function, &mut allocate_keys(count)?)
}

fn inventory_count(function: &ResolvedFunction) -> Result<Option<usize>> {
    if function.loan_plan.loans.is_empty() {
        return Ok(None);
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
        Ok(()) => Ok(Some(count)),
        // A bounded inventory is optional proof of sharing. When its limits
        // cannot establish that proof, retain the conservative full debit.
        Err(WalkError::Uncertain) => Ok(None),
        Err(WalkError::Invalid) => Err(refusal()),
    }
}

/// Count all candidate references, including duplicates, before allocation.
/// The legacy HIR walk preserves its bounded/invalid-backing fallback contract.
fn candidate_count(function: &ResolvedFunction) -> Result<Option<usize>> {
    // Validate the full proof even when the HIR census will fall back.
    let mut count = 0usize;
    for identity in crate::loan_plan::owned_capacity::proof_identities(&function.loan_plan) {
        identity.shared_allocation_key().ok_or_else(refusal)?;
        identity.shared_allocation_bytes().ok_or_else(refusal)?;
        count = count.checked_add(1).ok_or_else(refusal)?;
    }
    if inventory_count(function)?.is_none() {
        return Ok(None);
    }
    Ok(Some(count))
}

fn allocate_candidates(count: usize) -> Result<RetentionScratch> {
    // Reserve both requested carriers before either is allocated. Charge any
    // allocator capacity excess too; replacing them never refunds old charges.
    let bytes = count
        .checked_mul(std::mem::size_of::<usize>() + std::mem::size_of::<u8>())
        .ok_or_else(refusal)?;
    super::reserve_builder_structure(bytes)?;
    let keys = Vec::<usize>::with_capacity(count);
    let matched = Vec::<u8>::with_capacity(count);
    let excess = keys
        .capacity()
        .checked_sub(count)
        .and_then(|extra| extra.checked_mul(std::mem::size_of::<usize>()))
        .and_then(|bytes| bytes.checked_add(matched.capacity().checked_sub(count)?))
        .ok_or_else(refusal)?;
    super::reserve_builder_structure(excess)?;
    Ok(RetentionScratch { keys, matched })
}

fn allocate_keys(count: usize) -> Result<Vec<usize>> {
    let bytes = count
        .checked_mul(std::mem::size_of::<usize>())
        .ok_or_else(refusal)?;
    super::reserve_builder_structure(bytes)?;
    let keys = Vec::with_capacity(count);
    let excess = keys
        .capacity()
        .checked_sub(count)
        .and_then(|count| count.checked_mul(std::mem::size_of::<usize>()))
        .ok_or_else(refusal)?;
    super::reserve_builder_structure(excess)?;
    Ok(keys)
}

fn inventory_bytes(function: &ResolvedFunction, keys: &mut Vec<usize>) -> Result<usize> {
    visit_function(function, &mut |identity| {
        keys.push(identity.shared_allocation_key().ok_or(WalkError::Invalid)?);
        Ok(())
    })
    .map_err(|_| refusal())?;
    keys.sort_unstable();
    keys.dedup();
    crate::loan_plan::owned_capacity::owned_capacity_bytes_excluding(&function.loan_plan, keys)
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
        E::Call { args, .. } | E::LiteralFormat { args, .. } | E::VecFieldRead { args, .. } => {
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
mod scratch_tests;
#[cfg(test)]
mod tests;
