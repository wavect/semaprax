//! Original full-field joins with funded mutation paths and scratch builders.
use super::*;
fn moved(binding: &ValidationBinding) -> Option<usize> {
    let nodes =
        crate::cleanup_plan::replay_btree_node_upper::<Vec<PlaceProjection>, Availability>()?
            .checked_add(crate::cleanup_plan::replay_btree_node_upper::<
                Vec<PlaceProjection>,
                (),
            >()?)?;
    let mut bytes = nodes.checked_mul(binding.moved_places.len())?;
    for path in binding.moved_places.keys() {
        bytes = bytes.checked_add(
            path.len()
                .checked_mul(std::mem::size_of::<PlaceProjection>())?,
        )?;
    }
    bytes = bytes.checked_add(
        crate::cleanup_plan::replay_btree_node_upper::<Vec<PlaceProjection>, ()>()?
            .checked_mul(binding.definitely_partial.len())?,
    )?;
    for path in &binding.definitely_partial {
        bytes = bytes.checked_add(
            path.len()
                .checked_mul(std::mem::size_of::<PlaceProjection>())?,
        )?;
    }
    Some(bytes)
}
fn prefixes(binding: &ValidationBinding) -> Option<usize> {
    let node = crate::cleanup_plan::replay_btree_node_upper::<Vec<PlaceProjection>, ()>()?;
    let mut bytes = 0usize;
    for path in binding
        .moved_places
        .keys()
        .chain(binding.definitely_partial.iter())
    {
        let count = path.len().checked_add(1)?;
        // The old candidate builder copies every prefix even when that prefix
        // is a duplicate. Its filtered output reuses vectors but builds a new
        // set. Keep both complete tree directories and every prefix payload.
        let slots = node.checked_mul(count)?.checked_mul(2)?;
        let payload = path
            .len()
            .checked_mul(count)?
            .checked_div(2)?
            .checked_mul(std::mem::size_of::<PlaceProjection>())?;
        bytes = bytes.checked_add(slots)?.checked_add(payload)?;
    }
    Some(bytes)
}
fn join_scratch(left: &ValidationBinding, right: &ValidationBinding) -> Result<(), Diagnostic> {
    let loans = left.active_loans.union(&right.active_loans).count();
    charge::reserve(
        moved(left)
            .and_then(|a| moved(right).and_then(|b| a.checked_add(b)))
            .and_then(|b| {
                prefixes(left)?
                    .checked_add(prefixes(right)?)?
                    .checked_add(b)
            })
            .and_then(|b| {
                crate::cleanup_plan::replay_btree_node_upper::<LoanId, ()>()?
                    .checked_mul(loans)?
                    .checked_add(b)
            }),
    )
}
pub(in crate::hir::validation) fn availability(
    target: &mut dyn Write,
    source: &dyn Read,
    ids: &dyn Ids,
) -> Result<(), Diagnostic> {
    for id in ids.ids() {
        let (Some(before), Some(after)) = (target.get(id), source.get(id)) else {
            continue;
        };
        if before.availability == after.availability
            && before.moved_places == after.moved_places
            && before.definitely_partial == after.definitely_partial
        {
            continue;
        }
        charge::reserve(moved(after))?;
        let target = target.get_mut(id)?.expect("checked existing merge binding");
        target.availability = after.availability;
        target.moved_places.clone_from(&after.moved_places);
        target
            .definitely_partial
            .clone_from(&after.definitely_partial);
    }
    Ok(())
}
pub(in crate::hir::validation) fn borrows(
    target: &mut dyn Write,
    source: &dyn Read,
    ids: &dyn Ids,
) -> Result<(), Diagnostic> {
    for id in ids.ids() {
        let (Some(before), Some(after)) = (target.get(id), source.get(id)) else {
            continue;
        };
        let added = after.active_loans.difference(&before.active_loans).count();
        if added == 0 {
            continue;
        }
        charge::reserve(
            crate::cleanup_plan::replay_btree_node_upper::<LoanId, ()>()
                .and_then(|n| n.checked_mul(added)),
        )?;
        target
            .get_mut(id)?
            .unwrap()
            .active_loans
            .extend(after.active_loans.iter().copied());
    }
    Ok(())
}
pub(in crate::hir::validation) fn conditional(
    target: &mut dyn Write,
    source: &dyn Read,
    ids: &dyn Ids,
) -> Result<(), Diagnostic> {
    for id in ids.ids() {
        let (Some(before), Some(after)) = (target.get(id), source.get(id)) else {
            continue;
        };
        // Empty projection state is already normalized. Equal nonempty
        // state still runs the original prefix normalization, including
        // prefixes added from definitely-partial paths.
        if before == after && before.moved_places.is_empty() && before.definitely_partial.is_empty()
        {
            continue;
        }
        join_scratch(before, after)?;
        let moved_places = HirValidator::join_moved_places(before, after);
        let definitely_partial = HirValidator::join_definitely_partial(before, after);
        let availability = before.availability.join(after.availability);
        // Full canonical builders already ran. Only the fields this join
        // writes authorize a no-op; immutable type/flags need not be cloned.
        if before.availability == availability
            && before.moved_places == moved_places
            && before.definitely_partial == definitely_partial
            && after.active_loans.is_subset(&before.active_loans)
        {
            charge::reserve(Some(0))?;
            continue;
        }
        let target = target.get_mut(id)?.unwrap();
        target.availability = availability;
        target
            .active_loans
            .extend(after.active_loans.iter().copied());
        target.moved_places = moved_places;
        target.definitely_partial = definitely_partial;
    }
    Ok(())
}
pub(in crate::hir::validation) fn branches(
    target: &mut dyn Write,
    left: &dyn Read,
    right: &dyn Read,
    ids: &dyn Ids,
) -> Result<(), Diagnostic> {
    for id in ids.ids() {
        let (Some(before), Some(left), Some(right)) = (target.get(id), left.get(id), right.get(id))
        else {
            continue;
        };
        if before == left
            && before == right
            && before.moved_places.is_empty()
            && before.definitely_partial.is_empty()
        {
            continue;
        }
        join_scratch(left, right)?;
        let availability = left.availability.join(right.availability);
        let active_loans = left
            .active_loans
            .union(&right.active_loans)
            .copied()
            .collect();
        let moved_places = HirValidator::join_moved_places(left, right);
        let definitely_partial = HirValidator::join_definitely_partial(left, right);
        if before.availability == availability
            && before.active_loans == active_loans
            && before.moved_places == moved_places
            && before.definitely_partial == definitely_partial
        {
            charge::reserve(Some(0))?;
            continue;
        }
        let target = target.get_mut(id)?.unwrap();
        target.availability = availability;
        target.active_loans = active_loans;
        target.moved_places = moved_places;
        target.definitely_partial = definitely_partial;
    }
    Ok(())
}

#[cfg(test)]
mod saved;
#[cfg(test)]
mod tests;
