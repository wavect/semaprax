//! Independent source-derived scalar-field-update callee replay at exact source
//! paths. It shares SPX-G173 with explicit sort keys: both are compiler-owned
//! literal descriptors that must equal retained HIR at each source identity.
use crate::ast::{Expr, ExprKind, ModuleUseKind, Program, Type};
use crate::bounded_output::budgeted_format;
use crate::diagnostic::Diagnostic;
use crate::hir;
use crate::workspace_graph::{graph_error, reserve_builder_structure, AuthoredDeclaration};
use std::collections::BTreeMap;
pub(super) fn verify(
    program: &Program,
    resolved: &hir::ResolvedProgram,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
) -> Result<(), Vec<Diagnostic>> {
    let fail = || {
        vec![graph_error(
            "SPX-G173",
            "scalar field update source descriptor disagrees with retained HIR",
        )]
    };
    // Path-free census first; injected synthetic HIR calls still reach the
    // exact comparison below even when the source has none.
    let source_uses = program.functions.iter().any(|function| {
        function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
            .any(contains_source)
    });
    let resolved_uses = resolved.functions.iter().any(|function| {
        function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
            .any(contains_resolved)
    }) || resolved.function_templates.iter().any(|function| {
        function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
            .any(contains_resolved)
    });
    if !source_uses && !resolved_uses {
        return Ok(());
    }
    let mut expected = Vec::new();
    for function in &program.functions {
        let owner = hir::DeclarationId::new(&function.stable_id);
        for (root, expression) in function
            .requires
            .iter()
            .enumerate()
            .map(|(i, e)| (budgeted_format(format_args!("requires.{i}")), e))
            .chain(std::iter::once((String::from("body"), &function.body)))
            .chain(
                function
                    .ensures
                    .iter()
                    .enumerate()
                    .map(|(i, e)| (budgeted_format(format_args!("ensures.{i}")), e)),
            )
        {
            crate::workspace_graph::source_callables::visit_ast_call_expressions(
                expression,
                &root,
                &mut |expr, path| {
                    let ExprKind::Call {
                        name,
                        type_arguments,
                        args,
                    } = &expr.kind
                    else {
                        unreachable!()
                    };
                    if name != crate::vec_ops::field_update::NAME {
                        return Ok(());
                    }
                    let [Type::Named { name, arguments }] = type_arguments.as_slice() else {
                        return Err(fail());
                    };
                    let [_, _, _, literal] = args.as_slice() else {
                        return Err(fail());
                    };
                    let ExprKind::String(field) = &literal.kind else {
                        return Err(fail());
                    };
                    if !arguments.is_empty() {
                        return Err(fail());
                    }
                    let declaration = program
                        .types
                        .iter()
                        .find(|ty| ty.name == *name)
                        .or_else(|| {
                            program
                                .module_uses
                                .iter()
                                .find(|item| {
                                    item.kind == ModuleUseKind::Type && item.alias == *name
                                })
                                .and_then(|item| authored.get(item.persistent_id.as_str()))
                                .and_then(|item| item.ty)
                        })
                        .ok_or_else(fail)?;
                    let descriptor = crate::vec_ops::field_update::source(declaration, field)
                        .and_then(|d| d.identity())
                        .ok_or_else(fail)?;
                    reserve_builder_structure(
                        std::mem::size_of::<(String, String, String)>()
                            + function.stable_id.len()
                            + descriptor.len(),
                    )?;
                    expected.push((
                        function.stable_id.clone(),
                        hir::workspace_expression_identity(&owner, path),
                        descriptor,
                    ));
                    Ok(())
                },
            )?;
        }
    }
    let mut actual = Vec::new();
    hir::visit_workspace_call_sites(resolved, &mut |owner, expression, target| {
        if target
            .as_str()
            .starts_with(crate::vec_ops::field_update::PREFIX)
        {
            reserve_builder_structure(
                std::mem::size_of::<(String, String, String)>()
                    + owner.as_str().len()
                    + expression.len()
                    + target.as_str().len(),
            )?;
            actual.push((
                owner.as_str().to_owned(),
                expression.to_owned(),
                target.as_str().to_owned(),
            ));
        }
        Ok::<(), Vec<Diagnostic>>(())
    })?;
    expected.sort();
    actual.sort();
    if expected != actual {
        return Err(fail());
    }
    Ok(())
}

fn contains_source(expression: &Expr) -> bool {
    // One indexed frame per ancestor; excess depth takes the full verifier.
    let mut stack = [None; 514];
    stack[0] = Some((expression, 0usize));
    let mut len = 1usize;
    while len != 0 {
        len -= 1;
        let (expression, next) = stack[len].take().expect("uniqueness census frame");
        if next == 0
            && matches!(&expression.kind, ExprKind::Call { name, .. }
                if name == crate::vec_ops::field_update::NAME)
        {
            return true;
        }
        if let Some(child) = expression.child(next) {
            if len + 2 > stack.len() {
                return true;
            }
            stack[len] = Some((expression, next + 1));
            stack[len + 1] = Some((child, 0));
            len += 2;
        }
    }
    false
}

fn contains_resolved(expression: &hir::ResolvedExpr) -> bool {
    let mut stack = [None; 514];
    stack[0] = Some((expression, 0usize));
    let mut len = 1usize;
    while len != 0 {
        len -= 1;
        let (expression, next) = stack[len].take().expect("resolved uniqueness census frame");
        if next == 0
            && matches!(&expression.kind,
            hir::ResolvedExprKind::Call { callee: target, .. }
                | hir::ResolvedExprKind::FunctionReference { target }
                if target.as_str().starts_with(crate::vec_ops::field_update::PREFIX))
        {
            return true;
        }
        if let Some(child) = hir::lexical_exit::child(expression, next) {
            if len + 2 > stack.len() {
                return true;
            }
            stack[len] = Some((expression, next + 1));
            stack[len + 1] = Some((child, 0));
            len += 2;
        }
    }
    false
}
