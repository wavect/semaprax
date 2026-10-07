//! False guards cannot consume an outer owner before the next arm is checked.
use super::*;
pub(super) fn unchanged(
    before: &BTreeMap<ValueId, ValidationBinding>,
    after: &BTreeMap<ValueId, ValidationBinding>,
) -> Result<(), Diagnostic> {
    if before.iter().all(|(id, old)| {
        after.get(id).is_some_and(|new| {
            old.availability == new.availability
                && old.moved_places == new.moved_places
                && old.definitely_partial == new.definitely_partial
        })
    }) {
        Ok(())
    } else {
        Err(hir_error(
            "resolved Copy variant guard changes surrounding ownership",
        ))
    }
}
