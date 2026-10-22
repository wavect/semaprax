//! Literal constructor inventory for recursively admitted closure bodies.
//! Shared identities allocate no backing. The full legacy fallback preserves
//! behavior if a future shape profile admits another kind; it is unpaid residual.
use super::*;
fn boxed(value: &ResolvedExpr) -> Result<Box<ResolvedExpr>, Diagnostic> {
    reserve(Some(std::mem::size_of::<ResolvedExpr>()))?;
    Ok(Box::new(expression(value)?))
}
fn binding(value: &ResolvedBinding) -> Result<ResolvedBinding, Diagnostic> {
    Ok(ResolvedBinding {
        id: value.id.clone(),
        name: text(format_args!("{}", value.name))?,
        ownership: value.ownership,
        ty: ty(&value.ty)?,
        span: value.span,
    })
}
fn statements(values: &[ResolvedStatement]) -> Result<Vec<ResolvedStatement>, Diagnostic> {
    let mut output = vector(values.len())?;
    for value in values {
        output.push(match value {
            ResolvedStatement::Let {
                binding: target,
                mutable,
                value,
                span,
            } => ResolvedStatement::Let {
                binding: binding(target)?,
                mutable: *mutable,
                value: expression(value)?,
                span: *span,
            },
            ResolvedStatement::Assign {
                binding: target,
                field,
                value,
                span,
            } => ResolvedStatement::Assign {
                binding: binding(target)?,
                field: field.clone(),
                value: expression(value)?,
                span: *span,
            },
            ResolvedStatement::While {
                condition,
                body,
                span,
            } => ResolvedStatement::While {
                condition: boxed(condition)?,
                body: boxed(body)?,
                span: *span,
            },
            // Full legacy statement, rather than a new admission refusal.
            other => other.clone(),
        });
    }
    Ok(output)
}
pub(super) fn expression(value: &ResolvedExpr) -> Result<ResolvedExpr, Diagnostic> {
    let ty = ty(&value.ty)?;
    let kind = match &value.kind {
        ResolvedExprKind::Place(place) => {
            reserve(
                place
                    .projections
                    .len()
                    .checked_mul(std::mem::size_of::<PlaceProjection>()),
            )?;
            ResolvedExprKind::Place(place.clone())
        }
        ResolvedExprKind::Unary { op, value } => ResolvedExprKind::Unary {
            op: *op,
            value: boxed(value)?,
        },
        ResolvedExprKind::Binary { op, left, right } => ResolvedExprKind::Binary {
            op: *op,
            left: boxed(left)?,
            right: boxed(right)?,
        },
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => ResolvedExprKind::If {
            condition: boxed(condition)?,
            then_branch: boxed(then_branch)?,
            else_branch: boxed(else_branch)?,
        },
        ResolvedExprKind::Call {
            callee,
            type_arguments,
            instance,
            args,
        } => {
            let mut types = vector(type_arguments.len())?;
            for argument in type_arguments {
                types.push(super::ty(argument)?);
            }
            reserve(instance.clone_payload_bytes())?;
            let instance = instance.clone();
            let mut arguments = vector(args.len())?;
            for argument in args {
                arguments.push(expression(argument)?);
            }
            ResolvedExprKind::Call {
                callee: callee.clone(),
                type_arguments: types,
                instance,
                args: arguments,
            }
        }
        ResolvedExprKind::Block {
            statements: values,
            tail,
        } => ResolvedExprKind::Block {
            statements: statements(values)?,
            tail: boxed(tail)?,
        },
        ResolvedExprKind::VecConstructor {
            element,
            count,
            statements: values,
            tail,
        } => ResolvedExprKind::VecConstructor {
            element: super::ty(element)?,
            count: *count,
            statements: statements(values)?,
            tail: boxed(tail)?,
        },
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_) => value.kind.clone(),
        // validate_shape recursively excludes all other current variants. Keep
        // an independent complete fallback if that admitted domain ever grows.
        other => other.clone(),
    };
    Ok(ResolvedExpr {
        id: value.id.clone(),
        ty,
        ownership: value.ownership,
        kind,
        span: value.span,
    })
}
