//! Content-ordered merge lookup with no cloned ownership or continuation keys.
use super::*;

pub(in crate::cleanup_plan::replay) const DIRECTORY_CONTROLS: usize = std::mem::size_of::<Vec<usize>>()
    + 2 * std::mem::size_of::<Result<(), Diagnostic>>()
    + 2 * std::mem::size_of::<(&ResolvedFunction, &mut ReplayBudget, usize, &str)>()
    // Subsequent per-input acquisition call, before its count/Result forms.
    + 2 * std::mem::size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>()
    + 2 * std::mem::size_of::<Result<(), Diagnostic>>()
    + 2 * std::mem::size_of::<usize>();

pub(in crate::cleanup_plan::replay) fn compare_paths(
    left: &ExprSkeletonPath,
    right: &ExprSkeletonPath,
) -> std::cmp::Ordering {
    use std::cmp::Ordering::Equal;
    match left.owned_source.cmp(&right.owned_source) {
        Equal => match left.failed.cmp(&right.failed) {
            Equal => match left.residual.cmp(&right.residual) {
                Equal => left.continuation.cmp(&right.continuation),
                other => other,
            },
            other => other,
        },
        other => other,
    }
}

impl SkeletonWork<'_, '_> {
    /// Factored replay only: merge paths whose cleanup state is identical
    /// into one path carrying the union of their observation sequences. Every
    /// later skeleton step depends only on that state, so the union denotes
    /// exactly the paths enumeration would produce.
    pub(in crate::cleanup_plan::replay) fn merged_paths(
        &mut self,
        paths: Vec<ExprSkeletonPath>,
    ) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
        if !self.budget.merge_paths || paths.len() < 2 {
            return Ok(paths);
        }
        self.charge(paths.len(), "factored path-state merge")?;
        type Group = (ExprSkeletonPath, Vec<Observations>);
        // The directory stores indices, never references into relocatable groups.
        // Its header is new storage and is acquired before construction.
        self.budget.reserve_bytes(
            self.function,
            DIRECTORY_CONTROLS,
            "factored borrowed merge directory",
        )?;
        super::merge_search::initialize!(self, search);
        let mut order = Vec::<usize>::new();
        let mut groups = Vec::<Group>::new();
        for mut path in paths {
            if !self.lexical_merge_normal(&path) {
                self.budget
                    .reserve_vec(self.function, &mut groups, 1, "isolated lexical path")?;
                groups.push((path, Vec::new()));
                continue;
            }
            // The retained owner removes the closure/compound Result entirely.
            // Original materialization work was charged before any new scratch.
            super::merge_search::search!(
                self,
                search,
                order,
                super::merge_search::PATH_INDEX,
                super::merge_search::PATH_COMPARE,
                compare_paths(&groups[search.group].0, &path)
            );
            if search.found {
                let index = order[search.position];
                self.budget.reserve_vec(
                    self.function,
                    &mut groups[index].1,
                    1,
                    "factored merge members",
                )?;
                groups[index].1.push(path.observations);
            } else {
                self.budget.reserve_vec(
                    self.function,
                    &mut order,
                    1,
                    "factored borrowed merge indices",
                )?;
                self.budget
                    .reserve_vec(self.function, &mut groups, 1, "factored merge groups")?;
                self.budget.reserve_array::<Observations>(
                    self.function,
                    1,
                    "factored first merge member",
                )?;
                let observations = std::mem::take(&mut path.observations);
                order.insert(search.position, groups.len());
                groups.push((path, vec![observations]));
            }
        }
        self.budget.reserve_array::<ExprSkeletonPath>(
            self.function,
            groups.len(),
            "factored merged paths",
        )?;
        let mut result = Vec::with_capacity(groups.len());
        for (mut path, mut members) in groups {
            if members.len() > 1 {
                path.observations = if self.budget.selected() {
                    self.union_observations(members)?
                } else {
                    self.ordinary_union(members)?
                };
            } else if let Some(observations) = members.pop() {
                // A singleton keeps its original prefix and Vec backing.
                path.observations = observations;
            }
            result.push(path);
        }
        Ok(result)
    }

    pub(in crate::cleanup_plan::replay) fn merged_suffixes<'s>(
        &mut self,
        paths: &'s [ExprSkeletonPath],
    ) -> Result<std::borrow::Cow<'s, [ExprSkeletonPath]>, Diagnostic> {
        if !self.budget.merge_paths || paths.len() < 2 {
            return Ok(std::borrow::Cow::Borrowed(paths));
        }
        self.budget.reserve_array::<ExprSkeletonPath>(
            self.function,
            paths.len(),
            "factored suffix path copies",
        )?;
        let mut owned = Vec::with_capacity(paths.len());
        for path in paths {
            owned.push(self.clone_payload(path, "factored suffix path backing")?);
        }
        Ok(std::borrow::Cow::Owned(self.merged_paths(owned)?))
    }

    /// Call states additionally merge only with identical commit lists.
    pub(in crate::cleanup_plan::replay) fn merged_call_states(
        &mut self,
        states: Vec<CallSkeletonState>,
    ) -> Result<Vec<CallSkeletonState>, Diagnostic> {
        if !self.budget.merge_paths || states.len() < 2 {
            return Ok(states);
        }
        self.charge(states.len(), "factored call-state merge")?;
        type Group = (CallSkeletonState, Vec<Observations>);
        // The directory stores indices, never references into relocatable groups.
        // Its header is new storage and is acquired before construction.
        self.budget.reserve_bytes(
            self.function,
            DIRECTORY_CONTROLS,
            "factored borrowed merge directory",
        )?;
        super::merge_search::initialize!(self, search);
        let mut order = Vec::<usize>::new();
        let mut groups = Vec::<Group>::new();
        for (mut path, commits) in states {
            if !self.lexical_merge_normal(&path)
                || (self.function.cleanup_plan.lexical_control.is_some() && !commits.is_empty())
            {
                self.budget.reserve_vec(
                    self.function,
                    &mut groups,
                    1,
                    "isolated lexical call state",
                )?;
                groups.push(((path, commits), Vec::new()));
                continue;
            }
            // The retained owner removes the closure/compound Result entirely.
            // Original materialization work was charged before any new scratch.
            super::merge_search::search!(
                self,
                search,
                order,
                super::merge_search::CALL_INDEX,
                super::merge_search::CALL_COMPARE,
                match compare_paths(&groups[search.group].0 .0, &path) {
                    std::cmp::Ordering::Equal => groups[search.group].0 .1.cmp(&commits),
                    other => other,
                }
            );
            if search.found {
                let index = order[search.position];
                self.budget.reserve_vec(
                    self.function,
                    &mut groups[index].1,
                    1,
                    "factored call merge members",
                )?;
                groups[index].1.push(path.observations);
            } else {
                self.budget.reserve_vec(
                    self.function,
                    &mut order,
                    1,
                    "factored borrowed merge indices",
                )?;
                self.budget.reserve_vec(
                    self.function,
                    &mut groups,
                    1,
                    "factored call merge groups",
                )?;
                self.budget.reserve_array::<Observations>(
                    self.function,
                    1,
                    "factored first call member",
                )?;
                let observations = std::mem::take(&mut path.observations);
                order.insert(search.position, groups.len());
                groups.push(((path, commits), vec![observations]));
            }
        }
        self.budget.reserve_array::<CallSkeletonState>(
            self.function,
            groups.len(),
            "factored merged call states",
        )?;
        let mut result = Vec::with_capacity(groups.len());
        for ((mut path, commits), mut members) in groups {
            if members.len() > 1 {
                path.observations = if self.budget.selected() {
                    self.union_observations(members)?
                } else {
                    self.ordinary_union(members)?
                };
            } else if let Some(observations) = members.pop() {
                // A singleton keeps its original prefix and Vec backing.
                path.observations = observations;
            }
            result.push((path, commits));
        }
        Ok(result)
    }
}
