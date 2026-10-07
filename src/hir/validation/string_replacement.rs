//! Whole String publication after a fully checked owning RHS.
use super::*;
pub(super) fn reopen(
    scope: &mut BTreeMap<ValueId, ValidationBinding>,
    id: &ValueId,
) -> Result<(), Diagnostic> {
    let target = scope
        .get_mut(id)
        .ok_or_else(|| hir_error("String replacement target is absent"))?;
    if target.ty != ResolvedType::String
        || target.ownership != OwnershipMode::Own
        || !target.active_loans.is_empty()
    {
        return Err(hir_error(
            "String replacement requires an unborrowed whole owner",
        ));
    }
    target.availability = Availability::Available;
    target.moved_places.clear();
    target.definitely_partial.clear();
    Ok(())
}
