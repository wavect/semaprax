//! Narrow Project linking support for flat generic owned-record templates.

use std::collections::BTreeSet;

use crate::ast::{Program, Type, TypeDeclaration, TypeDeclarationKind};
use crate::diagnostic::Diagnostic;

use super::expected_projection::cost::StructuralCost;

pub(super) fn template_is_admitted(declaration: &TypeDeclaration) -> bool {
    let parameters = parameter_names(declaration);
    let TypeDeclarationKind::Record { fields } = &declaration.kind else {
        return false;
    };
    !parameters.is_empty()
        && !fields.is_empty()
        && fields.iter().all(|field| match &field.ty {
            Type::I64
            | Type::I32
            | Type::Char
            | Type::U8
            | Type::Usize
            | Type::F32
            | Type::F64
            | Type::Bool
            | Type::Bytes => true,
            Type::Named { name, arguments } => {
                arguments.is_empty() && parameters.contains(name.as_str())
            }
            Type::String | Type::Str | Type::SliceU8 | Type::ArrayU8(_) => false,
        })
}

pub(super) fn rewrite_declaration_runtime_cost(
    declaration: &TypeDeclaration,
    target_module: &str,
    caller: &Program,
    programs: &[Program],
    cost: &mut StructuralCost,
) -> Result<(), Vec<Diagnostic>> {
    let parameters = parameter_names(declaration);
    for ty in declaration_types(declaration) {
        if !is_parameter(ty, &parameters) {
            super::expected_projection::rewrite_type_runtime_cost(
                ty,
                target_module,
                caller,
                programs,
                cost,
            )?;
        }
    }
    Ok(())
}

pub(super) fn rewrite_declaration(
    declaration: &mut TypeDeclaration,
    target_module: &str,
    caller: &Program,
    programs: &[Program],
) -> Result<(), Vec<Diagnostic>> {
    let parameters = parameter_names(declaration);
    for ty in declaration_types_mut(declaration) {
        if !is_parameter(ty, &parameters) {
            super::expected_projection::rewrite_type(ty, target_module, caller, programs)?;
        }
    }
    Ok(())
}

fn parameter_names(declaration: &TypeDeclaration) -> BTreeSet<String> {
    declaration
        .type_parameters
        .iter()
        .map(|parameter| parameter.name.clone())
        .collect()
}

fn is_parameter(ty: &Type, parameters: &BTreeSet<String>) -> bool {
    matches!(ty, Type::Named { name, arguments }
        if arguments.is_empty() && parameters.contains(name.as_str()))
}

fn declaration_types(declaration: &TypeDeclaration) -> Vec<&Type> {
    match &declaration.kind {
        TypeDeclarationKind::Record { fields } => fields.iter().map(|field| &field.ty).collect(),
        TypeDeclarationKind::Class { fields, methods } => fields
            .iter()
            .map(|field| &field.ty)
            .chain(methods.iter().flat_map(|method| {
                method
                    .params
                    .iter()
                    .map(|parameter| &parameter.ty)
                    .chain(std::iter::once(&method.return_type))
            }))
            .collect(),
        TypeDeclarationKind::Variant { cases } => cases
            .iter()
            .flat_map(|case| case.fields.iter().map(|field| &field.ty))
            .collect(),
        TypeDeclarationKind::Resource { .. } => unreachable!("resource imports are rejected"),
    }
}

fn declaration_types_mut(declaration: &mut TypeDeclaration) -> Vec<&mut Type> {
    match &mut declaration.kind {
        TypeDeclarationKind::Record { fields } => {
            fields.iter_mut().map(|field| &mut field.ty).collect()
        }
        TypeDeclarationKind::Class { fields, methods } => fields
            .iter_mut()
            .map(|field| &mut field.ty)
            .chain(methods.iter_mut().flat_map(|method| {
                method
                    .params
                    .iter_mut()
                    .map(|parameter| &mut parameter.ty)
                    .chain(std::iter::once(&mut method.return_type))
            }))
            .collect(),
        TypeDeclarationKind::Variant { cases } => cases
            .iter_mut()
            .flat_map(|case| case.fields.iter_mut().map(|field| &mut field.ty))
            .collect(),
        TypeDeclarationKind::Resource { .. } => unreachable!("resource imports are rejected"),
    }
}
