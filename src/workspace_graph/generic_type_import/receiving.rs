//! Actual imported declaration rewriting, with original receiver custody.
//! Full previous declaration and parameter-name receivers stay independent.
use super::parameter_names::{self, BorrowedNames};
use crate::ast::{Program, Type, TypeDeclaration, TypeDeclarationKind};
use crate::workspace_graph::constructor_failure::{receiving::Receiver, typed::Failure};
use crate::workspace_graph::expected_projection::{private_layout::LayoutClosure, type_rewrite};
use std::mem::{size_of, size_of_val};

fn one(
    ty: &mut Type,
    parameters: &BorrowedNames<'_>,
    target_module: &str,
    caller: &Program,
    programs: &[Program],
    owner: &mut Receiver,
) -> Result<(), Failure> {
    owner.entry::<()>()?;
    owner.reserve(
        size_of::<&mut Type>()
            + size_of::<&BorrowedNames<'_>>()
            + size_of::<&str>()
            + size_of::<&Program>()
            + size_of::<&[Program]>(),
    )?;
    if !matches!(&*ty, Type::Named { name, arguments } if arguments.is_empty() && parameters.contains(name))
    {
        type_rewrite::receiving::rewrite_with_layout(
            ty,
            target_module,
            caller,
            programs,
            None,
            owner,
        )?;
    }
    Ok(())
}
fn ordinary(
    declaration: &mut TypeDeclaration,
    target_module: &str,
    caller: &Program,
    programs: &[Program],
    owner: &mut Receiver,
) -> Result<(), Failure> {
    owner.entry::<()>()?;
    owner.reserve(
        size_of::<&mut TypeDeclaration>()
            + size_of::<&str>()
            + size_of::<&Program>()
            + size_of::<&[Program]>(),
    )?;
    let parameters = parameter_names::receiving::new(&declaration.type_parameters, owner)?;
    match &mut declaration.kind {
        TypeDeclarationKind::Record { fields } => {
            for field in fields {
                one(
                    &mut field.ty,
                    &parameters,
                    target_module,
                    caller,
                    programs,
                    owner,
                )?;
            }
        }
        TypeDeclarationKind::Class { fields, methods } => {
            for field in fields {
                one(
                    &mut field.ty,
                    &parameters,
                    target_module,
                    caller,
                    programs,
                    owner,
                )?;
            }
            for method in methods {
                for parameter in &mut method.params {
                    one(
                        &mut parameter.ty,
                        &parameters,
                        target_module,
                        caller,
                        programs,
                        owner,
                    )?;
                }
                one(
                    &mut method.return_type,
                    &parameters,
                    target_module,
                    caller,
                    programs,
                    owner,
                )?;
            }
        }
        TypeDeclarationKind::Variant { cases } => {
            for case in cases {
                for field in &mut case.fields {
                    one(
                        &mut field.ty,
                        &parameters,
                        target_module,
                        caller,
                        programs,
                        owner,
                    )?;
                }
            }
        }
        TypeDeclarationKind::Resource { .. } => unreachable!("resource imports are rejected"),
    }
    Ok(())
}
pub(in crate::workspace_graph) fn rewrite_declaration_with_layout(
    declaration: &mut TypeDeclaration,
    target_module: &str,
    caller: &Program,
    programs: &[Program],
    layout: Option<&LayoutClosure<'_>>,
    owner: &mut Receiver,
) -> Result<(), Failure> {
    #[cfg(test)]
    if super::use_original_receiver(4) {
        // The full independently compiled old declaration route remains the
        // actual owning-test baseline, including its original helper charges.
        return super::original::rewrite_declaration_with_layout(
            declaration,
            target_module,
            caller,
            programs,
            layout,
        )
        .map_err(Failure::from);
    }
    owner.entry::<()>()?;
    owner.reserve(
        size_of::<&mut TypeDeclaration>()
            + size_of::<&str>()
            + size_of::<&Program>()
            + size_of::<&[Program]>()
            + size_of_val(&layout),
    )?;
    if layout.is_none_or(|l| l.is_empty()) {
        return ordinary(declaration, target_module, caller, programs, owner);
    }
    if !declaration.type_parameters.is_empty()
        || matches!(
            declaration.kind,
            TypeDeclarationKind::Class { .. } | TypeDeclarationKind::Resource { .. }
        )
    {
        return ordinary(declaration, target_module, caller, programs, owner);
    }
    match &mut declaration.kind {
        TypeDeclarationKind::Record { fields } => {
            for field in fields {
                type_rewrite::receiving::rewrite_with_layout(
                    &mut field.ty,
                    target_module,
                    caller,
                    programs,
                    layout,
                    owner,
                )?;
            }
        }
        TypeDeclarationKind::Variant { cases } => {
            for case in cases {
                for field in &mut case.fields {
                    type_rewrite::receiving::rewrite_with_layout(
                        &mut field.ty,
                        target_module,
                        caller,
                        programs,
                        layout,
                        owner,
                    )?;
                }
            }
        }
        TypeDeclarationKind::Class { .. } | TypeDeclarationKind::Resource { .. } => {
            return Err(Failure::one(crate::diagnostic::Diagnostic::io_static(
                "SPX-G172",
                "private layout rewriting requires a monomorphic record or result variant",
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "receiving/tests.rs"]
mod tests;
