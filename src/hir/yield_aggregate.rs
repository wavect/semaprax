//! Issue #296 R20 (docs/RESUMABLE-EFFECTS-CONTINUATION-V1.md §12, blocker
//! (1) of the Agent lifecycle migration assessment): the bounded, flat,
//! non-recursive record or variant admitted as a `yields` request or
//! response type, and as an intermediate value in a `yields`-declaring
//! function's body.
//!
//! Depth is fixed at exactly one level: a record's own fields, or a
//! variant's own case fields, must themselves be admitted Copy scalars --
//! never another record or variant. A field can therefore never reintroduce
//! the enclosing declaration (or any other aggregate), so the shape is
//! non-recursive by construction and needs no separate cycle check, and it
//! is always fully `Copy` (`hir::DeclarationIndex::type_facts` would agree,
//! though this check does not need to consult it: a flat tuple of Copy
//! scalars needs no cleanup-plan exit path for `yield`, exactly like a bare
//! scalar). Owned `Bytes` leaves are deliberately **not** admitted here: a
//! record or variant field of type `Bytes` still fails this check (and
//! falls through to the ordinary non-scalar refusal), reserved for a future
//! increment. `arguments.is_empty()` refuses any generic instantiation.
//!
//! Kept as its own module, rather than inline in `hir::resolve_yield`, so
//! `hir::validation`'s independent re-derivation of this same admission (see
//! [`admits_yield_channel_type`]) shares these exact bound/shape rules
//! instead of drifting out of sync with a second copy of them.

use super::{DeclarationKind, ResolvedType};

pub(super) const MAX_YIELD_AGGREGATE_FIELDS: usize = 8;
pub(super) const MAX_YIELD_AGGREGATE_CASES: usize = 8;

/// `Ok(())` when `ty` is an admitted bounded aggregate; otherwise the exact
/// bound or shape rule it fails, for `resolve_yield`'s `SPX-T307` message.
/// Callers only reach this for a `ResolvedType::Nominal`; every other type
/// keeps the pre-existing non-scalar refusal.
pub(super) fn bounded_aggregate_refusal(
    declarations: &super::DeclarationIndex,
    ty: &ResolvedType,
) -> Result<(), &'static str> {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Err("not a record or variant");
    };
    if !arguments.is_empty() {
        return Err("a generic instantiation is not an admitted bounded aggregate");
    }
    let item = declarations
        .declaration(declaration)
        .ok_or("unresolved declaration")?;
    match item.kind {
        DeclarationKind::Record => {
            let fields = declarations
                .record_fields(declaration)
                .ok_or("unresolved record fields")?;
            if fields.len() > MAX_YIELD_AGGREGATE_FIELDS {
                return Err("a record exceeds the admitted bounded-aggregate field count");
            }
            if fields
                .iter()
                .any(|field| !super::is_scalar_resolved_type(&field.ty))
            {
                return Err(
                    "a record field is not an admitted Copy scalar; nested records, variants, \
                     and owned types (including `Bytes`) are not yet admitted inside a yield \
                     aggregate",
                );
            }
            Ok(())
        }
        DeclarationKind::Variant => {
            let cases = declarations
                .variant_cases(declaration)
                .ok_or("unresolved variant cases")?;
            if cases.len() > MAX_YIELD_AGGREGATE_CASES {
                return Err("a variant exceeds the admitted bounded-aggregate case count");
            }
            if cases
                .iter()
                .any(|case| case.fields.len() > MAX_YIELD_AGGREGATE_FIELDS)
            {
                return Err("a variant case exceeds the admitted bounded-aggregate field count");
            }
            if cases
                .iter()
                .flat_map(|case| &case.fields)
                .any(|field| !super::is_scalar_resolved_type(&field.ty))
            {
                return Err(
                    "a variant case field is not an admitted Copy scalar; nested records, \
                     variants, and owned types (including `Bytes`) are not yet admitted inside \
                     a yield aggregate",
                );
            }
            Ok(())
        }
        _ => Err("only a record or variant is an admitted bounded aggregate"),
    }
}

/// Whether `ty` is an admitted `yields` channel type: an ordinary Copy
/// scalar, or a bounded aggregate per [`bounded_aggregate_refusal`]. Used by
/// `hir::validation`'s re-derivation, which only needs the bool, not the
/// exact refusal reason `hir::resolve_yield` reports at first admission.
pub(super) fn admits_yield_channel_type(
    declarations: &super::DeclarationIndex,
    ty: &ResolvedType,
) -> bool {
    super::is_scalar_resolved_type(ty)
        || (matches!(ty, ResolvedType::Nominal { .. })
            && bounded_aggregate_refusal(declarations, ty).is_ok())
}
