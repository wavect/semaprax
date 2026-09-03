//! Class Inheritance v1 checks: method declarations, declared parents,
//! inheritance cycles, and member collision and override validation.

use crate::ast::{Program, Type, TypeDeclarationKind};
use crate::diagnostic::Diagnostic;
use crate::source_verify::declared_type::check_declared_type;
use crate::source_verify::diagnostics::{
    error, invalid_stable_id, reject_reserved_host_id, source_identifier,
};
use crate::source_verify::type_table::TypeTable;
use std::collections::{HashMap, HashSet};

pub(super) fn check_class_methods<'p>(
    program: &'p Program,
    types: &TypeTable<'p>,
    ids: &mut HashSet<&'p str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for declaration in &program.types {
        let TypeDeclarationKind::Class { fields: _, methods } = &declaration.kind else {
            continue;
        };
        let mut method_names = HashSet::new();
        for method in methods {
            if !source_identifier(&method.name) {
                diagnostics.push(error(
                    program,
                    "SPX-S104",
                    format!("`{}` is not a valid method identifier", method.name),
                    method.name_span,
                ));
            }
            if !method_names.insert(method.name.as_str()) {
                diagnostics.push(error(
                    program,
                    "SPX-S101",
                    format!(
                        "duplicate method `{}` in class `{}`",
                        method.name, declaration.name
                    ),
                    method.span,
                ));
            }
            reject_reserved_host_id(
                program,
                &method.stable_id,
                "method",
                method.span,
                diagnostics,
            );
            if method.stable_id.contains('\0') {
                diagnostics.push(invalid_stable_id(
                    program,
                    "SPX-S102",
                    format!("method `{}.{}`", declaration.name, method.name),
                    method.span,
                ));
            } else if !ids.insert(method.stable_id.as_str()) {
                diagnostics.push(error(
                    program,
                    "SPX-S102",
                    format!("duplicate stable id `{}`", method.stable_id),
                    method.span,
                ));
            }
            if !method.explicit_id {
                diagnostics.push(
                    Diagnostic::warning(
                        "SPX-S103",
                        format!(
                            "method `{}.{}` has an automatic identity that changes when renamed",
                            declaration.name, method.name
                        ),
                        method.name_span,
                    )
                    .at_path(&program.path)
                    .with_help("add @id(\"your.namespace.class.method\") before the method"),
                );
            }
            if !method.type_parameters.is_empty() {
                diagnostics.push(error(
                    program,
                    "SPX-T224",
                    format!(
                        "class method `{}.{}` cannot declare generic parameters in this slice",
                        declaration.name, method.name
                    ),
                    method.type_parameters[0].span,
                ));
            }
            for param in &method.params {
                if !source_identifier(&param.name) {
                    diagnostics.push(error(
                        program,
                        "SPX-S105",
                        format!("`{}` is not a valid parameter identifier", param.name),
                        param.span,
                    ));
                }
                check_declared_type(
                    program,
                    &param.ty,
                    param.span,
                    types,
                    &HashSet::new(),
                    diagnostics,
                );
            }
            check_declared_type(
                program,
                &method.return_type,
                method.span,
                types,
                &HashSet::new(),
                diagnostics,
            );
        }
    }
}

pub(super) fn check_class_parents<'p>(
    program: &'p Program,
    types: &TypeTable<'p>,
    class_parents: &mut HashMap<&'p str, &'p str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for declaration in &program.types {
        let TypeDeclarationKind::Class { .. } = &declaration.kind else {
            continue;
        };
        let Some(parent) = &declaration.extends else {
            continue;
        };
        let Type::Named {
            name: parent_name,
            arguments: parent_arguments,
        } = parent
        else {
            diagnostics.push(error(
                program,
                "SPX-T227",
                format!(
                    "class `{}` must extend a named class type",
                    declaration.name
                ),
                declaration.name_span,
            ));
            continue;
        };
        if !parent_arguments.is_empty() {
            diagnostics.push(error(
                program,
                "SPX-T227",
                format!(
                    "class `{}` extends generic type `{parent_name}`; inheritance over generic classes is closed in this slice",
                    declaration.name
                ),
                declaration.name_span,
            ));
            continue;
        }
        match types.declaration(parent_name) {
            None => {
                diagnostics.push(error(
                    program,
                    "SPX-T227",
                    format!(
                        "class `{}` extends unknown type `{parent_name}`",
                        declaration.name
                    ),
                    declaration.name_span,
                ));
            }
            Some(parent_declaration) => {
                if !matches!(parent_declaration.kind, TypeDeclarationKind::Class { .. }) {
                    diagnostics.push(error(
                        program,
                        "SPX-T227",
                        format!(
                            "class `{}` extends non-class type `{parent_name}`",
                            declaration.name
                        ),
                        declaration.name_span,
                    ));
                } else {
                    class_parents.insert(declaration.name.as_str(), parent_name.as_str());
                }
            }
        }
    }
}

pub(super) fn check_class_cycles<'p>(
    program: &'p Program,
    class_parents: &HashMap<&'p str, &'p str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for declaration in &program.types {
        if !matches!(declaration.kind, TypeDeclarationKind::Class { .. }) {
            continue;
        }
        let mut visited = HashSet::new();
        let mut cursor = class_parents.get(declaration.name.as_str()).copied();
        while let Some(parent_name) = cursor {
            if parent_name == declaration.name.as_str() || !visited.insert(parent_name) {
                diagnostics.push(error(
                    program,
                    "SPX-T228",
                    format!(
                        "class `{}` participates in an inheritance cycle",
                        declaration.name
                    ),
                    declaration.span,
                ));
                break;
            }
            cursor = class_parents.get(parent_name).copied();
        }
    }
}

pub(super) fn check_class_overrides<'p>(
    program: &'p Program,
    types: &TypeTable<'p>,
    class_parents: &HashMap<&'p str, &'p str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for declaration in &program.types {
        let TypeDeclarationKind::Class {
            fields: own_fields,
            methods: own_methods,
        } = &declaration.kind
        else {
            continue;
        };
        // Walk root-first so the nearest ancestor's members shadow outer ones.
        let mut chain = Vec::new();
        let mut visited = HashSet::new();
        let mut cursor = class_parents.get(declaration.name.as_str()).copied();
        while let Some(parent_name) = cursor {
            if !visited.insert(parent_name) {
                break;
            }
            chain.push(parent_name);
            cursor = class_parents.get(parent_name).copied();
        }
        let mut ancestor_members: Vec<(&str, bool)> = Vec::new();
        for ancestor_name in chain.iter().rev() {
            let Some(ancestor) = types.declaration(ancestor_name) else {
                continue;
            };
            let TypeDeclarationKind::Class {
                fields, methods, ..
            } = &ancestor.kind
            else {
                continue;
            };
            for field in fields {
                ancestor_members.push((field.name.as_str(), true));
            }
            for method in methods {
                ancestor_members.push((method.name.as_str(), false));
            }
        }
        for field in own_fields {
            if ancestor_members.iter().any(|(name, _)| *name == field.name) {
                diagnostics.push(error(
                    program,
                    "SPX-T229",
                    format!(
                        "class `{}` redeclares member `{}` from an ancestor",
                        declaration.name, field.name
                    ),
                    field.span,
                ));
            }
        }
        for method in own_methods {
            if ancestor_members
                .iter()
                .any(|(name, is_field)| *name == method.name && *is_field)
            {
                diagnostics.push(error(
                    program,
                    "SPX-T229",
                    format!(
                        "class `{}` redeclares member `{}` from an ancestor",
                        declaration.name, method.name
                    ),
                    method.span,
                ));
                continue;
            }
            // Nearest ancestor declaring the same method name defines the
            // override contract; the non-self signature must match exactly.
            for ancestor_name in chain.iter().rev() {
                let Some(ancestor) = types.declaration(ancestor_name) else {
                    continue;
                };
                let TypeDeclarationKind::Class { methods, .. } = &ancestor.kind else {
                    continue;
                };
                let Some(overridden) = methods
                    .iter()
                    .find(|candidate| candidate.name == method.name)
                else {
                    continue;
                };
                let same_signature = !overridden.params.is_empty()
                    && overridden.params.len() == method.params.len()
                    && overridden.params[1..]
                        .iter()
                        .zip(method.params[1..].iter())
                        .all(|(base, over)| base.mode == over.mode && base.ty == over.ty)
                    && overridden.return_type == method.return_type;
                if !same_signature {
                    diagnostics.push(error(
                        program,
                        "SPX-T230",
                        format!(
                            "override `{}.{}` does not exactly match `{}.{}`",
                            declaration.name, method.name, ancestor_name, method.name
                        ),
                        method.span,
                    ));
                }
                break;
            }
        }
    }
}
