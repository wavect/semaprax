//! Copy-only full-shape carrier checks; no State owner enters this conversion.
use super::*;
use crate::interpreter::resumable::ResumableChannelValue;
fn shape(declarations: &hir::DeclarationIndex, ty: &ResolvedType) -> bool {
    if hir::is_scalar_resolved_type(ty) {
        return true;
    }
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return false;
    };
    arguments.is_empty()
        && declarations
            .declaration(declaration)
            .is_some_and(|d| d.kind == hir::DeclarationKind::Record)
        && declarations
            .record_fields(declaration)
            .is_some_and(|fields| {
                !fields.is_empty()
                    && fields.len() <= 8
                    && fields.iter().all(|f| hir::is_scalar_resolved_type(&f.ty))
            })
}
pub(crate) fn valid_copy_carrier(
    declarations: &hir::DeclarationIndex,
    ty: &ResolvedType,
    value: &ResumableChannelValue,
) -> bool {
    if !shape(declarations, ty) {
        return false;
    }
    match value {
        ResumableChannelValue::Scalar(v) => hir::is_scalar_resolved_type(ty) && scalar_valid(ty, v),
        ResumableChannelValue::Record {
            declaration,
            fields,
        } => {
            let ResolvedType::Nominal {
                declaration: expected,
                ..
            } = ty
            else {
                return false;
            };
            if expected != declaration {
                return false;
            }
            let Some(canonical) = declarations.record_fields(declaration) else {
                return false;
            };
            canonical.len() == fields.len()
                && canonical
                    .iter()
                    .zip(fields)
                    .all(|(f, v)| scalar_valid(&f.ty, v))
        }
        _ => false,
    }
}
pub(super) fn value_of_copy(
    declarations: &hir::DeclarationIndex,
    ty: &ResolvedType,
    value: &ResumableChannelValue,
) -> Option<Value> {
    if !valid_copy_carrier(declarations, ty, value) {
        return None;
    }
    super::super::value_of_channel(declarations, ty, value, &mut 0)
}
