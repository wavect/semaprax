//! Original full-field joins with funded mutation paths and scratch builders.
use super::*;
mod moved_direct;
#[cfg(test)]
mod partial_retain;
mod prefix_stream {
    //! Ordered borrowed prefix census for the paid scope join.
    use super::*;
    use std::collections::{btree_map, btree_set};
    use std::mem::size_of;

    type Path = Vec<PlaceProjection>;
    type Set = BTreeSet<Path>;
    type Slice<'a> = &'a [PlaceProjection];
    type Head<'a> = Option<Slice<'a>>;
    type Moved<'a> = btree_map::Keys<'a, Path, Availability>;
    type Partial<'a> = btree_set::Iter<'a, Path>;
    type Paid = Result<(), Refusal>;

    enum Refusal {
        Layout,
        Budget,
    }

    impl Refusal {
        fn diagnostic(self) -> Diagnostic {
            hir_error_static(match self {
                Self::Layout => "ownership scope checkpoint layout overflowed",
                Self::Budget => "ownership scope checkpoint exceeds the active validation budget",
            })
        }
    }

    fn reserve(bytes: Option<usize>) -> Paid {
        let Some(bytes) = bytes else {
            crate::bounded_output::refuse_active_required(usize::MAX);
            return Err(Refusal::Layout);
        };
        if crate::bounded_output::reserve_active_required(bytes) {
            Ok(())
        } else {
            Err(Refusal::Budget)
        }
    }

    const OBSERVERS: usize = {
        #[cfg(test)]
        let stage = {
            use crate::bounded_output::stage_debits::{Operation, OperationGuard};
            2 * (size_of::<Operation>() + size_of::<OperationGuard>())
        };
        #[cfg(not(test))]
        let stage = 0;
        #[cfg(feature = "unstable-workflow-profiling")]
        let exclusive = {
            use crate::bounded_output::exclusive_debits::{Operation, OperationGuard};
            2 * (size_of::<Operation>() + size_of::<OperationGuard>())
        };
        #[cfg(not(feature = "unstable-workflow-profiling"))]
        let exclusive = 0;
        stage + exclusive
    };

    // Owning callers pay before joining. Four iterator acquisitions and terminal
    // reads, the array of borrowed heads, preceding path, caller/callee loans,
    // callback environment, returned set and fallible receiving roles.
    pub(super) const CONTROL: usize = OBSERVERS
        + 4 * size_of::<Moved<'_>>()
        + 4 * size_of::<Partial<'_>>()
        + 2 * size_of::<[Head<'_>; 4]>()
        + 2 * size_of::<Head<'_>>()
        + 4 * size_of::<(&ValidationBinding, &ValidationBinding)>()
        + 2 * size_of::<(&ValidationBinding, &ValidationBinding, &mut Set)>()
        + 2 * size_of::<Set>()
        + 2 * size_of::<Result<Set, Diagnostic>>()
        + 4 * size_of::<Paid>()
        + 4 * size_of::<Option<usize>>()
        + 2 * size_of::<[usize; 4]>()
        + 4 * size_of::<&BTreeMap<Path, Availability>>()
        + 4 * size_of::<&Set>()
        + 8 * size_of::<bool>()
        + 12 * size_of::<usize>();

    // Each original input advances its own iterator. Prepay the per-step fee
    // request here as well: there can be no more than one step per path plus EOF.
    const ADVANCE: usize = 2 * size_of::<&mut Moved<'_>>()
        + 2 * size_of::<&mut Partial<'_>>()
        + 2 * size_of::<Option<&Path>>()
        + 2 * size_of::<&Path>()
        + 2 * size_of::<Head<'_>>()
        + 2 * size_of::<Option<usize>>()
        + 4 * size_of::<Paid>()
        + 4 * size_of::<usize>()
        + 2 * size_of::<bool>();

    // One minimum scan, including all head inspections, the equal-head mask,
    // and the selected and previous path, LCP bound and prefix range roles.
    const STEP: usize = 4 * size_of::<Head<'_>>()
        + 8 * size_of::<Slice<'_>>()
        + 2 * size_of::<Head<'_>>()
        + 6 * size_of::<std::cmp::Ordering>()
        + 4 * size_of::<u8>()
        + size_of::<[usize; 8]>()
        + 8 * size_of::<bool>()
        + 4 * size_of::<Paid>()
        + 4 * size_of::<Option<usize>>();

    const MIN_CONTROL: usize =
        2 * size_of::<Option<usize>>() + 2 * size_of::<Paid>() + 4 * size_of::<usize>();

    // Slice ordering may inspect every pair in the shorter projection path.
    // Fund both borrowed operands and the comparison's result before the
    // library comparator can inspect that pair.
    const MIN_PAIR: usize = 4 * size_of::<&PlaceProjection>() + 2 * size_of::<std::cmp::Ordering>();

    const LCP_PAIR: usize =
        4 * size_of::<&PlaceProjection>() + 3 * size_of::<usize>() + 2 * size_of::<bool>();

    // Per distinct emitted prefix: the borrowed slice, callback invocation,
    // predicate and spill, fee calculation, and possible owned insert transport.
    // Actual retained node and vector backing are reserved inside the callback.
    const PREFIX: usize = 6 * size_of::<Slice<'_>>()
        + 4 * size_of::<&ValidationBinding>()
        + 4 * size_of::<Availability>()
        + 6 * size_of::<bool>()
        + 4 * size_of::<usize>()
        + 2 * size_of::<Option<usize>>()
        + 4 * size_of::<Paid>()
        + 2 * size_of::<(&mut Set, Path)>();

    pub(super) fn join(
        left: &ValidationBinding,
        right: &ValidationBinding,
    ) -> Result<Set, Diagnostic> {
        #[cfg(test)]
        let _operation = crate::bounded_output::stage_debits::operation_guard(
            crate::bounded_output::stage_debits::Operation::ScopeCheckpoint,
        );
        #[cfg(feature = "unstable-workflow-profiling")]
        let _exclusive_operation = crate::bounded_output::exclusive_debits::operation_guard(
            crate::bounded_output::exclusive_debits::Operation::ScopeCheckpoint,
        );
        let mut result = Set::new();
        let outcome = visit(left, right, |prefix| {
            if HirValidator::place_availability(left, prefix) == Availability::Moved
                && HirValidator::place_availability(right, prefix) == Availability::Moved
            {
                reserve(
                    crate::cleanup_plan::replay_btree_node_upper::<Path, ()>().and_then(|node| {
                        prefix
                            .len()
                            .checked_mul(size_of::<PlaceProjection>())?
                            .checked_add(node)
                    }),
                )?;
                result.insert(prefix.to_vec());
            }
            Ok(())
        });
        if let Err(refusal) = outcome {
            return Err(refusal.diagnostic());
        }
        Ok(result)
    }

    fn visit(
        left: &ValidationBinding,
        right: &ValidationBinding,
        mut visit: impl FnMut(Slice<'_>) -> Paid,
    ) -> Paid {
        // An inventory that starts empty never contributes a head. The first
        // occupied head needs no comparison with a previously selected path.
        let inventories = usize::from(!left.moved_places.is_empty())
            + usize::from(!right.moved_places.is_empty())
            + usize::from(!left.definitely_partial.is_empty())
            + usize::from(!right.definitely_partial.is_empty());
        let step = STEP + inventories.saturating_sub(1) * MIN_CONTROL;
        reserve(
            left.moved_places
                .len()
                .checked_add(right.moved_places.len())
                .and_then(|count| count.checked_add(left.definitely_partial.len()))
                .and_then(|count| count.checked_add(right.definitely_partial.len()))
                .and_then(|count| count.checked_add(1))
                .and_then(|rows| rows.checked_mul(ADVANCE)),
        )?;
        let mut moved = [left.moved_places.keys(), right.moved_places.keys()];
        let mut partial = [
            left.definitely_partial.iter(),
            right.definitely_partial.iter(),
        ];
        let mut heads = [
            moved[0].next().map(Vec::as_slice),
            moved[1].next().map(Vec::as_slice),
            partial[0].next().map(Vec::as_slice),
            partial[1].next().map(Vec::as_slice),
        ];
        let mut previous: Head<'_> = None;
        loop {
            reserve(Some(step))?;
            let mut selected: Head<'_> = None;
            let mut sources = 0u8;
            for (index, &head) in heads.iter().enumerate() {
                let Some(path) = head else { continue };
                let order = if let Some(least) = selected {
                    reserve(path.len().min(least.len()).checked_mul(MIN_PAIR))?;
                    path.cmp(least)
                } else {
                    std::cmp::Ordering::Less
                };
                match order {
                    std::cmp::Ordering::Less => {
                        selected = Some(path);
                        sources = 1 << index;
                    }
                    std::cmp::Ordering::Equal => sources |= 1 << index,
                    std::cmp::Ordering::Greater => {}
                }
            }
            let Some(path) = selected else {
                return Ok(());
            };
            // A lexicographic prefix describes a contiguous interval. The
            // predecessor therefore covers every prefix already emitted.
            let first = if let Some(prior) = previous {
                let limit = prior.len().min(path.len());
                reserve(limit.checked_mul(LCP_PAIR))?;
                let mut common = 0;
                while common < limit && prior[common] == path[common] {
                    common += 1;
                }
                common + 1
            } else {
                0
            };
            if let Some(remaining) = path.len().checked_sub(first) {
                reserve(
                    remaining
                        .checked_add(1)
                        .and_then(|count| count.checked_mul(PREFIX)),
                )?;
                let mut length = first;
                while length <= path.len() {
                    visit(&path[..length])?;
                    length += 1;
                }
            }
            previous = Some(path);
            for (index, head) in heads.iter_mut().enumerate() {
                if sources & (1 << index) != 0 {
                    *head = if index < moved.len() {
                        moved[index].next().map(Vec::as_slice)
                    } else {
                        partial[index - moved.len()].next().map(Vec::as_slice)
                    };
                }
            }
        }
    }

    #[cfg(test)]
    #[allow(dead_code, reason = "complete immediate scope receiver and payer")]
    pub(super) mod before;
    #[cfg(test)]
    mod tests;
    #[cfg(test)]
    pub(super) use tests::{before_guard, before_selected, historical_merge};
}
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
fn moved_join_output(binding: &ValidationBinding) -> Option<usize> {
    // Only the final BTreeMap node remains; the owned candidate BTreeSet
    // was physically removed by the borrowed ordered merge. Existing path
    // payload upper, partial prefixes and all output/COW payments remain.
    let nodes =
        crate::cleanup_plan::replay_btree_node_upper::<Vec<PlaceProjection>, Availability>()?;
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
fn join_scratch(left: &ValidationBinding, right: &ValidationBinding) -> Result<(), Diagnostic> {
    charge::reserve(Some(moved_direct::CONTROL + prefix_stream::CONTROL))?;
    let count = left
        .moved_places
        .len()
        .checked_add(right.moved_places.len());
    charge::reserve(count.and_then(|n| moved_direct::ROW.checked_mul(n)))?;
    let loans = left.active_loans.union(&right.active_loans).count();
    charge::reserve(
        moved_join_output(left)
            .and_then(|a| moved_join_output(right).and_then(|b| a.checked_add(b)))
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
    #[cfg(test)]
    if prefix_stream::before_selected() {
        return prefix_stream::before::conditional(target, source, ids);
    }
    #[cfg(test)]
    if partial_retain::before_selected() {
        return partial_retain::before::conditional(target, source, ids);
    }
    #[cfg(test)]
    if moved_direct::before_selected() {
        return moved_direct::before::conditional(target, source, ids);
    }
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
        let moved_places = moved_direct::join(before, after);
        let definitely_partial = prefix_stream::join(before, after)?;
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
    #[cfg(test)]
    if prefix_stream::before_selected() {
        return prefix_stream::before::branches(target, left, right, ids);
    }
    #[cfg(test)]
    if partial_retain::before_selected() {
        return partial_retain::before::branches(target, left, right, ids);
    }
    #[cfg(test)]
    if moved_direct::before_selected() {
        return moved_direct::before::branches(target, left, right, ids);
    }
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
        let moved_places = moved_direct::join(left, right);
        let definitely_partial = prefix_stream::join(left, right)?;
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
