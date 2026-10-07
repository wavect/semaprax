//! String Collections v1 position admission for `Map<string, i64>` values.
//!
//! A map is created by `map_new` as a `let` initializer, threaded through the
//! same-owner reopens `m = map_add(m, …)` / `m = map_set(m, …)`, and read only
//! as the first operand of a map operation. Every other position (a second
//! binding, a branch or match result, a block tail, a user-function argument,
//! a closure capture, a field) is refused here with one stable diagnostic
//! before cleanup planning, so no backend ever meets a map outside this
//! closed profile.

use super::{by_id, is_same_owner_concat_hir, StringOp};
use crate::ast::{Span, Type};
use crate::hir::{
    ResolvedExpr, ResolvedExprKind, ResolvedFunction, ResolvedStatement, ResolvedType,
};

/// The stable diagnostic code for a map outside its admitted positions.
pub(crate) const MAP_POSITION_CODE: &str = "SPX-T275";

/// The help line every position refusal carries.
pub(crate) const MAP_POSITION_HELP: &str = "create a map with `let mut counts = map_new(64usize);`, update it with `counts = map_add(counts, key, 1);`, and pass the binding only as the first argument of a `map_*` operation";

/// One refused position: the message and the offending expression's span.
pub(crate) struct MapPositionRefusal {
    pub(crate) message: String,
    pub(crate) span: Span,
}

#[derive(Clone, Copy, PartialEq)]
enum Context {
    LetValue,
    Reopen,
    MapOperand,
    Other,
}

/// Check every `Map<string, i64>` value of one resolved function.
pub(crate) fn check_function(function: &ResolvedFunction) -> Result<(), MapPositionRefusal> {
    if function.return_type == ResolvedType::StringMap
        || function
            .params
            .iter()
            .any(|parameter| parameter.ty == ResolvedType::StringMap)
    {
        return Err(MapPositionRefusal {
            message: "`Map<string, i64>` is not admitted in a function signature; String Collections v1 maps are local bindings".to_owned(),
            span: function.span,
        });
    }
    let mut pending = vec![(&function.body, Context::Other)];
    pending.extend(
        function
            .requires
            .iter()
            .chain(&function.ensures)
            .map(|contract| (contract, Context::Other)),
    );
    while let Some((expression, context)) = pending.pop() {
        if expression.ty == ResolvedType::StringMap {
            check_value(expression, context)?;
        }
        match &expression.kind {
            ResolvedExprKind::Block { statements, tail } => {
                pending.push((tail, Context::Other));
                for statement in statements.iter().rev() {
                    match statement {
                        ResolvedStatement::Let { value, .. } => {
                            pending.push((value, Context::LetValue));
                        }
                        ResolvedStatement::Assign {
                            binding,
                            field: None,
                            value,
                            ..
                        } if is_same_owner_concat_hir(value, &binding.id) => {
                            pending.push((value, Context::Reopen));
                        }
                        _ => {
                            for index in (0..statement.child_count()).rev() {
                                if let Some(child) = statement.child(index) {
                                    pending.push((child, Context::Other));
                                }
                            }
                        }
                    }
                }
            }
            ResolvedExprKind::Call { callee, args, .. }
                if by_id(callee.as_str()).is_some_and(is_map_operation) =>
            {
                for (index, argument) in args.iter().enumerate().rev() {
                    pending.push((
                        argument,
                        if index == 0 {
                            Context::MapOperand
                        } else {
                            Context::Other
                        },
                    ));
                }
            }
            _ => {
                let mut children = Vec::new();
                crate::hir::push_resolved_expression_children_in_authored_order(
                    expression,
                    &mut children,
                );
                pending.extend(children.into_iter().map(|child| (child, Context::Other)));
            }
        }
    }
    Ok(())
}

fn is_map_operation(op: StringOp) -> bool {
    op.param_types().first() == Some(&ResolvedType::StringMap)
}

fn check_value(expression: &ResolvedExpr, context: Context) -> Result<(), MapPositionRefusal> {
    let refuse = |message: &str| {
        Err(MapPositionRefusal {
            message: message.to_owned(),
            span: expression.span,
        })
    };
    match (&expression.kind, context) {
        (ResolvedExprKind::Call { callee, .. }, _) => match by_id(callee.as_str()) {
            Some(StringOp::MapNew) if context == Context::LetValue => Ok(()),
            Some(StringOp::MapNew) => refuse(
                "`map_new` must initialize a `let` binding; String Collections v1 maps are local bindings",
            ),
            Some(op) if op.reopens_map() && context == Context::Reopen => Ok(()),
            Some(op) if op.reopens_map() => refuse(&format!(
                "`{}` is admitted only as the same-owner update `counts = {}(counts, key, value);`",
                op.name(),
                op.name()
            )),
            _ => refuse("this call cannot produce a `Map<string, i64>`"),
        },
        (ResolvedExprKind::Place(place), Context::MapOperand) if place.projections.is_empty() => {
            Ok(())
        }
        (ResolvedExprKind::Place(_), _) => refuse(
            "a `Map<string, i64>` binding can only be passed as the first argument of a `map_*` operation; it cannot be copied, moved, captured, or returned",
        ),
        _ => refuse(
            "a `Map<string, i64>` value must be a `map_new` binding; it cannot come from a branch, match, or block",
        ),
    }
}

/// Whether a source type mentions `Map<string, i64>` anywhere.
pub(crate) fn type_mentions_map(ty: &Type) -> bool {
    let mut pending = vec![ty];
    while let Some(ty) = pending.pop() {
        match ty {
            Type::StringMap => return true,
            Type::Named { arguments, .. } => pending.extend(arguments),
            Type::Function { parameters, result } => {
                pending.extend(parameters);
                pending.push(result);
            }
            _ => {}
        }
    }
    false
}
