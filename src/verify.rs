//! Compatibility facade for source-level semantic verification.
//!
//! New compiler stages should consume [`crate::hir`] so resolved identities and
//! validated types remain the shared semantic boundary. This module stays
//! public for callers that need the established parsed-source diagnostics.

use crate::ast::Program;
use crate::diagnostic::Diagnostic;

/// Verify parsed source while preserving the established diagnostic contract.
pub fn verify(program: &Program) -> Vec<Diagnostic> {
    crate::source_verify::verify(program)
}
