use crate::diagnostic::Diagnostic;
use crate::hir::{self, ResolvedExpr, ResolvedExprKind, ResolvedProgram, ResolvedType};
use std::collections::BTreeMap;

use super::{backend_error, c_value_type, CEmitter, COutput, CValue};

pub(super) fn c_type(ty: &ResolvedType) -> Result<String, Diagnostic> {
    if !hir::function_value::is_signature(ty) {
        return Err(backend_error(
            "native function-value type is not an admitted scalar signature",
        ));
    }
    let mut symbol = String::from("spx_fn_");
    for byte in ty.identity_key().bytes() {
        use std::fmt::Write as _;
        write!(symbol, "{byte:02x}").expect("writing to a string cannot fail");
    }
    Ok(symbol)
}

pub(super) fn emit_typedefs(
    output: &mut impl COutput,
    program: &ResolvedProgram,
    resource_abi: &super::native_resource::NativeResourceAbi,
) -> Result<(), Diagnostic> {
    let mut signatures = BTreeMap::new();
    for function in &program.functions {
        insert(&mut signatures, &function.return_type)?;
        for parameter in &function.params {
            insert(&mut signatures, &parameter.ty)?;
        }
        let mut failure = None;
        hir::function_value::walk(function, |expression| {
            if failure.is_none() {
                failure = insert(&mut signatures, &expression.ty).err();
            }
        });
        if let Some(failure) = failure {
            return Err(failure);
        }
    }
    for signature in signatures.into_values() {
        let ResolvedType::Function { parameters, result } = signature else {
            unreachable!()
        };
        write!(
            output,
            "typedef spx_status_token (*{})(struct spx_context *spx_ctx",
            c_type(&ResolvedType::Function {
                parameters: parameters.clone(),
                result: result.clone()
            })?
        )
        .expect("writing to a string cannot fail");
        for parameter in &parameters {
            write!(
                output,
                ", {}",
                c_value_type(program, resource_abi, parameter)?
            )
            .expect("writing to a string cannot fail");
        }
        writeln!(
            output,
            ", {} *spx_result_out);",
            c_value_type(program, resource_abi, &result)?
        )
        .expect("writing to a string cannot fail");
    }
    if hir::function_value::requires_function_values(program) {
        output.push('\n');
    }
    Ok(())
}

fn insert(
    signatures: &mut BTreeMap<String, ResolvedType>,
    ty: &ResolvedType,
) -> Result<(), Diagnostic> {
    if matches!(ty, ResolvedType::Function { .. }) {
        if !hir::function_value::is_signature(ty) {
            return Err(backend_error(
                "native function-value signature is not admitted",
            ));
        }
        signatures.insert(ty.identity_key(), ty.clone());
    }
    Ok(())
}

pub(super) fn emit_reference<O: COutput>(
    emitter: &mut CEmitter<'_, O>,
    expr: &ResolvedExpr,
) -> Result<CValue, Diagnostic> {
    let ResolvedExprKind::FunctionReference { target } = &expr.kind else {
        unreachable!()
    };
    hir::function_value::validate_reference(emitter.program, target, &expr.ty)?;
    let target = emitter
        .functions
        .get(&crate::hir::FunctionExecutionId::Monomorphic(
            target.clone(),
        ))
        .ok_or_else(|| {
            backend_error("function reference target is not indexed for native emission")
        })?;
    Ok(CValue {
        code: target.symbol.clone(),
        ty: expr.ty.clone(),
    })
}

pub(super) fn emit_invoke<O: COutput>(
    emitter: &mut CEmitter<'_, O>,
    expr: &ResolvedExpr,
    callable: &ResolvedExpr,
    args: &[ResolvedExpr],
) -> Result<CValue, Diagnostic> {
    hir::function_value::validate_invocation(expr)?;
    let callable_value = emitter.emit_expr(callable)?;
    emitter.require_type(
        &callable_value.ty,
        &callable.ty,
        "function invocation callable",
    )?;
    let staged = emitter.temporary(&callable.ty)?;
    emitter.line(&format!("{staged} = {};", callable_value.code));
    let ResolvedType::Function { parameters, result } = &callable.ty else {
        unreachable!()
    };
    let mut values = Vec::with_capacity(args.len());
    for (index, argument) in args.iter().enumerate() {
        let value = emitter.emit_expr(argument)?;
        emitter.require_type(
            &value.ty,
            &parameters[index],
            "function invocation argument",
        )?;
        let staged_argument = emitter.temporary(&value.ty)?;
        emitter.line(&format!("{staged_argument} = {};", value.code));
        values.push(CValue {
            code: staged_argument,
            ty: value.ty,
        });
    }
    let temporary = emitter.call_result_temporary(result)?;
    let arguments = values
        .iter()
        .map(|value| value.code.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    emitter.line(&format!(
        "spx_status = {staged}(spx_ctx{}{}, &{temporary});",
        if arguments.is_empty() { "" } else { ", " },
        arguments
    ));
    emitter.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
    emitter.require_type(&expr.ty, result, "function invocation result")?;
    Ok(CValue {
        code: temporary,
        ty: expr.ty.clone(),
    })
}

pub(super) fn resolved_expr_children<'a>(
    expression: &'a ResolvedExpr,
) -> Box<dyn Iterator<Item = &'a ResolvedExpr> + 'a> {
    match &expression.kind {
        ResolvedExprKind::Binary { left, right, .. } => {
            Box::new([left.as_ref(), right.as_ref()].into_iter())
        }
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => Box::new([source.as_ref(), start.as_ref(), end.as_ref()].into_iter()),
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value } => Box::new(std::iter::once(value.as_ref())),
        ResolvedExprKind::Block { statements, tail } => Box::new(
            statements
                .iter()
                .flat_map(|statement| {
                    (0..statement.child_count()).filter_map(move |index| statement.child(index))
                })
                .chain(std::iter::once(tail.as_ref())),
        ),
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => Box::new(
            [
                condition.as_ref(),
                then_branch.as_ref(),
                else_branch.as_ref(),
            ]
            .into_iter(),
        ),
        ResolvedExprKind::Call { args, .. } => Box::new(args.iter()),
        ResolvedExprKind::Invoke { callable, args } => {
            Box::new(std::iter::once(callable.as_ref()).chain(args.iter()))
        }
        ResolvedExprKind::FunctionReference { .. } => Box::new(std::iter::empty()),
        ResolvedExprKind::NativeRustImportCall(call) => Box::new(call.args.iter()),
        ResolvedExprKind::HostCommandCall(call) => Box::new(call.args.iter()),
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => {
            Box::new(fields.iter().map(|field| &field.value))
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => Box::new(
            std::iter::once(scrutinee.as_ref()).chain(
                arms.iter()
                    .filter_map(|arm| arm.guard.as_deref())
                    .chain(arms.iter().map(|arm| &arm.value)),
            ),
        ),
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            Box::new(std::iter::once(base.as_ref()).chain(fields.iter().map(|field| &field.value)))
        }
        ResolvedExprKind::BorrowPlace { .. } => Box::new(std::iter::empty()),
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::String(_)
        | ResolvedExprKind::ArrayU8(_)
        | ResolvedExprKind::RepeatArrayU8 { .. }
        | ResolvedExprKind::Place(_) => Box::new(std::iter::empty()),
    }
}
