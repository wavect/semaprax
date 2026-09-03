//! Independent replay of retained Workspace AST, HIR, and typed edges.
//!
//! This module validates already-retained compiler facts under the parent's
//! bounded builder ledger. It has no filesystem, locking, mutation, rendering,
//! publication, backend, or runtime authority.

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::{ModuleUseKind, Program};
use crate::diagnostic::Diagnostic;
use crate::hir;

use super::{
    budgeted_edge_clone, graph_error, limit_error, push_edge, reserve_builder_structure,
    visit_ast_call_sites, CallOccurrenceKey, WorkspaceDeclarationFact, WorkspaceEdge,
    WorkspaceResolvedModule, MAX_CALLS,
};

pub(super) fn validate_retained_facts(
    programs: &[Program],
    modules: &[WorkspaceResolvedModule],
    edges: &[WorkspaceEdge],
) -> Result<(), Vec<Diagnostic>> {
    for program in programs {
        let resolved = modules
            .iter()
            .find(|item| item.module == program.module)
            .ok_or_else(|| {
                vec![graph_error(
                    "SPX-G173",
                    "retained workspace module is missing",
                )]
            })?;
        if resolved.permits != program.permits || resolved.path != program.path {
            return Err(vec![graph_error(
                "SPX-G173",
                "workspace module permit/path facts disagree with retained HIR",
            )]);
        }
    }

    let mut actual_type_sites = Vec::new();
    for module in modules {
        let source = programs
            .iter()
            .find(|program| program.module == module.module)
            .expect("retained module belongs to parsed source");
        let imported_type_ids = source
            .module_uses
            .iter()
            .filter(|item| item.kind == ModuleUseKind::Type)
            .map(|item| item.persistent_id.as_str())
            .collect::<BTreeSet<_>>();
        for declaration in &module.types {
            match &declaration.kind {
                hir::ResolvedTypeDeclarationKind::Resource { .. } => {}
                hir::ResolvedTypeDeclarationKind::Record { fields }
                | hir::ResolvedTypeDeclarationKind::Class { fields, .. } => {
                    for (index, field) in fields.iter().enumerate() {
                        let path = crate::bounded_output::budgeted_format(format_args!(
                            "type.{}.field.{index}",
                            declaration.id
                        ));
                        collect_resolved_type_sites(
                            declaration.id.as_str(),
                            &field.ty,
                            &path,
                            None,
                            &imported_type_ids,
                            &mut actual_type_sites,
                        )?;
                    }
                }
                hir::ResolvedTypeDeclarationKind::Variant { cases } => {
                    for (case_index, case) in cases.iter().enumerate() {
                        for (field_index, field) in case.fields.iter().enumerate() {
                            let path = crate::bounded_output::budgeted_format(format_args!(
                                "type.{}.case.{case_index}.field.{field_index}",
                                declaration.id
                            ));
                            collect_resolved_type_sites(
                                declaration.id.as_str(),
                                &field.ty,
                                &path,
                                None,
                                &imported_type_ids,
                                &mut actual_type_sites,
                            )?;
                        }
                    }
                }
            }
        }
        for function in &module.functions {
            collect_resolved_signature_sites(
                &function.id,
                &function.params,
                &function.return_type,
                &imported_type_ids,
                &mut actual_type_sites,
            )?;
            collect_resolved_function_type_sites(
                &function.id,
                &function.requires,
                &function.body,
                &function.ensures,
                &imported_type_ids,
                &mut actual_type_sites,
            )?;
        }
        for template in &module.function_templates {
            collect_resolved_signature_sites(
                &template.id,
                &template.params,
                &template.return_type,
                &imported_type_ids,
                &mut actual_type_sites,
            )?;
            collect_resolved_function_type_sites(
                &template.id,
                &template.requires,
                &template.body,
                &template.ensures,
                &imported_type_ids,
                &mut actual_type_sites,
            )?;
        }
    }
    let mut expected_type_sites = Vec::new();
    for edge in edges.iter().filter(|edge| edge.kind == "type_reference") {
        reserve_builder_structure(std::mem::size_of::<(String, String, String, String)>())?;
        expected_type_sites.push((
            crate::bounded_output::budgeted_clone(&edge.caller),
            crate::bounded_output::budgeted_clone(&edge.expression),
            crate::bounded_output::budgeted_clone(&edge.ast_path),
            crate::bounded_output::budgeted_clone(&edge.target),
        ));
    }
    expected_type_sites.sort();
    actual_type_sites.sort();
    if expected_type_sites != actual_type_sites {
        return Err(vec![graph_error(
            "SPX-G173",
            "workspace explicit type-reference facts disagree with retained HIR",
        )]);
    }

    let authenticated_calls = reconstruct_authenticated_call_edges(programs, modules)?;
    validate_retained_call_projection(programs, modules, &authenticated_calls)?;
    let mut emitted_calls = Vec::new();
    for edge in edges.iter().filter(|edge| edge.kind == "call") {
        push_edge(&mut emitted_calls, budgeted_edge_clone(edge))?;
    }
    emitted_calls.sort();
    if emitted_calls != authenticated_calls {
        return Err(vec![graph_error(
            "SPX-G173",
            "emitted workspace call edges disagree with authenticated AST/HIR occurrences",
        )]);
    }
    validate_effect_and_capability_edges_against_calls(modules, edges, &authenticated_calls)?;
    Ok(())
}

fn reconstruct_authenticated_call_edges(
    programs: &[Program],
    modules: &[WorkspaceResolvedModule],
) -> Result<Vec<WorkspaceEdge>, Vec<Diagnostic>> {
    let module_paths = modules
        .iter()
        .map(|module| (module.module.as_str(), module.path.as_str()))
        .collect::<BTreeMap<_, _>>();
    let mut calls = Vec::new();
    for program in programs {
        let function_uses = program
            .module_uses
            .iter()
            .filter(|item| item.kind == ModuleUseKind::Function)
            .map(|item| (item.alias.as_str(), item))
            .collect::<BTreeMap<_, _>>();
        for function in &program.functions {
            let owner =
                hir::DeclarationId::new(crate::bounded_output::budgeted_clone(&function.stable_id));
            for (site, expressions) in [
                ("requires", function.requires.as_slice()),
                ("body", std::slice::from_ref(&function.body)),
                ("ensures", function.ensures.as_slice()),
            ] {
                for (root_index, expression) in expressions.iter().enumerate() {
                    let root = match site {
                        "requires" => crate::bounded_output::budgeted_format(format_args!(
                            "requires.{root_index}"
                        )),
                        "body" => crate::bounded_output::budgeted_clone("body"),
                        "ensures" => crate::bounded_output::budgeted_format(format_args!(
                            "ensures.{root_index}"
                        )),
                        _ => unreachable!(),
                    };
                    let mut ordinal = 0usize;
                    visit_ast_call_sites(expression, &root, &mut |name, path| {
                        let call_ordinal = ordinal;
                        ordinal = ordinal
                            .checked_add(1)
                            .ok_or_else(|| vec![limit_error("calls", MAX_CALLS)])?;
                        let Some(module_use) = function_uses.get(name) else {
                            return Ok(());
                        };
                        let target_path = module_paths
                            .get(module_use.target_module.as_str())
                            .ok_or_else(|| {
                                vec![graph_error(
                                    "SPX-G173",
                                    "authenticated call target module has no retained path",
                                )]
                            })?;
                        push_edge(
                            &mut calls,
                            WorkspaceEdge {
                                caller_path: crate::bounded_output::budgeted_clone(&program.path),
                                caller: crate::bounded_output::budgeted_clone(&function.stable_id),
                                target_path: crate::bounded_output::budgeted_clone(target_path),
                                target: crate::bounded_output::budgeted_clone(
                                    &module_use.persistent_id,
                                ),
                                kind: "call",
                                site,
                                expression: hir::workspace_expression_identity(&owner, path),
                                ast_path: crate::bounded_output::budgeted_clone(path),
                                alias: crate::bounded_output::budgeted_clone(&module_use.alias),
                                ordinal: call_ordinal,
                            },
                        )
                    })?;
                }
            }
        }
    }
    calls.sort();
    Ok(calls)
}

fn validate_retained_call_projection(
    programs: &[Program],
    modules: &[WorkspaceResolvedModule],
    authenticated_calls: &[WorkspaceEdge],
) -> Result<(), Vec<Diagnostic>> {
    let mut actual = Vec::new();
    for module in modules {
        let imported_targets = programs
            .iter()
            .find(|program| program.module == module.module)
            .expect("retained module belongs to authenticated source")
            .module_uses
            .iter()
            .filter(|item| item.kind == ModuleUseKind::Function)
            .map(|item| item.persistent_id.as_str())
            .collect::<BTreeSet<_>>();
        for function in &module.functions {
            collect_retained_call_projection(
                &function.id,
                &function.requires,
                &function.body,
                &function.ensures,
                &imported_targets,
                &mut actual,
            )?;
        }
        for template in &module.function_templates {
            collect_retained_call_projection(
                &template.id,
                &template.requires,
                &template.body,
                &template.ensures,
                &imported_targets,
                &mut actual,
            )?;
        }
    }
    let mut expected = Vec::new();
    for edge in authenticated_calls {
        reserve_builder_structure(std::mem::size_of::<(String, String, String)>())?;
        expected.push((
            crate::bounded_output::budgeted_clone(&edge.caller),
            crate::bounded_output::budgeted_clone(&edge.expression),
            crate::bounded_output::budgeted_clone(&edge.target),
        ));
    }
    expected.sort();
    actual.sort();
    if actual != expected {
        return Err(vec![graph_error(
            "SPX-G173",
            "authenticated workspace call occurrences disagree with retained HIR",
        )]);
    }
    Ok(())
}

fn collect_retained_call_projection(
    owner: &hir::DeclarationId,
    requires: &[hir::ResolvedExpr],
    body: &hir::ResolvedExpr,
    ensures: &[hir::ResolvedExpr],
    imported_targets: &BTreeSet<&str>,
    output: &mut Vec<(String, String, String)>,
) -> Result<(), Vec<Diagnostic>> {
    let mut error = None;
    for expression in requires.iter().chain(std::iter::once(body)).chain(ensures) {
        visit_resolved_calls(expression, &mut |expression, target| {
            if error.is_none() && imported_targets.contains(target.as_str()) {
                if let Err(diagnostics) =
                    reserve_builder_structure(std::mem::size_of::<(String, String, String)>())
                {
                    error = Some(diagnostics);
                    return;
                }
                output.push((
                    crate::bounded_output::budgeted_clone(owner.as_str()),
                    crate::bounded_output::budgeted_format(format_args!("{}", expression.id)),
                    crate::bounded_output::budgeted_clone(target.as_str()),
                ));
            }
        });
    }
    match error {
        Some(diagnostics) => Err(diagnostics),
        None => Ok(()),
    }
}

fn visit_resolved_calls(
    expression: &hir::ResolvedExpr,
    visit: &mut impl FnMut(&hir::ResolvedExpr, &hir::DeclarationId),
) {
    match &expression.kind {
        hir::ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => {
            visit_resolved_calls(source, visit);
            visit_resolved_calls(start, visit);
            visit_resolved_calls(end, visit);
        }
        hir::ResolvedExprKind::Call { callee, args, .. } => {
            visit(expression, callee);
            for argument in args {
                visit_resolved_calls(argument, visit);
            }
        }
        hir::ResolvedExprKind::NativeRustImportCall(call) => {
            for argument in &call.args {
                visit_resolved_calls(argument, visit);
            }
        }
        hir::ResolvedExprKind::HostCommandCall(call) => {
            for argument in &call.args {
                visit_resolved_calls(argument, visit);
            }
        }
        hir::ResolvedExprKind::Unary { value, .. } => visit_resolved_calls(value, visit),
        hir::ResolvedExprKind::Binary { left, right, .. } => {
            visit_resolved_calls(left, visit);
            visit_resolved_calls(right, visit);
        }
        hir::ResolvedExprKind::String(_) => {}
        hir::ResolvedExprKind::ArrayU8(_)
        | hir::ResolvedExprKind::RepeatArrayU8 { .. }
        | hir::ResolvedExprKind::BorrowPlace { .. } => {}
        hir::ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                for index in 0..statement.child_count() {
                    visit_resolved_calls(
                        statement
                            .child(index)
                            .expect("resolved statement child count is canonical"),
                        visit,
                    );
                }
            }
            visit_resolved_calls(tail, visit);
        }
        hir::ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            visit_resolved_calls(condition, visit);
            visit_resolved_calls(then_branch, visit);
            visit_resolved_calls(else_branch, visit);
        }
        hir::ResolvedExprKind::ConstructRecord { fields, .. }
        | hir::ResolvedExprKind::ConstructVariant { fields, .. } => {
            for field in fields {
                visit_resolved_calls(&field.value, visit);
            }
        }
        hir::ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            visit_resolved_calls(scrutinee, visit);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    visit_resolved_calls(guard, visit);
                }
                visit_resolved_calls(&arm.value, visit);
            }
        }
        hir::ResolvedExprKind::Try { operand, .. }
        | hir::ResolvedExprKind::TryOption { operand, .. } => {
            visit_resolved_calls(operand, visit);
        }
        hir::ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            visit_resolved_calls(base, visit);
            for field in fields {
                visit_resolved_calls(&field.value, visit);
            }
        }
        hir::ResolvedExprKind::Project { base, .. } => visit_resolved_calls(base, visit),
        hir::ResolvedExprKind::Upcast { source } => visit_resolved_calls(source, visit),
        hir::ResolvedExprKind::Int(_)
        | hir::ResolvedExprKind::Int32(_)
        | hir::ResolvedExprKind::Char(_)
        | hir::ResolvedExprKind::Uint8(_)
        | hir::ResolvedExprKind::Usize(_)
        | hir::ResolvedExprKind::Float32(_)
        | hir::ResolvedExprKind::Float64(_)
        | hir::ResolvedExprKind::Bool(_)
        | hir::ResolvedExprKind::Place(_) => {}
    }
}

pub(super) fn validate_effect_and_capability_edges(
    modules: &[WorkspaceResolvedModule],
    edges: &[WorkspaceEdge],
) -> Result<(), Vec<Diagnostic>> {
    let mut calls = Vec::new();
    for edge in edges.iter().filter(|edge| edge.kind == "call") {
        push_edge(&mut calls, budgeted_edge_clone(edge))?;
    }
    validate_effect_and_capability_edges_against_calls(modules, edges, &calls)
}

fn validate_effect_and_capability_edges_against_calls(
    modules: &[WorkspaceResolvedModule],
    edges: &[WorkspaceEdge],
    authenticated_calls: &[WorkspaceEdge],
) -> Result<(), Vec<Diagnostic>> {
    let mut modules_by_path = BTreeMap::new();
    let mut target_functions = BTreeMap::new();
    let mut target_effects = BTreeMap::new();
    let mut caller_effects = BTreeMap::new();
    let mut module_permits = BTreeMap::new();
    for module in modules {
        if modules_by_path
            .insert(module.path.as_str(), module)
            .is_some()
        {
            return Err(vec![graph_error(
                "SPX-G173",
                "retained workspace module paths are not unique",
            )]);
        }
        let permits = module
            .permits
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        if permits.len() != module.permits.len()
            || module_permits
                .insert(module.module.as_str(), permits)
                .is_some()
        {
            return Err(vec![graph_error(
                "SPX-G173",
                "retained workspace module capability authority is not canonical",
            )]);
        }
        for function in &module.functions {
            let effects = function
                .effects
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>();
            if effects.len() != function.effects.len() {
                return Err(vec![graph_error(
                    "SPX-G173",
                    "retained workspace function effects are not canonical",
                )]);
            }
            if target_functions
                .insert(function.id.as_str(), module)
                .is_some()
                || target_effects
                    .insert(function.id.as_str(), function.effects.as_slice())
                    .is_some()
                || caller_effects
                    .insert((module.module.as_str(), function.id.as_str()), effects)
                    .is_some()
            {
                return Err(vec![graph_error(
                    "SPX-G173",
                    "retained workspace function authority is duplicated",
                )]);
            }
        }
        for template in &module.function_templates {
            let effects = template
                .effects
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>();
            if effects.len() != template.effects.len() {
                return Err(vec![graph_error(
                    "SPX-G173",
                    "retained workspace template effects are not canonical",
                )]);
            }
            if caller_effects
                .insert((module.module.as_str(), template.id.as_str()), effects)
                .is_some()
            {
                return Err(vec![graph_error(
                    "SPX-G173",
                    "retained workspace callable authority is duplicated",
                )]);
            }
        }
    }

    let mut calls = BTreeMap::new();
    let mut actual_effects = BTreeMap::<CallOccurrenceKey<'_>, BTreeSet<&str>>::new();
    for call in authenticated_calls {
        if calls
            .insert(CallOccurrenceKey::from_edge(call), call)
            .is_some()
        {
            return Err(vec![graph_error(
                "SPX-G173",
                "authenticated workspace call occurrence is duplicated",
            )]);
        }
    }
    for edge in edges {
        if edge.kind == "effect_requirement"
            && !actual_effects
                .entry(CallOccurrenceKey::from_edge(edge))
                .or_default()
                .insert(edge.target.as_str())
        {
            return Err(vec![graph_error(
                "SPX-G173",
                "workspace call effect requirement is duplicated",
            )]);
        }
    }

    for (occurrence, call) in calls {
        let caller_module = modules_by_path.get(occurrence.caller_path).ok_or_else(|| {
            vec![graph_error(
                "SPX-G173",
                "workspace call path has no retained module authority",
            )]
        })?;
        let target_module = target_functions.get(call.target.as_str()).ok_or_else(|| {
            vec![graph_error(
                "SPX-G173",
                "workspace call target has no retained function authority",
            )]
        })?;
        let required = target_effects
            .get(call.target.as_str())
            .expect("retained target effect authority was indexed");
        if target_module.path != call.target_path {
            return Err(vec![graph_error(
                "SPX-G173",
                "workspace call target path disagrees with retained authority",
            )]);
        }
        let actual = actual_effects.remove(&occurrence).unwrap_or_default();
        if actual.len() != required.len()
            || required
                .iter()
                .any(|effect| !actual.contains(effect.as_str()))
        {
            return Err(vec![graph_error(
                "SPX-G173",
                "workspace call effect requirements disagree with retained target HIR",
            )]);
        }
        let declared = caller_effects
            .get(&(caller_module.module.as_str(), occurrence.caller))
            .ok_or_else(|| {
                vec![graph_error(
                    "SPX-G173",
                    "workspace call owner has no retained callable authority",
                )]
            })?;
        let permits = module_permits
            .get(caller_module.module.as_str())
            .expect("retained module permit authority was indexed");
        if required
            .iter()
            .any(|effect| !declared.contains(effect.as_str()) || !permits.contains(effect.as_str()))
        {
            return Err(vec![graph_error(
                "SPX-G173",
                "workspace caller effect/capability authority join disagrees",
            )]);
        }
    }
    if !actual_effects.is_empty() {
        return Err(vec![graph_error(
            "SPX-G173",
            "workspace effect requirement has no exact call occurrence",
        )]);
    }

    let mut expected_capabilities = Vec::new();
    for module in modules {
        for (ordinal, permit) in module.permits.iter().enumerate() {
            let path = crate::bounded_output::budgeted_format(format_args!("permit.{ordinal}"));
            push_edge(
                &mut expected_capabilities,
                WorkspaceEdge {
                    caller_path: crate::bounded_output::budgeted_clone(&module.path),
                    caller: crate::bounded_output::budgeted_clone(&module.module),
                    target_path: crate::bounded_output::budgeted_clone(&module.path),
                    target: crate::bounded_output::budgeted_clone(permit),
                    kind: "capability_authority",
                    site: "module",
                    expression: crate::bounded_output::budgeted_clone(&path),
                    ast_path: path,
                    alias: String::new(),
                    ordinal,
                },
            )?;
        }
    }
    let mut actual_capabilities = Vec::new();
    for edge in edges
        .iter()
        .filter(|edge| edge.kind == "capability_authority")
    {
        push_edge(&mut actual_capabilities, budgeted_edge_clone(edge))?;
    }
    expected_capabilities.sort();
    actual_capabilities.sort();
    if actual_capabilities != expected_capabilities {
        return Err(vec![graph_error(
            "SPX-G173",
            "workspace capability-authority edges disagree with retained module permits",
        )]);
    }
    Ok(())
}

fn collect_resolved_signature_sites(
    owner: &hir::DeclarationId,
    params: &[hir::ResolvedParam],
    result: &hir::ResolvedType,
    imported: &BTreeSet<&str>,
    out: &mut Vec<(String, String, String, String)>,
) -> Result<(), Vec<Diagnostic>> {
    for (index, param) in params.iter().enumerate() {
        let path =
            crate::bounded_output::budgeted_format(format_args!("function.{owner}.param.{index}"));
        collect_resolved_type_sites(owner.as_str(), &param.ty, &path, None, imported, out)?;
    }
    let path = crate::bounded_output::budgeted_format(format_args!("function.{owner}.return"));
    collect_resolved_type_sites(owner.as_str(), result, &path, None, imported, out)?;
    Ok(())
}

fn collect_resolved_type_sites(
    owner: &str,
    ty: &hir::ResolvedType,
    path: &str,
    expression: Option<&str>,
    imported: &BTreeSet<&str>,
    out: &mut Vec<(String, String, String, String)>,
) -> Result<(), Vec<Diagnostic>> {
    let hir::ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(());
    };
    if imported.contains(declaration.as_str()) {
        reserve_builder_structure(std::mem::size_of::<(String, String, String, String)>())?;
        out.push((
            crate::bounded_output::budgeted_clone(owner),
            crate::bounded_output::budgeted_clone(expression.unwrap_or(path)),
            crate::bounded_output::budgeted_clone(path),
            crate::bounded_output::budgeted_clone(declaration.as_str()),
        ));
    }
    for (index, argument) in arguments.iter().enumerate() {
        collect_resolved_type_sites(
            owner,
            argument,
            &crate::bounded_output::budgeted_format(format_args!("{path}.argument.{index}")),
            expression,
            imported,
            out,
        )?;
    }
    Ok(())
}

fn collect_resolved_function_type_sites(
    owner: &hir::DeclarationId,
    requires: &[hir::ResolvedExpr],
    body: &hir::ResolvedExpr,
    ensures: &[hir::ResolvedExpr],
    imported: &BTreeSet<&str>,
    out: &mut Vec<(String, String, String, String)>,
) -> Result<(), Vec<Diagnostic>> {
    for (root, expression) in requires
        .iter()
        .enumerate()
        .map(|(index, expression)| {
            (
                crate::bounded_output::budgeted_format(format_args!("requires.{index}")),
                expression,
            )
        })
        .chain(std::iter::once((
            crate::bounded_output::budgeted_clone("body"),
            body,
        )))
        .chain(ensures.iter().enumerate().map(|(index, expression)| {
            (
                crate::bounded_output::budgeted_format(format_args!("ensures.{index}")),
                expression,
            )
        }))
    {
        collect_resolved_expression_type_sites(owner, expression, &root, imported, out)?;
    }
    Ok(())
}

fn collect_resolved_expression_type_sites(
    owner: &hir::DeclarationId,
    expression: &hir::ResolvedExpr,
    path: &str,
    imported: &BTreeSet<&str>,
    out: &mut Vec<(String, String, String, String)>,
) -> Result<(), Vec<Diagnostic>> {
    let expression_id = crate::bounded_output::budgeted_format(format_args!("{}", expression.id));
    match &expression.kind {
        hir::ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => {
            collect_resolved_expression_type_sites(
                owner,
                source,
                &format!("{path}.source"),
                imported,
                out,
            )?;
            collect_resolved_expression_type_sites(
                owner,
                start,
                &format!("{path}.start"),
                imported,
                out,
            )?;
            collect_resolved_expression_type_sites(
                owner,
                end,
                &format!("{path}.end"),
                imported,
                out,
            )?;
        }
        hir::ResolvedExprKind::String(_) => {}
        hir::ResolvedExprKind::ArrayU8(_)
        | hir::ResolvedExprKind::RepeatArrayU8 { .. }
        | hir::ResolvedExprKind::BorrowPlace { .. } => {}
        hir::ResolvedExprKind::Call {
            type_arguments,
            args,
            ..
        } => {
            for (index, argument) in type_arguments.iter().enumerate() {
                collect_resolved_type_sites(
                    owner.as_str(),
                    argument,
                    &crate::bounded_output::budgeted_format(format_args!(
                        "{path}.type_argument.{index}"
                    )),
                    Some(&expression_id),
                    imported,
                    out,
                )?;
            }
            for (index, argument) in args.iter().enumerate() {
                collect_resolved_expression_type_sites(
                    owner,
                    argument,
                    &crate::bounded_output::budgeted_format(format_args!("{path}.arg.{index}")),
                    imported,
                    out,
                )?;
            }
        }
        hir::ResolvedExprKind::NativeRustImportCall(call) => {
            for (index, argument) in call.args.iter().enumerate() {
                collect_resolved_expression_type_sites(
                    owner,
                    argument,
                    &crate::bounded_output::budgeted_format(format_args!(
                        "{path}.native_rust_arg.{index}"
                    )),
                    imported,
                    out,
                )?;
            }
        }
        hir::ResolvedExprKind::HostCommandCall(call) => {
            for (index, argument) in call.args.iter().enumerate() {
                collect_resolved_expression_type_sites(
                    owner,
                    argument,
                    &crate::bounded_output::budgeted_format(format_args!(
                        "{path}.host_command_arg.{index}"
                    )),
                    imported,
                    out,
                )?;
            }
        }
        hir::ResolvedExprKind::Unary { value, .. } => collect_resolved_expression_type_sites(
            owner,
            value,
            &crate::bounded_output::budgeted_format(format_args!("{path}.value")),
            imported,
            out,
        )?,
        hir::ResolvedExprKind::Binary { left, right, .. } => {
            collect_resolved_expression_type_sites(
                owner,
                left,
                &crate::bounded_output::budgeted_format(format_args!("{path}.left")),
                imported,
                out,
            )?;
            collect_resolved_expression_type_sites(
                owner,
                right,
                &crate::bounded_output::budgeted_format(format_args!("{path}.right")),
                imported,
                out,
            )?;
        }
        hir::ResolvedExprKind::Block { statements, tail } => {
            for (index, statement) in statements.iter().enumerate() {
                for child_index in 0..statement.child_count() {
                    let segment = if matches!(statement, hir::ResolvedStatement::While { .. }) {
                        if child_index == 0 {
                            "condition"
                        } else {
                            "body"
                        }
                    } else {
                        "value"
                    };
                    collect_resolved_expression_type_sites(
                        owner,
                        statement
                            .child(child_index)
                            .expect("resolved statement child count is canonical"),
                        &crate::bounded_output::budgeted_format(format_args!(
                            "{path}.s{index}.{segment}"
                        )),
                        imported,
                        out,
                    )?;
                }
            }
            collect_resolved_expression_type_sites(
                owner,
                tail,
                &crate::bounded_output::budgeted_format(format_args!("{path}.tail")),
                imported,
                out,
            )?;
        }
        hir::ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            for (suffix, child) in [
                ("condition", condition.as_ref()),
                ("then", then_branch.as_ref()),
                ("else", else_branch.as_ref()),
            ] {
                collect_resolved_expression_type_sites(
                    owner,
                    child,
                    &crate::bounded_output::budgeted_format(format_args!("{path}.{suffix}")),
                    imported,
                    out,
                )?;
            }
        }
        hir::ResolvedExprKind::ConstructRecord { fields, .. }
        | hir::ResolvedExprKind::ConstructVariant { fields, .. } => {
            collect_resolved_type_sites(
                owner.as_str(),
                &expression.ty,
                &crate::bounded_output::budgeted_format(format_args!("{path}.type")),
                Some(&expression_id),
                imported,
                out,
            )?;
            for (index, field) in fields.iter().enumerate() {
                collect_resolved_expression_type_sites(
                    owner,
                    &field.value,
                    &crate::bounded_output::budgeted_format(format_args!(
                        "{path}.field.{index}.value"
                    )),
                    imported,
                    out,
                )?;
            }
        }
        hir::ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            collect_resolved_expression_type_sites(
                owner,
                scrutinee,
                &crate::bounded_output::budgeted_format(format_args!("{path}.scrutinee")),
                imported,
                out,
            )?;
            for (index, arm) in arms.iter().enumerate() {
                collect_resolved_pattern_type_sites(
                    owner.as_str(),
                    &arm.pattern,
                    &crate::bounded_output::budgeted_format(format_args!(
                        "{path}.arm.{index}.pattern"
                    )),
                    &expression_id,
                    imported,
                    out,
                )?;
                if let Some(guard) = &arm.guard {
                    collect_resolved_expression_type_sites(
                        owner,
                        guard,
                        &crate::bounded_output::budgeted_format(format_args!(
                            "{path}.arm.{index}.guard"
                        )),
                        imported,
                        out,
                    )?;
                }
                collect_resolved_expression_type_sites(
                    owner,
                    &arm.value,
                    &crate::bounded_output::budgeted_format(format_args!(
                        "{path}.arm.{index}.value"
                    )),
                    imported,
                    out,
                )?;
            }
        }
        hir::ResolvedExprKind::Try { operand, .. }
        | hir::ResolvedExprKind::TryOption { operand, .. } => {
            collect_resolved_expression_type_sites(
                owner,
                operand,
                &crate::bounded_output::budgeted_format(format_args!("{path}.operand")),
                imported,
                out,
            )?;
        }
        hir::ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            collect_resolved_expression_type_sites(
                owner,
                base,
                &crate::bounded_output::budgeted_format(format_args!("{path}.base")),
                imported,
                out,
            )?;
            for (index, field) in fields.iter().enumerate() {
                collect_resolved_expression_type_sites(
                    owner,
                    &field.value,
                    &crate::bounded_output::budgeted_format(format_args!(
                        "{path}.field.{index}.value"
                    )),
                    imported,
                    out,
                )?;
            }
        }
        hir::ResolvedExprKind::Project { base, .. } => collect_resolved_expression_type_sites(
            owner,
            base,
            &crate::bounded_output::budgeted_format(format_args!("{path}.base")),
            imported,
            out,
        )?,
        hir::ResolvedExprKind::Upcast { source } => collect_resolved_expression_type_sites(
            owner,
            source,
            &crate::bounded_output::budgeted_format(format_args!("{path}.source")),
            imported,
            out,
        )?,
        hir::ResolvedExprKind::Int(_)
        | hir::ResolvedExprKind::Int32(_)
        | hir::ResolvedExprKind::Char(_)
        | hir::ResolvedExprKind::Uint8(_)
        | hir::ResolvedExprKind::Usize(_)
        | hir::ResolvedExprKind::Float32(_)
        | hir::ResolvedExprKind::Float64(_)
        | hir::ResolvedExprKind::Bool(_)
        | hir::ResolvedExprKind::Place(_) => {}
    }
    Ok(())
}

fn collect_resolved_pattern_type_sites(
    owner: &str,
    pattern: &hir::ResolvedMatchPattern,
    path: &str,
    expression: &str,
    imported: &BTreeSet<&str>,
    out: &mut Vec<(String, String, String, String)>,
) -> Result<(), Vec<Diagnostic>> {
    match pattern {
        hir::ResolvedMatchPattern::Variant { variant, .. } => {
            if imported.contains(variant.as_str()) {
                reserve_builder_structure(std::mem::size_of::<(String, String, String, String)>())?;
                out.push((
                    crate::bounded_output::budgeted_clone(owner),
                    crate::bounded_output::budgeted_clone(expression),
                    crate::bounded_output::budgeted_clone(path),
                    crate::bounded_output::budgeted_clone(variant.as_str()),
                ));
            }
        }
        hir::ResolvedMatchPattern::Record { record, fields, .. } => {
            if imported.contains(record.as_str()) {
                reserve_builder_structure(std::mem::size_of::<(String, String, String, String)>())?;
                out.push((
                    crate::bounded_output::budgeted_clone(owner),
                    crate::bounded_output::budgeted_clone(expression),
                    crate::bounded_output::budgeted_clone(path),
                    crate::bounded_output::budgeted_clone(record.as_str()),
                ));
            }
            for (index, field) in fields.iter().enumerate() {
                collect_resolved_record_pattern_type_sites(
                    owner,
                    &field.pattern,
                    &crate::bounded_output::budgeted_format(format_args!(
                        "{path}.field.{index}.pattern"
                    )),
                    expression,
                    imported,
                    out,
                )?;
            }
        }
        hir::ResolvedMatchPattern::Wildcard => {}
        // Refutable Match v1: scalar patterns contribute no named-type sites.
        hir::ResolvedMatchPattern::Literal(_) | hir::ResolvedMatchPattern::Binding(_) => {}
        hir::ResolvedMatchPattern::Or(alternatives) => {
            for (index, alternative) in alternatives.iter().enumerate() {
                collect_resolved_pattern_type_sites(
                    owner,
                    alternative,
                    &crate::bounded_output::budgeted_format(format_args!(
                        "{path}.alternative.{index}"
                    )),
                    expression,
                    imported,
                    out,
                )?;
            }
        }
    }
    Ok(())
}

fn collect_resolved_record_pattern_type_sites(
    owner: &str,
    pattern: &hir::ResolvedRecordMatchFieldPattern,
    path: &str,
    expression: &str,
    imported: &BTreeSet<&str>,
    out: &mut Vec<(String, String, String, String)>,
) -> Result<(), Vec<Diagnostic>> {
    let hir::ResolvedRecordMatchFieldPattern::Record { record, fields, .. } = pattern else {
        return Ok(());
    };
    if imported.contains(record.as_str()) {
        reserve_builder_structure(std::mem::size_of::<(String, String, String, String)>())?;
        out.push((
            crate::bounded_output::budgeted_clone(owner),
            crate::bounded_output::budgeted_clone(expression),
            crate::bounded_output::budgeted_clone(path),
            crate::bounded_output::budgeted_clone(record.as_str()),
        ));
    }
    for (index, field) in fields.iter().enumerate() {
        collect_resolved_record_pattern_type_sites(
            owner,
            &field.pattern,
            &crate::bounded_output::budgeted_format(format_args!("{path}.field.{index}.pattern")),
            expression,
            imported,
            out,
        )?;
    }
    Ok(())
}

/// Exact Native Rust interfaces one scalar Project closure retains, with the
/// union of their imports' declared effects.
///
/// The scalar linker profile has no callable ABI for an ordinary interface
/// import, so this inventory is empty for every other Project profile and the
/// admission and linking of those profiles is unchanged.
pub(super) struct ScalarNativeImports {
    interfaces: Vec<hir::ResolvedInterface>,
    effects: BTreeSet<String>,
}

/// Select the Native Rust import inventory the scalar Project profile retains
/// beside its linked closure.
pub(super) fn scalar_native_imports<'a>(
    profile: crate::project::ProjectProfile,
    interfaces: impl IntoIterator<Item = &'a hir::ResolvedInterface>,
) -> ScalarNativeImports {
    if profile != crate::project::ProjectProfile::ScalarV1 {
        return ScalarNativeImports {
            interfaces: Vec::new(),
            effects: BTreeSet::new(),
        };
    }
    let interfaces = interfaces.into_iter().cloned().collect::<Vec<_>>();
    let effects = interfaces
        .iter()
        .flat_map(|interface| &interface.imports)
        .filter(|import| import.native_rust)
        .flat_map(|import| import.effects.iter().cloned())
        .collect();
    ScalarNativeImports {
        interfaces,
        effects,
    }
}

impl ScalarNativeImports {
    /// Whether every one of these declared effects or module permits is
    /// carried by a retained Native Rust import. With nothing retained only
    /// the empty declaration is admitted, which is the historical rule.
    pub(super) fn effects_admitted(&self, declared: &[String]) -> bool {
        declared.iter().all(|effect| self.effects.contains(effect))
    }

    /// Link one scalar closure, retaining the selected interfaces and their
    /// imports in the linked declaration index. With nothing retained this is
    /// the unchanged pure scalar linker.
    pub(super) fn link(
        self,
        module: String,
        entrypoint: hir::DeclarationId,
        functions: Vec<hir::LinkedScalarFunction>,
        declarations: &BTreeMap<String, WorkspaceDeclarationFact>,
    ) -> Result<hir::ResolvedProgram, Diagnostic> {
        if self.interfaces.is_empty() {
            return hir::link_scalar_workspace(module, entrypoint, functions);
        }
        let mut declaration_facts = BTreeMap::new();
        for linked in &functions {
            retain_linked_fact(
                declarations,
                &mut declaration_facts,
                &linked.function.id,
                hir::DeclarationKind::Function,
                None,
            )?;
        }
        for interface in &self.interfaces {
            retain_linked_fact(
                declarations,
                &mut declaration_facts,
                &interface.id,
                hir::DeclarationKind::Interface,
                None,
            )?;
            for import in &interface.imports {
                retain_linked_fact(
                    declarations,
                    &mut declaration_facts,
                    &import.id,
                    hir::DeclarationKind::Import,
                    Some(&interface.id),
                )?;
            }
        }
        hir::link_scalar_native_rust_workspace(
            module,
            entrypoint,
            functions,
            hir::LinkedScalarNatives {
                interfaces: self.interfaces,
                declaration_facts,
            },
        )
    }
}

/// Whether one retained module's declared permits stay inside the Project
/// profile's admitted authority.
pub(super) fn permits_admitted(
    profile: crate::project::ProjectProfile,
    module: &WorkspaceResolvedModule,
    entry_module: &str,
    natives: &ScalarNativeImports,
) -> bool {
    module.permits.is_empty()
        || natives.effects_admitted(&module.permits)
        || (matches!(
            profile,
            crate::project::ProjectProfile::UsefulDataCommandV1
                | crate::project::ProjectProfile::UsefulDataCommandV2
        ) && module.module == entry_module
            && module.permits == [crate::host_io_ops::STDOUT_WRITE_EFFECT])
        || (matches!(
            profile,
            crate::project::ProjectProfile::LanguageCommandIoV1
                | crate::project::ProjectProfile::LineCommandIoV1
        ) && module.module == entry_module
            && module.permits
                == [
                    crate::command_io_ops::ARGS_READ_EFFECT,
                    crate::command_io_ops::STDERR_WRITE_EFFECT,
                    crate::command_io_ops::STDIN_READ_EFFECT,
                    crate::host_io_ops::STDOUT_WRITE_EFFECT,
                ])
}

/// Bind one retained declaration to its authenticated Phase-A fact. Both an
/// absent fact and a disagreeing or repeated one fail closed.
fn retain_linked_fact(
    authenticated: &BTreeMap<String, WorkspaceDeclarationFact>,
    selected: &mut BTreeMap<hir::DeclarationId, hir::LinkedDeclarationFact>,
    id: &hir::DeclarationId,
    kind: hir::DeclarationKind,
    owner: Option<&hir::DeclarationId>,
) -> Result<(), Diagnostic> {
    let Some(fact) = authenticated.get(id.as_str()) else {
        return Err(graph_error(
            "SPX-G173",
            format!("scalar Native Rust declaration `{id}` has no Phase-A fact"),
        ));
    };
    if fact.kind != kind || fact.owner.as_deref() != owner.map(hir::DeclarationId::as_str) {
        return Err(graph_error(
            "SPX-G173",
            format!("scalar Native Rust declaration `{id}` disagrees with its Phase-A fact"),
        ));
    }
    if selected
        .insert(
            id.clone(),
            hir::LinkedDeclarationFact {
                kind: fact.kind,
                origin: fact.origin,
                owner: owner.cloned(),
            },
        )
        .is_some()
    {
        return Err(graph_error(
            "SPX-G173",
            format!("scalar Native Rust declaration `{id}` is selected more than once"),
        ));
    }
    Ok(())
}
