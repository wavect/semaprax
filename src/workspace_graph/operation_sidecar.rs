//! Bounded AST/HIR occurrence proof for Semantic Workspace Operations.
//!
//! This module derives authenticated, canonically ordered rename/alias facts.
//! It has no filesystem, locking, staging, publication, or runtime authority.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

use crate::ast::{Expr, ExprKind, ModuleUseKind, Program, Span, Type, TypeDeclarationKind};
use crate::diagnostic::Diagnostic;
use crate::hir;

use super::{
    active_builder_limit, graph_error, limit_error, reserve_builder_structure, AuthoredDeclaration,
    WorkspaceOperationDeclaration, WorkspaceOperationImport, WorkspaceOperationOccurrence,
    WorkspaceOperationSidecar, WorkspaceResolvedModule, WorkspaceSource,
};

pub(super) fn build_operation_sidecar(
    programs: &[Program],
    sources: &[WorkspaceSource],
    modules: &[WorkspaceResolvedModule],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
) -> Result<WorkspaceOperationSidecar, Vec<Diagnostic>> {
    let modules = modules
        .iter()
        .map(|module| OperationModule {
            path: &module.path,
            types: &module.types,
            interfaces: &module.interfaces,
            functions: &module.functions,
            function_templates: &module.function_templates,
        })
        .collect::<Vec<_>>();
    build_sidecar(programs, sources, &modules, authored, true)
}

struct OperationModule<'a> {
    path: &'a str,
    types: &'a [hir::ResolvedTypeDeclaration],
    interfaces: &'a [hir::ResolvedInterface],
    functions: &'a [hir::ResolvedFunction],
    function_templates: &'a [hir::ResolvedFunctionTemplate],
}

/// Reuse the exact source/HIR occurrence join over an already checked Project.
/// Borrowed views retain no additional HIR and grant no source authority.
pub(crate) fn project_operation_sidecar(
    programs: &[Program],
    sources: &[WorkspaceSource],
    modules: &[super::WorkspaceGraphProjectionModule],
) -> Result<WorkspaceOperationSidecar, Vec<Diagnostic>> {
    let authored = super::index_authored(programs)?;
    let modules = modules
        .iter()
        .map(|module| OperationModule {
            path: module.path(),
            types: module.types(),
            interfaces: module.interfaces(),
            functions: module.functions(),
            function_templates: module.function_templates(),
        })
        .collect::<Vec<_>>();
    build_sidecar(programs, sources, &modules, &authored, false)
}

fn build_sidecar(
    programs: &[Program],
    sources: &[WorkspaceSource],
    modules: &[OperationModule<'_>],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    fingerprints: bool,
) -> Result<WorkspaceOperationSidecar, Vec<Diagnostic>> {
    let source_bytes = sources
        .iter()
        .try_fold(0usize, |total, source| {
            total.checked_add(source.source.len())
        })
        .ok_or_else(|| vec![limit_error("change_builder_bytes", active_builder_limit())])?;
    let structural_prebound = source_bytes
        .checked_mul(4)
        .and_then(|bytes| {
            bytes.checked_add(programs.len().checked_mul(std::mem::size_of::<Program>())?)
        })
        .and_then(|bytes| {
            bytes.checked_add(authored.len().checked_mul(
                std::mem::size_of::<WorkspaceOperationDeclaration>()
                    + std::mem::size_of::<WorkspaceOperationImport>()
                    + std::mem::size_of::<(String, usize)>(),
            )?)
        })
        .ok_or_else(|| vec![limit_error("change_builder_bytes", active_builder_limit())])?;
    reserve_builder_structure(structural_prebound)?;
    let mut declarations = Vec::new();
    let mut declaration_index = BTreeMap::new();
    for program in programs {
        for declaration in &program.types {
            let kind = match declaration.kind {
                TypeDeclarationKind::Resource { .. } => "resource",
                TypeDeclarationKind::Record { .. } | TypeDeclarationKind::Class { .. } => "record",
                TypeDeclarationKind::Variant { .. } => "variant",
            };
            push_operation_declaration(
                &mut declarations,
                &mut declaration_index,
                program,
                &declaration.stable_id,
                kind,
                declaration.explicit_id,
                &declaration.name,
                declaration.name_span,
                declaration.span,
            )?;
            match &declaration.kind {
                TypeDeclarationKind::Record { fields } => {
                    for field in fields {
                        push_operation_member(
                            &mut declarations,
                            &mut declaration_index,
                            program,
                            &field.stable_id,
                            "record_field",
                            declaration.explicit_id && field.explicit_id,
                            &field.name,
                            field.name_span,
                            field.span,
                            &declaration.stable_id,
                        )?;
                    }
                }
                TypeDeclarationKind::Variant { cases } => {
                    for case in cases {
                        push_operation_member(
                            &mut declarations,
                            &mut declaration_index,
                            program,
                            &case.stable_id,
                            "variant_case",
                            declaration.explicit_id && case.explicit_id,
                            &case.name,
                            case.name_span,
                            case.span,
                            &declaration.stable_id,
                        )?;
                        for field in &case.fields {
                            push_operation_member(
                                &mut declarations,
                                &mut declaration_index,
                                program,
                                &field.stable_id,
                                "variant_field",
                                declaration.explicit_id && case.explicit_id && field.explicit_id,
                                &field.name,
                                field.name_span,
                                field.span,
                                &case.stable_id,
                            )?;
                        }
                    }
                }
                _ => {}
            }
        }
        for interface in &program.interfaces {
            push_operation_declaration(
                &mut declarations,
                &mut declaration_index,
                program,
                &interface.stable_id,
                "interface",
                interface.explicit_id,
                &interface.name,
                interface.name_span,
                interface.span,
            )?;
        }
        for function in &program.functions {
            push_operation_declaration(
                &mut declarations,
                &mut declaration_index,
                program,
                &function.stable_id,
                if function.type_parameters.is_empty() {
                    "function"
                } else {
                    "function_template"
                },
                function.explicit_id,
                &function.name,
                function.name_span,
                function.span,
            )?;
        }
    }
    let mut imports = Vec::new();
    let mut import_index = BTreeMap::new();
    for program in programs {
        for module_use in &program.module_uses {
            if module_use.kind == ModuleUseKind::Protocol {
                continue;
            }
            let target = authored
                .get(module_use.persistent_id.as_str())
                .ok_or_else(|| {
                    vec![graph_error(
                        "SPX-G173",
                        "workspace operations import target is absent",
                    )]
                })?;
            let family_matches = match module_use.kind {
                ModuleUseKind::Function => target.function.is_some(),
                ModuleUseKind::Type => target.ty.is_some(),
                ModuleUseKind::Protocol => unreachable!(),
            };
            if !target.explicit || !family_matches || target.module != module_use.target_module {
                continue;
            }
            let key = (
                program.path.clone(),
                module_use.kind,
                module_use.persistent_id.clone(),
                module_use.target_module.clone(),
            );
            if import_index.insert(key, imports.len()).is_some() {
                return Err(vec![graph_error(
                    "SPX-G173",
                    "workspace operations import binding is duplicated",
                )]);
            }
            imports.push(WorkspaceOperationImport {
                path: crate::bounded_output::budgeted_clone(&program.path),
                kind: match module_use.kind {
                    ModuleUseKind::Function => "function",
                    ModuleUseKind::Type => "type",
                    ModuleUseKind::Protocol => unreachable!(),
                },
                target_id: crate::bounded_output::budgeted_clone(&module_use.persistent_id),
                target_module: crate::bounded_output::budgeted_clone(&module_use.target_module),
                alias: crate::bounded_output::budgeted_clone(&module_use.alias),
                occurrences: Vec::new(),
            });
        }
    }
    // Occurrence binding uses this canonical order for logarithmic direct-import
    // lookup; the authenticated key is unique from the construction above.
    imports.sort_by(|left, right| {
        (&left.path, left.kind, &left.target_id, &left.target_module).cmp(&(
            &right.path,
            right.kind,
            &right.target_id,
            &right.target_module,
        ))
    });
    let sources = sources
        .iter()
        .map(|source| (source.path.as_str(), source.source.as_str()))
        .collect::<BTreeMap<_, _>>();
    for program in programs {
        let source = sources.get(program.path.as_str()).ok_or_else(|| {
            vec![graph_error(
                "SPX-G173",
                "workspace operations retained source is absent",
            )]
        })?;
        // The lexer can produce at most one token per input byte plus EOF. Debit
        // that conservative envelope before it allocates so operation-sidecar
        // discovery cannot briefly exceed its active builder authority.
        reserve_builder_structure(
            source
                .len()
                .checked_add(1)
                .and_then(|count| count.checked_mul(std::mem::size_of::<crate::lexer::Token>()))
                .ok_or_else(|| vec![limit_error("change_builder_bytes", active_builder_limit())])?,
        )?;
        let tokens = crate::lexer::lex(source, &program.path).map_err(|error| vec![error])?;
        for module_use in &program.module_uses {
            if module_use.kind == ModuleUseKind::Protocol {
                continue;
            }
            let alias_span = module_use_alias_span(&tokens, module_use)?;
            let family = match module_use.kind {
                ModuleUseKind::Function => "function",
                ModuleUseKind::Type => "type",
                ModuleUseKind::Protocol => unreachable!(),
            };
            let index = imports
                .binary_search_by(|item| {
                    (&item.path, item.kind, &item.target_id, &item.target_module).cmp(&(
                        &program.path,
                        family,
                        &module_use.persistent_id,
                        &module_use.target_module,
                    ))
                })
                .map_err(|_| operation_sidecar_disagreement())?;
            reserve_builder_structure(std::mem::size_of::<WorkspaceOperationOccurrence>())?;
            imports[index]
                .occurrences
                .push(WorkspaceOperationOccurrence {
                    path: crate::bounded_output::budgeted_clone(&program.path),
                    span: alias_span,
                    owner: None,
                    shorthand_binding: None,
                });
        }
        let resolved = modules
            .iter()
            .find(|module| module.path == program.path)
            .ok_or_else(|| {
                vec![graph_error(
                    "SPX-G173",
                    "workspace operations retained HIR module is absent",
                )]
            })?;
        collect_program_operation_occurrences(
            program,
            resolved,
            &tokens,
            &declaration_index,
            &import_index,
            &mut declarations,
            &mut imports,
        )?;
    }
    declarations.sort_by(|left, right| {
        (&left.path, left.kind, &left.id).cmp(&(&right.path, right.kind, &right.id))
    });
    imports.sort_by(|left, right| {
        (&left.path, left.kind, &left.target_id, &left.target_module).cmp(&(
            &right.path,
            right.kind,
            &right.target_id,
            &right.target_module,
        ))
    });
    for occurrences in declarations
        .iter_mut()
        .map(|item| &mut item.occurrences)
        .chain(imports.iter_mut().map(|item| &mut item.occurrences))
    {
        occurrences.sort_by(|left, right| {
            (&left.path, left.span.start, left.span.end, &left.owner).cmp(&(
                &right.path,
                right.span.start,
                right.span.end,
                &right.owner,
            ))
        });
        if occurrences.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(vec![graph_error(
                "SPX-G173",
                "workspace operations occurrence proof is duplicated",
            )]);
        }
    }
    if !fingerprints {
        return Ok(WorkspaceOperationSidecar {
            declarations,
            imports,
        });
    }
    reserve_builder_structure(
        declarations
            .len()
            .checked_mul(std::mem::size_of::<String>())
            .ok_or_else(|| vec![limit_error("change_builder_bytes", active_builder_limit())])?,
    )?;
    let normalized_fingerprints = declarations
        .iter()
        .map(|declaration| {
            operation_declaration_fingerprint(declaration, &declarations, &imports, &sources)
        })
        .collect::<Result<Vec<_>, _>>()?;
    for (declaration, fingerprint) in declarations.iter_mut().zip(normalized_fingerprints) {
        declaration.normalized_fingerprint = fingerprint;
    }
    Ok(WorkspaceOperationSidecar {
        declarations,
        imports,
    })
}

fn operation_declaration_fingerprint(
    declaration: &WorkspaceOperationDeclaration,
    declarations: &[WorkspaceOperationDeclaration],
    imports: &[WorkspaceOperationImport],
    sources: &BTreeMap<&str, &str>,
) -> Result<String, Vec<Diagnostic>> {
    let source = sources
        .get(declaration.path.as_str())
        .ok_or_else(operation_sidecar_disagreement)?;
    source
        .get(declaration.span.start..declaration.span.end)
        .ok_or_else(operation_sidecar_disagreement)?;
    let occurrence_count = declarations
        .iter()
        .map(|target| target.occurrences.len())
        .chain(imports.iter().map(|target| target.occurrences.len()))
        .try_fold(0usize, usize::checked_add)
        .ok_or_else(operation_sidecar_disagreement)?;
    reserve_builder_structure(
        occurrence_count
            .checked_mul(std::mem::size_of::<(Span, &str, Option<&str>)>())
            .ok_or_else(|| vec![limit_error("change_builder_bytes", active_builder_limit())])?,
    )?;
    let mut substitutions = declarations
        .iter()
        .flat_map(|target| {
            target.occurrences.iter().filter_map(|occurrence| {
                (occurrence.path == declaration.path
                    && occurrence.span.start >= declaration.span.start
                    && occurrence.span.end <= declaration.span.end)
                    .then_some((
                        occurrence.span,
                        target.id.as_str(),
                        occurrence.shorthand_binding.as_deref(),
                    ))
            })
        })
        .chain(imports.iter().flat_map(|target| {
            target.occurrences.iter().filter_map(|occurrence| {
                (occurrence.path == declaration.path
                    && occurrence.span.start >= declaration.span.start
                    && occurrence.span.end <= declaration.span.end)
                    .then_some((
                        occurrence.span,
                        target.target_id.as_str(),
                        occurrence.shorthand_binding.as_deref(),
                    ))
            })
        }))
        .collect::<Vec<_>>();
    substitutions.sort_by_key(|(span, _, _)| (span.start, span.end));
    if substitutions
        .windows(2)
        .any(|pair| pair[0].0.end > pair[1].0.start)
    {
        return Err(operation_sidecar_disagreement());
    }
    let mut hasher = Sha256::new();
    hasher.update(b"semaprax.semantic-workspace-operations.normalized-declaration.v1\0");
    let mut cursor = declaration.span.start;
    for (span, identity, shorthand_binding) in substitutions {
        hasher.update(&source.as_bytes()[cursor..span.start]);
        hasher.update((identity.len() as u64).to_le_bytes());
        hasher.update(identity.as_bytes());
        if let Some(binding) = shorthand_binding {
            hasher.update(b": ");
            hasher.update(binding.as_bytes());
        }
        cursor = span.end;
    }
    hasher.update(&source.as_bytes()[cursor..declaration.span.end]);
    reserve_builder_structure(71)?;
    Ok(crate::bounded_output::budgeted_format(format_args!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )))
}

#[allow(
    clippy::too_many_arguments,
    reason = "sealed declaration-sidecar fact construction keeps every authenticated component explicit"
)]
fn push_operation_declaration(
    declarations: &mut Vec<WorkspaceOperationDeclaration>,
    index: &mut BTreeMap<String, usize>,
    program: &Program,
    id: &str,
    kind: &'static str,
    explicit: bool,
    name: &str,
    name_span: Span,
    span: Span,
) -> Result<(), Vec<Diagnostic>> {
    reserve_builder_structure(
        std::mem::size_of::<WorkspaceOperationDeclaration>()
            + std::mem::size_of::<WorkspaceOperationOccurrence>(),
    )?;
    if index.insert(id.to_owned(), declarations.len()).is_some() {
        return Err(vec![graph_error(
            "SPX-G173",
            "workspace operations declaration identity is duplicated",
        )]);
    }
    declarations.push(WorkspaceOperationDeclaration {
        path: crate::bounded_output::budgeted_clone(&program.path),
        module: crate::bounded_output::budgeted_clone(&program.module),
        id: crate::bounded_output::budgeted_clone(id),
        kind,
        explicit,
        name: crate::bounded_output::budgeted_clone(name),
        namespace_owner: None,
        span,
        normalized_fingerprint: String::new(),
        occurrences: vec![WorkspaceOperationOccurrence {
            path: crate::bounded_output::budgeted_clone(&program.path),
            span: name_span,
            owner: Some(crate::bounded_output::budgeted_clone(id)),
            shorthand_binding: None,
        }],
    });
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn push_operation_member(
    declarations: &mut Vec<WorkspaceOperationDeclaration>,
    index: &mut BTreeMap<String, usize>,
    program: &Program,
    id: &str,
    kind: &'static str,
    explicit: bool,
    name: &str,
    name_span: Span,
    span: Span,
    parent: &str,
) -> Result<(), Vec<Diagnostic>> {
    push_operation_declaration(
        declarations,
        index,
        program,
        id,
        kind,
        explicit,
        name,
        name_span,
        span,
    )?;
    declarations
        .last_mut()
        .ok_or_else(operation_sidecar_disagreement)?
        .namespace_owner = Some(crate::bounded_output::budgeted_clone(parent));
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn collect_program_operation_occurrences(
    program: &Program,
    resolved: &OperationModule<'_>,
    tokens: &[crate::lexer::Token],
    declaration_index: &BTreeMap<String, usize>,
    import_index: &BTreeMap<(String, ModuleUseKind, String, String), usize>,
    declarations: &mut [WorkspaceOperationDeclaration],
    imports: &mut [WorkspaceOperationImport],
) -> Result<(), Vec<Diagnostic>> {
    for declaration in &program.types {
        let resolved_declaration = resolved
            .types
            .iter()
            .find(|item| item.id.as_str() == declaration.stable_id)
            .ok_or_else(operation_sidecar_disagreement)?;
        match (&declaration.kind, &resolved_declaration.kind) {
            (
                TypeDeclarationKind::Record { fields },
                hir::ResolvedTypeDeclarationKind::Record {
                    fields: resolved_fields,
                },
            ) => {
                if fields.len() != resolved_fields.len() {
                    return Err(operation_sidecar_disagreement());
                }
                for (field, resolved_field) in fields.iter().zip(resolved_fields) {
                    if field.stable_id != resolved_field.id.as_str()
                        || field.name != resolved_field.name
                    {
                        return Err(operation_sidecar_disagreement());
                    }
                    let mut cursor = field.name_span.end;
                    collect_operation_type_occurrences(
                        program,
                        &field.ty,
                        &resolved_field.ty,
                        tokens,
                        &mut cursor,
                        field.span.end,
                        Some(&declaration.stable_id),
                        declaration_index,
                        import_index,
                        declarations,
                        imports,
                    )?;
                }
            }
            (
                TypeDeclarationKind::Variant { cases },
                hir::ResolvedTypeDeclarationKind::Variant {
                    cases: resolved_cases,
                },
            ) => {
                if cases.len() != resolved_cases.len() {
                    return Err(operation_sidecar_disagreement());
                }
                for (case, resolved_case) in cases.iter().zip(resolved_cases) {
                    if case.stable_id != resolved_case.id.as_str()
                        || case.name != resolved_case.name
                    {
                        return Err(operation_sidecar_disagreement());
                    }
                    if case.fields.len() != resolved_case.fields.len() {
                        return Err(operation_sidecar_disagreement());
                    }
                    for (field, resolved_field) in case.fields.iter().zip(&resolved_case.fields) {
                        if field.stable_id != resolved_field.id.as_str()
                            || field.name != resolved_field.name
                        {
                            return Err(operation_sidecar_disagreement());
                        }
                        let mut cursor = field.name_span.end;
                        collect_operation_type_occurrences(
                            program,
                            &field.ty,
                            &resolved_field.ty,
                            tokens,
                            &mut cursor,
                            field.span.end,
                            Some(&declaration.stable_id),
                            declaration_index,
                            import_index,
                            declarations,
                            imports,
                        )?;
                    }
                }
            }
            (
                TypeDeclarationKind::Resource { .. },
                hir::ResolvedTypeDeclarationKind::Resource { .. },
            ) => {}
            _ => return Err(operation_sidecar_disagreement()),
        }
    }
    for interface in &program.interfaces {
        let resolved_interface = resolved
            .interfaces
            .iter()
            .find(|item| item.id.as_str() == interface.stable_id)
            .ok_or_else(operation_sidecar_disagreement)?;
        if interface.imports.len() != resolved_interface.imports.len() {
            return Err(operation_sidecar_disagreement());
        }
        for (import, resolved_import) in interface.imports.iter().zip(&resolved_interface.imports) {
            if import.params.len() != resolved_import.parameters.len() {
                return Err(operation_sidecar_disagreement());
            }
            for (index, (param, resolved_param)) in import
                .params
                .iter()
                .zip(&resolved_import.parameters)
                .enumerate()
            {
                let mut cursor = param.span.end;
                let end = import
                    .params
                    .get(index + 1)
                    .map_or(import.span.end, |next| next.span.start);
                collect_operation_type_occurrences(
                    program,
                    &param.ty,
                    &resolved_param.ty,
                    tokens,
                    &mut cursor,
                    end,
                    Some(&interface.stable_id),
                    declaration_index,
                    import_index,
                    declarations,
                    imports,
                )?;
            }
        }
    }
    for function in &program.functions {
        let (resolved_params, resolved_return, requires, body, ensures) =
            if function.type_parameters.is_empty() {
                let item = resolved
                    .functions
                    .iter()
                    .find(|item| item.id.as_str() == function.stable_id)
                    .ok_or_else(operation_sidecar_disagreement)?;
                (
                    item.params.as_slice(),
                    &item.return_type,
                    item.requires.as_slice(),
                    &item.body,
                    item.ensures.as_slice(),
                )
            } else {
                let item = resolved
                    .function_templates
                    .iter()
                    .find(|item| item.id.as_str() == function.stable_id)
                    .ok_or_else(operation_sidecar_disagreement)?;
                (
                    item.params.as_slice(),
                    &item.return_type,
                    item.requires.as_slice(),
                    &item.body,
                    item.ensures.as_slice(),
                )
            };
        if function.params.len() != resolved_params.len()
            || function.requires.len() != requires.len()
            || function.ensures.len() != ensures.len()
        {
            return Err(operation_sidecar_disagreement());
        }
        for (index, (param, resolved_param)) in
            function.params.iter().zip(resolved_params).enumerate()
        {
            let mut cursor = param.span.end;
            let end = function
                .params
                .get(index + 1)
                .map_or(function.body.span.start, |next| next.span.start);
            collect_operation_type_occurrences(
                program,
                &param.ty,
                &resolved_param.ty,
                tokens,
                &mut cursor,
                end,
                Some(&function.stable_id),
                declaration_index,
                import_index,
                declarations,
                imports,
            )?;
        }
        let mut return_cursor = tokens
            .iter()
            .find(|token| {
                token.span.start >= function.name_span.end
                    && token.span.end <= function.body.span.start
                    && token.kind == crate::lexer::TokenKind::Arrow
            })
            .map(|token| token.span.end)
            .ok_or_else(operation_sidecar_disagreement)?;
        collect_operation_type_occurrences(
            program,
            &function.return_type,
            resolved_return,
            tokens,
            &mut return_cursor,
            function.body.span.start,
            Some(&function.stable_id),
            declaration_index,
            import_index,
            declarations,
            imports,
        )?;
        for (source, resolved_expression) in function.requires.iter().zip(requires) {
            collect_operation_expr_occurrences(
                program,
                source,
                resolved_expression,
                tokens,
                &function.stable_id,
                declaration_index,
                import_index,
                declarations,
                imports,
            )?;
        }
        collect_operation_expr_occurrences(
            program,
            &function.body,
            body,
            tokens,
            &function.stable_id,
            declaration_index,
            import_index,
            declarations,
            imports,
        )?;
        for (source, resolved_expression) in function.ensures.iter().zip(ensures) {
            collect_operation_expr_occurrences(
                program,
                source,
                resolved_expression,
                tokens,
                &function.stable_id,
                declaration_index,
                import_index,
                declarations,
                imports,
            )?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn collect_operation_type_occurrences(
    program: &Program,
    source: &Type,
    resolved: &hir::ResolvedType,
    tokens: &[crate::lexer::Token],
    cursor: &mut usize,
    end: usize,
    owner: Option<&str>,
    declaration_index: &BTreeMap<String, usize>,
    import_index: &BTreeMap<(String, ModuleUseKind, String, String), usize>,
    declarations: &mut [WorkspaceOperationDeclaration],
    imports: &mut [WorkspaceOperationImport],
) -> Result<(), Vec<Diagnostic>> {
    let Type::Named { name, arguments } = source else {
        use hir::ResolvedType as R;
        return if matches!(
            (source, resolved),
            (Type::I64, R::I64)
                | (Type::I32, R::I32)
                | (Type::Char, R::Char)
                | (Type::U8, R::U8)
                | (Type::Usize, R::Usize)
                | (Type::F32, R::F32)
                | (Type::F64, R::F64)
                | (Type::Bool, R::Bool)
                | (Type::String, R::String)
                | (Type::Bytes, R::Bytes)
                | (Type::Str, R::Str)
                | (Type::SliceU8, R::SliceU8)
        ) || matches!((source,resolved),(Type::ArrayU8(a),R::ArrayU8(b)) if a==b)
        {
            Ok(())
        } else {
            Err(operation_sidecar_disagreement())
        };
    };
    if matches!(resolved, hir::ResolvedType::TypeParameter { .. }) {
        if !arguments.is_empty() {
            return Err(operation_sidecar_disagreement());
        }
        let span = find_identifier_token(tokens, name, *cursor, end)?;
        *cursor = span.end;
        return Ok(());
    }
    let hir::ResolvedType::Nominal {
        declaration,
        arguments: resolved_arguments,
    } = resolved
    else {
        return Err(operation_sidecar_disagreement());
    };
    if arguments.len() != resolved_arguments.len() {
        return Err(operation_sidecar_disagreement());
    }
    let span = find_identifier_token(tokens, name, *cursor, end)?;
    *cursor = span.end;
    push_bound_operation_occurrence(
        program,
        declaration.as_str(),
        ModuleUseKind::Type,
        span,
        owner,
        declaration_index,
        import_index,
        declarations,
        imports,
    )?;
    for (argument, resolved_argument) in arguments.iter().zip(resolved_arguments) {
        collect_operation_type_occurrences(
            program,
            argument,
            resolved_argument,
            tokens,
            cursor,
            end,
            owner,
            declaration_index,
            import_index,
            declarations,
            imports,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn collect_operation_expr_occurrences(
    program: &Program,
    source: &Expr,
    resolved: &hir::ResolvedExpr,
    tokens: &[crate::lexer::Token],
    owner: &str,
    declaration_index: &BTreeMap<String, usize>,
    import_index: &BTreeMap<(String, ModuleUseKind, String, String), usize>,
    declarations: &mut [WorkspaceOperationDeclaration],
    imports: &mut [WorkspaceOperationImport],
) -> Result<(), Vec<Diagnostic>> {
    use hir::ResolvedExprKind as R;
    match (&source.kind, &resolved.kind) {
        (
            ExprKind::Call {
                name,
                type_arguments,
                args,
            },
            R::BorrowPlace { operation, place },
        ) => {
            if !type_arguments.is_empty()
                || args.len() != 1
                || crate::byte_ops::by_id(operation.as_str()).map(|op| op.name())
                    != Some(name.as_str())
                || !operation_source_place(&args[0], place)
            {
                return Err(operation_sidecar_disagreement());
            }
            collect_operation_place_occurrences(
                program,
                &args[0],
                place,
                tokens,
                owner,
                declaration_index,
                declarations,
            )?;
        }
        (
            ExprKind::Call {
                name,
                type_arguments,
                args,
            },
            R::ByteRange {
                operation,
                source: base,
                start,
                end,
            },
        ) => {
            if name != crate::byte_ops::RANGE_NAME
                || operation.as_str() != crate::byte_ops::RANGE_ID
                || !type_arguments.is_empty()
                || args.len() != 3
            {
                return Err(operation_sidecar_disagreement());
            }
            for (source, resolved) in args
                .iter()
                .zip([base.as_ref(), start.as_ref(), end.as_ref()])
            {
                collect_operation_expr_occurrences(
                    program,
                    source,
                    resolved,
                    tokens,
                    owner,
                    declaration_index,
                    import_index,
                    declarations,
                    imports,
                )?;
            }
        }
        (
            ExprKind::Call {
                name,
                type_arguments,
                args,
            },
            R::Call {
                callee,
                type_arguments: resolved_types,
                args: resolved_args,
                ..
            },
        ) => {
            let span = find_identifier_token(tokens, name, source.span.start, source.span.end)?;
            push_bound_operation_occurrence(
                program,
                callee.as_str(),
                ModuleUseKind::Function,
                span,
                Some(owner),
                declaration_index,
                import_index,
                declarations,
                imports,
            )?;
            if type_arguments.len() != resolved_types.len() || args.len() != resolved_args.len() {
                return Err(operation_sidecar_disagreement());
            }
            let mut cursor = span.end;
            for (ty, resolved_ty) in type_arguments.iter().zip(resolved_types) {
                collect_operation_type_occurrences(
                    program,
                    ty,
                    resolved_ty,
                    tokens,
                    &mut cursor,
                    source.span.end,
                    Some(owner),
                    declaration_index,
                    import_index,
                    declarations,
                    imports,
                )?;
            }
            for (child, resolved_child) in args.iter().zip(resolved_args) {
                collect_operation_expr_occurrences(
                    program,
                    child,
                    resolved_child,
                    tokens,
                    owner,
                    declaration_index,
                    import_index,
                    declarations,
                    imports,
                )?;
            }
        }
        (ExprKind::Unary { value, .. }, R::Unary { value: right, .. })
        | (ExprKind::Try { operand: value }, R::Try { operand: right, .. })
        | (ExprKind::Try { operand: value }, R::TryOption { operand: right, .. }) => {
            collect_operation_expr_occurrences(
                program,
                value,
                right,
                tokens,
                owner,
                declaration_index,
                import_index,
                declarations,
                imports,
            )?;
        }
        (
            ExprKind::Binary { left, right, .. },
            R::Binary {
                left: resolved_left,
                right: resolved_right,
                ..
            },
        ) => {
            for (child, resolved_child) in [
                (left.as_ref(), resolved_left.as_ref()),
                (right, resolved_right),
            ] {
                collect_operation_expr_occurrences(
                    program,
                    child,
                    resolved_child,
                    tokens,
                    owner,
                    declaration_index,
                    import_index,
                    declarations,
                    imports,
                )?;
            }
        }
        (
            ExprKind::Block { statements, tail },
            R::Block {
                statements: resolved_statements,
                tail: resolved_tail,
            },
        ) => {
            if statements.len() != resolved_statements.len() {
                return Err(operation_sidecar_disagreement());
            }
            for (statement, resolved_statement) in statements.iter().zip(resolved_statements) {
                if let (
                    crate::ast::Statement::Assign {
                        field: source_field,
                        ..
                    },
                    hir::ResolvedStatement::Assign {
                        field: resolved_field,
                        ..
                    },
                ) = (statement, resolved_statement)
                {
                    match (source_field, resolved_field) {
                        (Some(field), Some(id)) => push_member_operation_occurrence(
                            program,
                            id.as_str(),
                            field.span,
                            tokens,
                            owner,
                            None,
                            declaration_index,
                            declarations,
                        )?,
                        (None, None) => {}
                        _ => return Err(operation_sidecar_disagreement()),
                    }
                }
                if let (
                    crate::ast::Statement::Let {
                        declared: Some(ty),
                        name_span,
                        value,
                        ..
                    },
                    hir::ResolvedStatement::Let { binding, .. },
                ) = (statement, resolved_statement)
                {
                    let mut cursor = name_span.end;
                    collect_operation_type_occurrences(
                        program,
                        ty,
                        &binding.ty,
                        tokens,
                        &mut cursor,
                        value.span.start,
                        Some(owner),
                        declaration_index,
                        import_index,
                        declarations,
                        imports,
                    )?;
                }
                let same_kind = matches!(
                    (statement, resolved_statement),
                    (
                        crate::ast::Statement::Let { .. },
                        hir::ResolvedStatement::Let { .. }
                    ) | (
                        crate::ast::Statement::Assign { .. },
                        hir::ResolvedStatement::Assign { .. }
                    ) | (
                        crate::ast::Statement::Unsafe { .. },
                        hir::ResolvedStatement::Unsafe { .. }
                    ) | (
                        crate::ast::Statement::While { .. },
                        hir::ResolvedStatement::While { .. }
                    )
                );
                if !same_kind || statement.child_count() != resolved_statement.child_count() {
                    return Err(operation_sidecar_disagreement());
                }
                for index in 0..statement.child_count() {
                    let value = statement
                        .child(index)
                        .ok_or_else(operation_sidecar_disagreement)?;
                    let resolved_value = resolved_statement
                        .child(index)
                        .ok_or_else(operation_sidecar_disagreement)?;
                    collect_operation_expr_occurrences(
                        program,
                        value,
                        resolved_value,
                        tokens,
                        owner,
                        declaration_index,
                        import_index,
                        declarations,
                        imports,
                    )?;
                }
            }
            collect_operation_expr_occurrences(
                program,
                tail,
                resolved_tail,
                tokens,
                owner,
                declaration_index,
                import_index,
                declarations,
                imports,
            )?;
        }
        (
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            },
            R::If {
                condition: rc,
                then_branch: rt,
                else_branch: re,
            },
        ) => {
            for (child, resolved_child) in [
                (condition.as_ref(), rc.as_ref()),
                (then_branch, rt),
                (else_branch, re),
            ] {
                collect_operation_expr_occurrences(
                    program,
                    child,
                    resolved_child,
                    tokens,
                    owner,
                    declaration_index,
                    import_index,
                    declarations,
                    imports,
                )?;
            }
        }
        (
            ExprKind::ConstructRecord {
                type_name,
                type_span,
                type_arguments,
                fields,
            },
            R::ConstructRecord {
                record,
                fields: resolved_fields,
            },
        ) => {
            push_bound_operation_occurrence(
                program,
                record.as_str(),
                ModuleUseKind::Type,
                *type_span,
                Some(owner),
                declaration_index,
                import_index,
                declarations,
                imports,
            )?;
            collect_operation_field_values(
                program,
                fields,
                resolved_fields,
                tokens,
                owner,
                declaration_index,
                import_index,
                declarations,
                imports,
            )?;
            if source_text_token(tokens, *type_span)? != type_name {
                return Err(operation_sidecar_disagreement());
            }
            collect_constructor_type_arguments(
                program,
                type_arguments,
                &resolved.ty,
                tokens,
                type_span.end,
                source.span.end,
                owner,
                declaration_index,
                import_index,
                declarations,
                imports,
            )?;
        }
        (
            ExprKind::ConstructVariant {
                type_name,
                type_span,
                type_arguments,
                case_span,
                fields,
                ..
            },
            R::ConstructVariant {
                variant,
                case,
                fields: resolved_fields,
                ..
            },
        ) => {
            push_member_operation_occurrence(
                program,
                case.as_str(),
                *case_span,
                tokens,
                owner,
                None,
                declaration_index,
                declarations,
            )?;
            push_bound_operation_occurrence(
                program,
                variant.as_str(),
                ModuleUseKind::Type,
                *type_span,
                Some(owner),
                declaration_index,
                import_index,
                declarations,
                imports,
            )?;
            collect_operation_field_values(
                program,
                fields,
                resolved_fields,
                tokens,
                owner,
                declaration_index,
                import_index,
                declarations,
                imports,
            )?;
            if source_text_token(tokens, *type_span)? != type_name {
                return Err(operation_sidecar_disagreement());
            }
            collect_constructor_type_arguments(
                program,
                type_arguments,
                &resolved.ty,
                tokens,
                type_span.end,
                source.span.end,
                owner,
                declaration_index,
                import_index,
                declarations,
                imports,
            )?;
        }
        (
            ExprKind::Match {
                scrutinee, arms, ..
            },
            R::Match {
                scrutinee: resolved_scrutinee,
                arms: resolved_arms,
                ..
            },
        ) => {
            if arms.len() != resolved_arms.len() {
                return Err(operation_sidecar_disagreement());
            }
            collect_operation_expr_occurrences(
                program,
                scrutinee,
                resolved_scrutinee,
                tokens,
                owner,
                declaration_index,
                import_index,
                declarations,
                imports,
            )?;
            for (arm, resolved_arm) in arms.iter().zip(resolved_arms) {
                match (&arm.guard, &resolved_arm.guard) {
                    (None, None) => {}
                    (Some(source), Some(resolved)) => collect_operation_expr_occurrences(
                        program,
                        source,
                        resolved,
                        tokens,
                        owner,
                        declaration_index,
                        import_index,
                        declarations,
                        imports,
                    )?,
                    _ => return Err(operation_sidecar_disagreement()),
                }
                collect_operation_pattern_occurrences(
                    program,
                    &arm.pattern,
                    &resolved_arm.pattern,
                    tokens,
                    owner,
                    declaration_index,
                    import_index,
                    declarations,
                    imports,
                )?;
                collect_operation_expr_occurrences(
                    program,
                    &arm.value,
                    &resolved_arm.value,
                    tokens,
                    owner,
                    declaration_index,
                    import_index,
                    declarations,
                    imports,
                )?;
            }
        }
        (
            ExprKind::UpdateRecord { base, fields },
            R::UpdateRecord {
                base: resolved_base,
                fields: resolved_fields,
                ..
            },
        ) => {
            collect_operation_expr_occurrences(
                program,
                base,
                resolved_base,
                tokens,
                owner,
                declaration_index,
                import_index,
                declarations,
                imports,
            )?;
            collect_operation_field_values(
                program,
                fields,
                resolved_fields,
                tokens,
                owner,
                declaration_index,
                import_index,
                declarations,
                imports,
            )?;
        }
        (
            ExprKind::Project {
                base, field_span, ..
            },
            R::Project {
                base: resolved_base,
                field,
                ..
            },
        ) => {
            push_member_operation_occurrence(
                program,
                field.as_str(),
                *field_span,
                tokens,
                owner,
                None,
                declaration_index,
                declarations,
            )?;
            collect_operation_expr_occurrences(
                program,
                base,
                resolved_base,
                tokens,
                owner,
                declaration_index,
                import_index,
                declarations,
                imports,
            )?;
        }
        (ExprKind::Project { .. } | ExprKind::Var(_), R::Place(place))
            if operation_source_place(source, place) =>
        {
            collect_operation_place_occurrences(
                program,
                source,
                place,
                tokens,
                owner,
                declaration_index,
                declarations,
            )?;
        }
        (ExprKind::Int(_), R::Int(_))
        | (ExprKind::Int32(_), R::Int32(_))
        | (ExprKind::Char(_), R::Char(_))
        | (ExprKind::Uint8(_), R::Uint8(_))
        | (ExprKind::Usize(_), R::Usize(_))
        | (ExprKind::ArrayU8(_), R::ArrayU8(_))
        | (ExprKind::RepeatArrayU8 { .. }, R::RepeatArrayU8 { .. })
        | (ExprKind::Float32(_), R::Float32(_))
        | (ExprKind::Float64(_), R::Float64(_))
        | (ExprKind::String(_), R::String(_))
        | (ExprKind::Bool(_), R::Bool(_)) => {}
        _ => return Err(operation_sidecar_disagreement()),
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn collect_operation_field_values(
    program: &Program,
    fields: &[crate::ast::FieldInitializer],
    resolved_fields: &[hir::ResolvedFieldInitializer],
    tokens: &[crate::lexer::Token],
    owner: &str,
    declaration_index: &BTreeMap<String, usize>,
    import_index: &BTreeMap<(String, ModuleUseKind, String, String), usize>,
    declarations: &mut [WorkspaceOperationDeclaration],
    imports: &mut [WorkspaceOperationImport],
) -> Result<(), Vec<Diagnostic>> {
    if fields.len() != resolved_fields.len() {
        return Err(operation_sidecar_disagreement());
    }
    for (field, resolved) in fields.iter().zip(resolved_fields) {
        push_member_operation_occurrence(
            program,
            resolved.field.as_str(),
            field.name_span,
            tokens,
            owner,
            None,
            declaration_index,
            declarations,
        )?;
        collect_operation_expr_occurrences(
            program,
            &field.value,
            &resolved.value,
            tokens,
            owner,
            declaration_index,
            import_index,
            declarations,
            imports,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn collect_constructor_type_arguments(
    program: &Program,
    arguments: &[Type],
    resolved: &hir::ResolvedType,
    tokens: &[crate::lexer::Token],
    start: usize,
    end: usize,
    owner: &str,
    declaration_index: &BTreeMap<String, usize>,
    import_index: &BTreeMap<(String, ModuleUseKind, String, String), usize>,
    declarations: &mut [WorkspaceOperationDeclaration],
    imports: &mut [WorkspaceOperationImport],
) -> Result<(), Vec<Diagnostic>> {
    let resolved_arguments = match resolved {
        hir::ResolvedType::Nominal { arguments, .. } => arguments.as_slice(),
        _ if arguments.is_empty() => return Ok(()),
        _ => return Err(operation_sidecar_disagreement()),
    };
    if arguments.len() != resolved_arguments.len() {
        return Err(operation_sidecar_disagreement());
    }
    let mut cursor = start;
    for (argument, resolved_argument) in arguments.iter().zip(resolved_arguments) {
        collect_operation_type_occurrences(
            program,
            argument,
            resolved_argument,
            tokens,
            &mut cursor,
            end,
            Some(owner),
            declaration_index,
            import_index,
            declarations,
            imports,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn collect_operation_pattern_occurrences(
    program: &Program,
    source: &crate::ast::MatchPattern,
    resolved: &hir::ResolvedMatchPattern,
    tokens: &[crate::lexer::Token],
    owner: &str,
    declaration_index: &BTreeMap<String, usize>,
    import_index: &BTreeMap<(String, ModuleUseKind, String, String), usize>,
    declarations: &mut [WorkspaceOperationDeclaration],
    imports: &mut [WorkspaceOperationImport],
) -> Result<(), Vec<Diagnostic>> {
    match (source, resolved) {
        (
            crate::ast::MatchPattern::Variant {
                type_name,
                type_span,
                case_span,
                fields,
                ..
            },
            hir::ResolvedMatchPattern::Variant {
                variant,
                case,
                fields: resolved_fields,
            },
        ) => {
            push_member_operation_occurrence(
                program,
                case.as_str(),
                *case_span,
                tokens,
                owner,
                None,
                declaration_index,
                declarations,
            )?;
            if fields.len() != resolved_fields.len() {
                return Err(operation_sidecar_disagreement());
            }
            for (field, resolved_field) in fields.iter().zip(resolved_fields) {
                if field.binding != resolved_field.binding.name {
                    return Err(operation_sidecar_disagreement());
                }
                let shorthand =
                    (field.name_span == field.binding_span).then_some(field.binding.as_str());
                push_member_operation_occurrence(
                    program,
                    resolved_field.field.as_str(),
                    field.name_span,
                    tokens,
                    owner,
                    shorthand,
                    declaration_index,
                    declarations,
                )?;
            }
            if source_text_token(tokens, *type_span)? != type_name {
                return Err(operation_sidecar_disagreement());
            }
            push_bound_operation_occurrence(
                program,
                variant.as_str(),
                ModuleUseKind::Type,
                *type_span,
                Some(owner),
                declaration_index,
                import_index,
                declarations,
                imports,
            )?;
        }
        (
            crate::ast::MatchPattern::Record {
                type_name,
                type_span,
                fields,
                ..
            },
            hir::ResolvedMatchPattern::Record {
                record,
                fields: resolved_fields,
                ..
            },
        ) => {
            if source_text_token(tokens, *type_span)? != type_name {
                return Err(operation_sidecar_disagreement());
            }
            push_bound_operation_occurrence(
                program,
                record.as_str(),
                ModuleUseKind::Type,
                *type_span,
                Some(owner),
                declaration_index,
                import_index,
                declarations,
                imports,
            )?;
            collect_nested_record_pattern_occurrences(
                program,
                fields,
                resolved_fields,
                tokens,
                owner,
                declaration_index,
                import_index,
                declarations,
                imports,
            )?;
        }
        (crate::ast::MatchPattern::Wildcard { .. }, hir::ResolvedMatchPattern::Wildcard) => {}
        (
            crate::ast::MatchPattern::Binding { name, .. },
            hir::ResolvedMatchPattern::Binding(binding),
        ) if name == &binding.name => {}
        (
            crate::ast::MatchPattern::Literal { value, .. },
            hir::ResolvedMatchPattern::Literal(actual),
        ) if hir::PatternValue::from_ast(*value) == *actual => {}
        (
            crate::ast::MatchPattern::Or { alternatives, .. },
            hir::ResolvedMatchPattern::Or(actual),
        ) if alternatives.len() == actual.len() => {
            for (source, resolved) in alternatives.iter().zip(actual) {
                collect_operation_pattern_occurrences(
                    program,
                    source,
                    resolved,
                    tokens,
                    owner,
                    declaration_index,
                    import_index,
                    declarations,
                    imports,
                )?;
            }
        }
        _ => return Err(operation_sidecar_disagreement()),
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn collect_nested_record_pattern_occurrences(
    program: &Program,
    fields: &[crate::ast::RecordMatchPatternField],
    resolved_fields: &[hir::ResolvedRecordMatchPatternField],
    tokens: &[crate::lexer::Token],
    owner: &str,
    declaration_index: &BTreeMap<String, usize>,
    import_index: &BTreeMap<(String, ModuleUseKind, String, String), usize>,
    declarations: &mut [WorkspaceOperationDeclaration],
    imports: &mut [WorkspaceOperationImport],
) -> Result<(), Vec<Diagnostic>> {
    if fields.len() != resolved_fields.len() {
        return Err(operation_sidecar_disagreement());
    }
    for (field, resolved_field) in fields.iter().zip(resolved_fields) {
        let shorthand = match &field.pattern {
            crate::ast::RecordMatchFieldPattern::Binding { name, span }
                if *span == field.name_span =>
            {
                Some(name.as_str())
            }
            _ => None,
        };
        push_member_operation_occurrence(
            program,
            resolved_field.field.as_str(),
            field.name_span,
            tokens,
            owner,
            shorthand,
            declaration_index,
            declarations,
        )?;
        match (&field.pattern, &resolved_field.pattern) {
            (
                crate::ast::RecordMatchFieldPattern::Record {
                    type_name,
                    type_span,
                    fields,
                    ..
                },
                hir::ResolvedRecordMatchFieldPattern::Record {
                    record,
                    fields: resolved_fields,
                    ..
                },
            ) => {
                if source_text_token(tokens, *type_span)? != type_name {
                    return Err(operation_sidecar_disagreement());
                }
                push_bound_operation_occurrence(
                    program,
                    record.as_str(),
                    ModuleUseKind::Type,
                    *type_span,
                    Some(owner),
                    declaration_index,
                    import_index,
                    declarations,
                    imports,
                )?;
                collect_nested_record_pattern_occurrences(
                    program,
                    fields,
                    resolved_fields,
                    tokens,
                    owner,
                    declaration_index,
                    import_index,
                    declarations,
                    imports,
                )?;
            }
            (
                crate::ast::RecordMatchFieldPattern::Binding { name, .. },
                hir::ResolvedRecordMatchFieldPattern::Binding(binding),
            ) if name == &binding.name => {}
            (
                crate::ast::RecordMatchFieldPattern::Wildcard { .. },
                hir::ResolvedRecordMatchFieldPattern::Wildcard,
            ) => {}
            _ => return Err(operation_sidecar_disagreement()),
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn push_bound_operation_occurrence(
    program: &Program,
    target_id: &str,
    family: ModuleUseKind,
    span: Span,
    owner: Option<&str>,
    declaration_index: &BTreeMap<String, usize>,
    _import_index: &BTreeMap<(String, ModuleUseKind, String, String), usize>,
    declarations: &mut [WorkspaceOperationDeclaration],
    imports: &mut [WorkspaceOperationImport],
) -> Result<(), Vec<Diagnostic>> {
    reserve_builder_structure(std::mem::size_of::<WorkspaceOperationOccurrence>())?;
    let occurrence = WorkspaceOperationOccurrence {
        path: crate::bounded_output::budgeted_clone(&program.path),
        span,
        owner: owner.map(crate::bounded_output::budgeted_clone),
        shorthand_binding: None,
    };
    let family_text = match family {
        ModuleUseKind::Function => "function",
        ModuleUseKind::Type => "type",
        ModuleUseKind::Protocol => "protocol",
    };
    if let Ok(index) = imports.binary_search_by(|item| {
        (&item.path, item.kind, item.target_id.as_str()).cmp(&(
            &program.path,
            family_text,
            target_id,
        ))
    }) {
        imports[index].occurrences.push(occurrence);
    } else if let Some(index) = declaration_index.get(target_id).copied() {
        if declarations[index].path == program.path {
            declarations[index].occurrences.push(occurrence);
        } else {
            return Err(operation_sidecar_disagreement());
        }
    } else if !(family == ModuleUseKind::Type
        && matches!(
            target_id,
            crate::prelude::OPTION_ID | crate::prelude::RESULT_ID
        )
        || family == ModuleUseKind::Function
            && (crate::byte_ops::by_id(target_id).is_some()
                || crate::string_ops::by_id(target_id).is_some()
                || crate::str_ops::by_id(target_id).is_some()
                || crate::host_io_ops::by_id(target_id).is_some()))
    {
        return Err(operation_sidecar_disagreement());
    }
    Ok(())
}

// A resolved place contains no evaluated subtree. Only a lexical root and
// field projections can correspond to it; never discard a constructor/call.
fn operation_source_place(source: &Expr, place: &hir::Place) -> bool {
    let mut source = source;
    let mut fields = 0usize;
    while let ExprKind::Project { base, .. } = &source.kind {
        source = base;
        fields += 1;
    }
    matches!(&source.kind, ExprKind::Var(_)) && fields == place.projections.len()
}

#[allow(clippy::too_many_arguments)]
fn collect_operation_place_occurrences(
    program: &Program,
    source: &Expr,
    place: &hir::Place,
    tokens: &[crate::lexer::Token],
    owner: &str,
    index: &BTreeMap<String, usize>,
    declarations: &mut [WorkspaceOperationDeclaration],
) -> Result<(), Vec<Diagnostic>> {
    if !operation_source_place(source, place) {
        return Err(operation_sidecar_disagreement());
    }
    let mut source = source;
    for projection in place.projections.iter().rev() {
        let ExprKind::Project {
            base, field_span, ..
        } = &source.kind
        else {
            return Err(operation_sidecar_disagreement());
        };
        let hir::PlaceProjection::Field(field) = projection else {
            return Err(operation_sidecar_disagreement());
        };
        push_member_operation_occurrence(
            program,
            field.as_str(),
            *field_span,
            tokens,
            owner,
            None,
            index,
            declarations,
        )?;
        source = base;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn push_member_operation_occurrence(
    program: &Program,
    target: &str,
    span: Span,
    tokens: &[crate::lexer::Token],
    owner: &str,
    shorthand: Option<&str>,
    index: &BTreeMap<String, usize>,
    declarations: &mut [WorkspaceOperationDeclaration],
) -> Result<(), Vec<Diagnostic>> {
    let Some(position) = index.get(target).copied() else {
        // Compiler-owned members have no source declaration or rename authority.
        return if matches!(
            target,
            crate::prelude::OPTION_NONE_ID
                | crate::prelude::OPTION_SOME_ID
                | crate::prelude::OPTION_SOME_VALUE_ID
                | crate::prelude::RESULT_OK_ID
                | crate::prelude::RESULT_OK_VALUE_ID
                | crate::prelude::RESULT_ERR_ID
                | crate::prelude::RESULT_ERR_ERROR_ID
        ) {
            Ok(())
        } else {
            Err(operation_sidecar_disagreement())
        };
    };
    let declaration = &mut declarations[position];
    if declaration.namespace_owner.is_none() || source_text_token(tokens, span)? != declaration.name
    {
        return Err(operation_sidecar_disagreement());
    }
    reserve_builder_structure(std::mem::size_of::<WorkspaceOperationOccurrence>())?;
    declaration.occurrences.push(WorkspaceOperationOccurrence {
        path: crate::bounded_output::budgeted_clone(&program.path),
        span,
        owner: Some(crate::bounded_output::budgeted_clone(owner)),
        shorthand_binding: shorthand.map(crate::bounded_output::budgeted_clone),
    });
    Ok(())
}

fn find_identifier_token(
    tokens: &[crate::lexer::Token],
    name: &str,
    start: usize,
    end: usize,
) -> Result<Span, Vec<Diagnostic>> {
    let first = tokens.partition_point(|token| token.span.end <= start);
    tokens[first..]
        .iter()
        .take_while(|token| token.span.start < end)
        .find(|token| {
            token.span.start >= start
                && token.span.end <= end
                && matches!(&token.kind, crate::lexer::TokenKind::Ident(value) if value == name)
        })
        .map(|token| token.span)
        .ok_or_else(operation_sidecar_disagreement)
}

fn source_text_token(tokens: &[crate::lexer::Token], span: Span) -> Result<&str, Vec<Diagnostic>> {
    tokens
        .binary_search_by_key(&span.start, |token| token.span.start)
        .ok()
        .and_then(|index| tokens.get(index))
        .filter(|token| token.span == span)
        .and_then(|token| match &token.kind {
            crate::lexer::TokenKind::Ident(value) => Some(value.as_str()),
            _ => None,
        })
        .ok_or_else(operation_sidecar_disagreement)
}

fn module_use_alias_span(
    tokens: &[crate::lexer::Token],
    module_use: &crate::ast::ModuleUse,
) -> Result<Span, Vec<Diagnostic>> {
    let first = tokens.partition_point(|token| token.span.end <= module_use.span.start);
    let scoped =
        &tokens[first..tokens.partition_point(|token| token.span.start < module_use.span.end)];
    let mut meaningful = scoped
        .iter()
        .filter(|token| !matches!(token.kind, crate::lexer::TokenKind::Eof));
    let semicolon = meaningful
        .next_back()
        .ok_or_else(operation_sidecar_disagreement)?;
    let alias = meaningful
        .next_back()
        .ok_or_else(operation_sidecar_disagreement)?;
    let keyword = meaningful
        .next_back()
        .ok_or_else(operation_sidecar_disagreement)?;
    match (&keyword.kind, &alias.kind, &semicolon.kind) {
        (
            crate::lexer::TokenKind::Ident(as_keyword),
            crate::lexer::TokenKind::Ident(alias_name),
            crate::lexer::TokenKind::Semicolon,
        ) if as_keyword == "as" && alias_name == &module_use.alias => Ok(alias.span),
        _ => Err(operation_sidecar_disagreement()),
    }
}

fn operation_sidecar_disagreement() -> Vec<Diagnostic> {
    vec![graph_error(
        "SPX-G173",
        "workspace operations AST/HIR occurrence proof disagrees",
    )]
}
