//! Authenticated projection of explicit imported-type sites through lowered HIR.

use std::collections::BTreeSet;

use crate::diagnostic::Diagnostic;
use crate::hir::{
    self, OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedStatement, ResolvedType,
};

struct ForOwnProjection<'a> {
    source: &'a ResolvedExpr,
    body: &'a ResolvedExpr,
}

/// Collect only authored type references from the exact compiler-owned
/// `for own` expansion. The synthetic `iter_next<T>` calls, `IterStep<T>`
/// constructors, and inferred item binding repeat `T` in HIR without adding
/// an explicit source reference.
pub(super) fn collect_authored_for_own(
    owner: &hir::DeclarationId,
    statement: &ResolvedStatement,
    path: &str,
    imported: &BTreeSet<&str>,
    out: &mut Vec<(String, String, String, String)>,
) -> Result<bool, Vec<Diagnostic>> {
    let Some(projection) = authenticate_for_own(statement) else {
        return Ok(false);
    };
    super::collect_resolved_expression_type_sites(
        owner,
        projection.source,
        &format!("{path}.value.s0.value.arg.0"),
        imported,
        out,
    )?;
    super::collect_resolved_expression_type_sites(
        owner,
        projection.body,
        &format!("{path}.value.s1.body.s0.value.arm.1.value.s0.value"),
        imported,
        out,
    )?;
    Ok(true)
}

fn authenticate_for_own(statement: &ResolvedStatement) -> Option<ForOwnProjection<'_>> {
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
    (wrapper.name == "#for-own"
        && wrapper.ty == ResolvedType::I64
        && wrapper.ownership == OwnershipMode::Value
        && value.ty == ResolvedType::I64
        && value.ownership == OwnershipMode::Value
        && matches!(tail.kind, ResolvedExprKind::Int(0))
        && step.name == "#for-own-step"
        && protocol.step.id == step.id
        && callee.as_str() == crate::iterator_ops::NEXT_ID
        && seed.ty == crate::iterator_ops::resolved_iter_step(element.clone())
        && seed.ownership == OwnershipMode::Own
        && source.ty == crate::iterator_ops::resolved_iter(element.clone())
        && source.ownership == OwnershipMode::Own)
        .then_some(ForOwnProjection {
            source,
            body: protocol.authored_body,
        })
}
