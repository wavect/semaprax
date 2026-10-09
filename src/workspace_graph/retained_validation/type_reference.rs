//! Authenticated projection of explicit imported-type sites through lowered HIR.

use std::collections::BTreeSet;

use crate::diagnostic::Diagnostic;
use crate::hir::{
    self, ExpressionId, FunctionExecutionId, OwnershipMode, ResolvedExpr, ResolvedExprKind,
    ResolvedStatement, ResolvedType, ValueId,
};

struct ForOwnProjection<'a> {
    source: &'a ResolvedExpr,
    body: &'a ResolvedExpr,
}

/// Collect only authored type references from the exact compiler-owned
/// `for own` expansion. The synthetic `iter_next<T>` calls, `IterStep<T>`
/// constructors, and inferred item binding repeat `T` in HIR without adding
/// an explicit source reference.
pub(super) fn collect_authored_for_own<'a>(
    owner: &'a hir::DeclarationId,
    statement: &'a ResolvedStatement,
    path: &str,
    imported: &BTreeSet<&str>,
    out: &mut Vec<super::type_projection::TypeSite<'a>>,
) -> Result<bool, Vec<Diagnostic>> {
    let Some(projection) = authenticate_for_own(owner, statement, path) else {
        return Ok(false);
    };
    super::collect_resolved_expression_type_sites(
        owner,
        projection.source,
        &crate::bounded_output::budgeted_format(format_args!("{path}.value.s0.value.arg.0")),
        imported,
        out,
    )?;
    super::collect_resolved_expression_type_sites(
        owner,
        projection.body,
        &crate::bounded_output::budgeted_format(format_args!(
            "{path}.value.s1.body.s0.value.arm.1.value.s0.value"
        )),
        imported,
        out,
    )?;
    Ok(true)
}

fn authenticate_for_own<'a>(
    owner: &hir::DeclarationId,
    statement: &'a ResolvedStatement,
    path: &str,
) -> Option<ForOwnProjection<'a>> {
    let ResolvedStatement::Let {
        binding: wrapper,
        mutable: false,
        value,
        ..
    } = statement
    else {
        return None;
    };
    let ResolvedExprKind::Block { statements, tail } = &value.kind else {
        return None;
    };
    let [ResolvedStatement::Let {
        binding: step,
        mutable: true,
        value: seed,
        ..
    }, ResolvedStatement::While {
        condition, body, ..
    }] = statements.as_slice()
    else {
        return None;
    };
    let protocol = hir::iterator_loop::recognize(condition, body)?;
    let ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance: None,
        args,
    } = &seed.kind
    else {
        return None;
    };
    let ([element], [source]) = (type_arguments.as_slice(), args.as_slice()) else {
        return None;
    };
    let execution = FunctionExecutionId::Monomorphic(hir::DeclarationId::new(
        crate::bounded_output::budgeted_clone(owner.as_str()),
    ));
    let value_path = crate::bounded_output::budgeted_format(format_args!("{path}.value"));
    let step_path = crate::bounded_output::budgeted_format(format_args!("{value_path}.s0"));
    let source_path =
        crate::bounded_output::budgeted_format(format_args!("{step_path}.value.arg.0"));
    let while_path = crate::bounded_output::budgeted_format(format_args!("{value_path}.s1"));
    let authored_body_path = crate::bounded_output::budgeted_format(format_args!(
        "{value_path}.s1.body.s0.value.arm.1.value.s0.value"
    ));
    (wrapper.name == "#for-own"
        && wrapper.id == ValueId::local(&execution, path)
        && wrapper.ty == ResolvedType::I64
        && wrapper.ownership == OwnershipMode::Value
        && value.id == ExpressionId::new(&execution, &value_path)
        && value.ty == ResolvedType::I64
        && value.ownership == OwnershipMode::Value
        && tail.id
            == ExpressionId::new(
                &execution,
                &crate::bounded_output::budgeted_format(format_args!("{value_path}.tail")),
            )
        && tail.ty == ResolvedType::I64
        && tail.ownership == OwnershipMode::Value
        && matches!(tail.kind, ResolvedExprKind::Int(0))
        && step.name == "#for-own-step"
        && step.id == ValueId::local(&execution, &step_path)
        && step.ty == seed.ty
        && step.ownership == OwnershipMode::Own
        && protocol.step == step
        && condition.id
            == ExpressionId::new(
                &execution,
                &crate::bounded_output::budgeted_format(format_args!("{while_path}.condition")),
            )
        && body.id
            == ExpressionId::new(
                &execution,
                &crate::bounded_output::budgeted_format(format_args!("{while_path}.body")),
            )
        && callee.as_str() == crate::iterator_ops::NEXT_ID
        && seed.id
            == ExpressionId::new(
                &execution,
                &crate::bounded_output::budgeted_format(format_args!("{step_path}.value")),
            )
        && seed.ty == crate::iterator_ops::resolved_iter_step(element.clone())
        && seed.ownership == OwnershipMode::Own
        && source.id == ExpressionId::new(&execution, &source_path)
        && source.ty == crate::iterator_ops::resolved_iter(element.clone())
        && source.ownership == OwnershipMode::Own
        && protocol.authored_body.id == ExpressionId::new(&execution, &authored_body_path))
    .then_some(ForOwnProjection {
        source,
        body: protocol.authored_body,
    })
}
