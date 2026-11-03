//! Exact declaration-component refusal facts; never a replacement checked type.
use super::super::Recovery;
use crate::ast::program_source::ProgramSource;
use crate::ast::{Function, Span, Type};
use crate::diagnostic::Diagnostic;
use crate::source_verify::{declared_type::check_declared_type, type_table::TypeTable};
use std::collections::HashSet;

impl<'p> Recovery<'p> {
    /// Recheck only after an existing declaration refusal at this exact owner
    /// component. Full six-field equality authenticates the original receipt.
    pub(in crate::source_verify) fn before_actual_callee_component_refusal(
        &self,
        function: &'p Function,
        parameter: Option<usize>,
        program: &dyn ProgramSource,
        types: &TypeTable<'_>,
        diagnostics: &[Diagnostic],
    ) -> Option<usize> {
        if !self.unknown_declarations {
            return None;
        }
        let (ty, span): (&Type, Span) = match parameter {
            Some(index) => {
                let parameter = function.params.get(index)?;
                (&parameter.ty, parameter.span)
            }
            None => (&function.return_type, function.span),
        };
        if !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-T001" && diagnostic.span == Some(span))
        {
            return None;
        }
        let parameters = function
            .type_parameters
            .iter()
            .map(|parameter| parameter.name.as_str())
            .collect::<HashSet<_>>();
        let mut scratch = Vec::new();
        check_declared_type(program, ty, span, types, &parameters, &mut scratch);
        scratch
            .iter()
            .filter(|refusal| refusal.code == "SPX-T001")
            .find_map(|refusal| {
                diagnostics.iter().position(|original| {
                    original.code == refusal.code
                        && original.severity == refusal.severity
                        && original.message == refusal.message
                        && original.path == refusal.path
                        && original.span == refusal.span
                        && original.help == refusal.help
                })
            })
    }

    /// A refused return is an actual child dependency even when its original
    /// declaration receipt predates this initializer's diagnostic checkpoint.
    pub(in crate::source_verify) fn before_actual_read_callee_refusal(&mut self, origin: usize) {
        self.last_read = Some(origin);
        self.reads += 1;
    }
}

impl<'p> Recovery<'p> {
    pub(in crate::source_verify) fn before_actual_declared_field_refusal(
        &self,
        container: &str,
        field: &crate::ast::FieldDeclaration,
        program: &dyn ProgramSource,
        types: &TypeTable<'_>,
        diagnostics: &[Diagnostic],
    ) -> Option<usize> {
        if !self.unknown_declarations {
            return None;
        }
        // Effective class fields can be borrowed from a cloned parent prefix.
        // Authenticate the complete raw field against its actual declaring owner
        // before using that owner's type parameters. The finite declaration
        // count bounds malformed inheritance cycles without inventing an owner.
        let mut declaration = types.declaration(container)?;
        let mut owner = None;
        for _ in 0..types.declarations.len() {
            let holds_field = match &declaration.kind {
                crate::ast::TypeDeclarationKind::Record { fields }
                | crate::ast::TypeDeclarationKind::Class { fields, .. } => fields.contains(field),
                crate::ast::TypeDeclarationKind::Variant { cases } => {
                    cases.iter().any(|case| case.fields.contains(field))
                }
                crate::ast::TypeDeclarationKind::Resource { .. } => false,
            };
            if holds_field {
                owner = Some(declaration);
                break;
            }
            if !matches!(
                declaration.kind,
                crate::ast::TypeDeclarationKind::Class { .. }
            ) {
                break;
            }
            let Some(Type::Named { name, .. }) = declaration.extends.as_ref() else {
                break;
            };
            let Some(parent) = types.declaration(name) else {
                break;
            };
            declaration = parent;
        }
        let declaration = owner?;
        if !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-T001" && diagnostic.span == Some(field.span))
        {
            return None;
        }
        let parameters = declaration
            .type_parameters
            .iter()
            .map(|parameter| parameter.name.as_str())
            .collect::<HashSet<_>>();
        let mut scratch = Vec::new();
        check_declared_type(
            program,
            &field.ty,
            field.span,
            types,
            &parameters,
            &mut scratch,
        );
        scratch
            .iter()
            .filter(|refusal| refusal.code == "SPX-T001")
            .find_map(|refusal| {
                diagnostics.iter().position(|original| {
                    original.code == refusal.code
                        && original.severity == refusal.severity
                        && original.message == refusal.message
                        && original.path == refusal.path
                        && original.span == refusal.span
                        && original.help == refusal.help
                })
            })
    }
}
