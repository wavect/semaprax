//! Argument ownership checking at call boundaries, including the
//! invocation-local loans that borrowed byte-data arguments activate.

use super::binding::{Binding, CheckedValue, SourceLoan, SourceLoanId};
use super::diagnostics::error;
use super::loans::{has_active_overlapping_loan, mark_value_sources_moved};
use super::place::source_place;
use super::type_table::TypeTable;
use crate::ast::{Expr, ExprKind, Function, Param, ParamMode, Program, Type};
use crate::diagnostic::Diagnostic;
use std::collections::HashMap;

#[allow(clippy::too_many_arguments)]
pub(super) fn check_argument_ownership(
    program: &Program,
    current: &Function,
    callee: &str,
    arg: &Expr,
    param: &crate::ast::Param,
    actual: Option<&CheckedValue>,
    variables: &mut HashMap<String, Binding>,
    types: &TypeTable<'_>,
    allow_moves: bool,
    implicit_unique_ownership: bool,
    allow_monomorphic_borrowed_bytes_call: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(actual) = actual else {
        return;
    };
    if !types.needs_drop(&actual.ty) {
        return;
    }
    // HIR normalizes every uniquely-owned by-value parameter to an ownership
    // transfer. Replay that normalization at the source boundary so callers
    // cannot retain a string that the resolved callee has consumed.
    let mode = if implicit_unique_ownership
        && param.mode == ParamMode::Value
        && param.ty.is_uniquely_owned()
    {
        ParamMode::Own
    } else {
        param.mode
    };
    match mode {
        ParamMode::Own => {
            if let Some(place) = source_place(arg, variables, types) {
                if variables
                    .get(&place.root)
                    .is_some_and(|binding| has_active_overlapping_loan(binding, &place.projections))
                {
                    diagnostics.push(error(
                        program,
                        "SPX-T265",
                        "move or call transfer would invalidate a lexical byte view",
                        arg.span,
                    ));
                    return;
                }
            }
            if actual.mode != ParamMode::Own {
                if matches!(actual.mode, ParamMode::Borrow | ParamMode::Shared)
                    && source_place(arg, variables, types)
                        .is_some_and(|place| !place.projections.is_empty())
                {
                    diagnostics.push(error(
                        program,
                        "SPX-O108",
                        "cannot move an owned field through a borrowed or shared record",
                        arg.span,
                    ));
                    return;
                }
                diagnostics.push(
                    error(
                        program,
                        "SPX-O102",
                        format!(
                            "argument to `{}.{}` is {}, so `{current_name}` cannot transfer it to `{callee}`",
                            current.name,
                            param.name,
                            actual.mode.text(),
                            current_name = current.name
                        ),
                        arg.span,
                    )
                    .with_help(format!(
                        "provide an owned `{}` value at this ownership boundary",
                        actual.ty
                    )),
                );
            } else if allow_moves {
                mark_value_sources_moved(program, arg, variables, types, diagnostics);
            } else {
                diagnostics.push(error(
                    program,
                    "SPX-O105",
                    format!("contract expression cannot transfer a resource into `{callee}`"),
                    arg.span,
                ));
            }
        }
        ParamMode::Shared if actual.mode != ParamMode::Shared => diagnostics.push(
            error(
                program,
                "SPX-O103",
                format!("`{callee}` requires shared resource ownership"),
                arg.span,
            )
            .with_help("create or receive an explicitly shared resource before this call"),
        ),
        ParamMode::Borrow
            if types.is_flat_owned_byte_record(&param.ty)
                || types.is_flat_owned_byte_variant(&param.ty) =>
        {
            if !matches!(actual.mode, ParamMode::Own | ParamMode::Borrow)
                || !source_place(arg, variables, types)
                    .is_some_and(|place| place.projections.is_empty())
            {
                diagnostics.push(
                    error(
                        program,
                        "SPX-O118",
                        "borrowed owned-Bytes aggregate argument must be an unprojected named place",
                        arg.span,
                    )
                    .with_help("bind the owned aggregate to a local before borrowing it"),
                );
            }
        }
        ParamMode::Borrow if param.ty == Type::Bytes => {
            let exact_byte_view = crate::byte_ops::by_name(callee).is_some_and(|operation| {
                operation == crate::byte_ops::ByteOp::BytesAsSlice
                    && source_byte_view_place_is_admitted(operation, arg, variables, types)
            });
            if !exact_byte_view
                && !source_borrowed_bytes_call_place_is_admitted(
                    arg,
                    variables,
                    types,
                    allow_monomorphic_borrowed_bytes_call,
                )
            {
                diagnostics.push(
                    error(
                        program,
                        "SPX-T266",
                        "borrowed Bytes call requires an exact admitted storage place",
                        arg.span,
                    )
                    .with_help(
                        "use an owned Bytes local or one direct Bytes field of a flat owned record",
                    ),
                );
            }
        }
        ParamMode::Borrow if types.contains_owned_bytes(&param.ty) => diagnostics.push(
            error(
                program,
                "SPX-O118",
                "borrowed owned-Bytes aggregate is outside the closed flat profile",
                arg.span,
            )
            .with_help("borrow an exact named flat owned-Bytes aggregate place"),
        ),
        ParamMode::Borrow | ParamMode::Shared | ParamMode::Value => {}
    }
}

pub(super) fn source_borrowed_bytes_call_place_is_admitted(
    expression: &Expr,
    variables: &HashMap<String, Binding>,
    types: &TypeTable<'_>,
    allow_monomorphic_call: bool,
) -> bool {
    if !allow_monomorphic_call {
        return false;
    }
    let Some(place) = source_place(expression, variables, types) else {
        return false;
    };
    if place.ty != Type::Bytes {
        return false;
    }
    if place.projections.is_empty() {
        return matches!(expression.kind, ExprKind::Var(_))
            && matches!(place.mode, ParamMode::Own | ParamMode::Borrow);
    }
    place.projections.len() == 1
        && place.mode == ParamMode::Own
        && variables.get(&place.root).is_some_and(|binding| {
            binding.mode == ParamMode::Own && types.is_flat_owned_byte_record(&binding.ty)
        })
}

pub(super) fn activate_borrowed_bytes_call_loans(
    arguments: &[Expr],
    parameters: &[Param],
    variables: &mut HashMap<String, Binding>,
    types: &TypeTable<'_>,
) -> Vec<(String, SourceLoanId)> {
    let mut active = Vec::new();
    for (index, (borrowed, parameter)) in arguments.iter().zip(parameters).enumerate() {
        if parameter.mode != ParamMode::Borrow
            || parameter.ty != Type::Bytes
            || !source_borrowed_bytes_call_place_is_admitted(borrowed, variables, types, true)
        {
            continue;
        }
        let Some(origin) = source_place(borrowed, variables, types) else {
            continue;
        };
        let id = SourceLoanId {
            borrower: format!("borrowed call argument {index}"),
            start: borrowed.span.start,
            end: borrowed.span.end,
        };
        if let Some(owner) = variables.get_mut(&origin.root) {
            owner.active_loans.insert(SourceLoan {
                id: id.clone(),
                projections: origin.projections,
            });
            active.push((origin.root, id));
        }
    }
    active
}

pub(super) fn release_borrowed_bytes_call_loans(
    variables: &mut HashMap<String, Binding>,
    active: &[(String, SourceLoanId)],
) {
    for (root, loan) in active {
        if let Some(owner) = variables.get_mut(root) {
            owner.active_loans.retain(|candidate| candidate.id != *loan);
        }
    }
}

pub(super) fn source_byte_view_place_is_admitted(
    operation: crate::byte_ops::ByteOp,
    expression: &Expr,
    variables: &HashMap<String, Binding>,
    types: &TypeTable<'_>,
) -> bool {
    let Some(place) = source_place(expression, variables, types) else {
        return false;
    };
    if place.projections.is_empty() {
        return matches!(expression.kind, ExprKind::Var(_));
    }
    operation == crate::byte_ops::ByteOp::BytesAsSlice
        && place.projections.len() == 1
        && place.ty == Type::Bytes
        && variables.get(&place.root).is_some_and(|binding| {
            binding.mode == ParamMode::Own && types.is_flat_owned_byte_record(&binding.ty)
        })
}
