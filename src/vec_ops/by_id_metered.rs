//! Exact finite opcode comparisons before activity; legacy lookup is unchanged.
use super::*;
pub(crate) fn by_id_with_work(
    id: &str,
    work: &mut crate::hir::renewal_probes::Work<'_>,
) -> Result<Option<VecOp>, ()> {
    crate::hir::renewal_probes::units(work, id.len().min(sort_keys::PREFIX.len()) + 1)?;
    if id.starts_with(sort_keys::PREFIX) {
        return Ok(Some(VecOp::SortBy));
    }
    if id.starts_with(field_update::PREFIX) {
        crate::hir::renewal_probes::units(work, field_update::PREFIX.len() + 1)?;
        return Ok(Some(VecOp::UpdateField));
    }
    // Borrowed field uniqueness is deliberately unclassified here: every
    // renewal-profile caller admits only owner-reopening or Push operations, so
    // its None result is equivalent and existing lookup work stays unchanged.
    const KEYS: [(&str, VecOp); 15] = [
        (string_slot::ID, VecOp::StrAt),
        (string_slot::BYTES_ID, VecOp::StrAt),
        (WITH_CAPACITY_ID, VecOp::WithCapacity),
        (PUSH_ID, VecOp::Push),
        (LEN_ID, VecOp::Len),
        (CAPACITY_ID, VecOp::Capacity),
        (GET_ID, VecOp::Get),
        (RESERVE_EXACT_ID, VecOp::ReserveExact),
        (SET_ID, VecOp::Set),
        (CLEAR_ID, VecOp::Clear),
        (SORT_ID, VecOp::Sort),
        (CLONE_AT_ID, VecOp::CloneAt),
        (REPLACE_ID, VecOp::Replace),
        (RESERVE_OWNED_ID, VecOp::ReserveOwned),
        (SORT_OWNED_ID, VecOp::SortOwned),
    ];
    for (key, op) in KEYS {
        crate::hir::renewal_probes::units(work, id.len().min(key.len()) + 1)?;
        if id == key {
            return Ok(Some(op));
        }
    }
    Ok(None)
}
