//! Payload-free variant equality.
//!
//! `a == b` and `a != b` are admitted over one non-generic variant whose
//! every case carries no payload. Such a value is Copy and its whole meaning
//! is the selected case, so equality compares the case tag. The HIR keeps the
//! ordinary `Binary` node; each backend compares the tags it already stores.

use super::ids::DeclarationId;
use super::nodes::{DeclarationKind, ResolvedProgram, ResolvedType};
use super::DeclarationIndex;

impl ResolvedProgram {
    /// Whether `ty` is a non-generic variant whose every case is payload-free:
    /// the only aggregate type `==` and `!=` admit.
    pub fn is_payload_free_variant(&self, ty: &ResolvedType) -> bool {
        payload_free_variant(&self.declarations, ty).is_some()
    }
}

pub(crate) fn payload_free_variant<'a>(
    declarations: &DeclarationIndex,
    ty: &'a ResolvedType,
) -> Option<&'a DeclarationId> {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return None;
    };
    if declarations.declaration(declaration)?.kind != DeclarationKind::Variant
        || !arguments.is_empty()
        || declarations
            .type_parameters(declaration)
            .is_some_and(|parameters| !parameters.is_empty())
    {
        return None;
    }
    let cases = declarations.variant_cases(declaration)?;
    (!cases.is_empty() && cases.iter().all(|case| case.fields.is_empty())).then_some(declaration)
}
