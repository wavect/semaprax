//! Generic instantiation for resolved declarations.
//!
//! Substitutes one concrete argument vector into a declaration-owned
//! template so payload validation, layouts, and backends agree.

use std::collections::BTreeMap;

use crate::ast::Type;
use crate::cleanup::CleanupInventory;
use crate::cleanup_plan::CleanupPlan;
use crate::diagnostic::Diagnostic;
use crate::loan_plan::LoanPlan;

use super::expr_nodes::{ResolvedExpr, ResolvedExprKind, ResolvedStatement};
use super::ids::{DeclarationId, ExpressionId, FunctionExecutionId, FunctionInstanceId, ValueId};
use super::nodes::{
    ResolvedBinding, ResolvedFunction, ResolvedFunctionTemplate, ResolvedNativeRustImportCall,
    ResolvedParam, ResolvedType,
};
use super::{hir_error, Place};

/// Substitute one concrete generic instantiation into a declaration-owned
/// type template. Consumers share this helper so payload validation, type
/// facts, layouts, and backends cannot disagree about parameter identity.
pub(crate) fn substitute_type(
    template: &ResolvedType,
    owner: &DeclarationId,
    arguments: &[ResolvedType],
) -> Result<ResolvedType, Diagnostic> {
    enum Frame<'a> {
        Enter(&'a ResolvedType),
        Finish(&'a DeclarationId, usize),
    }
    let mut frames = vec![Frame::Enter(template)];
    let mut resolved = Vec::new();
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Enter(template) => match template {
                ResolvedType::Unit => resolved.push(ResolvedType::Unit),
                ResolvedType::I64 => resolved.push(ResolvedType::I64),
                ResolvedType::I32 => resolved.push(ResolvedType::I32),
                ResolvedType::Char => resolved.push(ResolvedType::Char),
                ResolvedType::U8 => resolved.push(ResolvedType::U8),
                ResolvedType::Usize => resolved.push(ResolvedType::Usize),
                ResolvedType::ArrayU8(length) => resolved.push(ResolvedType::ArrayU8(*length)),
                ResolvedType::F32 => resolved.push(ResolvedType::F32),
                ResolvedType::F64 => resolved.push(ResolvedType::F64),
                ResolvedType::Bool => resolved.push(ResolvedType::Bool),
                ResolvedType::String => resolved.push(ResolvedType::String),
                ResolvedType::Bytes => resolved.push(ResolvedType::Bytes),
                ResolvedType::Str => resolved.push(ResolvedType::Str),
                ResolvedType::SliceU8 => resolved.push(ResolvedType::SliceU8),
                ResolvedType::TypeParameter {
                    owner: parameter_owner,
                    index,
                } => {
                    if parameter_owner != owner {
                        return Err(hir_error(format!(
                            "type template for `{owner}` contains foreign parameter owner `{parameter_owner}`"
                        )));
                    }
                    resolved.push(
                        arguments
                            .get(usize::try_from(*index).map_err(|_| {
                                hir_error(format!("type parameter index {index} does not fit usize"))
                            })?)
                            .cloned()
                            .ok_or_else(|| {
                                hir_error(format!(
                                    "type template for `{owner}` references missing parameter {index}"
                                ))
                            })?,
                    );
                }
                ResolvedType::Nominal {
                    declaration,
                    arguments,
                } => {
                    frames.push(Frame::Finish(declaration, arguments.len()));
                    frames.extend(arguments.iter().rev().map(Frame::Enter));
                }
            },
            Frame::Finish(declaration, count) => {
                let split = resolved
                    .len()
                    .checked_sub(count)
                    .ok_or_else(|| hir_error("type substitution traversal is incomplete"))?;
                let nested = resolved.drain(split..).collect();
                resolved.push(ResolvedType::Nominal {
                    declaration: declaration.clone(),
                    arguments: nested,
                });
            }
        }
    }
    if resolved.len() != 1 {
        return Err(hir_error("type substitution traversal did not settle"));
    }
    Ok(resolved
        .pop()
        .expect("substitution result count checked above"))
}

pub(super) fn substitute_source_function_type(
    function: &crate::ast::Function,
    arguments: &[Type],
    template: &Type,
) -> Option<Type> {
    enum Frame<'a> {
        Enter(&'a Type),
        Finish(&'a str, usize),
    }
    let mut frames = vec![Frame::Enter(template)];
    let mut resolved = Vec::new();
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Enter(template) => match template {
                Type::I64 => resolved.push(Type::I64),
                Type::I32 => resolved.push(Type::I32),
                Type::Char => resolved.push(Type::Char),
                Type::U8 => resolved.push(Type::U8),
                Type::Usize => resolved.push(Type::Usize),
                Type::ArrayU8(length) => resolved.push(Type::ArrayU8(*length)),
                Type::F32 => resolved.push(Type::F32),
                Type::F64 => resolved.push(Type::F64),
                Type::Bool => resolved.push(Type::Bool),
                Type::String => resolved.push(Type::String),
                Type::Bytes => resolved.push(Type::Bytes),
                Type::Str => resolved.push(Type::Str),
                Type::SliceU8 => resolved.push(Type::SliceU8),
                Type::Named {
                    name,
                    arguments: nested,
                } => {
                    if nested.is_empty() {
                        if let Some(index) = function
                            .type_parameters
                            .iter()
                            .position(|parameter| parameter.name == *name)
                        {
                            resolved.push(arguments.get(index)?.clone());
                            continue;
                        }
                    }
                    frames.push(Frame::Finish(name, nested.len()));
                    frames.extend(nested.iter().rev().map(Frame::Enter));
                }
            },
            Frame::Finish(name, count) => {
                let split = resolved.len().checked_sub(count)?;
                let arguments = resolved.drain(split..).collect();
                resolved.push(Type::Named {
                    name: name.to_owned(),
                    arguments,
                });
            }
        }
    }
    (resolved.len() == 1).then(|| resolved.pop().expect("type count checked above"))
}

pub(super) fn specialize_source_function(
    function: &crate::ast::Function,
    arguments: &[Type],
) -> Option<crate::ast::Function> {
    let mut specialized = function.clone();
    specialized.type_parameters.clear();
    for param in &mut specialized.params {
        param.ty = substitute_source_function_type(function, arguments, &param.ty)?;
    }
    specialized.return_type =
        substitute_source_function_type(function, arguments, &function.return_type)?;
    Some(specialized)
}

pub(super) fn materialize_function_template(
    template: &ResolvedFunctionTemplate,
    arguments: &[ResolvedType],
) -> Result<ResolvedFunction, Diagnostic> {
    let instance = FunctionInstanceId::derive(&template.id, arguments);
    let execution = FunctionExecutionId::Generic(instance);
    let mut values = BTreeMap::new();
    let params = template
        .params
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            let id = ValueId::parameter(&execution, index);
            values.insert(parameter.id.clone(), id.clone());
            Ok(ResolvedParam {
                id,
                name: parameter.name.clone(),
                ownership: parameter.ownership,
                ty: substitute_type(&parameter.ty, &template.id, arguments)?,
                span: parameter.span,
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    let result_id = ValueId::result(&execution);
    let return_type = substitute_type(&template.return_type, &template.id, arguments)?;
    let requires = template
        .requires
        .iter()
        .enumerate()
        .map(|(index, expression)| {
            materialize_template_expr(
                template,
                arguments,
                &execution,
                expression,
                &values,
                &format!("requires.{index}"),
            )
        })
        .collect::<Result<_, _>>()?;
    let body = materialize_template_expr(
        template,
        arguments,
        &execution,
        &template.body,
        &values,
        "body",
    )?;
    let mut ensures_values = values;
    ensures_values.insert(template.result_id.clone(), result_id.clone());
    let ensures = template
        .ensures
        .iter()
        .enumerate()
        .map(|(index, expression)| {
            materialize_template_expr(
                template,
                arguments,
                &execution,
                expression,
                &ensures_values,
                &format!("ensures.{index}"),
            )
        })
        .collect::<Result<_, _>>()?;
    Ok(ResolvedFunction {
        id: template.id.clone(),
        name: template.name.clone(),
        params,
        result_id,
        return_type,
        effects: template.effects.clone(),
        requires,
        ensures,
        body,
        cleanup: CleanupInventory::unresolved(),
        cleanup_plan: CleanupPlan::unresolved(),
        loan_plan: LoanPlan::unresolved(),
        span: template.span,
    })
}

pub(super) fn resolved_scalar_substitutions(parameter_count: usize) -> Vec<Vec<ResolvedType>> {
    debug_assert!((1..=2).contains(&parameter_count));
    (0..(1_usize << parameter_count))
        .map(|bits| {
            (0..parameter_count)
                .map(|index| {
                    if bits & (1 << index) == 0 {
                        ResolvedType::I64
                    } else {
                        ResolvedType::Bool
                    }
                })
                .collect()
        })
        .collect()
}

pub(super) fn same_function_meaning(
    expected: &ResolvedFunction,
    actual: &ResolvedFunction,
) -> bool {
    expected.id == actual.id
        && expected.name == actual.name
        && expected.params == actual.params
        && expected.result_id == actual.result_id
        && expected.return_type == actual.return_type
        && expected.effects == actual.effects
        && expected.requires == actual.requires
        && expected.ensures == actual.ensures
        && expected.body == actual.body
        && expected.span == actual.span
}

pub(super) fn materialize_template_expr(
    template: &ResolvedFunctionTemplate,
    arguments: &[ResolvedType],
    execution: &FunctionExecutionId,
    expression: &ResolvedExpr,
    values: &BTreeMap<ValueId, ValueId>,
    path: &str,
) -> Result<ResolvedExpr, Diagnostic> {
    let kind = match &expression.kind {
        ResolvedExprKind::Int(value) => ResolvedExprKind::Int(*value),
        ResolvedExprKind::Int32(value) => ResolvedExprKind::Int32(*value),
        ResolvedExprKind::Char(value) => ResolvedExprKind::Char(*value),
        ResolvedExprKind::Uint8(value) => ResolvedExprKind::Uint8(*value),
        ResolvedExprKind::Usize(value) => ResolvedExprKind::Usize(*value),
        ResolvedExprKind::ArrayU8(_)
        | ResolvedExprKind::RepeatArrayU8 { .. }
        | ResolvedExprKind::ByteRange { .. }
        | ResolvedExprKind::HostCommandCall(_) => {
            return Err(hir_error(
                "generic template uses portable byte data outside the generic slice",
            ));
        }
        // The rooted owned-String view is the one byte view inside the generic
        // slice. It carries no projection, so materialization only remaps the
        // storage root; every other byte operation and every projected place
        // stays outside.
        ResolvedExprKind::BorrowPlace { operation, place } => {
            if crate::byte_ops::by_id(operation.as_str())
                != Some(crate::byte_ops::ByteOp::StringAsStr)
                || !place.projections.is_empty()
            {
                return Err(hir_error(
                    "generic template uses portable byte data outside the generic slice",
                ));
            }
            ResolvedExprKind::BorrowPlace {
                operation: operation.clone(),
                place: Place {
                    root: values
                        .get(&place.root)
                        .cloned()
                        .ok_or_else(|| hir_error("generic template place is out of scope"))?,
                    projections: Vec::new(),
                },
            }
        }
        ResolvedExprKind::Float32(bits) => ResolvedExprKind::Float32(*bits),
        ResolvedExprKind::Float64(bits) => ResolvedExprKind::Float64(*bits),
        ResolvedExprKind::Bool(value) => ResolvedExprKind::Bool(*value),
        ResolvedExprKind::String(value) => ResolvedExprKind::String(value.clone()),
        ResolvedExprKind::Place(place) => ResolvedExprKind::Place(Place {
            root: values
                .get(&place.root)
                .cloned()
                .ok_or_else(|| hir_error("generic template place is out of scope"))?,
            projections: place.projections.clone(),
        }),
        ResolvedExprKind::Call {
            callee,
            type_arguments,
            instance,
            args,
        } => {
            if instance.is_some() || !type_arguments.is_empty() {
                return Err(hir_error(
                    "generic templates cannot call generic function instances",
                ));
            }
            ResolvedExprKind::Call {
                callee: callee.clone(),
                type_arguments: Vec::new(),
                instance: None,
                args: args
                    .iter()
                    .enumerate()
                    .map(|(index, argument)| {
                        materialize_template_expr(
                            template,
                            arguments,
                            execution,
                            argument,
                            values,
                            &format!("{path}.arg.{index}"),
                        )
                    })
                    .collect::<Result<_, _>>()?,
            }
        }
        ResolvedExprKind::NativeRustImportCall(call) => {
            ResolvedExprKind::NativeRustImportCall(ResolvedNativeRustImportCall {
                expression: ExpressionId::new(execution, path),
                import: call.import.clone(),
                args: call
                    .args
                    .iter()
                    .enumerate()
                    .map(|(index, argument)| {
                        materialize_template_expr(
                            template,
                            arguments,
                            execution,
                            argument,
                            values,
                            &format!("{path}.native-rust-arg.{index}"),
                        )
                    })
                    .collect::<Result<_, _>>()?,
                result: call.result.clone(),
            })
        }
        ResolvedExprKind::Unary { op, value } => ResolvedExprKind::Unary {
            op: *op,
            value: Box::new(materialize_template_expr(
                template,
                arguments,
                execution,
                value,
                values,
                &format!("{path}.value"),
            )?),
        },
        ResolvedExprKind::Binary { op, left, right } => ResolvedExprKind::Binary {
            op: *op,
            left: Box::new(materialize_template_expr(
                template,
                arguments,
                execution,
                left,
                values,
                &format!("{path}.left"),
            )?),
            right: Box::new(materialize_template_expr(
                template,
                arguments,
                execution,
                right,
                values,
                &format!("{path}.right"),
            )?),
        },
        ResolvedExprKind::Block { statements, tail } => {
            let mut block_values = values.clone();
            let mut materialized = Vec::with_capacity(statements.len());
            for (index, statement) in statements.iter().enumerate() {
                let statement_path = format!("{path}.s{index}");
                match statement {
                    ResolvedStatement::Let {
                        binding,
                        mutable,
                        value,
                        span,
                    } => {
                        let value = materialize_template_expr(
                            template,
                            arguments,
                            execution,
                            value,
                            &block_values,
                            &format!("{statement_path}.value"),
                        )?;
                        let id = ValueId::local(execution, &statement_path);
                        block_values.insert(binding.id.clone(), id.clone());
                        materialized.push(ResolvedStatement::Let {
                            binding: ResolvedBinding {
                                id,
                                name: binding.name.clone(),
                                ownership: binding.ownership,
                                ty: substitute_type(&binding.ty, &template.id, arguments)?,
                                span: binding.span,
                            },
                            mutable: *mutable,
                            value,
                            span: *span,
                        });
                    }
                    ResolvedStatement::Assign { .. } => {
                        return Err(hir_error(
                            "generic template statements cannot assign to local bindings",
                        ));
                    }
                    ResolvedStatement::While { .. } => {
                        return Err(hir_error("generic templates cannot contain while loops"));
                    }
                    ResolvedStatement::Unsafe { audit, body, span } => {
                        let body = materialize_template_expr(
                            template,
                            arguments,
                            execution,
                            body,
                            &block_values,
                            &format!("{statement_path}.body"),
                        )?;
                        materialized.push(ResolvedStatement::Unsafe {
                            audit: audit.clone(),
                            body: Box::new(body),
                            span: *span,
                        });
                    }
                }
            }
            ResolvedExprKind::Block {
                statements: materialized,
                tail: Box::new(materialize_template_expr(
                    template,
                    arguments,
                    execution,
                    tail,
                    &block_values,
                    &format!("{path}.tail"),
                )?),
            }
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => ResolvedExprKind::If {
            condition: Box::new(materialize_template_expr(
                template,
                arguments,
                execution,
                condition,
                values,
                &format!("{path}.condition"),
            )?),
            then_branch: Box::new(materialize_template_expr(
                template,
                arguments,
                execution,
                then_branch,
                values,
                &format!("{path}.then"),
            )?),
            else_branch: Box::new(materialize_template_expr(
                template,
                arguments,
                execution,
                else_branch,
                values,
                &format!("{path}.else"),
            )?),
        },
        ResolvedExprKind::ConstructRecord { .. }
        | ResolvedExprKind::ConstructVariant { .. }
        | ResolvedExprKind::Match { .. }
        | ResolvedExprKind::Try { .. }
        | ResolvedExprKind::TryOption { .. }
        | ResolvedExprKind::UpdateRecord { .. }
        | ResolvedExprKind::Project { .. }
        | ResolvedExprKind::Upcast { .. } => {
            return Err(hir_error(
                "generic template uses an expression outside the direct-scalar slice",
            ));
        }
    };
    Ok(ResolvedExpr {
        id: ExpressionId::new(execution, path),
        ty: substitute_type(&expression.ty, &template.id, arguments)?,
        ownership: expression.ownership,
        kind,
        span: expression.span,
    })
}
