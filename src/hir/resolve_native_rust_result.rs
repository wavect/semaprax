//! Native Rust import-result lowering, isolated from the iterative resolver's
//! size-bounded frame engine.

use crate::ast::Span;
use crate::diagnostic::Diagnostic;

use super::{DeclarationKind, ResolvedImportResultKind, ResolvedType, Resolver};

impl Resolver<'_> {
    pub(super) fn resolve_native_rust_result(
        &self,
        result: &crate::ast::ImportResult,
        span: Span,
    ) -> Result<(ResolvedImportResultKind, ResolvedType), Diagnostic> {
        Ok(match result {
            crate::ast::ImportResult::Unit => (ResolvedImportResultKind::Unit, ResolvedType::Unit),
            crate::ast::ImportResult::I64 => (ResolvedImportResultKind::I64, ResolvedType::I64),
            crate::ast::ImportResult::Bool => (ResolvedImportResultKind::Bool, ResolvedType::Bool),
            crate::ast::ImportResult::OwnedString => {
                (ResolvedImportResultKind::OwnedString, ResolvedType::String)
            }
            crate::ast::ImportResult::OwnedOptionString => (
                ResolvedImportResultKind::OwnedOptionString,
                self.resolve_type(&result.value_type(), span)?,
            ),
            crate::ast::ImportResult::OwnedResultStringI64 => (
                ResolvedImportResultKind::OwnedResultStringI64,
                self.resolve_type(&result.value_type(), span)?,
            ),
            crate::ast::ImportResult::OwnedResultStringOptionI64 => (
                ResolvedImportResultKind::OwnedResultStringOptionI64,
                self.resolve_type(&result.value_type(), span)?,
            ),
            crate::ast::ImportResult::ResultI64I64 => (
                ResolvedImportResultKind::ResultI64I64,
                self.resolve_type(&result.value_type(), span)?,
            ),
            crate::ast::ImportResult::OwnedResource { .. } => {
                let ty = self.resolve_type(&result.value_type(), span)?;
                let ResolvedType::Nominal {
                    declaration,
                    arguments,
                } = &ty
                else {
                    return Err(self.error(
                        "SPX-H006",
                        "native Rust owned result did not resolve to a resource",
                        span,
                    ));
                };
                if !arguments.is_empty()
                    || self
                        .declarations
                        .declaration(declaration)
                        .is_none_or(|item| item.kind != DeclarationKind::Resource)
                {
                    return Err(self.error(
                        "SPX-H006",
                        "native Rust owned result did not resolve to a resource",
                        span,
                    ));
                }
                (
                    ResolvedImportResultKind::OwnedResource {
                        resource: declaration.clone(),
                    },
                    ty,
                )
            }
        })
    }
}
