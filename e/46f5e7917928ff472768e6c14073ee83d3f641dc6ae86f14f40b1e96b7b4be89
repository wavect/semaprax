//! A label starts with one target; only a second distinct child owns a tree.
#[cfg(test)]
use super::{
    own_weighted, weighted_successor, Diagnostic, ReplayBudget, ResolvedFunction, SharedWeighted,
};
use std::collections::BTreeMap;
#[cfg(test)]
use std::mem::size_of;

pub(super) enum Targets {
    One { child: usize, weight: u64 },
    Many(BTreeMap<usize, u64>),
}
// Complete prior helpers qualify old receiving boundaries only.
#[cfg(test)]
impl Targets {
    pub(super) fn one(
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
        child: usize,
        weight: u64,
    ) -> Result<Self, Diagnostic> {
        if budget.pays_allocations() {
            budget.reserve_bytes(
                function,
                2 * size_of::<Self>()
                    + size_of::<Result<Self, Diagnostic>>()
                    + size_of::<Option<u64>>()
                    + size_of::<Option<Self>>(),
                "factored singleton target initializer and receiving",
            )?;
        }
        budget.reserve_bytes(function, 0, "factored singleton target authentication")?;
        // Preserve the first route's original zero-plus-weight operation.
        let weight = 0_u64
            .checked_add(weight)
            .expect("zero plus u64 cannot overflow");
        Ok(Self::One { child, weight })
    }
    pub(super) fn promote(
        &mut self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
        child: usize,
        weight: u64,
    ) -> Result<(), Diagnostic> {
        let Self::One {
            child: first,
            weight: total,
        } = self
        else {
            unreachable!("only a singleton promotes")
        };
        assert_ne!(*first, child);
        if budget.pays_allocations() {
            budget.reserve_bytes(
                function,
                size_of::<Result<(), Diagnostic>>()
                    + 2 * size_of::<BTreeMap<usize, u64>>()
                    + size_of::<Self>()
                    + 2 * size_of::<Option<u64>>()
                    + 2 * size_of::<(usize, u64)>(),
                "factored successor target promotion carriers",
            )?;
        }
        budget.reserve_bytes(
            function,
            0,
            "factored successor target promotion authentication",
        )?;
        // BOTH acquisitions precede EITHER tree allocation. No refund/credit.
        budget.reserve_btree_node::<usize, u64>(function, "factored successor target node")?;
        budget.reserve_btree_node::<usize, u64>(function, "factored successor target node")?;
        let mut targets = BTreeMap::new();
        targets.insert(*first, *total);
        let sum = 0_u64
            .checked_add(weight)
            .expect("zero plus u64 cannot overflow");
        targets.insert(child, sum);
        *self = Self::Many(targets);
        Ok(())
    }
    pub(super) fn finish(
        self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<SharedWeighted, Diagnostic> {
        if budget.pays_allocations() {
            budget.reserve_bytes(
                function,
                size_of::<Self>()
                    + size_of::<Result<SharedWeighted, Diagnostic>>()
                    + 2 * size_of::<SharedWeighted>()
                    + size_of::<Vec<(usize, u64)>>()
                    + size_of::<(usize, u64)>()
                    + size_of::<std::iter::Once<(usize, u64)>>(),
                "factored successor target finish receiving",
            )?;
        }
        budget.reserve_bytes(
            function,
            0,
            "factored successor target finish authentication",
        )?;
        match self {
            Self::One { child, weight } => {
                // Same exact one-entry Vec/Rc producer as weighted_successor;
                // no temporary target tree or FromIterator capacity rounding.
                budget.reserve_array::<(usize, u64)>(
                    function,
                    1,
                    "factored weighted successor vector",
                )?;
                let mut state = Vec::with_capacity(1);
                state.extend(std::iter::once((child, weight)));
                own_weighted(function, budget, state)
            }
            Self::Many(targets) => weighted_successor(function, budget, targets),
        }
    }
}
