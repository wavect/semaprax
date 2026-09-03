//! Exact `own Bytes` provider-use authentication for signature replacement.
//!
//! This module admits one narrow rewrite: the old owner may occur exactly once,
//! as the unprojected root of the compiler builtin `bytes_as_slice(owner)`.

use super::*;
use crate::diagnostic::Diagnostic;
use crate::hir::{ResolvedExpr, ResolvedExprKind};

pub(super) fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G469", message)]
}

pub(super) fn authenticate_and_rewrite(
    revision: &ProjectRevision,
    function: &mut Function,
    owner: &Param,
    owner_index: usize,
    replacement: &str,
) -> Result<()> {
    let mut checked = None;
    for module in revision.semantic.image_modules() {
        for candidate in module
            .functions()
            .iter()
            .filter(|candidate| candidate.id.as_str() == function.stable_id)
        {
            if checked.replace(candidate).is_some() {
                return Err(invalid(
                    "owner-to-view replacement has ambiguous checked provider identity",
                ));
            }
        }
    }
    let checked = checked.ok_or_else(|| {
        invalid("owner-to-view replacement has no authenticated checked provider")
    })?;
    let parameter = checked.params.get(owner_index).ok_or_else(|| {
        invalid("owner-to-view replacement checked parameter inventory disagrees")
    })?;
    if parameter.name != owner.name
        || parameter.ownership != OwnershipMode::Own
        || parameter.ty != ResolvedType::Bytes
    {
        return Err(invalid(
            "owner-to-view replacement source and checked owner disagree",
        ));
    }

    let mut uses = 0usize;
    let mut pending = checked
        .requires
        .iter()
        .chain(std::iter::once(&checked.body))
        .chain(&checked.ensures)
        .collect::<Vec<_>>();
    while let Some(expression) = pending.pop() {
        match &expression.kind {
            ResolvedExprKind::BorrowPlace { operation, place } if place.root == parameter.id => {
                if operation.as_str() != crate::byte_ops::BYTES_AS_SLICE_ID
                    || !place.projections.is_empty()
                {
                    return Err(invalid(
                        "owner-to-view replacement requires the exact unprojected bytes_as_slice builtin use",
                    ));
                }
                uses += 1;
            }
            ResolvedExprKind::Place(place) | ResolvedExprKind::BorrowPlace { place, .. }
                if place.root == parameter.id =>
            {
                return Err(invalid(
                    "owner-to-view replacement owner has a non-view provider use",
                ));
            }
            _ => push_children(expression, &mut pending),
        }
    }
    if uses != 1 {
        return Err(invalid(
            "owner-to-view replacement requires exactly one authenticated bytes_as_slice owner use",
        ));
    }

    rewrite_source(function, owner, replacement, uses)
}

fn rewrite_source(
    function: &mut Function,
    owner: &Param,
    replacement: &str,
    authenticated_uses: usize,
) -> Result<()> {
    let mut nodes = 0usize;
    let mut source_calls = 0usize;
    let mut source_places = 0usize;
    super::super::walk_function(function, &mut nodes, &mut |expression| {
        if matches!(&expression.kind, ExprKind::Var(name) if name == &owner.name) {
            source_places += 1;
        }
        if matches!(&expression.kind, ExprKind::Call { name, type_arguments, args }
            if name == crate::byte_ops::BYTES_AS_SLICE_NAME
                && type_arguments.is_empty()
                && matches!(args.as_slice(), [Expr { kind: ExprKind::Var(name), .. }] if name == &owner.name))
        {
            source_calls += 1;
        }
        Ok(())
    })?;
    if source_calls != authenticated_uses || source_places != authenticated_uses {
        return Err(invalid(
            "owner-to-view replacement source uses do not match authenticated builtin evidence",
        ));
    }
    let mut rewrite_nodes = 0usize;
    super::super::walk_function(function, &mut rewrite_nodes, &mut |expression| {
        if matches!(&expression.kind, ExprKind::Call { name, type_arguments, args }
            if name == crate::byte_ops::BYTES_AS_SLICE_NAME
                && type_arguments.is_empty()
                && matches!(args.as_slice(), [Expr { kind: ExprKind::Var(name), .. }] if name == &owner.name))
        {
            expression.kind = ExprKind::Var(replacement.to_owned());
        }
        Ok(())
    })
}

fn push_children<'a>(expression: &'a ResolvedExpr, pending: &mut Vec<&'a ResolvedExpr>) {
    crate::hir::push_resolved_expression_children_in_authored_order(expression, pending);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_source_builtin_use_rewrites_without_admitting_an_owner_alias() {
        let mut program = crate::parse(
            "module sample; @id(\"sample.read\") fn read(input: own Bytes)->usize { byte_len(bytes_as_slice(input)) }",
            "sample.spx",
        )
        .unwrap();
        let owner = program.functions[0].params[0].clone();
        rewrite_source(&mut program.functions[0], &owner, "view", 1).unwrap();
        let source = crate::format::canonical(&program);
        assert!(source.contains("byte_len(view)"));
        assert!(!source.contains("bytes_as_slice(input)"));
    }

    #[test]
    fn source_shape_rejects_any_additional_owner_occurrence() {
        let mut program = crate::parse(
            "module sample; @id(\"sample.read\") fn read(input: own Bytes)->Bytes { let view = bytes_as_slice(input); input }",
            "sample.spx",
        )
        .unwrap();
        let owner = program.functions[0].params[0].clone();
        let errors = rewrite_source(&mut program.functions[0], &owner, "view", 1).unwrap_err();
        assert!(errors.iter().any(|error| error.code == "SPX-G469"));
    }
}
