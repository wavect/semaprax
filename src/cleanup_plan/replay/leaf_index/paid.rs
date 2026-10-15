//! Fallible full backing for the original ordered place-leaf result.
use super::*;

struct Under<'a> {
    flags: &'a [LivenessFlagId],
    output: Vec<LivenessFlagId>,
    next: usize,
}

impl Leaves {
    pub(in crate::cleanup_plan::replay) fn under_with_budget(
        &self,
        function: &ResolvedFunction,
        place: &CleanupPlace,
        budget: &mut ReplayBudget,
    ) -> Result<Vec<LivenessFlagId>, Diagnostic> {
        if !budget.pays_allocations() {
            return Ok(self.under(place));
        }
        const PHASE: &str = "cleanup place leaf result";
        budget.reserve_bytes(function, 0, PHASE)?;
        budget.charge(function, 0, PHASE)?;
        budget.reserve_bytes(
            function,
            // New function/budget argument transport, including caller/receiver overlap.
            2 * std::mem::size_of::<&ResolvedFunction>()
                + 2 * std::mem::size_of::<&mut ReplayBudget>()
                + std::mem::size_of::<Under<'static>>()
                + std::mem::size_of::<Option<&Vec<LivenessFlagId>>>()
                + std::mem::size_of::<Result<Vec<LivenessFlagId>, Diagnostic>>()
                + std::mem::size_of::<Vec<LivenessFlagId>>()
                + std::mem::size_of::<bool>()
                + 2 * std::mem::size_of::<&[LivenessFlagId]>()
                + std::mem::size_of::<&Vec<DeclarationId>>()
                + 2 * std::mem::size_of::<&DeclarationId>()
                + 2 * std::mem::size_of::<
                    std::iter::Zip<
                        std::slice::Iter<'static, DeclarationId>,
                        std::slice::Iter<'static, DeclarationId>,
                    >,
                >()
                + 2 * std::mem::size_of::<std::slice::Iter<'static, DeclarationId>>()
                + std::mem::size_of::<Option<(&DeclarationId, &DeclarationId)>>()
                + std::mem::size_of::<LivenessFlagId>()
                + std::mem::size_of::<usize>(),
            PHASE,
        )?;
        budget.charge(function, 9, PHASE)?;
        let mut state = Under {
            flags: self
                .by_storage
                .get(&place.storage)
                .map_or(&[] as &[LivenessFlagId], Vec::as_slice),
            output: Vec::new(),
            next: 0,
        };
        while state.next < state.flags.len() {
            budget.charge(function, 3, PHASE)?;
            let flag = state.flags[state.next];
            state.next += 1;
            let projections = &self.by_flag[&flag].place.projections;
            budget.charge(function, 1, PHASE)?;
            if place.projections.len() > projections.len() {
                continue;
            }
            let mut matches = true;
            for (left, right) in projections.iter().zip(&place.projections) {
                let units = left
                    .as_str()
                    .len()
                    .min(right.as_str().len())
                    .checked_add(1)
                    .ok_or_else(|| {
                        budget.budget_refusal(function, "place projection work overflow", "")
                    })?;
                budget.charge(function, units, PHASE)?;
                if left != right {
                    matches = false;
                    break;
                }
            }
            if matches {
                if state.output.len() == state.output.capacity() {
                    let units = state.output.len().checked_add(1).ok_or_else(|| {
                        budget.budget_refusal(function, "place result relocation work overflow", "")
                    })?;
                    budget.charge(function, units, PHASE)?;
                }
                budget.reserve_vec(function, &mut state.output, 1, PHASE)?;
                state.output.push(flag);
            }
        }
        Ok(state.output)
    }
}
