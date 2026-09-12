//! Move/borrow-scope merge and join helpers shared by every branching form
//! (`if`, `match`, loops) that `validate_expr_iterative` replays: merging a
//! single successor scope back into its parent, and joining two or more
//! conditional scopes into one baseline before that merge.
use super::*;

impl HirValidator<'_> {
    pub(super) fn merge_availability(
        target: &mut BTreeMap<ValueId, ValidationBinding>,
        source: &BTreeMap<ValueId, ValidationBinding>,
        ids: &[ValueId],
    ) {
        for id in ids {
            if let (Some(target), Some(source)) = (target.get_mut(id), source.get(id)) {
                target.availability = source.availability;
                target.moved_places.clone_from(&source.moved_places);
                target
                    .definitely_partial
                    .clone_from(&source.definitely_partial);
            }
        }
    }

    pub(super) fn merge_lexical_borrows(
        target: &mut BTreeMap<ValueId, ValidationBinding>,
        source: &BTreeMap<ValueId, ValidationBinding>,
        ids: &[ValueId],
    ) {
        for id in ids {
            if let (Some(target), Some(source)) = (target.get_mut(id), source.get(id)) {
                target
                    .active_loans
                    .extend(source.active_loans.iter().copied());
            }
        }
    }

    pub(super) fn join_conditional(
        baseline: &mut BTreeMap<ValueId, ValidationBinding>,
        conditional: &BTreeMap<ValueId, ValidationBinding>,
        ids: &[ValueId],
    ) {
        for id in ids {
            if let (Some(baseline), Some(conditional)) = (baseline.get_mut(id), conditional.get(id))
            {
                let moved_places = Self::join_moved_places(baseline, conditional);
                let definitely_partial = Self::join_definitely_partial(baseline, conditional);
                baseline.availability = baseline.availability.join(conditional.availability);
                baseline
                    .active_loans
                    .extend(conditional.active_loans.iter().copied());
                baseline.moved_places = moved_places;
                baseline.definitely_partial = definitely_partial;
            }
        }
    }

    pub(super) fn join_branches(
        target: &mut BTreeMap<ValueId, ValidationBinding>,
        then_scope: &BTreeMap<ValueId, ValidationBinding>,
        else_scope: &BTreeMap<ValueId, ValidationBinding>,
        ids: &[ValueId],
    ) {
        for id in ids {
            if let (Some(target), Some(then_value), Some(else_value)) =
                (target.get_mut(id), then_scope.get(id), else_scope.get(id))
            {
                target.availability = then_value.availability.join(else_value.availability);
                target.active_loans = then_value
                    .active_loans
                    .union(&else_value.active_loans)
                    .copied()
                    .collect();
                target.moved_places = Self::join_moved_places(then_value, else_value);
                target.definitely_partial = Self::join_definitely_partial(then_value, else_value);
            }
        }
    }

    pub(super) fn place_availability(
        binding: &ValidationBinding,
        requested: &[PlaceProjection],
    ) -> Availability {
        if binding.availability != Availability::Available {
            return binding.availability;
        }
        let mut maybe_moved = false;
        for (moved, state) in &binding.moved_places {
            if path_is_prefix(moved, requested) || path_is_prefix(requested, moved) {
                if *state == Availability::Moved {
                    return Availability::Moved;
                }
                maybe_moved = true;
            }
        }
        if binding
            .definitely_partial
            .iter()
            .any(|partial| path_is_prefix(requested, partial))
        {
            return Availability::Moved;
        }
        if maybe_moved {
            Availability::MaybeMoved
        } else {
            Availability::Available
        }
    }

    fn join_moved_places(
        left: &ValidationBinding,
        right: &ValidationBinding,
    ) -> BTreeMap<Vec<PlaceProjection>, Availability> {
        left.moved_places
            .keys()
            .chain(right.moved_places.keys())
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .filter_map(|path| {
                let left = left
                    .moved_places
                    .get(&path)
                    .copied()
                    .unwrap_or(Availability::Available);
                let right = right
                    .moved_places
                    .get(&path)
                    .copied()
                    .unwrap_or(Availability::Available);
                let state = left.join(right);
                (state != Availability::Available).then_some((path, state))
            })
            .collect()
    }

    fn join_definitely_partial(
        left: &ValidationBinding,
        right: &ValidationBinding,
    ) -> BTreeSet<Vec<PlaceProjection>> {
        let mut candidates = BTreeSet::new();
        for path in left
            .moved_places
            .keys()
            .chain(right.moved_places.keys())
            .chain(left.definitely_partial.iter())
            .chain(right.definitely_partial.iter())
        {
            for length in 0..=path.len() {
                candidates.insert(path[..length].to_vec());
            }
        }
        candidates
            .into_iter()
            .filter(|path| {
                Self::place_availability(left, path) == Availability::Moved
                    && Self::place_availability(right, path) == Availability::Moved
            })
            .collect()
    }
}
