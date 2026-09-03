//! Invocation-local loan bookkeeping: borrow origins, loan activation and
//! release, move marking, and the conditional joins over binding state.

use super::arguments::source_byte_view_place_is_admitted;
use super::binding::{Availability, Binding, BorrowOrigin, SourceLoan, SourceLoanId};
use super::diagnostics::error;
use super::place::{join_definitely_partial, join_moved_places, source_place, SourcePlace};
use super::type_table::TypeTable;
use crate::ast::{Expr, ExprKind, ParamMode, Program, Span, Statement, Type};
use crate::diagnostic::Diagnostic;
use std::collections::HashMap;

pub(super) fn local_borrow_origin(
    expression: &Expr,
    borrower: &str,
    borrower_span: Span,
    variables: &HashMap<String, Binding>,
    types: &TypeTable<'_>,
) -> Option<BorrowOrigin> {
    let (root, projections, parent_loan) = match &expression.kind {
        ExprKind::Var(source) if matches!(variables.get(source)?.ty, Type::SliceU8 | Type::Str) => {
            let parent = variables.get(source)?.borrow_origin.clone();
            parent.map_or_else(
                || Some((source.clone(), Vec::new(), None)),
                |origin| Some((origin.root, origin.projections, Some(origin.loan))),
            )?
        }
        ExprKind::Call { name, args, .. } => {
            let operation = crate::byte_ops::by_name(name)?;
            if !operation.is_view() && operation != crate::byte_ops::ByteOp::Range {
                return None;
            }
            let source = args.first()?;
            if !source_byte_view_place_is_admitted(operation, source, variables, types)
                && operation != crate::byte_ops::ByteOp::Range
            {
                return None;
            }
            if operation == crate::byte_ops::ByteOp::Range {
                let ExprKind::Var(source) = &source.kind else {
                    return None;
                };
                let origin = variables.get(source)?.borrow_origin.clone()?;
                (origin.root, origin.projections, Some(origin.loan))
            } else {
                let place = source_place(source, variables, types)?;
                if place.projections.is_empty() {
                    let parent = variables.get(&place.root)?.borrow_origin.clone();
                    parent.map_or_else(
                        || (place.root, Vec::new(), None),
                        |origin| (origin.root, origin.projections, Some(origin.loan)),
                    )
                } else {
                    (place.root, place.projections, None)
                }
            }
        }
        _ => return None,
    };
    Some(BorrowOrigin {
        root,
        projections,
        loan: SourceLoanId {
            borrower: borrower.to_owned(),
            start: borrower_span.start,
            end: borrower_span.end,
        },
        parent: parent_loan,
    })
}

pub(super) fn activate_local_loan(variables: &mut HashMap<String, Binding>, origin: &BorrowOrigin) {
    if let Some(owner) = variables.get_mut(&origin.root) {
        owner.active_loans.insert(SourceLoan {
            id: origin.loan.clone(),
            projections: origin.projections.clone(),
        });
    }
}

pub(super) fn activate_match_loan(
    variables: &mut HashMap<String, Binding>,
    place: &SourcePlace,
    span: Span,
) {
    if let Some(owner) = variables.get_mut(&place.root) {
        owner.active_loans.insert(SourceLoan {
            id: SourceLoanId {
                borrower: "match borrow".to_owned(),
                start: span.start,
                end: span.end,
            },
            projections: place.projections.clone(),
        });
    }
}

pub(super) fn has_active_overlapping_loan(binding: &Binding, projections: &[String]) -> bool {
    binding.active_loans.iter().any(|loan| {
        loan.projections.starts_with(projections) || projections.starts_with(&loan.projections)
    })
}

pub(super) fn expression_uses_name(expression: &Expr, name: &str) -> bool {
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        match &expression.kind {
            ExprKind::Var(candidate) if candidate == name => return true,
            ExprKind::Call { args, .. } | ExprKind::SuperMethod { args, .. } => {
                pending.extend(args)
            }
            ExprKind::MethodCall { receiver, args, .. } => {
                pending.push(receiver);
                pending.extend(args);
            }
            ExprKind::Unary { value, .. }
            | ExprKind::Try { operand: value }
            | ExprKind::Project { base: value, .. } => pending.push(value),
            ExprKind::Binary { left, right, .. } => {
                pending.push(left);
                pending.push(right);
            }
            ExprKind::Block { statements, tail } => {
                pending.push(tail);
                for statement in statements {
                    match statement {
                        Statement::Let { value, .. } | Statement::Assign { value, .. } => {
                            pending.push(value)
                        }
                        Statement::Unsafe { body, .. } => pending.push(body),
                        Statement::While {
                            condition, body, ..
                        } => {
                            pending.push(condition);
                            pending.push(body);
                        }
                    }
                }
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                pending.push(condition);
                pending.push(then_branch);
                pending.push(else_branch);
            }
            ExprKind::ConstructRecord { fields, .. }
            | ExprKind::ConstructVariant { fields, .. } => {
                pending.extend(fields.iter().map(|field| &field.value));
            }
            ExprKind::Match {
                scrutinee, arms, ..
            } => {
                pending.push(scrutinee);
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        pending.push(guard);
                    }
                    pending.push(&arm.value);
                }
            }
            ExprKind::UpdateRecord { base, fields } => {
                pending.push(base);
                pending.extend(fields.iter().map(|field| &field.value));
            }
            ExprKind::Int(_)
            | ExprKind::Int32(_)
            | ExprKind::Char(_)
            | ExprKind::Uint8(_)
            | ExprKind::Usize(_)
            | ExprKind::ArrayU8(_)
            | ExprKind::RepeatArrayU8 { .. }
            | ExprKind::Float32(_)
            | ExprKind::Float64(_)
            | ExprKind::Bool(_)
            | ExprKind::String(_)
            | ExprKind::Var(_) => {}
        }
    }
    false
}

pub(super) fn release_dead_local_loans(
    variables: &mut HashMap<String, Binding>,
    remaining: &[Statement],
    tail: &Expr,
) {
    let dead = variables
        .iter()
        .filter_map(|(name, binding)| {
            let origin = binding.borrow_origin.as_ref()?;
            let used = remaining.iter().any(|statement| match statement {
                Statement::Let { value, .. } | Statement::Assign { value, .. } => {
                    expression_uses_name(value, name)
                }
                Statement::Unsafe { body, .. } => expression_uses_name(body, name),
                Statement::While {
                    condition, body, ..
                } => expression_uses_name(condition, name) || expression_uses_name(body, name),
            }) || expression_uses_name(tail, name);
            (!used).then_some((origin.root.clone(), origin.loan.clone()))
        })
        .collect::<Vec<_>>();
    for (root, loan) in dead {
        if let Some(owner) = variables.get_mut(&root) {
            owner.active_loans.retain(|active| active.id != loan);
        }
    }
}

pub(super) fn mark_value_sources_moved(
    program: &Program,
    expr: &Expr,
    variables: &mut HashMap<String, Binding>,
    types: &TypeTable<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    enum Frame<'a> {
        Enter(&'a Expr, usize),
        AfterThen {
            else_branch: &'a Expr,
            parent: usize,
            then_scope: usize,
            names: Vec<String>,
        },
        AfterElse {
            parent: usize,
            else_scope: usize,
            names: Vec<String>,
            then_variables: HashMap<String, Binding>,
        },
    }
    let root = std::mem::take(variables);
    let mut scopes = vec![root];
    let mut frames = vec![Frame::Enter(expr, 0)];
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Enter(expr, scope) => match &expr.kind {
                ExprKind::Var(name) => {
                    if let Some(binding) = scopes[scope].get_mut(name) {
                        if types.needs_drop(&binding.ty)
                            && binding.mode == ParamMode::Own
                            && binding.availability == Availability::Available
                        {
                            if has_active_overlapping_loan(binding, &[]) {
                                diagnostics.push(error(
                                    program,
                                    "SPX-T265",
                                    "move or transfer would invalidate an active shared loan",
                                    expr.span,
                                ));
                            } else {
                                binding.availability = Availability::Moved;
                            }
                        }
                    }
                }
                ExprKind::Block { tail, .. } => frames.push(Frame::Enter(tail, scope)),
                ExprKind::Project { base, .. } => {
                    if let Some(place) = source_place(expr, &scopes[scope], types) {
                        if let Some(binding) = scopes[scope].get_mut(&place.root) {
                            if binding.mode == ParamMode::Own {
                                if has_active_overlapping_loan(binding, &place.projections) {
                                    diagnostics.push(error(
                                        program,
                                        "SPX-T265",
                                        "move or transfer would invalidate an active shared loan",
                                        expr.span,
                                    ));
                                } else {
                                    binding
                                        .moved_places
                                        .insert(place.projections, Availability::Moved);
                                }
                            }
                        }
                    } else {
                        frames.push(Frame::Enter(base, scope));
                    }
                }
                ExprKind::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    let names = scopes[scope].keys().cloned().collect::<Vec<_>>();
                    let then_scope = scopes.len();
                    scopes.push(scopes[scope].clone());
                    frames.push(Frame::AfterThen {
                        else_branch,
                        parent: scope,
                        then_scope,
                        names,
                    });
                    frames.push(Frame::Enter(then_branch, then_scope));
                }
                ExprKind::UpdateRecord { .. } | ExprKind::ConstructRecord { .. } => {}
                _ => {}
            },
            Frame::AfterThen {
                else_branch,
                parent,
                then_scope,
                names,
            } => {
                debug_assert_eq!(then_scope + 1, scopes.len());
                let then_variables = scopes.pop().expect("then move scope is active");
                let else_scope = scopes.len();
                scopes.push(scopes[parent].clone());
                frames.push(Frame::AfterElse {
                    parent,
                    else_scope,
                    names,
                    then_variables,
                });
                frames.push(Frame::Enter(else_branch, else_scope));
            }
            Frame::AfterElse {
                parent,
                else_scope,
                names,
                then_variables,
            } => {
                debug_assert_eq!(else_scope + 1, scopes.len());
                let else_variables = scopes.pop().expect("else move scope is active");
                for name in names {
                    if let Some(binding) = scopes[parent].get_mut(&name) {
                        let then_state = then_variables
                            .get(&name)
                            .map_or(Availability::Available, |value| value.availability);
                        let else_state = else_variables
                            .get(&name)
                            .map_or(Availability::Available, |value| value.availability);
                        binding.availability = then_state.join(else_state);
                        if let (Some(then_binding), Some(else_binding)) =
                            (then_variables.get(&name), else_variables.get(&name))
                        {
                            binding.moved_places = join_moved_places(then_binding, else_binding);
                            binding.definitely_partial =
                                join_definitely_partial(then_binding, else_binding);
                        }
                    }
                }
            }
        }
    }
    *variables = scopes.pop().expect("root move scope is retained");
}

pub(super) fn merge_moved(
    target: &mut HashMap<String, Binding>,
    source: &HashMap<String, Binding>,
    names: &[String],
) {
    for name in names {
        if let (Some(target), Some(source)) = (target.get_mut(name), source.get(name)) {
            target.availability = source.availability;
            target.moved_places.clone_from(&source.moved_places);
            target
                .definitely_partial
                .clone_from(&source.definitely_partial);
        }
    }
}

pub(super) fn join_conditional(
    baseline: &mut HashMap<String, Binding>,
    conditional: &HashMap<String, Binding>,
    names: &[String],
) {
    for name in names {
        if let (Some(baseline), Some(conditional)) = (baseline.get_mut(name), conditional.get(name))
        {
            let moved_places = join_moved_places(baseline, conditional);
            let definitely_partial = join_definitely_partial(baseline, conditional);
            baseline.availability = baseline.availability.join(conditional.availability);
            baseline
                .active_loans
                .extend(conditional.active_loans.iter().cloned());
            baseline.moved_places = moved_places;
            baseline.definitely_partial = definitely_partial;
        }
    }
}
