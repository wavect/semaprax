//! Narrow Project linking support for flat generic owned-record templates.

use crate::ast::{Program, Type, TypeDeclaration, TypeDeclarationKind};
use crate::diagnostic::Diagnostic;

use super::expected_projection::cost::StructuralCost;
#[path = "generic_type_import/parameter_names.rs"]
mod parameter_names;
use parameter_names::BorrowedNames;

pub(super) fn template_is_admitted(declaration: &TypeDeclaration) -> Result<bool, Vec<Diagnostic>> {
    #[cfg(test)]
    if use_original_receiver(0) {
        return Ok(original::template_is_admitted(declaration));
    }
    let TypeDeclarationKind::Record { fields } = &declaration.kind else {
        return Ok(false);
    };
    if declaration.type_parameters.is_empty() || fields.is_empty() {
        return Ok(false);
    }
    let parameters = BorrowedNames::new(&declaration.type_parameters)?;
    Ok(fields.iter().all(|field| match &field.ty {
        Type::Never => false,
        Type::I64
        | Type::I32
        | Type::Char
        | Type::U8
        | Type::Usize
        | Type::F32
        | Type::F64
        | Type::Bool
        | Type::Bytes => true,
        Type::Named { name, arguments } => arguments.is_empty() && parameters.contains(name),
        Type::String
        | Type::Str
        | Type::SliceU8
        | Type::StringMap
        | Type::ArrayU8(_)
        | Type::OnceFunction
        | Type::OnceFunctionI64
        | Type::OnceFunctionI64Pair
        | Type::MutFunctionI64
        | Type::Function { .. } => false,
    }))
}

pub(super) fn rewrite_declaration_runtime_cost(
    declaration: &TypeDeclaration,
    target_module: &str,
    caller: &Program,
    programs: &[Program],
    cost: &mut StructuralCost,
) -> Result<(), Vec<Diagnostic>> {
    #[cfg(test)]
    if use_original_receiver(1) {
        return original::rewrite_declaration_runtime_cost(
            declaration,
            target_module,
            caller,
            programs,
            cost,
        );
    }
    let parameters = BorrowedNames::new(&declaration.type_parameters)?;
    visit_types(&declaration.kind, &mut |ty| {
        if !is_parameter(ty, &parameters) {
            super::expected_projection::rewrite_type_runtime_cost(
                ty,
                target_module,
                caller,
                programs,
                cost,
            )?;
        }
        Ok(())
    })?;
    Ok(())
}

pub(super) fn rewrite_declaration_runtime_cost_with_layout(
    declaration: &TypeDeclaration,
    target_module: &str,
    caller: &Program,
    programs: &[Program],
    cost: &mut StructuralCost,
    layout: Option<&super::expected_projection::private_layout::LayoutClosure<'_>>,
) -> Result<(), Vec<Diagnostic>> {
    #[cfg(test)]
    if use_original_receiver(2) {
        return original::rewrite_declaration_runtime_cost_with_layout(
            declaration,
            target_module,
            caller,
            programs,
            cost,
            layout,
        );
    }
    if layout.is_none_or(|l| l.is_empty()) {
        return rewrite_declaration_runtime_cost(
            declaration,
            target_module,
            caller,
            programs,
            cost,
        );
    }
    if !declaration.type_parameters.is_empty()
        || matches!(
            declaration.kind,
            TypeDeclarationKind::Class { .. } | TypeDeclarationKind::Resource { .. }
        )
    {
        return rewrite_declaration_runtime_cost(
            declaration,
            target_module,
            caller,
            programs,
            cost,
        );
    }
    let mut rewrite = |ty: &Type| {
        super::expected_projection::type_rewrite::runtime_cost_with_layout(
            ty,
            target_module,
            caller,
            programs,
            cost,
            layout,
        )
    };
    match &declaration.kind {
        TypeDeclarationKind::Record { fields } => {
            for field in fields {
                rewrite(&field.ty)?;
            }
        }
        TypeDeclarationKind::Variant { cases } => {
            for case in cases {
                for field in &case.fields {
                    rewrite(&field.ty)?;
                }
            }
        }
        TypeDeclarationKind::Class { .. } | TypeDeclarationKind::Resource { .. } => {
            return Err(vec![super::graph_error(
                "SPX-G172",
                "private layout rewriting requires a monomorphic record or result variant",
            )]);
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
    #[cfg(test)]
    if use_original_receiver(3) {
        return original::rewrite_declaration(declaration, target_module, caller, programs);
    }
    let parameters = BorrowedNames::new(&declaration.type_parameters)?;
    visit_types_mut(&mut declaration.kind, &mut |ty| {
        if !is_parameter(ty, &parameters) {
            super::expected_projection::rewrite_type(ty, target_module, caller, programs)?;
        }
        Ok(())
    })?;
    Ok(())
}

pub(super) fn rewrite_declaration_with_layout(
    declaration: &mut TypeDeclaration,
    target_module: &str,
    caller: &Program,
    programs: &[Program],
    layout: Option<&super::expected_projection::private_layout::LayoutClosure<'_>>,
) -> Result<(), Vec<Diagnostic>> {
    #[cfg(test)]
    if use_original_receiver(4) {
        return original::rewrite_declaration_with_layout(
            declaration,
            target_module,
            caller,
            programs,
            layout,
        );
    }
    if layout.is_none_or(|l| l.is_empty()) {
        return rewrite_declaration(declaration, target_module, caller, programs);
    }
    if !declaration.type_parameters.is_empty()
        || matches!(
            declaration.kind,
            TypeDeclarationKind::Class { .. } | TypeDeclarationKind::Resource { .. }
        )
    {
        return rewrite_declaration(declaration, target_module, caller, programs);
    }
    let rewrite = |ty: &mut Type| {
        super::expected_projection::type_rewrite::rewrite_with_layout(
            ty,
            target_module,
            caller,
            programs,
            layout,
        )
    };
    match &mut declaration.kind {
        TypeDeclarationKind::Record { fields } => {
            for field in fields {
                rewrite(&mut field.ty)?;
            }
        }
        TypeDeclarationKind::Variant { cases } => {
            for case in cases {
                for field in &mut case.fields {
                    rewrite(&mut field.ty)?;
                }
            }
        }
        TypeDeclarationKind::Class { .. } | TypeDeclarationKind::Resource { .. } => {
            return Err(vec![super::graph_error(
                "SPX-G172",
                "private layout rewriting requires a monomorphic record or result variant",
            )]);
        }
    }
    Ok(())
}

fn is_parameter(ty: &Type, parameters: &BorrowedNames<'_>) -> bool {
    matches!(ty, Type::Named { name, arguments }
        if arguments.is_empty() && parameters.contains(name))
}

fn visit_types(
    kind: &TypeDeclarationKind,
    visit: &mut impl FnMut(&Type) -> Result<(), Vec<Diagnostic>>,
) -> Result<(), Vec<Diagnostic>> {
    match kind {
        TypeDeclarationKind::Record { fields } => {
            for field in fields {
                visit(&field.ty)?;
            }
        }
        TypeDeclarationKind::Class { fields, methods } => {
            for field in fields {
                visit(&field.ty)?;
            }
            for method in methods {
                for parameter in &method.params {
                    visit(&parameter.ty)?;
                }
                visit(&method.return_type)?;
            }
        }
        TypeDeclarationKind::Variant { cases } => {
            for case in cases {
                for field in &case.fields {
                    visit(&field.ty)?;
                }
            }
        }
        TypeDeclarationKind::Resource { .. } => unreachable!("resource imports are rejected"),
    }
    Ok(())
}

fn visit_types_mut(
    kind: &mut TypeDeclarationKind,
    visit: &mut impl FnMut(&mut Type) -> Result<(), Vec<Diagnostic>>,
) -> Result<(), Vec<Diagnostic>> {
    match kind {
        TypeDeclarationKind::Record { fields } => {
            for field in fields {
                visit(&mut field.ty)?;
            }
        }
        TypeDeclarationKind::Class { fields, methods } => {
            for field in fields {
                visit(&mut field.ty)?;
            }
            for method in methods {
                for parameter in &mut method.params {
                    visit(&mut parameter.ty)?;
                }
                visit(&mut method.return_type)?;
            }
        }
        TypeDeclarationKind::Variant { cases } => {
            for case in cases {
                for field in &mut case.fields {
                    visit(&mut field.ty)?;
                }
            }
        }
        TypeDeclarationKind::Resource { .. } => unreachable!("resource imports are rejected"),
    }
    Ok(())
}

// Lexical test adapters keep the complete old file byte-for-byte; its bodies
// resolve their unchanged workspace helpers through this parent only.
#[cfg(test)]
use super::{expected_projection, graph_error};
#[cfg(test)]
#[path = "generic_type_import/borrowed_tests.rs"]
mod borrowed_tests;
#[cfg(test)]
#[path = "generic_type_import/original.rs"]
mod original;

#[cfg(test)]
thread_local! {
    static ORIGINAL_RECEIVER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static ORIGINAL_CALLS: std::cell::Cell<[usize; 5]> = const { std::cell::Cell::new([0; 5]) };
}
#[cfg(test)]
fn use_original_receiver(operation: usize) -> bool {
    let selected = ORIGINAL_RECEIVER.with(std::cell::Cell::get);
    if selected {
        ORIGINAL_CALLS.with(|calls| {
            let mut observed = calls.get();
            observed[operation] += 1;
            calls.set(observed);
        });
    }
    selected
}
#[cfg(test)]
fn original_receiver_calls() -> [usize; 5] {
    ORIGINAL_CALLS.with(std::cell::Cell::get)
}
#[cfg(test)]
fn with_original_receiver<T>(operation: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            ORIGINAL_RECEIVER.with(|current| current.set(self.0));
        }
    }
    let _restore = Restore(ORIGINAL_RECEIVER.with(|current| current.replace(true)));
    operation()
}

#[path = "generic_type_import/receiving.rs"]
pub(in crate::workspace_graph) mod receiving;
