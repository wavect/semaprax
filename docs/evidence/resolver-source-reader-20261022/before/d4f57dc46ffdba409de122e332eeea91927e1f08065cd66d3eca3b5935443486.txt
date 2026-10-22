//! Constructor-local funding; shape and canonical plan builders remain ordinary.
use super::*;
use crate::cleanup_plan::ReplayClonePayload;
use std::fmt::Write as _;
mod copy;

pub(in crate::hir) fn reserve(bytes: Option<usize>) -> Result<(), Diagnostic> {
    let Some(bytes) = bytes else {
        crate::bounded_output::refuse_active_required(usize::MAX);
        return Err(hir_error_static("closure constructor layout overflowed"));
    };
    if crate::bounded_output::reserve_active_required(bytes) {
        Ok(())
    } else {
        Err(hir_error_static(
            "closure constructor exceeds the active validation budget",
        ))
    }
}
struct Count(usize);
impl std::fmt::Write for Count {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        self.0 = self.0.checked_add(text.len()).ok_or(std::fmt::Error)?;
        Ok(())
    }
}
fn text(arguments: std::fmt::Arguments<'_>) -> Result<String, Diagnostic> {
    let mut count = Count(0);
    if count.write_fmt(arguments).is_err() {
        reserve(None)?;
    }
    reserve(Some(count.0))?;
    let mut output = String::with_capacity(count.0);
    reserve(Some(output.capacity() - count.0))?;
    output.write_fmt(arguments).expect("counted closure text");
    Ok(output)
}
pub(in crate::hir) fn capture_path(path: &str, index: usize) -> Result<String, Diagnostic> {
    text(format_args!("{path}.capture.{index}"))
}
pub(in crate::hir) fn identity(expression: &ExpressionId) -> Result<DeclarationId, Diagnostic> {
    DeclarationId::paid_owned(
        text(format_args!("semaprax.closure.v1:{}", expression.as_str()))?,
        |bytes| reserve(Some(bytes)),
    )
}
fn result(id: &DeclarationId) -> Result<ValueId, Diagnostic> {
    let spelling = text(format_args!(
        "declaration:{}:{}:value:result:0:",
        id.as_str().len(),
        id
    ))?;
    reserve(DeclarationId::shared_backing_bytes(spelling.len()))?;
    Ok(ValueId::new(spelling))
}
fn ty(value: &ResolvedType) -> Result<ResolvedType, Diagnostic> {
    if let Some(bytes) = value.clone_payload_bytes() {
        reserve(Some(bytes))?;
    } else {
        // Shape admission does not constrain every hostile metadata type tree.
        // Keep the complete old clone if the existing bounded type census cannot
        // describe it; this is an explicit unpaid residual, never a new refusal
        // based on an assumed admitted type depth or visit count.
        reserve(Some(0))?;
    }
    Ok(value.clone())
}
fn vector<T>(count: usize) -> Result<Vec<T>, Diagnostic> {
    reserve(count.checked_mul(std::mem::size_of::<T>()))?;
    let output = Vec::with_capacity(count);
    reserve((output.capacity() - count).checked_mul(std::mem::size_of::<T>()))?;
    Ok(output)
}
pub(in crate::hir) fn function(
    program: &ResolvedProgram,
    expression: &ResolvedExpr,
) -> Result<ResolvedFunction, Diagnostic> {
    crate::hir::closure::validate_shape(program, expression)?;
    let ResolvedExprKind::Closure {
        parameters,
        captures,
        body,
    } = &expression.kind
    else {
        return Err(hir_error("expected closure expression"));
    };
    let result_type = match &expression.ty {
        ResolvedType::Function { result, .. } => result.as_ref(),
        ResolvedType::OnceFunction
        | ResolvedType::OnceFunctionI64
        | ResolvedType::OnceFunctionI64Pair
        | ResolvedType::MutFunctionI64 => &ResolvedType::I64,
        _ => return Err(hir_error("closure has no callable signature")),
    };
    let id = identity(&expression.id)?;
    let execution = FunctionExecutionId::Monomorphic(id.clone());
    let mut params = vector(captures.len() + parameters.len())?;
    for binding in captures
        .iter()
        .map(|capture| &capture.binding)
        .chain(parameters)
    {
        let name = text(format_args!("{}", binding.name))?;
        params.push(ResolvedParam {
            id: binding.id.clone(),
            name,
            ownership: binding.ownership,
            ty: ty(&binding.ty)?,
            span: binding.span,
        });
    }
    let mut function = ResolvedFunction {
        id: id.clone(),
        name: text(format_args!("closure"))?,
        params,
        result_id: result(&id)?,
        return_type: ty(result_type)?,
        effects: Vec::new(),
        yields: None,
        requires: Vec::new(),
        ensures: Vec::new(),
        body: copy::expression(body)?,
        cleanup: crate::cleanup::CleanupInventory::unresolved(),
        cleanup_plan: crate::cleanup_plan::CleanupPlan::unresolved(),
        loan_plan: crate::loan_plan::LoanPlan::unresolved(),
        span: expression.span,
    };
    // These complete ordinary builders retain their original order and work.
    // Their residual allocations are not covered by this payload funding claim.
    function.loan_plan = crate::loan_plan::build_plan(program, &function)?;
    function.cleanup = crate::cleanup::build_inventory(program, &function)?;
    function.cleanup_plan =
        crate::cleanup_plan::build_plan_for_execution(program, &function, &execution)?;
    Ok(function)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[rustfmt::skip]
mod original_tests;
