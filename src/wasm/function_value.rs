//! Local allocation for Function Value v1 and ordinary core expressions.
use std::collections::HashMap;

use crate::diagnostic::Diagnostic;
use crate::hir::{ResolvedExpr, ResolvedExprKind, ResolvedStatement, ResolvedType};

use super::{LocalLayout, Signature};

pub(super) fn collect_locals(
    expr: &ResolvedExpr,
    parameter_count: u32,
    layout: &mut LocalLayout,
) -> Result<(), Diagnostic> {
    match &expr.kind {
        ResolvedExprKind::Invoke { callable, args } => {
            collect_locals(callable, parameter_count, layout)?;
            for arg in args {
                collect_locals(arg, parameter_count, layout)?;
            }
            let index = parameter_count + layout.declarations.len() as u32;
            layout.declarations.push(callable.ty.clone());
            if layout
                .function_scratch
                .insert(expr.id.as_str().to_owned(), index)
                .is_some()
            {
                return Err(Diagnostic::io(
                    "SPX-W108",
                    "duplicate WebAssembly function invocation scratch",
                ));
            }
        }
        ResolvedExprKind::Call { args, .. } => {
            for arg in args {
                collect_locals(arg, parameter_count, layout)?;
            }
        }
        ResolvedExprKind::NativeRustImportCall(call) => {
            for arg in &call.args {
                collect_locals(arg, parameter_count, layout)?;
            }
        }
        ResolvedExprKind::HostCommandCall(call) => {
            for arg in &call.args {
                collect_locals(arg, parameter_count, layout)?;
            }
        }
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => {
            collect_locals(source, parameter_count, layout)?;
            collect_locals(start, parameter_count, layout)?;
            collect_locals(end, parameter_count, layout)?;
        }
        ResolvedExprKind::Unary { value, .. } => {
            collect_locals(value, parameter_count, layout)?;
        }
        ResolvedExprKind::Try { operand, .. } | ResolvedExprKind::TryOption { operand, .. } => {
            collect_locals(operand, parameter_count, layout)?;
        }
        ResolvedExprKind::Binary { left, right, .. } => {
            collect_locals(left, parameter_count, layout)?;
            collect_locals(right, parameter_count, layout)?;
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                match statement {
                    ResolvedStatement::Let { binding, value, .. } => {
                        collect_locals(value, parameter_count, layout)?;
                        let index = parameter_count + layout.declarations.len() as u32;
                        layout.declarations.push(binding.ty.clone());
                        if layout.lets.insert(binding.id.clone(), index).is_some() {
                            return Err(Diagnostic::io(
                                "SPX-W108",
                                format!("duplicate WebAssembly local identity `{}`", binding.id),
                            ));
                        }
                    }
                    // An assignment target reuses its `let` local and an
                    // unsafe boundary adds none; only their values contribute
                    // to the local walk.
                    ResolvedStatement::Assign { value, .. } => {
                        collect_locals(value, parameter_count, layout)?;
                    }
                    ResolvedStatement::Unsafe { body, .. } => {
                        collect_locals(body, parameter_count, layout)?;
                    }
                    ResolvedStatement::While {
                        condition, body, ..
                    } => {
                        collect_locals(condition, parameter_count, layout)?;
                        collect_locals(body, parameter_count, layout)?;
                    }
                }
            }
            collect_locals(tail, parameter_count, layout)?;
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_locals(condition, parameter_count, layout)?;
            collect_locals(then_branch, parameter_count, layout)?;
            collect_locals(else_branch, parameter_count, layout)?;
        }
        ResolvedExprKind::ConstructRecord { fields, .. } => {
            for field in fields {
                collect_locals(&field.value, parameter_count, layout)?;
            }
        }
        ResolvedExprKind::ConstructVariant { fields, .. } => {
            for field in fields {
                collect_locals(&field.value, parameter_count, layout)?;
            }
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            collect_locals(scrutinee, parameter_count, layout)?;
            if matches!(
                scrutinee.ty,
                ResolvedType::I64
                    | ResolvedType::I32
                    | ResolvedType::U8
                    | ResolvedType::Char
                    | ResolvedType::Bool
            ) {
                // Refutable Match v1: stage the scrutinee once in its own
                // dedicated local so every arm test re-reads exactly one
                // evaluation.
                let index = parameter_count + layout.declarations.len() as u32;
                layout.declarations.push(scrutinee.ty.clone());
                if layout
                    .match_scratch
                    .insert(expr.id.as_str().to_owned(), index)
                    .is_some()
                {
                    return Err(Diagnostic::io(
                        "SPX-W108",
                        format!(
                            "duplicate WebAssembly local identity for match `{}`",
                            expr.id
                        ),
                    ));
                }
                // Binding arms alias the staged scrutinee local: reading the
                // binding reads exactly one evaluation of the scrutinee.
                for arm in arms {
                    if let crate::hir::ResolvedMatchPattern::Binding(binding) = &arm.pattern {
                        layout.lets.insert(binding.id.clone(), index);
                    }
                }
            }
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_locals(guard.as_ref(), parameter_count, layout)?;
                }
                collect_locals(&arm.value, parameter_count, layout)?;
            }
        }
        ResolvedExprKind::Project { base, .. } => {
            collect_locals(base, parameter_count, layout)?;
        }
        ResolvedExprKind::Upcast { source } => {
            collect_locals(source, parameter_count, layout)?;
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            collect_locals(base, parameter_count, layout)?;
            for field in fields {
                collect_locals(&field.value, parameter_count, layout)?;
            }
        }
        ResolvedExprKind::FunctionReference { .. } => {}
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::ArrayU8(_)
        | ResolvedExprKind::RepeatArrayU8 { .. }
        | ResolvedExprKind::String(_)
        | ResolvedExprKind::Place(_)
        | ResolvedExprKind::BorrowPlace { .. } => {}
    }
    Ok(())
}

/// The table entries and every indirect-call signature that core lowering
/// needs. Invocation signatures are collected independently of references:
/// a private helper can receive and invoke a callback even if the current
/// module has no concrete function-reference expression.
pub(super) fn table_plan(
    program: &crate::hir::ResolvedProgram,
) -> (Vec<&crate::hir::ResolvedFunction>, Vec<ResolvedType>) {
    use std::collections::BTreeMap;

    let mut signatures = BTreeMap::new();
    for function in &program.functions {
        crate::hir::function_value::walk(function, |expression| {
            if let ResolvedExprKind::Invoke { callable, .. } = &expression.kind {
                signatures.insert(callable.ty.identity_key(), callable.ty.clone());
            }
        });
    }
    (
        crate::hir::function_value::target_universe(program),
        signatures.into_values().collect(),
    )
}

pub(super) fn intern_type(
    signature: Signature,
    types: &mut Vec<Signature>,
    indexes: &mut HashMap<Signature, u32>,
) -> u32 {
    if let Some(index) = indexes.get(&signature) {
        return *index;
    }
    let index = types.len() as u32;
    types.push(signature.clone());
    indexes.insert(signature, index);
    index
}
